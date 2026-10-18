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

/// Where the directory a call was launched from sits with respect to Git.
///
/// Three answers, not two: *no repository* exempts a write the requirement does
/// not reach, while *a repository nothing here could read* holds it, and a
/// refusal or a session may name neither of them "this repository".
pub(crate) enum Scope {
    /// Git named the working tree that contains it.
    Repository(PathBuf),
    /// Git named none, and no ancestor holds a `.git` entry.
    Outside,
    /// Neither could be established: Git could not be asked, or a `.git` entry
    /// is there and did not become a working tree.
    Unknown,
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
/// a write that lands outside every repository the gate can place is not a write to this
/// repository. `declared` are the paths a write tool named in a patch body, empty for every other
/// call; they are placed one by one, and a write to any repository is held. Under `ask` a write
/// the gate cannot place is allowed rather than put to the person; `required` still holds it.
pub fn decision(
    context: &GateContext,
    action: &Action,
    how: Sensitivity,
    literal: Option<&str>,
    declared: &[String],
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
    let scope = scope_of(&context.repo_dir);
    match placement(context, action, literal, declared, &scope) {
        Placement::Outside => None,
        Placement::Inside => held(context, requirement, &scope),
        Placement::Unplaceable => match requirement {
            IssueRequirement::Ask => Some(Decision::Allow(allowed(target))),
            _ => held(context, requirement, &scope),
        },
    }
}

fn held(context: &GateContext, requirement: IssueRequirement, scope: &Scope) -> Option<Decision> {
    let assisted = context.workflow == Some(crate::config::Workflow::Assisted);
    match requirement {
        IssueRequirement::Required => Some(Decision::Deny(Box::new(refusal(
            assisted,
            requirement,
            scope,
        )))),
        IssueRequirement::Ask => Some(Decision::Ask(Box::new(refusal(
            assisted,
            requirement,
            scope,
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

fn placement(
    context: &GateContext,
    action: &Action,
    literal: Option<&str>,
    declared: &[String],
    scope: &Scope,
) -> Placement {
    let Action::Write { target } = action else {
        return Placement::Inside;
    };
    match literal {
        Some(line) => shell_write(context, target, line, scope),
        None => file_tool(context, target, declared, scope),
    }
}

fn file_tool(context: &GateContext, target: &str, declared: &[String], scope: &Scope) -> Placement {
    if declared.is_empty() && super::names_no_write_path(target) {
        // No field and no body named a path, so `target` is the tool's own name
        // and the destination was never read. Held rather than placed by name,
        // which is what a relative target would otherwise answer for.
        return Placement::Inside;
    }
    let targets: Vec<&str> = if declared.is_empty() {
        vec![target]
    } else {
        declared.iter().map(String::as_str).collect()
    };
    // Only *all targets outside* exempts the write: one placed inside, or one
    // that could not be placed, holds it — the asymmetry `paths::placed` states.
    for target in targets {
        if !matches!(against(context, scope, target), Placement::Outside) {
            return Placement::Inside;
        }
    }
    Placement::Outside
}

/// A shell write is placed only by literal redirect targets; a named repository operation arrives
/// first in the classifier and writes this checkout itself, and everything else needs running.
fn shell_write(context: &GateContext, target: &str, line: &str, scope: &Scope) -> Placement {
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
                Path::new(target).is_absolute()
                    && matches!(against(context, scope, target), Placement::Outside)
            }) =>
        {
            Placement::Outside
        }
        Some(_) => Placement::Inside,
        None => Placement::Unplaceable,
    }
}

/// Where one named target lands, given the scope the call was launched in.
fn against(context: &GateContext, scope: &Scope, target: &str) -> Placement {
    let target = Path::new(target);
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        context.repo_dir.join(target)
    };
    let Some(landing) = crate::paths::placed(&target) else {
        return Placement::Unplaceable;
    };
    match scope {
        // The checkout the call was launched in: a sibling checkout it does not
        // cover stays outside, as it always did.
        Scope::Repository(root) => {
            if crate::paths::covers(root, &landing) {
                Placement::Inside
            } else {
                Placement::Outside
            }
        }
        // Nothing here was a repository, so the target is one only where a
        // `.git` entry says so — a repository, or a claimed worktree, from
        // outside both.
        Scope::Outside => match holds_a_git_entry(&landing) {
            Some(true) => Placement::Inside,
            Some(false) => Placement::Outside,
            None => Placement::Unplaceable,
        },
        // Nothing was established. An unknown is not clearance.
        Scope::Unknown => Placement::Inside,
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

/// Which repository, if any, contains `directory`.
///
/// Git answers first, being the authority on where a working tree begins; only
/// where it fails is the filesystem walked for a `.git` entry, and the two
/// answers are told apart — no entry anywhere is `Outside`, an entry Git would
/// not use is `Unknown`.
pub(crate) fn scope_of(directory: &Path) -> Scope {
    if let Some(root) = git_root(directory) {
        return Scope::Repository(root);
    }
    match holds_a_git_entry(directory) {
        Some(true) => Scope::Unknown,
        Some(false) => Scope::Outside,
        None => Scope::Unknown,
    }
}

fn git_root(directory: &Path) -> Option<PathBuf> {
    let answer = super::proof_git_command(directory, &["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !answer.status.success() {
        return None;
    }
    let text = String::from_utf8(answer.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| PathBuf::from(text))
}

/// Whether a `.git` entry sits in `directory` or in any ancestor.
///
/// `Some(false)` only where every ancestor could be read: a directory this
/// process cannot open answers `None`, because an unreadable ancestor is not
/// proof that no repository holds the path.
fn holds_a_git_entry(directory: &Path) -> Option<bool> {
    let mut probe = Some(directory);
    while let Some(directory) = probe {
        match std::fs::symlink_metadata(directory.join(".git")) {
            Ok(_) => return Some(true),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) => {}
            Err(_) => return None,
        }
        probe = directory.parent();
    }
    Some(false)
}

/// The refusal for an unclaimed write under the requirement in force.
///
/// The clause bound to `scope` keeps it honest: only a scope Git named may name
/// a directory, and a hold reached by *failing* to place the write says so
/// rather than implying the write landed in a repository.
pub(crate) fn refusal(assisted: bool, requirement: IssueRequirement, scope: &Scope) -> Refusal {
    let way_in = if assisted {
        "a local claim taken with `assist` in a worktree, which gives the isolated checkout to \
         write in"
    } else {
        "an issue claimed with `claim`, filed first with `create` when none covers this \
         change, then a branch started with `start_branch`, which gives the isolated \
         checkout to write in"
    };
    let here = match scope {
        Scope::Repository(_) => " in this repository",
        Scope::Outside => {
            ", and this write could not be placed outside every repository, so it is held"
        }
        Scope::Unknown => {
            ", and whether this directory or this write's landing is a repository could not \
             be established"
        }
    };
    Refusal::not_started(
        CODE,
        format!(
            "this run holds no issue, and `Issue requirement` is `{}`{here}: a write \
             without a claim skips the claim, the review receipt and the delivery gate",
            requirement.as_value()
        ),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_in),
    )
}

#[cfg(test)]
mod tests;
