//! The sweep for branches and checkouts no delivery owns.
//!
//! A delivery removes the remote head it published and the checkout it worked
//! in. Everything else accumulates: a branch pushed under a name other than the
//! pull request's, a checkout whose run died or was reclaimed, a pull request
//! closed without merging, a judge's directory. This module lists what a
//! delivery no longer owns and deletes it only on `--confirm`.
//!
//! **No removal until `--confirm`.** Discovery refreshes remote refs and answers a list; nothing here
//! deletes on its own. The listing is the decision: the contract has whoever
//! reads `sweep` rows pass `--confirm`, operator or run, without asking again.
//!
//! **Nothing is dropped from the listing.** A checkout whose branch is neither
//! merged nor closed can also qualify as a remote-gone orphan. Otherwise it is
//! kept with its reason, so the sweep never hides work it declines to delete.
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
    /// Whether it carries commits reachable from no remote, or uncommitted work.
    pub unpublished: bool,
    /// The uncommitted files, each as `<marker> <path>`.
    pub uncommitted: Vec<String>,
    /// The run a live claim attributes it to, when one does.
    pub held: Option<String>,
    /// A failed or negative read that keeps a remote-gone candidate.
    pub kept: Option<String>,
}

/// The decision, and the one place the two never-relaxed rules live.
///
/// Order matters: a checkout that carries unpublished commits is kept even when
/// it is also held, because the two reasons point at the same outcome and the
/// stronger one is the one worth naming.
pub fn decide(observed: Observed) -> Option<Swept> {
    if observed.sweepable.is_none() && observed.kind != Kind::Worktree {
        return None;
    }
    let (delete, reason) = if let Some(why) = observed.kept {
        (false, why)
    } else if observed.unpublished {
        (false, carries(&observed.uncommitted))
    } else if let Some(holder) = observed.held {
        (false, format!("a live claim holds it ({holder})"))
    } else if let Some(why) = observed.sweepable {
        (true, why)
    } else {
        (
            false,
            "its branch is neither merged nor closed on this tracker".to_owned(),
        )
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

const NAMED_FILES: usize = 5;

fn carries(uncommitted: &[String]) -> String {
    if uncommitted.is_empty() {
        return "carries commits not on the remote".to_owned();
    }
    let named: Vec<&str> = uncommitted
        .iter()
        .take(NAMED_FILES)
        .map(String::as_str)
        .collect();
    let more = match uncommitted.len().saturating_sub(NAMED_FILES) {
        0 => String::new(),
        rest => format!(", and {rest} more"),
    };
    format!(
        "carries {} uncommitted file(s): {}{more}",
        uncommitted.len(),
        named.join(", ")
    )
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
    /// Initial ownership observations, retained conservatively alongside fresh reads.
    pub ownership: Result<Vec<std::path::PathBuf>, String>,
    /// The pointer directory used to refresh ownership before orphan retirement.
    pub ownership_root: Option<std::path::PathBuf>,
}

/// Reads pointer and session directories conservatively, refusing corrupt or
/// unlocated ownership. Silence is not proof that a session ended.
pub fn ownership(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let unreadable = super::session::unreadable_holdings(root);
    if !unreadable.is_empty() {
        return Err(format!(
            "ownership could not be read: {}",
            unreadable.join(", ")
        ));
    }
    let mut paths = Vec::new();
    for pointer in super::session::holdings(root) {
        if pointer.worktree.is_none() && pointer.repo_dir.is_none() {
            return Err(format!(
                "ownership has no readable checkout ({})",
                pointer.run_id
            ));
        }
        paths.extend(pointer.worktree);
        paths.extend(pointer.repo_dir);
    }
    let opened = crate::transport::store::open_for_read(&crate::transport::store::beside(root))
        .map_err(|failure| failure.detail())?;
    let Some(opened) = opened else {
        return Ok(paths);
    };
    let sessions = opened
        .sessions()
        .map_err(|failure| failure.detail())?
        .into_iter()
        .filter(|row| row.ended_at.is_none())
        .map(|row| {
            row.repo_dir
                .map(std::path::PathBuf::from)
                .filter(|path| path.is_absolute())
                .ok_or_else(|| format!("session {} has no readable checkout", row.run_id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.extend(sessions);
    Ok(paths)
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
    let mut refreshed = None;
    for (key, entry) in &registered {
        if *key == main {
            continue;
        }
        let branch = entry.branch.clone();
        let issue = branch.as_deref().and_then(issue_of_branch);
        let held = held_worktree(view, &entry.path, states);
        let (mut unpublished, uncommitted) = worktree_unpublished(&entry.path);
        let mut kept = None;
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
                    _ => outside_the_root(view, &entry.path).or_else(|| {
                        match remote_gone(view, &entry.path, branch, &mut refreshed) {
                            Ok(_) => {
                                unpublished = false;
                                Some(REMOTE_GONE.to_owned())
                            }
                            Err(why) => {
                                kept = Some(why);
                                None
                            }
                        }
                    }),
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
            uncommitted,
            held,
            kept,
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
            uncommitted: Vec::new(),
            held,
            kept: None,
        });
    }
    Ok(observed)
}

const REMOTE_GONE: &str = "its remote branch is gone and every commit is on a remote";

fn check_ownership(view: &View<'_>, path: &Path) -> Result<(), String> {
    if let Some(pointer) = view.holdings.iter().find(|pointer| {
        pointer.unreadable
            || (pointer.worktree.is_none() && pointer.repo_dir.is_none())
            || pointer
                .worktree
                .iter()
                .chain(pointer.repo_dir.iter())
                .any(|named| crate::paths::same_directory(named, path))
    }) {
        return Err(format!("ownership is held or unknown ({})", pointer.run_id));
    }
    let sessions = view
        .ownership
        .as_ref()
        .map_err(|why| format!("ownership could not be read: {why}"))?;
    if sessions
        .iter()
        .any(|named| crate::paths::same_directory(named, path))
    {
        return Err("a session not known ended holds this checkout".to_owned());
    }
    let root = view
        .ownership_root
        .as_deref()
        .ok_or_else(|| "ownership root could not be read".to_owned())?;
    let latest = ownership(root).map_err(|why| format!("ownership could not be read: {why}"))?;
    if latest
        .iter()
        .any(|named| crate::paths::same_directory(named, path))
    {
        return Err("ownership is held by a session or run pointer not known released".to_owned());
    }
    Ok(())
}

fn check_branch(path: &Path, branch: &str) -> Result<(), String> {
    let current = run(
        &["git", "symbolic-ref", "--quiet", "--short", "HEAD"],
        Some(path),
        How::read(),
    )
    .map_err(|failure| format!("checkout state could not be read: {}", failure.detail()))?;
    if current.stdout.trim() != branch {
        return Err("the checkout's branch changed".to_owned());
    }
    Ok(())
}

fn tracking_identity(path: &Path, branch: &str) -> Result<(String, String), String> {
    let remote = config_one(path, &format!("branch.{branch}.remote"))?;
    let reference = config_one(path, &format!("branch.{branch}.merge"))?;
    let (remote, reference) = match (remote, reference) {
        (Some(remote), Some(reference)) => (remote, reference),
        (None, None) => return Err("no configured upstream; untracked checkout is kept".to_owned()),
        _ => return Err("tracking identity could not be read completely".to_owned()),
    };
    if remote == "."
        || remote.starts_with('-')
        || config_one(path, &format!("remote.{remote}.url"))?.is_none()
        || !reference.starts_with("refs/heads/")
    {
        return Err("tracking identity is local-only, invalid or unknown".to_owned());
    }
    run(
        &["git", "check-ref-format", &reference],
        Some(path),
        How::read(),
    )
    .map_err(|failure| format!("tracking identity is invalid: {}", failure.detail()))?;
    Ok((remote, reference))
}

fn config_one(path: &Path, key: &str) -> Result<Option<String>, String> {
    let output = run(
        &["git", "config", "--get-all", key],
        Some(path),
        How::tolerated(),
    )
    .map_err(|failure| format!("tracking identity could not be read: {}", failure.detail()))?;
    let values: Vec<&str> = output.stdout.lines().collect();
    match (output.status, values.as_slice()) {
        (0, [value]) if !value.trim().is_empty() => Ok(Some((*value).to_owned())),
        (1, []) => Ok(None),
        _ => Err(format!(
            "tracking identity could not be read unambiguously ({key})"
        )),
    }
}

fn remote_gone(
    view: &View<'_>,
    path: &Path,
    branch: &str,
    refreshed: &mut Option<Result<(), String>>,
) -> Result<String, String> {
    check_ownership(view, path)?;
    check_branch(path, branch)?;
    let files = super::changes::uncommitted(path)
        .map_err(|why| format!("checkout state could not be read: {why}"))?;
    if !files.is_empty() {
        return Err(carries(
            &files
                .iter()
                .map(|file| format!("{} {}", file.marker, file.path))
                .collect::<Vec<_>>(),
        ));
    }
    let (tracking_remote, reference) = tracking_identity(path, branch)?;
    refreshed
        .get_or_insert_with(|| {
            run(
                &["git", "fetch", "--prune", "--all"],
                Some(&view.context.repo_dir),
                How::read(),
            )
            .map(|_| ())
            .map_err(|failure| format!("remote state could not be read: {}", failure.detail()))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let remote = run(
        &[
            "git",
            "ls-remote",
            "--exit-code",
            "--heads",
            &tracking_remote,
            &reference,
        ],
        Some(path),
        How::tolerated(),
    )
    .map_err(|failure| format!("remote state could not be read: {}", failure.detail()))?;
    match remote.status {
        0 => return Err("its branch is neither merged nor closed on this tracker; its remote branch still exists".to_owned()),
        2 if remote.stdout.trim().is_empty() => {}
        _ => return Err(format!("remote state could not be read: {}", remote.stderr.trim())),
    }
    let head = run(
        &["git", "rev-parse", "--verify", "HEAD^{commit}"],
        Some(path),
        How::read(),
    )
    .map_err(|failure| format!("checkout head could not be read: {}", failure.detail()))?
    .stdout
    .trim()
    .to_ascii_lowercase();
    let commits = run(
        &["git", "rev-list", "--count", &head, "--not", "--remotes"],
        Some(path),
        How::read(),
    )
    .map_err(|failure| format!("checkout commits could not be read: {}", failure.detail()))?;
    match commits.stdout.trim().parse::<u64>() {
        Ok(0) => Ok(head),
        Ok(_) => Err(carries(&[])),
        Err(_) => Err("checkout commits could not be read".to_owned()),
    }
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

/// Whether a checkout carries work the remote does not have, and the
/// uncommitted files it names: uncommitted or untracked bytes, or commits
/// reachable from no remote-tracking ref. A failed read is **true**, the safe
/// direction.
fn worktree_unpublished(path: &Path) -> (bool, Vec<String>) {
    let files = match super::changes::uncommitted(path) {
        Ok(files) => files,
        Err(_) => return (true, Vec::new()),
    };
    if !files.is_empty() {
        let named = files
            .iter()
            .map(|change| format!("{} {}", change.marker, change.path))
            .collect();
        return (true, named);
    }
    // `HEAD`, not `--all`: a commit one checkout has not pushed is not a reason
    // to keep every other checkout in the repository.
    let unpushed = match run(
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
    };
    (unpushed, Vec::new())
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
    confirm_worktree_after(view, entry, || {})
}

fn confirm_worktree_after(
    view: &View<'_>,
    entry: &Swept,
    after_proof: impl FnOnce(),
) -> serde_json::Value {
    let path = std::path::PathBuf::from(&entry.name);
    let kept = |why| {
        let mut value = branch::Retired {
            worktree: path.clone(),
            worktree_removed: false,
            branch: entry.branch.clone(),
            branch_deleted: false,
            reason: Some(why),
        }
        .to_json();
        value["kind"] = serde_json::json!("worktree");
        value
    };
    let proven = if entry.reason == REMOTE_GONE {
        let checked = entry
            .branch
            .as_deref()
            .ok_or_else(|| "checkout branch could not be read".to_owned())
            .and_then(|branch| remote_gone(view, &path, branch, &mut None));
        match checked {
            Ok(head) => Some(head),
            Err(why) => return kept(why),
        }
    } else {
        None
    };
    if entry.branch.is_none() {
        return remove_detached_worktree(view, &path);
    }
    after_proof();
    if proven.is_some() {
        let checked = check_ownership(view, &path)
            .and_then(|()| check_branch(&path, entry.branch.as_deref().unwrap_or_default()));
        if let Err(why) = checked {
            return kept(why);
        }
    }
    let branch = entry.branch.clone();
    let expected = proven.unwrap_or_else(|| worktree_head(&path));
    let removed = branch::retire_checkout(view.context, &path, &expected);
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

fn worktree_head(path: &Path) -> String {
    run(&["git", "rev-parse", "HEAD"], Some(path), How::tolerated())
        .ok()
        .map(|output| output.stdout.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
