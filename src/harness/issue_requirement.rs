//! What a run holding no claim may write in a checkout that has a tracker.

use std::path::{Path, PathBuf};

use super::{Action, Decision, GateContext, Sensitivity, session};
use crate::config::IssueRequirement;
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this refusal.
pub const CODE: &str = "issue-required";

/// The requirement in force here, or `None` where nothing can be required.
///
/// A claim cannot be made without a tracker a run can hold one against (a transport to swear it
/// through, or a read-only tracker under an assisted workflow) and a contract that says how, so
/// requiring one without them would be a lock nobody can open.
pub fn in_force(context: &GateContext) -> Option<IssueRequirement> {
    (context.issue_requirement != IssueRequirement::Off
        && context
            .tracker
            .adjudicates_unclaimed_writes(context.workflow)
        && context.skill_root.join(crate::skill::CONTRACT).is_file())
    .then_some(context.issue_requirement)
}

/// Where a routine write lands, as far as the gate can place it.
enum Placement {
    Inside,
    Outside,
    Unplaceable,
}

/// The decision for a write made by a run holding no claim, or `None` when this is not one.
///
/// Only a routine write is one: a boundary is held by the checks that already answer for it, and
/// a write that lands outside the checkout is not a write to this repository. Under `ask` a write
/// the gate cannot place is allowed rather than put to the person; `required` still holds it.
pub fn decision(
    context: &GateContext,
    action: &Action,
    how: Sensitivity,
    literal: Option<&str>,
) -> Option<Decision> {
    let Action::Write { target } = action else {
        return None;
    };
    if how != Sensitivity::Routine {
        return None;
    }
    let requirement = in_force(context)?;
    if in_a_claimed_checkout(context, target) {
        return None;
    }
    match placement(context, action, literal) {
        Placement::Outside => None,
        Placement::Inside => held(context, requirement),
        Placement::Unplaceable => match requirement {
            IssueRequirement::Ask => Some(Decision::Allow(allowed(target))),
            _ => held(context, requirement),
        },
    }
}

fn held(context: &GateContext, requirement: IssueRequirement) -> Option<Decision> {
    match requirement {
        IssueRequirement::Required => Some(Decision::Deny(Box::new(refusal(
            context.workflow == Some(crate::config::Workflow::Assisted),
            requirement,
        )))),
        IssueRequirement::Ask => Some(Decision::Ask(Box::new(refusal(
            context.workflow == Some(crate::config::Workflow::Assisted),
            requirement,
        )))),
        IssueRequirement::Off => None,
    }
}

fn allowed(target: &str) -> String {
    format!(
        "this run holds no issue and `Issue requirement` is `ask`; this call's write ({target}) \
         cannot be placed inside or outside the checkout without running it, so it is allowed, and \
         a checkout it dirties is named when the next session starts"
    )
}

fn placement(context: &GateContext, action: &Action, literal: Option<&str>) -> Placement {
    let Action::Write { target } = action else {
        return Placement::Inside;
    };
    match literal {
        Some(line) => shell_write(context, target, line),
        None => file_tool(context, target),
    }
}

fn file_tool(context: &GateContext, target: &str) -> Placement {
    if lands_in_the_checkout(&context.repo_dir, target) {
        Placement::Inside
    } else {
        Placement::Outside
    }
}

/// A shell write is placed only by literal redirect targets; a named repository operation arrives
/// first in the classifier and writes this checkout itself, and everything else needs running.
fn shell_write(context: &GateContext, target: &str, line: &str) -> Placement {
    if super::shell::writes_a_file(&line.to_ascii_lowercase()).as_deref() != Some(target) {
        return Placement::Inside;
    }
    // `rg --pre` runs a program the redirect targets cannot bound, so it is held
    // however the line redirects.
    if super::shell::runs_a_program_after_pre(line) {
        return Placement::Inside;
    }
    match super::shell::redirect_targets(line) {
        Some(targets)
            if targets.iter().all(|target| {
                Path::new(target).is_absolute() && !lands_in_the_checkout(&context.repo_dir, target)
            }) =>
        {
            Placement::Outside
        }
        Some(_) => Placement::Inside,
        None => Placement::Unplaceable,
    }
}

fn in_a_claimed_checkout(context: &GateContext, target: &str) -> bool {
    let path = Path::new(target);
    let landing = if path.is_absolute() {
        match crate::paths::placed(path) {
            Some(landing) => landing,
            None => return false,
        }
    } else {
        context.repo_dir.clone()
    };
    session::holdings(&context.state_root)
        .iter()
        .filter(|run| match (&run.worktree, &run.repo_dir) {
            (Some(worktree), Some(repo_dir)) => worktree != repo_dir,
            _ => false,
        })
        .filter_map(|run| run.worktree.as_ref())
        .any(|worktree| {
            crate::paths::covers(worktree, &context.repo_dir)
                && crate::paths::covers(worktree, &landing)
        })
}

fn lands_in_the_checkout(repo_dir: &Path, target: &str) -> bool {
    let path = Path::new(target);
    if !path.is_absolute() {
        return true;
    }
    let Some(landing) = crate::paths::placed(path) else {
        return true;
    };
    let root = super::proof_git_command(repo_dir, &["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|answer| answer.status.success())
        .and_then(|answer| String::from_utf8(answer.stdout).ok())
        .map(|text| PathBuf::from(text.trim()))
        .unwrap_or_else(|| repo_dir.to_path_buf());
    crate::paths::covers(&root, &landing)
}

/// The refusal for an unclaimed write under the requirement in force.
pub fn refusal(assisted: bool, requirement: IssueRequirement) -> Refusal {
    let way_in = if assisted {
        "a local claim taken with `assist` in a worktree, which gives the isolated checkout to \
         write in"
    } else {
        "an issue claimed with `claim`, filed first with `create` when none covers this \
         change, then a branch started with `start_branch`, which gives the isolated \
         checkout to write in"
    };
    Refusal::not_started(
        CODE,
        format!(
            "this run holds no issue, and `Issue requirement` is `{}` in this repository: a write \
             without a claim skips the claim, the review receipt and the delivery gate",
            requirement.as_value()
        ),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_in),
    )
}

#[cfg(test)]
mod tests;
