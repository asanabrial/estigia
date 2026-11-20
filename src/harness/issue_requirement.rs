//! What a run holding no claim may not put on a repository's default branch.

use std::path::{Path, PathBuf};

use super::{Action, Decision, GateContext};
use crate::config::{IssueRequirement, UnclaimedGithubWrites};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The stable code of this refusal.
pub const CODE: &str = "issue-required";

const COMMITS: &[&str] = &["git commit", "git cherry-pick", "git revert", "git am"];

/// The requirement the configuration puts in force, or `None` where nothing can be required.
///
/// A claim cannot be made without a tracker a run can hold one against (a transport to swear it
/// through, or a read-only tracker under an assisted workflow) and a contract that says how, so
/// requiring one without them would be a lock nobody can open. Whether the checkout a call runs
/// in can name an issue is a separate question, asked where the session is told and again for the
/// checkout a call runs in.
///
/// A repository file that is there and cannot be read answers [`IssueRequirement::On`]
/// whatever the layers underneath it say: the rows it carries are unknown, and an unknown result
/// is not clearance. The same preconditions still hold, so a checkout with no tracker to claim
/// against is not turned into a lock. The refusal that stops the call carries the read failure
/// rather than the policy, so the operator is shown the file to repair.
pub fn in_force(context: &GateContext) -> Option<IssueRequirement> {
    let available = context
        .tracker
        .adjudicates_unclaimed_writes(context.workflow)
        && context.skill_root.join(crate::skill::CONTRACT).is_file();
    if context.repository_config_unreadable.is_some() {
        return available.then_some(IssueRequirement::On);
    }
    (context.issue_requirement != IssueRequirement::Off && available)
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

/// Where the directory a call was launched from sits with respect to Git.
///
/// Three answers, not two: *no repository* exempts a call the requirement does
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

/// How a hold was reached: by work that reaches the default branch, or by a merge or another
/// boundary `Unclaimed GitHub writes` could have let pass.
#[derive(Clone, Copy)]
pub(crate) enum Reach {
    Placed,
    GithubWrite,
}

#[derive(Default)]
struct Reading {
    commits: bool,
    merges: bool,
    pushes: bool,
    pr_merges: bool,
    other_boundary: bool,
}

impl Reading {
    fn of(action: &Action, literal: Option<&str>) -> Self {
        let mut reading = Self::default();
        match action {
            Action::Boundary { command, .. } => match command.as_str() {
                "git push" => reading.pushes = true,
                "git merge" => reading.merges = true,
                "gh pr merge" => reading.pr_merges = true,
                "git pull" | "gh pr create" => {}
                _ => reading.other_boundary = true,
            },
            Action::Write { target } => reading.commits = COMMITS.contains(&target.as_str()),
            Action::Remote { .. } | Action::Untouched => {}
        }
        if let Some(line) = literal {
            reading.commits |= super::unclaimed::names_any(line, COMMITS);
            reading.merges |= super::unclaimed::names_any(line, &["git merge"]);
            reading.pushes |= super::unclaimed::names_any(line, &["git push"]);
            reading.pr_merges |= super::unclaimed::names_any(line, &["gh pr merge"]);
        }
        reading
    }

    fn is_empty(&self) -> bool {
        !(self.commits || self.merges || self.pushes || self.pr_merges || self.other_boundary)
    }
}

/// The decision for a call made by a run holding no claim, or `None` when this is not one.
///
/// Only what puts work on the repository's default branch is held: a commit-creating git write or
/// a `git merge` while the checkout's current branch is the default branch, a push whose
/// destination is the default branch, and `gh pr merge`. Every other modification passes, and so
/// does `git pull`, `gh pr create` and a GitHub write that is no boundary. A boundary that is none
/// of those (`git tag`, `gh release`, a `gh api` mutation, a command this repository declared
/// irreversible) is held as before unless `Unclaimed GitHub writes` is `allowed` in a repository
/// file this process could read; so are `gh pr merge` and `git merge`.
///
/// What cannot be established is held, for this set only: a current branch that cannot be read, a
/// directory a `cd` or `-C` does not name literally, a push destination the line does not spell.
/// `pushed` are the remote refs a push names when git reported them, empty for every other call.
pub fn decision(
    context: &GateContext,
    action: &Action,
    literal: Option<&str>,
    pushed: &[String],
) -> Option<Decision> {
    let reading = Reading::of(action, literal);
    if reading.is_empty() {
        return None;
    }
    let requirement = in_force(context)?;
    let scope = scope_of(&context.repo_dir);
    let directories = directories(context, literal);
    let on_default = (reading.commits || reading.merges)
        .then(|| may_be_on_the_default_branch(directories.as_deref()));
    let pushes_to_default = reading.pushes
        && push_may_reach_the_default_branch(directories.as_deref(), literal, pushed);
    if (reading.commits && on_default == Some(true)) || pushes_to_default {
        return held(context, requirement, &scope, Reach::Placed);
    }
    if context.unclaimed_github_writes == UnclaimedGithubWrites::Allowed
        && context.repository_config_unreadable.is_none()
    {
        return None;
    }
    let outside = matches!(scope, Scope::Outside);
    let github = (reading.merges && on_default == Some(true))
        || ((reading.pr_merges || reading.other_boundary) && !outside);
    if github {
        return held(context, requirement, &scope, Reach::GithubWrite);
    }
    None
}

fn held(
    context: &GateContext,
    requirement: IssueRequirement,
    scope: &Scope,
    reach: Reach,
) -> Option<Decision> {
    if let Some(refusal) = &context.repository_config_unreadable {
        return Some(Decision::Deny(Box::new(refusal.clone())));
    }
    let assisted = context.workflow == Some(crate::config::Workflow::Assisted);
    match requirement {
        IssueRequirement::On => Some(Decision::Deny(Box::new(refusal(
            assisted,
            requirement,
            scope,
            reach,
        )))),
        IssueRequirement::Off => None,
    }
}

enum Branch {
    Default,
    Other,
    Unread,
    NoRepository,
}

/// The checkouts a call's git commands run in that the tracker can name an issue for, or `None`
/// when the line sends git somewhere it does not name literally.
fn directories(context: &GateContext, literal: Option<&str>) -> Option<Vec<PathBuf>> {
    let named = match literal {
        None => vec![context.repo_dir.clone()],
        Some(line) => match git_directories(line) {
            Some(named) => named.into_iter().map(PathBuf::from).collect(),
            None if moves_git(line) => return None,
            None => vec![context.repo_dir.clone()],
        },
    };
    Some(
        named
            .into_iter()
            .filter(|directory| match scope_of(directory) {
                Scope::Repository(root) => names_an_issue(context, &root),
                Scope::Outside | Scope::Unknown => true,
            })
            .collect(),
    )
}

fn moves_git(line: &str) -> bool {
    line.split_whitespace().any(|word| {
        let bare = word.trim_matches(|c: char| !c.is_alphanumeric() && !"/_-.".contains(c));
        matches!(bare, "cd" | "pushd" | "popd" | "-C")
            || word.contains("GIT_DIR")
            || word.contains("GIT_WORK_TREE")
            || word.starts_with("--git-dir")
            || word.starts_with("--work-tree")
    })
}

fn may_be_on_the_default_branch(directories: Option<&[PathBuf]>) -> bool {
    let Some(directories) = directories else {
        return true;
    };
    directories
        .iter()
        .any(|directory| matches!(branch_of(directory), Branch::Default | Branch::Unread))
}

fn branch_of(directory: &Path) -> Branch {
    let root = match scope_of(directory) {
        Scope::Repository(root) => root,
        Scope::Outside => return Branch::NoRepository,
        Scope::Unknown => return Branch::Unread,
    };
    match current_branch(&root) {
        Some(Some(name)) if is_the_default_branch(&root, &name) => Branch::Default,
        Some(_) => Branch::Other,
        None => Branch::Unread,
    }
}

/// The branch `HEAD` names: `Some(None)` for a detached `HEAD`, `None` when it cannot be read.
fn current_branch(root: &Path) -> Option<Option<String>> {
    let answer = super::proof_git_command(root, &["symbolic-ref", "-q", "--short", "HEAD"])
        .output()
        .ok()?;
    if answer.status.success() {
        let name = String::from_utf8(answer.stdout).ok()?.trim().to_owned();
        return (!name.is_empty()).then_some(Some(name));
    }
    (answer.status.code() == Some(1)).then_some(None)
}

fn is_the_default_branch(root: &Path, name: &str) -> bool {
    match default_branch(root) {
        Some(default) => default == name,
        None => matches!(name, "main" | "master"),
    }
}

fn default_branch(root: &Path) -> Option<String> {
    let answer = super::proof_git_command(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .output()
    .ok()?;
    if !answer.status.success() {
        return None;
    }
    let text = String::from_utf8(answer.stdout).ok()?;
    text.trim().strip_prefix("origin/").map(str::to_owned)
}

fn names_the_default_branch(directory: &Path, name: &str) -> bool {
    match scope_of(directory) {
        Scope::Repository(root) => is_the_default_branch(&root, name),
        Scope::Outside => false,
        Scope::Unknown => true,
    }
}

fn push_may_reach_the_default_branch(
    directories: Option<&[PathBuf]>,
    literal: Option<&str>,
    pushed: &[String],
) -> bool {
    let Some(directories) = directories else {
        return true;
    };
    if directories.is_empty() {
        return false;
    }
    if !pushed.is_empty() {
        return directories.iter().any(|directory| {
            pushed
                .iter()
                .any(|remote| destination_may_be_the_default_branch(remote, directory))
        });
    }
    let Some(line) = literal else {
        return true;
    };
    let pushes = pushes_in(line);
    pushes.is_empty()
        || directories.iter().any(|directory| {
            pushes
                .iter()
                .any(|arguments| arguments_may_reach_the_default_branch(arguments, directory))
        })
}

fn destination_may_be_the_default_branch(destination: &str, directory: &Path) -> bool {
    if destination.is_empty() {
        return true;
    }
    let name = destination
        .strip_prefix("refs/heads/")
        .unwrap_or(destination);
    !name.starts_with("refs/") && names_the_default_branch(directory, name)
}

/// The words after `push` in every `git push` the line spells out.
fn pushes_in(line: &str) -> Vec<Vec<String>> {
    let text = super::shell::unquote_plain_words(line);
    let mut found = Vec::new();
    for segment in text.split(|c: char| ";&|()`\n".contains(c)) {
        let words: Vec<&str> = segment.split_whitespace().collect();
        let Some(at) = words
            .iter()
            .position(|word| word.rsplit('/').next() == Some("git"))
        else {
            continue;
        };
        let mut rest = words[at + 1..].iter();
        let mut subcommand = None;
        while let Some(word) = rest.next() {
            match *word {
                "-C" | "-c" => {
                    rest.next();
                }
                option if option.starts_with('-') => {}
                named => {
                    subcommand = Some(named);
                    break;
                }
            }
        }
        if subcommand == Some("push") {
            found.push(rest.map(|word| (*word).to_owned()).collect());
        }
    }
    found
}

fn arguments_may_reach_the_default_branch(arguments: &[String], directory: &Path) -> bool {
    let mut positional: Vec<&str> = Vec::new();
    let mut words = arguments.iter();
    while let Some(word) = words.next() {
        if word.contains(['<', '>']) {
            break;
        }
        match word.as_str() {
            "--all" | "--mirror" => return true,
            "-o" | "--push-option" | "--receive-pack" | "--exec" | "--repo" => {
                words.next();
            }
            option if option.starts_with('-') => {}
            named => positional.push(named),
        }
    }
    let refspecs = positional.get(1..).unwrap_or_default();
    if refspecs.is_empty() {
        return current_may_be_the_default_branch(directory);
    }
    refspecs
        .iter()
        .any(|refspec| refspec_may_reach_the_default_branch(refspec, directory))
}

fn current_may_be_the_default_branch(directory: &Path) -> bool {
    matches!(branch_of(directory), Branch::Default | Branch::Unread)
}

fn refspec_may_reach_the_default_branch(refspec: &str, directory: &Path) -> bool {
    if refspec.contains(|c: char| "$`'\"\\*?~[{^".contains(c)) {
        return true;
    }
    let refspec = refspec.trim_start_matches('+');
    let destination = refspec.rsplit_once(':').map_or(refspec, |(_, to)| to);
    match destination {
        "" => true,
        "HEAD" => current_may_be_the_default_branch(directory),
        named => destination_may_be_the_default_branch(named, directory),
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
        (Path::new(word).is_absolute() && !word.contains(|c: char| "$`'\"\\*?~[{(".contains(c)))
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

/// The refusal for an unclaimed call that would put work on the default branch.
///
/// The clause bound to `scope` keeps it honest: only a scope Git named may name a directory.
pub(crate) fn refusal(
    assisted: bool,
    requirement: IssueRequirement,
    scope: &Scope,
    reach: Reach,
) -> Refusal {
    let way_in = if assisted {
        "a local claim taken with `assist` in a worktree, which gives the isolated checkout to \
         work in; or `Issue required` set to `off` for this repository, when its default branch \
         needs no claim"
    } else {
        "an issue claimed with `claim`, filed first with `create` when none covers this \
         change, then a branch started with `claim`'s `branch` and `base`, which gives the isolated \
         checkout to work in; or `Issue required` set to `off` for this repository, \
         when its default branch needs no claim"
    };
    let way_in = match reach {
        Reach::GithubWrite => format!(
            "{way_in}; or `Unclaimed GitHub writes` set to `allowed` for this repository, when \
             only its GitHub writes need no claim"
        ),
        Reach::Placed => way_in.to_string(),
    };
    let here = match scope {
        Scope::Repository(_) => " in this repository",
        Scope::Outside => {
            ", and this call reaches a repository from a directory that is not inside one"
        }
        Scope::Unknown => {
            ", and whether this directory or the repository this call reaches is a repository \
             could not be established, so the call is held"
        }
    };
    Refusal::not_started(
        CODE,
        format!(
            "this run holds no issue, and `Issue required` is `{}`{here}: work reaching the \
             default branch without an issue and a pull request skips the claim, the review \
             receipt and the delivery gate",
            requirement.as_value()
        ),
        Resolution::no_command(NoCommandReason::OperatorKnowledge, way_in),
    )
}

#[cfg(test)]
mod tests;
