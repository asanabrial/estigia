//! Whether the repository's git operation window is open for this dispatch.
//!
//! The answer is read once, where an operation starts, and every later
//! decision in that operation reads the same answer; a second reading could
//! disagree with the first across a boundary minute. [`tripwire`] holds the
//! other half: while the answer is closed, the process boundary itself refuses
//! to spawn a call that would reach GitHub.

use std::cell::RefCell;

use super::{Context, Failure, How};
use crate::config::GitOperationWindow;
use crate::harness::operation_window as schedule;

/// The window's answer for one operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Nothing is held back: the window is disabled or the clock is inside it.
    Open,
    /// Nothing may reach GitHub; the schedule that said so is carried for the
    /// refusal that names it.
    Closed(GitOperationWindow),
}

std::thread_local! {
    static SCOPE: RefCell<Option<Mode>> = const { RefCell::new(None) };
}

/// The mode a dispatch decided for this thread, if one is running.
pub(crate) fn scoped() -> Option<Mode> {
    SCOPE.with(|cell| cell.borrow().clone())
}

/// Runs `body` with `mode` as the answer every [`state`] call returns.
///
/// Restored afterwards even when `body` panics, and nested scopes restore the
/// one they replaced.
pub(crate) fn with_mode<T>(mode: Mode, body: impl FnOnce() -> T) -> T {
    struct Restore(Option<Mode>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            SCOPE.with(|cell| *cell.borrow_mut() = previous);
        }
    }
    let _restore = Restore(SCOPE.with(|cell| cell.replace(Some(mode))));
    body()
}

/// The window's answer for `context`: the dispatch's own when one is running,
/// otherwise the repository's schedule read against the clock now.
///
/// A schedule that does not parse and a clock that will not say are closed.
pub(crate) fn state(context: &Context) -> Mode {
    if let Some(mode) = scoped() {
        return mode;
    }
    let fallback = context
        .get("Git operation window")
        .and_then(GitOperationWindow::parse)
        .unwrap_or(GitOperationWindow::Disabled);
    let window = schedule::effective_in(&context.skill_dir, &context.repo_dir, &fallback);
    if schedule::permits_now(&window, schedule::now_minutes()) {
        Mode::Open
    } else {
        Mode::Closed(window)
    }
}

/// The refusal for a call that would reach GitHub while the window is closed,
/// when one is.
///
/// Only a dispatch's own scope arms it: a call outside any scope belongs to no
/// operation that decided the window.
pub(crate) fn tripwire(args: &[&str], how: How) -> Option<Failure> {
    if !matches!(scoped(), Some(Mode::Closed(_))) || !is_outbound_write(args, how) {
        return None;
    }
    Some(Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "outbox-bypass",
        "message": format!(
            "`{}` would write to GitHub while the git operation window is closed, and no queue \
             was asked to hold it",
            args.join(" ")
        ),
        "action": "wait for the git operation window to open, or set `Git operation window` to \
                   `disabled`",
    })))
}

const ISSUE_AND_PR_WRITES: &[&str] = &[
    "create",
    "edit",
    "comment",
    "close",
    "reopen",
    "ready",
    "merge",
    "delete",
    "lock",
    "unlock",
    "pin",
    "unpin",
    "transfer",
    "review",
    "update-branch",
    "revert",
];

const WRITING_REPO_VERBS: &[&str] = &[
    "create",
    "edit",
    "delete",
    "fork",
    "rename",
    "archive",
    "unarchive",
    "sync",
];

const READING_PROJECT_VERBS: &[&str] = &["list", "view", "field-list", "item-list"];
const READING_RELEASE_VERBS: &[&str] = &["list", "view", "download", "verify"];
const FIELD_FLAGS: &[&str] = &["-f", "--field", "--raw-field", "--input"];
const FLAGS_WITH_A_VALUE: &[&str] = &["-r", "--repo", "-h", "--hostname"];

/// Whether this call reaches GitHub in a way that changes something there.
///
/// Anything declared with [`How::write`] against `gh`, plus the spellings that
/// change GitHub without the caller having said so. `git push` is the one git
/// call that leaves the machine.
pub(crate) fn is_outbound_write(args: &[&str], how: How) -> bool {
    match args.first().copied() {
        Some("gh") => how.writes || gh_writes(&args[1..]),
        Some("git") => git_subcommand(&args[1..]) == Some("push"),
        _ => false,
    }
}

fn git_subcommand<'a>(args: &[&'a str]) -> Option<&'a str> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match *arg {
            "-c" | "-C" | "--git-dir" | "--work-tree" | "--namespace" => {
                rest.next();
            }
            flag if flag.starts_with('-') => {}
            subcommand => return Some(subcommand),
        }
    }
    None
}

/// Whether this `gh` call is an API call that changes something, read from
/// the words of the call itself. Flags are compared without regard to case, so
/// a line the gate lowercased is read as the one the shell runs.
pub(crate) fn gh_api_writes(args: &[&str]) -> bool {
    args.first().is_some_and(|word| *word == "gh")
        && gh_positionals(&args[1..]).first() == Some(&"api")
        && api_writes(args)
}

fn gh_positionals<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut positional = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if FLAGS_WITH_A_VALUE.contains(&arg.to_ascii_lowercase().as_str()) {
            rest.next();
        } else if !arg.starts_with('-') {
            positional.push(*arg);
        }
    }
    positional
}

fn gh_writes(args: &[&str]) -> bool {
    let positional = gh_positionals(args);
    let verb = positional.get(1).copied().unwrap_or_default();
    match positional.first().copied() {
        Some("api") => api_writes(args),
        Some("issue" | "pr") => {
            ISSUE_AND_PR_WRITES.contains(&verb)
                || (verb == "develop" && !args.iter().any(|arg| matches!(*arg, "-l" | "--list")))
        }
        Some("label") => matches!(verb, "create" | "edit" | "delete" | "clone"),
        Some("release") => !READING_RELEASE_VERBS.contains(&verb),
        Some("workflow") => matches!(verb, "run" | "enable" | "disable"),
        Some("project") => !READING_PROJECT_VERBS.contains(&verb),
        Some("repo") => match verb {
            "deploy-key" | "autolink" => matches!(
                positional.get(2).copied().unwrap_or_default(),
                "add" | "create" | "delete"
            ),
            _ => WRITING_REPO_VERBS.contains(&verb),
        },
        Some("cache") => verb == "delete",
        Some("ssh-key" | "gpg-key") => matches!(verb, "add" | "delete"),
        Some("run") => matches!(verb, "rerun" | "cancel" | "delete"),
        Some("secret" | "variable") => matches!(verb, "set" | "delete"),
        Some("gist") => matches!(verb, "create" | "edit" | "delete"),
        _ => false,
    }
}

fn api_writes(args: &[&str]) -> bool {
    let mut method = None;
    let mut fields = false;
    let mut endpoint = None;
    let mut unreadable = false;
    let mut rest = args.iter().skip_while(|arg| **arg != "api").skip(1);
    while let Some(arg) = rest.next() {
        let flag = arg.to_ascii_lowercase();
        if matches!(flag.as_str(), "-x" | "--method") {
            method = rest.next().map(|value| value.to_ascii_uppercase());
        } else if let Some(value) = flag
            .strip_prefix("--method=")
            .or_else(|| flag.strip_prefix("-x"))
        {
            method = Some(value.to_ascii_uppercase());
        } else if FIELD_FLAGS.contains(&flag.as_str()) {
            fields = true;
            let value = rest.next().copied().unwrap_or_default();
            unreadable |= flag == "--input" || reads_a_file(value);
        } else if attached_field(&flag) {
            fields = true;
            let value = if flag.starts_with("--") {
                arg.split_once('=').map_or("", |(_, value)| value)
            } else {
                &arg[2..]
            };
            unreadable |= flag.starts_with("--input=") || reads_a_file(value);
        } else if FLAGS_WITH_A_VALUE.contains(&flag.as_str())
            || matches!(
                flag.as_str(),
                "-q" | "--jq" | "-t" | "--template" | "--cache" | "-p" | "--preview"
            )
        {
            rest.next();
        } else if !arg.starts_with('-') && endpoint.is_none() {
            endpoint = Some(*arg);
        }
    }
    if endpoint == Some("graphql") {
        return unreadable || args.iter().any(|arg| arg.contains("mutation"));
    }
    match method {
        Some(method) => method != "GET",
        None => fields,
    }
}

fn reads_a_file(field: &str) -> bool {
    field
        .split_once('=')
        .is_some_and(|(_, value)| value.starts_with('@'))
}

fn attached_field(arg: &str) -> bool {
    (arg.len() > 2 && arg.starts_with("-f"))
        || ["--field=", "--raw-field=", "--input="]
            .iter()
            .any(|flag| arg.starts_with(flag))
}

#[cfg(test)]
mod tests;
