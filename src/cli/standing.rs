//! `estigia where <ref>`: the one answer to where an issue stands.
//!
//! Every field is read separately and carries its own `unavailable` reason when it could not
//! be, so a failed read is never a quiet empty one. What it reads and what it cannot is in
//! [`docs/where.md`](../../docs/where.md).

use std::path::Path;

use serde_json::{Value, json};

use super::feed::utc;
use crate::harness::session;
use crate::identity::{Identity, Reference};
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::scope::{self, Places, Prefer, Scope};
use crate::transport::store::{
    self, FeedRow, FeedWindow, Ledger, MESSAGE, OutboxState, Repo, Store, TRANSITION,
};
use crate::transport::{Context, Failure};

const RECENT: usize = 5;
const PREVIEW: usize = 200;
const LINEAR: &str = "the Linear binding reads an issue's title, description and state type only";

/// What the tracker says about an issue and a pull request.
pub(crate) trait Tracker {
    /// The REST issue object: `state` and `html_url`.
    fn issue(&self, repo: &Repo, number: u64) -> Result<Value, String>;
    /// `number`, `url`, `state`, `headRefOid` and `baseRefOid` of a pull request.
    fn pull(&self, repo: &Repo, number: u64) -> Result<Value, String>;
}

struct Gh;

impl Gh {
    fn read(args: &[&str]) -> Result<Value, String> {
        crate::transport::gh_json(args, None)
            .map_err(|failure| why(&failure))?
            .ok_or_else(|| "gh answered nothing".to_owned())
    }
}

impl Tracker for Gh {
    fn issue(&self, repo: &Repo, number: u64) -> Result<Value, String> {
        Self::read(&[
            "api",
            &format!("repos/{}/issues/{number}", scope::spelled(repo)),
        ])
    }

    fn pull(&self, repo: &Repo, number: u64) -> Result<Value, String> {
        Self::read(&[
            "pr",
            "view",
            &number.to_string(),
            "-R",
            &scope::spelled(repo),
            "--json",
            "number,url,state,headRefOid,baseRefOid",
        ])
    }
}

pub(crate) struct Inputs<'a> {
    pub(crate) context: &'a Context,
    pub(crate) root: &'a Path,
    pub(crate) now: Option<u64>,
    pub(crate) tracker: &'a dyn Tracker,
}

fn why(failure: &Failure) -> String {
    match failure {
        Failure::Stop(payload) | Failure::ConfigDefect(payload) => {
            let field = |name: &str| {
                payload
                    .get(name)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            };
            format!("{}: {}", field("reason"), field("action"))
        }
        other => other.detail(),
    }
}

fn section(read: Result<Value, String>) -> Value {
    read.unwrap_or_else(|reason| json!({ "unavailable": reason }))
}

fn refusal_of(failure: &Failure) -> Refusal {
    let answer = crate::harness::tracker::Answer {
        code: failure.code(),
        body: Some(failure.envelope()),
    };
    crate::harness::tracker::translate(&answer, "where")
        .unwrap_or_else(|| unreachable!("a transport failure never carries exit code zero"))
}

pub(super) fn malformed(detail: &str) -> Refusal {
    Refusal::not_started(
        "where-reference-malformed",
        format!("{detail}; nothing was read"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`owner/name#N`, an issue number, or a tracker key such as `ENG-123`",
        ),
    )
}

fn unavailable_everywhere(reason: &str) -> Value {
    json!({ "unavailable": reason })
}

fn linear(key: &str, taken_at: u64) -> Value {
    let mut answer = json!({
        "reference": key,
        "taken_at": taken_at,
        "time": utc(taken_at),
        "scope": { "tracker": "linear", "issue": key },
    });
    for field in FIELDS {
        answer[field] = unavailable_everywhere(LINEAR);
    }
    answer
}

const FIELDS: [&str; 10] = [
    "issue",
    "workflow",
    "transitions",
    "holders",
    "pull_request",
    "review",
    "tasks",
    "blocked_until",
    "messages",
    "outbox",
];

fn recent(mut rows: Vec<FeedRow>, kind: u8) -> Vec<FeedRow> {
    rows.retain(|row| row.source == kind);
    rows.sort_by_key(|row| (row.at, row.id));
    rows.dedup_by_key(|row| row.id);
    let skip = rows.len().saturating_sub(RECENT);
    rows.split_off(skip)
}

fn window<'a>(
    until: u64,
    repo: Option<&'a str>,
    issue: Option<u64>,
    run: Option<&'a str>,
) -> FeedWindow<'a> {
    FeedWindow {
        since: 0,
        until,
        repo,
        issue,
        run,
        bodies: true,
    }
}

fn transitions(store: &Store, spelled: &str, number: u64, until: u64) -> Result<Value, String> {
    let rows = store
        .feed_rows(&window(until, Some(spelled), Some(number), None))
        .map_err(|failure| why(&failure))?;
    let rows: Vec<Value> = recent(rows, TRANSITION)
        .into_iter()
        .map(|row| json!({ "at": row.at, "time": utc(row.at), "from": row.from, "to": row.to, "run_id": row.run_id }))
        .collect();
    Ok(json!({ "rows": rows }))
}

struct Holder {
    run_id: String,
    row: Value,
}

fn holders(root: &Path, scope: &Scope) -> Result<Vec<Holder>, String> {
    let every = crate::cli::sessions::entries(root).map_err(|refusal| refusal.message)?;
    Ok(every
        .into_iter()
        .filter(|entry| entry.issue == Some(Identity::Number(scope.number)))
        .filter(|entry| {
            [entry.repo_dir.as_deref(), entry.worktree.as_deref()]
                .into_iter()
                .flatten()
                .find_map(|dir| scope::repo_at(Path::new(dir)))
                .as_ref()
                == Some(&scope.repo)
        })
        .map(|entry| {
            let branch = entry.worktree.as_deref().and_then(|tree| {
                crate::transport::run(
                    &["git", "branch", "--show-current"],
                    Some(Path::new(tree)),
                    crate::transport::How::tolerated(),
                )
                .ok()
                .filter(|out| out.status == 0 && !out.stdout.trim().is_empty())
                .map(|out| out.stdout.trim().to_owned())
            });
            Holder {
                row: json!({
                    "run_id": entry.run_id,
                    "runtime": entry.runtime,
                    "status": entry.status,
                    "last_seen": entry.last_seen,
                    "silent_for": entry.silent_for,
                    "worktree": entry.worktree,
                    "branch": branch,
                }),
                run_id: entry.run_id,
            }
        })
        .collect())
}

fn messages(store: &Store, held: &[Holder], until: u64) -> Result<Value, String> {
    let mut found = Vec::new();
    for holder in held {
        found.extend(
            store
                .feed_rows(&window(until, None, None, Some(&holder.run_id)))
                .map_err(|failure| why(&failure))?,
        );
    }
    let rows: Vec<Value> = recent(found, MESSAGE)
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id, "at": row.at, "time": utc(row.at), "from": row.run_id, "to": row.to,
                "preview": row.body.chars().take(PREVIEW).collect::<String>(),
            })
        })
        .collect();
    Ok(json!({ "rows": rows }))
}

fn outbox(store: &Store, repo: &Repo, number: u64) -> Result<Value, String> {
    let rows: Vec<Value> = store
        .outbox(repo)
        .map_err(|failure| why(&failure))?
        .into_iter()
        .filter(|row| row.issue == number && row.state != OutboxState::Sent)
        .map(|row| {
            json!({
                "seq": row.seq, "kind": row.kind.as_str(), "state": row.state.as_str(),
                "enqueued_at": row.enqueued_at, "attempted_at": row.attempted_at,
                "detail": row.detail, "status": "queued, not landed",
            })
        })
        .collect();
    Ok(json!({ "rows": rows }))
}

fn tasks(store: &Store, repo: &Repo, number: u64) -> Result<Value, String> {
    let rows = store.tasks(repo, number).map_err(|failure| why(&failure))?;
    let done = rows.iter().filter(|row| row.done).count();
    Ok(json!({
        "done": done,
        "total": rows.len(),
        "open": rows.iter().filter(|row| !row.done).map(|row| json!({"id": row.id, "text": row.text})).collect::<Vec<_>>(),
    }))
}

fn review(
    path: &Path,
    repo: &Repo,
    number: u64,
) -> Result<Option<crate::transport::claim::Judged>, String> {
    let comments = Ledger::read_timeline_at(path, repo, number).map_err(|failure| why(&failure))?;
    crate::transport::claim::review_judged(&comments).map_err(|failure| why(&failure))
}

fn from_store(
    inputs: &Inputs<'_>,
    scope: &Scope,
    until: u64,
) -> [(&'static str, Result<Value, String>); 7] {
    let path = store::beside(inputs.root);
    let spelled = scope::spelled(&scope.repo);
    let opened = match store::open_for_read(&path) {
        Ok(Some(opened)) => Ok(opened),
        Ok(None) => Err("the local store does not exist yet, so nothing was read".to_owned()),
        Err(failure) => Err(why(&failure)),
    }
    .and_then(|opened| match opened.holds_repo(&scope.repo) {
        Ok(true) => Ok(opened),
        Ok(false) => Err(format!("the local store holds no repository {spelled}")),
        Err(failure) => Err(why(&failure)),
    });
    let with = |read: &dyn Fn(&Store) -> Result<Value, String>| match &opened {
        Ok(store) => read(store),
        Err(reason) => Err(reason.clone()),
    };
    [
        (
            "workflow",
            with(
                &|store| match store.recorded_state(&scope.repo, scope.number) {
                    Ok(Some(Some(state))) => Ok(json!({ "state": state })),
                    Ok(Some(None)) => Err(format!(
                        "the store holds {spelled}#{} with no state recorded",
                        scope.number
                    )),
                    Ok(None) => Err(format!(
                        "the store holds no issue {} in {spelled}",
                        scope.number
                    )),
                    Err(failure) => Err(why(&failure)),
                },
            ),
        ),
        (
            "transitions",
            with(&|store| transitions(store, &spelled, scope.number, until)),
        ),
        (
            "tasks",
            with(&|store| tasks(store, &scope.repo, scope.number)),
        ),
        (
            "blocked_until",
            with(&|store| {
                store
                    .blocked_clauses(&scope.repo, scope.number)
                    .map(|clauses| json!({ "clauses": clauses }))
                    .map_err(|failure| why(&failure))
            }),
        ),
        (
            "outbox",
            with(&|store| outbox(store, &scope.repo, scope.number)),
        ),
        (
            "holders",
            with(&|_| {
                holders(inputs.root, scope).map(|held| json!({ "rows": held.iter().map(|h| h.row.clone()).collect::<Vec<_>>() }))
            }),
        ),
        (
            "messages",
            with(&|store| {
                holders(inputs.root, scope).and_then(|held| messages(store, &held, until))
            }),
        ),
    ]
}

fn pull_request(
    tracker: &dyn Tracker,
    repo: &Repo,
    judged: &Result<Option<crate::transport::claim::Judged>, String>,
) -> (Result<Value, String>, Option<String>) {
    let number = match judged {
        Ok(Some(found)) => found.receipt.pr,
        Ok(None) => {
            return (
                Err("no publication is recorded, so the store names no pull request".to_owned()),
                None,
            );
        }
        Err(reason) => {
            return (
                Err(format!(
                    "the pull request is named by the review receipt: {reason}"
                )),
                None,
            );
        }
    };
    match tracker.pull(repo, number) {
        Ok(pull) => {
            let text = |key: &str| pull.get(key).and_then(Value::as_str).map(str::to_owned);
            let head = text("headRefOid");
            (
                Ok(json!({
                    "number": number, "url": text("url"), "state": text("state"),
                    "head": head, "base": text("baseRefOid"),
                })),
                head,
            )
        }
        Err(reason) => (
            Err(format!(
                "the pull request could not be read from the tracker: {reason}"
            )),
            None,
        ),
    }
}

/// Where one issue stands, as one answer.
pub(crate) fn answer(reference: &str, inputs: &Inputs<'_>) -> Result<Value, Refusal> {
    let taken_at = inputs.now.ok_or_else(super::feed::clock_unreadable)?;
    let parsed = Reference::parse(reference).map_err(|detail| malformed(&detail))?;
    if let Reference::Local(Identity::Key(key)) = &parsed {
        return Ok(linear(key, taken_at));
    }
    let path = store::beside(inputs.root);
    let scope = scope::resolve(
        inputs.context,
        &parsed,
        &Places {
            store_file: &path,
            state_root: Some(inputs.root),
        },
        Prefer::Ambiguity,
    )
    .map_err(|failure| refusal_of(&failure))?;
    let spelled = scope::spelled(&scope.repo);

    let mut answer = json!({
        "reference": reference,
        "taken_at": taken_at,
        "time": utc(taken_at),
        "scope": { "repo": spelled, "issue": scope.number, "basis": scope.basis.word() },
    });
    for (name, read) in from_store(inputs, &scope, taken_at) {
        answer[name] = section(read);
    }
    answer["issue"] = section(
        inputs
            .tracker
            .issue(&scope.repo, scope.number)
            .and_then(|data| {
                let text = |key: &str| data.get(key).and_then(Value::as_str).map(str::to_owned);
                text("state")
                    .map(|state| json!({ "state": state.to_lowercase(), "url": text("html_url") }))
                    .ok_or_else(|| "the tracker's answer carried no state".to_owned())
            }),
    );

    let judged = review(&path, &scope.repo, scope.number);
    let (pull, live_head) = pull_request(inputs.tracker, &scope.repo, &judged);
    answer["pull_request"] = section(pull);
    answer["review"] = section(match judged {
        Ok(Some(found)) => Ok(json!({
            "verdict": found.verdict,
            "escalated": found.escalated,
            "epoch": found.receipt.epoch,
            "pr": found.receipt.pr,
            "head": found.receipt.head,
            "base": found.receipt.base,
            "digest": found.receipt.digest,
            "bound_to_live_head": live_head.map(|live| live == found.receipt.head),
        })),
        Ok(None) => Ok(json!({ "recorded": false })),
        Err(reason) => Err(reason),
    });
    Ok(answer)
}

fn line(name: &str, section: &Value) -> String {
    match section.get("unavailable").and_then(Value::as_str) {
        Some(reason) => format!("{name:<14} unavailable: {reason}"),
        None => format!("{name:<14} {section}"),
    }
}

pub(crate) fn read(reference: &str, context: &Context, root: &Path) -> Result<Value, Refusal> {
    answer(
        reference,
        &Inputs {
            context,
            root,
            now: session::now_seconds(),
            tracker: &Gh,
        },
    )
}

pub(super) fn show(json: bool, reference: &str) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = Context::live(crate::harness::control_surface(), repo_dir, None);
    let said = read(reference, &context, &root)?;
    if json {
        say!("{said}");
        return Ok(());
    }
    say!(
        "{}  as of {}",
        said["scope"],
        said["time"].as_str().unwrap_or_default()
    );
    for name in FIELDS {
        say!("{}", line(name, &said[name]));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
