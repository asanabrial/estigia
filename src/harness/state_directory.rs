//! The directory the binary's own records live in, which an agent's tools may
//! not write.
//!
//! The store, the job directories and the run pointers are all read back as
//! decisions: a claim, an admission, a checks receipt. Nothing else in this
//! crate distinguishes who wrote a file there, so a write tool naming one is
//! refused for every run, claimed or not, the way the person-only record is.
//! Reads stay free.

use std::path::{Path, PathBuf};

use crate::outcome::{NoCommandReason, Refusal, Resolution};

use super::{Action, Decision, GateContext};

/// The stable code of the refusal.
pub const CODE: &str = "state-directory-is-for-the-binary";

/// What a caller read off the call, for a door that decides it from a reading
/// rather than from the classified action alone.
#[derive(Debug, Default, Clone, Copy)]
pub struct Seen<'a> {
    /// The single command string, when the call was a shell call.
    pub literal: Option<&'a str>,
    /// The line the classifier searched, when the payload carried no single
    /// command string — an argv list is one command with its spaces taken out.
    pub text: Option<&'a str>,
    /// The paths a write tool named in a patch body.
    pub write_targets: &'a [String],
}

/// Refuses a write whose spelling or landing is inside the state directory.
///
/// Asked before any stand-down, claim or operation-window decision, so none of
/// them can lift it: what the rows in that directory say is read back as
/// authority, and an agent's write is not the code that owns them.
pub fn deny(context: &GateContext, action: &Action, seen: Seen<'_>) -> Option<Decision> {
    let Action::Write { target } = action else {
        return None;
    };
    let state = directory_of(context);
    let named = lands_inside(target, &state)
        || seen
            .write_targets
            .iter()
            .any(|target| lands_inside(target, &state));
    let said = [seen.literal, seen.text]
        .into_iter()
        .flatten()
        .any(|text| names_state_directory(text, &state));
    (named || said).then(|| Decision::Deny(Box::new(refusal(&state))))
}

/// The state directory a context decides in.
///
/// `session::state_root` composes `<state>/runs`, so that shape is read one
/// level up; a context naming any other directory is taken to name the
/// directory itself.
pub fn directory_of(context: &GateContext) -> PathBuf {
    match context
        .state_root
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some("runs") => context
            .state_root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| context.state_root.clone()),
        _ => context.state_root.clone(),
    }
}

/// Whether `target`, taken as a path, lands inside `state_directory`.
///
/// The landing is resolved rather than the spelling matched, so a link inside
/// a checkout that points at the state directory is refused as the directory it
/// reaches.
pub fn lands_inside(target: &str, state_directory: &Path) -> bool {
    let Some(placed) = expand_home(target) else {
        return false;
    };
    crate::paths::covers(state_directory, &placed)
}

/// Whether a shell line or a patch body names the state directory anywhere in
/// it: the home-relative `.estigia` segment, the resolved directory, or a token
/// that resolves onto it through a link.
pub fn names_state_directory(text: &str, state_directory: &Path) -> bool {
    let state = super::normalise(&state_directory.to_string_lossy());
    let bare = state.trim_matches('/');
    let dequoted = dequoted(text);
    let spelling = |path: &str| {
        path.contains("/.estigia/") || (!bare.is_empty() && path.contains(&format!("/{bare}/")))
    };
    let resolves = |view: &str| tokens(view).any(|word| lands_inside(&word, state_directory));
    [text, dequoted.as_str()]
        .iter()
        .any(|view| super::text_names_path(view, &spelling) || resolves(view))
}

fn dequoted(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '"' | '\'' | '\\'))
        .collect()
}

/// The words of a shell line or patch body a path can stand in, split where the
/// shell separates one word from the next.
fn tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| {
        c.is_whitespace() || matches!(c, '&' | ';' | '|' | '(' | ')' | '<' | '>' | '"' | '\'')
    })
    .filter(|word| !word.is_empty())
    .map(str::to_owned)
}

/// A target with `~`, `$HOME` or `${HOME}` resolved, or `None` when it starts
/// with one of them and there is no home to resolve it against.
fn expand_home(target: &str) -> Option<PathBuf> {
    let home = || crate::paths::home_dir().ok();
    for (prefix, keeps) in [
        ("~/", true),
        ("$HOME/", true),
        ("${HOME}/", true),
        ("~", false),
        ("$HOME", false),
        ("${HOME}", false),
    ] {
        if let Some(rest) = target.strip_prefix(prefix)
            && (keeps || rest.is_empty())
        {
            return Some(home()?.join(rest));
        }
    }
    Some(PathBuf::from(target))
}

/// The refusal a write into `state` is answered with.
///
/// Public so the ratchet in `src/cli/tests.rs` puts the code through its real
/// constructor rather than a typed copy of the literal.
pub(crate) fn refusal(state: &Path) -> Refusal {
    Refusal::not_started(
        CODE,
        format!(
            "`{}` holds the store, the job directories and the run records every decision in this \
             checkout is read back from, and only the binary writes them: an agent's write there \
             would be read later as a claim, an admission or a receipt the code that owns them \
             never made",
            state.display()
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the `estigia` command that records this, run by the binary itself or by the person; \
             an agent's tools never write into the state directory",
        ),
    )
}

#[cfg(test)]
mod tests;
