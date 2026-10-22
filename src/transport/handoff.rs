//! Handing an issue to another runtime's session.
//!
//! The record itself is local: it is what a later session of that runtime reads
//! at `SessionStart`, and what lets its `dev:<runtime>` label be explained to a
//! claim rather than refused as foreign. The tracker sees only the label, and
//! that projection is queued through the outbox with the same read-back a claim
//! uses. Delivery to a particular pane or terminal is deliberately not here:
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

/// Records the handoff locally and projects its `dev:<runtime>` label.
///
/// A handoff of an issue a live claim holds is refused before anything is
/// written: the label states where an unheld issue is going, and overwriting the
/// holder's projection would tell every reader the wrong thing. Releasing the
/// issue is the caller's move, not this operation's.
pub fn record(context: &Context, what: &Handoff<'_>) -> Result<Value, Failure> {
    crate::harness::session::runtime_component(what.runtime)
        .map_err(|_| runtime_unreadable(what.runtime))?;
    require_unheld(context, what.issue, what.runtime, what.now)?;

    let projection = if outbox::is_closed(context) {
        let repo = claim::repo_of(context)?;
        let mut held = Store::open(&store::store_path()?)?;
        let seq = outbox::queue_in(
            &mut held,
            &repo,
            what.issue,
            OutboxKind::Reconverge,
            &json!({ "handoff": what.runtime }),
            what.now,
        )?;
        json!({ "projection": "deferred", "queued": seq })
    } else {
        project_labels(context, what.issue, what.runtime)?;
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

/// Refuses a handoff of an issue a live claim holds under another runtime.
pub(crate) fn require_unheld(
    context: &Context,
    issue: u64,
    runtime: &str,
    now: &str,
) -> Result<(), Failure> {
    let comments = claim::timeline(context, issue, now)?;
    let Some(held) = claim::holding(&comments, now).event else {
        return Ok(());
    };
    if held.runtime.as_deref() == Some(runtime) {
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
            "`{issue}` is held by `{}` ({holder}); release it before handing it to {runtime}, \
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

/// Moves the issue's `dev:` labels to exactly the runtime this handoff names,
/// and reads the result back until it agrees.
pub(crate) fn project_labels(context: &Context, issue: u64, runtime: &str) -> Result<(), Failure> {
    let wanted = format!("dev:{runtime}");
    let projected = |data: &Value| labels_of(data) == [wanted.clone()].into_iter().collect();
    let data = read_labels(context, issue)?;
    let present: Vec<String> = labels_of(&data).into_iter().collect();
    let (add, remove) = claim::converge_projection(&present, Some(runtime));
    let mut edit: Vec<String> = ["gh", "issue", "edit", &issue.to_string()]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    for gone in &remove {
        edit.push("--remove-label".to_owned());
        edit.push(gone.clone());
    }
    for landed in &add {
        commands::ensure_label(context, landed, "bfd4f2")?;
        edit.push("--add-label".to_owned());
        edit.push(landed.clone());
    }
    let wrote = edit.len() > 4;
    if wrote {
        let borrowed: Vec<&str> = edit.iter().map(String::as_str).collect();
        super::run(&borrowed, Some(&context.repo_dir), super::How::write())?;
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
