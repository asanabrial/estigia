use std::path::Path;

use super::{mailbox, session};
use crate::outcome::{NoCommandReason, Refusal, RefusalClass, Resolution};
use crate::transport::store;

/// One handoff as the transport answered it.
pub(crate) struct Handed<'a> {
    pub(crate) issue: u64,
    pub(crate) runtime: &'a str,
    pub(crate) from_run: &'a str,
    pub(crate) projection: &'a str,
    pub(crate) at: &'a str,
    pub(crate) due_in: Option<u64>,
}

/// Records a handoff the transport already projected or queued, and the duty it opens: the
/// sender's right to one `done` or `blocked`, due `due_in` seconds from now when given.
///
/// The caller has written to the tracker by the time this runs, so a store that
/// will not take the row leaves the world changed and this record missing. That
/// is an unknown outcome, not a failure to start.
pub(crate) fn record(
    state_root: &Path,
    repo_dir: &Path,
    handed: &Handed<'_>,
) -> Result<(), Refusal> {
    let Handed {
        issue,
        runtime,
        from_run,
        projection,
        at,
        due_in,
    } = *handed;
    if issue == 0 || runtime.is_empty() || projection.is_empty() || at.is_empty() {
        return Err(row_unwritten(
            "the transport's answer named no issue, runtime, projection or time",
            issue,
            runtime,
            projection,
        ));
    }
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| row_unwritten(&failure.detail(), issue, runtime, projection))?;
    let repo = repo_dir.display().to_string();
    let unwritten = |failure: crate::transport::Failure| {
        row_unwritten(&failure.detail(), issue, runtime, projection)
    };
    opened
        .record_handoff(&repo, issue, runtime, Some(from_run), projection, at)
        .map_err(unwritten)?;
    let due_at = match due_in {
        Some(seconds) => session::now_seconds().map(|now| now.saturating_add(seconds)),
        None => None,
    };
    opened
        .duty_opened((&repo, issue, runtime), from_run, due_at)
        .map_err(unwritten)
}

/// What a recipient reports for a handoff it accepted.
pub(crate) struct Completion<'a> {
    pub(crate) issue: u64,
    pub(crate) kind: &'a str,
    pub(crate) outcome: Option<&'a str>,
    pub(crate) text: &'a str,
}

/// A handoff closed by [`complete`], and whether the sender was told by mail.
#[derive(Debug)]
pub(crate) struct Reported {
    pub(crate) outcome: String,
    pub(crate) mail: Result<i64, String>,
}

pub(crate) fn not_accepted(run_id: &str, issue: u64) -> Refusal {
    Refusal::not_started(
        "handoff-not-accepted",
        format!(
            "{run_id} has not accepted a handoff of #{issue}, so there is no duty for it to \
             close; nothing was recorded"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the run that claimed the handed-over issue, or `claim` it first",
        ),
    )
}

pub(crate) fn already_settled(issue: u64, outcome: &str) -> Refusal {
    Refusal::not_started(
        "handoff-already-settled",
        format!(
            "the handoff of #{issue} already ended as `{outcome}`; a handoff takes exactly one \
             `done` or `blocked`, so nothing was recorded"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a `note` to the sender, who decides what a later word changes",
        ),
    )
}

pub(crate) fn completion_invalid(kind: &str) -> Refusal {
    Refusal::not_started(
        "handoff-completion-invalid",
        format!(
            "a handoff ends with `done` and an outcome of succeeded or failed, or with `blocked` \
             and no outcome; got type `{kind}`"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "type `done` with `outcome`, or type `blocked`",
        ),
    )
}

fn unavailable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-store-unavailable",
        format!("the handoff record could not be read or written: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "try again; nothing was recorded",
        ),
    )
}

/// Binds the handoffs of `issue` to `run_id` when its runtime and repository are the ones they
/// were handed to, which is what accepting one means. Best effort: a store that cannot be
/// written leaves the handoff unaccepted, and a duty nobody accepted is not enforced.
pub(crate) fn accept(state_root: &Path, repo_dir: &Path, run_id: &str, issue: u64) -> bool {
    let session::RunIdShape::Named { runtime } = session::run_id_shape(run_id) else {
        return false;
    };
    let (Ok(opened), Some(now)) = (
        store::Store::open(&store::beside(state_root)),
        session::now_seconds(),
    ) else {
        return false;
    };
    let Ok(rows) = opened.handoffs() else {
        return false;
    };
    rows.iter()
        .filter(|row| row.issue == issue && row.runtime == runtime)
        .filter(|row| crate::paths::same_directory(Path::new(&row.repo_dir), repo_dir))
        .any(|row| {
            opened
                .duty_accepted((&row.repo_dir, issue, &row.runtime), run_id, now)
                .unwrap_or(false)
        })
}

fn outcome_of(completion: &Completion<'_>) -> Result<&'static str, Refusal> {
    match (completion.kind, completion.outcome) {
        ("done", Some("succeeded")) => Ok("succeeded"),
        ("done", Some("failed")) => Ok("failed"),
        ("blocked", None) => Ok("blocked"),
        (kind, _) => Err(completion_invalid(kind)),
    }
}

fn open_duty(
    opened: &store::Store,
    run_id: &str,
    completion: &Completion<'_>,
) -> Result<store::Duty, Refusal> {
    outcome_of(completion)?;
    let duty = opened
        .duty_held_by(run_id, completion.issue)
        .map_err(|failure| unavailable(&failure.detail()))?
        .ok_or_else(|| not_accepted(run_id, completion.issue))?;
    match &duty.outcome {
        Some(outcome) => Err(already_settled(completion.issue, outcome)),
        None => Ok(duty),
    }
}

/// Refuses what [`complete`] would refuse, writing nothing.
pub(crate) fn completable(
    state_root: &Path,
    run_id: &str,
    completion: &Completion<'_>,
) -> Result<store::Duty, Refusal> {
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| unavailable(&failure.detail()))?;
    open_duty(&opened, run_id, completion)
}

/// Ends the handoff `run_id` accepted with the one word it owes, then tells the sender.
///
/// The store settles first, and only the first word does: the update names the open duty, so a
/// second `done`, or a `blocked` after one, changes no row and is refused. The mail is the
/// sender's notice; a mailbox that will not take it leaves the outcome standing and says so.
pub(crate) fn complete(
    state_root: &Path,
    run_id: &str,
    completion: &Completion<'_>,
) -> Result<Reported, Refusal> {
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| unavailable(&failure.detail()))?;
    let duty = open_duty(&opened, run_id, completion)?;
    let outcome = outcome_of(completion)?;
    let now = session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))?;
    let taken = opened
        .duty_settled(&duty, run_id, outcome, completion.text, now)
        .map_err(|failure| unavailable(&failure.detail()))?;
    if !taken {
        return Err(already_settled(completion.issue, outcome));
    }
    let body = format!(
        "Handoff of #{} {outcome}: {}",
        completion.issue,
        completion.text.trim()
    );
    let mail = mailbox::send_shaped(
        state_root,
        Some(run_id),
        &duty.sender,
        &body,
        &mailbox::Shape {
            kind: Some(completion.kind),
            ..mailbox::Shape::default()
        },
    )
    .map(|receipt| receipt.id)
    .map_err(|refusal| refusal.to_string());
    Ok(Reported {
        outcome: outcome.to_owned(),
        mail,
    })
}

fn row_unwritten(detail: &str, issue: u64, runtime: &str, projection: &str) -> Refusal {
    Refusal::not_started(
        "handoff-row-unwritten",
        format!(
            "the `dev:{runtime}` projection for #{issue} is {projection}, and the handoff row \
             could not be written to this machine's store: {detail}. The label is on the tracker; \
             this session will not see the handoff at `SessionStart`"
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the issue's labels to establish what landed, then repeat the handoff",
        ),
    )
    .classed(RefusalClass::Unknown)
}

#[cfg(test)]
mod tests;
