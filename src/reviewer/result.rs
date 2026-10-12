//! Reading what a reviewer answered.
//!
//! One schema per role. An answer is one JSON object, bare or inside one
//! ```json fence, with no field the schema does not name; anything else is
//! refused as `reviewer-result-malformed` and nothing of it is read. The
//! vocabularies and the rules a finding owes are the ones the transport records
//! against. For a finding this applies every check `record_review_finding`
//! makes from the answer's text alone: the id, the class, the vocabularies
//! (a blank word included), the severe declarations, the location's presence
//! and format, and the format of every citation in `evidence` and `impact`.
//! An admitted answer can still be refused by the recorder on a check that
//! needs the trees or the ledger, so a recorder refusal means the reviewer's
//! answer was unusable, never a recorded result.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::role::Answer;
use super::{ReviewerCode, Role};
use crate::outcome::Refusal;
use crate::transport::claim::{
    FINDING_CAUSAL_DISPOSITIONS, FINDING_CLASSES, FINDING_EVIDENCE_CLASSES, FINDING_ORIGINS,
    REFUTATION_OUTCOMES, says_nothing,
};
use crate::transport::claim::{malformed_citation, parse_finding_location};

/// One finding a lens reported.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// The finding's identity, unique within the answer.
    pub id: String,
    /// One of `FINDING_CLASSES`.
    pub class: String,
    /// What the finding cites.
    pub evidence: String,
    /// What it costs if it holds.
    pub impact: String,
    /// The `path:line` it names.
    #[serde(default)]
    pub location: Option<String>,
    /// One of `FINDING_EVIDENCE_CLASSES`; owed by a severe finding.
    #[serde(default)]
    pub evidence_class: Option<String>,
    /// One of `FINDING_CAUSAL_DISPOSITIONS`; owed by a severe finding.
    #[serde(default)]
    pub causal_disposition: Option<String>,
    /// One of `FINDING_ORIGINS`.
    #[serde(default)]
    pub origin: Option<String>,
    /// The finding this one reassesses.
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FindingsAnswer {
    findings: Vec<Finding>,
}

/// One refuter outcome.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    /// The candidate finding's identity.
    pub id: String,
    /// One of `REFUTATION_OUTCOMES`.
    pub outcome: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomesAnswer {
    outcomes: Vec<Outcome>,
}

/// One criterion's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Criterion {
    /// It holds.
    Pass,
    /// It does not.
    Fail,
}

/// The validator's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    /// The candidate was inspected and both criteria are answered.
    Available {
        /// Whether the correction fixed the parent's frozen severe findings.
        original_criteria: Criterion,
        /// Whether the correction broke anything on the paths it touched.
        correction_regression: Criterion,
    },
    /// The candidate could not be inspected, so there is no verdict.
    Unavailable {
        /// Why it could not.
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidationAnswer {
    inspection: String,
    #[serde(default)]
    original_criteria: Option<Criterion>,
    #[serde(default)]
    correction_regression: Option<Criterion>,
    #[serde(default)]
    reason: Option<String>,
}

/// What a role answered, read and admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleResult {
    /// A lens's findings, possibly none.
    Findings(Vec<Finding>),
    /// A refuter's outcomes. Whether one answers every candidate is for the
    /// caller, who holds the candidates.
    Outcomes(Vec<Outcome>),
    /// The validator's inspection.
    Validation(Validation),
}

/// Reads `raw` as `role`'s answer.
pub fn parse(role: Role, raw: &[u8]) -> Result<RoleResult, Refusal> {
    let text = std::str::from_utf8(raw).map_err(|_| malformed("the answer is not UTF-8"))?;
    let object = unfenced(text)?;
    match role.answer() {
        Answer::Findings => {
            let answer: FindingsAnswer = decode(object)?;
            admit_findings(&answer.findings)?;
            Ok(RoleResult::Findings(answer.findings))
        }
        Answer::Outcomes => {
            let answer: OutcomesAnswer = decode(object)?;
            admit_outcomes(&answer.outcomes)?;
            Ok(RoleResult::Outcomes(answer.outcomes))
        }
        Answer::Inspection => {
            let answer: ValidationAnswer = decode(object)?;
            Ok(RoleResult::Validation(admit_validation(answer)?))
        }
    }
}

fn malformed(why: impl std::fmt::Display) -> Refusal {
    ReviewerCode::ResultMalformed.refusal(format!("the reviewer's answer is unusable: {why}"))
}

fn decode<'a, T: Deserialize<'a>>(object: &'a str) -> Result<T, Refusal> {
    serde_json::from_str(object).map_err(malformed)
}

fn unfenced(text: &str) -> Result<&str, Refusal> {
    let text = text.trim();
    let Some(after) = text.strip_prefix("```") else {
        return Ok(text);
    };
    let (info, rest) = after
        .split_once('\n')
        .ok_or_else(|| malformed("a code fence with nothing in it"))?;
    if !matches!(info.trim(), "" | "json") {
        return Err(malformed(format!(
            "a code fence of kind {:?}, not json",
            info.trim()
        )));
    }
    rest.trim_end()
        .strip_suffix("```")
        .map(str::trim)
        .ok_or_else(|| malformed("a code fence that is never closed, or text after it"))
}

fn blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|text| text.trim().is_empty())
}

fn in_vocabulary(field: &str, value: &Option<String>, vocabulary: &[&str]) -> Result<(), Refusal> {
    match value.as_deref().filter(|text| !text.is_empty()) {
        Some(word) if !vocabulary.contains(&word) => Err(malformed(format!(
            "`{field}` is {word:?}, and the words are {vocabulary:?}"
        ))),
        _ => Ok(()),
    }
}

/// The first `id` that appears more than once, or `None` when every finding in
/// one answer names a distinct one.
///
/// The capture refuses a repeat as well as the parser, because a hand-built
/// `RoleResult` never passes through [`parse`]; both call this so the rule has
/// one home.
pub(crate) fn repeated_id(findings: &[Finding]) -> Option<&str> {
    let mut seen = BTreeSet::new();
    findings
        .iter()
        .find(|finding| !seen.insert(finding.id.as_str()))
        .map(|finding| finding.id.as_str())
}

fn admit_findings(findings: &[Finding]) -> Result<(), Refusal> {
    if findings.iter().any(|finding| finding.id.trim().is_empty()) {
        return Err(malformed("a finding with no `id`"));
    }
    if let Some(id) = repeated_id(findings) {
        return Err(malformed(format!("two findings share the id {id:?}")));
    }
    for finding in findings {
        if !FINDING_CLASSES.contains(&finding.class.as_str()) {
            return Err(malformed(format!(
                "`class` of {:?} is {:?}, and the words are {FINDING_CLASSES:?}",
                finding.id, finding.class
            )));
        }
        if says_nothing(&finding.evidence) || says_nothing(&finding.impact) {
            return Err(malformed(format!(
                "{:?} says nothing in `evidence` or `impact`",
                finding.id
            )));
        }
        for (field, text) in [("evidence", &finding.evidence), ("impact", &finding.impact)] {
            if let Some(word) = malformed_citation(text) {
                return Err(malformed(format!(
                    "`{field}` of {:?} cites {word:?}, which is not a repository-relative \
                     `path:line` the recorder can resolve",
                    finding.id
                )));
            }
        }
        match finding.location.as_deref().map(str::trim) {
            Some(location) if !location.is_empty() => {
                if parse_finding_location(location).is_none() {
                    return Err(malformed(format!(
                        "the location of {:?} is {location:?}, not `path:line` or `path:start-end` \
                         with a repository-relative path",
                        finding.id
                    )));
                }
            }
            _ => {
                return Err(malformed(format!(
                    "the finding {:?} names no `location`, and a sealed lens owes one",
                    finding.id
                )));
            }
        }
        in_vocabulary(
            "evidence_class",
            &finding.evidence_class,
            FINDING_EVIDENCE_CLASSES,
        )?;
        in_vocabulary(
            "causal_disposition",
            &finding.causal_disposition,
            FINDING_CAUSAL_DISPOSITIONS,
        )?;
        in_vocabulary("origin", &finding.origin, FINDING_ORIGINS)?;
        if finding.class == "severe" {
            for (name, value) in [
                ("evidence_class", &finding.evidence_class),
                ("causal_disposition", &finding.causal_disposition),
            ] {
                if blank(value) {
                    return Err(malformed(format!(
                        "the severe finding {:?} does not state `{name}`",
                        finding.id
                    )));
                }
            }
        }
    }
    Ok(())
}

fn admit_outcomes(outcomes: &[Outcome]) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for entry in outcomes {
        if entry.id.trim().is_empty() {
            return Err(malformed("an outcome with no `id`"));
        }
        if !seen.insert(entry.id.as_str()) {
            return Err(malformed(format!("two outcomes answer {:?}", entry.id)));
        }
        if !REFUTATION_OUTCOMES.contains(&entry.outcome.as_str()) {
            return Err(malformed(format!(
                "the outcome for {:?} is {:?}, and the words are {REFUTATION_OUTCOMES:?}",
                entry.id, entry.outcome
            )));
        }
    }
    Ok(())
}

fn admit_validation(answer: ValidationAnswer) -> Result<Validation, Refusal> {
    match answer.inspection.as_str() {
        "available" => match (answer.original_criteria, answer.correction_regression) {
            (Some(original_criteria), Some(correction_regression)) => Ok(Validation::Available {
                original_criteria,
                correction_regression,
            }),
            _ => Err(malformed(
                "an inspected candidate needs `pass` or `fail` for each criterion",
            )),
        },
        "unavailable" => {
            if answer.original_criteria.is_some() || answer.correction_regression.is_some() {
                return Err(malformed("an unavailable inspection names no criterion"));
            }
            match answer.reason.filter(|reason| !reason.trim().is_empty()) {
                Some(reason) => Ok(Validation::Unavailable { reason }),
                None => Err(malformed("an unavailable inspection carries a reason")),
            }
        }
        other => Err(malformed(format!(
            "`inspection` is {other:?}, and the words are `available` and `unavailable`"
        ))),
    }
}

#[cfg(test)]
mod tests;
