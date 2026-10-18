use std::sync::{Arc, Mutex};

use super::status::{named, open, stored};
use super::*;
use crate::outcome::MutationOutcome;
use crate::outcome::Refusal;
use crate::reviewer::Receipt;
use crate::reviewer::Recording;
use crate::reviewer::job::{JobStatus, OnThread, Spawner, job_status};
use crate::reviewer::result::{Criterion, Finding, Outcome, RoleResult, Validation};
use crate::reviewer::{Adapter, Invocation, ReviewerCode};
use crate::transport::claim::capture::{
    ANSWER_SCOPED, Batch, CaptureInput, Check, Lens, MAX_WAIT_SECONDS, REFUTATION_ANSWER_SCOPED,
    after_a_write, collect_with, launch_with, offered_role, patience_for, prepare_with,
    record_lens_answer, record_refuter_answer, record_validator_answer, take_slot,
};
use crate::transport::store::NewJob;
use crate::transport::store::{forget_scratch_store, store_path};

const ROLE: &str = RELIABILITY_LENS;

struct World {
    _root: tempfile::TempDir,
    context: Context,
    receipt: ReviewTransaction,
    comments: Vec<ownership::Comment>,
}

fn finding(id: &str, class: &str, location: Option<&str>) -> Finding {
    Finding {
        id: id.to_owned(),
        class: class.to_owned(),
        evidence: "src/foo.rs:1 now carries a comment where the body was".to_owned(),
        impact: "a reader of the changed line is told something the code does not do".to_owned(),
        location: location.map(str::to_owned),
        evidence_class: None,
        causal_disposition: None,
        origin: None,
        parent: None,
    }
}

fn warning(id: &str) -> Finding {
    finding(id, "warning", Some("src/foo.rs:1"))
}

fn world_of(timeline: impl FnOnce(&ReviewTransaction, &str) -> Vec<ownership::Comment>) -> World {
    let (root, context, receipt) = medium_change();
    let payload =
        crate::transport::target::review_payload(&context.repo_dir, &receipt.base, &receipt.head)
            .expect("the change has a payload");
    let comments = timeline(&receipt, &payload.digest);
    forget_scratch_store();
    stored(&context, &comments);
    World {
        _root: root,
        context,
        receipt,
        comments,
    }
}

fn world() -> World {
    world_of(|receipt, _| vec![live_claim("claude-a"), published(receipt)])
}

fn repair_world() -> World {
    world_of(|receipt, _| {
        let parent = ReviewTransaction {
            epoch: op(2),
            head: "d".repeat(40),
            digest: "e".repeat(64),
            ..receipt.clone()
        };
        vec![
            live_claim("claude-a"),
            published(&parent),
            published_with_parent("claude-a", receipt, &parent, "delta-2-1"),
        ]
    })
}

fn record_against(
    world: &World,
    receipt: &ReviewTransaction,
    findings: Vec<Finding>,
) -> (Recording, Option<Failure>) {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&world.comments),
    )]);
    let context = named(&world.context);
    let lens = Lens {
        issue: 42,
        run_id: "claude-a",
        role: ROLE,
        focus: "",
        receipt,
        now: "2026-08-31T00:00:00Z",
    };
    let mut held = None;
    let recording = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let ledger = Ledger::open(&context, 42, NOW).expect("the ledger");
        record_lens_answer(
            &context,
            &ledger,
            &lens,
            &RoleResult::Findings(findings),
            &mut held,
        )
    });
    (recording, held)
}

fn record(world: &World, findings: Vec<Finding>) -> (Recording, Option<Failure>) {
    record_against(world, &world.receipt, findings)
}

fn on_the_timeline(world: &World) -> (Vec<ReviewFinding>, Vec<String>) {
    let comments = Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue");
    (
        findings_for(&comments, &world.receipt),
        first_protocol_markers(&comments, "review-lens-report")
            .iter()
            .filter(|marker| marker.get("epoch") == Some(&world.receipt.epoch))
            .filter_map(|marker| marker.get("role").cloned())
            .collect(),
    )
}

fn nothing_was_written(world: &World) {
    let (findings, reports) = on_the_timeline(world);
    assert!(findings.is_empty(), "findings were written: {findings:?}");
    assert!(reports.is_empty(), "a lens report was written: {reports:?}");
}

fn rejected_as(recording: &Recording) -> &'static str {
    match recording {
        Recording::AnswerRejected(refusal) => {
            assert!(refusal.outcome.is_clean(), "{refusal:?}");
            refusal.code
        }
        other => panic!("expected an answer rejection, got {other:?}"),
    }
}

#[test]
fn every_finding_and_then_the_lens_report_are_recorded() {
    let world = world();

    let (recording, held) = record(
        &world,
        vec![
            warning("a-comment-hides-the-body"),
            warning("a-second-look"),
        ],
    );

    assert_eq!(recording, Recording::Recorded, "{held:?}");
    assert!(held.is_none());
    let (findings, reports) = on_the_timeline(&world);
    let mut ids: Vec<&str> = findings.iter().map(|found| found.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, ["a-comment-hides-the-body", "a-second-look"]);
    assert!(findings.iter().all(|found| found.reviewer == ROLE));
    assert!(
        findings
            .iter()
            .all(|found| found.lens.as_deref() == Some(ROLE))
    );
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0], ROLE);
}

#[test]
fn a_lens_that_found_nothing_still_reports_complete() {
    let world = world();

    let (recording, held) = record(&world, Vec::new());

    assert_eq!(recording, Recording::Recorded, "{held:?}");
    let (findings, reports) = on_the_timeline(&world);
    assert!(findings.is_empty());
    assert_eq!(reports.len(), 1);
}

#[test]
fn recording_the_same_answer_twice_reads_back_the_first() {
    let world = world();
    let answer = || vec![warning("a-comment-hides-the-body")];

    assert_eq!(record(&world, answer()).0, Recording::Recorded);
    assert_eq!(record(&world, answer()).0, Recording::Recorded);

    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1);
    assert_eq!(reports.len(), 1);
}

#[test]
fn a_repeated_finding_id_rejects_the_answer_and_writes_nothing() {
    let world = world();

    let (recording, held) = record(
        &world,
        vec![
            warning("a-comment-hides-the-body"),
            warning("a-comment-hides-the-body"),
        ],
    );

    assert_eq!(rejected_as(&recording), "reviewer-result-malformed");
    assert!(held.is_none());
    nothing_was_written(&world);
}

#[test]
fn a_read_that_fails_after_a_write_is_reported_as_a_write() {
    for failure in [
        Failure::Read("the store went away".to_owned()),
        Failure::RateLimited("the budget is spent".to_owned()),
    ] {
        let reported = after_a_write(failure);
        assert!(
            matches!(reported, Failure::Write(_)),
            "a failed read after a write was read as {reported:?}"
        );
    }
}

#[test]
fn a_config_defect_after_a_write_is_reported_as_committed() {
    let reported = after_a_write(Failure::ConfigDefect(serde_json::json!({
        "ok": false,
        "reason": "invalid-lens-role",
    })));

    match reported {
        Failure::Stop(payload) => assert_eq!(payload["world"], "committed"),
        other => panic!("a config defect after a write was read as {other:?}"),
    }
}

#[test]
fn one_finding_outside_the_change_rejects_the_answer_and_writes_nothing() {
    let world = world();

    let (recording, held) = record(
        &world,
        vec![
            warning("a-valid-one"),
            finding("outside", "warning", Some("elsewhere.rs:1")),
        ],
    );

    assert_eq!(
        rejected_as(&recording),
        "lens-finding-location-outside-the-diff"
    );
    assert!(held.is_none());
    nothing_was_written(&world);
}

fn every_answer_scoped_refusal() -> Vec<(&'static str, bool, Finding)> {
    let with = |mutate: &dyn Fn(&mut Finding)| {
        let mut found = warning("subject");
        mutate(&mut found);
        found
    };
    let severe = |mutate: &dyn Fn(&mut Finding)| {
        with(&|found| {
            found.class = "severe".to_owned();
            found.evidence_class = Some("deterministic".to_owned());
            found.causal_disposition = Some("introduced".to_owned());
            mutate(found);
        })
    };
    vec![
        (
            "lens-finding-location-required",
            false,
            with(&|found| found.location = None),
        ),
        (
            "finding-location-required",
            false,
            severe(&|found| found.location = None),
        ),
        (
            "finding-location-malformed",
            false,
            with(&|found| found.location = Some("nonsense".to_owned())),
        ),
        (
            "finding-location-outside-the-change",
            false,
            severe(&|found| found.location = Some("src/foo.rs:99".to_owned())),
        ),
        (
            "lens-finding-location-outside-the-diff",
            false,
            with(&|found| found.location = Some("elsewhere.rs:1".to_owned())),
        ),
        (
            "finding-citation-unresolvable",
            false,
            with(&|found| found.evidence = "src/nope.rs:3 drops the write".to_owned()),
        ),
        (
            "finding-parent-without-lineage",
            false,
            with(&|found| found.parent = Some("earlier".to_owned())),
        ),
        (
            "finding-parent-unknown",
            true,
            with(&|found| found.parent = Some("never-recorded".to_owned())),
        ),
        (
            "new-blocker-without-origin",
            true,
            severe(&|found| found.causal_disposition = Some("pre-existing".to_owned())),
        ),
        (
            "invalid-finding-class",
            false,
            with(&|found| found.class = "bogus".to_owned()),
        ),
        (
            "invalid-finding-origin",
            false,
            with(&|found| found.origin = Some("bogus".to_owned())),
        ),
        (
            "invalid-finding-evidence-class",
            false,
            with(&|found| found.evidence_class = Some("bogus".to_owned())),
        ),
        (
            "invalid-finding-causal-disposition",
            false,
            with(&|found| found.causal_disposition = Some("bogus".to_owned())),
        ),
        (
            "incomplete-review-finding",
            false,
            with(&|found| found.evidence = String::new()),
        ),
    ]
}

#[test]
fn each_answer_scoped_refusal_rejects_the_answer_and_writes_nothing() {
    for (code, needs_lineage, bad) in every_answer_scoped_refusal() {
        let world = if needs_lineage {
            repair_world()
        } else {
            world()
        };

        let (recording, held) = record(&world, vec![warning("a-valid-one"), bad]);

        assert_eq!(rejected_as(&recording), code, "{held:?}");
        assert!(held.is_none(), "{code}: {held:?}");
        nothing_was_written(&world);
    }
}

#[test]
fn the_answer_scoped_codes_are_exactly_the_ones_measured_and_exist_in_the_recorder() {
    let mut measured: Vec<&str> = every_answer_scoped_refusal()
        .iter()
        .map(|(code, _, _)| *code)
        .collect();
    let mut declared: Vec<&str> = ANSWER_SCOPED.to_vec();
    measured.sort_unstable();
    declared.sort_unstable();
    assert_eq!(measured, declared);

    let source = crate::source_text::code_of(include_str!("../../claim.rs"));
    for code in ANSWER_SCOPED {
        assert!(
            source.contains(&format!("\"reason\": \"{code}\""))
                || source.contains(&format!("\"{code}\"")),
            "`{code}` is no longer a reason `record_review_finding` raises"
        );
    }
}

#[test]
fn a_lost_claim_is_the_recorders_refusal_under_its_own_code_and_writes_nothing() {
    let world = world_of(|receipt, _| vec![published(receipt)]);

    let (recording, held) = record(&world, vec![warning("a-valid-one")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a lost claim was read as {recording:?}");
    };
    let kept = held.expect("the recorder's own failure is kept");
    assert_eq!(kept.envelope()["reason"], refusal.code);
    assert_eq!(refusal.code, "not-current-live-holder");
    nothing_was_written(&world);
}

#[test]
fn a_receipt_that_is_not_the_newest_publication_is_refused_and_not_rejected() {
    let world = world();
    let stale = ReviewTransaction {
        epoch: op(99),
        ..world.receipt.clone()
    };

    let (recording, held) = record_against(&world, &stale, vec![warning("a-valid-one")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a stale receipt was read as {recording:?}");
    };
    assert_eq!(refusal.code, "published-transaction-mismatch");
    assert!(held.is_some());
    nothing_was_written(&world);
}

#[test]
fn a_write_that_fails_after_another_landed_is_reported_as_committed() {
    let world = world();
    let second = finding_operation_id("claude-a", ROLE, &world.receipt, "the-second", "warning");
    let squatter = wrote(
        "9",
        crate::transport::markers::render(
            "review-lens-report",
            &[
                ("run-id", "claude-a"),
                ("op-id", &second),
                ("role", RISK_LENS),
                ("epoch", &world.receipt.epoch),
                ("pr", &world.receipt.pr.to_string()),
                ("head", &world.receipt.head),
                ("base", &world.receipt.base),
                ("digest", &world.receipt.digest),
            ],
        )
        .expect("a marker"),
    );
    let world = World {
        comments: [world.comments.clone(), vec![squatter]].concat(),
        ..world
    };
    forget_scratch_store();
    stored(&world.context, &world.comments);

    let (recording, held) = record(&world, vec![warning("the-first"), warning("the-second")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a failed second write was read as {recording:?}");
    };
    assert_eq!(refusal.code, "operation-id-kind-conflict");
    assert_eq!(refusal.outcome, MutationOutcome::Committed, "{refusal:?}");
    assert!(held.is_some());
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(reports.iter().all(|report| report != ROLE));
}

#[test]
fn a_lens_report_that_fails_after_a_finding_landed_is_reported_as_committed() {
    let world = world();
    let report = lens_report_operation_id("claude-a", ROLE, &world.receipt);
    let squatter = wrote(
        "9",
        crate::transport::markers::render("spend", &[("op-id", &report)]).expect("a marker"),
    );
    let world = World {
        comments: [world.comments.clone(), vec![squatter]].concat(),
        ..world
    };
    forget_scratch_store();
    stored(&world.context, &world.comments);

    let (recording, held) = record(&world, vec![warning("a-comment-hides-the-body")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a failed report was read as {recording:?}");
    };
    assert_eq!(refusal.code, "operation-id-kind-conflict");
    assert_eq!(refusal.outcome, MutationOutcome::Committed, "{refusal:?}");
    assert!(held.is_some());
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(reports.iter().all(|report| report != ROLE));
}

const CRITERIA: &str = "The change keeps `main` returning nothing.";

fn published_with_payload(receipt: &ReviewTransaction, payload_digest: &str) -> ownership::Comment {
    wrote(
        "3",
        crate::transport::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("payload-digest", payload_digest),
            ],
        )
        .expect("a receipt marker"),
    )
}

fn capturable() -> World {
    capturable_as("claude-a")
}

fn capturable_as(run_id: &'static str) -> World {
    world_of(|receipt, payload| vec![live_claim(run_id), published_with_payload(receipt, payload)])
}

struct Stub {
    answer: Result<Vec<u8>, Refusal>,
    seen: Arc<Mutex<Vec<Invocation>>>,
}

impl Adapter for Stub {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        self.seen
            .lock()
            .expect("the invocations")
            .push(invocation.clone());
        self.answer.clone()
    }
}

fn answer_of(findings: &[serde_json::Value]) -> Result<Vec<u8>, Refusal> {
    Ok(serde_json::json!({"findings": findings})
        .to_string()
        .into_bytes())
}

fn answered_finding(id: &str, location: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "class": "warning",
        "evidence": "src/foo.rs:1 now carries a comment where the body was",
        "impact": "a reader of the changed line is told something the code does not do",
        "location": location,
    })
}

fn script_for(world: &World, extra: Vec<serde_json::Value>) -> String {
    let mut stdout = stdout_with(&world.comments);
    stdout["body"] = serde_json::json!(CRITERIA);
    let mut entries = vec![
        open(&world.receipt),
        crate::test_env::issue_view_answer(42, stdout),
    ];
    entries.extend(extra);
    crate::test_env::answers(&entries)
}

const LAUNCHED_AT: u64 = 1_000;
const NOW: &str = "2026-08-31T00:00:00Z";

type Seen = Arc<Mutex<Vec<Invocation>>>;

fn input_for<'a>(name: &'a str, run_id: &'a str) -> CaptureInput<'a> {
    CaptureInput {
        issue: 42,
        run_id,
        name,
        wait_seconds: 0,
        now: NOW,
    }
}

fn input_waiting<'a>(name: &'a str, run_id: &'a str, wait_seconds: u64) -> CaptureInput<'a> {
    CaptureInput {
        issue: 42,
        run_id,
        name,
        wait_seconds,
        now: NOW,
    }
}

fn answering(answer: Result<Vec<u8>, Refusal>, seen: &Seen) -> impl Spawner + use<> {
    let seen = Arc::clone(seen);
    OnThread(move || {
        Ok(Box::new(Stub {
            answer: answer.clone(),
            seen: Arc::clone(&seen),
        }) as Box<dyn Adapter>)
    })
}

fn launch_as(
    world: &World,
    run_id: &str,
    name: &str,
    spawner: &dyn Spawner,
) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        launch_with(&context, &input_for(name, run_id), spawner, LAUNCHED_AT)
    })
}

fn launch(
    world: &World,
    name: &str,
    answer: Result<Vec<u8>, Refusal>,
) -> (Result<serde_json::Value, Failure>, Seen) {
    let seen = Seen::default();
    let outcome = launch_as(world, "claude-a", name, &answering(answer, &seen));
    (outcome, seen)
}

fn collect_as(
    world: &World,
    run_id: &str,
    name: &str,
    clock: u64,
) -> Result<serde_json::Value, Failure> {
    collect_killing(world, run_id, name, clock, &|_| {})
}

fn collect_killing(
    world: &World,
    run_id: &str,
    name: &str,
    clock: u64,
    kill: &dyn Fn(u32),
) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        collect_with(&context, &input_for(name, run_id), clock, kill)
    })
}

fn collect(world: &World, name: &str) -> Result<serde_json::Value, Failure> {
    collect_as(world, "claude-a", name, LAUNCHED_AT)
}

fn collect_waiting(
    world: &World,
    run_id: &str,
    name: &str,
    clock: u64,
    wait_seconds: u64,
    kill: &dyn Fn(u32),
) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        collect_with(
            &context,
            &input_waiting(name, run_id, wait_seconds),
            clock,
            kill,
        )
    })
}

fn job_dir(launched: &serde_json::Value) -> std::path::PathBuf {
    store_path()
        .expect("the scratch store")
        .with_file_name("jobs")
        .join(launched["job"].as_str().expect("a job id"))
}

fn wait_until_done(launched: &serde_json::Value) {
    let dir = job_dir(launched);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while matches!(job_status(&dir), Ok(JobStatus::Running)) {
        assert!(std::time::Instant::now() < until, "the job never finished");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn open_ledger_result(world: &World) -> Result<Ledger, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || Ledger::open(&context, 42, NOW))
}

fn open_ledger(world: &World) -> Ledger {
    open_ledger_result(world).expect("the ledger")
}

fn job_state(world: &World, launched: &serde_json::Value) -> String {
    let id = launched["job"].as_str().expect("a job id").to_owned();
    let ledger = open_ledger(world);
    ledger
        .transact(|tx| tx.job_by_id(&id))
        .expect("the job is readable")
        .expect("the job is recorded")
        .state
}

fn capture(
    world: &World,
    name: &str,
    answer: Result<Vec<u8>, Refusal>,
) -> (Result<serde_json::Value, Failure>, Vec<Invocation>) {
    let (launched, seen) = launch(world, name, answer);
    let outcome = launched.and_then(|launched| {
        wait_until_done(&launched);
        collect(world, name)
    });
    let invocations = seen.lock().expect("the invocations").clone();
    (outcome, invocations)
}

/// A `collect` status offering `name` against `receipt`, for a capture driven without the offer
/// `review_status` would derive. The receipt is the one the status names, so a test can hand the
/// capture an offer the live timeline would not produce.
fn status_offering(receipt: &ReviewTransaction, name: &str) -> serde_json::Value {
    serde_json::json!({
        "next": {
            "kind": "collect",
            "reason_code": "validator-owed",
            "operation": null,
            "arguments": {
                "issue": 42,
                "epoch": receipt.epoch,
                "pr": receipt.pr,
                "head": receipt.head,
                "base": receipt.base,
                "digest": receipt.digest,
            },
            "inputs": [{"name": name, "capture": "launch_review_input"}],
        }
    })
}

fn next_after(world: &World) -> serde_json::Value {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || review_status(&context, 42))
        .unwrap_or_else(|failure| panic!("status refused: {}", failure.envelope()))["next"]
        .clone()
}

fn refused_as(outcome: &Result<serde_json::Value, Failure>) -> String {
    match outcome {
        Err(failure) => failure.envelope()["reason"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        Ok(answer) => panic!("expected a refusal, got {answer}"),
    }
}

#[test]
fn capturing_the_offered_lens_launches_it_once_and_moves_the_review_past_the_missing_report() {
    let world = capturable();
    let before = next_after(&world);
    assert_eq!(before["reason_code"], "lens-report-missing", "{before}");

    let (outcome, launched) = capture(
        &world,
        RELIABILITY_LENS,
        answer_of(&[answered_finding("a-comment-hides-the-body", "src/foo.rs:1")]),
    );

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["findings_recorded"], 1);
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].role, RELIABILITY_LENS);
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(
        prompt.contains("src/foo.rs") && prompt.contains(CRITERIA),
        "the reviewer was not handed the diff and the issue's criteria"
    );
    assert_eq!(launched[0].receipt.epoch, world.receipt.epoch);
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1);
    assert_eq!(reports, [RELIABILITY_LENS]);
    let after = next_after(&world);
    assert_eq!(after["kind"], "execute", "{after}");
    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
}

#[test]
fn findings_recorded_is_what_landed_on_the_timeline_not_the_answer_count() {
    let world = capturable();
    let operation = op(900);
    let pr = world.receipt.pr.to_string();
    let standing = wrote(
        "9",
        crate::transport::markers::render(
            "review-finding",
            &[
                ("run-id", "claude-a"),
                ("op-id", &operation),
                ("reviewer", ROLE),
                ("epoch", &world.receipt.epoch),
                ("pr", &pr),
                ("head", &world.receipt.head),
                ("base", &world.receipt.base),
                ("digest", &world.receipt.digest),
                ("id", "already-there"),
                ("class", "warning"),
                ("evidence", "observed earlier"),
                ("impact", "costs something"),
                ("location", "src/foo.rs:1"),
            ],
        )
        .expect("a finding marker"),
    );
    let world = World {
        comments: [world.comments.clone(), vec![standing]].concat(),
        ..world
    };
    forget_scratch_store();
    stored(&world.context, &world.comments);

    let (outcome, _) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let (findings, _) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(
        answer["findings_recorded"], 1,
        "findings_recorded must count the timeline, not the answer: {answer}"
    );
}

#[test]
fn an_answer_with_a_finding_outside_the_change_is_malformed_and_records_nothing() {
    let world = capturable();

    let (outcome, launched) = capture(
        &world,
        RELIABILITY_LENS,
        answer_of(&[
            answered_finding("a-valid-one", "src/foo.rs:1"),
            answered_finding("outside", "elsewhere.rs:1"),
        ]),
    );

    assert_eq!(refused_as(&outcome), "reviewer-result-malformed");
    assert_eq!(launched.len(), 1);
    nothing_was_written(&world);
    assert_eq!(
        next_after(&world)["reason_code"],
        "lens-report-missing",
        "the review moved on over a rejected answer"
    );
}

#[test]
fn a_lost_claim_refuses_under_its_own_code_before_any_reviewer_runs() {
    let world = world_of(|receipt, payload| vec![published_with_payload(receipt, payload)]);

    let (outcome, launched) = capture(
        &world,
        RELIABILITY_LENS,
        answer_of(&[answered_finding("a-valid-one", "src/foo.rs:1")]),
    );

    assert_eq!(refused_as(&outcome), "not-current-live-holder");
    assert!(launched.is_empty(), "a reviewer ran without a claim");
    nothing_was_written(&world);
}

#[test]
fn a_name_the_collect_does_not_offer_is_refused_and_no_reviewer_runs() {
    let world = capturable();

    let (outcome, launched) = capture(
        &world,
        RISK_LENS,
        answer_of(&[answered_finding("a-valid-one", "src/foo.rs:1")]),
    );

    assert_eq!(refused_as(&outcome), "capture-input-not-offered");
    assert!(launched.is_empty(), "a reviewer ran for a name not offered");
    nothing_was_written(&world);
}

#[test]
fn nothing_is_captured_once_the_review_is_past_collecting() {
    let world = capturable();
    let (first, _) = capture(&world, RELIABILITY_LENS, answer_of(&[]));
    assert!(first.is_ok(), "{first:?}");

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert_eq!(refused_as(&outcome), "capture-input-not-offered");
    assert!(launched.is_empty());
}

#[test]
fn a_missing_reviewer_cli_refuses_under_its_own_code_and_records_nothing() {
    let world = capturable();
    let missing = OnThread(|| {
        Err::<Box<dyn Adapter>, _>(
            ReviewerCode::CliMissing.refusal("neither claude nor codex is installed"),
        )
    });

    let launched = launch_as(&world, "claude-a", RELIABILITY_LENS, &missing)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    wait_until_done(&launched);
    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "reviewer-cli-missing");
    assert_eq!(job_state(&world, &launched), "abandoned");
    nothing_was_written(&world);
}

#[test]
fn a_publication_that_recorded_no_payload_digest_is_refused_before_a_reviewer_runs() {
    let world = world();

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert_eq!(refused_as(&outcome), "payload-digest-unavailable");
    assert!(launched.is_empty());
}

#[test]
fn a_publication_whose_recorded_payload_is_not_the_one_rebuilt_is_refused_before_it_runs() {
    let world = world_of(|receipt, _| {
        vec![
            live_claim("claude-a"),
            published_with_payload(receipt, &"9".repeat(64)),
        ]
    });

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert_eq!(refused_as(&outcome), "reviewer-payload-mismatch");
    assert!(launched.is_empty());
    nothing_was_written(&world);
}

#[test]
fn a_receipt_that_moved_while_the_reviewer_ran_is_stale_and_records_nothing() {
    let world = capturable();
    let (launched, _) = launch(
        &world,
        RELIABILITY_LENS,
        answer_of(&[answered_finding("a-valid-one", "src/foo.rs:1")]),
    );
    let launched = launched.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    wait_until_done(&launched);
    let receipt = ReviewTransaction {
        epoch: op(77),
        ..world.receipt.clone()
    };
    let payload = crate::transport::target::review_payload(
        &world.context.repo_dir,
        &receipt.base,
        &receipt.head,
    )
    .expect("a payload");
    open_ledger_result(&world)
        .and_then(|ledger| {
            ledger.append(&published_with_payload(&receipt, &payload.digest).body, NOW)
        })
        .expect("a newer publication lands while the reviewer runs");

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "capture-result-stale");
    assert_eq!(job_state(&world, &launched), "stale");
    nothing_was_written(&world);
    assert!(job_dir(&launched).join("result.json").exists());
}

fn relay_launch(world: &World, name: &str) -> Result<serde_json::Value, Failure> {
    relay_launch_as(world, name, "claude-a")
}

fn relay_launch_as(world: &World, name: &str, run_id: &str) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::super::capture::launch_relay(&context, &input_for(name, run_id), LAUNCHED_AT)
    })
}

fn relay_admit_as(world: &World, name: &str, bytes: &[u8]) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::super::relay_admit(&context, "claude-a", name, bytes, NOW)
    })
}

/// An OpenCode relay answer carries the launch the host makes: a built-in
/// sub-agent and the marker prompt the relay plugin intercepts, never the
/// reserved reviewer as an agent. Another host's answer carries none.
#[test]
fn an_opencode_relay_answer_carries_the_launch_the_plugin_intercepts() {
    let world = capturable_as("opencode-ses12345deadbeef");
    let launched = relay_launch_as(&world, RELIABILITY_LENS, "opencode-ses12345deadbeef")
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(launched["relay"], true, "{launched}");
    assert_eq!(launched["launch"]["agent"], "general", "{launched}");
    assert_eq!(launched["launch"]["background"], false, "{launched}");
    assert_eq!(
        launched["launch"]["prompt"],
        format!("<!-- estigia:review name={RELIABILITY_LENS} -->"),
        "{launched}"
    );

    let other = relay_launch(&capturable(), RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert!(other.get("launch").is_none(), "{other}");
}

/// A relayed answer is admitted against the job's own receipt: the host built no
/// prompt, and the binary binds what it produced to the publication it was
/// launched for.
#[test]
fn a_relayed_answer_is_admitted_against_the_job_receipt() {
    let world = capturable();
    let launched = relay_launch(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(launched["relay"], true, "{launched}");
    let prompt = crate::reviewer::job::prompt_of(&job_dir(&launched)).expect("the prompt");
    assert!(
        String::from_utf8_lossy(&prompt).contains(CRITERIA),
        "the relay job did not hold the binary's prompt"
    );

    let outcome = relay_admit_as(
        &world,
        RELIABILITY_LENS,
        &answer_of(&[answered_finding("a-valid-one", "src/foo.rs:1")]).expect("an answer"),
    );

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["findings_recorded"], 1, "{answer}");
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1);
    assert_eq!(reports, [RELIABILITY_LENS]);
}

/// A relayed answer for a receipt the review has moved past is refused with the
/// stale class, and nothing is recorded.
#[test]
fn a_relayed_answer_for_a_stale_receipt_is_refused_with_the_stale_class() {
    let world = capturable();
    let launched = relay_launch(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let receipt = ReviewTransaction {
        epoch: op(77),
        ..world.receipt.clone()
    };
    let payload = crate::transport::target::review_payload(
        &world.context.repo_dir,
        &receipt.base,
        &receipt.head,
    )
    .expect("a payload");
    open_ledger_result(&world)
        .and_then(|ledger| {
            ledger.append(&published_with_payload(&receipt, &payload.digest).body, NOW)
        })
        .expect("a newer publication lands while the reviewer runs");

    let outcome = relay_admit_as(
        &world,
        RELIABILITY_LENS,
        &answer_of(&[answered_finding("a-valid-one", "src/foo.rs:1")]).expect("an answer"),
    );

    assert_eq!(refused_as(&outcome), "capture-result-stale");
    assert_eq!(job_state(&world, &launched), "stale");
    nothing_was_written(&world);
}

#[test]
fn the_reviewer_runs_on_the_model_the_reviewer_routing_names() {
    let mut world = capturable();
    world.context.config = vec![(
        "Model routing".to_owned(),
        "reviewer=opus".to_owned(),
        Source::Written,
    )];

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(launched[0].model.as_deref(), Some("opus"));
}

#[test]
fn a_relay_job_hands_the_host_the_model_the_reviewer_routing_named() {
    let mut world = capturable();
    world.context.config = vec![(
        "Model routing".to_owned(),
        "reviewer=anthropic/claude-x".to_owned(),
        Source::Written,
    )];
    relay_launch(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let model = super::super::relay_model(&named(&world.context), "claude-a", RELIABILITY_LENS);

    assert_eq!(
        model.expect("a model"),
        Some("anthropic/claude-x".to_owned())
    );
}

#[test]
fn a_relay_job_with_no_routing_hands_the_host_no_model() {
    let world = capturable();
    relay_launch(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let model = super::super::relay_model(&named(&world.context), "claude-a", RELIABILITY_LENS);

    assert_eq!(model.expect("no failure"), None);
}

#[test]
fn a_relay_model_is_refused_for_a_slot_no_job_holds() {
    let world = capturable();
    relay_launch(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let other = super::super::relay_model(&named(&world.context), "claude-a", "review-risk")
        .map(|_| serde_json::Value::Null);

    assert_eq!(refused_as(&other), "capture-job-missing");
}

#[test]
fn with_no_routing_the_reviewer_cli_picks_its_own_model() {
    let world = capturable();

    let (_, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert_eq!(launched[0].model, None);
}

const ROUTED_PER_ROLE: &str =
    "reviewer=lens-model, refuter=refuter-model, validator=validator-model";

fn routed_per_role(mut world: World) -> World {
    world.context.config = vec![(
        "Model routing".to_owned(),
        ROUTED_PER_ROLE.to_owned(),
        Source::Written,
    )];
    world
}

#[test]
fn a_lens_launches_on_the_reviewer_model_when_every_role_is_routed() {
    let world = routed_per_role(capturable());

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(launched[0].model.as_deref(), Some("lens-model"));
}

#[test]
fn the_refuter_launches_on_the_refuter_model_when_every_role_is_routed() {
    let world = routed_per_role(refutable());

    let (outcome, launched) = capture(
        &world,
        BATCH,
        outcomes_of(&[("f-1", "refuted"), ("f-2", "refuted")]),
    );

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(launched[0].role, REFUTER);
    assert_eq!(launched[0].model.as_deref(), Some("refuter-model"));
}

#[test]
fn the_validator_launches_on_the_validator_model_when_every_role_is_routed() {
    let world = routed_per_role(validator_owed());

    let (outcome, launched) = capture(&world, VALIDATOR, passing());

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(launched[0].role, VALIDATOR);
    assert_eq!(launched[0].model.as_deref(), Some("validator-model"));
}

fn lens_report_focuses(world: &World) -> Vec<Option<String>> {
    let comments = Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue");
    first_protocol_markers(&comments, "review-lens-report")
        .iter()
        .filter(|marker| marker.get("epoch") == Some(&world.receipt.epoch))
        .map(|marker| marker.get("focus").cloned())
        .collect()
}

#[test]
fn a_lens_a_recovered_lineage_froze_the_focus_of_is_reported_under_that_focus() {
    let world = world_of(|receipt, payload| {
        let parent = ReviewTransaction {
            epoch: op(2),
            head: "d".repeat(40),
            digest: "e".repeat(64),
            ..receipt.clone()
        };
        let successor = wrote(
            "8",
            crate::transport::markers::render(
                "published",
                &[
                    ("run-id", "claude-a"),
                    ("epoch", &receipt.epoch),
                    ("pr", &receipt.pr.to_string()),
                    ("head", &receipt.head),
                    ("base", &receipt.base),
                    ("digest", &receipt.digest),
                    ("payload-digest", payload),
                    ("parent", &parent.epoch),
                    ("parent-pr", &parent.pr.to_string()),
                    ("parent-head", &parent.head),
                    ("parent-base", &parent.base),
                    ("parent-digest", &parent.digest),
                    ("delta", "delta-2-3"),
                    ("recovered-from", &parent.epoch),
                ],
            )
            .expect("a published marker"),
        );
        vec![
            live_claim("claude-a"),
            published(&parent),
            computed_by("codex-b", &parent, "escalated"),
            recovery_marker(&parent, "medium", "risk"),
            successor,
        ]
    });
    let before = next_after(&world);
    assert_eq!(before["inputs"][0]["name"], RISK_LENS, "{before}");
    assert_eq!(before["inputs"].as_array().map(Vec::len), Some(1));

    let (outcome, _) = capture(&world, RISK_LENS, answer_of(&[]));

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(
        lens_report_focuses(&world),
        [Some("risk".to_owned())],
        "the report names no focus, so the lineage's pinned `risk` disagrees with it"
    );
}

#[test]
fn a_lens_reported_under_the_default_focus_names_none() {
    let world = capturable();

    let (outcome, _) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(lens_report_focuses(&world), [None]);
}

fn lens_correction_world(range_reason: Option<&str>) -> World {
    lens_correction_world_marked(range_reason, |_, _, _| {})
}

fn lens_correction_world_marked(
    range_reason: Option<&str>,
    edit: impl FnOnce(
        &ReviewTransaction,
        &mut Vec<(&'static str, String)>,
        &mut Vec<ownership::Comment>,
    ),
) -> World {
    let (root, context, parent) = medium_change();
    let repo = context.repo_dir.clone();
    crate::paths::replace_atomically(&repo.join("src/bar.rs"), "fn corrected() {}\n")
        .expect("the correction file");
    git(&repo, &["add", "src/bar.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "correction"]);
    let child = ReviewTransaction {
        epoch: op(2),
        head: head_of(&repo),
        digest: "e".repeat(64),
        ..parent.clone()
    };
    let parent_payload =
        crate::transport::target::review_payload(&repo, &parent.base, &parent.head)
            .expect("the parent payload");
    let child_payload = crate::transport::target::review_payload(&repo, &child.base, &child.head)
        .expect("the child payload");
    let mut owned: Vec<(&'static str, String)> = vec![
        ("run-id", "claude-a".to_owned()),
        ("epoch", child.epoch.clone()),
        ("pr", child.pr.to_string()),
        ("head", child.head.clone()),
        ("base", child.base.clone()),
        ("digest", child.digest.clone()),
        ("payload-digest", child_payload.digest),
        ("parent", parent.epoch.clone()),
        ("parent-pr", parent.pr.to_string()),
        ("parent-head", parent.head.clone()),
        ("parent-base", parent.base.clone()),
        ("parent-digest", parent.digest.clone()),
        ("delta", "delta-2-1".to_owned()),
    ];
    match range_reason {
        Some(reason) => owned.push(("delta-range-reason", reason.to_owned())),
        None => owned.push(("delta-range", "parent-head..head".to_owned())),
    }
    let mut extra = Vec::new();
    edit(&parent, &mut owned, &mut extra);
    let fields: Vec<(&str, &str)> = owned
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let repair = wrote(
        "8",
        crate::transport::markers::render("published", &fields).expect("a published marker"),
    );
    let comments = vec![
        live_claim("claude-a"),
        published_with_payload(&parent, &parent_payload.digest),
        recorded_finding_on(
            &parent,
            11,
            "parent-defect",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
        repair,
    ];
    let mut comments = comments;
    comments.extend(extra);
    forget_scratch_store();
    stored(&context, &comments);
    World {
        _root: root,
        context,
        receipt: child,
        comments,
    }
}

fn lens_prompt(world: &World) -> String {
    let (outcome, launched) = capture(world, RELIABILITY_LENS, answer_of(&[]));
    outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    String::from_utf8_lossy(&launched[0].prompt).into_owned()
}

#[test]
fn a_lens_over_a_correction_with_an_honest_range_reads_the_delta() {
    let world = lens_correction_world(None);
    let repo = &world.context.repo_dir;
    let parent_head = world.comments.iter().find_map(|comment| {
        crate::transport::markers::parse(&comment.body)
            .into_iter()
            .find_map(|marker| marker.get("parent-head").cloned())
    });
    let delta = crate::transport::target::review_payload_declared_at(
        repo,
        &parent_head.expect("the parent head"),
        &world.receipt.head,
        &world.receipt.base,
    )
    .expect("the delta");

    let prompt = lens_prompt(&world);

    assert!(
        prompt.contains("diff --git a/src/bar.rs"),
        "the delta is missing"
    );
    assert!(
        !prompt.contains("diff --git a/src/foo.rs"),
        "the lens was handed the whole target"
    );
    assert!(
        prompt.contains(&format!("\npayload-digest: {}", delta.digest)),
        "the prompt does not carry the delta's digest"
    );
}

#[test]
fn a_lens_over_a_correction_whose_range_is_not_honest_reads_the_whole_target() {
    for reason in [
        "history-rewritten",
        "empty-repair-range",
        "base-advanced",
        "merged-range",
    ] {
        let world = lens_correction_world(Some(reason));
        let full = crate::transport::target::review_payload(
            &world.context.repo_dir,
            &world.receipt.base,
            &world.receipt.head,
        )
        .expect("the target");

        let prompt = lens_prompt(&world);

        assert!(prompt.contains("diff --git a/src/foo.rs"), "{reason}");
        assert!(prompt.contains("diff --git a/src/bar.rs"), "{reason}");
        assert!(
            prompt.contains(&format!("\npayload-digest: {}", full.digest)),
            "{reason}"
        );
    }
}

fn assert_lens_reads_the_whole_target(world: &World, why: &str) {
    let full = crate::transport::target::review_payload(
        &world.context.repo_dir,
        &world.receipt.base,
        &world.receipt.head,
    )
    .expect("the target");

    let prompt = lens_prompt(world);

    assert!(prompt.contains("diff --git a/src/foo.rs"), "{why}");
    assert!(prompt.contains("diff --git a/src/bar.rs"), "{why}");
    assert!(
        prompt.contains(&format!("\npayload-digest: {}", full.digest)),
        "{why}"
    );
}

fn set_field(fields: &mut Vec<(&'static str, String)>, name: &'static str, value: &str) {
    fields.retain(|(existing, _)| *existing != name);
    fields.push((name, value.to_owned()));
}

#[test]
fn a_lens_over_a_recovered_successor_reads_the_whole_target() {
    let world = lens_correction_world_marked(None, |parent, fields, comments| {
        set_field(fields, "recovered-from", &parent.epoch);
        comments.push(recovery_marker(parent, "medium", "reliability"));
    });

    assert_lens_reads_the_whole_target(&world, "recovered");
}

#[test]
fn a_lens_over_a_correction_whose_parent_is_on_another_pull_request_reads_the_whole_target() {
    let world = lens_correction_world_marked(None, |_, fields, _| {
        set_field(fields, "parent-pr", "9999");
    });

    assert_lens_reads_the_whole_target(&world, "cross-pr");
}

#[test]
fn a_lens_over_a_marker_naming_both_the_honest_range_and_a_reason_reads_the_whole_target() {
    let world = lens_correction_world_marked(Some("base-advanced"), |_, fields, _| {
        set_field(fields, "delta-range", "parent-head..head");
    });

    assert_lens_reads_the_whole_target(&world, "range and reason");
}

#[test]
fn a_lens_over_a_marker_with_neither_range_field_reads_the_whole_target() {
    let world = lens_correction_world_marked(None, |_, fields, _| {
        fields.retain(|(name, _)| *name != "delta-range");
    });

    assert_lens_reads_the_whole_target(&world, "legacy marker");
}

#[test]
fn a_correction_whose_parent_head_cannot_be_resolved_refuses_the_lens_and_launches_nothing() {
    let world = lens_correction_world_marked(None, |_, fields, _| {
        set_field(fields, "parent-head", &"f".repeat(40));
    });

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert_eq!(refused_as(&outcome), "read-failed");
    assert!(launched.is_empty(), "a lens was launched on no payload");
}

fn refuter_prompt(world: World) -> String {
    let receipt = world.receipt.clone();
    let mut comments = world.comments.clone();
    comments.push(recorded_lens_report_for(
        &receipt,
        14,
        RELIABILITY_LENS,
        None,
    ));
    comments.push(recorded_finding_on(
        &receipt,
        15,
        "f-1",
        "severe",
        Some("inferential"),
        Some("introduced"),
    ));
    forget_scratch_store();
    stored(&world.context, &comments);
    let world = World { comments, ..world };

    let (outcome, launched) = capture(&world, BATCH, outcomes_of(&[("f-1", "corroborated")]));

    outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(launched[0].role, REFUTER);
    String::from_utf8_lossy(&launched[0].prompt).into_owned()
}

#[test]
fn the_refuter_over_a_delta_scoped_correction_reads_what_the_lens_reads() {
    let world = lens_correction_world(None);
    let lens = lens_prompt(&world);
    let parent_head = world
        .comments
        .iter()
        .find_map(|comment| {
            crate::transport::markers::parse(&comment.body)
                .into_iter()
                .find_map(|marker| marker.get("parent-head").cloned())
        })
        .expect("the parent head");
    let delta = crate::transport::target::review_payload_declared_at(
        &world.context.repo_dir,
        &parent_head,
        &world.receipt.head,
        &world.receipt.base,
    )
    .expect("the delta");

    let prompt = refuter_prompt(world);

    assert!(prompt.contains("diff --git a/src/bar.rs"));
    assert!(!prompt.contains("diff --git a/src/foo.rs"));
    assert!(prompt.contains(&format!("\npayload-digest: {}", delta.digest)));
    assert!(prompt.contains(&delta.text));
    assert!(lens.contains(&delta.text));
}

#[test]
fn the_refuter_over_a_correction_that_is_not_delta_scoped_reads_the_whole_target() {
    for reason in ["base-advanced", "merged-range"] {
        let world = lens_correction_world(Some(reason));
        let full = crate::transport::target::review_payload(
            &world.context.repo_dir,
            &world.receipt.base,
            &world.receipt.head,
        )
        .expect("the target");

        let prompt = refuter_prompt(world);

        assert!(prompt.contains("diff --git a/src/foo.rs"), "{reason}");
        assert!(prompt.contains("diff --git a/src/bar.rs"), "{reason}");
        assert!(
            prompt.contains(&format!("\npayload-digest: {}", full.digest)),
            "{reason}"
        );
    }
}

#[test]
fn a_lens_over_an_initial_review_is_handed_the_whole_target_and_its_recorded_digest() {
    let world = world_of(|receipt, payload| {
        vec![
            live_claim("claude-a"),
            published_with_payload(receipt, payload),
        ]
    });
    let full = crate::transport::target::review_payload(
        &world.context.repo_dir,
        &world.receipt.base,
        &world.receipt.head,
    )
    .expect("the target");

    let prompt = lens_prompt(&world);

    assert!(prompt.contains(&full.text), "the payload was altered");
    assert!(prompt.contains(&format!("\npayload-digest: {}", full.digest)));
}

#[test]
fn a_lens_answer_over_a_delta_round_is_recorded() {
    let world = lens_correction_world(None);
    let on_the_delta = serde_json::json!({
        "id": "bar-note",
        "class": "warning",
        "evidence": "src/bar.rs:1 now carries a comment where the body was",
        "impact": "a reader of the changed line is told something the code does not do",
        "location": "src/bar.rs:1",
    });

    let (outcome, _) = capture(&world, RELIABILITY_LENS, answer_of(&[on_the_delta]));

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["findings_recorded"], 1);
}

fn repair_capturable_with(
    parent_findings: impl FnOnce(&ReviewTransaction) -> Vec<ownership::Comment>,
) -> World {
    world_of(|receipt, payload| {
        let parent = ReviewTransaction {
            epoch: op(2),
            head: "d".repeat(40),
            digest: "e".repeat(64),
            ..receipt.clone()
        };
        let repair = wrote(
            "8",
            crate::transport::markers::render(
                "published",
                &[
                    ("run-id", "claude-a"),
                    ("epoch", &receipt.epoch),
                    ("pr", &receipt.pr.to_string()),
                    ("head", &receipt.head),
                    ("base", &receipt.base),
                    ("digest", &receipt.digest),
                    ("payload-digest", payload),
                    ("parent", &parent.epoch),
                    ("parent-pr", &parent.pr.to_string()),
                    ("parent-head", &parent.head),
                    ("parent-base", &parent.base),
                    ("parent-digest", &parent.digest),
                    ("delta", "delta-2-1"),
                ],
            )
            .expect("a published marker"),
        );
        let mut comments = vec![live_claim("claude-a"), published(&parent)];
        comments.extend(parent_findings(&parent));
        comments.push(repair);
        comments
    })
}

fn repair_capturable() -> World {
    repair_capturable_with(|parent| {
        vec![recorded_finding_on(
            parent,
            11,
            "parent-defect",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        )]
    })
}

fn repair_without_parent_findings() -> World {
    repair_capturable_with(|_| Vec::new())
}

#[test]
fn a_lens_over_a_repair_is_told_so_and_a_severe_answer_with_an_origin_is_recorded() {
    let world = repair_capturable();
    let severe = serde_json::json!({
        "id": "new-defect",
        "class": "severe",
        "evidence": "src/foo.rs:1 now carries a comment where the body was",
        "impact": "a reader of the changed line is told something the code does not do",
        "location": "src/foo.rs:1",
        "evidence_class": "deterministic",
        "causal_disposition": "introduced",
        "origin": "introduced",
    });

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[severe]));

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["findings_recorded"], 1);
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(
        prompt.contains(&op(2)),
        "the prompt never names the parent epoch"
    );
    assert!(
        prompt.contains(
            "A severe finding that is new to this repair must carry `origin`, one of introduced or exposed"
        ),
        "the prompt never says a new severe finding owes an origin"
    );
    assert!(
        prompt.contains("the parent's recorded finding ids are parent-defect."),
        "the prompt never lists the parent's finding ids for `parent` to cite"
    );
    assert!(
        prompt.contains("The parent publication recorded the findings below"),
        "the prompt never labels the parent's findings as the parent's"
    );
    assert!(
        prompt.contains("observed the defect directly"),
        "the parent's findings never reached the prompt"
    );
    assert!(prompt.contains(CRITERIA), "the issue's criteria were lost");
}

#[test]
fn a_repair_whose_parent_recorded_nothing_is_told_so() {
    let world = repair_without_parent_findings();

    let (outcome, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    assert!(outcome.is_ok(), "{outcome:?}");
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(
        prompt.contains("the parent recorded no findings"),
        "the no-parent-findings branch of the repair notice never ran"
    );
}

#[test]
fn a_lens_finding_that_cites_a_recorded_parent_is_recorded() {
    let world = repair_capturable();
    let citing = serde_json::json!({
        "id": "reassessed",
        "class": "warning",
        "evidence": "src/foo.rs:1 now carries a comment where the body was",
        "impact": "a reader of the changed line is told something the code does not do",
        "location": "src/foo.rs:1",
        "parent": "parent-defect",
    });

    let (outcome, _) = capture(&world, RELIABILITY_LENS, answer_of(&[citing]));

    assert!(outcome.is_ok(), "{outcome:?}");
    let (findings, _) = on_the_timeline(&world);
    let found = findings
        .iter()
        .find(|finding| finding.id == "reassessed")
        .unwrap_or_else(|| panic!("the citing finding was not recorded: {findings:?}"));
    assert_eq!(found.parent.as_deref(), Some("parent-defect"));
}

#[test]
fn a_first_publication_is_not_described_as_a_repair() {
    let world = capturable();

    let (_, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(!prompt.contains("Repair lineage"));
}

#[test]
fn the_first_publication_of_a_second_pull_request_carries_no_repair_notice_or_findings() {
    let world = world_of(|receipt, payload| {
        let other = ReviewTransaction {
            epoch: op(2),
            pr: receipt.pr + 1,
            head: "d".repeat(40),
            digest: "e".repeat(64),
            ..receipt.clone()
        };
        vec![
            live_claim("claude-a"),
            published(&other),
            recorded_finding_on(&other, 5, "f-other", "severe", None, Some("introduced")),
            published_with_payload(receipt, payload),
        ]
    });

    let (_, launched) = capture(&world, RELIABILITY_LENS, answer_of(&[]));

    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(!prompt.contains("Repair lineage"), "{prompt}");
    assert!(!prompt.contains("f-other"), "{prompt}");
    assert!(prompt.contains("## Findings\n\nnone"), "{prompt}");
}

const REFUTER: &str = "review-refuter";
const BATCH: &str = "refutation-batch";

fn refutable() -> World {
    world_of(|receipt, payload| {
        vec![
            live_claim("claude-a"),
            published_with_payload(receipt, payload),
            recorded_lens_report_for(receipt, 4, RELIABILITY_LENS, None),
            recorded_finding_on(
                receipt,
                6,
                "f-1",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
            recorded_finding_on(
                receipt,
                7,
                "f-2",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
            recorded_finding_on(
                receipt,
                8,
                "f-det",
                "severe",
                Some("deterministic"),
                Some("introduced"),
            ),
            recorded_finding_on(receipt, 9, "w-1", "warning", None, None),
        ]
    })
}

fn outcomes_of(entries: &[(&str, &str)]) -> Result<Vec<u8>, Refusal> {
    let outcomes: Vec<serde_json::Value> = entries
        .iter()
        .map(|(id, outcome)| serde_json::json!({"id": id, "outcome": outcome}))
        .collect();
    Ok(serde_json::json!({"outcomes": outcomes})
        .to_string()
        .into_bytes())
}

fn refutations_on_the_timeline(world: &World) -> Vec<ReviewRefutation> {
    let comments = Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue");
    refutations_for(&comments, &world.receipt)
}

#[test]
fn capturing_the_offered_batch_launches_the_refuter_once_with_exactly_the_candidates_and_records_it()
 {
    let world = refutable();
    let before = next_after(&world);
    assert_eq!(before["reason_code"], "refutation-missing", "{before}");

    let (outcome, launched) = capture(
        &world,
        BATCH,
        outcomes_of(&[("f-1", "corroborated"), ("f-2", "refuted")]),
    );

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["outcomes_recorded"], 2);
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].role, REFUTER);
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(prompt.contains("\"id\":\"f-1\"") && prompt.contains("\"id\":\"f-2\""));
    assert!(
        !prompt.contains("f-det") && !prompt.contains("w-1"),
        "the refuter was handed a finding that was not offered"
    );
    assert!(prompt.contains("src/foo.rs") && prompt.contains(CRITERIA));
    let recorded = refutations_on_the_timeline(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].refuter, REFUTER);
    let mut ids: Vec<(&str, &str)> = recorded[0]
        .outcomes
        .iter()
        .map(|entry| (entry.id.as_str(), entry.outcome.as_str()))
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, [("f-1", "corroborated"), ("f-2", "refuted")]);
    let after = next_after(&world);
    assert_ne!(after["reason_code"], "refutation-missing", "{after}");
    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
}

#[test]
fn an_answer_that_names_what_was_not_offered_or_omits_what_was_is_malformed_and_writes_nothing() {
    let wrong: [&[(&str, &str)]; 5] = [
        &[("f-1", "refuted")],
        &[("f-1", "refuted"), ("f-2", "refuted"), ("ghost", "refuted")],
        &[("f-1", "refuted"), ("ghost", "refuted")],
        &[("f-1", "refuted"), ("f-2", "refuted"), ("f-det", "refuted")],
        &[],
    ];
    for entries in wrong {
        let world = refutable();

        let (outcome, launched) = capture(&world, BATCH, outcomes_of(entries));

        assert_eq!(
            refused_as(&outcome),
            "reviewer-result-malformed",
            "{entries:?}"
        );
        assert_eq!(launched.len(), 1);
        assert!(
            refutations_on_the_timeline(&world).is_empty(),
            "{entries:?}"
        );
        assert_eq!(next_after(&world)["reason_code"], "refutation-missing");
    }
}

#[test]
fn a_lost_claim_refuses_a_refutation_batch_under_its_own_code_before_the_refuter_runs() {
    let world = world_of(|receipt, payload| {
        vec![
            published_with_payload(receipt, payload),
            recorded_lens_report_for(receipt, 4, RELIABILITY_LENS, None),
            recorded_finding_on(
                receipt,
                6,
                "f-1",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
        ]
    });

    let (outcome, launched) = capture(&world, BATCH, outcomes_of(&[("f-1", "refuted")]));

    assert_eq!(refused_as(&outcome), "not-current-live-holder");
    assert!(launched.is_empty(), "the refuter ran without a claim");
    assert!(refutations_on_the_timeline(&world).is_empty());
}

#[test]
fn a_batch_offered_for_a_follow_up_settled_finding_too_is_captured_and_the_review_moves_on() {
    let world = world_of(|receipt, payload| {
        vec![
            live_claim("claude-a"),
            published_with_payload(receipt, payload),
            recorded_lens_report_for(receipt, 4, RELIABILITY_LENS, None),
            recorded_finding_on(
                receipt,
                6,
                "f-1",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
            recorded_finding_on(
                receipt,
                7,
                "f-2",
                "severe",
                Some("inferential"),
                Some("pre-existing"),
            ),
        ]
    });
    let before = next_after(&world);
    assert_eq!(before["reason_code"], "refutation-missing", "{before}");
    assert_eq!(
        before["inputs"][0]["findings"],
        serde_json::json!(["f-1", "f-2"])
    );

    let (outcome, launched) = capture(
        &world,
        BATCH,
        outcomes_of(&[("f-1", "corroborated"), ("f-2", "inconclusive")]),
    );

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["outcomes_recorded"], 2);
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(prompt.contains("\"id\":\"f-1\"") && prompt.contains("\"id\":\"f-2\""));
    assert_eq!(refutations_on_the_timeline(&world).len(), 1);
    let after = next_after(&world);
    assert_ne!(after["reason_code"], "refutation-missing", "{after}");
    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
}

#[test]
fn the_refuter_is_not_a_name_the_collect_offers_so_it_is_not_captured_under_it() {
    let world = refutable();

    let (outcome, launched) = capture(&world, REFUTER, outcomes_of(&[("f-1", "refuted")]));

    assert_eq!(refused_as(&outcome), "capture-input-not-offered");
    assert!(launched.is_empty());
}

#[test]
fn a_refuters_own_answer_scoped_rejection_surfaces_as_malformed_from_the_capture() {
    let world = refutable();

    let (outcome, launched) = capture(
        &world,
        BATCH,
        outcomes_of(&[("f-1", "bogus"), ("f-2", "refuted")]),
    );

    assert_eq!(refused_as(&outcome), "reviewer-result-malformed");
    assert_eq!(launched.len(), 1);
    assert!(refutations_on_the_timeline(&world).is_empty());
}

#[test]
fn an_input_that_is_offered_but_not_launchable_is_refused() {
    let status = serde_json::json!({
        "next": {
            "kind": "collect",
            "reason_code": "lens-report-missing",
            "inputs": [{"name": "review-refuter", "capture": "launch_review_input"}],
        }
    });

    let failure = offered_role(&status, "review-refuter").expect_err("offered but not launchable");

    assert_eq!(failure.envelope()["reason"], "capture-input-not-offered");
    assert_eq!(failure.envelope()["name"], "review-refuter");
}

fn record_batch(
    world: &World,
    offered: &[&str],
    answered: &[(&str, &str)],
) -> (Recording, Option<Failure>) {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&world.comments),
    )]);
    let context = named(&world.context);
    let offered: Vec<String> = offered.iter().map(|id| (*id).to_owned()).collect();
    let batch = Batch {
        issue: 42,
        run_id: "claude-a",
        receipt: &world.receipt,
        offered: &offered,
        now: "2026-08-31T00:00:00Z",
    };
    let answer = RoleResult::Outcomes(
        answered
            .iter()
            .map(|(id, outcome)| Outcome {
                id: (*id).to_owned(),
                outcome: (*outcome).to_owned(),
            })
            .collect(),
    );
    let mut held = None;
    let recording = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refuter_answer(&context, &batch, &answer, &mut held)
    });
    (recording, held)
}

fn record_validator(
    world: &World,
    attempt: &str,
    answer: Result<Vec<u8>, Refusal>,
) -> (Recording, Option<Failure>, Option<serde_json::Value>) {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&world.comments),
    )]);
    let context = named(&world.context);
    let check = Check {
        issue: 42,
        run_id: "claude-a",
        receipt: &world.receipt,
        attempt,
        now: "2026-08-31T00:00:00Z",
    };
    let parsed = crate::reviewer::result::parse(
        crate::reviewer::Role::Validator,
        &answer.expect("answer bytes"),
    )
    .expect("the answer parses");
    let mut held = None;
    let mut recorded = None;
    let recording = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_validator_answer(&context, &check, &parsed, &mut held, &mut recorded)
    });
    (recording, held, recorded)
}

struct RefutationCase {
    code: &'static str,
    offered: Vec<&'static str>,
    answered: Vec<(&'static str, &'static str)>,
}

fn every_refutation_answer_scoped_refusal() -> Vec<RefutationCase> {
    let case = |code, offered: &[&'static str], answered: &[(&'static str, &'static str)]| {
        RefutationCase {
            code,
            offered: offered.to_vec(),
            answered: answered.to_vec(),
        }
    };
    vec![
        case("invalid-refutation-batch", &[""], &[("", "refuted")]),
        case("invalid-refutation-outcome", &["f-1"], &[("f-1", "bogus")]),
        case(
            "refutation-batch-duplicate-finding",
            &["f-1"],
            &[("f-1", "refuted"), ("f-1", "refuted")],
        ),
        case(
            "refutation-unknown-finding",
            &["w-1"],
            &[("w-1", "refuted")],
        ),
        case(
            "refutation-includes-deterministic-finding",
            &["f-det"],
            &[("f-det", "refuted")],
        ),
    ]
}

#[test]
fn each_refutation_answer_scoped_refusal_rejects_the_answer_and_writes_nothing() {
    for case in every_refutation_answer_scoped_refusal() {
        let world = refutable();

        let (recording, held) = record_batch(&world, &case.offered, &case.answered);

        assert_eq!(rejected_as(&recording), case.code, "{held:?}");
        assert!(held.is_none(), "{}: {held:?}", case.code);
        assert!(
            refutations_on_the_timeline(&world).is_empty(),
            "{}",
            case.code
        );
    }
}

#[test]
fn the_refutation_answer_scoped_codes_are_exactly_the_ones_measured_and_exist_in_the_recorder() {
    let mut measured: Vec<&str> = every_refutation_answer_scoped_refusal()
        .iter()
        .map(|case| case.code)
        .collect();
    let mut declared: Vec<&str> = REFUTATION_ANSWER_SCOPED.to_vec();
    measured.sort_unstable();
    declared.sort_unstable();
    assert_eq!(measured, declared);
    assert!(
        !declared.contains(&"refutation-batch-incomplete")
            && !declared.contains(&"refutation-batch-already-recorded")
            && !declared.contains(&"reviewer-not-distinct"),
        "a refusal about the attempt was read as one about the answer"
    );

    let source = crate::source_text::code_of(include_str!("../../claim.rs"));
    for code in REFUTATION_ANSWER_SCOPED {
        assert!(
            source.contains(&format!("\"reason\": \"{code}\"")),
            "`{code}` is no longer a reason `record_refutation` raises"
        );
    }
}

#[test]
fn a_batch_the_recorder_refuses_on_the_claim_is_the_attempts_refusal() {
    let world = world_of(|receipt, payload| {
        vec![
            published_with_payload(receipt, payload),
            recorded_finding_on(
                receipt,
                6,
                "f-1",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
        ]
    });

    let (recording, held) = record_batch(&world, &["f-1"], &[("f-1", "refuted")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a lost claim was read as {recording:?}");
    };
    assert_eq!(refusal.code, "not-current-live-holder");
    assert!(held.is_some());
    assert!(refutations_on_the_timeline(&world).is_empty());
}

fn hand_recorded(
    world: &World,
    refuter: &str,
    outcomes: &str,
) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&world.comments),
    )]);
    let context = named(&world.context);
    let operation_id = "7".repeat(32);
    let refutation = refutation_review(&operation_id, refuter, &world.receipt, outcomes);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_refutation(&context, &refutation)
    })
}

#[test]
fn the_capture_credits_the_role_label_while_a_hand_call_naming_the_publisher_is_still_refused() {
    let world = refutable();
    let both = r#"[{"id":"f-1","outcome":"refuted"},{"id":"f-2","outcome":"refuted"}]"#;

    let by_hand = hand_recorded(&world, "claude-a", both).expect_err("the publisher was credited");
    assert_eq!(by_hand.envelope()["reason"], "reviewer-not-distinct");
    assert_eq!(by_hand.envelope()["publisher"], "claude-a");
    assert!(refutations_on_the_timeline(&world).is_empty());

    let (outcome, _) = capture(
        &world,
        BATCH,
        outcomes_of(&[("f-1", "refuted"), ("f-2", "refuted")]),
    );

    assert!(outcome.is_ok(), "{outcome:?}");
    let recorded = refutations_on_the_timeline(&world);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].refuter, REFUTER);
    assert_ne!(recorded[0].refuter, "claude-a");
    assert_eq!(recorded[0].attester, "claude-a");
}

#[test]
fn a_hand_call_naming_the_requester_of_a_handoff_is_still_refused() {
    let world = world_of(|receipt, payload| {
        vec![
            live_claim("claude-a"),
            published_with_payload(receipt, payload),
            handoff_by("codex-b", &"b".repeat(32), &"2".repeat(32), receipt),
            recorded_finding_on(
                receipt,
                6,
                "f-1",
                "severe",
                Some("inferential"),
                Some("introduced"),
            ),
        ]
    });

    let by_hand = hand_recorded(&world, "codex-b", r#"[{"id":"f-1","outcome":"refuted"}]"#)
        .expect_err("a requester was credited");

    assert_eq!(by_hand.envelope()["reason"], "reviewer-not-distinct");
}

#[test]
fn a_batch_the_recorder_finds_short_of_its_population_is_the_attempts_refusal_not_the_answers() {
    let world = refutable();

    let (recording, held) = record_batch(&world, &["f-1"], &[("f-1", "refuted")]);

    let Recording::Refused(refusal) = recording else {
        panic!("a short batch was read as {recording:?}");
    };
    assert_eq!(refusal.code, "refutation-batch-incomplete");
    assert!(held.is_some());
    assert!(refutations_on_the_timeline(&world).is_empty());
}

const VALIDATOR: &str = "review-validator";

fn validator_world(extra: &dyn Fn(&ReviewTransaction) -> Vec<ownership::Comment>) -> World {
    validator_world_with(None, extra)
}

fn validator_world_with(
    range_reason: Option<&str>,
    extra: &dyn Fn(&ReviewTransaction) -> Vec<ownership::Comment>,
) -> World {
    validator_world_over(range_reason, None, extra)
}

#[derive(Clone, Copy)]
enum MovedBase {
    Clean,
    Conflicting,
}

fn integrate_moved_base(repo: &std::path::Path, base: &str, moved: MovedBase) -> String {
    git(repo, &["checkout", "--quiet", "-b", "side", base]);
    let (file, content) = match moved {
        MovedBase::Clean => ("src/other.rs", "fn other() {}\n"),
        MovedBase::Conflicting => ("src/foo.rs", "fn main() { /* other */ }\n"),
    };
    crate::paths::replace_atomically(&repo.join(file), content).expect("the moved base");
    git(repo, &["add", file]);
    git(repo, &["commit", "--quiet", "-m", "main moved"]);
    let new_base = head_of(repo);
    git(repo, &["checkout", "--quiet", "main"]);
    match moved {
        MovedBase::Clean => git(
            repo,
            &["merge", "--quiet", "--no-ff", "-m", "merge", "side"],
        ),
        MovedBase::Conflicting => git(
            repo,
            &["merge", "--quiet", "-s", "ours", "-m", "merge", "side"],
        ),
    }
    new_base
}

fn validator_world_over(
    range_reason: Option<&str>,
    moved_base: Option<MovedBase>,
    extra: &dyn Fn(&ReviewTransaction) -> Vec<ownership::Comment>,
) -> World {
    let (root, context, parent) = medium_change();
    let repo = context.repo_dir.clone();
    let new_base = moved_base.map(|moved| integrate_moved_base(&repo, &parent.base, moved));
    std::io::Write::write_all(
        &mut std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(repo.join("src/bar.rs"))
            .expect("the correction file"),
        b"fn corrected() {}\n",
    )
    .expect("the correction");
    git(&repo, &["add", "src/bar.rs"]);
    git(&repo, &["commit", "--quiet", "-m", "correction"]);
    let child = ReviewTransaction {
        epoch: op(2),
        head: head_of(&repo),
        base: new_base.unwrap_or_else(|| parent.base.clone()),
        digest: "e".repeat(64),
        ..parent
    };
    let mut comments = vec![
        live_claim("claude-a"),
        published(&parent),
        recorded_finding_on(
            &parent,
            11,
            "parent-defect",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
        recorded_finding_on(&parent, 12, "parent-note", "warning", None, None),
        computed_by("codex-b", &parent, "rejected"),
        published_over_parent(
            "claude-a",
            &child,
            &parent,
            "delta-2-1",
            range_reason,
            moved_base.is_some(),
        ),
        recorded_lens_report_for(&child, 3, RELIABILITY_LENS, None),
    ];
    comments.extend(extra(&child));
    forget_scratch_store();
    stored(&context, &comments);
    World {
        _root: root,
        context,
        receipt: child,
        comments,
    }
}

fn validator_owed() -> World {
    validator_world(&|_| Vec::new())
}

fn prior_unavailable(receipt: &ReviewTransaction, n: u32) -> ownership::Comment {
    let operation = op(700 + n);
    let pr = receipt.pr.to_string();
    wrote(
        &n.to_string(),
        crate::transport::markers::render(
            "review-validator-result",
            &[
                ("run-id", "claude-a"),
                ("op-id", &operation),
                ("epoch", &receipt.epoch),
                ("pr", &pr),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("inspection", "unavailable"),
                ("reason", "the trees were unreadable"),
            ],
        )
        .expect("a validator result marker"),
    )
}

fn validation_of(body: serde_json::Value) -> Result<Vec<u8>, Refusal> {
    Ok(body.to_string().into_bytes())
}

fn passing() -> Result<Vec<u8>, Refusal> {
    validation_of(serde_json::json!({
        "inspection": "available",
        "original_criteria": "pass",
        "correction_regression": "pass",
    }))
}

fn failing() -> Result<Vec<u8>, Refusal> {
    validation_of(serde_json::json!({
        "inspection": "available",
        "original_criteria": "fail",
        "correction_regression": "pass",
    }))
}

fn unavailable_answer() -> Result<Vec<u8>, Refusal> {
    validation_of(serde_json::json!({
        "inspection": "unavailable",
        "reason": "the diff was cut short",
    }))
}

fn validator_results(world: &World) -> Vec<crate::transport::markers::Marker> {
    let comments = Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue");
    comments
        .iter()
        .flat_map(|comment| crate::transport::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("review-validator-result"))
        .filter(|marker| marker.get("epoch") == Some(&world.receipt.epoch))
        .collect()
}

fn escalated_receipts(world: &World) -> Vec<crate::transport::markers::Marker> {
    let comments = Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue");
    comments
        .iter()
        .flat_map(|comment| crate::transport::markers::parse(&comment.body))
        .filter(|marker| marker.get("kind").map(String::as_str) == Some("review-receipt"))
        .filter(|marker| marker.get("epoch") == Some(&world.receipt.epoch))
        .filter(|marker| marker.get("terminal-state").map(String::as_str) == Some("escalated"))
        .collect()
}

#[test]
fn capturing_the_owed_validator_launches_it_once_over_the_correction_and_records_one_result() {
    let world = validator_owed();
    let before = next_after(&world);
    assert_eq!(before["reason_code"], "validator-owed", "{before}");

    let (outcome, launched) = capture(&world, VALIDATOR, passing());

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["name"], VALIDATOR);
    assert_eq!(answer["result"]["outcome"], "passed", "{answer}");
    assert_eq!(launched.len(), 1);
    assert_eq!(launched[0].role, VALIDATOR);
    let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
    assert!(
        prompt.contains("diff --git a/src/bar.rs"),
        "the validator was not handed the correction"
    );
    assert!(
        !prompt.contains("diff --git a/src/foo.rs"),
        "the validator was handed the whole change, not the correction"
    );
    assert!(prompt.contains("\"id\":\"parent-defect\""));
    assert!(
        !prompt.contains("parent-note"),
        "the validator was handed a parent finding that is not severe"
    );
    assert_eq!(validator_results(&world).len(), 1);
    let after = next_after(&world);
    assert_ne!(after["reason_code"], "validator-owed", "{after}");
    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
}

#[test]
fn a_validator_answer_that_is_not_a_well_formed_inspection_is_malformed_and_records_nothing() {
    let wrong = [
        serde_json::json!({"inspection": "available", "original_criteria": "maybe", "correction_regression": "pass"}),
        serde_json::json!({"inspection": "available", "original_criteria": "pass"}),
        serde_json::json!({"inspection": "unavailable", "reason": "  "}),
        serde_json::json!({"inspection": "unavailable", "reason": "no trees", "original_criteria": "pass"}),
        serde_json::json!({"inspection": "looked"}),
    ];
    for body in wrong {
        let world = validator_owed();

        let (outcome, launched) = capture(&world, VALIDATOR, validation_of(body.clone()));

        assert_eq!(refused_as(&outcome), "reviewer-result-malformed", "{body}");
        assert_eq!(launched.len(), 1);
        assert!(validator_results(&world).is_empty(), "{body}");
        assert_eq!(next_after(&world)["reason_code"], "validator-owed");
    }
}

fn failing_regression() -> Result<Vec<u8>, Refusal> {
    validation_of(serde_json::json!({
        "inspection": "available",
        "original_criteria": "pass",
        "correction_regression": "fail",
    }))
}

fn computed_over(world: &World) -> serde_json::Value {
    let bin = crate::test_env::scripted_gh();
    let script = script_for(world, lane_read_answers());
    let context = named(&world.context);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        show_computed_verdict(&context, 42, &world.receipt, None, false, NOW)
    })
    .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()))
}

#[test]
fn a_conclusive_fail_with_corrections_remaining_is_recorded_and_stands_rejected() {
    for (answer, cause) in [
        (failing(), "original_criteria_failed"),
        (failing_regression(), "correction_regression_failed"),
    ] {
        let world = validator_owed();

        let (outcome, _) = capture(&world, VALIDATOR, answer);

        let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
        assert_eq!(answer["result"]["outcome"], "rejected", "{answer}");
        assert_eq!(answer["result"]["cause"], cause);
        assert_eq!(validator_results(&world).len(), 1);
        let computed = computed_over(&world);
        assert_eq!(computed["outcome"], "rejected", "{computed}");
        assert_eq!(
            computed["open"],
            serde_json::json!(["validator"]),
            "{computed}"
        );
        assert_eq!(computed["escalated"], serde_json::json!([]), "{computed}");
    }
}

#[test]
fn a_conclusive_fail_at_the_limit_is_recorded_and_escalates() {
    for (answer, cause) in [
        (failing(), "original_criteria_failed"),
        (failing_regression(), "correction_regression_failed"),
    ] {
        let mut world = validator_owed();
        world.context.config = corrections_limit_row("1");

        let (outcome, _) = capture(&world, VALIDATOR, answer);

        let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
        assert_eq!(answer["result"]["outcome"], "escalated", "{answer}");
        assert_eq!(answer["result"]["cause"], cause);
        let computed = computed_over(&world);
        assert_eq!(computed["outcome"], "escalated", "{computed}");
        assert_eq!(computed["escalated"][0]["reason"], cause, "{computed}");
    }
}

#[test]
fn an_unavailable_inspection_is_recorded_under_the_next_attempt_and_the_third_escalates() {
    let world = validator_world(&|receipt| vec![prior_unavailable(receipt, 7)]);

    let (outcome, _) = capture(&world, VALIDATOR, unavailable_answer());

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["result"]["outcome"], "inconclusive", "{answer}");
    assert_eq!(answer["result"]["inconclusive_attempts"], 2);
    assert_eq!(validator_results(&world).len(), 2);
    assert_eq!(next_after(&world)["reason_code"], "validator-owed");

    let world = validator_world(&|receipt| {
        vec![prior_unavailable(receipt, 7), prior_unavailable(receipt, 8)]
    });

    let (outcome, _) = capture(&world, VALIDATOR, unavailable_answer());

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["result"]["outcome"], "escalated", "{answer}");
    assert_eq!(answer["result"]["cause"], "validator_inconclusive");
    assert_eq!(validator_results(&world).len(), 3);
}

#[test]
fn no_validator_message_names_an_operation_id_the_caller_cannot_see() {
    for source in [
        crate::source_text::code_of(include_str!("../capture.rs")),
        crate::source_text::code_of(include_str!("../validator.rs")),
    ] {
        assert!(
            !source.contains("operation id"),
            "a message names an operation id the caller cannot see"
        );
    }
}

#[test]
fn a_correction_whose_recorded_range_carries_upstream_is_escalated_not_offered() {
    let world = validator_world_with(Some("history-rewritten"), &|_| Vec::new());

    let after = next_after(&world);

    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
    assert_eq!(after["reason_code"], "verdict-computable", "{after}");
}

#[test]
fn the_validator_is_refused_when_the_correction_diff_carries_upstream() {
    let world = validator_world_with(Some("history-rewritten"), &|_| Vec::new());
    let bin = crate::test_env::scripted_gh();
    let script = script_for(&world, lane_read_answers());
    let context = named(&world.context);
    let status = status_offering(&world.receipt, VALIDATOR);
    let refused = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        prepare_with(&context, &input_for(VALIDATOR, "claude-a"), &status)
            .err()
            .map(|failure| failure.envelope())
    })
    .expect("the validator was prepared over upstream work");

    assert_eq!(
        refused["reason"], "validator-delta-carries-upstream",
        "{refused}"
    );
    assert!(validator_results(&world).is_empty());
}

fn timeline_of(world: &World) -> Vec<ownership::Comment> {
    Ledger::read_timeline_at(
        &store_path().expect("the scratch store"),
        &super::super::status::repo_of(&named(&world.context)).expect("the repository"),
        42,
    )
    .expect("the store holds the issue")
}

#[test]
fn a_correction_that_merged_a_moved_base_is_offered_the_validator_not_escalated() {
    let world = validator_world_over(Some("base-advanced"), Some(MovedBase::Clean), &|_| {
        Vec::new()
    });

    let after = next_after(&world);

    assert_eq!(after["reason_code"], "validator-owed", "{after}");
    assert_eq!(after["inputs"][0]["name"], VALIDATOR, "{after}");
}

#[test]
fn the_validator_of_a_merged_base_reads_the_diff_from_the_integrated_tree_and_approves_on_a_pass() {
    for reason in ["base-advanced", "merged-range"] {
        let world = validator_world_over(Some(reason), Some(MovedBase::Clean), &|_| Vec::new());

        let (outcome, launched) = capture(&world, VALIDATOR, passing());

        let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
        assert_eq!(answer["result"]["outcome"], "passed", "{reason}: {answer}");
        let prompt = String::from_utf8_lossy(&launched[0].prompt).into_owned();
        assert!(prompt.contains("diff --git a/src/bar.rs"), "{reason}");
        assert!(
            !prompt.contains("diff --git a/src/other.rs"),
            "{reason}: the validator read the base's advance"
        );
        assert!(
            !prompt.contains("diff --git a/src/foo.rs"),
            "{reason}: the validator read the parent's change"
        );
        let verdict = computed_verdict(
            &timeline_of(&world),
            &world.receipt,
            RiskLevel::Medium,
            Some(&[RELIABILITY_LENS]),
            false,
        );
        assert_eq!(
            verdict.outcome,
            ComputedOutcome::Approved,
            "{reason}: {verdict:?}"
        );
        assert!(verdict.escalated.is_empty(), "{reason}: {verdict:?}");
    }
}

#[test]
fn a_failing_validator_at_the_limit_escalates_a_correction_that_merged_a_moved_base() {
    let mut world = validator_world_over(Some("base-advanced"), Some(MovedBase::Clean), &|_| {
        Vec::new()
    });
    world.context.config = corrections_limit_row("1");

    let (outcome, _) = capture(&world, VALIDATOR, failing());

    let answer = outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(answer["result"]["outcome"], "escalated", "{answer}");
    assert_eq!(answer["result"]["cause"], "original_criteria_failed");
}

#[test]
fn a_merge_that_does_not_integrate_cleanly_is_refused_whatever_the_marker_says() {
    let world = validator_world_over(Some("base-advanced"), Some(MovedBase::Conflicting), &|_| {
        Vec::new()
    });
    let bin = crate::test_env::scripted_gh();
    let script = script_for(&world, lane_read_answers());
    let context = named(&world.context);
    let status = status_offering(&world.receipt, VALIDATOR);
    let refused = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        prepare_with(&context, &input_for(VALIDATOR, "claude-a"), &status)
            .err()
            .map(|failure| failure.envelope())
    })
    .expect("the validator was prepared over an unintegrated merge");

    assert_eq!(
        refused["reason"], "validator-delta-carries-upstream",
        "{refused}"
    );
}

#[test]
fn a_rewritten_history_is_escalated_even_when_the_marker_says_integrated() {
    let world = validator_world_over(Some("history-rewritten"), Some(MovedBase::Clean), &|_| {
        Vec::new()
    });

    let after = next_after(&world);

    assert_eq!(after["operation"], "record_computed_verdict", "{after}");
    let verdict = computed_verdict(
        &timeline_of(&world),
        &world.receipt,
        RiskLevel::Medium,
        Some(&[RELIABILITY_LENS]),
        false,
    );
    assert_eq!(verdict.outcome, ComputedOutcome::Escalated, "{verdict:?}");
    assert_eq!(
        verdict.escalated[0].reason,
        "validator-delta-carries-upstream"
    );
}

#[test]
fn the_inspection_of_a_merged_base_correction_starts_from_the_integrated_tree() {
    let world = validator_world_over(Some("base-advanced"), Some(MovedBase::Clean), &|_| {
        Vec::new()
    });
    let bin = crate::test_env::scripted_gh();
    let script = script_for(&world, lane_read_answers());
    let context = named(&world.context);
    let inspected = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        inspect_candidate(
            &context,
            42,
            &InspectRequest {
                epoch: &world.receipt.epoch,
                request: "name-status",
                path: None,
                side: None,
                worktree: None,
            },
            NOW,
        )
    })
    .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    assert_eq!(inspected["text"], "A\tsrc/bar.rs\n", "{inspected}");
}

#[test]
fn a_second_recording_of_the_same_validator_answer_replays_the_recorded_result() {
    let mut world = validator_world(&|_| Vec::new());
    world.context.config = corrections_limit_row("1");

    let (first_recording, first_held, first_recorded) = record_validator(&world, "1", failing());
    assert_eq!(first_recording, Recording::Recorded);
    assert!(first_held.is_none(), "{first_held:?}");
    assert_eq!(
        first_recorded.expect("a result")["reused_existing_result"],
        false
    );
    assert_eq!(validator_results(&world).len(), 1);
    assert_eq!(
        escalated_receipts(&world).len(),
        1,
        "the fail did not settle"
    );

    let (second_recording, second_held, second_recorded) = record_validator(&world, "1", failing());
    assert_eq!(second_recording, Recording::Recorded);
    assert!(second_held.is_none(), "{second_held:?}");
    assert_eq!(
        second_recorded.expect("a result")["reused_existing_result"],
        true
    );
    assert_eq!(
        validator_results(&world).len(),
        1,
        "the result was recorded twice"
    );
    assert_eq!(
        escalated_receipts(&world).len(),
        1,
        "the replay settled a second escalation"
    );
}

#[test]
fn a_lost_claim_refuses_the_validator_before_it_runs() {
    let world = {
        let held = validator_owed();
        let comments: Vec<ownership::Comment> = held.comments.iter().skip(1).cloned().collect();
        forget_scratch_store();
        stored(&held.context, &comments);
        World { comments, ..held }
    };

    let (outcome, launched) = capture(&world, VALIDATOR, passing());

    assert_eq!(refused_as(&outcome), "not-current-live-holder");
    assert!(launched.is_empty(), "the validator ran without a claim");
    assert!(validator_results(&world).is_empty());
}

#[test]
fn the_validator_is_not_captured_when_the_collect_does_not_offer_it() {
    let world = capturable();

    let (outcome, launched) = capture(&world, VALIDATOR, passing());

    assert_eq!(refused_as(&outcome), "capture-input-not-offered");
    assert!(launched.is_empty());
}

fn record_validation(
    world: &World,
    validation: Validation,
) -> (Recording, Option<Failure>, Option<serde_json::Value>) {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&world.comments),
    )]);
    let context = named(&world.context);
    let check = Check {
        issue: 42,
        run_id: "claude-a",
        receipt: &world.receipt,
        attempt: "1",
        now: "2026-08-31T00:00:00Z",
    };
    let (mut held, mut recorded) = (None, None);
    let recording = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_validator_answer(
            &context,
            &check,
            &RoleResult::Validation(validation),
            &mut held,
            &mut recorded,
        )
    });
    (recording, held, recorded)
}

#[test]
fn a_shape_the_recorder_refuses_rejects_the_answer_with_nothing_held_and_nothing_written() {
    let world = validator_owed();

    let (recording, held, recorded) = record_validation(
        &world,
        Validation::Unavailable {
            reason: "  ".to_owned(),
        },
    );

    assert_eq!(rejected_as(&recording), "invalid-validator-result");
    assert!(held.is_none(), "{held:?}");
    assert!(recorded.is_none());
    assert!(validator_results(&world).is_empty());
}

#[test]
fn an_inspected_answer_is_recorded_whatever_it_concludes() {
    for (original, regression) in [
        (Criterion::Pass, Criterion::Pass),
        (Criterion::Fail, Criterion::Pass),
        (Criterion::Pass, Criterion::Fail),
    ] {
        let world = validator_owed();

        let (recording, held, recorded) = record_validation(
            &world,
            Validation::Available {
                original_criteria: original,
                correction_regression: regression,
            },
        );

        assert_eq!(recording, Recording::Recorded, "{held:?}");
        assert!(recorded.is_some());
        assert_eq!(validator_results(&world).len(), 1);
    }
}

fn collect_counting(
    world: &World,
    name: &str,
    failing: Option<u32>,
    counts: &std::path::Path,
) -> Result<serde_json::Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let mut extra = Vec::new();
    if let Some(nth) = failing {
        extra.push(serde_json::json!({
            "matches": "per_page",
            "nth": nth,
            "status": 1,
            "stderr": "the tracker went away",
        }));
    }
    extra.extend(lane_read_answers());
    extra.push(serde_json::json!({"matches": "per_page", "stdout": "[]", "status": 0}));
    let script = script_for(world, extra);
    let context = named(&world.context);
    crate::test_env::with_counting_gh(bin.path(), &script, counts, || {
        collect_with(&context, &input_for(name, "claude-a"), LAUNCHED_AT, &|_| {})
    })
}

fn launched_and_done(world: &World) -> serde_json::Value {
    let (launched, _) = launch(
        world,
        RELIABILITY_LENS,
        answer_of(&[answered_finding("a-comment-hides-the-body", "src/foo.rs:1")]),
    );
    let launched = launched.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    wait_until_done(&launched);
    launched
}

#[test]
fn a_count_read_that_fails_after_the_findings_landed_reports_the_write() {
    let world = capturable();
    launched_and_done(&world);
    let dir = tempfile::tempdir().expect("a directory for the counter");

    let outcome = collect_counting(
        &world,
        RELIABILITY_LENS,
        Some(10),
        &dir.path().join("count"),
    );

    assert_eq!(
        refused_as(&outcome),
        "write-failed",
        "the finding and its report landed before the count read failed: {outcome:?}"
    );
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!(findings.len(), 1);
    assert_eq!(reports, [RELIABILITY_LENS]);
}

#[test]
fn a_read_that_fails_before_any_write_is_not_reported_as_a_write() {
    let world = capturable();
    let seen = Seen::default();
    let bin = crate::test_env::scripted_gh();
    let mut extra = vec![serde_json::json!({
        "matches": "per_page",
        "nth": 1,
        "status": 1,
        "stderr": "the tracker is unreachable",
    })];
    extra.extend(lane_read_answers());
    extra.push(serde_json::json!({"matches": "per_page", "stdout": "[]", "status": 0}));
    let script = script_for(&world, extra);
    let context = named(&world.context);
    let dir = tempfile::tempdir().expect("a directory for the counter");
    let spawner = answering(answer_of(&[]), &seen);

    let outcome =
        crate::test_env::with_counting_gh(bin.path(), &script, &dir.path().join("count"), || {
            launch_with(
                &context,
                &input_for(RELIABILITY_LENS, "claude-a"),
                &spawner,
                LAUNCHED_AT,
            )
        });

    assert_eq!(
        refused_as(&outcome),
        "read-failed",
        "nothing was written, so the caller must not be told a write may have landed: {outcome:?}"
    );
    assert!(
        seen.lock().expect("the invocations").is_empty(),
        "the publication lookup is read before the reviewer runs"
    );
    nothing_was_written(&world);
}

#[test]
fn a_launch_opens_the_ledger_once_and_asks_github_no_more_than_it_must() {
    let world = capturable();
    let seen = Seen::default();
    let spawner = answering(answer_of(&[]), &seen);
    let bin = crate::test_env::scripted_gh();
    let script = script_for(&world, lane_read_answers());
    let context = named(&world.context);
    let dir = tempfile::tempdir().expect("a log directory");
    let log = dir.path().join("gh.log");

    let outcome = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        launch_with(
            &context,
            &input_for(RELIABILITY_LENS, "claude-a"),
            &spawner,
            LAUNCHED_AT,
        )
    });

    outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let calls = std::fs::read_to_string(&log).expect("the log");
    let invocations = calls.lines().count();
    assert!(
        invocations <= 4,
        "a launch asked GitHub {invocations} times:\n{calls}"
    );
}

#[test]
fn a_collect_opens_the_ledger_once_and_asks_github_no_more_than_it_must() {
    let world = capturable();
    launched_and_done(&world);
    let bin = crate::test_env::scripted_gh();
    let mut extra = lane_read_answers();
    extra.push(serde_json::json!({"matches": "per_page", "stdout": "[]", "status": 0}));
    let script = script_for(&world, extra);
    let context = named(&world.context);
    let dir = tempfile::tempdir().expect("a log directory");
    let log = dir.path().join("gh.log");

    let outcome = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        collect_with(
            &context,
            &input_for(RELIABILITY_LENS, "claude-a"),
            LAUNCHED_AT,
            &|_| {},
        )
    });

    outcome.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let calls = std::fs::read_to_string(&log).expect("the log");
    let invocations = calls.lines().count();
    assert!(
        invocations <= 21,
        "a collect asked GitHub {invocations} times:\n{calls}"
    );
}

#[test]
fn a_control_comment_posted_before_the_collect_is_still_seen() {
    let world = capturable();
    launched_and_done(&world);
    let adjudication = serde_json::json!({
        "id": 9001,
        "node_id": "IC_adjudication",
        "user": {"login": "fixture"},
        "created_at": "2026-08-31T00:00:00Z",
        "updated_at": "2026-08-31T00:00:00Z",
        "body": crate::transport::markers::render(
            "adjudication",
            &[("run-id", "claude-b"), ("target", "claude-a")],
        )
        .expect("an adjudication marker"),
    });
    let bin = crate::test_env::scripted_gh();
    let mut extra = vec![
        serde_json::json!({
            "matches": "issues/42/comments",
            "stdout": serde_json::Value::Array(vec![adjudication]).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "api user",
            "stdout": "{\"login\":\"fixture\"}",
            "status": 0,
        }),
    ];
    extra.extend(lane_read_answers());
    let script = script_for(&world, extra);
    let context = named(&world.context);

    let outcome = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        collect_with(
            &context,
            &input_for(RELIABILITY_LENS, "claude-a"),
            LAUNCHED_AT,
            &|_| {},
        )
    });

    assert_eq!(refused_as(&outcome), "control-message", "{outcome:?}");
    nothing_was_written(&world);
}

struct Held {
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl Adapter for Held {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        let _ = self.release.lock().expect("the gate").recv();
        answer_of(&[])
    }
}

fn held() -> (impl Spawner + use<>, std::sync::mpsc::Sender<()>) {
    let (release, gate) = std::sync::mpsc::channel();
    let gate = Arc::new(Held {
        release: Mutex::new(gate),
    });
    let spawner = OnThread(move || Ok(Box::new(Shared(Arc::clone(&gate))) as Box<dyn Adapter>));
    (spawner, release)
}

struct Shared(Arc<Held>);

impl Adapter for Shared {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        self.0.review(invocation)
    }
}

struct Dies;

impl Spawner for Dies {
    fn start(&self, dir: &std::path::Path) -> Result<u32, Failure> {
        crate::paths::replace_atomically(&dir.join("lock"), "").expect("a lock file");
        crate::paths::replace_atomically(&dir.join("started"), "").expect("a started mark");
        Ok(4242)
    }
}

fn launched(world: &World, name: &str, answer: Result<Vec<u8>, Refusal>) -> serde_json::Value {
    let (launched, _) = launch(world, name, answer);
    launched.unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()))
}

#[test]
fn a_launch_answers_the_job_and_its_deadline_and_holds_the_slot_until_it_is_collected() {
    let world = capturable();
    let (spawner, release) = held();

    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    assert_eq!(first["ok"], true);
    assert_eq!(first["deadline_at"], LAUNCHED_AT + 15 * 60);
    assert!(first["job"].as_str().is_some_and(|job| !job.is_empty()));
    nothing_was_written(&world);
    let (second, _) = launch(&world, RELIABILITY_LENS, answer_of(&[]));
    let refused = second.expect_err("the slot is taken");
    assert_eq!(refused.envelope()["reason"], "capture-already-running");
    assert_eq!(refused.envelope()["job"], first["job"]);
    assert_eq!(refused.envelope()["deadline_at"], first["deadline_at"]);
    release.send(()).expect("the reviewer is waiting");
    wait_until_done(&first);
}

#[test]
fn collecting_before_the_reviewer_has_finished_is_refused_and_writes_nothing() {
    let world = capturable();
    let (spawner, release) = held();
    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let early = collect(&world, RELIABILITY_LENS).expect_err("not finished");

    assert_eq!(early.envelope()["reason"], "capture-running");
    assert_eq!(early.envelope()["deadline_at"], first["deadline_at"]);
    let action = early.envelope()["action"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        action.contains("review_status") && !action.contains("collect_review_input"),
        "{action}"
    );
    assert_eq!(job_state(&world, &first), "running");
    nothing_was_written(&world);
    release.send(()).expect("the reviewer is waiting");
    wait_until_done(&first);
}

#[test]
fn collecting_past_the_deadline_and_its_grace_kills_the_group_and_abandons_the_job() {
    let world = capturable();
    let (spawner, release) = held();
    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let killed = std::cell::Cell::new(None);
    let at = |clock| {
        collect_killing(&world, "claude-a", RELIABILITY_LENS, clock, &|pid| {
            killed.set(Some(pid))
        })
    };

    let within = at(LAUNCHED_AT + 15 * 60 + 59);
    assert_eq!(refused_as(&within), "capture-running");
    assert_eq!(killed.get(), None);
    let past = at(LAUNCHED_AT + 15 * 60 + 60);

    assert_eq!(refused_as(&past), "reviewer-deadline");
    assert_eq!(killed.get(), Some(std::process::id()));
    assert_eq!(job_state(&world, &first), "abandoned");
    nothing_was_written(&world);
    release.send(()).expect("the reviewer is waiting");
}

#[test]
fn a_bounded_wait_records_the_answer_once_the_reviewer_finishes() {
    let world = capturable();
    let (spawner, release) = held();
    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        release.send(()).expect("the reviewer is waiting");
    });

    let recorded = collect_waiting(
        &world,
        "claude-a",
        RELIABILITY_LENS,
        LAUNCHED_AT,
        5,
        &|_| {},
    )
    .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    assert_eq!(recorded["findings_recorded"], 0);
    assert_eq!(job_state(&world, &first), "admitted");
    let (findings, reports) = on_the_timeline(&world);
    assert!(findings.is_empty());
    assert_eq!(reports, [RELIABILITY_LENS]);
    releaser.join().expect("the releaser threads");
}

#[test]
fn a_bounded_wait_that_elapses_is_refused_capture_running_and_writes_nothing() {
    let world = capturable();
    let (spawner, release) = held();
    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let elapsed = std::time::Instant::now();
    let refused = collect_waiting(
        &world,
        "claude-a",
        RELIABILITY_LENS,
        LAUNCHED_AT,
        1,
        &|_| {},
    )
    .expect_err("the reviewer outlasts the wait");

    assert_eq!(refused.envelope()["reason"], "capture-running");
    assert_eq!(refused.envelope()["deadline_at"], first["deadline_at"]);
    assert!(
        elapsed.elapsed() >= std::time::Duration::from_secs(1),
        "the call refused without waiting out `wait_seconds`"
    );
    assert_eq!(job_state(&world, &first), "running");
    nothing_was_written(&world);
    release.send(()).expect("the reviewer is waiting");
    wait_until_done(&first);
}

#[test]
fn a_bounded_wait_still_kills_the_group_past_the_deadline_and_its_grace() {
    let world = capturable();
    let (spawner, release) = held();
    let first = launch_as(&world, "claude-a", RELIABILITY_LENS, &spawner)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    let killed = std::cell::Cell::new(None);

    let past = collect_waiting(
        &world,
        "claude-a",
        RELIABILITY_LENS,
        LAUNCHED_AT + 15 * 60 + 60,
        5,
        &|pid| killed.set(Some(pid)),
    );

    assert_eq!(refused_as(&past), "reviewer-deadline");
    assert_eq!(killed.get(), Some(std::process::id()));
    assert_eq!(job_state(&world, &first), "abandoned");
    nothing_was_written(&world);
    release.send(()).expect("the reviewer is waiting");
}

#[test]
fn the_wait_is_capped_at_its_maximum_and_at_the_jobs_own_deadline() {
    assert_eq!(
        patience_for(100_000, LAUNCHED_AT + 100_000, LAUNCHED_AT),
        MAX_WAIT_SECONDS
    );
    assert_eq!(
        patience_for(MAX_WAIT_SECONDS, LAUNCHED_AT + 100, LAUNCHED_AT),
        100
    );
    assert_eq!(patience_for(30, LAUNCHED_AT + 100, LAUNCHED_AT), 30);
    assert_eq!(patience_for(30, LAUNCHED_AT, LAUNCHED_AT), 0);
}

#[test]
fn collecting_the_answer_records_it_once_and_a_second_collect_writes_nothing() {
    let world = capturable();
    let first = launched(
        &world,
        RELIABILITY_LENS,
        answer_of(&[answered_finding("a-comment-hides-the-body", "src/foo.rs:1")]),
    );
    wait_until_done(&first);

    let recorded = collect(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    assert_eq!(recorded["findings_recorded"], 1);
    assert_eq!(job_state(&world, &first), "admitted");
    assert!(!job_dir(&first).join("result.json").exists());
    assert_eq!(next_after(&world)["operation"], "record_computed_verdict");
    let again = collect(&world, RELIABILITY_LENS)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(again["admitted"], true);
    assert_eq!(again["findings_recorded"], 1);
    let (findings, reports) = on_the_timeline(&world);
    assert_eq!((findings.len(), reports.len()), (1, 1));
}

#[test]
fn a_runner_that_died_is_lost_the_job_is_abandoned_and_the_slot_launches_again() {
    let world = capturable();
    let dead = launch_as(&world, "claude-a", RELIABILITY_LENS, &Dies)
        .unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "capture-runner-lost");
    assert_eq!(job_state(&world, &dead), "abandoned");
    let again = launched(&world, RELIABILITY_LENS, answer_of(&[]));
    wait_until_done(&again);
    assert!(collect(&world, RELIABILITY_LENS).is_ok());
}

#[test]
fn only_the_run_that_launched_the_job_collects_it() {
    let world = capturable();
    let first = launched(&world, RELIABILITY_LENS, answer_of(&[]));
    wait_until_done(&first);

    let outcome = collect_as(&world, "claude-b", RELIABILITY_LENS, LAUNCHED_AT);

    assert_eq!(refused_as(&outcome), "capture-job-other-run");
    assert_eq!(job_state(&world, &first), "running");
    nothing_was_written(&world);
}

#[test]
fn collecting_with_no_job_for_the_slot_is_refused() {
    let world = capturable();

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "capture-job-missing");
}

#[test]
fn a_reviewer_refusal_in_the_result_passes_through_under_its_own_code() {
    let world = capturable();
    let first = launched(
        &world,
        RELIABILITY_LENS,
        Err(ReviewerCode::Unavailable.refusal("out of credit")),
    );
    wait_until_done(&first);

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "reviewer-unavailable");
    assert_eq!(job_state(&world, &first), "abandoned");
    nothing_was_written(&world);
}

#[test]
fn a_malformed_answer_abandons_the_job_and_records_nothing() {
    let world = capturable();
    let first = launched(&world, RELIABILITY_LENS, Ok(b"not json".to_vec()));
    wait_until_done(&first);

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "reviewer-result-malformed");
    assert_eq!(job_state(&world, &first), "abandoned");
    nothing_was_written(&world);
}

#[test]
fn a_validator_admitted_again_after_a_crash_records_one_result_not_two() {
    let world = validator_owed();
    let first = launched(&world, VALIDATOR, unavailable_answer());
    wait_until_done(&first);
    let result = std::fs::read(job_dir(&first).join("result.json")).expect("a result");
    let ledger = open_ledger(&world);
    let id = first["job"].as_str().expect("a job").to_owned();
    assert!(
        ledger
            .transact(|tx| tx.compare_and_set_state(&id, "running", "admitting"))
            .expect("a swap")
    );

    collect(&world, VALIDATOR).unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));
    assert_eq!(validator_results(&world).len(), 1);
    crate::paths::replace_bytes_atomically(&job_dir(&first).join("result.json"), &result)
        .expect("the result is back");
    assert!(
        ledger
            .transact(|tx| tx.compare_and_set_state(&id, "admitted", "admitting"))
            .expect("a swap")
    );

    collect(&world, VALIDATOR).unwrap_or_else(|failure| panic!("refused: {}", failure.envelope()));

    assert_eq!(validator_results(&world).len(), 1);
    assert_eq!(job_state(&world, &first), "admitted");
}

#[test]
fn a_finished_job_of_another_run_gives_up_its_slot_to_a_new_launch() {
    let world = capturable();
    let first = launched(&world, RELIABILITY_LENS, answer_of(&[]));
    wait_until_done(&first);
    let ledger = open_ledger(&world);
    let receipt = Receipt {
        epoch: world.receipt.epoch.clone(),
        pr: world.receipt.pr,
        head: world.receipt.head.clone(),
        base: world.receipt.base.clone(),
        digest: world.receipt.digest.clone(),
        payload_digest: String::new(),
    };
    let offered = serde_json::json!([]);
    let new = NewJob {
        id: "a-second-job",
        run_id: "claude-b",
        name: RELIABILITY_LENS,
        receipt: &receipt,
        offered: &offered,
        attempt: None,
        started_at: LAUNCHED_AT,
        deadline_at: LAUNCHED_AT,
        dir: "unused",
    };

    assert!(
        take_slot(&ledger, &new, "claude-b")
            .expect("a slot")
            .is_none()
    );
    assert_eq!(job_state(&world, &first), "abandoned");
}

#[test]
fn a_job_recorded_against_other_bytes_than_the_offered_receipt_is_stale_even_in_the_same_epoch() {
    let world = capturable();
    let ledger = open_ledger(&world);
    let receipt = Receipt {
        epoch: world.receipt.epoch.clone(),
        pr: world.receipt.pr,
        head: "f".repeat(40),
        base: world.receipt.base.clone(),
        digest: world.receipt.digest.clone(),
        payload_digest: String::new(),
    };
    let offered = serde_json::json!([]);
    let new = NewJob {
        id: "other-bytes",
        run_id: "claude-a",
        name: RELIABILITY_LENS,
        receipt: &receipt,
        offered: &offered,
        attempt: None,
        started_at: LAUNCHED_AT,
        deadline_at: LAUNCHED_AT,
        dir: "unused",
    };
    assert!(
        take_slot(&ledger, &new, "claude-a")
            .expect("a slot")
            .is_none()
    );

    let outcome = collect(&world, RELIABILITY_LENS);

    assert_eq!(refused_as(&outcome), "capture-result-stale");
    nothing_was_written(&world);
}
