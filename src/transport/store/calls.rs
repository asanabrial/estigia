use std::cell::RefCell;

use super::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Graphql,
    Rest,
    Other,
}

impl Kind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Graphql => "graphql",
            Self::Rest => "rest",
            Self::Other => "other",
        }
    }
}

const VALUE_FLAGS: &[&str] = &[
    "-X",
    "--method",
    "-f",
    "--raw-field",
    "-F",
    "--field",
    "-H",
    "--header",
    "-q",
    "--jq",
    "-t",
    "--template",
    "--input",
    "--hostname",
    "--cache",
    "-p",
    "--preview",
];

pub(crate) fn classify(args: &[&str]) -> Option<Kind> {
    if args.first() != Some(&"gh") {
        return None;
    }
    Some(match args.get(1).copied() {
        Some("api") => {
            let mut rest = args[2..].iter();
            let mut endpoint = None;
            while let Some(arg) = rest.next() {
                if VALUE_FLAGS.contains(arg) {
                    rest.next();
                } else if !arg.starts_with('-') {
                    endpoint = Some(*arg);
                    break;
                }
            }
            if endpoint == Some("graphql") {
                Kind::Graphql
            } else {
                Kind::Rest
            }
        }
        Some("issue" | "pr" | "project") => Kind::Graphql,
        _ => Kind::Other,
    })
}

struct Tally {
    operation: String,
    graphql: u32,
    rest: u32,
    other: u32,
}

thread_local! {
    static SCOPE: RefCell<Option<Tally>> = const { RefCell::new(None) };
    #[cfg(test)]
    static TEST_PATH: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

pub(crate) fn note(args: &[&str]) {
    let Some(kind) = classify(args) else { return };
    SCOPE.with(|scope| {
        if let Some(tally) = scope.borrow_mut().as_mut() {
            match kind {
                Kind::Graphql => tally.graphql += 1,
                Kind::Rest => tally.rest += 1,
                Kind::Other => tally.other += 1,
            }
        }
    });
}

pub(crate) fn counting<T>(operation: &str, body: impl FnOnce() -> T) -> T {
    let outer = SCOPE.with(|scope| {
        scope.replace(Some(Tally {
            operation: operation.to_owned(),
            graphql: 0,
            rest: 0,
            other: 0,
        }))
    });
    let answer = body();
    if let Some(tally) = SCOPE.with(|scope| scope.replace(outer)) {
        record(&tally);
    }
    answer
}

fn counting_path() -> Option<std::path::PathBuf> {
    #[cfg(test)]
    {
        TEST_PATH.with(|path| path.borrow().clone())
    }
    #[cfg(not(test))]
    {
        super::default_path(None).ok()
    }
}

#[cfg(test)]
pub(super) fn named_path() -> Option<std::path::PathBuf> {
    TEST_PATH.with(|path| path.borrow().clone())
}

#[cfg(test)]
pub(crate) fn count_into<T>(path: &std::path::Path, body: impl FnOnce() -> T) -> T {
    TEST_PATH.with(|slot| slot.replace(Some(path.to_path_buf())));
    let answer = body();
    TEST_PATH.with(|slot| slot.replace(None));
    answer
}

const COUNTER_WAIT: std::time::Duration = std::time::Duration::from_millis(200);

fn record(tally: &Tally) {
    let rows = [
        (Kind::Graphql, tally.graphql),
        (Kind::Rest, tally.rest),
        (Kind::Other, tally.other),
    ];
    if rows.iter().all(|(_, count)| *count == 0) {
        return;
    }
    let Some(path) = counting_path() else { return };
    let Ok(store) = Store::open_waiting(&path, COUNTER_WAIT) else {
        return;
    };
    let at = crate::harness::session::now_seconds().unwrap_or_default();
    let Ok(transaction) = rusqlite::Transaction::new_unchecked(
        store.connection(),
        rusqlite::TransactionBehavior::Immediate,
    ) else {
        return;
    };
    for (kind, count) in rows.into_iter().filter(|(_, count)| *count > 0) {
        if transaction
            .execute(
                "INSERT INTO calls (operation, kind, count, at) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![tally.operation, kind.as_str(), count, super::int(at)],
            )
            .is_err()
        {
            return;
        }
    }
    let _ = transaction.commit();
}
