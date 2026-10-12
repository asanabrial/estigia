//! The operator-invoked sweep for branches and checkouts no delivery owns.
//!
//! A delivery removes the remote head it published and the checkout it worked
//! in. Everything else accumulates: a branch pushed under a name other than the
//! pull request's, a checkout whose run died or was reclaimed, a pull request
//! closed without merging, a judge's directory. This module lists what a
//! delivery no longer owns and deletes it only when the operator confirms.
//!
//! **Read-only until `--confirm`.** Discovery answers a list; nothing here
//! deletes on its own, because deleting what no claim owns without a person
//! deciding is a write nobody asked for.
//!
//! Two rules are never relaxed, and they are the whole reason the decision is a
//! pure function: nothing deletes a branch or checkout a live claim holds, and
//! nothing deletes one that carries commits not on the remote.

use std::collections::BTreeMap;
use std::path::Path;

use crate::transport::{Context, Failure, How, branch, run};

use super::session::Run;

/// Which half of the machine a swept thing lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A head on `origin`.
    RemoteHead,
    /// A checkout git has registered.
    Worktree,
}

impl Kind {
    /// The word a person reads.
    pub fn word(self) -> &'static str {
        match self {
            Self::RemoteHead => "remote head",
            Self::Worktree => "worktree",
        }
    }
}

/// One thing the sweep decided about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Swept {
    /// Which half of the machine it is.
    pub kind: Kind,
    /// The branch name or the checkout path.
    pub name: String,
    /// The branch checked out or pushed, when it has one.
    pub branch: Option<String>,
    /// The issue its name or a run pointer attributes it to, when either does.
    pub issue: Option<u64>,
    /// Whether `--confirm` would remove it.
    pub delete: bool,
    /// Why it is listed, and — when it is kept — why it is kept.
    pub reason: String,
}

/// One observation, before the decision. Fed rather than gathered so the rule
/// can be measured without a tracker, a remote and a clock standing by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// Which half of the machine it is.
    pub kind: Kind,
    /// The branch name or the checkout path.
    pub name: String,
    /// The branch checked out or pushed, when it has one.
    pub branch: Option<String>,
    /// The issue its name or a run pointer attributes it to.
    pub issue: Option<u64>,
    /// Why it is worth sweeping at all. `None` when it is not — a branch still
    /// open, a checkout a delivery has not finished with.
    pub sweepable: Option<String>,
    /// Whether it carries commits reachable from no remote.
    pub unpublished: bool,
    /// The run a live claim attributes it to, when one does.
    pub held: Option<String>,
}

/// The decision, and the one place the two never-relaxed rules live.
///
/// Order matters: a checkout that carries unpublished commits is kept even when
/// it is also held, because the two reasons point at the same outcome and the
/// stronger one is the one worth naming.
pub fn decide(observed: Observed) -> Option<Swept> {
    let why = observed.sweepable?;
    let (delete, reason) = if observed.unpublished {
        (false, "carries commits not on the remote".to_owned())
    } else if let Some(holder) = observed.held {
        (false, format!("a live claim holds it ({holder})"))
    } else {
        (true, why)
    };
    Some(Swept {
        kind: observed.kind,
        name: observed.name,
        branch: observed.branch,
        issue: observed.issue,
        delete,
        reason,
    })
}

/// Everything the sweep reads against.
pub struct View<'a> {
    /// The transport context, for `git` and `gh`.
    pub context: &'a Context,
    /// The base branch, which a merged head is an ancestor of.
    pub base: &'a str,
    /// Every run pointer on this machine.
    pub holdings: &'a [Run],
    /// The configured `Worktree location` prefix, when one is set. A registered
    /// checkout under it is the ordinary case; one outside it is the shape a
    /// judge's directory takes.
    pub root: Option<&'a Path>,
}

/// Reads the machine and decides what the sweep would do.
pub fn sweep(view: &View<'_>) -> Result<Vec<Swept>, Failure> {
    let mut out = Vec::new();
    let mut states: BTreeMap<u64, Option<bool>> = BTreeMap::new();
    for observed in observe(view, &mut states)? {
        if let Some(swept) = decide(observed) {
            out.push(swept);
        }
    }
    Ok(out)
}

fn observe(
    view: &View<'_>,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Result<Vec<Observed>, Failure> {
    let mut observed = Vec::new();
    observed.extend(observe_worktrees(view, states)?);
    observed.extend(observe_heads(view, states)?);
    Ok(observed)
}

fn observe_worktrees(
    view: &View<'_>,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Result<Vec<Observed>, Failure> {
    let registered = branch::registered_worktrees(view.context)?;
    let main = branch::normalise_path(&view.context.repo_dir);
    let mut observed = Vec::new();
    for (key, entry) in &registered {
        if *key == main {
            continue;
        }
        let branch = entry.branch.clone();
        let issue = branch.as_deref().and_then(issue_of_branch);
        let held = held_worktree(view, &entry.path, states);
        let unpublished = worktree_unpublished(&entry.path);
        let sweepable = match &branch {
            Some(branch) => {
                let merged = merged(view.context, &format!("refs/heads/{branch}"), view.base);
                let closed = issue.and_then(|issue| issue_closed(view.context, issue, states));
                match (merged, closed) {
                    (Some(true), _) => {
                        Some(format!("branch `{branch}` is merged into {}", view.base))
                    }
                    (_, Some(true)) => {
                        Some(format!("issue #{} is closed", issue.unwrap_or_default()))
                    }
                    _ => outside_the_root(view, &entry.path),
                }
            }
            // A detached checkout names no branch and no issue, so nothing
            // about it says whether it is finished. It is listed as an orphan
            // and deleted only on confirmation.
            None => outside_the_root(view, &entry.path)
                .or_else(|| Some("a detached checkout no branch owns".to_owned())),
        };
        observed.push(Observed {
            kind: Kind::Worktree,
            name: entry.path.display().to_string(),
            branch,
            issue,
            sweepable,
            unpublished,
            held,
        });
    }
    Ok(observed)
}

fn observe_heads(
    view: &View<'_>,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Result<Vec<Observed>, Failure> {
    let listing = run(
        &["git", "ls-remote", "--heads", "origin"],
        Some(&view.context.repo_dir),
        How::tolerated(),
    )?;
    if listing.status != 0 {
        return Err(Failure::Read(format!(
            "git ls-remote --heads origin failed ({}): {}",
            listing.status,
            listing.stderr.trim()
        )));
    }
    let base_ref = format!("refs/heads/{}", view.base);
    let mut observed = Vec::new();
    for line in listing.stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((oid, reference)) = line.split_once('\t') else {
            return Err(Failure::Read(format!(
                "git ls-remote --heads origin answered in an unexpected format: {line:?}"
            )));
        };
        let reference = reference.trim();
        if reference == base_ref {
            continue;
        }
        let Some(branch_name) = reference.strip_prefix("refs/heads/") else {
            continue;
        };
        let branch_name = branch_name.to_owned();
        let issue = issue_of_branch(&branch_name);
        let held = held_head(view, issue, states);
        let merged = merged(view.context, oid.trim(), view.base);
        let closed_issue = issue.and_then(|issue| issue_closed(view.context, issue, states));
        let closed_pr = pr_closed(view.context, &branch_name);
        let sweepable = match (merged, closed_issue, closed_pr) {
            (Some(true), _, _) => Some(format!("`{branch_name}` is merged into {}", view.base)),
            (_, Some(true), _) => Some(format!("issue #{} is closed", issue.unwrap_or_default())),
            (_, _, Some(true)) => Some(format!(
                "`{branch_name}`'s pull request is closed or merged"
            )),
            _ => None,
        };
        observed.push(Observed {
            kind: Kind::RemoteHead,
            name: branch_name.clone(),
            branch: Some(branch_name),
            issue,
            sweepable,
            // A head is on the remote by definition.
            unpublished: false,
            held,
        });
    }
    Ok(observed)
}

/// A checkout under the configured root is the ordinary case; one outside it is
/// the shape a judge's directory takes, which no delivery owns.
fn outside_the_root(view: &View<'_>, path: &Path) -> Option<String> {
    let root = view.root?;
    let under = crate::paths::covers(root, path);
    (!under).then(|| format!("{} is outside the configured worktree root", path.display()))
}

/// The directory prefix of a worktree template, up to its first `<…>` scope.
///
/// The configured `Worktree location` row is a template
/// (`…/worktree/<repo>/<branch>`), not a directory, so "outside the configured
/// root" is asked of the literal prefix the template fixes. A template with no
/// scope at all is its own prefix; `unset` has none.
pub fn root_prefix(template: Option<&str>) -> Option<std::path::PathBuf> {
    let template = template?;
    if template.trim().is_empty() || template.eq_ignore_ascii_case("unset") {
        return None;
    }
    let prefix = match template.find('<') {
        Some(at) => &template[..at],
        None => template,
    };
    let path = std::path::PathBuf::from(prefix);
    path.is_absolute().then_some(path)
}

/// The issue a branch name carries, by the convention `issue-<n>-…`.
fn issue_of_branch(branch: &str) -> Option<u64> {
    branch
        .strip_prefix("issue-")?
        .split('-')
        .next()?
        .parse()
        .ok()
}

/// Whether a run pointer that names this checkout keeps it.
fn held_worktree(
    view: &View<'_>,
    path: &Path,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Option<String> {
    for pointer in view.holdings {
        if pointer.unreadable {
            continue;
        }
        let Some(worktree) = pointer.worktree.as_deref() else {
            continue;
        };
        if crate::paths::same_directory(worktree, path)
            && !pointer_issue_closed(view, pointer, states)
        {
            return Some(pointer.run_id.clone());
        }
    }
    None
}

/// Whether a run pointer that holds this issue keeps a head carrying it.
fn held_head(
    view: &View<'_>,
    issue: Option<u64>,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Option<String> {
    let issue = issue?;
    for pointer in view.holdings {
        if pointer.unreadable || pointer.issue_number() != Some(issue) {
            continue;
        }
        if !pointer_issue_closed(view, pointer, states) {
            return Some(pointer.run_id.clone());
        }
    }
    None
}

/// A pointer's issue read as closed. A failed read is **not** closed: an unknown
/// result is never clearance, and a head kept costs a branch.
fn pointer_issue_closed(
    view: &View<'_>,
    pointer: &Run,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> bool {
    pointer
        .issue_number()
        .and_then(|issue| issue_closed(view.context, issue, states))
        .unwrap_or(false)
}

/// Whether the tracker reports one issue closed. `None` when the read failed.
fn issue_closed(
    context: &Context,
    issue: u64,
    states: &mut BTreeMap<u64, Option<bool>>,
) -> Option<bool> {
    if let Some(known) = states.get(&issue) {
        return *known;
    }
    let answer = crate::transport::gh_json(
        &["issue", "view", &issue.to_string(), "--json", "state"],
        Some(&context.repo_dir),
    )
    .ok()
    .flatten()
    .and_then(|data| {
        data.get("state")
            .and_then(serde_json::Value::as_str)
            .map(|state| state.eq_ignore_ascii_case("closed"))
    });
    states.insert(issue, answer);
    answer
}

/// Whether every pull request for a head is closed or merged. `None` when the
/// read failed or there is no pull request.
fn pr_closed(context: &Context, branch: &str) -> Option<bool> {
    let data = crate::transport::gh_json(
        &[
            "pr", "list", "--head", branch, "--state", "all", "--json", "state",
        ],
        Some(&context.repo_dir),
    )
    .ok()
    .flatten()?;
    let entries = data.as_array()?;
    if entries.is_empty() {
        return None;
    }
    let all_closed = entries.iter().all(|entry| {
        entry
            .get("state")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|state| {
                state.eq_ignore_ascii_case("closed") || state.eq_ignore_ascii_case("merged")
            })
    });
    Some(all_closed)
}

/// Whether a ref is an ancestor of the base. `None` when the read failed — a
/// failed ancestry is never "not merged" and never "merged".
fn merged(context: &Context, reference: &str, base: &str) -> Option<bool> {
    let base_ref = format!("refs/remotes/origin/{base}");
    let answer = run(
        &["git", "merge-base", "--is-ancestor", reference, &base_ref],
        Some(&context.repo_dir),
        How::tolerated(),
    )
    .ok()?;
    match answer.status {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
}

/// Whether a checkout carries work the remote does not have: uncommitted or
/// untracked bytes, or commits reachable from no remote-tracking ref. A failed
/// read is **true**, the safe direction.
fn worktree_unpublished(path: &Path) -> bool {
    let dirty = run(
        &["git", "status", "--porcelain"],
        Some(path),
        How::tolerated(),
    );
    if !matches!(dirty, Ok(output) if output.stdout.trim().is_empty()) {
        return true;
    }
    // `HEAD`, not `--all`: a commit one checkout has not pushed is not a reason
    // to keep every other checkout in the repository.
    match run(
        &["git", "rev-list", "--count", "HEAD", "--not", "--remotes"],
        Some(path),
        How::tolerated(),
    ) {
        Ok(output) => output
            .stdout
            .trim()
            .parse::<u64>()
            .map(|n| n > 0)
            .unwrap_or(true),
        Err(_) => true,
    }
}

/// Removes every swept entry marked for deletion, reading each one back.
pub fn confirm(view: &View<'_>, swept: &[Swept]) -> Vec<serde_json::Value> {
    swept
        .iter()
        .filter(|entry| entry.delete)
        .map(|entry| match entry.kind {
            Kind::RemoteHead => confirm_head(view, &entry.name),
            Kind::Worktree => confirm_worktree(view, entry),
        })
        .collect()
}

fn confirm_head(view: &View<'_>, branch: &str) -> serde_json::Value {
    let retired = branch::retire_remote_branches(view.context, &[branch.to_owned()]);
    let mut value = retired
        .first()
        .map(branch::RemoteRetired::to_json)
        .unwrap_or_else(|| serde_json::json!({ "branch": branch, "deleted": false }));
    value["kind"] = serde_json::json!("remote-head");
    value
}

fn confirm_worktree(view: &View<'_>, entry: &Swept) -> serde_json::Value {
    let path = std::path::PathBuf::from(&entry.name);
    if entry.branch.is_none() {
        return remove_detached_worktree(view, &path);
    }
    let branch = entry.branch.clone();
    let removed = branch::retire_checkout(view.context, &path, &worktree_head(&path));
    let mut value = removed.to_json();
    value["kind"] = serde_json::json!("worktree");
    if branch.is_some() && value.get("branch").is_none() {
        value["branch"] = serde_json::json!(branch);
    }
    value
}

/// Removes a checkout git registered but no branch owns — a judge's directory,
/// which `retire_checkout` refuses because it is not on a branch. Run from the
/// main checkout, never from inside the target.
fn remove_detached_worktree(view: &View<'_>, path: &Path) -> serde_json::Value {
    let name = path.display().to_string();
    let removal = run(
        &["git", "worktree", "remove", "--", name.as_str()],
        Some(&view.context.repo_dir),
        How::write(),
    );
    let mut value = match removal {
        Ok(_) => serde_json::json!({
            "kind": "worktree",
            "worktree": path.display().to_string(),
            "removed": true,
            "outcome": "removed",
        }),
        Err(failure) => serde_json::json!({
            "kind": "worktree",
            "worktree": path.display().to_string(),
            "removed": false,
            "outcome": "kept",
            "detail": failure.detail(),
        }),
    };
    if value.get("outcome").and_then(serde_json::Value::as_str) == Some("removed") {
        let _ = run(
            &["git", "worktree", "prune"],
            Some(&view.context.repo_dir),
            How::write(),
        );
    }
    value["branch"] = serde_json::Value::Null;
    value
}

/// The checkout's own head, which `retire_checkout` compares against the
/// delivery it was proved against. The sweep hands a branch checkout back to
/// itself: the check that matters here is `git status --porcelain`, which
/// `retire_checkout` already makes, and the sweep has no delivery to compare to.
fn worktree_head(path: &Path) -> String {
    run(&["git", "rev-parse", "HEAD"], Some(path), How::tolerated())
        .ok()
        .map(|output| output.stdout.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
