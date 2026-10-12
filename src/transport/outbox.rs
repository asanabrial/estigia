//! The numbers an issue carries before GitHub has given it one.
//!
//! An issue created while the git operation window is closed is held under a
//! provisional number far above any GitHub will assign, so no real issue can
//! be mistaken for it and no provisional one for a real issue.

use std::path::Path;

use super::store::{self, OutboxKind, Store};
use super::window::{self, Mode};
use super::{Context, Failure};

/// The first provisional issue number: above every number GitHub can issue and
/// below the largest integer a JSON reader keeps exactly.
pub(crate) const PROVISIONAL_BASE: u64 = 9_000_000_000_000;

/// Whether `issue` is a provisional number rather than one GitHub issued.
pub(crate) fn is_provisional(issue: u64) -> bool {
    issue >= PROVISIONAL_BASE
}

/// Whether the git operation window is closed for this operation, so that a
/// write to GitHub is queued instead of sent.
pub(crate) fn is_closed(context: &Context) -> bool {
    matches!(window::state(context), Mode::Closed(_))
}

/// How many writes wait in the default store, for the repository or any other:
/// the number a refusal quotes, so a store that cannot be read says none.
pub(crate) fn queued_writes() -> usize {
    store::store_path()
        .and_then(|path| store::open_for_read(&path))
        .ok()
        .flatten()
        .and_then(|held| held.unsent_outbox().ok())
        .unwrap_or_default()
}

/// Queues one write for `issue` and answers its place in the queue.
///
/// The payload carries no issue number: a provisional number is rewritten in
/// the queue's own column when GitHub issues the real one, and a copy inside
/// the payload would keep the old number.
pub(crate) fn queue(
    context: &Context,
    issue: u64,
    kind: OutboxKind,
    payload: &serde_json::Value,
    now: &str,
) -> Result<u64, Failure> {
    let repo = super::claim::repo_of(context)?;
    let mut held = Store::open(&store::store_path()?)?;
    queue_in(&mut held, &repo, issue, kind, payload, now)
}

/// [`queue`] into a store the caller already holds.
pub(crate) fn queue_in(
    store: &mut Store,
    repo: &store::Repo,
    issue: u64,
    kind: OutboxKind,
    payload: &serde_json::Value,
    now: &str,
) -> Result<u64, Failure> {
    let op_id = crate::harness::session::mint_operation_id(&format!("{kind:?}{payload}"), issue);
    store.enqueue(repo, issue, kind, payload, &op_id, now)
}

/// Queues the native link of `branch` to `issue`, unless a link of that branch
/// is already waiting, and answers what `develop_link` answers with the link
/// not made yet.
pub(crate) fn queue_link(
    context: &Context,
    issue: u64,
    branch: &str,
    base: &str,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let repo = super::claim::repo_of(context)?;
    let mut held = Store::open(&store::store_path()?)?;
    let waiting = held.outbox(&repo)?.into_iter().find(|row| {
        row.kind == OutboxKind::Link
            && row.issue == issue
            && row.state != store::OutboxState::Sent
            && row.payload["branch"] == branch
    });
    let queued = match waiting {
        Some(row) => row.seq,
        None => queue_in(
            &mut held,
            &repo,
            issue,
            OutboxKind::Link,
            &serde_json::json!({ "branch": branch, "base": base }),
            now,
        )?,
    };
    Ok(serde_json::json!({ "linked": false, "outcome": "deferred", "queued": queued }))
}

/// The number `issue` stands for: its real number once GitHub issued one, and
/// `issue` itself for every other number, provisional or not.
pub(crate) fn resolve_in(store: &Store, issue: u64) -> Result<u64, Failure> {
    if !is_provisional(issue) {
        return Ok(issue);
    }
    Ok(store
        .provisional_real_anywhere(issue)?
        .flatten()
        .unwrap_or(issue))
}

/// [`resolve_in`] over the default store, which an ordinary number never opens
/// and a store that does not exist yet cannot answer.
pub(crate) fn resolve(issue: u64) -> Result<u64, Failure> {
    if !is_provisional(issue) {
        return Ok(issue);
    }
    match store::open_for_read(&store::store_path()?)? {
        Some(held) => resolve_in(&held, issue),
        None => Ok(issue),
    }
}

/// Rewrites what lives outside the store and still names a provisional issue
/// whose real number is known: run pointers under `state_root`, and the
/// ownership record of the worktree each one points at. Answers how many files
/// it changed; a second pass over the same state changes none.
pub(crate) fn rekey_files(store: &Store, state_root: &Path) -> Result<usize, Failure> {
    let mut changed = 0;
    for run in crate::harness::session::holdings(state_root) {
        let Some(provisional) = run.issue_number().filter(|number| is_provisional(*number)) else {
            continue;
        };
        let real = resolve_in(store, provisional)?;
        if real == provisional {
            continue;
        }
        if let Some(worktree) = run.worktree.as_deref() {
            changed += usize::from(rekey_ownership(worktree, provisional, real)?);
        }
        let (_, landed) = crate::harness::session::updated(state_root, &run.run_id, |held| {
            if held.issue_number() == Some(provisional) {
                held.issue = Some(crate::identity::Identity::Number(real));
            }
        });
        if !landed {
            return Err(Failure::Write(format!(
                "the pointer of {} still names issue {provisional} after its rekey to {real}",
                run.run_id
            )));
        }
        changed += 1;
    }
    Ok(changed)
}

fn rekey_ownership(worktree: &Path, provisional: u64, real: u64) -> Result<bool, Failure> {
    let file = super::branch::ownership_path(worktree)?;
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Ok(false);
    };
    let Ok(mut record) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(false);
    };
    if record.get("issue").and_then(serde_json::Value::as_u64) != Some(provisional) {
        return Ok(false);
    }
    record["issue"] = serde_json::json!(real);
    let body = serde_json::to_string_pretty(&record)
        .map_err(|why| Failure::Write(format!("could not render {}: {why}", file.display())))?;
    crate::paths::replace_atomically(&file, &body)
        .map_err(|why| Failure::Write(format!("could not write {}: {why}", file.display())))?;
    Ok(true)
}

pub(crate) mod flush;
#[cfg(test)]
mod offline;
#[cfg(test)]
mod tests;
