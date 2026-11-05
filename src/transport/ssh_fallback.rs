//! A `git` network call SSH never connected for, repeated once over HTTPS.
//!
//! `docs/github-binding.md` says why, and `docs/honesty.md` what it does not reach.

use super::{Failure, Output};

const NETWORK_VERBS: [&str; 4] = ["fetch", "ls-remote", "pull", "push"];

const OVER_HTTPS: [&str; 8] = [
    "-c",
    "url.https://github.com/.insteadOf=git@github.com:",
    "-c",
    "url.https://github.com/.insteadOf=ssh://git@github.com/",
    "-c",
    "credential.https://github.com.helper=",
    "-c",
    "credential.https://github.com.helper=!gh auth git-credential",
];

/// Git prints this only when the initial contact with the remote failed, so nothing was
/// sent, which is what makes repeating a write safe.
const INITIAL_CONTACT_FAILED: &str = "Could not read from remote repository";

const NEVER_OPENED: [&str; 6] = [
    "Permission denied (publickey",
    "kex_exchange_identification",
    "Connection refused",
    "Connection timed out",
    "Operation timed out",
    "Could not resolve hostname",
];

/// The environment the repeated attempt adds: it has no terminal to ask on.
pub(super) const NO_PROMPT: (&str, &str) = ("GIT_TERMINAL_PROMPT", "0");

/// Git translates [`INITIAL_CONTACT_FAILED`], so a network call has to run untranslated
/// for its failure to be read.
pub(super) const UNTRANSLATED: (&str, &str) = ("LC_ALL", "C");

pub(super) fn is_network_call(args: &[&str]) -> bool {
    matches!(args, ["git", verb, ..] if NETWORK_VERBS.contains(verb))
}

fn never_connected(stderr: &str) -> bool {
    stderr.contains(INITIAL_CONTACT_FAILED)
        && NEVER_OPENED.iter().any(|message| stderr.contains(message))
}

/// The arguments to repeat `args` with, when `first` shows SSH never connected.
pub(super) fn repeated<'a>(
    args: &[&'a str],
    first: &Result<Output, Failure>,
) -> Option<Vec<&'a str>> {
    let ["git", verb, rest @ ..] = args else {
        return None;
    };
    if !NETWORK_VERBS.contains(verb) {
        return None;
    }
    let stderr = match first {
        Ok(output) if output.status != 0 => output.stderr.as_str(),
        Err(Failure::Read(detail) | Failure::Write(detail)) => detail.as_str(),
        _ => return None,
    };
    if !never_connected(stderr) {
        return None;
    }
    let mut repeated = vec!["git"];
    repeated.extend(OVER_HTTPS);
    repeated.push(verb);
    repeated.extend_from_slice(rest);
    Some(repeated)
}

/// The answer once both attempts have run. A success, or a second attempt whose fate is
/// unknown, is what the caller has to hear; otherwise the first failure stands, carrying
/// the second.
pub(super) fn settled(
    first: Result<Output, Failure>,
    second: Result<Output, Failure>,
) -> Result<Output, Failure> {
    let also = |detail: String, more: &str| format!("{detail}; over HTTPS too: {more}");
    match (first, second) {
        (_, Ok(output)) if output.status == 0 => Ok(output),
        (_, Err(unknown @ Failure::Timeout(_))) => Err(unknown),
        (Err(Failure::Read(detail)), Err(second)) => {
            Err(Failure::Read(also(detail, &second.detail())))
        }
        (Err(Failure::Write(detail)), Err(second)) => {
            Err(Failure::Write(also(detail, &second.detail())))
        }
        (first, _) => first,
    }
}

#[cfg(all(test, unix))]
mod tests;
