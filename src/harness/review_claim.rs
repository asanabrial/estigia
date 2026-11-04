//! What a run holding a review-only claim may do: read, and nothing else here.
//!
//! The claim names one pull request this run did not build. No boundary and no
//! remote write is admitted, wherever it runs, and a file write is refused when
//! it lands in this repository, the review's own worktree included.
//! `docs/assisted-review.md` has the road and `docs/honesty.md` what this cannot see.

use std::path::Path;

use crate::outcome::{NoCommandReason, Observed, Refusal, Resolution};

use super::assisted::{Landing, aimed_at, landing, remote_write};
use super::session::Run;
use super::{Action, Aside, Decision, GateContext, Sensitivity};

const REFUSAL: &str = "review-claim-admits-no-write";

fn refused(run: &Run, what: &str) -> Refusal {
    let pull = run
        .reviewed_pr
        .map_or_else(|| "a pull request".to_owned(), |pr| format!("#{pr}"));
    Refusal::not_started(
        REFUSAL,
        format!(
            "{} holds a review-only claim on {pull}, which reviews and changes nothing: {what}",
            run.run_id
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`pr_review` `release`, which puts the claim down, and then the write under a claim \
             that admits it",
        ),
    )
}

pub(super) fn decision(
    context: &GateContext,
    run: &Run,
    action: &Action,
    how: Sensitivity,
    observed: &Observed,
) -> Decision {
    let subject = action.subject().unwrap_or_default();
    let deny = |what: &str| {
        Decision::Deny(Box::new(super::prefixed(
            refused(run, what),
            &subject,
            observed,
        )))
    };
    match action {
        Action::Untouched => Decision::Outside(Aside::NotWatched),
        Action::Remote { command } => deny(&format!("`{command}` writes to the host")),
        Action::Boundary { command, .. } => deny(&format!("`{command}` is not review")),
        Action::Write { target } => {
            if let Some(command) = remote_write(action, observed) {
                return deny(&format!("`{command}` writes to the host"));
            }
            write_decision(context, run, action, target, how).unwrap_or_else(|why| deny(&why))
        }
    }
}

fn write_decision(
    context: &GateContext,
    run: &Run,
    action: &Action,
    target: &str,
    how: Sensitivity,
) -> Result<Decision, String> {
    let from = aimed_at(run, action).map_or_else(|| context.repo_dir.clone(), |aimed| aimed.from);
    let in_this_repository = run.covered().any(|reference| {
        crate::paths::covers(reference, &from) || super::same_git_repository(reference, &from)
    });
    if !in_this_repository {
        return Ok(Decision::Outside(Aside::AnotherCheckout));
    }
    let Some(worktree) = run.worktree.as_deref() else {
        return Err("no worktree is recorded, so where this lands is unknown".to_owned());
    };
    match landing(worktree, Path::new(&context.repo_dir), target) {
        Landing::Elsewhere if how == Sensitivity::Routine => {
            Ok(Decision::Outside(Aside::OutsideTheClaim))
        }
        Landing::Inside => Err(format!(
            "`{target}` edits the reviewed checkout, and the bytes under review are not this \
             run's to change"
        )),
        Landing::SameRepository | Landing::Unplaced | Landing::Unplaceable | Landing::Elsewhere => {
            Err(format!("`{target}` is a repository write"))
        }
    }
}

#[cfg(test)]
mod tests;
