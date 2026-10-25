//! `Decision mode: auto`: a question tool is refused, the recommended answer
//! is recorded for the run, and the `Stop` hook reports what was decided.

use std::path::Path;

use serde_json::Value;

use super::{Decision, GateContext, session};
use crate::config::DecisionMode;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, TakenDecision};

/// The tools that put a question to the person and wait for the answer.
pub const QUESTION_TOOLS: &[&str] = &["AskUserQuestion"];

const RECOMMENDED: &str = "(recommended)";

/// The answer each question in an `AskUserQuestion` payload recommends: the
/// options whose label says `(Recommended)`, or the first option when none does.
pub(crate) fn recommended(tool_input: &Value) -> Vec<TakenDecision> {
    let Some(questions) = tool_input.get("questions").and_then(Value::as_array) else {
        return Vec::new();
    };
    questions
        .iter()
        .filter_map(|question| {
            let asked = text(question, "question").or_else(|| text(question, "header"))?;
            let options = question
                .get("options")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let marked: Vec<&Value> = options
                .iter()
                .filter(|option| {
                    text(option, "label")
                        .is_some_and(|label| label.to_lowercase().contains(RECOMMENDED))
                })
                .collect();
            let chosen = if marked.is_empty() {
                options.first().into_iter().collect()
            } else {
                marked
            };
            let decision = chosen
                .iter()
                .filter_map(|option| answer(option))
                .collect::<Vec<_>>()
                .join("; ");
            Some(TakenDecision {
                question: asked,
                decision: if decision.is_empty() {
                    "no option was offered; the run decided on its own judgement".to_owned()
                } else {
                    decision
                },
            })
        })
        .collect()
}

fn text(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn answer(option: &Value) -> Option<String> {
    let label = text(option, "label")?;
    let at = label.to_lowercase().find(RECOMMENDED);
    let label = at
        .map_or(label.as_str(), |at| label[..at].trim_end())
        .to_owned();
    Some(match text(option, "description") {
        Some(description) => format!("{label} — {description}"),
        None => label,
    })
}

/// The refusal for a question tool under `Decision mode: auto`, after the
/// recommended answers are recorded for `run_id`. `None` under `ask`, or for
/// any other tool.
pub fn refuse(
    context: &GateContext,
    run_id: &str,
    tool_name: &str,
    tool_input: &Value,
) -> Option<Decision> {
    if context.decision_mode != DecisionMode::Auto || !QUESTION_TOOLS.contains(&tool_name) {
        return None;
    }
    let taken = recommended(tool_input);
    let recorded = record(&context.state_root, run_id, &taken);
    Some(Decision::Deny(Box::new(refusal(&taken, recorded))))
}

pub(crate) fn refusal(taken: &[TakenDecision], recorded: bool) -> Refusal {
    let named = if taken.is_empty() {
        "No question could be read from the call, so decide on your own judgement and say what \
         you decided in your final message."
            .to_owned()
    } else {
        let listed = taken
            .iter()
            .map(|taken| format!("\"{}\" → {}", taken.question, taken.decision))
            .collect::<Vec<_>>()
            .join("; ");
        format!("Apply the recommended answer and continue: {listed}.")
    };
    let kept = if recorded || taken.is_empty() {
        " The person is shown every decision when the turn ends."
    } else {
        " The decision could not be recorded in the local store, so name it in your final message."
    };
    Refusal::not_started(
        "question-decided-here",
        format!("`Decision mode` is `auto` here: nobody is asked. {named}{kept}"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the operator chose to have the run decide; apply the recommended answer and do not \
             ask again in prose",
        ),
    )
}

fn record(state_root: &Path, run_id: &str, taken: &[TakenDecision]) -> bool {
    let Some(now) = session::now_seconds() else {
        return false;
    };
    let Ok(opened) = store::Store::open(&store::beside(state_root)) else {
        return false;
    };
    taken
        .iter()
        .all(|taken| opened.decision_taken(run_id, taken, now).is_ok())
}

/// The decisions `run_id` took since the last report, marked reported, as the
/// text shown to the person. `None` when there are none or the store cannot be read.
pub fn report(state_root: &Path, run_id: &str) -> Option<String> {
    let path = store::beside(state_root);
    store::open_for_read(&path).ok()??;
    let now = session::now_seconds()?;
    let taken = store::Store::open(&path)
        .ok()?
        .decisions_reported(run_id, now)
        .ok()?;
    if taken.is_empty() {
        return None;
    }
    let listed = taken
        .iter()
        .enumerate()
        .map(|(at, taken)| format!("{}. {}\n   → {}", at + 1, taken.question, taken.decision))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "Decisions taken without asking (`Decision mode: auto`):\n{listed}"
    ))
}

#[cfg(test)]
mod tests;
