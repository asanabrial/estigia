//! The verdict computed from one receipt's finding ledger, and the lens
//! completion event that keeps an empty ledger from reading as approval.

use std::path::Path;

use super::recover::{InheritedPolicy, inherited_policy};
use super::validator::{
    VALIDATOR_ROLE, ValidatorState, owes_validator, range_delta_unisolable, validator_state,
};
use super::{
    LaneClearance, OperationKind, ReviewFinding, ReviewTransaction, VISIBILITY_ATTEMPTS,
    approved_receipt_stands, corrections_remain, first_protocol_markers, first_severe_findings,
    operation_marker, protocol_body, receipt_write_failed, record_approved_receipt,
    refutation_outcomes, reject_operation_kind_conflict, require_cleared_publication_lane,
    require_operation_id, require_recorded_receipt, review_receipt, timeline, verify_claim,
};
use crate::transport::tier::{REVIEW_FOCUSES, ReviewFocus, RiskLevel, tier_level};
use crate::transport::{Context, Failure, ownership};

/// The four words a computed verdict may carry.
///
/// `incomplete` is an outcome of the computation and never of a recording, and
/// no outcome is recorded while a licensed lens is still out:
/// [`record_computed_verdict`] refuses both, so the one marker a receipt may
/// hold is a final answer over every lens the tier licenses.
pub const COMPUTED_OUTCOMES: &[&str] = &["approved", "rejected", "escalated", "incomplete"];

/// One of [`COMPUTED_OUTCOMES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputedOutcome {
    /// Every licensed lens reported and nothing is open or escalated.
    Approved,
    /// At least one severe finding is still open and nothing is escalated.
    Rejected,
    /// A severe finding the ledger cannot settle: it has to go to a person.
    Escalated,
    /// A lens the receipt's tier licenses has not reported.
    Incomplete,
}

impl ComputedOutcome {
    /// The word recorded on the marker.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Escalated => "escalated",
            Self::Incomplete => "incomplete",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "escalated" => Some(Self::Escalated),
            "incomplete" => Some(Self::Incomplete),
            _ => None,
        }
    }
}

/// A severe finding the ledger cannot settle, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Escalation {
    /// The finding's id.
    pub id: String,
    /// What the ledger lacks to settle it.
    pub reason: String,
}

impl Escalation {
    /// A plain sentence saying what happened and what the person who takes the decision weighs.
    pub fn why(&self) -> String {
        let id = &self.id;
        match self.reason.as_str() {
            "evidence-class-missing" => format!(
                "the severe finding `{id}` was recorded without saying how it is evidenced, so \
                 the ledger cannot tell whether it blocks the change; decide whether it stands"
            ),
            "causal-disposition-missing" => format!(
                "the severe finding `{id}` was recorded without saying whether this change or \
                 the base caused it, so the ledger cannot tell whether it blocks; decide whether \
                 this change owns it"
            ),
            "insufficient-evidence" => format!(
                "the severe finding `{id}` has insufficient evidence to be confirmed or \
                 dropped; decide whether it blocks the change"
            ),
            "causal-disposition-unknown" => format!(
                "the cause of the severe finding `{id}` is unknown, so the ledger cannot say \
                 whether this change or the base owns it; decide whether it blocks the change"
            ),
            "no-refuter-result" => format!(
                "the inferential severe finding `{id}` has no refuter's answer, so nothing \
                 confirms or drops it; decide whether it stands"
            ),
            "lens-focus-disagreement" => "the lenses reviewed with different focuses, so the \
                 review has no single focus; decide which focus the change is judged under"
                .to_owned(),
            "validator-delta-carries-upstream" => "the correction's history was rewritten or its \
                 merge with the moved base did not integrate cleanly, so its fix cannot be \
                 isolated from the base; decide whether to accept the change without a targeted \
                 validation"
                .to_owned(),
            "original_criteria_failed" => "the targeted validator found the correction did not \
                 fix the parent's severe findings; decide whether to accept the change anyway or \
                 publish a new one"
                .to_owned(),
            "correction_regression_failed" => "the targeted validator found the correction \
                 broke something on the paths it touched; decide whether to accept the change \
                 anyway or publish a new one"
                .to_owned(),
            "validator_inconclusive" => "the targeted validator could not inspect the correction \
                 in three attempts, so nothing confirms the fix; decide whether to accept the \
                 change without a targeted validation"
                .to_owned(),
            other => format!("the ledger escalated `{id}` as `{other}`; decide whether it stands"),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        let mut entry = serde_json::json!(self);
        entry["why"] = serde_json::Value::String(self.why());
        entry
    }
}

/// The plain sentence naming why one receipt stands escalated, from its explicit escalation or
/// from the computed verdict recorded over it; `None` when the timeline records neither.
pub(super) fn escalation_story(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Option<String> {
    let publisher = super::all_publications(comments)
        .ok()?
        .into_iter()
        .find(|found| found.receipt == *receipt)
        .map(|found| found.publisher);
    if let Some(terminal) = super::standing_terminal(comments, receipt, publisher)
        && terminal.terminal_state == "escalated"
    {
        return Some(format!(
            "a run handed the decision to {} with `escalate_review`: {}; the person decides \
             whether to accept the change as it stands or have the remaining work published as a \
             new change",
            terminal.escalated_to.as_deref().unwrap_or("a person"),
            terminal
                .reason
                .as_deref()
                .unwrap_or("no reason was recorded"),
        ));
    }
    let recorded = computed_verdicts_for(comments, receipt);
    let sentences: Vec<String> = recorded
        .first()?
        .verdict
        .escalated
        .iter()
        .map(|found| format!("{}.", capitalised(&found.why())))
        .collect();
    (!sentences.is_empty()).then(|| sentences.join(" "))
}

/// The plain sentence naming what a failed targeted validator found, for the correction that
/// answers it; `None` when the receipt's validator did not conclusively fail.
pub(super) fn validator_rejection_story(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Option<String> {
    let ValidatorState::Failed(cause) = validator_state(comments, receipt) else {
        return None;
    };
    let found = match cause {
        "original_criteria_failed" => "did not fix the parent's severe findings",
        _ => "broke something on the paths it touched",
    };
    Some(format!(
        "the targeted validator found the correction {found}; the next correction fixes that"
    ))
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// What the ledger of one exact receipt computes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputedVerdict {
    /// The verdict.
    pub outcome: ComputedOutcome,
    /// Severe findings that stand and are open for correction.
    pub open: Vec<String>,
    /// Severe findings the ledger cannot settle.
    pub escalated: Vec<Escalation>,
    /// Severe findings owned by the base rather than by this change: not
    /// blocking, and each one a follow-up.
    pub follow_ups: Vec<String>,
    /// Licensed lens roles that have filed no completion event.
    pub missing_roles: Vec<String>,
}

impl ComputedVerdict {
    pub(super) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "outcome": self.outcome.as_str(),
            "open": self.open,
            "escalated": self.escalated.iter().map(Escalation::to_json).collect::<Vec<_>>(),
            "follow_ups": self.follow_ups,
            "missing_roles": self.missing_roles,
        })
    }
}

/// Every role a receipt's review panel can license, and so every role a run may
/// report complete: the four tier lenses, plus the delta lens a
/// `warning`/`suggestion`-cleared narrow repair runs alone. The one list
/// [`record_lens_report`] and [`ReviewLensReport::from_marker`] both read, so a
/// report a run may record is a report the reader accepts.
const REPORTABLE_LENS_ROLES: [&str; 5] = [
    crate::harness::roles::RESERVED_LENS,
    crate::harness::roles::RESERVED_RISK,
    crate::harness::roles::RESERVED_READBACK,
    crate::harness::roles::RESERVED_READABILITY,
    crate::harness::roles::RESERVED_RELIABILITY,
];

/// A lens's completion, recorded whether or not it found anything.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewLensReport {
    operation_id: String,
    attester: String,
    receipt: ReviewTransaction,
    role: String,
    focus: Option<String>,
}

impl ReviewLensReport {
    fn from_marker(marker: &crate::transport::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let attester = marker.get("run-id")?.clone();
        let role = marker.get("role")?.clone();
        if !ownership::is_operation_id(&operation_id)
            || attester.is_empty()
            || !REPORTABLE_LENS_ROLES.contains(&role.as_str())
        {
            return None;
        }
        Some(Self {
            operation_id,
            attester,
            receipt: ReviewTransaction::from_marker(marker)?,
            role,
            focus: marker
                .get("focus")
                .filter(|value| REVIEW_FOCUSES.contains(&value.as_str()))
                .cloned(),
        })
    }
}

fn lens_reports_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ReviewLensReport> {
    first_protocol_markers(comments, "review-lens-report")
        .iter()
        .filter_map(ReviewLensReport::from_marker)
        .filter(|report| report.receipt == *receipt)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReviewComputedVerdict {
    operation_id: String,
    recorder: String,
    pub(super) receipt: ReviewTransaction,
    level: Option<RiskLevel>,
    pub(super) verdict: ComputedVerdict,
}

impl ReviewComputedVerdict {
    pub(super) fn from_marker(marker: &crate::transport::markers::Marker) -> Option<Self> {
        let operation_id = marker.get("op-id")?.clone();
        let recorder = marker.get("run-id")?.clone();
        if !ownership::is_operation_id(&operation_id) || recorder.is_empty() {
            return None;
        }
        Some(Self {
            operation_id,
            recorder,
            receipt: ReviewTransaction::from_marker(marker)?,
            level: marker.get("level").and_then(|word| RiskLevel::parse(word)),
            verdict: ComputedVerdict {
                outcome: ComputedOutcome::parse(marker.get("outcome")?)?,
                open: serde_json::from_str(marker.get("open")?).ok()?,
                escalated: serde_json::from_str(marker.get("escalated")?).ok()?,
                follow_ups: serde_json::from_str(marker.get("follow-ups")?).ok()?,
                missing_roles: serde_json::from_str(marker.get("missing")?).ok()?,
            },
        })
    }
}

fn computed_verdicts_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<ReviewComputedVerdict> {
    first_protocol_markers(comments, "review-computed-verdict")
        .iter()
        .filter_map(ReviewComputedVerdict::from_marker)
        .filter(|recorded| recorded.receipt == *receipt)
        .collect()
}

/// The runs that recorded a computed verdict over this exact receipt.
pub(super) fn recorders_for(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Vec<String> {
    computed_verdicts_for(comments, receipt)
        .into_iter()
        .map(|recorded| recorded.recorder)
        .collect()
}

/// The verdict one receipt's ledger computes, in v4's order: the first row a
/// severe finding matches decides it.
///
/// A finding missing its `evidence_class` or `causal_disposition` is escalated,
/// as is one the ledger calls `insufficient` or `unknown`; a `pre-existing` or
/// `base-only` one is a follow-up and blocks nothing; a `deterministic` one is
/// open; an `inferential` one follows its refuter's word, and escalates when it
/// has none. Escalated outranks rejected, and rejected outranks approved —
/// which additionally needs every licensed lens to have reported, so a receipt
/// with an empty ledger is not approval by omission.
///
/// `licensed_roles` names the panel that sized the review: the narrower roles
/// [`crate::transport::tier::review_tier_for_repair`] answered for a repair,
/// which may be the delta lens alone or a repair's own single lens rather than
/// the whole change's four. `None` is the whole change's own panel — the roles
/// `level` licenses under the focus the reports agree on.
///
/// A conclusive validator failure is open for correction, id `validator`, while
/// `correction_available` and escalated once it is not; an inconclusive validator
/// and an upstream-carrying correction escalate either way.
///
/// At `medium` the lens reports must agree on one focus, an unnamed focus being
/// the default; a disagreement is escalated as the finding `lens-focus`.
pub fn computed_verdict(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    level: RiskLevel,
    licensed_roles: Option<&[&str]>,
    correction_available: bool,
) -> ComputedVerdict {
    let outcomes = refutation_outcomes(comments, receipt);
    let mut open = Vec::new();
    let mut escalated = Vec::new();
    let mut follow_ups = Vec::new();
    for (id, finding) in first_severe_findings(comments, receipt) {
        match settle(&finding, outcomes.get(&id).map(String::as_str)) {
            Settled::Open => open.push(id),
            Settled::Escalated(reason) => escalated.push(Escalation {
                id,
                reason: reason.to_owned(),
            }),
            Settled::FollowUp => follow_ups.push(id),
            Settled::Dropped => {}
        }
    }

    let reports = lens_reports_for(comments, receipt);
    let mut focuses: Vec<ReviewFocus> = Vec::new();
    for report in &reports {
        let named = ReviewFocus::parse(report.focus.as_deref()).unwrap_or_default();
        if !focuses.contains(&named) {
            focuses.push(named);
        }
    }
    let pinned = inherited_policy(comments, receipt)
        .ok()
        .flatten()
        .map(|policy| policy.focus);
    if level == RiskLevel::Medium
        && (focuses.len() > 1 || pinned.is_some_and(|pinned| focuses.iter().any(|f| *f != pinned)))
    {
        escalated.push(Escalation {
            id: "lens-focus".to_owned(),
            reason: "lens-focus-disagreement".to_owned(),
        });
    }
    let focus = match focuses.as_slice() {
        _ if pinned.is_some() => pinned.unwrap_or_default(),
        [only] => *only,
        _ => ReviewFocus::default(),
    };
    let licensed: Vec<&str> = match licensed_roles {
        Some(roles) => roles.to_vec(),
        None => level.lens_roles(focus),
    };
    let mut missing_roles: Vec<String> = licensed
        .iter()
        .filter(|role| !reports.iter().any(|report| report.role.as_str() == **role))
        .map(|role| (*role).to_owned())
        .collect();
    if owes_validator(comments, receipt) {
        let unisolable = range_delta_unisolable(comments, receipt);
        match validator_state(comments, receipt) {
            ValidatorState::Passed => {}
            ValidatorState::Pending if unisolable => escalated.push(Escalation {
                id: "validator".to_owned(),
                reason: "validator-delta-carries-upstream".to_owned(),
            }),
            ValidatorState::Pending => missing_roles.push(VALIDATOR_ROLE.to_owned()),
            ValidatorState::Failed(_) if correction_available => open.push("validator".to_owned()),
            state => escalated.push(Escalation {
                id: "validator".to_owned(),
                reason: state.cause().unwrap_or_default().to_owned(),
            }),
        }
    }

    let outcome = if !escalated.is_empty() {
        ComputedOutcome::Escalated
    } else if !open.is_empty() {
        ComputedOutcome::Rejected
    } else if !missing_roles.is_empty() {
        ComputedOutcome::Incomplete
    } else {
        ComputedOutcome::Approved
    };
    ComputedVerdict {
        outcome,
        open,
        escalated,
        follow_ups,
        missing_roles,
    }
}

enum Settled {
    Open,
    Escalated(&'static str),
    FollowUp,
    Dropped,
}

fn settle(finding: &ReviewFinding, refuted: Option<&str>) -> Settled {
    let Some(evidence_class) = finding.evidence_class.as_deref() else {
        return Settled::Escalated("evidence-class-missing");
    };
    let Some(causal) = finding.causal_disposition.as_deref() else {
        return Settled::Escalated("causal-disposition-missing");
    };
    if evidence_class == "insufficient" {
        return Settled::Escalated("insufficient-evidence");
    }
    if causal == "unknown" {
        return Settled::Escalated("causal-disposition-unknown");
    }
    if matches!(causal, "pre-existing" | "base-only") {
        return Settled::FollowUp;
    }
    if evidence_class == "deterministic" {
        return Settled::Open;
    }
    match refuted {
        Some("refuted") => Settled::Dropped,
        Some(_) => Settled::Open,
        None => Settled::Escalated("no-refuter-result"),
    }
}

pub(super) fn computed_for(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    worktree: Option<&Path>,
) -> Result<(RiskLevel, ComputedVerdict), Failure> {
    let policy = policy_for(context, comments, receipt, worktree)?;
    let licensed = licensed_lens_roles(context, comments, receipt, policy.focus, worktree)?;
    let licensed: Option<Vec<&str>> = licensed
        .as_ref()
        .map(|roles| roles.iter().map(String::as_str).collect());
    let correction_available = corrections_remain(context, comments, receipt)?;
    Ok((
        policy.level,
        computed_verdict(
            comments,
            receipt,
            policy.level,
            licensed.as_deref(),
            correction_available,
        ),
    ))
}

/// The narrower lens roles one receipt's review was sized by, when it was
/// reviewed as a repair rather than as the whole change.
///
/// A first publication, a publication that recorded a raise (whose `high` tier
/// keeps its own panel) — and any publication whose recorded lineage does not
/// name an honest narrow repair range — answers `None`, so the computed verdict
/// reads the whole change's tier roles exactly as before. A repair whose
/// publication recorded the honest `parent-head..head` range answers what
/// [`crate::transport::tier::review_tier_for_repair`] named for it: the same
/// operation that sized the panel it was actually reviewed by, so a
/// `warning`/`suggestion`-cleared narrow repair licenses the delta lens alone,
/// and a `medium` repair over a `high` whole change licenses its one lens
/// rather than the whole change's four.
///
/// The recomputation is deterministic over the receipt's own frozen object ids:
/// `repair_range` reads only `base`, `parent_head` and `head`, none of which the
/// receipt leaves behind when `main` moves, so where the base stands now cannot
/// change the panel. A disagreement with the recorded range is a read failure
/// rather than a silently different panel. A receipt in a recovered lineage
/// keeps the whole change's roles, because the inherited tier is a floor
/// `review_tier_for_repair` does not apply and the floor must still widen the
/// demand it was recorded to widen.
fn licensed_lens_roles(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    focus: ReviewFocus,
    worktree: Option<&Path>,
) -> Result<Option<Vec<String>>, Failure> {
    if super::recover::inherited_policy(comments, receipt)?.is_some()
        || recorded_raise(comments, receipt)?
    {
        return Ok(None);
    }
    let Some(publication) = super::all_publications(comments)?
        .into_iter()
        .find(|found| found.receipt == *receipt)
    else {
        return Ok(None);
    };
    let Some(lineage) = publication.lineage.filter(|lineage| !lineage.recovered) else {
        return Ok(None);
    };
    if lineage.delta_range.as_deref() != Some(super::REPAIR_RANGE_LABEL) {
        return Ok(None);
    }
    let clearing = super::receipt_standing(comments, &lineage.parent)?.clearing_finding;
    let answer = crate::transport::tier::review_tier_for_repair(
        context,
        &receipt.base,
        &lineage.parent.head,
        &receipt.head,
        worktree,
        clearing.as_deref(),
        Some(focus.word()),
    )?;
    if answer["range"].as_str() != Some("repair") {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "repair-range-disagrees-with-recorded-lineage",
            "recorded": lineage.delta_range,
            "answered": answer["range"],
            "action": "this receipt's publication recorded an honest narrow repair range, and the \
                       same range no longer recomputes from the receipt's own object ids; the \
                       panel it was reviewed by cannot be recovered, so publish the work again",
        })));
    }
    Ok(Some(
        answer["lens_roles"]
            .as_array()
            .map(|roles| {
                roles
                    .iter()
                    .filter_map(|role| role.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
    ))
}

/// The tier and lens focus this receipt is reviewed under: its own tier, raised
/// to the one a recovered lineage froze, and that lineage's focus. A receipt in
/// no successor lineage keeps the tier it derives and the focus its lens reports
/// agree on.
pub(super) fn policy_for(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    worktree: Option<&Path>,
) -> Result<InheritedPolicy, Failure> {
    let tier = tier_level(context, &receipt.base, &receipt.head, worktree)?;
    let tier = match recorded_raise(comments, receipt)? {
        true => tier.max(RiskLevel::High),
        false => tier,
    };
    Ok(match inherited_policy(comments, receipt)? {
        Some(inherited) => InheritedPolicy {
            level: inherited.level.max(tier),
            focus: inherited.focus,
        },
        None => {
            let reports = lens_reports_for(comments, receipt);
            let focus = match reports.as_slice() {
                [only, rest @ ..] if rest.iter().all(|r| r.focus == only.focus) => {
                    ReviewFocus::parse(only.focus.as_deref()).unwrap_or_default()
                }
                _ => ReviewFocus::default(),
            };
            InheritedPolicy { level: tier, focus }
        }
    })
}

/// Whether the receipt's publication recorded a raise from the fixed risk list.
/// A word outside it counts for nothing: the raise only ever lifts a tier.
fn recorded_raise(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<bool, Failure> {
    Ok(super::all_publications(comments)?
        .iter()
        .find(|found| found.receipt == *receipt)
        .and_then(|found| found.raise.as_deref())
        .is_some_and(|item| crate::transport::tier::RISK_ITEMS.contains(&item)))
}

/// Whether the receipt carries any computed-verdict marker or lens report, so
/// the computed path alone decides it.
pub(super) fn has_computed_evidence(
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> bool {
    !computed_verdicts_for(comments, receipt).is_empty()
        || !lens_reports_for(comments, receipt).is_empty()
}

/// Recomputes the receipt's verdict over the live ledger and clears it only
/// when that recomputation is `approved` and agrees with the recorded marker.
///
/// A marker is a record, not a verdict: it goes stale when a finding lands
/// after it, so it never clears alone. A `low` receipt licenses no lens and
/// needs no marker; one that has a marker must still agree with it.
pub(super) fn require_computed_clearance(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    worktree: Option<&Path>,
) -> Result<(), Failure> {
    let (level, computed) = computed_for(context, comments, receipt, worktree)?;
    let recorded = computed_verdicts_for(comments, receipt);
    let detail = computed.to_json().to_string();
    if let Some(marker) = recorded.first() {
        if marker.level != Some(level) {
            return Err(super::stop(
                "computed-verdict-tier-mismatch",
                detail,
                "the tier changed after the verdict was recorded; republish and review the \
                 current change",
            ));
        }
        if marker.verdict != computed {
            return Err(super::stop(
                "computed-verdict-stale",
                detail,
                "the ledger changed after the verdict was recorded; keep the PR draft and \
                 review again on a new publication",
            ));
        }
    }
    match computed.outcome {
        ComputedOutcome::Escalated => Err(super::stop(
            "computed-verdict-escalated",
            detail,
            "keep the PR draft; an escalated finding goes to a person",
        )),
        ComputedOutcome::Rejected => Err(super::stop(
            "computed-verdict-rejected",
            detail,
            "keep the PR draft; correct the open findings and publish again",
        )),
        ComputedOutcome::Incomplete => Err(super::stop(
            "computed-verdict-incomplete",
            detail,
            "keep the PR draft; call `review_status`, then `launch_review_input` and then `collect_review_input` for every lens slot \
                 it offers, and for the validator after a correction, then record the verdict",
        )),
        ComputedOutcome::Approved if recorded.is_empty() && level != RiskLevel::Low => {
            Err(super::stop(
                "computed-verdict-unrecorded",
                detail,
                "keep the PR draft; record the computed verdict with `record_computed_verdict`",
            ))
        }
        ComputedOutcome::Approved => Ok(()),
    }
}

/// Everything one lens-completion event records against one exact receipt.
pub struct LensReportReview<'a> {
    /// The issue whose receipt the lens reviewed.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// Stable across every retry of this report.
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
    /// The reserved lens role that finished.
    pub role: &'a str,
    /// The focus a `medium` change ran, or empty for the default.
    pub focus: &'a str,
    /// The moment the live review claim is judged.
    pub now: &'a str,
}

/// Records that one lens finished against one exact receipt, with or without
/// findings.
///
/// Idempotent on the receipt and the role: a second report for the same role,
/// from whichever run, reads back the first. The role must be one a tier can
/// license, and the receipt must be the newest publication on its pull request.
pub fn record_lens_report(
    context: &Context,
    report: &LensReportReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(report.operation_id))?;
    if !REPORTABLE_LENS_ROLES.contains(&report.role) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-lens-role", "value": report.role,
            "expected": REPORTABLE_LENS_ROLES,
            "action": "name one of the lens roles a review panel licenses: the four tier lenses, \
                       or `review-lens` for a delta-lens repair",
        })));
    }
    if !report.focus.is_empty() && !REVIEW_FOCUSES.contains(&report.focus) {
        return Err(Failure::ConfigDefect(serde_json::json!({
            "ok": false, "reason": "invalid-lens-focus", "value": report.focus,
            "expected": REVIEW_FOCUSES,
            "action": "name one of the four focuses, or omit it for the default",
        })));
    }
    let receipt = review_receipt(
        report.epoch,
        report.pr,
        report.head,
        report.base,
        report.digest,
    )?;
    let read = || timeline(context, report.issue, report.now);
    let pr_field = report.pr.to_string();
    let expected = [
        ("run-id", report.run_id),
        ("role", report.role),
        ("epoch", report.epoch),
        ("pr", pr_field.as_str()),
        ("head", report.head),
        ("base", report.base),
        ("digest", report.digest),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-lens-report"])?;
    let reused = |persisted: &ReviewLensReport| {
        serde_json::json!({
            "ok": true,
            "issue": report.issue,
            "attester": persisted.attester,
            "role": persisted.role,
            "focus": persisted.focus,
            "reused_existing_report": true,
        })
    };
    if let Some(marker) = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewLensReport,
        &expected,
    )? {
        let persisted = ReviewLensReport::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-lens-report-operation-conflict",
            }))
        })?;
        return Ok(reused(&persisted));
    }
    require_recorded_receipt(&comments, &receipt, "cannot be recorded as a lens report")?;
    if let Some(standing) = lens_reports_for(&comments, &receipt)
        .iter()
        .find(|standing| standing.role == report.role)
    {
        return Ok(reused(standing));
    }

    verify_claim(
        context,
        report.issue,
        report.run_id,
        "review",
        report.now,
        None,
    )?;

    let mut written: Vec<(&str, &str)> = vec![
        ("run-id", report.run_id),
        ("op-id", &operation_id),
        ("role", report.role),
        ("epoch", report.epoch),
        ("pr", &pr_field),
        ("head", report.head),
        ("base", report.base),
        ("digest", report.digest),
    ];
    if !report.focus.is_empty() {
        written.push(("focus", report.focus));
    }
    let marker =
        crate::transport::markers::render("review-lens-report", &written).ok_or_else(|| {
            Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
        })?;
    let body = protocol_body(
        &format!(
            "Lens `{}` reported complete by `{}` for publication epoch `{}`.",
            report.role, report.run_id, report.epoch
        ),
        &marker,
    );
    crate::transport::commands::record_entry(context, report.issue, &body, report.now)?;

    for _ in 0..VISIBILITY_ATTEMPTS {
        let visible = read().ok().is_some_and(|data| {
            operation_marker(
                &data,
                &operation_id,
                OperationKind::ReviewLensReport,
                &expected,
            )
            .is_ok_and(|found| found.is_some())
        });
        if visible {
            return Ok(serde_json::json!({
                "ok": true,
                "issue": report.issue,
                "attester": report.run_id,
                "role": report.role,
                "focus": (!report.focus.is_empty()).then_some(report.focus),
                "reused_existing_report": false,
            }));
        }
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-lens-report-readback-failed", "world": "committed",
        "action": "repeat the same call unchanged; it reads back what was committed",
    })))
}

/// What the computed verdict of one exact receipt asks.
pub struct ComputedVerdictReview<'a> {
    /// The issue whose receipt is being judged.
    pub issue: u64,
    /// The run recording it, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// Stable across every retry of this recording.
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
    /// The checkout `base..head` is read from.
    pub worktree: Option<&'a Path>,
    /// Local verification standing in for a lane that never ran a step.
    pub never_ran_evidence_file: Option<&'a Path>,
    /// The moment the live review claim is judged.
    pub now: &'a str,
}

/// Answers what the receipt's ledger computes, recording nothing.
///
/// `generated_files` lists every base-declared generated file the change touches. It
/// reproduces them, which runs repository code, only when `reproduce` is set; it
/// informs the reader and decides nothing here.
pub fn show_computed_verdict(
    context: &Context,
    issue: u64,
    receipt: &ReviewTransaction,
    worktree: Option<&Path>,
    reproduce: bool,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let receipt = review_receipt(
        &receipt.epoch,
        receipt.pr,
        &receipt.head,
        &receipt.base,
        &receipt.digest,
    )?;
    let comments = timeline(context, issue, now)?;
    require_recorded_receipt(&comments, &receipt, "cannot be given a computed verdict")?;
    let (_, computed) = computed_for(context, &comments, &receipt, worktree)?;
    let mut answer = computed.to_json();
    answer["ok"] = serde_json::json!(true);
    answer["recorded"] = serde_json::json!(!computed_verdicts_for(&comments, &receipt).is_empty());
    answer["generated_files"] = crate::transport::generated::report(
        worktree.unwrap_or(&context.repo_dir),
        &receipt.base,
        &receipt.head,
        reproduce,
    );
    Ok(answer)
}

/// Records the verdict the receipt's ledger computes, once.
///
/// The caller names the receipt and never the outcome. Every outcome is
/// refused while a licensed lens has not reported, so the one marker a receipt
/// holds is a final answer over every lens its tier licenses, and carries that
/// tier for the release-time recomputation to compare; recording
/// again reads back that marker when the ledger still computes the same words
/// and refuses when it no longer does. `approved` additionally needs the
/// publication lane cleared, never-ran evidence included.
pub fn record_computed_verdict(
    context: &Context,
    request: &ComputedVerdictReview<'_>,
) -> Result<serde_json::Value, Failure> {
    let operation_id = require_operation_id(Some(request.operation_id))?;
    let receipt = review_receipt(
        request.epoch,
        request.pr,
        request.head,
        request.base,
        request.digest,
    )?;
    let read = || timeline(context, request.issue, request.now);
    let pr_field = request.pr.to_string();
    let expected = [
        ("run-id", request.run_id),
        ("epoch", request.epoch),
        ("pr", pr_field.as_str()),
        ("head", request.head),
        ("base", request.base),
        ("digest", request.digest),
    ];
    let comments = read()?;
    reject_operation_kind_conflict(&comments, &operation_id, &["review-computed-verdict"])?;
    let reused = |persisted: &ComputedVerdict| {
        let mut answer = persisted.to_json();
        answer["ok"] = serde_json::json!(true);
        answer["issue"] = serde_json::json!(request.issue);
        answer["reused_existing_verdict"] = serde_json::json!(true);
        answer
    };
    if let Some(marker) = operation_marker(
        &comments,
        &operation_id,
        OperationKind::ReviewComputedVerdict,
        &expected,
    )? {
        let persisted = ReviewComputedVerdict::from_marker(&marker).ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false, "reason": "review-computed-verdict-operation-conflict",
            }))
        })?;
        close_the_transaction(
            context,
            request,
            &comments,
            &receipt,
            &persisted.verdict,
            &persisted.recorder,
        )?;
        return Ok(reused(&persisted.verdict));
    }
    require_recorded_receipt(&comments, &receipt, "cannot be given a computed verdict")?;
    let (level, computed) = computed_for(context, &comments, &receipt, request.worktree)?;
    if let Some(standing) = computed_verdicts_for(&comments, &receipt).first() {
        if standing.verdict == computed {
            close_the_transaction(
                context,
                request,
                &comments,
                &receipt,
                &standing.verdict,
                &standing.recorder,
            )?;
            return Ok(reused(&standing.verdict));
        }
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "computed-verdict-already-recorded",
            "recorded": standing.verdict.to_json(),
            "computed": computed.to_json(),
            "action": "the verdict already on this receipt stands; a changed ledger is judged \
                       on a new publication recorded with `republish_review`",
        })));
    }
    if !computed.missing_roles.is_empty() {
        return Err(Failure::Stop(serde_json::json!({
            "ok": false,
            "reason": "computed-verdict-incomplete",
            "computed": computed.outcome.as_str(),
            "missing_roles": computed.missing_roles,
            "action": "call `review_status`, then `launch_review_input` and then `collect_review_input` for each missing lens slot it \
                       offers, and for the validator after a correction, then record the verdict \
                       again",
        })));
    }

    verify_claim(
        context,
        request.issue,
        request.run_id,
        "review",
        request.now,
        None,
    )?;

    let mut never_ran_evidence: Option<String> = None;
    let mut lane_classification: Option<&'static str> = None;
    if computed.outcome == ComputedOutcome::Approved {
        match require_cleared_publication_lane(
            context,
            request.pr,
            request.head,
            request.base,
            request.never_ran_evidence_file,
        )? {
            LaneClearance::Ordinary => {}
            LaneClearance::NeverRan { evidence } => {
                never_ran_evidence = Some(evidence);
                lane_classification = Some("never-ran");
            }
            LaneClearance::NotConsulted => lane_classification = Some("not-consulted"),
        }
        let regenerated = crate::transport::generated::require_reproduction(
            request.worktree.unwrap_or(&context.repo_dir),
            request.base,
            request.head,
        )?;
        if regenerated > 0 {
            verify_claim(
                context,
                request.issue,
                request.run_id,
                "review",
                request.now,
                None,
            )?;
        }
    }

    let json = |value: serde_json::Value| value.to_string();
    let open = json(serde_json::json!(computed.open));
    let escalated = json(serde_json::json!(computed.escalated));
    let follow_ups = json(serde_json::json!(computed.follow_ups));
    let missing = json(serde_json::json!(computed.missing_roles));
    let mut written: Vec<(&str, &str)> = vec![
        ("run-id", request.run_id),
        ("op-id", &operation_id),
        ("epoch", request.epoch),
        ("pr", &pr_field),
        ("head", request.head),
        ("base", request.base),
        ("digest", request.digest),
        ("level", level.as_str()),
        ("outcome", computed.outcome.as_str()),
        ("open", &open),
        ("escalated", &escalated),
        ("follow-ups", &follow_ups),
        ("missing", &missing),
    ];
    if let Some(classification) = lane_classification {
        written.push(("classification", classification));
    }
    if let Some(evidence) = &never_ran_evidence {
        written.push(("never-ran-evidence", evidence));
    }
    let marker = crate::transport::markers::render("review-computed-verdict", &written)
        .ok_or_else(|| {
            Failure::Stop(serde_json::json!({"ok": false, "reason": "invalid-marker-attribute"}))
        })?;
    let body = protocol_body(
        &format!(
            "Computed review verdict `{}` for publication epoch `{}`, recorded by `{}`: {} open, \
             {} escalated, {} follow-up.",
            computed.outcome.as_str(),
            request.epoch,
            request.run_id,
            computed.open.len(),
            computed.escalated.len(),
            computed.follow_ups.len()
        ),
        &marker,
    );
    crate::transport::commands::record_entry(context, request.issue, &body, request.now)?;

    for _ in 0..VISIBILITY_ATTEMPTS {
        let seen = read().ok();
        let visible = seen.as_ref().is_some_and(|data| {
            operation_marker(
                data,
                &operation_id,
                OperationKind::ReviewComputedVerdict,
                &expected,
            )
            .is_ok_and(|found| found.is_some())
        });
        if visible {
            close_the_transaction(
                context,
                request,
                seen.as_deref().unwrap_or_default(),
                &receipt,
                &computed,
                request.run_id,
            )?;
            let mut answer = computed.to_json();
            answer["ok"] = serde_json::json!(true);
            answer["issue"] = serde_json::json!(request.issue);
            answer["reused_existing_verdict"] = serde_json::json!(false);
            return Ok(answer);
        }
    }
    Err(Failure::Stop(serde_json::json!({
        "ok": false, "reason": "review-computed-verdict-readback-failed", "world": "committed",
        "action": "repeat the same call unchanged; it reads back what was committed",
    })))
}

/// Records the `approved` terminal receipt locally once an `approved` verdict is
/// confirmed on the record, and nothing otherwise.
///
/// The receipt is a local record, never a comment: it is what makes an approval
/// final against a later escalation and what supersedes an earlier one, and a
/// replay of the verdict writes a receipt the first call could not. It credits
/// the recording run, so a run the review forbade — the publisher, or a
/// requester of a handoff — records a receipt that supersedes nothing.
fn close_the_transaction(
    context: &Context,
    request: &ComputedVerdictReview<'_>,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
    verdict: &ComputedVerdict,
    recorder: &str,
) -> Result<(), Failure> {
    if verdict.outcome == ComputedOutcome::Approved && !approved_receipt_stands(comments, receipt) {
        record_approved_receipt(
            context,
            request.issue,
            recorder,
            recorder,
            receipt,
            request.now,
        )
        .map_err(receipt_write_failed)?;
    }
    Ok(())
}
