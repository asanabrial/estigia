//! The reviewer adapter a host drives when no reviewer CLI is installed.
//!
//! A relay cannot run a reviewer in this process: the host runs it, OpenCode through
//! its own sub-agent tool and Pi in process through its model registry, and the
//! review relay hands the binary the exact prompt and the reviewer's raw answer. [`Relay::review`] therefore refuses, because a
//! caller that reaches it asked this process to run a reviewer it cannot run —
//! and that refusal is the coded `unavailable` naming the CLI to install.

use super::{Adapter, Invocation, ReviewerCode};
use crate::outcome::Refusal;

const OPEN: &str = "<estigia-review-result";
const CLOSE: &str = "</estigia-review-result>";

pub(crate) fn framed_prompt(mut prompt: Vec<u8>, job: &str) -> Vec<u8> {
    prompt.extend_from_slice(
        format!(
            "\n\n## OpenCode relay output channel\n\n\
             The JSON-only requirement above applies to the result inside this channel, not to \
             repository-required acknowledgments. Emit any acknowledgment required by your \
             inherited instructions first, outside the channel. Then emit exactly this one \
             job-bound block, with the role's complete JSON answer replacing RESULT_JSON:\n\n\
             <estigia-review-result job=\"{job}\">\nRESULT_JSON\n{CLOSE}\n\n\
             Put both delimiters on their own lines. Emit no other channel block and nothing \
             after the closing delimiter. Unframed JSON is not admitted. Do not place review \
             findings in the acknowledgment: only the JSON inside the channel is recorded.\n"
        )
        .as_bytes(),
    );
    prompt
}

pub(crate) fn framed_result(raw: &[u8]) -> Result<(&str, &[u8]), Refusal> {
    let malformed = || {
        ReviewerCode::ResultMalformed.refusal(
            "expected exactly one job-bound OpenCode result channel after any acknowledgment",
        )
    };
    if raw.len() > super::run::MAX_OUTPUT {
        return Err(
            ReviewerCode::OutputTooLarge.refusal("the relay answer exceeds its output limit")
        );
    }
    let text = std::str::from_utf8(raw).map_err(|_| malformed())?;
    let channel_lines = |marker: &str| {
        text.match_indices(marker)
            .filter(|(at, _)| *at == 0 || text.as_bytes()[at - 1] == b'\n')
            .map(|(at, _)| at)
            .collect::<Vec<_>>()
    };
    let starts = channel_lines(OPEN);
    let ends = channel_lines(CLOSE);
    if starts.len() != 1 || ends.len() != 1 || starts[0] >= ends[0] {
        return Err(malformed());
    }
    let channel = &text[starts[0] + OPEN.len()..];
    let (header, rest) = channel.split_once('\n').ok_or_else(malformed)?;
    let job = header
        .trim_end_matches('\r')
        .strip_prefix(" job=\"")
        .and_then(|header| header.strip_suffix("\">"))
        .filter(|job| {
            job.len() == 32
                && job
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
        .ok_or_else(malformed)?;
    let body_at = text.len() - rest.len();
    let answer = text.get(body_at..ends[0]).ok_or_else(malformed)?;
    let suffix = &text[ends[0] + CLOSE.len()..];
    if !answer.ends_with('\n') || !suffix.trim().is_empty() {
        return Err(malformed());
    }
    Ok((job, answer.as_bytes()))
}

/// The relay: no reviewer CLI is installed, so the host runs the reviewer and
/// the binary builds the prompt and admits the answer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Relay;

impl Relay {
    /// The relay adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Adapter for Relay {
    fn review(&self, _invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        Err(ReviewerCode::Unavailable.refusal(
            "no `claude` or `codex` CLI is installed, so this process cannot run the reviewer \
             itself; the host relays it through the OpenCode review plugin or the Pi review \
             extension, or install the `claude` or the `codex` CLI so it resolves on PATH",
        ))
    }

    fn relays(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_tokens_inside_json_strings_are_not_delimiters() {
        let job = "a".repeat(32);
        let json =
            r#"{"findings": [], "quoted": "<estigia-review-result and </estigia-review-result>"}"#;
        let raw = format!("Acknowledged.\r\n{OPEN} job=\"{job}\">\r\n{json}\r\n{CLOSE}\r\n");
        let (id, answer) = framed_result(raw.as_bytes()).expect("one channel");
        assert_eq!(id, job);
        assert_eq!(std::str::from_utf8(answer).expect("UTF-8").trim(), json);
    }

    #[test]
    fn a_channel_rejects_non_utf8_and_bounds_acknowledgments_too() {
        assert_eq!(
            framed_result(&[0xff]).expect_err("invalid UTF-8").code,
            "reviewer-result-malformed"
        );
        let huge = vec![b'a'; super::super::run::MAX_OUTPUT + 1];
        assert_eq!(
            framed_result(&huge)
                .expect_err("oversized acknowledgment")
                .code,
            "reviewer-output-too-large"
        );
    }
}
