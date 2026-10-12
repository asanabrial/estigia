//! The read-only look at a correction that a validator is allowed.
//!
//! Bound to a receipt: the two commits it names are resolved and compared with
//! the text given, and only the paths the correction changed over its parent
//! can be asked about. Every read runs in an isolated git environment, so a
//! checkout's own attributes and configuration cannot change what the reviewer
//! is shown.

use std::path::Path;

use super::ReviewerCode;
use crate::outcome::Refusal;
use crate::transport::{self, Failure, How, Output, tier};

/// The most bytes one inspection may return. Past it the answer is a refusal,
/// never a truncation.
pub const INSPECT_CEILING: usize = 256 * 1024;

/// The two commits a correction is read between.
#[derive(Debug, Clone, Copy)]
pub struct InspectReceipt<'a> {
    /// The full id of the commit the correction was made over, or of the tree git merges that
    /// commit and an integrated base into when the correction merged a moved base.
    pub parent_head: &'a str,
    /// The full id of the corrected candidate.
    pub head: &'a str,
}

/// Which side of the correction an object is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The commit the correction was made over.
    Parent,
    /// The corrected candidate.
    Candidate,
}

/// What to look at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inspect {
    /// `git diff --name-status` over the correction.
    NameStatus,
    /// `git diff --numstat` over the correction.
    Numstat,
    /// `git diff --stat` over the correction.
    Stat,
    /// The unified diff of the correction, or of one path of it.
    Patch {
        /// A path the correction changed; every changed path when `None`.
        path: Option<String>,
    },
    /// One changed path's content on one side.
    Object {
        /// A path the correction changed.
        path: String,
        /// The side to read it from.
        side: Side,
    },
}

/// Answers `request` about the correction `receipt` names in `repo`.
///
/// Refuses with `reviewer-inspect-transaction-unresolved` when either id is not a
/// full commit id this checkout holds, `reviewer-inspect-outside-correction`
/// for a path the correction did not change, `reviewer-inspect-absent-side`
/// for a changed path with no copy on the side asked for,
/// `reviewer-input-too-large` past
/// [`INSPECT_CEILING`] and `reviewer-inspect-unreadable` when git could not be
/// read. A refusal is never an empty inspection.
pub fn inspect(
    repo: &Path,
    receipt: &InspectReceipt<'_>,
    request: &Inspect,
) -> Result<String, Refusal> {
    resolve_parent(repo, receipt.parent_head)?;
    resolve_commit(repo, receipt.head)?;
    let parent_tree = tier::tree_at(repo, receipt.parent_head).map_err(unreadable)?;
    let candidate_tree = tier::tree_at(repo, receipt.head).map_err(unreadable)?;
    let changed = tier::tree_pair_changed_paths_at(repo, &parent_tree, &candidate_tree)
        .map_err(unreadable)?;

    let text = match request {
        Inspect::NameStatus => diff(repo, receipt, &["--name-status"], None)?,
        Inspect::Numstat => diff(repo, receipt, &["--numstat"], None)?,
        Inspect::Stat => diff(repo, receipt, &["--stat"], None)?,
        Inspect::Patch { path } => {
            if let Some(path) = path {
                inside(&changed, path)?;
            }
            diff(repo, receipt, &["--unified=3"], path.as_deref())?
        }
        Inspect::Object { path, side } => {
            inside(&changed, path)?;
            let commit = match side {
                Side::Parent => receipt.parent_head,
                Side::Candidate => receipt.head,
            };
            let spec = format!("{commit}:{path}");
            let listed = git(repo, &["git", "ls-tree", "--name-only", commit, "--", path])?;
            if listed.status != 0 {
                return Err(unreadable_text(&listed));
            }
            if listed.stdout.trim().is_empty() {
                return Err(ReviewerCode::InspectAbsentSide.refusal(format!(
                    "{path:?} has no copy on the {side:?} side of this correction"
                )));
            }
            let size = git(repo, &["git", "cat-file", "-s", &spec])?;
            if size.status != 0 {
                return Err(unreadable_text(&size));
            }
            let bytes: usize = size
                .stdout
                .trim()
                .parse()
                .map_err(|_| unreadable_text(&size))?;
            if bytes > INSPECT_CEILING {
                return Err(ReviewerCode::InputTooLarge.refusal(format!(
                    "{path:?} is {bytes} bytes and the ceiling is {INSPECT_CEILING}"
                )));
            }
            let output = git(repo, &["git", "cat-file", "blob", &spec])?;
            if output.status != 0 {
                return Err(unreadable_text(&output));
            }
            output.stdout
        }
    };
    if text.len() > INSPECT_CEILING {
        return Err(ReviewerCode::InputTooLarge.refusal(format!(
            "the inspection is {} bytes and the ceiling is {INSPECT_CEILING}",
            text.len()
        )));
    }
    Ok(text)
}

fn resolve_parent(repo: &Path, id: &str) -> Result<(), Refusal> {
    let unresolved = |why: &str| {
        ReviewerCode::InspectTransactionUnresolved
            .refusal(format!("the receipt's parent {id:?} {why}"))
    };
    if id.len() < 40 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(unresolved("is not a full object id"));
    }
    let output = transport::run(&["git", "cat-file", "-t", id], Some(repo), How::tolerated())
        .map_err(unreadable)?;
    if output.status != 0 || !matches!(output.stdout.trim(), "commit" | "tree") {
        return Err(unresolved(
            "does not name a commit or tree this checkout holds",
        ));
    }
    Ok(())
}

fn resolve_commit(repo: &Path, id: &str) -> Result<(), Refusal> {
    let unresolved = |why: &str| {
        ReviewerCode::InspectTransactionUnresolved
            .refusal(format!("the receipt's commit {id:?} {why}"))
    };
    if id.len() < 40 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(unresolved("is not a full commit id"));
    }
    let output = transport::run(
        &[
            "git",
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{id}^{{commit}}"),
        ],
        Some(repo),
        How::tolerated(),
    )
    .map_err(unreadable)?;
    if output.status != 0 || output.stdout.trim() != id {
        return Err(unresolved("does not name a commit this checkout holds"));
    }
    Ok(())
}

fn inside(changed: &[String], path: &str) -> Result<(), Refusal> {
    if changed.iter().any(|candidate| candidate == path) {
        return Ok(());
    }
    Err(ReviewerCode::InspectOutsideCorrection.refusal(format!(
        "{path:?} is not a path this correction changed; it changed {changed:?}"
    )))
}

fn diff(
    repo: &Path,
    receipt: &InspectReceipt<'_>,
    shape: &[&str],
    path: Option<&str>,
) -> Result<String, Refusal> {
    let pathspec = path.map(|path| format!(":(literal){path}"));
    let mut rest: Vec<&str> = vec!["--no-renames"];
    rest.extend_from_slice(shape);
    rest.extend([receipt.parent_head, receipt.head, "--"]);
    if let Some(pathspec) = &pathspec {
        rest.push(pathspec);
    }
    let output = git(repo, &transport::pinned_diff_args(&rest))?;
    if output.status != 0 && output.status != 1 {
        return Err(ReviewerCode::InspectUnreadable.refusal(format!(
            "git diff failed ({}): {}",
            output.status,
            output.stderr.trim()
        )));
    }
    Ok(output.stdout)
}

fn git(repo: &Path, args: &[&str]) -> Result<Output, Refusal> {
    let (_guard, envs) = transport::isolated_diff_env(repo).map_err(unreadable)?;
    let envs: Vec<(&str, &str)> = envs
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let output = transport::run_with_isolated_env(args, Some(repo), How::tolerated(), &envs)
        .map_err(unreadable)?;
    if output.stdout_replaced {
        return Err(
            ReviewerCode::InspectUnreadable.refusal("git produced bytes that are not UTF-8")
        );
    }
    Ok(output)
}

fn unreadable_text(output: &Output) -> Refusal {
    ReviewerCode::InspectUnreadable.refusal(format!(
        "git answered {}: {}",
        output.status,
        output.stderr.trim()
    ))
}

fn unreadable(failure: Failure) -> Refusal {
    ReviewerCode::InspectUnreadable.refusal(format!("{failure:?}"))
}

#[cfg(test)]
mod tests;
