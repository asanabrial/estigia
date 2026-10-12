//! One reviewer run from a prompt input to a parsed answer.

use super::prompt::{self, PromptInput};
use super::result::{self, RoleResult};
use super::{Adapter, Invocation, ReviewerCode};
use crate::outcome::Refusal;

/// Builds the prompt for `input`, runs it through `adapter` and reads what came
/// back as the role's answer.
///
/// Every refusal of every step passes through under its own code: a payload that
/// is not the one the receipt published refuses before the adapter is called,
/// and an answer outside the role's contract is `reviewer-result-malformed`,
/// never a partial result. A parsed answer is not recorded here; [`launch_recorded`]
/// hands it to a recorder.
pub fn launch(
    adapter: &dyn Adapter,
    input: &PromptInput<'_>,
    model: Option<String>,
) -> Result<RoleResult, Refusal> {
    let prompt = prompt::build(input)?;
    let invocation = Invocation {
        role: input.role.name().to_owned(),
        receipt: input.receipt.clone(),
        prompt,
        model,
    };
    let raw = adapter.review(&invocation)?;
    result::parse(input.role, &raw)
}

/// What a recorder says about an answer it was handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recording {
    /// The answer was recorded.
    Recorded,
    /// The recorder checked the answer itself and found it unusable.
    AnswerRejected(Refusal),
    /// The recorder refused for a reason that judges nothing about the answer.
    Refused(Refusal),
}

/// [`launch`], then hands the parsed answer to `record`.
///
/// Only a recorder that says [`Recording::AnswerRejected`], with an unstarted
/// write, turns the answer into `reviewer-result-malformed`, whose message keeps
/// the recorder's code and message. That is the recorder's statement and is
/// never read off a refusal's code or class. Every other refusal, including an
/// `AnswerRejected` whose write may have landed, is returned as it came: its
/// code, class, outcome and resolution are the recorder's.
pub fn launch_recorded(
    adapter: &dyn Adapter,
    input: &PromptInput<'_>,
    model: Option<String>,
    record: &mut dyn FnMut(&RoleResult) -> Recording,
) -> Result<RoleResult, Refusal> {
    let answer = launch(adapter, input, model)?;
    let recording = record(&answer);
    settle(answer, recording)
}

/// What a parsed answer comes to once its recorder has spoken: the answer when it was recorded,
/// otherwise the refusal [`launch_recorded`] describes.
pub fn settle(answer: RoleResult, recording: Recording) -> Result<RoleResult, Refusal> {
    match recording {
        Recording::Recorded => Ok(answer),
        Recording::AnswerRejected(refusal) if refusal.outcome.is_clean() => {
            Err(ReviewerCode::ResultMalformed.refusal(format!(
                "the reviewer's answer is unusable: the recorder rejected it as {}: {}",
                refusal.code, refusal.message
            )))
        }
        Recording::AnswerRejected(refusal) | Recording::Refused(refusal) => Err(refusal),
    }
}

#[cfg(test)]
mod tests;
