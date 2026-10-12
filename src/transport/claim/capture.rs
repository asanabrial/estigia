//! Starting a reviewer as a detached job, and recording what it answered once it has.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::computed::policy_for;
use super::validator::{
    VALIDATOR_ROLE, ValidatorResultReview, delta_parent, inconclusive_attempts, validator_shape,
};
use super::{
    FINDING_ORIGINS, FindingReview, LensReportReview, PublicationLineage, Refutation,
    ReviewTransaction, all_publications, finding_operation_id, findings_for, first_severe_findings,
    lens_report_operation_id, precheck_review_finding, record_lens_report, record_refutation,
    record_review_finding, record_validator_result, review_operation_id, review_status, stop, text,
    timeline, verify_held,
};
use crate::config::ModelRouting;
use crate::harness::session::now_seconds;
use crate::harness::tracker;
use crate::outcome::{Refusal, Resolution};
use crate::reviewer::job::{
    self, JobResult, JobStatus, RecordedRefusal, Spawner, discard_result, job_status, model_of,
    prompt_of, write_job,
};
use crate::reviewer::prompt::{FindingForPrompt, PromptInput};
use crate::reviewer::result::{
    self, Criterion, Finding, Outcome, RoleResult, Validation, repeated_id,
};
use crate::reviewer::{
    DEFAULT_DEADLINE, Invocation, Receipt, Recording, ReviewerCode, Role, prompt, reviewer_model,
    settle,
};
use crate::transport::Context;
use crate::transport::Failure;
use crate::transport::store::{Inserted, Job, Ledger, NewJob, Store, open_for_read, store_path};
use crate::transport::target::{review_payload, review_payload_declared_at};
use crate::transport::tier::{ReviewFocus, RiskLevel};

const TOOL: &str = "collect_review_input";
pub(super) const GRACE_SECONDS: u64 = 60;

/// How often a bounded wait re-checks the running job.
const WAIT_POLL: std::time::Duration = std::time::Duration::from_secs(1);

/// The longest a single `collect_review_input` waits for a running reviewer.
///
/// A reviewer runs under a 15 minute deadline, so waiting past it only holds
/// the collecting run without a job that can still answer.
pub(super) const MAX_WAIT_SECONDS: u64 = 15 * 60;

/// How long one collect waits, in seconds, before it refuses `capture-running`.
///
/// Bounded by the caller's `wait_seconds`, by [`MAX_WAIT_SECONDS`] and by the
/// point the job would be abandoned (`deadline_at` plus its grace), so a wait
/// never holds the caller past a job that can still answer.
pub(super) fn patience_for(wait_seconds: u64, deadline: u64, clock: u64) -> u64 {
    wait_seconds
        .min(MAX_WAIT_SECONDS)
        .min(deadline.saturating_sub(clock))
}

pub(super) const ANSWER_SCOPED: &[&str] = &[
    "finding-location-required",
    "lens-finding-location-required",
    "finding-location-malformed",
    "finding-location-outside-the-change",
    "lens-finding-location-outside-the-diff",
    "finding-citation-unresolvable",
    "finding-parent-without-lineage",
    "finding-parent-unknown",
    "new-blocker-without-origin",
    "invalid-finding-class",
    "invalid-finding-origin",
    "invalid-finding-evidence-class",
    "invalid-finding-causal-disposition",
    "incomplete-review-finding",
];

pub(super) const REFUTATION_ANSWER_SCOPED: &[&str] = &[
    "invalid-refutation-batch",
    "invalid-refutation-outcome",
    "refutation-batch-duplicate-finding",
    "refutation-unknown-finding",
    "refutation-includes-deterministic-finding",
];

const REFUTATION_BATCH: &str = "refutation-batch";

/// What `launch_review_input` and `collect_review_input` are asked: the issue, the run and the input
/// they name.
#[derive(Debug, Clone, Copy)]
pub struct CaptureInput<'a> {
    /// The issue whose review is being advanced.
    pub issue: u64,
    /// The run launching or collecting, which must hold the live `review` claim.
    pub run_id: &'a str,
    /// The `name` of one input `review_status` offered in a `collect`.
    pub name: &'a str,
    /// Seconds `collect_review_input` waits for a running job before refusing
    /// `capture-running`; `0` checks once. `launch_review_input` ignores it.
    pub wait_seconds: u64,
    /// The moment the live review claim is judged.
    pub now: &'a str,
}

pub(super) struct Lens<'a> {
    pub issue: u64,
    pub run_id: &'a str,
    pub role: &'a str,
    pub focus: &'a str,
    pub receipt: &'a ReviewTransaction,
    pub now: &'a str,
}

pub(super) fn refusal_of(failure: &Failure) -> Refusal {
    let answer = tracker::Answer {
        code: failure.code(),
        body: Some(failure.envelope()),
    };
    let Some(refusal) = tracker::translate(&answer, TOOL) else {
        unreachable!("a transport failure never carries exit code zero")
    };
    refusal
}

fn answer_scoped(failure: &Failure, scoped: &[&str]) -> bool {
    matches!(failure, Failure::Stop(_) | Failure::ConfigDefect(_))
        && failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|reason| scoped.contains(&reason))
}

pub(super) fn after_a_write(failure: Failure) -> Failure {
    match failure {
        Failure::Stop(mut payload) | Failure::ConfigDefect(mut payload) => {
            if let Some(object) = payload.as_object_mut() {
                object.insert("world".to_owned(), serde_json::json!("committed"));
            }
            Failure::Stop(payload)
        }
        Failure::Read(detail) | Failure::RateLimited(detail) => Failure::Write(detail),
        other => other,
    }
}

fn finding_review<'a>(
    lens: &'a Lens<'a>,
    finding: &'a Finding,
    operation_id: &'a str,
) -> FindingReview<'a> {
    let text = |value: &'a Option<String>| value.as_deref().unwrap_or_default();
    FindingReview {
        issue: lens.issue,
        run_id: lens.run_id,
        reviewer: lens.role,
        operation_id,
        epoch: &lens.receipt.epoch,
        pr: lens.receipt.pr,
        head: &lens.receipt.head,
        base: &lens.receipt.base,
        digest: &lens.receipt.digest,
        id: &finding.id,
        class: &finding.class,
        evidence: &finding.evidence,
        impact: &finding.impact,
        parent: text(&finding.parent),
        origin: text(&finding.origin),
        evidence_class: text(&finding.evidence_class),
        causal_disposition: text(&finding.causal_disposition),
        location: text(&finding.location),
        lens: lens.role,
        now: lens.now,
    }
}

/// Records a lens's findings and then its report, or records nothing.
///
/// Every finding is checked, against the same rules `record_review_finding` applies, before
/// the first one is written: a finding that fails one of the answer-scoped checks rejects the
/// whole answer and leaves the timeline as it was. A failure of any other kind is the recorder's
/// own and is left in `held` exactly as it came. So is a failure of a write, except that once an
/// earlier finding has landed it is marked world `committed` (see `after_a_write`: a clean refusal
/// becomes a `Stop` carrying that world, and a failed read becomes a `Write`).
pub(super) fn record_lens_answer(
    context: &Context,
    ledger: &Ledger,
    lens: &Lens<'_>,
    answer: &RoleResult,
    held: &mut Option<Failure>,
) -> Recording {
    let RoleResult::Findings(findings) = answer else {
        unreachable!("a lens role answers with findings")
    };
    if let Some(id) = repeated_id(findings) {
        return Recording::AnswerRejected(
            ReviewerCode::ResultMalformed
                .refusal(format!("the answer repeats the finding id {id:?}")),
        );
    }
    let mut refused = |failure: Failure| {
        let refusal = refusal_of(&failure);
        *held = Some(failure);
        Recording::Refused(refusal)
    };
    let comments = match ledger.timeline() {
        Ok(comments) => comments,
        Err(failure) => return refused(failure),
    };
    let operation_ids: Vec<String> = findings
        .iter()
        .map(|finding| {
            finding_operation_id(
                lens.run_id,
                lens.role,
                lens.receipt,
                &finding.id,
                &finding.class,
            )
        })
        .collect();
    for (finding, operation_id) in findings.iter().zip(&operation_ids) {
        let review = finding_review(lens, finding, operation_id);
        if let Err(failure) = precheck_review_finding(context, &review, &comments) {
            if answer_scoped(&failure, ANSWER_SCOPED) {
                return Recording::AnswerRejected(refusal_of(&failure));
            }
            return refused(failure);
        }
    }
    let mut wrote = false;
    for (finding, operation_id) in findings.iter().zip(&operation_ids) {
        let review = finding_review(lens, finding, operation_id);
        if let Err(failure) = record_review_finding(context, &review) {
            return refused(if wrote {
                after_a_write(failure)
            } else {
                failure
            });
        }
        wrote = true;
    }
    let operation_id = lens_report_operation_id(lens.run_id, lens.role, lens.receipt);
    let report = LensReportReview {
        issue: lens.issue,
        run_id: lens.run_id,
        operation_id: &operation_id,
        epoch: &lens.receipt.epoch,
        pr: lens.receipt.pr,
        head: &lens.receipt.head,
        base: &lens.receipt.base,
        digest: &lens.receipt.digest,
        role: lens.role,
        focus: lens.focus,
        now: lens.now,
    };
    match record_lens_report(context, &report) {
        Ok(_) => Recording::Recorded,
        Err(failure) => refused(if wrote {
            after_a_write(failure)
        } else {
            failure
        }),
    }
}

pub(super) struct Batch<'a> {
    pub issue: u64,
    pub run_id: &'a str,
    pub receipt: &'a ReviewTransaction,
    pub offered: &'a [String],
    pub now: &'a str,
}

/// Records a refuter's batch, or records nothing.
///
/// The answer must name exactly the findings the `collect` offered, once each: a finding it
/// names that was not offered, or an offered one it leaves out, rejects the whole answer
/// before the recorder is called. The batch is one write, so there is no partial case; the
/// recorder's answer-scoped refusals reject the answer and every other refusal is the
/// recorder's, left in `held` as it came.
pub(super) fn record_refuter_answer(
    context: &Context,
    batch: &Batch<'_>,
    answer: &RoleResult,
    held: &mut Option<Failure>,
) -> Recording {
    let RoleResult::Outcomes(outcomes) = answer else {
        unreachable!("the refuter role answers with outcomes")
    };
    if let Some(stray) = outcomes
        .iter()
        .find(|entry| !batch.offered.contains(&entry.id))
    {
        return Recording::AnswerRejected(refusal_of(&stop(
            "refutation-unknown-finding",
            format!(
                "the answer names `{}`, and the candidates are {:?}",
                stray.id, batch.offered
            ),
            "answer for exactly the candidates under `## Findings`, and none for anything else",
        )));
    }
    if let Some(left_out) = batch
        .offered
        .iter()
        .find(|id| !outcomes.iter().any(|entry| &entry.id == *id))
    {
        return Recording::AnswerRejected(refusal_of(&stop(
            "refutation-batch-incomplete",
            format!(
                "the answer leaves out `{left_out}`, and the candidates are {:?}",
                batch.offered
            ),
            "answer for every candidate under `## Findings`",
        )));
    }
    let rendered = serde_json::to_string(
        &outcomes
            .iter()
            .map(|Outcome { id, outcome }| serde_json::json!({"id": id, "outcome": outcome}))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();
    let refuter = Role::Refuter.name();
    let pr = batch.receipt.pr.to_string();
    let operation_id = review_operation_id(
        "review-refutation",
        &[
            batch.run_id,
            refuter,
            &batch.receipt.epoch,
            &pr,
            &batch.receipt.head,
            &batch.receipt.base,
            &batch.receipt.digest,
        ],
    );
    let refutation = Refutation {
        issue: batch.issue,
        run_id: batch.run_id,
        refuter,
        operation_id: &operation_id,
        epoch: &batch.receipt.epoch,
        pr: batch.receipt.pr,
        head: &batch.receipt.head,
        base: &batch.receipt.base,
        digest: &batch.receipt.digest,
        outcomes: &rendered,
        now: batch.now,
    };
    match record_refutation(context, &refutation) {
        Ok(_) => Recording::Recorded,
        Err(failure) if answer_scoped(&failure, REFUTATION_ANSWER_SCOPED) => {
            Recording::AnswerRejected(refusal_of(&failure))
        }
        Err(failure) => {
            let refusal = refusal_of(&failure);
            *held = Some(failure);
            Recording::Refused(refusal)
        }
    }
}

pub(super) struct Check<'a> {
    pub issue: u64,
    pub run_id: &'a str,
    pub receipt: &'a ReviewTransaction,
    pub attempt: &'a str,
    pub now: &'a str,
}

fn criterion(verdict: Criterion) -> &'static str {
    match verdict {
        Criterion::Pass => "pass",
        Criterion::Fail => "fail",
    }
}

/// Records a validator's inspection, or records nothing.
///
/// The answer's shape is checked, with the function `record_validator_result` applies, before
/// the timeline is read: a shape the check refuses rejects the answer and leaves `held` unset.
/// The attempt is never the answer's: it is the one `launch_review_input` froze, the inconclusive
/// results on the timeline then plus one, so the recorder's only refusals are about the receipt and its budget and are left in
/// `held` as they came. A conclusive `fail` and an `unavailable` inspection are recorded, not
/// rejected.
pub(super) fn record_validator_answer(
    context: &Context,
    check: &Check<'_>,
    answer: &RoleResult,
    held: &mut Option<Failure>,
    recorded: &mut Option<serde_json::Value>,
) -> Recording {
    let RoleResult::Validation(validation) = answer else {
        unreachable!("the validator role answers with an inspection")
    };
    let (inspection, original, regression, reason) = match validation {
        Validation::Available {
            original_criteria,
            correction_regression,
        } => (
            "available",
            criterion(*original_criteria),
            criterion(*correction_regression),
            "",
        ),
        Validation::Unavailable { reason } => ("unavailable", "", "", reason.as_str()),
    };
    if let Err(failure) = validator_shape(inspection, original, regression, reason) {
        return Recording::AnswerRejected(refusal_of(&failure));
    }
    let mut refused = |failure: Failure| {
        let refusal = refusal_of(&failure);
        *held = Some(failure);
        Recording::Refused(refusal)
    };
    let attempt = check.attempt;
    let pr = check.receipt.pr.to_string();
    let operation_id = review_operation_id(
        "review-validator-result",
        &[
            check.run_id,
            VALIDATOR_ROLE,
            &check.receipt.epoch,
            &pr,
            &check.receipt.head,
            &check.receipt.base,
            &check.receipt.digest,
            attempt,
        ],
    );
    let request = ValidatorResultReview {
        issue: check.issue,
        run_id: check.run_id,
        operation_id: &operation_id,
        epoch: &check.receipt.epoch,
        pr: check.receipt.pr,
        head: &check.receipt.head,
        base: &check.receipt.base,
        digest: &check.receipt.digest,
        inspection,
        original_criteria: original,
        correction_regression: regression,
        reason,
        attempt,
        now: check.now,
    };
    let outcome = match record_validator_result(context, &request) {
        Err(Failure::Write(_)) => record_validator_result(context, &request),
        other => other,
    };
    match outcome {
        Ok(answer) => {
            *recorded = Some(answer);
            Recording::Recorded
        }
        Err(failure) => refused(failure),
    }
}

fn prompt_finding(found: &super::ReviewFinding) -> FindingForPrompt {
    FindingForPrompt {
        id: found.id.clone(),
        class: found.class.clone(),
        evidence: found.evidence.clone(),
        impact: found.impact.clone(),
        location: found.location.clone(),
    }
}

fn repair_notice<'a>(parent_epoch: &str, parent_ids: impl Iterator<Item = &'a str>) -> String {
    let ids: Vec<&str> = parent_ids.collect();
    let known = if ids.is_empty() {
        "the parent recorded no findings".to_owned()
    } else {
        format!("the parent's recorded finding ids are {}", ids.join(", "))
    };
    format!(
        "\n\n## Repair lineage\n\nThis publication is a repair of epoch {parent_epoch}. A severe \
         finding that is new to this repair must carry `origin`, one of {}: \
         `introduced` if the repair created the defect, `exposed` if it made an existing one \
         reachable. A finding that reassesses one the parent recorded must name it in `parent`, and \
         only a recorded id is accepted: {known}.",
        FINDING_ORIGINS.join(" or ")
    )
}

fn not_offered(name: &str, detail: String, action: String) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "capture-input-not-offered",
        "name": name,
        "detail": detail,
        "action": action,
    }))
}

fn reviewer_stop(code: &str, detail: String, action: &str) -> Failure {
    match code {
        "reviewer-cli-missing" => stop("reviewer-cli-missing", detail, action),
        "reviewer-exited-nonzero" => stop("reviewer-exited-nonzero", detail, action),
        "reviewer-no-answer" => stop("reviewer-no-answer", detail, action),
        "reviewer-error-result" => stop("reviewer-error-result", detail, action),
        "reviewer-output-too-large" => stop("reviewer-output-too-large", detail, action),
        "reviewer-deadline" => stop("reviewer-deadline", detail, action),
        "reviewer-unavailable" => stop("reviewer-unavailable", detail, action),
        "reviewer-result-malformed" => stop("reviewer-result-malformed", detail, action),
        "reviewer-payload-mismatch" => stop("reviewer-payload-mismatch", detail, action),
        "reviewer-input-too-large" => stop("reviewer-input-too-large", detail, action),
        _ => stop("reviewer-launch-failed", detail, action),
    }
}

fn reviewer_failure(refusal: Refusal) -> Failure {
    let action = match &refusal.resolution {
        Resolution::Run { command } => command.clone(),
        Resolution::NoCommand { detail, .. } => detail.clone(),
    };
    reviewer_stop(refusal.code, refusal.message, &action)
}

fn recorded_failure(refusal: &RecordedRefusal) -> Failure {
    let resolution = &refusal.resolution;
    let action = resolution
        .get("command")
        .or_else(|| resolution.get("detail"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("run the review again; this one is not counted");
    reviewer_stop(&refusal.code, refusal.message.clone(), action)
}

pub(super) fn offered_role(status: &serde_json::Value, name: &str) -> Result<Role, Failure> {
    let next = &status["next"];
    let kind = next["kind"].as_str().unwrap_or_default();
    let reason = next["reason_code"].as_str().unwrap_or_default();
    if kind != "collect" {
        return Err(not_offered(
            name,
            format!("`review_status` answers `{kind}` ({reason}), not a `collect`"),
            "run `review_status` and do what its `next` names".to_owned(),
        ));
    }
    let offered: Vec<&str> = next["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| input["name"].as_str())
        .collect();
    if !offered.contains(&name) {
        return Err(not_offered(
            name,
            format!("`review_status` offers {offered:?} ({reason}), not `{name}`"),
            "capture one of the inputs `review_status` offers".to_owned(),
        ));
    }
    if name == REFUTATION_BATCH {
        return Ok(Role::Refuter);
    }
    match Role::from_name(name) {
        Some(role) if role.is_lens() || role == Role::Validator => Ok(role),
        _ => Err(not_offered(
            name,
            format!("`{name}` is offered, and it is not an input this tool launches"),
            "capture it with the tool `review_status` names for it".to_owned(),
        )),
    }
}

fn offered_candidates(status: &serde_json::Value) -> Vec<String> {
    status["next"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|input| input["name"] == REFUTATION_BATCH)
        .flat_map(|input| input["findings"].as_array().into_iter().flatten())
        .filter_map(|id| id.as_str().map(str::to_owned))
        .collect()
}

fn receipt_of(status: &serde_json::Value) -> Result<ReviewTransaction, Failure> {
    let arguments = &status["next"]["arguments"];
    let receipt = ReviewTransaction {
        epoch: text(arguments, "epoch"),
        pr: arguments["pr"].as_u64().unwrap_or_default(),
        head: text(arguments, "head"),
        base: text(arguments, "base"),
        digest: text(arguments, "digest"),
    };
    if receipt.is_complete() {
        Ok(receipt)
    } else {
        Err(Failure::Read(
            "`review_status` offered an input and named no complete receipt to capture it against"
                .to_owned(),
        ))
    }
}

fn lens_reads_delta(lineage: &PublicationLineage, receipt: &ReviewTransaction) -> bool {
    !lineage.recovered
        && lineage.parent.pr == receipt.pr
        && lineage.delta_range.as_deref() == Some(super::REPAIR_RANGE_LABEL)
        && lineage.delta_range_reason.is_none()
}

pub(super) struct Prepared {
    receipt: ReviewTransaction,
    offered: Vec<String>,
    attempt: Option<String>,
    invocation: Invocation,
    ledger: Ledger,
}

fn prepare(context: &Context, input: &CaptureInput<'_>) -> Result<Prepared, Failure> {
    let status = review_status(context, input.issue)?;
    prepare_with(context, input, &status)
}

pub(super) fn prepare_with(
    context: &Context,
    input: &CaptureInput<'_>,
    status: &serde_json::Value,
) -> Result<Prepared, Failure> {
    let role = offered_role(status, input.name)?;
    let receipt = receipt_of(status)?;

    let ledger = Ledger::open(context, input.issue, input.now)?;
    verify_held(
        context,
        &ledger,
        input.issue,
        input.run_id,
        "review",
        input.now,
        None,
    )?;

    let comments = ledger.timeline()?;
    let Some(recorded) = all_publications(&comments)?
        .into_iter()
        .find(|publication| publication.receipt == receipt)
    else {
        return Err(Failure::Read(
            "the receipt `review_status` named is not a publication on the timeline".to_owned(),
        ));
    };
    let lineage = recorded.lineage.clone();
    let validator = role == Role::Validator;
    let delta_scoped = role.is_lens()
        && lineage
            .as_ref()
            .is_some_and(|lineage| lens_reads_delta(lineage, &receipt));
    let payload = match (&lineage, validator) {
        (Some(lineage), true) => {
            let parent = delta_parent(&context.repo_dir, lineage, &receipt.base)?;
            review_payload_declared_at(&context.repo_dir, &parent, &receipt.head, &receipt.base)?
        }
        (None, true) => {
            return Err(Failure::Read(
                "`review_status` offered the validator for a receipt that records no parent"
                    .to_owned(),
            ));
        }
        (Some(lineage), false) if role.is_lens() && lens_reads_delta(lineage, &receipt) => {
            review_payload_declared_at(
                &context.repo_dir,
                &lineage.parent.head,
                &receipt.head,
                &receipt.base,
            )?
        }
        (_, false) => review_payload(&context.repo_dir, &receipt.base, &receipt.head)?,
    };
    let payload_digest = if validator || delta_scoped {
        payload.digest.clone()
    } else {
        recorded.payload_digest.ok_or_else(|| {
            Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "payload-digest-unavailable",
                "issue": input.issue,
                "action": "this receipt's marker carries no recorded `payload-digest`, so \
                           there is nothing to hold the rebuilt diff against; publish the \
                           review target again",
            }))
        })?
    };
    let issue_body = super::super::commands::issue_view(context, input.issue, "body")?;
    let mut criteria = text(&issue_body, "body");
    let refuter = role == Role::Refuter;
    let mut parent_findings = Vec::new();
    let offered = offered_candidates(status);
    if refuter {
        let standing = first_severe_findings(&comments, &receipt);
        for id in &offered {
            let Some(found) = standing.get(id) else {
                return Err(Failure::Read(format!(
                    "`review_status` offered the finding `{id}` and the timeline holds no severe \
                     finding of that id on this receipt"
                )));
            };
            parent_findings.push(prompt_finding(found));
        }
    } else if let Some(lineage) = &lineage {
        if validator {
            parent_findings = first_severe_findings(&comments, &lineage.parent)
                .values()
                .map(prompt_finding)
                .collect();
        } else {
            let recorded = findings_for(&comments, &lineage.parent);
            criteria.push_str(&repair_notice(
                &lineage.parent.epoch,
                recorded.iter().map(|found| found.id.as_str()),
            ));
            parent_findings = recorded.iter().map(prompt_finding).collect();
        }
    }
    let routing =
        ModelRouting::parse_keeping_what_parses(context.get("Model routing").unwrap_or(""));
    let reviewed = Receipt {
        epoch: receipt.epoch.clone(),
        pr: receipt.pr,
        head: receipt.head.clone(),
        base: receipt.base.clone(),
        digest: receipt.digest.clone(),
        payload_digest,
    };
    let prompt = prompt::build(&PromptInput {
        role,
        receipt: &reviewed,
        payload: payload.text.as_bytes(),
        criteria: &criteria,
        findings: &parent_findings,
    })
    .map_err(reviewer_failure)?;
    let attempt = validator.then(|| (inconclusive_attempts(&comments, &receipt) + 1).to_string());
    Ok(Prepared {
        receipt,
        offered,
        attempt,
        invocation: Invocation {
            role: role.name().to_owned(),
            receipt: reviewed,
            prompt,
            model: reviewer_model(&routing.0),
        },
        ledger,
    })
}

fn jobs_dir() -> Result<std::path::PathBuf, Failure> {
    Ok(store_path()?.with_file_name("jobs"))
}

static LAUNCHES: AtomicU64 = AtomicU64::new(0);

fn job_id(input: &CaptureInput<'_>, epoch: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default()
        .to_string();
    let count = LAUNCHES.fetch_add(1, Ordering::Relaxed).to_string();
    let process = std::process::id().to_string();
    review_operation_id(
        "review-job",
        &[input.run_id, input.name, epoch, &nanos, &count, &process],
    )
}

fn already_running(job: &Job) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "capture-already-running",
        "job": job.id,
        "deadline_at": job.deadline_at,
        "detail": format!(
            "job {} already holds the slot for `{}` on this receipt, and its deadline_at is {}",
            job.id, job.name, job.deadline_at
        ),
        "action": "ask `review_status` again: it names what collects this job once it has \
                   finished, and the slot is launched again only after that job is collected \
                   or abandoned",
    }))
}

fn still_running(job: &Job) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "capture-running",
        "job": job.id,
        "deadline_at": job.deadline_at,
        "detail": format!(
            "job {} for `{}` has not finished; its deadline_at is {}",
            job.id, job.name, job.deadline_at
        ),
        "action": "ask `review_status` again: it names what collects this job once it has finished, \
                   and at the latest after `deadline_at`; nothing was recorded",
    }))
}

fn runner_lost(job: &Job) -> Failure {
    stop(
        "capture-runner-lost",
        format!(
            "the process running job {} for `{}` ended without leaving an answer",
            job.id, job.name
        ),
        "the job is abandoned and nothing was recorded; call `launch_review_input` again for \
         the same name",
    )
}

fn result_stale(job: &Job) -> Failure {
    stop(
        "capture-result-stale",
        format!(
            "the review moved after job {} for `{}` was launched, so its answer is for a receipt \
             that is no longer the one `review_status` offers",
            job.id, job.name
        ),
        "the answer was discarded and nothing was recorded; ask `review_status` and launch \
         what it offers now",
    )
}

fn job_missing(name: &str) -> Failure {
    stop(
        "capture-job-missing",
        format!("no job holds the slot for `{name}` on the current receipt"),
        "call `launch_review_input` for this name once `review_status` offers it",
    )
}

fn job_other_run(job: &Job) -> Failure {
    stop(
        "capture-job-other-run",
        format!(
            "job {} for `{}` was launched by run {}, and only that run collects it",
            job.id, job.name, job.run_id
        ),
        "call `launch_review_input` for this name from this run once `review_status` offers it; \
         a job whose reviewer has finished or died no longer holds the slot",
    )
}

fn leave(ledger: &Ledger, job: &Job, from: &str, to: &str) -> Result<(), Failure> {
    ledger
        .transact(|tx| tx.compare_and_set_state(&job.id, from, to))
        .map(|_| ())
}

pub(super) fn take_slot(
    ledger: &Ledger,
    new: &NewJob<'_>,
    run_id: &str,
) -> Result<Option<Job>, Failure> {
    for _ in 0..2 {
        let held = ledger.transact(|tx| match tx.insert_job(new)? {
            Inserted::Recorded => Ok(None),
            Inserted::SlotTaken => Ok(Some(tx.job_for_slot(&new.receipt.epoch, new.name)?)),
        })?;
        let Some(held) = held else {
            return Ok(None);
        };
        let Some(holder) = held else {
            continue;
        };
        let dead = !matches!(job_status(Path::new(&holder.dir)), Ok(JobStatus::Running));
        if holder.run_id == run_id || !dead {
            return Ok(Some(holder));
        }
        leave(ledger, &holder, &holder.state, "abandoned")?;
    }
    Err(Failure::Read(
        "the slot for this review job kept changing hands".to_owned(),
    ))
}

/// Starts the reviewer one `review_status` `collect` offers, detached, and returns before it has
/// answered.
///
/// Runs every check the reviewer's launch has always had: `review_status` still a `collect` offering
/// `input.name`, the receipt it names, the claim, the payload held against the one the publication
/// recorded, and the prompt built from them. The job directory is written, the job recorded
/// against its slot, and only then is the runner started; a slot a running job already holds is
/// refused `capture-already-running`. Records nothing on the timeline.
pub fn launch_review_input(
    context: &Context,
    input: &CaptureInput<'_>,
) -> Result<serde_json::Value, Failure> {
    let started =
        now_seconds().ok_or_else(|| Failure::Read("the clock is unreadable".to_owned()))?;
    match crate::reviewer::select_reviewer(input.run_id) {
        Ok(adapter) if adapter.relays() => launch_relay(context, input, started),
        Ok(_) => launch_with(context, input, &job::Detached::current(), started),
        Err(refusal) => Err(reviewer_failure(refusal)),
    }
}

/// Starts a relay job: no runner is spawned, because the host runs the reviewer.
///
/// The job directory and its ledger row are written exactly as a CLI launch's
/// are, so `estigia review prompt` can materialise the exact prompt and
/// `estigia review admit` can bind the answer to the same receipt. The answer
/// carries `relay: true`, which is what tells the host to launch the reviewer
/// through its own sub-agent tool.
pub(super) fn launch_relay(
    context: &Context,
    input: &CaptureInput<'_>,
    started_at: u64,
) -> Result<serde_json::Value, Failure> {
    let mut answer = launch_with(context, input, &job::Relayed, started_at)?;
    answer["relay"] = serde_json::Value::Bool(true);
    Ok(answer)
}

fn relay_store() -> Result<Store, Failure> {
    open_for_read(&store_path()?)?.ok_or_else(|| {
        Failure::Read("the local store does not exist, so no relay job is recorded".to_owned())
    })
}

/// The exact prompt of the relay job one run launched for one reviewer name.
///
/// Read from the job directory `launch_review_input` wrote, so the host is handed
/// the bytes the binary built and not a second rendering. A job that is no longer
/// live is `capture-job-missing`: the review has moved past it, or it was already
/// admitted.
pub fn relay_prompt(_context: &Context, run_id: &str, name: &str) -> Result<Vec<u8>, Failure> {
    prompt_of(Path::new(&live_relay_job(run_id, name)?.dir))
}

/// The model the `reviewer` routing row named when the relay job was launched, or
/// `None` when it named none.
///
/// Read from the same job as [`relay_prompt`], so a host that runs the reviewer
/// itself takes the model from the one place the routing row is read, never from
/// a second copy of it.
pub fn relay_model(
    _context: &Context,
    run_id: &str,
    name: &str,
) -> Result<Option<String>, Failure> {
    model_of(Path::new(&live_relay_job(run_id, name)?.dir))
}

fn live_relay_job(run_id: &str, name: &str) -> Result<Job, Failure> {
    let store = relay_store()?;
    let Some((_issue, job)) = store.relay_job(run_id, name)? else {
        return Err(job_missing(name));
    };
    if !matches!(job.state.as_str(), "running" | "admitting") {
        return Err(job_missing(name));
    }
    Ok(job)
}

/// Admits the raw answer a host's relay produced for one reviewer job.
///
/// The same binding and staleness rules `collect_review_input` applies, with the
/// answer handed over directly rather than read from a job's `result.json`: the
/// job's receipt is held against the one `review_status` offers now, and an
/// answer for a receipt the review has moved past is refused `capture-result-stale`
/// with nothing recorded. A job already admitted answers again and writes nothing.
pub fn relay_admit(
    context: &Context,
    run_id: &str,
    name: &str,
    bytes: &[u8],
    now: &str,
) -> Result<serde_json::Value, Failure> {
    let store = relay_store()?;
    let Some((issue, job)) = store.relay_job(run_id, name)? else {
        return Err(job_missing(name));
    };
    if job.run_id != run_id {
        return Err(job_other_run(&job));
    }
    let input = CaptureInput {
        issue,
        run_id,
        name,
        wait_seconds: 0,
        now,
    };
    let status = review_status(context, issue)?;
    let ledger = Ledger::open(context, issue, now)?;
    match job.state.as_str() {
        "admitted" => return admitted_again(context, &input, &job),
        "running" | "admitting" => {}
        _ => return Err(job_missing(name)),
    }
    let role = offered_role(&status, name);
    let current = receipt_of(&status).ok().filter(|r| same_receipt(r, &job));
    let (Ok(role), Some(receipt)) = (role, current) else {
        leave(&ledger, &job, &job.state, "stale")?;
        return Err(result_stale(&job));
    };
    admit(context, &input, &ledger, &job, role, &receipt, bytes)
}

pub(super) fn launch_with(
    context: &Context,
    input: &CaptureInput<'_>,
    spawner: &dyn Spawner,
    started_at: u64,
) -> Result<serde_json::Value, Failure> {
    let prepared = prepare(context, input)?;
    let id = job_id(input, &prepared.receipt.epoch);
    let dir = jobs_dir()?.join(&id);
    write_job(&dir, &prepared.invocation)?;
    let deadline_at = started_at + DEFAULT_DEADLINE.as_secs();
    let offered = serde_json::json!(prepared.offered);
    let dir_text = dir.to_string_lossy();
    let new = NewJob {
        id: &id,
        run_id: input.run_id,
        name: input.name,
        receipt: &prepared.invocation.receipt,
        offered: &offered,
        attempt: prepared.attempt.as_deref(),
        started_at,
        deadline_at,
        dir: &dir_text,
    };
    let ledger = &prepared.ledger;
    let taken = take_slot(ledger, &new, input.run_id).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&dir);
    })?;
    if let Some(holder) = taken {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(already_running(&holder));
    }
    let pid = match spawner.start(&dir) {
        Ok(pid) => pid,
        Err(failure) => {
            ledger.transact(|tx| tx.compare_and_set_state(&id, "running", "abandoned"))?;
            return Err(failure);
        }
    };
    ledger.transact(|tx| tx.mark_spawned(&id, pid))?;
    Ok(serde_json::json!({
        "ok": true,
        "issue": input.issue,
        "name": input.name,
        "epoch": prepared.receipt.epoch,
        "job": id,
        "deadline_at": deadline_at,
    }))
}

fn lens_findings(
    context: &Context,
    input: &CaptureInput<'_>,
    receipt: &ReviewTransaction,
) -> Result<usize, Failure> {
    let comments = timeline(context, input.issue, input.now).map_err(after_a_write)?;
    Ok(findings_for(&comments, receipt)
        .into_iter()
        .filter(|found| found.reviewer == input.name)
        .count())
}

fn same_receipt(current: &ReviewTransaction, job: &Job) -> bool {
    let held = &job.receipt;
    current.epoch == held.epoch
        && current.pr == held.pr
        && current.head == held.head
        && current.base == held.base
        && current.digest == held.digest
}

/// Records the answer of the job a `launch_review_input` started, once it has one.
///
/// Only the launching run collects. `review_status` is read again and must still be a `collect`
/// offering `input.name` against the job's own receipt, or the job is marked `stale` and nothing is
/// recorded. A job still running is refused `capture-running`; `input.wait_seconds` above zero
/// waits that many seconds for it first, re-checking each second, and is capped at
/// `MAX_WAIT_SECONDS` and at the point the job would be abandoned (`deadline_at` plus its grace).
/// One past its deadline and a
/// minute's grace has its process group killed and is abandoned. A runner that died is
/// `capture-runner-lost`, and a reviewer that refused passes through under its own code. An answer
/// is admitted exactly once: the job moves `running` to `admitting` by compare-and-swap, the
/// recorders write under operation ids that do not change, and the job ends `admitted`. A job a
/// crash left `admitting` is admitted again without writing twice.
pub fn collect_review_input(
    context: &Context,
    input: &CaptureInput<'_>,
) -> Result<serde_json::Value, Failure> {
    let clock = now_seconds().ok_or_else(|| Failure::Read("the clock is unreadable".to_owned()))?;
    collect_with(context, input, clock, &job::kill_group)
}

pub(super) fn collect_with(
    context: &Context,
    input: &CaptureInput<'_>,
    clock: u64,
    kill: &dyn Fn(u32),
) -> Result<serde_json::Value, Failure> {
    let status = review_status(context, input.issue)?;
    let ledger = Ledger::open(context, input.issue, input.now)?;
    let epoch = text(&status["receipt"], "epoch");
    let slot = ledger.transact(|tx| tx.job_for_slot(&epoch, input.name))?;
    let Some(job) = slot else {
        let Some(stray) = ledger.transact(|tx| tx.live_job_for_name(input.name))? else {
            return Err(job_missing(input.name));
        };
        if stray.run_id != input.run_id {
            return Err(job_other_run(&stray));
        }
        leave(&ledger, &stray, &stray.state, "stale")?;
        return Err(result_stale(&stray));
    };
    if job.run_id != input.run_id {
        return Err(job_other_run(&job));
    }
    match job.state.as_str() {
        "admitted" => return admitted_again(context, input, &job),
        "running" | "admitting" => {}
        _ => return Err(job_missing(input.name)),
    }
    let role = offered_role(&status, input.name);
    let current = receipt_of(&status).ok().filter(|r| same_receipt(r, &job));
    let (Ok(role), Some(receipt)) = (role, current) else {
        leave(&ledger, &job, &job.state, "stale")?;
        return Err(result_stale(&job));
    };
    let dir = Path::new(&job.dir);
    let deadline = job.deadline_at + GRACE_SECONDS;
    let patience = patience_for(input.wait_seconds, deadline, clock);
    let waited = std::time::Instant::now();
    loop {
        match job_status(dir)? {
            JobStatus::Running => {
                if clock >= deadline {
                    if let Some(pid) = job.pid {
                        kill(pid);
                    }
                    leave(&ledger, &job, &job.state, "abandoned")?;
                    return Err(reviewer_failure(ReviewerCode::Deadline.refusal(format!(
                        "job {} was still running a minute past its deadline_at {}, and its \
                         process group was killed",
                        job.id, job.deadline_at
                    ))));
                }
                if waited.elapsed().as_secs() >= patience {
                    return Err(still_running(&job));
                }
                std::thread::sleep(WAIT_POLL);
            }
            JobStatus::Lost => {
                leave(&ledger, &job, &job.state, "abandoned")?;
                return Err(runner_lost(&job));
            }
            JobStatus::Finished(JobResult::Refused(refusal)) => {
                leave(&ledger, &job, &job.state, "abandoned")?;
                return Err(recorded_failure(&refusal));
            }
            JobStatus::Finished(JobResult::Answer(bytes)) => {
                return admit(context, input, &ledger, &job, role, &receipt, &bytes);
            }
        }
    }
}

fn admitted_again(
    context: &Context,
    input: &CaptureInput<'_>,
    job: &Job,
) -> Result<serde_json::Value, Failure> {
    let mut answer = serde_json::json!({
        "ok": true,
        "issue": input.issue,
        "name": input.name,
        "epoch": job.receipt.epoch,
        "job": job.id,
        "admitted": true,
    });
    if Role::from_name(input.name).is_some_and(Role::is_lens) {
        let receipt = ReviewTransaction {
            epoch: job.receipt.epoch.clone(),
            pr: job.receipt.pr,
            head: job.receipt.head.clone(),
            base: job.receipt.base.clone(),
            digest: job.receipt.digest.clone(),
        };
        answer["findings_recorded"] = lens_findings(context, input, &receipt)?.into();
    }
    Ok(answer)
}

fn admit(
    context: &Context,
    input: &CaptureInput<'_>,
    ledger: &Ledger,
    job: &Job,
    role: Role,
    receipt: &ReviewTransaction,
    bytes: &[u8],
) -> Result<serde_json::Value, Failure> {
    if job.state == "running"
        && !ledger.transact(|tx| tx.compare_and_set_state(&job.id, "running", "admitting"))?
    {
        return Err(still_running(job));
    }
    let answer = match result::parse(role, bytes) {
        Ok(answer) => answer,
        Err(refusal) => {
            leave(ledger, job, "admitting", "abandoned")?;
            return Err(reviewer_failure(refusal));
        }
    };
    let comments = ledger.timeline()?;
    let policy = policy_for(context, &comments, receipt, None)?;
    let focus = if policy.level == RiskLevel::Medium && policy.focus != ReviewFocus::default() {
        policy.focus.word()
    } else {
        ""
    };
    let offered: Vec<String> = job
        .offered
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|id| id.as_str().map(str::to_owned))
        .collect();
    let lens = Lens {
        issue: input.issue,
        run_id: input.run_id,
        role: input.name,
        focus,
        receipt,
        now: input.now,
    };
    let batch = Batch {
        issue: input.issue,
        run_id: input.run_id,
        receipt,
        offered: &offered,
        now: input.now,
    };
    let check = Check {
        issue: input.issue,
        run_id: input.run_id,
        receipt,
        attempt: job.attempt.as_deref().unwrap_or_default(),
        now: input.now,
    };
    let mut held = None;
    let mut validated = None;
    let recording = match &answer {
        RoleResult::Outcomes(_) => record_refuter_answer(context, &batch, &answer, &mut held),
        RoleResult::Validation(_) => {
            record_validator_answer(context, &check, &answer, &mut held, &mut validated)
        }
        _ => record_lens_answer(context, ledger, &lens, &answer, &mut held),
    };
    let admitted = match settle(answer, recording) {
        Ok(answer) => answer,
        Err(refusal) => {
            if held.is_none() {
                leave(ledger, job, "admitting", "abandoned")?;
            }
            return Err(held.take().unwrap_or_else(|| reviewer_failure(refusal)));
        }
    };
    let mut response = serde_json::json!({
        "ok": true,
        "issue": input.issue,
        "name": input.name,
        "epoch": receipt.epoch,
        "job": job.id,
    });
    match admitted {
        RoleResult::Findings(_) => {
            response["findings_recorded"] = lens_findings(context, input, receipt)?.into();
        }
        RoleResult::Outcomes(outcomes) => response["outcomes_recorded"] = outcomes.len().into(),
        RoleResult::Validation(_) => response["result"] = validated.unwrap_or_default(),
    }
    leave(ledger, job, "admitting", "admitted")?;
    let _ = discard_result(Path::new(&job.dir));
    Ok(response)
}
