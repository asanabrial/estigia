//! The address vocabulary both messaging verbs share.
//!
//! A person names a recipient by what they can see — the runtime, the
//! repository, the issue, the short name Estigia gave the session — and never
//! has to list first and parse a run id out of the answer. Resolution reads the
//! registry at the moment of the call and over live rows only, so a session
//! that registered a second ago is found and a dead one is not offered.

use std::path::Path;

use serde_json::{Value, json};

use crate::cli::sessions::{Listed, basename};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

pub(crate) const CLOSEST: usize = 5;

pub(crate) struct Narrowed {
    pub(crate) matched: Vec<Listed>,
    pub(crate) closest: Vec<Listed>,
    pub(crate) live: usize,
}

impl Narrowed {
    pub(crate) fn lines(rows: &[Listed]) -> String {
        rows.iter()
            .map(|row| format!("  {}", row.line()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The one row an address names, or the refusal that says why it names none
/// or several. Both doors go through here, so they cannot disagree.
pub(crate) fn one(narrowed: Narrowed, said: &str) -> Result<Listed, Refusal> {
    match narrowed.matched.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(no_live_session(said, &narrowed)),
        _ => Err(recipient_ambiguous(said, &narrowed)),
    }
}

pub(crate) fn no_live_session(address: &str, narrowed: &Narrowed) -> Refusal {
    let closest = if narrowed.closest.is_empty() {
        "no session of any runtime is live on this machine".to_owned()
    } else {
        format!(
            "the closest live sessions are:\n{}",
            Narrowed::lines(&narrowed.closest)
        )
    };
    Refusal::not_started(
        "message-no-live-session",
        format!(
            "nobody live matches `{address}` — {} of the sessions on this machine are live; \
             {closest}",
            narrowed.live
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "one of the lines above, or an address such as `opencode`, `opencode@estigia`, \
             `#96` or `estigia#96`",
        ),
    )
}

pub(crate) fn recipient_ambiguous(address: &str, narrowed: &Narrowed) -> Refusal {
    Refusal::not_started(
        "message-recipient-ambiguous",
        format!(
            "{} live sessions match `{address}`; nothing was sent:\n{}",
            narrowed.matched.len(),
            Narrowed::lines(&narrowed.matched)
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the name at the start of the line you meant, or a narrower address such as \
             `opencode@estigia`",
        ),
    )
}

/// A bare number has always meant an issue number, so it keeps meaning one —
/// at both doors, which is why the CLI reaches it here rather than by itself.
pub(crate) fn spoken(said: &str) -> String {
    let digits = said.strip_prefix('#').unwrap_or(said);
    if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return format!("#{digits}");
    }
    said.to_owned()
}

fn matches_repo(entry: &Listed, wanted: &str) -> bool {
    entry.repo_dir.as_deref().is_some_and(|dir| {
        basename(dir) == wanted
            || dir.ends_with(&format!("/{wanted}"))
            || crate::paths::same_directory(Path::new(dir), Path::new(wanted))
    })
}

fn issue_of(entry: &Listed) -> Option<&crate::identity::Identity> {
    entry.issue.as_ref()
}

fn names_issue(entry: &Listed, issue: &str) -> bool {
    matches!(
        issue_of(entry),
        Some(crate::identity::Identity::Number(number)) if number.to_string() == issue
    )
}

fn known_runtime(rows: &[Listed], word: &str) -> bool {
    rows.iter().any(|row| row.runtime == word)
}

fn live_in(every: &[Listed], repo_dir: Option<&str>) -> Vec<Listed> {
    every
        .iter()
        .filter(|entry| entry.status == "live")
        .filter(|entry| {
            repo_dir.is_none_or(|wanted| {
                entry.repo_dir.as_deref().is_some_and(|held| {
                    crate::paths::same_directory(Path::new(held), Path::new(wanted))
                })
            })
        })
        .cloned()
        .collect()
}

fn in_worktree(entry: &Listed, wanted: &str) -> bool {
    [entry.worktree.as_deref(), entry.repo_dir.as_deref()]
        .into_iter()
        .flatten()
        .any(|held| {
            basename(held) == wanted
                || crate::paths::same_directory(Path::new(held), Path::new(wanted))
        })
}

/// The live sessions a group address names: [`resolve_in`]'s vocabulary, plus the words that
/// only make sense for several sessions at once.
///
/// `all` and `idle` (a live session holding no issue) take an optional leading `@`; `repo:<name>`
/// and `worktree:<name or path>` name the checkout a session runs in. Anything else is [`resolve_in`]'s, so a group
/// is never a vocabulary the single-recipient verbs lack.
pub(crate) fn group(
    root: &Path,
    address: &str,
    repo_dir: Option<&str>,
) -> Result<Narrowed, Refusal> {
    let word = address.trim();
    let bare = word.strip_prefix('@').unwrap_or(word);
    let every = crate::cli::sessions::entries(root)?;
    let live = live_in(&every, repo_dir);
    let matched: Vec<Listed> = match bare {
        "all" => live.clone(),
        "idle" => live
            .iter()
            .filter(|entry| issue_of(entry).is_none())
            .cloned()
            .collect(),
        _ => match (bare.strip_prefix("repo:"), bare.strip_prefix("worktree:")) {
            (Some(repo), _) => live
                .iter()
                .filter(|entry| matches_repo(entry, repo.trim()))
                .cloned()
                .collect(),
            (_, Some(worktree)) => live
                .iter()
                .filter(|entry| in_worktree(entry, worktree.trim()))
                .cloned()
                .collect(),
            _ => return resolve_in(root, word, repo_dir),
        },
    };
    let closest = live
        .iter()
        .filter(|entry| !matched.iter().any(|found| found.run_id == entry.run_id))
        .take(CLOSEST)
        .cloned()
        .collect();
    Ok(Narrowed {
        matched,
        closest,
        live: live.len(),
    })
}

pub(crate) fn resolve(root: &Path, address: &str) -> Result<Narrowed, Refusal> {
    resolve_in(root, address, None)
}

/// [`resolve`], narrowed to the sessions a directory holds.
///
/// `repo_dir` is compared as a directory, not as text: a trailing separator or
/// a symlink is the same checkout, and a session that registered no directory
/// is not in any repository and so matches none.
pub(crate) fn resolve_in(
    root: &Path,
    address: &str,
    repo_dir: Option<&str>,
) -> Result<Narrowed, Refusal> {
    let every = crate::cli::sessions::entries(root)?;
    let live = live_in(&every, repo_dir);
    let address = address.trim();
    let explicit_runtime = address.strip_prefix("runtime:");
    let word = explicit_runtime.unwrap_or(address);

    let bare = explicit_runtime.is_none() && !word.contains('#') && !word.contains('@');
    if bare {
        // A run id is unique by construction; a short name is unique only
        // within one repository, so every row carrying it is a candidate and
        // the caller refuses several rather than picking the first.
        if let Some(named) = every.iter().find(|entry| entry.run_id == word) {
            return Ok(Narrowed {
                matched: vec![named.clone()],
                closest: Vec::new(),
                live: live.len(),
            });
        }
        let named: Vec<Listed> = every
            .iter()
            .filter(|entry| entry.name.as_deref() == Some(word))
            .cloned()
            .collect();
        if !named.is_empty() {
            return Ok(Narrowed {
                matched: named,
                closest: Vec::new(),
                live: live.len(),
            });
        }
    }

    let mut wanted_runtime: Option<String> = None;
    let mut wanted_repos: Vec<String> = Vec::new();
    let mut wanted_issue: Option<String> = None;
    let mut wanted_name: Option<String> = None;

    if let Some(runtime) = explicit_runtime {
        wanted_runtime = Some(runtime.trim().to_owned());
    } else if let Some((repo, issue)) = word.split_once('#') {
        wanted_issue = Some(issue.trim().to_owned());
        if !repo.trim().is_empty() {
            wanted_repos.push(repo.trim().to_owned());
        }
    } else if let Some((runtime, repo)) = word.split_once('@') {
        wanted_runtime = Some(runtime.trim().to_owned());
        wanted_repos.push(repo.trim().to_owned());
    } else if known_runtime(&every, word) {
        wanted_runtime = Some(word.to_owned());
    } else {
        wanted_name = Some(word.to_owned());
    }

    let matched: Vec<Listed> = live
        .iter()
        .filter(|entry| {
            wanted_runtime
                .as_deref()
                .is_none_or(|runtime| entry.runtime == runtime)
                && wanted_repos.iter().all(|repo| matches_repo(entry, repo))
                && wanted_issue
                    .as_deref()
                    .is_none_or(|issue| names_issue(entry, issue))
                && wanted_name
                    .as_deref()
                    .is_none_or(|name| entry.name.as_deref() == Some(name) || entry.run_id == name)
        })
        .cloned()
        .collect();

    let mut closest: Vec<Listed> = live
        .iter()
        .filter(|entry| !matched.iter().any(|found| found.run_id == entry.run_id))
        .cloned()
        .collect();
    closest.sort_by_key(|entry| {
        let same = wanted_runtime
            .as_deref()
            .is_some_and(|runtime| entry.runtime == runtime)
            || wanted_repos.iter().any(|repo| matches_repo(entry, repo));
        (
            !same,
            entry.silent_for.unwrap_or(u64::MAX),
            entry.run_id.clone(),
        )
    });
    closest.truncate(CLOSEST);

    Ok(Narrowed {
        matched,
        closest,
        live: live.len(),
    })
}

pub(crate) fn around(root: &Path) -> Result<Value, Refusal> {
    let every = crate::cli::sessions::entries(root)?;
    let live: Vec<Value> = every
        .iter()
        .filter(|entry| entry.status == "live")
        .map(|entry| {
            let mut row = json!({
                "name": entry.spoken(),
                "run_id": entry.run_id,
                "runtime": entry.runtime,
                "repo": entry.repo_dir.as_deref().map(basename),
                "issue": entry.issue,
                "state": entry.state,
                "silent_for": entry.silent_for,
                "waiting": entry.waiting,
                "build": entry.build.as_deref().filter(|build| *build != crate::build_info::VERSION),
            });
            if let Some(pane) = &entry.pane {
                row["pane"] = json!(pane);
            }
            if !entry.handoffs.is_empty() {
                row["handoffs"] = json!(entry.handoffs);
            }
            row
        })
        .collect();
    let behind: Vec<String> = every
        .iter()
        .filter(|entry| entry.status == "live")
        .filter_map(|entry| {
            entry.build.as_deref().and_then(|build| {
                (build != crate::build_info::VERSION)
                    .then(|| format!("{} runs {build}", entry.spoken()))
            })
        })
        .collect();
    Ok(json!({
        "live": live,
        "live_count": live.len(),
        "of": every.len(),
        "build": crate::build_info::VERSION,
        "skew": behind,
        "stale_after": crate::cli::sessions::STALE_AFTER_SECONDS,
    }))
}
