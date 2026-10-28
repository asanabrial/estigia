use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::sessions::{
    CURSOR_TTL_SECONDS, DEFAULT_LIMIT, MAX_SELECTOR_BYTES, PAGE_VERSION, cursor_expired,
    cursor_mismatch, cursor_of, cursor_store_replaced, cursor_token, limit_too_large,
    selector_too_large,
};
use crate::harness::session;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, FeedRow, FeedWindow, MESSAGE, TRANSITION};

pub(crate) use super::sessions::MAX_LIMIT;

const DEFAULT_SINCE: &str = "24h";
const BODY_CHARACTERS: usize = 200;

pub(crate) const CANNOT_SEE: &[&str] = &[
    "a merge, unless a command re-read it from the tracker: the store keeps no merge",
    "a consent prompt: a question put to a person is not stored",
    "a turn that ended: no hook records it",
    "another machine: this is one local store",
    "a run on a runtime with no gate (zcode, deepseek-harness, agents) or a pre-tool-only gate \
     (cursor) leaves no lifecycle events, and silence there is not quiet",
    "a gate decision older than 30 days, and any state change made before transitions were kept",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Selectors {
    repo: Option<String>,
    runtime: Option<String>,
    run: Option<String>,
    issue: Option<u64>,
    kind: Option<String>,
    since: Option<String>,
    bodies: bool,
}

impl Selectors {
    fn of(arguments: &Value) -> Self {
        let field = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Self {
            repo: field("repo"),
            runtime: field("runtime"),
            run: field("run"),
            issue: arguments.get("issue").and_then(Value::as_u64),
            kind: field("kind"),
            since: field("since"),
            bodies: arguments
                .get("bodies")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    fn over_the_bound(&self) -> Option<(&'static str, usize)> {
        [
            ("repo", &self.repo),
            ("runtime", &self.runtime),
            ("run", &self.run),
            ("kind", &self.kind),
            ("since", &self.since),
        ]
        .into_iter()
        .find_map(|(name, value)| {
            value
                .as_ref()
                .filter(|value| value.len() > MAX_SELECTOR_BYTES)
                .map(|value| (name, value.len()))
        })
    }
}

type Key = (u64, u8, i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Cursor {
    version: u64,
    selectors: Selectors,
    taken_at: u64,
    since_at: u64,
    last: Key,
    store: String,
}

#[derive(Serialize, Clone)]
struct Event {
    at: u64,
    time: String,
    repo: Option<String>,
    issue: Option<u64>,
    run_id: Option<String>,
    runtime: Option<String>,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip)]
    key: Key,
}

pub(super) struct Page {
    taken_at: u64,
    since_at: u64,
    limit: u64,
    matched: usize,
    next_cursor: Option<String>,
    events: Vec<Event>,
}

impl Page {
    fn answer(&self) -> Value {
        serde_json::json!({
            "version": PAGE_VERSION,
            "taken_at": self.taken_at,
            "since": self.since_at,
            "limit": self.limit,
            "returned": self.events.len(),
            "matched": self.matched,
            "more": self.next_cursor.is_some(),
            "next_cursor": self.next_cursor,
            "scope": "the local store of this machine",
            "cannot_see": CANNOT_SEE,
            "events": self.events,
        })
    }
}

pub(super) fn clock_unreadable() -> Refusal {
    Refusal::unavailable(
        "clock-unreadable",
        "the clock could not be read, so no window of the feed could be taken",
        Resolution::no_command(NoCommandReason::WorldAction, "a working system clock"),
    )
}

pub(super) fn store_unreadable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "feed-store-unreadable",
        format!("the local store could not be read for the feed: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the feed again; a missing feed is not an empty one",
        ),
    )
}

pub(super) fn repo_unknown(repo: &str) -> Refusal {
    Refusal::not_started(
        "feed-repo-unknown",
        format!("the local store holds no repository named `{repo}`, so nothing was listed"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a repository as `owner/name`, spelled as the store holds it",
        ),
    )
}

pub(super) fn since_malformed(text: &str) -> Refusal {
    Refusal::not_started(
        "feed-since-malformed",
        format!(
            "`since` is `{text}`, which is neither a duration like `90m`, `6h`, `2d` nor a UTC time; nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a duration such as `6h`, or a time such as `2026-10-21T08:00:00Z`",
        ),
    )
}

fn since_of(text: &str, now: u64) -> Result<u64, Refusal> {
    let relative = text.len() > 1 && text.is_char_boundary(text.len() - 1);
    if relative {
        let (digits, unit) = text.split_at(text.len() - 1);
        let unit = match unit {
            "m" => Some(60),
            "h" => Some(3_600),
            "d" => Some(86_400),
            _ => None,
        };
        if let (Ok(count), Some(unit)) = (digits.parse::<u64>(), unit) {
            return Ok(now.saturating_sub(count.saturating_mul(unit)));
        }
    }
    crate::transport::ownership::parse_stamp(text)
        .map(|stamp| u64::try_from(stamp).unwrap_or_default())
        .ok_or_else(|| since_malformed(text))
}

pub(crate) fn utc(at: u64) -> String {
    let days = i64::try_from(at / 86_400).unwrap_or_default();
    let rest = at % 86_400;
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn runtime_of(run_id: &str) -> String {
    match session::run_id_shape(run_id) {
        session::RunIdShape::Named { runtime } | session::RunIdShape::Nameless { runtime } => {
            runtime.to_owned()
        }
        _ => "unknown".to_owned(),
    }
}

fn event_of(row: FeedRow, bodies: bool) -> Event {
    let held = |issue: Option<u64>| issue.map_or_else(String::new, |issue| format!("#{issue} "));
    let preview = match row.source {
        TRANSITION => format!(
            "{}{} -> {}",
            held(row.issue),
            row.from.as_deref().unwrap_or("none"),
            row.to.as_deref().unwrap_or("none")
        ),
        MESSAGE => format!("to {}", row.to.as_deref().unwrap_or("an unknown recipient")),
        _ => format!("{}{}", held(row.issue), row.kind),
    };
    let body = (bodies && !row.body.is_empty()).then(|| {
        let text: String = row.body.chars().take(BODY_CHARACTERS).collect();
        text
    });
    Event {
        time: utc(row.at),
        runtime: row.run_id.as_deref().map(runtime_of),
        key: (row.at, row.source, row.id),
        at: row.at,
        repo: row.repo,
        issue: row.issue,
        run_id: row.run_id,
        kind: row.kind,
        from: row.from,
        to: row.to,
        preview,
        body,
    }
}

fn store_failure(failure: crate::transport::Failure) -> Refusal {
    store_unreadable(&failure.detail())
}

pub(crate) fn page(arguments: &Value, root: &Path, now: Option<u64>) -> Result<Value, Refusal> {
    one_page(arguments, root, now).map(|page| page.answer())
}

pub(super) fn one_page(arguments: &Value, root: &Path, now: Option<u64>) -> Result<Page, Refusal> {
    let now = now.ok_or_else(clock_unreadable)?;
    let selectors = Selectors::of(arguments);
    if let Some((name, bytes)) = selectors.over_the_bound() {
        return Err(selector_too_large(name, bytes));
    }
    let limit = match arguments.get("limit").and_then(Value::as_u64) {
        Some(limit) if limit > MAX_LIMIT => return Err(limit_too_large(limit)),
        Some(limit) => limit,
        None => DEFAULT_LIMIT,
    };
    let path = store::beside(root);
    let store_id = store::store_identity(&path);
    let (taken_at, since_at, after) = match arguments.get("cursor").and_then(Value::as_str) {
        Some(token) => {
            let cursor: Cursor = cursor_of(token)?;
            if cursor.version != PAGE_VERSION {
                return Err(cursor_mismatch("version"));
            }
            if cursor.selectors != selectors {
                return Err(cursor_mismatch("selectors"));
            }
            if cursor.store != store_id {
                return Err(cursor_store_replaced());
            }
            if now.saturating_sub(cursor.taken_at) > CURSOR_TTL_SECONDS {
                return Err(cursor_expired());
            }
            (cursor.taken_at, cursor.since_at, Some(cursor.last))
        }
        None => (
            now,
            since_of(selectors.since.as_deref().unwrap_or(DEFAULT_SINCE), now)?,
            None,
        ),
    };

    let mut events: Vec<Event> = match store::open_for_read(&path).map_err(store_failure)? {
        Some(opened) => {
            if let Some(repo) = selectors.repo.as_deref()
                && !opened.repository_known(repo).map_err(store_failure)?
            {
                return Err(repo_unknown(repo));
            }
            opened
                .feed_rows(&FeedWindow {
                    since: since_at,
                    until: taken_at,
                    repo: selectors.repo.as_deref(),
                    issue: selectors.issue,
                    run: selectors.run.as_deref(),
                    bodies: selectors.bodies,
                })
                .map_err(store_failure)?
                .into_iter()
                .map(|row| event_of(row, selectors.bodies))
                .collect()
        }
        None => {
            if let Some(repo) = selectors.repo.as_deref() {
                return Err(repo_unknown(repo));
            }
            Vec::new()
        }
    };
    events.retain(|event| {
        selectors
            .kind
            .as_deref()
            .is_none_or(|kind| kind == event.kind)
            && selectors
                .runtime
                .as_deref()
                .is_none_or(|runtime| event.runtime.as_deref() == Some(runtime))
    });
    events.sort_by_key(|event| event.key);
    let matched = events.len();
    let start = after.map_or(0, |last| events.partition_point(|event| event.key <= last));
    let mut page: Vec<Event> = events
        .into_iter()
        .skip(start)
        .take(limit as usize)
        .collect();
    let next_cursor = (matched > start + page.len())
        .then(|| page.last())
        .flatten()
        .map(|last| {
            cursor_token(&Cursor {
                version: PAGE_VERSION,
                selectors: selectors.clone(),
                taken_at,
                since_at,
                last: last.key,
                store: store_id.clone(),
            })
        });
    page.shrink_to_fit();
    Ok(Page {
        taken_at,
        since_at,
        limit,
        matched,
        next_cursor,
        events: page,
    })
}

fn line(event: &Event) -> String {
    let place = match (&event.repo, event.issue) {
        (Some(repo), Some(issue)) => format!("{repo}#{issue}"),
        (Some(repo), None) => repo.clone(),
        _ => "-".to_owned(),
    };
    let who = event.run_id.as_deref().unwrap_or("-");
    let mut text = format!(
        "{}  {}  {}  {}  {}",
        event.time, place, who, event.kind, event.preview
    );
    if let Some(body) = &event.body {
        text.push_str(&format!("\n    {}", body.replace('\n', " ")));
    }
    text
}

pub(super) fn feed(json: bool, arguments: &Value) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let answer = one_page(arguments, &root, session::now_seconds())?;
    if json {
        say!("{}", answer.answer());
        return Ok(());
    }
    for event in &answer.events {
        say!("{}", line(event));
    }
    match answer.next_cursor.as_deref() {
        Some(cursor) => say!(
            "{} of {} event(s) since {}; more remain — the next cursor is {cursor}",
            answer.events.len(),
            answer.matched,
            utc(answer.since_at)
        ),
        None => say!(
            "{} of {} event(s) since {}; nothing more remains",
            answer.events.len(),
            answer.matched,
            utc(answer.since_at)
        ),
    }
    say!("this feed cannot see:");
    for blind in CANNOT_SEE {
        say!("  - {blind}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
