//! The local claim of an assisted run, and what it admits.
//!
//! Under `Workflow: assisted` nothing on the tracker backs a run's claim, so
//! the gate reads the pointer against bytes on this machine instead: the
//! ownership marker inside the run's worktree and the branch it has checked
//! out. `docs/honesty.md` says what that cannot tell.

use std::path::Path;

use crate::config::Workflow;
use crate::outcome::{NoCommandReason, Observed, Refusal, Resolution};

use super::session::Run;
use super::{Action, Aside, Decision, GateContext, Sensitivity};

enum Landing {
    Inside,
    SameRepository,
    Elsewhere,
    Unplaceable,
    Unplaced,
}

fn way_out(text: impl Into<String>) -> Resolution {
    Resolution::no_command(NoCommandReason::OperatorKnowledge, text)
}

fn names_no_path(target: &str) -> bool {
    super::WRITE_TOOLS
        .iter()
        .any(|tool| tool.eq_ignore_ascii_case(target))
}

fn landing(worktree: &Path, from: &Path, target: &str) -> Landing {
    if names_no_path(target) {
        return Landing::Unplaced;
    }
    let Some(placed) = crate::paths::placed(&from.join(target)) else {
        return Landing::Unplaceable;
    };
    if crate::paths::covers(worktree, &placed) {
        return Landing::Inside;
    }
    let existing = placed
        .ancestors()
        .find(|ancestor| ancestor.is_dir())
        .unwrap_or(&placed);
    if super::same_git_repository(worktree, existing) {
        Landing::SameRepository
    } else {
        Landing::Elsewhere
    }
}

fn marker_agrees(
    run: &Run,
    issue: &crate::identity::Identity,
    branch: &str,
    worktree: &Path,
) -> Result<(), String> {
    let marker = crate::transport::branch::read_ownership(worktree)
        .map_err(|failure| {
            format!(
                "the ownership marker could not be read: {}",
                failure.detail()
            )
        })?
        .ok_or_else(|| {
            format!(
                "{} holds no ownership marker, so no run is known to own it",
                worktree.display()
            )
        })?;
    let text = |key: &str| marker.get(key).and_then(serde_json::Value::as_str);
    let recorded = marker
        .get("issue")
        .cloned()
        .and_then(|value| serde_json::from_value::<crate::identity::Identity>(value).ok());
    if text("run_id") != Some(run.run_id.as_str())
        || recorded.as_ref() != Some(issue)
        || text("branch") != Some(branch)
    {
        return Err(format!(
            "the ownership marker in {} names another run, issue or branch",
            worktree.display()
        ));
    }
    Ok(())
}

fn branch_checked_out(branch: &str, worktree: &Path) -> Result<(), String> {
    let answer = super::proof_git_command(worktree, &["symbolic-ref", "HEAD"])
        .output()
        .map_err(|error| format!("git could not be asked which branch is checked out: {error}"))?;
    let head = String::from_utf8_lossy(&answer.stdout).trim().to_owned();
    if answer.status.success() && head == format!("refs/heads/{branch}") {
        Ok(())
    } else {
        Err(format!(
            "{} does not have {branch} checked out",
            worktree.display()
        ))
    }
}

pub(super) fn decision(
    context: &GateContext,
    run: &mut Run,
    action: &Action,
    how: Sensitivity,
    observed: &Observed,
    subject: &str,
) -> Decision {
    let deny =
        |refusal: Refusal| Decision::Deny(Box::new(super::prefixed(refusal, subject, observed)));
    let Some(issue) = run.issue.clone() else {
        return Decision::Outside(Aside::NothingSworn);
    };

    let mut references = run.worktree.iter().chain(run.repo_dir.iter()).peekable();
    if references.peek().is_some()
        && !references.any(|reference| {
            crate::paths::covers(reference, &context.repo_dir)
                || super::same_git_repository(reference, &context.repo_dir)
        })
    {
        return Decision::Outside(Aside::AnotherCheckout);
    }

    if context.workflow != Some(Workflow::Assisted) {
        let cause = match context.workflow {
            None => "the contract could not be read, so no row says it is",
            Some(_) => "the `Workflow` row is not `assisted` here any longer",
        };
        return deny(Refusal::not_started(
            "assisted-claim-not-in-force",
            format!(
                "{} holds issue #{issue} locally, an assisted claim, and {cause}",
                run.run_id
            ),
            way_out(format!(
                "the row read as `assisted` again, or this pointer taken away \u{2014} {} is \
                 Estigia's own state and no claim of anybody's",
                super::session::pointer_path(&context.state_root, &run.run_id).display()
            )),
        ));
    }

    if let Some(refusal) = super::contract_not_installed(context) {
        return deny(refusal);
    }
    if let Some(refusal) = super::operation_window::check_protected(context, action) {
        return deny(refusal);
    }

    let Some(worktree) = run.worktree.clone() else {
        return deny(Refusal::not_started(
            "assisted-worktree-not-started",
            format!(
                "{} holds issue #{issue} locally and has no worktree, so nothing is confined \
                 yet",
                run.run_id
            ),
            way_out("the `assist` tool, which makes the worktree the claim is confined to"),
        ));
    };

    if !crate::paths::covers(&worktree, &context.repo_dir) {
        return deny(outside_the_worktree(&worktree, &context.repo_dir));
    }
    if let Action::Write { target } = action {
        match landing(&worktree, &context.repo_dir, target) {
            Landing::Inside => {}
            Landing::Unplaced => {
                return deny(Refusal::not_started(
                    "assisted-write-unplaced",
                    format!(
                        "`{target}` is either a write tool whose paths live in its body or a shell \
                         command whose destination the gate cannot place, so this write cannot be \
                         confined to {}",
                        worktree.display()
                    ),
                    way_out(
                        "a write tool that names its path, or the change made by a person; a \
                         shell command such as `patch` is not admitted here",
                    ),
                ));
            }
            Landing::SameRepository | Landing::Unplaceable => {
                return deny(outside_the_worktree(&worktree, Path::new(target)));
            }
            Landing::Elsewhere if how == Sensitivity::Routine => {
                return Decision::Outside(Aside::OutsideTheClaim);
            }
            Landing::Elsewhere => {}
        }
    }

    if how == Sensitivity::Routine && run.within_window(context.window) {
        return Decision::Allow(format!(
            "issue #{issue} was verified locally inside the renewal window"
        ));
    }

    let standing = run
        .branch
        .clone()
        .ok_or_else(|| "the pointer names no branch".to_owned())
        .and_then(|branch| {
            marker_agrees(run, &issue, &branch, &worktree)?;
            branch_checked_out(&branch, &worktree)?;
            Ok(branch)
        });
    let branch = match standing {
        Ok(branch) => branch,
        Err(why) => {
            return deny(Refusal::not_started(
                "assisted-claim-stale",
                format!(
                    "{}'s local claim on issue #{issue} no longer matches this machine: {why}",
                    run.run_id
                ),
                way_out(format!(
                    "the pointer taken away \u{2014} {} is Estigia's own state \u{2014} and the \
                     checkout started again with `assist`",
                    super::session::pointer_path(&context.state_root, &run.run_id).display()
                )),
            ));
        }
    };
    run.mark_verified();

    if let Action::Boundary { command, .. } = action {
        if super::is_delivery(action) {
            return deny(Refusal::not_started(
                "assisted-does-not-deliver",
                format!("`{command}` delivers, and an assisted run delivers nothing"),
                way_out("a person landing the work, outside this run"),
            ));
        }
        if command == "gh pr create" {
            return deny(Refusal::not_started(
                "assisted-publishes-through-draft-pr",
                "an assisted run publishes as a draft pull request that Estigia opens and reads \
                 back, and a shell `gh pr create` has no read-back and no tie to this run"
                    .to_owned(),
                way_out("the `draft_pr` tool, which pushes the run's branch and opens the draft"),
            ));
        }
        if command == "git push" {
            let wanted = format!("refs/heads/{branch}");
            if observed.push_refs.is_empty() {
                return deny(Refusal::not_started(
                    "assisted-push-unconfined",
                    format!(
                        "a push is admitted only for {wanted}, and the refs it names are known \
                         only to the pre-push hook, which has not spoken for this call"
                    ),
                    way_out("the push made where git's own pre-push hook reads its refs"),
                ));
            }
            if observed
                .push_refs
                .iter()
                .any(|(_, remote)| remote.is_empty())
            {
                return deny(Refusal::not_started(
                    "assisted-push-unconfined",
                    format!(
                        "a push is admitted only for {wanted}, and a line the pre-push hook \
                         reported has no remote ref, so where it lands cannot be confined"
                    ),
                    way_out(
                        "a push naming the run's own branch by name, as both source and target",
                    ),
                ));
            }
            if let Some(other) = observed
                .push_refs
                .iter()
                .flat_map(|(local, remote)| [local, remote])
                .find(|name| **name != wanted)
            {
                return deny(Refusal::not_started(
                    "assisted-push-unconfined",
                    format!(
                        "this push names {other}, and an assisted run may push only {wanted} \
                         to {wanted}"
                    ),
                    way_out(
                        "a push naming the run's own branch by name, as both source and target; \
                         git reports `HEAD` to the hook as `HEAD`, so `git push origin HEAD` is \
                         refused",
                    ),
                ));
            }
        }
    }

    Decision::Allow(format!("issue #{issue} is held locally by {}", run.run_id))
}

fn outside_the_worktree(worktree: &Path, landed: &Path) -> Refusal {
    Refusal::not_started(
        "assisted-outside-the-worktree",
        format!(
            "{} is not inside {}, the only place this assisted claim \
             admits a write",
            landed.display(),
            worktree.display()
        ),
        way_out("the same change made inside that worktree"),
    )
}

#[cfg(test)]
mod tests;
