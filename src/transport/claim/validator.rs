//! The targeted validator's result, and what it does to a correction.

use super::{
    Ledger, PublicationLineage, ReviewTransaction, Standing, all_publications, corrections_remain,
    receipt_standing, receipts_for, record_escalated_receipt, require_operation_id,
    require_recorded_receipt, review_receipt, timeline, verify_claim,
};
use crate::transport::tier::WholeChangeReason;
use crate::transport::{Context, Failure, ownership};

/// Inconclusive inspections a correction tolerates before it escalates: v4's
/// `maxCompactTargetedValidatorAttempts`.
pub const VALIDATOR_ATTEMPTS: usize = 3;

/// The role a correction's computed verdict waits for, named like a lens in
/// `missing_roles`.
pub(super) const VALIDATOR_ROLE: &str = "review-validator";

const ESCALATED_TO: &str = "maintainer";

struct ValidatorResult {
    operation_id: String,
    verdicts: Option<(bool, bool)>,
}

impl ValidatorResult {
    fn from_marker(
        marker: &crate::transport::markers::Marker,
        receipt: &ReviewTransaction,
    ) -> Option<Self> {
        if ReviewTransaction::from_marker(marker)? != *receipt {
            return None;
        }
        let operation_id = marker.get("op-id")?.clone();
        let verdict = |name: &str| match marker.get(name).map(String::as_str) {
            Some("pass") => Some(true),
            Some("fail") => Some(false),
            _ => None,
        };
        let verdicts = match marker.get("inspection").map(String::as_str)? {
            "available" => Some((verdict("original")?, verdict("regression")?)),
            "unavailable" => None,
            _ => return None,
        };
        Some(Self {
            operation_id,
            verdicts,
        })
    }
}

fn results_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ValidatorResult> {
    let mut results: Vec<ValidatorResult> = Vec::new();
    for comment in comments
        .iter()
        .filter(|comment| comment.viewer_did_author && !comment.includes_created_edit)
    {
        for marker in crate::transport::markers::parse(&comment.body) {
            if marker.get("kind").map(String::as_str) != Some("review-validator-result") {
                continue;
            }
            if let Some(result) = ValidatorResult::from_marker(&marker, receipt)
                && !results
                    .iter()
                    .any(|seen| seen.operation_id == result.operation_id)
            {
                results.push(result);
            }
        }
    }
    results
}

pub(super) enum ValidatorState {
    Pending,
    Passed,
    Failed(&'static str),
    Exhausted,
}

pub(super) fn validator_state(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> ValidatorState {
    let results = results_for(comments, receipt);
    match results.iter().find_map(|result| result.verdicts) {
        Some((false, _)) => ValidatorState::Failed("original_criteria_failed"),
        Some((true, false)) => ValidatorState::Failed("correction_regression_failed"),
        Some((true, true)) => ValidatorState::Passed,
        None if results.len() >= VALIDATOR_ATTEMPTS => ValidatorState::Exhausted,
        None => ValidatorState::Pending,
    }
}

impl ValidatorState {
    pub(super) fn cause(&self) -> Option<&'static str> {
        match self {
            Self::Failed(cause) => Some(cause),
            Self::Exhausted => Some("validator_inconclusive"),
            Self::Pending | Self::Passed => None,
        }
    }

    pub(super) fn escalating_cause(&self, correction_available: bool) -> Option<&'static str> {
        match self {
            Self::Failed(_) if correction_available => None,
            _ => self.cause(),
        }
    }
}

/// Whether the receipt is a correction: a publication over a parent on its own
/// pull request that stands rejected or escalated. A timeline that cannot be
/// read to the end owes the validator rather than excusing it.
pub(super) fn owes_validator(comments: &[ownership::Comment], receipt: &ReviewTransaction) -> bool {
    let Ok(publications) = all_publications(comments) else {
        return true;
    };
    let Some(found) = publications.iter().find(|found| found.receipt == *receipt) else {
        return false;
    };
    match &found.lineage {
        Some(lineage) if lineage.parent.pr == receipt.pr && !lineage.recovered => {
            receipt_standing(comments, &lineage.parent)
                .map_or(true, |parent| parent.standing == Standing::Rejected)
        }
        _ => false,
    }
}

/// How the validator reads a correction's own diff.
pub(super) enum DeltaReading {
    /// The recorded range is the correction's own: `parent.head..head`.
    Own,
    /// The range carries a base the correction integrated cleanly, so the correction's own diff
    /// runs from the tree git merges the parent head and that base into.
    Integrated,
    /// The range carries more than the correction's own edits and no clean integrated tree
    /// separates them: the history was rewritten, or the merge did not integrate cleanly.
    Unisolable,
}

/// Read from the `delta-range-reason` and `delta-parent` `publish_with` recorded when
/// `repair_range` answered that the narrow range was not honest. An honest range records
/// `delta-range` instead and no reason, and a marker written before either field existed records
/// neither, so both read as the correction's own range here. A rewritten history is never
/// integrated, whatever else the marker says.
pub(super) fn delta_reading(lineage: &PublicationLineage) -> DeltaReading {
    let Some(reason) = lineage
        .delta_range_reason
        .as_deref()
        .and_then(WholeChangeReason::parse)
        .filter(WholeChangeReason::carries_upstream)
    else {
        return DeltaReading::Own;
    };
    if lineage.integrated_parent && !matches!(reason, WholeChangeReason::HistoryRewritten) {
        DeltaReading::Integrated
    } else {
        DeltaReading::Unisolable
    }
}

pub(super) fn range_delta_unisolable(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> bool {
    let Ok(publications) = all_publications(comments) else {
        return false;
    };
    publications
        .iter()
        .find(|found| found.receipt == *receipt)
        .and_then(|found| found.lineage.as_ref())
        .is_some_and(|lineage| matches!(delta_reading(lineage), DeltaReading::Unisolable))
}

/// The side of the correction the validator reads its diff from: the parent's head, or the tree
/// git merges the parent head and the integrated base into. Refuses
/// `validator-delta-carries-upstream` when the correction carries upstream work and that tree
/// cannot be built without a conflict, so a recorded flag is never trusted over the repository.
pub(super) fn delta_parent(
    repo: &std::path::Path,
    lineage: &PublicationLineage,
    base: &str,
) -> Result<String, Failure> {
    match delta_reading(lineage) {
        DeltaReading::Own => Ok(lineage.parent.head.clone()),
        DeltaReading::Integrated => {
            match super::integrated_parent_tree(repo, &lineage.parent.head, base) {
                Ok((tree, conflicted)) if conflicted.is_empty() => Ok(tree),
                _ => Err(delta_unisolable(lineage)),
            }
        }
        DeltaReading::Unisolable => Err(delta_unisolable(lineage)),
    }
}

fn delta_unisolable(lineage: &PublicationLineage) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "validator-delta-carries-upstream",
        "cause": lineage.delta_range_reason.as_deref().unwrap_or_default(),
        "action": "the correction's diff over its parent carries work that is not the correction's \
                   own and git cannot merge the parent and the base into a clean tree to \
                   separate it, so the validator cannot read the correction alone; a person \
                   decides whether to accept the correction without a targeted validation or \
                   publish a new change",
    }))
}

/// What one validator result records against one exact correction receipt.
pub struct ValidatorResultReview<'a> {
    /// The issue whose correction was validated.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// Stable across every retry of this attempt, and new for each attempt.
    pub operation_id: &'a str,
    /// The publication epoch.
    pub epoch: &'a str,
    /// The pull request number.
    pub pr: u64,
    /// The full published head SHA.
    pub head: &'a str,
    /// The full published base SHA.
    pub base: &'a str,
    /// The complete-target manifest digest.
    pub digest: &'a str,
    /// `available`, or `unavailable` when the frozen trees could not be read.
    pub inspection: &'a str,
    /// `pass` or `fail`: did the correction fix the parent's frozen findings.
    pub original_criteria: &'a str,
    /// `pass` or `fail`: did the correction break anything on the corrected paths.
    pub correction_regression: &'a str,
    /// Why the trees could not be read; required when `inspection` is `unavailable`.
    pub reason: &'a str,
    /// Which inconclusive attempt this is, `1` to `3`; required with `unavailable`.
    pub attempt: &'a str,
    /// The moment the live review claim is judged.
    pub now: &'a str,
}

fn invalid(detail: &str) -> Failure {
    Failure::ConfigDefect(serde_json::json!({
        "ok": false,
        "reason": "invalid-validator-result",
        "detail": detail,
        "action": "name `pass` or `fail` for both criteria when the candidate was inspected, or \
                   `--inspection unavailable` with a reason, the next `--attempt` (1 to 3) and \
                   neither criterion when it could not be; an inspection that failed is never a failed check",
    }))
}

/// The shape check `record_validator_result` applies to an answer, from its text alone, and
/// the inspection word it settles on. Refuses `invalid-validator-result`, always about the answer.
pub(super) fn validator_shape<'a>(
    inspection: &'a str,
    original_criteria: &str,
    correction_regression: &str,
    reason: &str,
) -> Result<&'a str, Failure> {
    let inspection = if inspection.is_empty() {
        "available"
    } else {
        inspection
    };
    let verdict = |word: &str| matches!(word, "pass" | "fail");
    match inspection {
        "available" => {
            if !verdict(original_criteria) || !verdict(correction_regression) {
                return Err(invalid(
                    "an inspected candidate needs `pass` or `fail` for each criterion",
                ));
            }
        }
        "unavailable" => {
            if !original_criteria.is_empty()
                || !correction_regression.is_empty()
                || reason.trim().is_empty()
            {
                return Err(invalid(
                    "an unavailable inspection carries a reason and its attempt, 1 to 3, and no criterion",
                ));
            }
        }
        _ => return Err(invalid("`inspection` is `available` or `unavailable`")),
    }
    Ok(inspection)
}

/// Inconclusive inspections recorded against `receipt`: the attempts already spent.
pub(super) fn inconclusive_attempts(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> usize {
    results_for(comments, receipt)
        .iter()
        .filter(|result| result.verdicts.is_none())
        .count()
}

fn answer(
    request: &ValidatorResultReview<'_>,
    state: &ValidatorState,
    attempts: usize,
    reused: bool,
    correction_available: bool,
) -> serde_json::Value {
    let outcome = match state {
        ValidatorState::Passed => "passed",
        ValidatorState::Failed(_) if correction_available => "rejected",
        ValidatorState::Failed(_) | ValidatorState::Exhausted => "escalated",
        ValidatorState::Pending => "inconclusive",
    };
    serde_json::json!({
        "ok": true,
        "issue": request.issue,
        "outcome": outcome,
        "cause": state.cause(),
        "inconclusive_attempts": attempts,
        "attempts_remaining": VALIDATOR_ATTEMPTS.saturating_sub(attempts),
        "reused_existing_result": reused,
    })
}

/// Records one targeted-validator result against one exact correction receipt.
///
/// The result is a local record and nothing is posted to GitHub. A conclusive
/// result is recorded once; an `unavailable` inspection spends nothing and is
/// retried up to [`VALIDATOR_ATTEMPTS`] times. The last inconclusive attempt, and a
/// conclusive failure once the lineage has spent every correction it is allowed, write
/// the local `escalated` receipt, which `release_ci` refuses, naming the cause. A
/// conclusive failure with a correction remaining writes nothing: the receipt stands
/// rejected and the computed verdict names the failure.
pub fn record_validator_result(
    context: &Context,
    request: &ValidatorResultReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(request.operation_id))?;
    let inspection = validator_shape(
        request.inspection,
        request.original_criteria,
        request.correction_regression,
        request.reason,
    )?;
    let receipt = review_receipt(
        request.epoch,
        request.pr,
        request.head,
        request.base,
        request.digest,
    )?;
    let comments = timeline(context, request.issue, request.now)?;
    require_recorded_receipt(&comments, &receipt, "cannot be given a validator result")?;
    if !owes_validator(&comments, &receipt) {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "validator-transaction-not-a-correction",
            "action": "the validator reads a correction, a publication over a parent that stands \
                       rejected or escalated; this receipt is not one, so it is reviewed as any \
                       publication is",
        })));
    }
    let ledger = Ledger::open(context, request.issue, request.now)?;
    let correction_available = corrections_remain(context, &comments, &receipt)?;
    let results = results_for(&comments, &receipt);
    let attempts = inconclusive_attempts(&comments, &receipt);
    let state = validator_state(&comments, &receipt);
    if results
        .iter()
        .any(|result| result.operation_id == operation_id)
    {
        verify_claim(
            context,
            request.issue,
            request.run_id,
            "review",
            request.now,
            None,
        )?;
        settle(&ledger, request, &receipt, &comments, correction_available)?;
        return Ok(answer(
            request,
            &state,
            attempts,
            true,
            correction_available,
        ));
    }
    match state {
        ValidatorState::Pending => {
            if inspection == "unavailable" && request.attempt != (attempts + 1).to_string() {
                return Err(invalid(&format!(
                    "the next inconclusive attempt is {}",
                    attempts + 1
                )));
            }
        }
        ValidatorState::Passed | ValidatorState::Failed(_) => {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "validator-result-already-recorded",
                "cause": state.cause(),
                "action": "a correction's conclusive validator result is recorded once and \
                           stands; a changed candidate is judged on a new publication recorded \
                           with `republish_review`",
            })));
        }
        ValidatorState::Exhausted => {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "validator-attempts-exhausted",
                "inconclusive_attempts": attempts,
                "action": "the correction escalated after its last inconclusive inspection; a \
                           person decides, and a changed candidate is judged on a new \
                           publication",
            })));
        }
    }
    verify_claim(
        context,
        request.issue,
        request.run_id,
        "review",
        request.now,
        None,
    )?;

    let pr_field = request.pr.to_string();
    let mut written: Vec<(&str, &str)> = vec![
        ("run-id", request.run_id),
        ("op-id", &operation_id),
        ("epoch", request.epoch),
        ("pr", &pr_field),
        ("head", request.head),
        ("base", request.base),
        ("digest", request.digest),
        ("inspection", inspection),
    ];
    if inspection == "available" {
        written.push(("original", request.original_criteria));
        written.push(("regression", request.correction_regression));
    } else {
        written.push(("reason", request.reason));
    }
    let marker = crate::transport::markers::render("review-validator-result", &written)
        .ok_or_else(|| {
            Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
        })?;
    ledger.append(&format!("{marker}\n"), request.now)?;

    let after = timeline(context, request.issue, request.now)?;
    settle(&ledger, request, &receipt, &after, correction_available)?;
    let attempts = inconclusive_attempts(&after, &receipt);
    Ok(answer(
        request,
        &validator_state(&after, &receipt),
        attempts,
        false,
        correction_available,
    ))
}

fn settle(
    ledger: &Ledger,
    request: &ValidatorResultReview<'_>,
    receipt: &ReviewTransaction,
    comments: &[ownership::Comment],
    correction_available: bool,
) -> Result<(), Failure> {
    let Some(cause) = validator_state(comments, receipt).escalating_cause(correction_available)
    else {
        return Ok(());
    };
    if receipts_for(comments, receipt)
        .iter()
        .any(|standing| standing.terminal_state == "escalated")
    {
        return Ok(());
    }
    record_escalated_receipt(
        ledger,
        request.run_id,
        receipt,
        ESCALATED_TO,
        cause,
        request.now,
    )
    .map_err(|failure| {
        Failure::Write(format!(
            "the validator result is recorded, but the `escalated` receipt could not be written \
             to the local store: {}; whether it landed is unconfirmed",
            failure.detail()
        ))
    })
}
