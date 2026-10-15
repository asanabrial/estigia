//! Uncommitted work, per checkout.
//!
//! Work is lost by being forgotten more often than by being deleted. This module
//! reads every checkout git has registered for the repository, and every one a
//! run pointer names, and reports what each holds that no commit does. Git is the
//! only source: nothing is attributed to a tool, and nothing here writes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::transport::{Context, Failure, How, branch, run};

use super::session::Run;

/// One uncommitted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// `M`odified, `A`dded, `D`eleted, `R`enamed or `U`ntracked.
    pub marker: char,
    /// The path relative to the checkout.
    pub path: String,
    /// Lines added, when git or the file can say.
    pub added: Option<u64>,
    /// Lines removed, when git can say.
    pub removed: Option<u64>,
}

/// The run that holds a checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// The run pointer's id.
    pub run_id: String,
    /// The issue the run holds.
    pub issue: Option<u64>,
}

/// What a checkout's `HEAD` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    /// On a branch.
    Branch,
    /// Detached.
    Detached,
    /// On a branch with no commit yet.
    NoCommits,
}

impl Head {
    /// The word JSON and a person read.
    pub fn word(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Detached => "detached",
            Self::NoCommits => "no commits yet",
        }
    }
}

/// One checkout, as git reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    /// Where it is.
    pub path: PathBuf,
    /// The branch checked out, when there is one.
    pub branch: Option<String>,
    /// What `HEAD` is. `None` when git could not be read here.
    pub head: Option<Head>,
    /// Why git could not be read here. Everything below is empty when it is set.
    pub unreadable: Option<String>,
    /// The uncommitted files.
    pub files: Vec<Change>,
    /// Commits not on the upstream, when it has one.
    pub ahead: Option<u64>,
    /// Unix seconds of the newest uncommitted file still on disk.
    pub modified: Option<u64>,
    /// The run that holds it, when one does.
    pub holder: Option<Holder>,
    /// Whether no run pointer holds it.
    pub orphan: bool,
}

const GIT_WAIT: Duration = Duration::from_secs(15);
const COUNTED_BYTES: u64 = 1 << 20;

fn read() -> How {
    How {
        timeout: Some(GIT_WAIT),
        ..How::tolerated()
    }
}

/// Every checkout of the repository and every one a run pointer names.
pub fn survey(context: &Context, holdings: &[Run]) -> Result<Vec<Tree>, Failure> {
    let registered = branch::registered_worktrees(context)?;
    let mut paths: Vec<PathBuf> = registered
        .values()
        .map(|entry| entry.path.clone())
        .collect();
    for pointer in holdings.iter().filter(|pointer| !pointer.unreadable) {
        if let Some(named) = &pointer.worktree
            && !paths
                .iter()
                .any(|known| crate::paths::same_directory(known, named))
        {
            paths.push(named.clone());
        }
    }
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| read_tree(path, holdings))
        .collect())
}

fn read_tree(path: PathBuf, holdings: &[Run]) -> Tree {
    let holder = holdings
        .iter()
        .filter(|pointer| !pointer.unreadable)
        .find(|pointer| {
            pointer
                .worktree
                .as_deref()
                .is_some_and(|named| crate::paths::same_directory(named, &path))
        })
        .map(|pointer| Holder {
            run_id: pointer.run_id.clone(),
            issue: pointer.issue_number(),
        });
    let orphan = holder.is_none();
    let mut tree = Tree {
        path,
        branch: None,
        head: None,
        unreadable: None,
        files: Vec::new(),
        ahead: None,
        modified: None,
        holder,
        orphan,
    };
    match uncommitted(&tree.path) {
        Ok(files) => tree.files = files,
        Err(why) => {
            tree.unreadable = Some(why);
            return tree;
        }
    }
    let symbolic = git(&tree.path, &["symbolic-ref", "-q", "--short", "HEAD"]);
    match symbolic {
        Some(name) => {
            tree.branch = Some(name.trim().to_owned());
            let born = git(&tree.path, &["rev-parse", "--verify", "-q", "HEAD"]).is_some();
            tree.head = Some(if born { Head::Branch } else { Head::NoCommits });
        }
        None => tree.head = Some(Head::Detached),
    }
    tree.ahead = git(&tree.path, &["rev-list", "--count", "@{u}..HEAD"])
        .and_then(|count| count.trim().parse().ok());
    tree.modified = tree
        .files
        .iter()
        .filter_map(|change| std::fs::metadata(tree.path.join(&change.path)).ok())
        .filter_map(|meta| meta.modified().ok())
        .filter_map(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_secs())
        .max();
    tree
}

fn git(at: &Path, arguments: &[&str]) -> Option<String> {
    let output = run(
        &["git"]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>(),
        Some(at),
        read(),
    )
    .ok()?;
    (output.status == 0).then_some(output.stdout)
}

/// The uncommitted files of one checkout. An `Err` is a checkout git could not
/// be read in, which is never the same answer as a clean one.
pub fn uncommitted(path: &Path) -> Result<Vec<Change>, String> {
    let output = run(
        &["git", "status", "--porcelain=v1", "-z", "-uall"],
        Some(path),
        read(),
    )
    .map_err(|failure| failure.detail())?;
    if output.status != 0 {
        return Err(format!(
            "git status failed ({}): {}",
            output.status,
            output.stderr.trim()
        ));
    }
    let counts = numstat(path);
    let mut files = Vec::new();
    let mut entries = output.stdout.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(code), Some(name)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        if code.contains(['R', 'C']) {
            entries.next();
        }
        let marker = marker_of(code);
        let (added, removed) = if marker == 'U' {
            (untracked_lines(&path.join(name)), None)
        } else {
            counts.get(name).copied().unwrap_or((None, None))
        };
        files.push(Change {
            marker,
            path: name.to_owned(),
            added,
            removed,
        });
    }
    Ok(files)
}

fn marker_of(code: &str) -> char {
    if code == "??" {
        'U'
    } else if code.contains(['R', 'C']) {
        'R'
    } else if code.contains('D') {
        'D'
    } else if code.contains('A') {
        'A'
    } else {
        'M'
    }
}

type Counts = std::collections::HashMap<String, (Option<u64>, Option<u64>)>;

fn numstat(path: &Path) -> Counts {
    let mut counts = Counts::new();
    let Some(text) = git(path, &["diff", "--numstat", "-z", "--no-renames", "HEAD"]) else {
        return counts;
    };
    for entry in text.split('\0') {
        let mut fields = entry.splitn(3, '\t');
        if let (Some(added), Some(removed), Some(name)) =
            (fields.next(), fields.next(), fields.next())
        {
            counts.insert(name.to_owned(), (added.parse().ok(), removed.parse().ok()));
        }
    }
    counts
}

fn untracked_lines(file: &Path) -> Option<u64> {
    let meta = std::fs::metadata(file).ok()?;
    if !meta.is_file() || meta.len() > COUNTED_BYTES {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
    let unterminated = bytes.last().is_some_and(|last| *last != b'\n');
    Some(newlines + u64::from(unterminated))
}

const NOTICE_BUDGET: Duration = Duration::from_secs(3);
const NOTICE_READ: Duration = Duration::from_secs(2);
const NAMED_TREES: usize = 5;
const NAMED_FILES: usize = 5;

/// What a session is told at its start about linked checkouts no run holds and
/// that carry uncommitted files. The base checkout is not named: it is not a
/// place work is parked. Reading stops at a short budget and says how many
/// checkouts it did not reach, because a checkout not read is not a clean one.
pub fn orphan_notice(context: &Context, holdings: &[Run]) -> Option<String> {
    let registered = branch::registered_worktrees(context).ok()?;
    let started = std::time::Instant::now();
    orphan_notice_of(&registered, holdings, || started.elapsed() > NOTICE_BUDGET)
}

type Registered = std::collections::BTreeMap<String, branch::RegisteredWorktree>;

fn orphan_notice_of(
    registered: &Registered,
    holdings: &[Run],
    mut over_budget: impl FnMut() -> bool,
) -> Option<String> {
    let mut dirty: Vec<(PathBuf, Option<String>, usize)> = Vec::new();
    let mut unread = 0usize;
    for entry in registered.values() {
        if entry.path.join(".git").is_dir()
            || is_bare_repository(&entry.path)
            || holdings
                .iter()
                .filter(|pointer| !pointer.unreadable)
                .any(|pointer| {
                    pointer
                        .worktree
                        .as_deref()
                        .is_some_and(|named| crate::paths::same_directory(named, &entry.path))
                })
        {
            continue;
        }
        if over_budget() {
            unread += 1;
            continue;
        }
        match quick_status(&entry.path) {
            Some(count) if count > 0 => {
                dirty.push((entry.path.clone(), entry.branch.clone(), count));
            }
            Some(_) => {}
            None => unread += 1,
        }
    }
    if dirty.is_empty() && unread == 0 {
        return None;
    }
    dirty.sort();
    let mut text = String::new();
    if !dirty.is_empty() {
        let named = dirty
            .iter()
            .take(NAMED_TREES)
            .map(|(path, branch, count)| {
                format!(
                    "`{}` ({}, {count} uncommitted file{})",
                    path.display(),
                    branch.as_deref().unwrap_or("detached"),
                    if *count == 1 { "" } else { "s" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let more = dirty.len().saturating_sub(NAMED_TREES);
        text.push_str(&format!(
            "{} checkout{} no run holds carr{} uncommitted work: {named}{}.",
            dirty.len(),
            if dirty.len() == 1 { "" } else { "s" },
            if dirty.len() == 1 { "ies" } else { "y" },
            if more > 0 {
                format!(", and {more} more")
            } else {
                String::new()
            }
        ));
    }
    if unread > 0 {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&format!(
            "{unread} checkout{} could not be read in time, so what {} hold{} is unknown.",
            if unread == 1 { "" } else { "s" },
            if unread == 1 { "it" } else { "they" },
            if unread == 1 { "s" } else { "" }
        ));
    }
    text.push_str(
        " `estigia changes` lists every checkout and what it holds; nothing here is removed.",
    );
    Some(text)
}

fn is_bare_repository(path: &Path) -> bool {
    !path.join(".git").exists() && path.join("HEAD").is_file() && path.join("objects").is_dir()
}

fn quick_status(path: &Path) -> Option<usize> {
    let output = run(
        &["git", "status", "--porcelain=v1", "-z", "-uall"],
        Some(path),
        How {
            timeout: Some(NOTICE_READ),
            ..How::tolerated()
        },
    )
    .ok()?;
    (output.status == 0).then(|| {
        output
            .stdout
            .split('\0')
            .filter(|entry| entry.len() > 3)
            .count()
    })
}

/// What `done` and `release` say about the checkout the run was working in:
/// the files it still holds uncommitted, and the command that lists them. A
/// checkout git cannot be read in, or that is gone, says nothing. This is a
/// notice and never a refusal; `release` already keeps a checkout that carries
/// them.
pub fn held_notice(worktree: &Path) -> Option<String> {
    let files = uncommitted(worktree).ok()?;
    if files.is_empty() {
        return None;
    }
    let named = files
        .iter()
        .take(NAMED_FILES)
        .map(|change| format!("`{}`", change.path))
        .collect::<Vec<_>>()
        .join(", ");
    let more = files.len().saturating_sub(NAMED_FILES);
    Some(format!(
        "estigia: this run's checkout `{}` still holds {} uncommitted file{}: {named}{}. \
         Nothing was refused and nothing was removed; commit or publish them, or keep the \
         checkout. `estigia changes` lists them.",
        worktree.display(),
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        if more > 0 {
            format!(", and {more} more")
        } else {
            String::new()
        }
    ))
}

#[cfg(test)]
mod tests;
