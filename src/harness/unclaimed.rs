//! What a run holding no claim may not do in a checkout where the push guard is installed.
//!
//! An operator who ran `estigia guard` in a repository declared that work there happens under
//! Estigia. In that checkout a commit or a push made with no claim is the way into the flow being
//! skipped, so both are refused, on every branch: the one way to commit is `claim` and then
//! `start_branch`, whose isolated checkout the claim covers (under `Workflow: assisted`, `assist`
//! in a worktree).

use std::path::Path;

use super::{Action, GateContext};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this refusal.
pub const CODE: &str = "unclaimed-commit";

const CREATES_A_COMMIT: &[&str] = &[
    "git commit",
    "git cherry-pick",
    "git revert",
    "git merge",
    "git am",
    "git rebase",
];

const RECOVERS: &[&str] = &["--abort", "--quit", "--skip"];

fn commits_or_pushes(literal: &str) -> bool {
    let view =
        super::shell::data_view(&super::shell::unquote_plain_words(literal)).to_ascii_lowercase();
    let mut normalized = format!("{} ", super::without_leading_options(&view));
    for fragment in CREATES_A_COMMIT {
        for flag in RECOVERS {
            normalized = normalized.replace(&format!("{fragment} {flag} "), " ");
        }
    }
    CREATES_A_COMMIT
        .iter()
        .chain(std::iter::once(&"git push"))
        .any(|fragment| super::contains_whole_command(&normalized, fragment))
}

fn is_a_push(action: &Action) -> bool {
    matches!(action, Action::Boundary { command, .. } if command == "git push")
}

/// Whether an unclaimed commit or push is refused in this checkout at all.
///
/// A claim cannot be made without a tracker a run can hold one against (a transport to swear it
/// through, or a read-only tracker under an assisted workflow) and a contract that says how, so
/// refusing without them would be a lock nobody can open.
pub fn applies(context: &GateContext, repo_dir: &Path) -> bool {
    context
        .tracker
        .adjudicates_unclaimed_writes(context.workflow)
        && context.skill_root.join(crate::skill::CONTRACT).is_file()
        && matches!(
            super::guard::state(repo_dir),
            super::guard::State::Installed | super::guard::State::Chained
        )
}

/// The refusal for this action when no claim holds the checkout, or `None` when this is not the
/// kind of call it refuses.
///
/// `literal` is the shell command line when there is one; a git hook has none and is handed the
/// push as an action.
pub fn refusal(
    context: &GateContext,
    repo_dir: &Path,
    action: &Action,
    literal: Option<&str>,
) -> Option<Refusal> {
    let named = literal.is_some_and(commits_or_pushes);
    if !named && !is_a_push(action) {
        return None;
    }
    if !applies(context, repo_dir) {
        return None;
    }
    let way_in = if context.workflow == Some(crate::config::Workflow::Assisted) {
        "a local claim taken with `assist` in a worktree, which gives the isolated checkout to \
         commit and push from"
    } else {
        "an issue claimed with `claim`, then a branch started with `start_branch`, which \
         gives the isolated checkout to commit and push from"
    };
    Some(Refusal::not_started(
        CODE,
        "this run holds no issue, and the push guard installed in this checkout means work here \
         goes through a claim: a commit or a push without one skips every check that follows"
            .to_owned(),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_in),
    ))
}

#[cfg(test)]
mod tests;
