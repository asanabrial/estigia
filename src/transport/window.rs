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
/// otherwise the schedule of the repository the write is aimed at, read against
/// the clock now.
///
/// A schedule that does not parse, a clock that will not say, and an aimed-at
/// repository whose rows this machine cannot read are closed.
pub(crate) fn state(context: &Context) -> Mode {
    if let Some(mode) = scoped() {
        return mode;
    }
    let window = match aimed_at(context) {
        Aimed::Caller => schedule::effective_in(&context.repo_dir, &GitOperationWindow::Disabled),
        Aimed::Checkout(checkout) => {
            schedule::effective_in(&checkout, &GitOperationWindow::Disabled)
        }
        Aimed::Unreadable => GitOperationWindow::Unreadable,
    };
    if schedule::permits_now(&window, schedule::now_minutes()) {
        Mode::Open
    } else {
        Mode::Closed(window)
    }
}

/// Which checkout's rows decide the window for `context`.
enum Aimed {
    /// The caller's own checkout, when the write is not pinned to another
    /// repository or is pinned to the caller's own.
    Caller,
    /// A checkout of the repository the write is aimed at, found among the
    /// checkouts that answer for themselves.
    Checkout(std::path::PathBuf),
    /// The aimed-at repository has no checkout whose rows can be read.
    Unreadable,
}

/// Resolves the aimed-at repository to a local checkout, and only to one.
///
/// `context.repo` is the pin the dispatch built from the filing repository or
/// the tracker's named one; the caller's `repo_dir` stays the caller's. A pin
/// that names the caller's own checkout decides there. Otherwise the checkout
/// is one the machine recorded for that repository — `known_repositories` lists
/// only checkouts carrying their own `estigia.local.md`, which is the rows the
/// issue means. No checkout for the pin closes rather than inheriting the
/// caller's schedule.
fn aimed_at(context: &Context) -> Aimed {
    let Some(pinned) = context.repo.as_deref() else {
        return Aimed::Caller;
    };
    let Ok(pinned) = crate::config::RepoRef::parse(pinned) else {
        return Aimed::Unreadable;
    };
    if crate::skill::github_repository_of(&context.repo_dir).as_ref() == Some(&pinned) {
        return Aimed::Caller;
    }
    let Ok(home) = crate::paths::home_dir() else {
        return Aimed::Unreadable;
    };
    match crate::skill::known_repositories(&home)
        .into_iter()
        .find(|checkout| crate::skill::github_repository_of(checkout).as_ref() == Some(&pinned))
    {
        Some(checkout) => Aimed::Checkout(checkout),
        None => Aimed::Unreadable,
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
        "action": "wait for the git operation window to open: this write was not queued, so \
                   nothing reached GitHub",
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

#[derive(Default)]
struct ApiCall<'a> {
    method: Option<String>,
    fields: bool,
    endpoint: Option<&'a str>,
    unreadable: bool,
    query: Option<String>,
    in_query: bool,
}

impl<'a> ApiCall<'a> {
    fn read(args: &[&'a str]) -> Self {
        let mut call = Self::default();
        let mut rest = args.iter().skip_while(|arg| **arg != "api").skip(1);
        while let Some(arg) = rest.next() {
            let flag = arg.to_ascii_lowercase();
            if call.in_query {
                if looks_like_a_flag(arg) {
                    call.in_query = false;
                } else if call.endpoint.is_some() || *arg != "graphql" {
                    if let Some(query) = call.query.as_mut() {
                        query.push(' ');
                        query.push_str(arg);
                    }
                    continue;
                }
            }
            if matches!(flag.as_str(), "-x" | "--method") {
                call.method = rest.next().map(|value| value.to_ascii_uppercase());
            } else if let Some(value) = flag
                .strip_prefix("--method=")
                .or_else(|| flag.strip_prefix("-x"))
            {
                call.method = Some(value.to_ascii_uppercase());
            } else if FIELD_FLAGS.contains(&flag.as_str()) {
                call.fields = true;
                let value = rest.next().copied().unwrap_or_default();
                call.unreadable |= flag == "--input" || reads_a_file(value);
                if flag != "--input" {
                    call.take_field(value);
                }
            } else if attached_field(&flag) {
                call.fields = true;
                let value = if flag.starts_with("--") {
                    arg.split_once('=').map_or("", |(_, value)| value)
                } else {
                    &arg[2..]
                };
                call.unreadable |= flag.starts_with("--input=") || reads_a_file(value);
                if !flag.starts_with("--input=") {
                    call.take_field(value);
                }
            } else if FLAGS_WITH_A_VALUE.contains(&flag.as_str())
                || matches!(
                    flag.as_str(),
                    "-q" | "--jq" | "-t" | "--template" | "--cache" | "-p" | "--preview"
                )
            {
                rest.next();
            } else if !arg.starts_with('-') && call.endpoint.is_none() {
                call.endpoint = Some(*arg);
            }
        }
        call
    }

    fn take_field(&mut self, field: &str) {
        let Some((key, value)) = field.split_once('=') else {
            return;
        };
        if !key.trim_matches(['"', '\'']).eq_ignore_ascii_case("query") {
            return;
        }
        if !value.trim_start_matches(['"', '\'']).starts_with('@') {
            self.query = Some(value.to_owned());
            self.in_query = true;
        }
    }

    fn graphql(&self) -> bool {
        self.endpoint == Some("graphql")
    }

    fn writes(&self, args: &[&str]) -> bool {
        if self.graphql() {
            return self.unreadable || args.iter().any(|arg| arg.contains("mutation"));
        }
        match &self.method {
            Some(method) => method != "GET",
            None => self.fields,
        }
    }
}

fn looks_like_a_flag(word: &str) -> bool {
    let mut bytes = word.bytes();
    bytes.next() == Some(b'-')
        && bytes
            .next()
            .is_some_and(|b| b == b'-' || b.is_ascii_alphabetic())
}

fn api_writes(args: &[&str]) -> bool {
    ApiCall::read(args).writes(args)
}

/// What a `gh api` call that writes would change, when it is a review thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReviewThreadCall {
    /// A mutation on the line names a thread or review comment, or the REST endpoint is one.
    Named,
    /// A GraphQL call whose query is not on the line, so what it mutates is unknown.
    QueryNotOnTheLine,
}

const REVIEW_THREAD_MUTATIONS: &[&str] = &[
    "resolvereviewthread",
    "unresolvereviewthread",
    "addpullrequestreviewthreadreply",
    "addpullrequestreviewcomment",
];

/// Whether this `gh` call is an API write that addresses a pull request review thread
/// or review comment, read from the same words [`gh_api_writes`] reads.
pub(crate) fn gh_api_review_thread(args: &[&str]) -> Option<ReviewThreadCall> {
    if !gh_api_writes(args) {
        return None;
    }
    let call = ApiCall::read(args);
    if call.graphql() {
        return match &call.query {
            Some(query) if names_a_thread_mutation(query) => Some(ReviewThreadCall::Named),
            Some(_) => None,
            None => Some(ReviewThreadCall::QueryNotOnTheLine),
        };
    }
    call.endpoint
        .is_some_and(addresses_review_comments)
        .then_some(ReviewThreadCall::Named)
}

fn names_a_thread_mutation(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    REVIEW_THREAD_MUTATIONS.iter().any(|name| {
        query.match_indices(name).any(|(at, _)| {
            !query[..at].chars().next_back().is_some_and(word)
                && !query[at + name.len()..].chars().next().is_some_and(word)
        })
    })
}

fn addresses_review_comments(endpoint: &str) -> bool {
    let path = endpoint.split(['?', '#']).next().unwrap_or_default();
    let path = path.trim_matches(|c: char| c == '/' || c == '"' || c == '\'');
    let segments: Vec<String> = path.split('/').map(str::to_ascii_lowercase).collect();
    let tail = |from_the_end: usize| {
        segments
            .len()
            .checked_sub(from_the_end)
            .and_then(|at| segments.get(at))
            .map(String::as_str)
    };
    (tail(3) == Some("pulls") && tail(1) == Some("comments"))
        || (tail(5) == Some("pulls") && tail(3) == Some("comments") && tail(1) == Some("replies"))
        || (tail(3) == Some("pulls") && tail(2) == Some("comments"))
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
