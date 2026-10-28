use std::path::Path;

use serde_json::{Value, json};

use super::session;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::{markers, store};

pub(crate) const MAX_BYTES: usize = 200;
pub(crate) const PHASES: &[&str] = &[
    "investigating",
    "implementing",
    "verifying",
    "waiting-on-person",
    "idle",
];
pub(crate) const TRUNCATED: &str = " [truncated]";
pub(crate) const REDACTED: &str = "[redacted]";

const SECRET_PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "glpat-",
    "sk-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "AKIA",
    "AIza",
];

fn token_shaped(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));
    if SECRET_PREFIXES
        .iter()
        .any(|prefix| word.starts_with(prefix) && word.len() >= 20)
    {
        return true;
    }
    word.len() >= 32
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        && word.chars().any(|c| c.is_ascii_digit())
        && word.chars().any(|c| c.is_ascii_alphabetic())
}

pub(crate) fn one_line(text: &str) -> String {
    let escaped = markers::escape_control_input(text).replace("<!--", "&lt;!--");
    let words: Vec<&str> = escaped
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .map(|word| if token_shaped(word) { REDACTED } else { word })
        .collect();
    let line = words.join(" ");
    if line.len() <= MAX_BYTES {
        return line;
    }
    let mut end = MAX_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{TRUNCATED}", line[..end].trim_end())
}

pub(crate) fn empty() -> Refusal {
    Refusal::not_started(
        "status-empty",
        "the status has no text, so nothing was recorded",
        Resolution::run("estigia status set <line>"),
    )
}

pub(crate) fn phase_unknown(phase: &str) -> Refusal {
    Refusal::not_started(
        "status-phase-unknown",
        format!(
            "`{phase}` is not a phase; the phases are {}, and nothing was recorded",
            PHASES.join(", ")
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "one of the listed phases, or none",
        ),
    )
}

pub(crate) fn unavailable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "status-store-unavailable",
        format!("the status could not be written: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "set it again; nothing was recorded",
        ),
    )
}

pub(crate) fn set(
    state_root: &Path,
    run_id: &str,
    text: &str,
    phase: Option<&str>,
) -> Result<Value, Refusal> {
    if let Some(phase) = phase.filter(|phase| !PHASES.contains(phase)) {
        return Err(phase_unknown(phase));
    }
    let line = one_line(text);
    if line.is_empty() {
        return Err(empty());
    }
    let now = session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))?;
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| unavailable(&failure.detail()))?;
    let replaced = opened
        .status_set(run_id, &line, phase, now)
        .map_err(|failure| unavailable(&failure.detail()))?;
    Ok(json!({
        "run_id": run_id,
        "status": line,
        "phase": phase,
        "set_at": now,
        "replaced": replaced,
        "self_reported": true,
    }))
}

#[cfg(test)]
mod tests;
