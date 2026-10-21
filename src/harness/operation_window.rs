//! The repository's daily range for commits, pushes, and branch creation.
//!
//! One typed row, disabled by default, read in this machine's local civil
//! time. Enabled, it denies the protected operations outside the permitted
//! period and never grants anything inside it: every claim, state, review,
//! and irreversible-boundary check still runs. An enabled value that does not
//! parse, a clock that will not say, and a repository whose rows this machine
//! cannot read deny the protected operations rather than falling back to
//! unrestricted — and nothing else.

use std::cell::RefCell;
use std::path::Path;

use crate::config::{GitOperationWindow, Setting};
use crate::harness::{Action, GateContext};
use crate::outcome::{NoCommandReason, Refusal, Resolution};

/// The clock the schedule is read against: minutes since Monday midnight, or
/// nothing when the clock will not say.
pub type Clock = fn() -> Option<u16>;

std::thread_local! {
    static NOW_OVERRIDE: RefCell<Option<Clock>> = const { RefCell::new(None) };
}

/// The minutes since Monday midnight now, in this machine's local civil time.
pub fn now_minutes() -> Option<u16> {
    if let Some(clock) = NOW_OVERRIDE.with(|cell| *cell.borrow()) {
        return clock();
    }
    local_minutes_now()
}

/// Runs `run` with the schedule reading `clock` instead of the machine.
///
/// Cleared afterwards even when `run` panics, so a fixed test time never leaks
/// into the next test sharing the thread.
pub fn with_now_override<T>(clock: Option<Clock>, run: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            NOW_OVERRIDE.with(|cell| *cell.borrow_mut() = None);
        }
    }
    NOW_OVERRIDE.with(|cell| *cell.borrow_mut() = clock);
    let _reset = Reset;
    run()
}

/// This machine's local civil time as minutes since Monday midnight.
pub fn local_minutes_now() -> Option<u16> {
    system_local_minutes()
}

#[cfg(unix)]
fn system_local_minutes() -> Option<u16> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as libc::time_t;
    let mut broken: libc::tm = unsafe { std::mem::zeroed() };
    let broken = unsafe { libc::localtime_r(&seconds, &mut broken) };
    if broken.is_null() {
        return None;
    }
    let broken = unsafe { *broken };
    (broken.tm_hour >= 0
        && broken.tm_hour < 24
        && broken.tm_min >= 0
        && broken.tm_min < 60
        && broken.tm_wday >= 0
        && broken.tm_wday < 7)
        .then_some(
            ((broken.tm_wday as u16 + 6) % 7) * 1440
                + broken.tm_hour as u16 * 60
                + broken.tm_min as u16,
        )
}

#[cfg(windows)]
fn system_local_minutes() -> Option<u16> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut stamp: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut stamp) };
    (stamp.wHour < 24 && stamp.wMinute < 60 && stamp.wDayOfWeek < 7)
        .then_some(((stamp.wDayOfWeek + 6) % 7) * 1440 + stamp.wHour * 60 + stamp.wMinute)
}

#[cfg(not(any(unix, windows)))]
fn system_local_minutes() -> Option<u16> {
    None
}

/// Whether this action is one the schedule may deny.
///
/// Pushes, commits, every spelling of local branch creation the classifier
/// recognises, and every call that would write to GitHub: merges, tags,
/// releases, pull requests, issue creation, comments and edits. Worktree
/// operations that resume an existing branch, deletions, listings, and ordinary
/// file writes are not among them.
pub fn action_is_protected(action: &Action) -> bool {
    match action {
        Action::Boundary { command, .. } => matches!(
            command.as_str(),
            "git push"
                | "git merge"
                | "git tag"
                | "gh pr merge"
                | "gh pr create"
                | "gh release create"
                | "gh release edit"
                | "gh api"
        ),
        Action::Write { target } => matches!(
            target.as_str(),
            "git commit"
                | "git branch"
                | "git checkout -b"
                | "git switch -c"
                | "git worktree add -b"
                | "gh issue create"
                | "gh issue develop"
        ),
        Action::Remote { .. } => true,
        Action::Untouched => false,
    }
}

/// The GitHub write this command line carries, if any, judged by the rule the
/// transport's own tripwire applies to the calls Estigia spawns.
pub fn remote_write_target(code: &str) -> Option<String> {
    for command in shell_commands(code) {
        let words: Vec<&str> = command
            .split_whitespace()
            .map(|word| word.trim_matches(|c| c == '"' || c == '\''))
            .collect();
        let Some(at) = words.iter().position(|word| *word == "gh") else {
            continue;
        };
        let call = &words[at..];
        if crate::transport::window::is_outbound_write(call, crate::transport::How::read()) {
            let named: Vec<&str> = call
                .iter()
                .copied()
                .filter(|word| !word.starts_with('-'))
                .take(3)
                .collect();
            return Some(named.join(" "));
        }
    }
    None
}

/// The period a protected operation is permitted in, as the denial words it.
pub fn permitted_period(window: &GitOperationWindow) -> Option<String> {
    let GitOperationWindow::Enabled {
        start, end, invert, ..
    } = window
    else {
        return None;
    };
    let days = window
        .days_value()
        .map_or(String::new(), |days| format!("{days} "));
    let side = if *invert { "outside" } else { "inside" };
    let suffix = if *invert { " inverted" } else { "" };
    Some(format!(
        "{side} {days}{:02}:{:02}-{:02}:{:02} local{suffix}",
        start / 60,
        start % 60,
        end / 60,
        end % 60
    ))
}

/// The denial for a protected operation outside the permitted period.
pub fn denial(window: &GitOperationWindow, now: Option<u16>) -> Refusal {
    let (code_detail, remedy) = match (window, now) {
        (GitOperationWindow::Invalid, _) => (
            "the repository's git operation window is enabled and does not parse, so no minute \
             can be shown inside it"
                .to_owned(),
            "a usable value, or `disabled`",
        ),
        (GitOperationWindow::Unreadable, _) => (
            "the repository this write is aimed at has no checkout on this machine whose git \
             operation window can be read, so no minute can be shown inside it"
                .to_owned(),
            "a checkout of that repository carrying its own rows, or `disabled` written in them",
        ),
        (_, None) => (
            "this machine could not say what time it is, so no minute can be shown inside the \
             repository's git operation window"
                .to_owned(),
            "a working system clock",
        ),
        (window @ GitOperationWindow::Enabled { .. }, Some(_)) => (
            format!(
                "outside the repository's git operation window: commits, pushes, branch \
                 creation, and writes to GitHub may run only {}",
                permitted_period(window).unwrap_or_default()
            ),
            "that range to arrive, or `disabled`",
        ),
        (GitOperationWindow::Disabled, _) => (
            "the repository's git operation window is disabled".to_owned(),
            "nothing: this denial should not have been raised",
        ),
    };
    Refusal::not_started(
        "git-operation-window-denied",
        code_detail,
        Resolution::no_command(NoCommandReason::OperatorKnowledge, remedy),
    )
}

/// The schedule's answer for one gated action, if it has one.
///
/// `Some` denies and `None` leaves the existing checks to decide: disabled
/// preserves today's behaviour, and inside the permitted period the claim,
/// state, review, and irreversible-boundary checks still run.
pub fn check_protected(context: &GateContext, action: &Action) -> Option<Refusal> {
    if !action_is_protected(action) {
        return None;
    }
    if permits_now(&context.operation_window, now_minutes()) {
        return None;
    }
    Some(denial(&context.operation_window, now_minutes()))
}

/// Whether the schedule permits a protected operation at this minute since
/// Monday midnight.
pub fn permits_now(window: &GitOperationWindow, now: Option<u16>) -> bool {
    match (window, now) {
        (GitOperationWindow::Disabled, _) => true,
        (_, None) => false,
        (GitOperationWindow::Invalid, _) => false,
        (GitOperationWindow::Unreadable, _) => false,
        (window, Some(minutes)) => window.permits(minutes),
    }
}

/// The effective schedule for this checkout: the repository's row over the
/// contract, or `fallback` when no document says.
///
/// A row that is present and does not parse is [`GitOperationWindow::Invalid`],
/// never the fallback: an unknown schedule denies rather than disabling
/// itself. Absent rows keep the fallback, so a repository that never answered
/// reads exactly as it did before the row existed.
pub fn effective_in(
    skill_root: &Path,
    repo_dir: &Path,
    fallback: &GitOperationWindow,
) -> GitOperationWindow {
    let mut raw: Option<String> = None;
    let contract = skill_root.join(crate::skill::CONTRACT);
    let documents = [
        std::fs::read_to_string(contract).ok(),
        crate::skill::local_overrides(skill_root)
            .ok()
            .flatten()
            .map(|(_, text)| text),
        crate::skill::repository_rows(repo_dir)
            .ok()
            .flatten()
            .map(|(_, text)| text),
    ];
    for text in documents.into_iter().flatten() {
        for (label, value) in crate::config::table_rows(&text) {
            if Setting::from_label(&label) == Some(Setting::GitOperationWindow) {
                raw = Some(value);
            }
        }
    }
    match raw {
        None => *fallback,
        Some(value) => GitOperationWindow::parse(&value).unwrap_or(GitOperationWindow::Invalid),
    }
}

/// The protected write this lowercased command line carries, if any.
///
/// A compound line holds one command per fragment, and the gate decides on
/// the one reported: when any of them is one the schedule may deny, that is
/// the one named, so a commit riding beside an unprotected worktree still
/// reads as the commit.
pub fn protected_write_target(normalized: &str) -> Option<&'static str> {
    if normalized.contains("git commit") {
        return Some("git commit");
    }
    if normalized.contains("git checkout -b") || normalized.contains("git checkout --orphan") {
        return Some("git checkout -b");
    }
    if normalized.contains("git switch -c")
        || normalized.contains("git switch --create")
        || normalized.contains("git switch --force-create")
        || normalized.contains("git switch --orphan")
    {
        return Some("git switch -c");
    }
    if worktree_add_creates_branch(normalized) {
        return Some("git worktree add -b");
    }
    if is_branch_creation(normalized) {
        return Some("git branch");
    }
    None
}

/// Words that end the command being read: newlines separate commands, and
/// `;`, `&&`, `||`, `|`, and `&` separate the ones sharing one line.
const SEPARATORS: &[&str] = &[";", "&&", "||", "|", "&"];

/// Whether this lowercased command line creates a local branch through `git
/// branch`: a positional name with none of the flags that make the same
/// command list, show, delete, move, or set an upstream on instead.
pub fn is_branch_creation(normalized: &str) -> bool {
    const NON_CREATING: &[&str] = &[
        "-d",
        "-D",
        "--delete",
        "-m",
        "-M",
        "--move",
        "-u",
        "--set-upstream-to",
        "--no-set-upstream-to",
        "--list",
        "--show-current",
        "-a",
        "-r",
        "--all",
        "--remotes",
        "-v",
        "--verbose",
        "--contains",
        "--no-contains",
        "--merged",
        "--no-merged",
        "--sort",
        "--format",
        "--points-at",
    ];
    for command in split_commands(normalized) {
        let words: Vec<&str> = command.split_whitespace().collect();
        let mut at = 0;
        while at + 1 < words.len() {
            if words[at] != "git" || words[at + 1] != "branch" {
                at += 1;
                continue;
            }
            let mut creating = false;
            for word in words.iter().skip(at + 2) {
                if SEPARATORS.contains(word) {
                    break;
                }
                if NON_CREATING.contains(word) {
                    creating = false;
                    break;
                }
                if !word.starts_with('-') {
                    creating = true;
                }
            }
            if creating {
                return true;
            }
            at += 2;
        }
    }
    false
}

/// Whether this lowercased `git worktree add` line carries a spelling that
/// creates a branch beside the worktree: `-b`/`-B`, or `--orphan`.
pub fn worktree_add_creates_branch(normalized: &str) -> bool {
    for command in split_commands(normalized) {
        let words: Vec<&str> = command.split_whitespace().collect();
        let mut at = 0;
        while at + 2 < words.len() {
            if words[at] != "git" || words[at + 1] != "worktree" || words[at + 2] != "add" {
                at += 1;
                continue;
            }
            for word in words.iter().skip(at + 3) {
                if SEPARATORS.contains(word) {
                    break;
                }
                let word = word.split_once('=').map_or(*word, |(flag, _)| flag);
                if word == "-b" || word == "--orphan" {
                    return true;
                }
            }
            at += 3;
        }
    }
    false
}

/// [`split_commands`] after every control operator outside quotes is set apart, so `a;b` and
/// `(a)` are read as the commands they are. A `$(…)` or backtick substitution is a boundary
/// wherever it stands, double quotes included, because its contents run. When the quotes or
/// substitutions do not balance the line is split on every operator, quoted or not.
pub(super) fn shell_commands(text: &str) -> Vec<String> {
    let spaced = space_operators(text, true)
        .or_else(|| space_operators(text, false))
        .unwrap_or_default();
    split_commands(&spaced)
}

#[derive(Clone, Copy, PartialEq)]
enum Frame {
    Double,
    Parens,
    Backticks,
}

fn space_operators(text: &str, respect_quotes: bool) -> Option<String> {
    let mut spaced = String::with_capacity(text.len());
    let mut frames: Vec<Frame> = Vec::new();
    let mut chars = text.chars().peekable();
    let mut escaped = false;
    while let Some(c) = chars.next() {
        let quoted_double = frames.last() == Some(&Frame::Double);
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if respect_quotes && c == '\'' && !quoted_double {
            spaced.push(c);
            loop {
                match chars.next() {
                    Some(inner) => {
                        spaced.push(inner);
                        if inner == '\'' {
                            break;
                        }
                    }
                    None => return (!respect_quotes).then_some(spaced),
                }
            }
            continue;
        } else if respect_quotes && c == '"' {
            if quoted_double {
                frames.pop();
            } else {
                frames.push(Frame::Double);
            }
        } else if c == '$' && chars.peek() == Some(&'(') {
            chars.next();
            frames.push(Frame::Parens);
            spaced.push_str(" ; ");
            continue;
        } else if c == '`' {
            if frames.last() == Some(&Frame::Backticks) {
                frames.pop();
            } else {
                frames.push(Frame::Backticks);
            }
            spaced.push_str(" ; ");
            continue;
        } else if c == ')' && frames.last() == Some(&Frame::Parens) {
            frames.pop();
            spaced.push_str(" ; ");
            continue;
        } else if (!quoted_double || !respect_quotes) && matches!(c, ';' | '&' | '|' | '(' | ')') {
            spaced.push_str(" ; ");
            continue;
        }
        spaced.push(c);
    }
    (frames.is_empty() || !respect_quotes).then_some(spaced)
}

/// One command per line: newlines separate commands, and `;`, `&&`, `||`,
/// `|`, and `&` separate the ones sharing one.
fn split_commands(normalized: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut current = String::new();
    for line in normalized.split('\n') {
        for word in line.split_whitespace() {
            if matches!(word, ";" | "&&" | "||" | "|" | "&") {
                if !current.is_empty() {
                    commands.push(std::mem::take(&mut current));
                }
                continue;
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            commands.push(std::mem::take(&mut current));
        }
    }
    commands
}

#[cfg(test)]
mod tests;
