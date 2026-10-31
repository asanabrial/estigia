//! What a run holding no claim may write in a checkout whose tracker can name an issue.

use std::path::{Path, PathBuf};

use super::{Action, Decision, GateContext, Sensitivity, session};
use crate::config::IssueRequirement;
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this refusal.
pub const CODE: &str = "issue-required";

/// The requirement the configuration puts in force, or `None` where nothing can be required.
///
/// A claim cannot be made without a tracker a run can hold one against (a transport to swear it
/// through, or a read-only tracker under an assisted workflow) and a contract that says how, so
/// requiring one without them would be a lock nobody can open. Whether the checkout a write lands
/// in can name an issue is a separate question, asked where the write is placed and again for the
/// checkout a call runs in.
pub fn in_force(context: &GateContext) -> Option<IssueRequirement> {
    (context.issue_requirement != IssueRequirement::Off
        && context
            .tracker
            .adjudicates_unclaimed_writes(context.workflow)
        && context.skill_root.join(crate::skill::CONTRACT).is_file())
    .then_some(context.issue_requirement)
}

/// Whether the configured tracker can name an issue for the checkout rooted at `root`.
///
/// GitHub with a named repository names one wherever the checkout is, as does every other tracker.
/// GitHub with none reads it from the checkout's remotes: zero remotes read successfully is a
/// tracker that can name nothing, while a failed read is not zero remotes and stays `true`.
pub(super) fn names_an_issue(context: &GateContext, root: &Path) -> bool {
    match &context.tracker {
        crate::config::Tracker::Github { repo: None } => {
            remotes(root).is_none_or(|remotes| !remotes.is_empty())
        }
        crate::config::Tracker::Github { repo: Some(_) }
        | crate::config::Tracker::Linear { .. } => true,
    }
}

fn remotes(root: &Path) -> Option<Vec<String>> {
    let answer = super::proof_git_command(root, &["remote"]).output().ok()?;
    if !answer.status.success() {
        return None;
    }
    let text = String::from_utf8(answer.stdout).ok()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// The requirement in force for a write landing in the checkout the call runs in.
///
/// [`in_force`] answers for the configuration alone; this narrows it by the checkout, because a
/// GitHub tracker with no named repository has nothing to claim against in a checkout with no
/// remote.
pub(super) fn in_force_here(context: &GateContext) -> Option<IssueRequirement> {
    let requirement = in_force(context)?;
    match scope_of(&context.repo_dir) {
        Scope::Repository(root) if !names_an_issue(context, &root) => None,
        _ => Some(requirement),
    }
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

/// How a hold was reached: by placing the write, or by failing to place it.
#[derive(Clone, Copy)]
pub(crate) enum Reach {
    Placed,
    Unplaceable,
}

/// The decision for a write made by a run holding no claim, or `None` when this is not one.
///
/// A routine write is one, and so is `git push`: it publishes this repository's refs, so it is held
/// as a commit is. Every other boundary is held by the checks that already answer for it, and a
/// write that lands outside every repository the gate can place is not a write to this
/// repository. `declared` are the paths a write tool named in a patch body, empty for every other
/// call; they are placed one by one, and a write to any repository is held. Under `ask` a write
/// the gate cannot place is put to the person when the call was launched in a repository, or from
/// a directory whose repository could not be established, and allowed from outside every
/// repository; `required` holds it either way.
pub fn decision(
    context: &GateContext,
    action: &Action,
    how: Sensitivity,
    literal: Option<&str>,
    declared: &[String],
) -> Option<Decision> {
    if let Some(changed) = watched_change(context) {
        return Some(changed);
    }
    if super::unclaimed::is_a_push(action) {
        let requirement = in_force(context)?;
        let scope = scope_of(&context.repo_dir);
        if matches!(scope, Scope::Outside) {
            return None;
        }
        return held(context, requirement, &scope, Reach::Placed);
    }
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
        Placement::Inside
            if matches!(
                &context.tracker,
                crate::config::Tracker::Github { repo: None }
            ) && no_landing_can_name_an_issue(context, action, literal, declared, &scope) =>
        {
            None
        }
        Placement::Inside => held(context, requirement, &scope, Reach::Placed),
        Placement::Unplaceable => match (requirement, &scope) {
            (IssueRequirement::Watch, Scope::Outside) => {
                Some(Decision::Allow(allowed(requirement, target)))
            }
            (IssueRequirement::Watch, Scope::Repository(root)) => match watch(context, root) {
                Some(()) => Some(Decision::Allow(watching(target))),
                None => held(context, requirement, &scope, Reach::Unplaceable),
            },
            _ => held(context, requirement, &scope, Reach::Unplaceable),
        },
    }
}

/// Whether every repository this write lands in is one the configured tracker cannot name an
/// issue for.
///
/// A target outside every repository grants nothing on its own: the answer is `true` only when at
/// least one target lands inside a repository and none of those repositories can name an issue.
/// A target this process cannot place, or one whose repository it cannot establish, answers
/// `false`, because an unknown is not clearance.
fn no_landing_can_name_an_issue(
    context: &GateContext,
    action: &Action,
    literal: Option<&str>,
    declared: &[String],
    scope: &Scope,
) -> bool {
    let Action::Write { target } = action else {
        return false;
    };
    match literal {
        Some(line) => no_shell_landing_can_name_an_issue(context, target, line, scope),
        None => no_file_landing_can_name_an_issue(context, target, declared),
    }
}

fn no_file_landing_can_name_an_issue(
    context: &GateContext,
    target: &str,
    declared: &[String],
) -> bool {
    let Some(targets) = file_targets(target, declared) else {
        return false;
    };
    let mut inside_a_repository = false;
    for target in targets {
        let Some(landing) = landing(context, target) else {
            return false;
        };
        let Some(root) = repository_of(&landing) else {
            continue;
        };
        inside_a_repository = true;
        if names_an_issue(context, &root) {
            return false;
        }
    }
    inside_a_repository
}

fn no_shell_landing_can_name_an_issue(
    context: &GateContext,
    target: &str,
    line: &str,
    scope: &Scope,
) -> bool {
    if super::shell::runs_a_program_after_pre(line) {
        return false;
    }
    if super::shell::writes_a_file(&line.to_ascii_lowercase()).as_deref() != Some(target) {
        return match scope {
            Scope::Repository(root) => !names_an_issue(context, root),
            _ => false,
        };
    }
    let Some(targets) = super::shell::redirect_targets(line) else {
        return false;
    };
    let mut inside_a_repository = false;
    for target in targets {
        let Some(landing) = landing(context, &target) else {
            return false;
        };
        let Some(root) = repository_of(&landing) else {
            continue;
        };
        inside_a_repository = true;
        if names_an_issue(context, &root) {
            return false;
        }
    }
    inside_a_repository
}

/// The repository `landing` belongs to, or `None` where git names none.
///
/// The first ancestor that exists is the one git is asked: a file that has not been written yet
/// sits under a directory that has, and a directory that will be created sits under one that
/// already exists.
fn repository_of(landing: &Path) -> Option<PathBuf> {
    let mut probe = Some(landing);
    while let Some(directory) = probe {
        if directory.is_dir() {
            return git_root(directory);
        }
        probe = directory.parent();
    }
    None
}

fn held(
    context: &GateContext,
    requirement: IssueRequirement,
    scope: &Scope,
    reach: Reach,
) -> Option<Decision> {
    let assisted = context.workflow == Some(crate::config::Workflow::Assisted);
    if let (IssueRequirement::Watch, Scope::Repository(root)) = (requirement, scope) {
        let _ = std::fs::remove_file(watched_path(context, root));
    }
    match requirement {
        IssueRequirement::Required => Some(Decision::Deny(Box::new(refusal(
            assisted,
            requirement,
            scope,
            reach,
        )))),
        IssueRequirement::Watch => Some(Decision::Ask(Box::new(refusal(
            assisted,
            requirement,
            scope,
            reach,
        )))),
        IssueRequirement::Off => None,
    }
}

fn watching(target: &str) -> String {
    format!(
        "this run holds no issue and `Unclaimed write policy` is `watch`; this call's write ({target}) \
         cannot be placed without running it, so it runs and this checkout's status is compared \
         on the next call, which is put to the person only if the checkout changed"
    )
}

/// Records the status of the checkout rooted at `root` before a write it cannot place, or `None`
/// when the status could not be read or recorded.
fn watch(context: &GateContext, root: &Path) -> Option<()> {
    let status = status_of(root)?;
    let path = watched_path(context, root);
    std::fs::create_dir_all(path.parent()?).ok()?;
    let body = serde_json::to_vec(&WatchedStatus {
        root: root.to_path_buf(),
        status,
    })
    .ok()?;
    crate::paths::replace_bytes_atomically(&path, &body).ok()
}

/// The question owed when the checkout a watched write ran in has changed since, which also
/// stops the watch; `None` when nothing is watched, the status is unchanged or unreadable.
pub(super) fn watched_change(context: &GateContext) -> Option<Decision> {
    let (path, named) = change_since_watch(context)?;
    let _ = std::fs::remove_file(&path);
    Some(Decision::Ask(Box::new(Refusal::not_started(
        CODE,
        format!(
            "this run holds no issue, and `Unclaimed write policy` is `watch`: since a write the gate \
             could not place ran, this checkout changed: {named}. Nothing was reverted"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "whether these changes stay: `git diff` and `git status` show them, and a claimed \
             issue is the way to keep them",
        ),
    ))))
}

/// What a shell call that just ran changed in the watched checkout, said at once; the record
/// stays, so the run's next call is still put to the person.
pub(super) fn watched_note(context: &GateContext) -> Option<String> {
    let (_, named) = change_since_watch(context)?;
    Some(format!(
        "estigia: this run holds no issue and the command that just ran changed this checkout: \
         {named}. Nothing was reverted; the next call is put to the person"
    ))
}

/// The watch record of the launch checkout and the paths changed since it was taken, or `None`
/// when `watch` is not in force, nothing is watched, or the status is unchanged or unreadable.
fn change_since_watch(context: &GateContext) -> Option<(PathBuf, String)> {
    if in_force(context) != Some(IssueRequirement::Watch) {
        return None;
    }
    let Scope::Repository(root) = scope_of(&context.repo_dir) else {
        return None;
    };
    let path = watched_path(context, &root);
    let recorded: WatchedStatus = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    let now = status_of(&root)?;
    let mut changed: Vec<&str> = now
        .symmetric_difference(&recorded.status)
        .map(|entry| entry.path.as_str())
        .collect();
    changed.dedup();
    if changed.is_empty() {
        return None;
    }
    let shown = changed.len().min(8);
    let named = changed[..shown]
        .iter()
        .map(|path| format!("`{path}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let more = if changed.len() > shown {
        format!(", and {} more", changed.len() - shown)
    } else {
        String::new()
    };
    Some((path, format!("{named}{more}")))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WatchedStatus {
    root: PathBuf,
    status: std::collections::BTreeSet<StatusEntry>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct StatusEntry {
    path: String,
    code: String,
    modified: Option<u128>,
    length: Option<u64>,
}

fn watched_path(context: &GateContext, root: &Path) -> PathBuf {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(root.to_string_lossy().as_bytes());
    let name: String = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let state = &context.state_root;
    state
        .parent()
        .unwrap_or(state)
        .join("watched")
        .join(format!("{name}.json"))
}

/// Every path `git status` reports in the checkout, untracked files included, with the time and
/// size it has on disk, so a second write to an already modified file is still a change.
fn status_of(root: &Path) -> Option<std::collections::BTreeSet<StatusEntry>> {
    let answer = super::proof_git_command(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .output()
    .ok()
    .filter(|answer| answer.status.success())?;
    let mut entries = std::collections::BTreeSet::new();
    let mut fields = answer.stdout.split(|byte| *byte == 0);
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let code = String::from_utf8_lossy(&field[..2]).into_owned();
        let path = String::from_utf8_lossy(&field[3..]).into_owned();
        if code.starts_with(['R', 'C']) {
            fields.next();
        }
        let metadata = std::fs::symlink_metadata(root.join(&path)).ok();
        entries.insert(StatusEntry {
            modified: metadata
                .as_ref()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_nanos()),
            length: metadata.as_ref().map(std::fs::Metadata::len),
            path,
            code,
        });
    }
    Some(entries)
}

fn allowed(requirement: IssueRequirement, target: &str) -> String {
    let requirement = requirement.as_value();
    format!(
        "this run holds no issue and `Unclaimed write policy` is `{requirement}`; this call's write ({target}) \
         cannot be placed inside or outside a repository without running it, and this directory \
         is outside every repository, so it is allowed, and a checkout it dirties is named when \
         the next session starts"
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
    let Some(targets) = file_targets(target, declared) else {
        // No field and no body named a path, so `target` is the tool's own name
        // and the destination was never read. Held rather than placed by name,
        // which is what a relative target would otherwise answer for.
        return Placement::Inside;
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

fn file_targets<'a>(target: &'a str, declared: &'a [String]) -> Option<Vec<&'a str>> {
    if declared.is_empty() && super::names_no_write_path(target) {
        return None;
    }
    Some(if declared.is_empty() {
        vec![target]
    } else {
        declared.iter().map(String::as_str).collect()
    })
}

/// A shell write is placed only by literal redirect targets; a named repository operation arrives
/// first in the classifier and writes this checkout itself, and everything else needs running.
fn shell_write(context: &GateContext, target: &str, line: &str, scope: &Scope) -> Placement {
    if target.starts_with("git ")
        && git_directories(line).is_some_and(|directories| {
            directories
                .iter()
                .all(|directory| matches!(against(context, scope, directory), Placement::Outside))
        })
    {
        return Placement::Unplaceable;
    }
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

/// The directories every git command on `line` runs in, when each is literal and absolute: a
/// leading `cd <dir> &&` for the whole line, or `git -C <dir>` on the command itself.
///
/// `None` for anything else — a second `cd`, a `pushd`, a git command with no literal directory,
/// a directory spelled with a variable, a quote or a glob, or `GIT_DIR`/`--git-dir`/
/// `--work-tree`, which send git somewhere the directory does not name.
fn git_directories(line: &str) -> Option<Vec<String>> {
    fn literal(word: &str) -> Option<String> {
        (word.starts_with('/') && !word.contains(|c: char| "$`'\"\\*?~[{(".contains(c)))
            .then(|| word.to_owned())
    }
    let line = line.trim_start();
    let (leading, rest) = match line.strip_prefix("cd ") {
        Some(after) => {
            let after = after.trim_start();
            let end = after.find(char::is_whitespace)?;
            let rest = after[end..].trim_start().strip_prefix("&&")?;
            (Some(literal(&after[..end])?), rest)
        }
        None => (None, line),
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    let bare = |word: &str| -> String {
        word.trim_matches(|c: char| !c.is_alphanumeric() && !"/_-.".contains(c))
            .to_owned()
    };
    if words.iter().any(|word| {
        matches!(
            bare(word).as_str(),
            "cd" | "pushd" | "popd" | "sh" | "bash" | "zsh" | "eval"
        ) || word.contains("GIT_DIR")
            || word.contains("GIT_WORK_TREE")
            || word.starts_with("--git-dir")
            || word.starts_with("--work-tree")
    }) {
        return None;
    }
    let mut directories = Vec::new();
    for (at, word) in words.iter().enumerate() {
        let named_git = bare(word);
        if named_git != "git" && !named_git.ends_with("/git") {
            continue;
        }
        if word.trim_start_matches(|c| ";&|".contains(c)) != named_git {
            return None;
        }
        let mut options = words[at + 1..].iter();
        let mut named = None;
        while let Some(option) = options.next().filter(|option| option.starts_with('-')) {
            match *option {
                "-C" => named = Some(literal(options.next()?)?),
                "-c" => {
                    options.next()?;
                }
                _ => {}
            }
        }
        directories.push(named.or_else(|| leading.clone())?);
    }
    (!directories.is_empty()).then_some(directories)
}

/// Where an absolute target lands, or a relative one from the directory the call runs in.
fn landing(context: &GateContext, target: &str) -> Option<PathBuf> {
    let target = Path::new(target);
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        context.repo_dir.join(target)
    };
    crate::paths::placed(&target)
}

/// Where one named target lands, given the scope the call was launched in.
fn against(context: &GateContext, scope: &Scope, target: &str) -> Placement {
    let Some(landing) = landing(context, target) else {
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
/// The clause bound to `scope` and `reach` keeps it honest: only a scope Git named may name
/// a directory, and a hold reached by *failing* to place the write says so
/// rather than implying the write landed in a repository.
pub(crate) fn refusal(
    assisted: bool,
    requirement: IssueRequirement,
    scope: &Scope,
    reach: Reach,
) -> Refusal {
    let way_in = if assisted {
        "a local claim taken with `assist` in a worktree, which gives the isolated checkout to \
         write in; or `Unclaimed write policy` set to `off` for this repository, when its writes \
         need no claim"
    } else {
        "an issue claimed with `claim`, filed first with `create` when none covers this \
         change, then a branch started with `start_branch`, which gives the isolated \
         checkout to write in; or `Unclaimed write policy` set to `off` for this repository, \
         when its writes need no claim"
    };
    let here = match (scope, reach) {
        (Scope::Repository(_), Reach::Placed) => " in this repository",
        (Scope::Repository(_), Reach::Unplaceable) => {
            ", and this write could not be placed inside or outside this repository without \
             running it, so it is held"
        }
        (Scope::Outside, _) => {
            ", and this write could not be placed outside every repository, so it is held"
        }
        (Scope::Unknown, _) => {
            ", and whether this directory or this write's landing is a repository could not \
             be established"
        }
    };
    Refusal::not_started(
        CODE,
        format!(
            "this run holds no issue, and `Unclaimed write policy` is `{}`{here}: a write \
             without a claim skips the claim, the review receipt and the delivery gate",
            requirement.as_value()
        ),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_in),
    )
}

#[cfg(test)]
mod tests;
