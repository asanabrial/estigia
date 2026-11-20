//! Handing an issue to another runtime's session.
//!
//! The record itself is local: it is what a later session of that runtime reads
//! at `SessionStart`. The tracker is told nothing about the runtime; the only
//! projection is the removal of a leftover `dev:` label, queued through the
//! outbox with the same read-back a claim uses. Delivery to a particular pane or terminal is deliberately not here:
//! that is a host convenience which would bypass consent and stamping.
//!
//! `docs/sessions.md` owns the behaviour; `docs/honesty.md` owns what is not
//! measured.

use serde_json::{Value, json};

use super::store::{self, OutboxKind, Store};
use super::{Context, Failure, claim, commands, outbox};

/// One handoff, as the caller names it.
pub struct Handoff<'a> {
    pub issue: u64,
    pub runtime: &'a str,
    pub run_id: &'a str,
    pub now: &'a str,
}

fn runtime_unreadable(runtime: &str) -> Failure {
    Failure::ConfigDefect(json!({
        "ok": false,
        "reason": "handoff-runtime-unreadable",
        "runtime": runtime,
        "action": "a runtime word a `dev:` label can carry: lowercase `a-z0-9` groups joined by \
                   single `-`, such as `opencode` or `claude`",
    }))
}

/// Records the handoff locally and strips any leftover `dev:` label.
///
/// A handoff of an issue a live claim holds is refused before anything is
/// written: it states where an unheld issue is going, and a handoff over a
/// holder would contradict the claim. Releasing the issue is the caller's move,
/// not this operation's.
pub fn record(context: &Context, what: &Handoff<'_>) -> Result<Value, Failure> {
    crate::harness::session::runtime_component(what.runtime)
        .map_err(|_| runtime_unreadable(what.runtime))?;
    require_unheld(context, what.issue, None, what.now)?;

    let projection = if outbox::is_closed(context) {
        let repo = claim::repo_of(context)?;
        let mut held = Store::open(&store::store_path()?)?;
        let seq = outbox::queue_in(
            &mut held,
            &repo,
            what.issue,
            OutboxKind::Reconverge,
            &json!({ QUEUED_RUNTIME: what.runtime }),
            what.now,
        )?;
        json!({ "projection": "deferred", "queued": seq })
    } else {
        project_labels(context, what.issue)?;
        json!({ "projection": "landed" })
    };

    Ok(json!({
        "ok": true,
        "issue": what.issue,
        "to": what.runtime,
        "from_run": what.run_id,
        "at": what.now,
        "projection": projection["projection"],
        "queued": projection.get("queued"),
    }))
}

/// Refuses a handoff of an issue a live claim holds, except one held under
/// `recipient`: a queued projection whose recipient already claimed the issue
/// still lands.
pub(crate) fn require_unheld(
    context: &Context,
    issue: u64,
    recipient: Option<&str>,
    now: &str,
) -> Result<(), Failure> {
    let comments = claim::timeline(context, issue, now)?;
    let Some(held) = claim::holding(&comments, now).event else {
        return Ok(());
    };
    if recipient.is_some() && held.runtime.as_deref() == recipient {
        return Ok(());
    }
    let holder = held
        .runtime
        .as_deref()
        .unwrap_or("a runtime the markers do not name");
    Err(Failure::Stop(json!({
        "ok": false,
        "reason": "handoff-issue-held",
        "issue": issue,
        "holder": held.run_id,
        "holder_runtime": held.runtime,
        "action": format!(
            "`{issue}` is held by `{}` ({holder}); release it before handing it over, \
             because a handoff is a destination and never a claim",
            held.run_id
        ),
    })))
}

fn labels_of(data: &Value) -> std::collections::BTreeSet<String> {
    claim::projection_state(data).1
}

fn read_labels(context: &Context, issue: u64) -> Result<Value, Failure> {
    super::gh_json(
        &["issue", "view", &issue.to_string(), "--json", "labels"],
        Some(&context.repo_dir),
    )?
    .ok_or_else(|| Failure::Read(format!("gh issue view {issue} returned nothing")))
}

/// Strips every `dev:` label from the issue and reads the result back until it
/// agrees. A handoff adds no label: where it is going lives in the local store.
pub(crate) fn project_labels(context: &Context, issue: u64) -> Result<(), Failure> {
    let projected = |data: &Value| labels_of(data).is_empty();
    let data = read_labels(context, issue)?;
    let present: Vec<String> = labels_of(&data).into_iter().collect();
    let remove = claim::stale_runtime_labels(&present);
    let mut edit: Vec<String> = ["gh", "issue", "edit", &issue.to_string()]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    for gone in &remove {
        edit.push("--remove-label".to_owned());
        edit.push(gone.clone());
    }
    let wrote = edit.len() > 4;
    if wrote {
        commands::edit_labels(context, &edit)?;
    }
    settle(context, issue, &projected, wrote)
}

/// Two agreeing reads, because the tracker is eventually consistent.
fn settle(
    context: &Context,
    issue: u64,
    projected: &impl Fn(&Value) -> bool,
    wrote: bool,
) -> Result<(), Failure> {
    let mut seen = Vec::new();
    for _ in 0..claim::VISIBILITY_ATTEMPTS {
        seen.push(read_labels(context, issue).ok());
        if claim::wait_for(&seen, projected, 2).is_some() {
            return Ok(());
        }
    }
    if wrote {
        Err(Failure::Write(format!(
            "the handoff label on #{issue} did not read back exactly"
        )))
    } else {
        Err(Failure::Read(format!(
            "the labels on #{issue} never settled on the handoff projection"
        )))
    }
}

/// The payload key a queued handoff carries, so the flush knows which runtime's
/// label to project.
pub(crate) const QUEUED_RUNTIME: &str = "handoff";
