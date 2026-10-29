use super::*;
use crate::harness::refusal_class::class_of_answer;
use crate::outcome::RefusalClass;
use crate::transport::claim::status::released_continuation;
use serde_json::{Value, json};

fn pull(receipt: &ReviewTransaction, state: &str, draft: bool) -> Value {
    json!({
        "matches": "pr view",
        "stdout": json!({
            "number": receipt.pr,
            "url": "https://github.com/o/r/pull/7",
            "headRefOid": receipt.head,
            "baseRefOid": receipt.base,
            "state": state,
            "isDraft": draft,
        })
        .to_string(),
        "status": 0,
    })
}

pub(super) fn open(receipt: &ReviewTransaction) -> Value {
    pull(receipt, "OPEN", true)
}

fn lane_green() -> Vec<Value> {
    lane_read_answers()
}

pub(super) fn stored(context: &Context, comments: &[ownership::Comment]) {
    crate::transport::store::forget_scratch_store();
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(comments),
    )]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        Ledger::open(context, 42, RECLAIM_NOW).expect("the first touch imports the issue")
    });
}

pub(super) fn named(context: &Context) -> Context {
    Context {
        repo: Some("fixture/repo".to_owned()),
        ..context.clone()
    }
}

fn status_over(
    context: &Context,
    comments: &[ownership::Comment],
    answers: &[Value],
) -> Result<Value, Failure> {
    stored(context, comments);
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(answers), || {
        review_status(&named(context), 42)
    })
}

fn next_of(
    context: &Context,
    comments: &[ownership::Comment],
    answers: &[Value],
) -> (Value, Value) {
    let answer = status_over(context, comments, answers)
        .unwrap_or_else(|failure| panic!("status refused: {}", failure.envelope()));
    assert_eq!(answer["ok"], true, "{answer}");
    (answer["next"].clone(), answer)
}

fn binding_of(receipt: &ReviewTransaction) -> Value {
    json!({
        "issue": 42,
        "epoch": receipt.epoch,
        "pr": receipt.pr,
        "head": receipt.head,
        "base": receipt.base,
        "digest": receipt.digest,
    })
}

fn class_of_failure(failure: &Failure) -> RefusalClass {
    let envelope = failure.envelope();
    class_of_answer(
        failure.code(),
        envelope["reason"].as_str().unwrap_or_default(),
    )
}

fn recovery_of(receipt: &ReviewTransaction) -> ownership::Comment {
    wrote(
        "7",
        super::super::super::markers::render(
            "review-recovery",
            &[
                ("run-id", "person"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("actor", "maintainer"),
                ("permission", "admin"),
                ("reason", "retry"),
                ("level", "low"),
                ("focus", "reliability"),
            ],
        )
        .expect("a recovery marker"),
    )
}

fn marked(kind: &str, receipt: &ReviewTransaction, extra: &[(&str, &str)]) -> ownership::Comment {
    let pr = receipt.pr.to_string();
    let mut fields = vec![
        ("run-id", "claude-a"),
        ("epoch", receipt.epoch.as_str()),
        ("pr", pr.as_str()),
        ("head", receipt.head.as_str()),
        ("base", receipt.base.as_str()),
        ("digest", receipt.digest.as_str()),
    ];
    fields.extend_from_slice(extra);
    let marker = super::super::super::markers::render(kind, &fields).expect("a marker");
    wrote("8", marker)
}

pub(super) fn refutation_of(receipt: &ReviewTransaction, outcomes: &str) -> ownership::Comment {
    let operation = op(500);
    let fields = [
        ("refuter", "codex-c"),
        ("op-id", &operation),
        ("outcomes", outcomes),
    ];
    marked("review-refutation", receipt, &fields)
}

#[test]
fn an_issue_with_no_publication_is_told_to_publish_expecting_none() {
    let (_root, context, _receipt) = low_change();

    let (next, answer) = next_of(&context, &[live_claim("claude-a")], &[]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "publish_review");
    assert_eq!(next["reason_code"], "no-publication");
    assert_eq!(
        next["arguments"],
        json!({"issue": 42, "expected_epoch": "none"})
    );
    assert_eq!(answer["receipt"], Value::Null);
    assert_eq!(answer["forecast"]["horizon"], "partial");
}

#[test]
fn a_missing_lens_is_collected_with_the_tool_that_launches_and_records_it() {
    let (_root, context, receipt) = medium_change();
    let comments = [live_claim("claude-a"), published(&receipt)];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "collect", "{answer}");
    assert_eq!(next["reason_code"], "lens-report-missing");
    assert_eq!(next["arguments"], binding_of(&receipt));
    assert_eq!(
        next["inputs"],
        json!([{"name": RELIABILITY_LENS, "capture": "launch_review_input"}])
    );
    assert_eq!(answer["receipt"]["epoch"], receipt.epoch);
}

#[test]
fn an_owed_validator_is_collected_for_launch_review_input() {
    let (_root, context, receipt) = medium_change();
    let parent = ReviewTransaction {
        epoch: op(2),
        head: "d".repeat(40),
        digest: "e".repeat(64),
        ..receipt.clone()
    };
    let comments = [
        live_claim("claude-a"),
        published(&parent),
        computed_by("codex-b", &parent, "rejected"),
        published_with_parent("claude-a", &receipt, &parent, "delta-2-1"),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "collect", "{answer}");
    assert_eq!(next["reason_code"], "validator-owed");
    assert_eq!(
        next["inputs"],
        json!([{"name": "review-validator", "capture": "launch_review_input"}])
    );
}

#[test]
fn a_severe_inferential_finding_with_no_refuter_is_collected_not_escalated() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("inferential"),
            Some("introduced"),
        ),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "collect", "{answer}");
    assert_eq!(next["reason_code"], "refutation-missing");
    assert_eq!(
        next["inputs"],
        json!([{"name": "refutation-batch", "capture": "launch_review_input", "findings": ["f-1"]}])
    );
}

#[test]
fn an_escalation_the_ledger_cannot_settle_by_refutation_is_recorded_as_the_verdict() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(&receipt, 6, "f-1", "severe", Some("insufficient"), None),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "record_computed_verdict");
}

#[test]
fn a_computable_verdict_is_recorded_with_the_receipt_after_the_lane_is_read() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());

    let (next, answer) = next_of(&context, &comments, &answers);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "record_computed_verdict");
    assert_eq!(next["reason_code"], "verdict-computable");
    assert_eq!(next["arguments"], binding_of(&receipt));
}

#[test]
fn a_computable_rejection_is_recorded_without_reading_the_lane() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["operation"], "record_computed_verdict", "{answer}");
}

#[test]
fn a_red_lane_over_a_computable_approval_is_a_coded_stop() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());
    answers[2] = json!({
        "matches": "check-runs",
        "stdout": check_runs(&[check_run("ci", "completed", Some("cancelled"))]).to_string(),
        "status": 0,
    });

    let (next, answer) = next_of(&context, &comments, &answers);

    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "publication-lane-red");
    assert_eq!(answer["forecast"]["horizon"], "terminal");
}

fn red_lane_answers() -> Vec<Value> {
    let mut answers = vec![open(&low_change().2)];
    answers.extend(lane_green());
    answers[2] = json!({
        "matches": "check-runs",
        "stdout": check_runs(&[check_run("ci", "completed", Some("failure"))]).to_string(),
        "status": 0,
    });
    answers
}

fn status_again(context: &Context, answers: &[Value]) -> Value {
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(answers), || {
        review_status(&named(context), 42)
    })
    .unwrap_or_else(|failure| panic!("status refused: {}", failure.envelope()))["next"]
        .clone()
}

#[test]
fn a_red_lane_offers_one_rerun_of_the_failed_jobs_then_asks_for_a_correction() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let mut answers = vec![open(&receipt)];
    answers.extend(red_lane_answers().into_iter().skip(1));

    let (first, _) = next_of(&context, &comments, &answers);

    assert_eq!(first["reason_code"], "publication-lane-red");
    assert_eq!(first["rerun"]["run_id"], "31865504912");
    assert_eq!(
        first["rerun"]["command"],
        "gh run rerun 31865504912 --failed"
    );
    let second = status_again(&context, &answers);
    assert_eq!(second["reason_code"], "publication-lane-red");
    assert!(second.get("rerun").is_none(), "{second}");
    assert!(
        second["continuation"]
            .as_str()
            .unwrap()
            .contains("needs a correction"),
        "{second}"
    );
}

#[test]
fn an_approved_receipt_is_released_to_ci_with_the_receipt() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "release_ci");
    assert_eq!(next["arguments"], binding_of(&receipt));
}

#[test]
fn an_approved_receipt_whose_ledger_moved_since_is_told_to_publish_again_every_time() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];

    for _ in 0..2 {
        let (next, _) = next_of(&context, &comments, &[open(&receipt)]);
        assert_eq!(next["reason_code"], "verdict-stale");
        assert_eq!(next["arguments"]["expected_epoch"], receipt.epoch);
    }
}

#[test]
fn a_refutation_batch_already_recorded_is_never_collected_again() {
    let (_root, context, receipt) = low_change();
    let inferential = |n, id| {
        recorded_finding_on(
            &receipt,
            n,
            id,
            "severe",
            Some("inferential"),
            Some("introduced"),
        )
    };
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        inferential(6, "f-1"),
        refutation_of(&receipt, r#"[{"id":"f-1","outcome":"corroborated"}]"#),
        inferential(7, "f-2"),
    ];
    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);
    assert_ne!(next["reason_code"], "refutation-missing", "{answer}");
    assert_eq!(next["operation"], "record_computed_verdict");
}

#[test]
fn a_configured_repository_that_does_not_parse_is_unavailable_not_replaced_by_origin() {
    let (_root, context, _receipt) = low_change();
    let bad = Context {
        repo: Some("not a repo".to_owned()),
        ..context
    };
    stored(&bad, &[live_claim("claude-a")]);
    let failure = review_status(&bad, 42).expect_err("a bad configured repository was replaced");
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn a_pull_request_answering_no_state_is_unavailable() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let stateless = json!({"matches": "pr view", "stdout": "{}", "status": 0});
    let failure = status_over(&context, &comments, &[stateless]).expect_err("answered");
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn an_approved_receipt_already_released_stops() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let (next, _) = next_of(&context, &comments, &[pull(&receipt, "OPEN", false)]);

    assert_eq!(next["kind"], "stop");
    assert_eq!(next["reason_code"], "released");
}

#[test]
fn a_rejected_receipt_with_its_correction_unspent_is_told_to_publish_expecting_its_epoch() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "rejected"),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "publish_review");
    assert_eq!(next["reason_code"], "correction-due");
    assert_eq!(
        next["arguments"],
        json!({"issue": 42, "expected_epoch": receipt.epoch})
    );
}

#[test]
fn a_rejected_correction_with_corrections_remaining_is_due_another_not_an_escalation() {
    let (_root, context) = context_with_no_repository();
    let (receipt, mut comments) = review_world_with_spent_correction();
    comments.push(computed_by("codex-b", &receipt, "rejected"));

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["reason_code"], "correction-due", "{answer}");
}

fn failed_validator_result(
    receipt: &ReviewTransaction,
    original: &str,
    regression: &str,
) -> ownership::Comment {
    let pr = receipt.pr.to_string();
    wrote(
        "8",
        crate::transport::markers::render(
            "review-validator-result",
            &[
                ("run-id", "claude-a"),
                ("op-id", &op(800)),
                ("epoch", &receipt.epoch),
                ("pr", &pr),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("inspection", "available"),
                ("original", original),
                ("regression", regression),
            ],
        )
        .expect("a validator result marker"),
    )
}

#[test]
fn a_correction_the_validator_failed_is_due_another_and_names_what_failed() {
    for (original, regression, phrase) in [
        ("fail", "pass", "did not fix the parent's severe findings"),
        ("pass", "fail", "broke something on the paths it touched"),
    ] {
        let (_root, context) = context_with_no_repository();
        let (receipt, mut comments) = review_world_with_spent_correction();
        comments.push(failed_validator_result(&receipt, original, regression));
        comments.push(computed_by("codex-b", &receipt, "rejected"));

        let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

        assert_eq!(next["reason_code"], "correction-due", "{answer}");
        let why = next["why"].as_str().expect("a plain reason");
        assert!(why.contains(phrase), "{why}");
        assert!(why.contains("next correction"), "{why}");
    }
}

#[test]
fn a_rejected_receipt_with_its_correction_spent_is_told_to_escalate() {
    let (_root, mut context) = context_with_no_repository();
    context.config = corrections_limit_row("1");
    let (receipt, mut comments) = review_world_with_spent_correction();
    comments.push(computed_by("codex-b", &receipt, "rejected"));

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "escalate_review");
    assert_eq!(next["reason_code"], "correction-spent");
    assert_eq!(next["arguments"], binding_of(&receipt));
    let why = next["why"].as_str().expect("a plain reason");
    assert!(
        why.contains("spent all its corrections") && why.contains("still stands rejected"),
        "{why}"
    );
}

#[test]
fn an_escalated_receipt_stops_with_the_persons_recover_command() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "escalated"),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "escalated");
    assert_eq!(next["operation"], Value::Null);
    let (closed, _) = next_of(&context, &comments, &[pull(&receipt, "CLOSED", false)]);
    assert_eq!(closed["reason_code"], "escalated");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains("estigia recover 42 --reason \"<why>\""),
        "{continuation}"
    );
    assert!(continuation.contains(&format!("--epoch {}", receipt.epoch)));
    let why = next["why"].as_str().expect("a plain reason");
    assert!(why.contains("`f-1`") && why.contains("no refuter"), "{why}");
}

/// The escalation answer is read before the head/base comparison, so a pull
/// request that moved off an unrecovered escalation still answers `escalated`
/// and hands the person's recover command over rather than hiding it behind a
/// `pull-request-moved` whose publication the spent correction would refuse.
/// Mutation: moving the escalation read back inside the head/base `else` turns
/// this red, and the answer is `pull-request-moved`.
#[test]
fn a_pull_request_that_moved_over_an_unrecovered_escalation_answers_escalated() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "escalated"),
    ];
    let moved = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);

    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "escalated");
    assert_eq!(next["operation"], Value::Null);
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains("estigia recover 42 --reason \"<why>\""),
        "{continuation}"
    );
    assert!(continuation.contains(&format!("--epoch {}", receipt.epoch)));
    let why = next["why"].as_str().expect("a plain reason");
    assert!(why.contains("`f-1`") && why.contains("no refuter"), "{why}");
}

#[test]
fn a_recovered_receipt_is_told_to_publish_its_successor() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "escalated"),
        recovery_of(&receipt),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["operation"], "publish_review", "{answer}");
    assert_eq!(next["reason_code"], "recovered");
    assert_eq!(next["arguments"]["expected_epoch"], receipt.epoch);
}

/// The escalation answer is read before the head/base comparison, so a
/// recovered escalation over a moved pull request names its successor
/// publication rather than `pull-request-moved`; both name `publish_review`,
/// and the successor publication is admitted at the moved head.
#[test]
fn a_recovered_escalation_over_a_moved_pull_request_is_answered_recovered() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim(REPUBLISH_RUN),
        published(&first),
        computed_by(REPUBLISH_RUN, &first, "escalated"),
        recovery_of(&first),
    ];
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "publish_review");
    assert_eq!(next["reason_code"], "recovered");
    assert_eq!(next["arguments"]["expected_epoch"], first.epoch);

    let (published, _) = world.publish_expecting(&comments, &moved_head, Some(&first.epoch));

    published.expect("the successor publication the answer named was refused");
}

#[test]
fn a_merged_pull_request_is_delivered() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];

    let (next, answer) = next_of(&context, &comments, &[pull(&receipt, "MERGED", false)]);

    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "delivered");
    assert_eq!(answer["forecast"]["horizon"], "terminal");
}

#[test]
fn a_closed_pull_request_is_published_afresh_expecting_none() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];

    let (next, _) = next_of(&context, &comments, &[pull(&receipt, "CLOSED", false)]);

    assert_eq!(next["operation"], "publish_review");
    assert_eq!(next["arguments"]["expected_epoch"], "none");
}

#[test]
fn a_pull_request_that_moved_off_the_receipt_is_a_coded_stop() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let moved = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt
    };

    let (next, _) = next_of(&context, &comments, &[open(&moved)]);

    assert_eq!(next["kind"], "stop");
    assert_eq!(next["reason_code"], "pull-request-moved");
}

#[test]
fn an_unreadable_store_is_unavailable_and_never_a_state() {
    let (_root, context, _receipt) = low_change();
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("state.db");
    crate::paths::replace_atomically(&path, "this is not a database").expect("writes");

    let failure =
        crate::transport::store::with_store_at(&path, || review_status(&named(&context), 42))
            .expect_err("an unreadable store was answered");

    assert_eq!(
        failure.envelope()["reason"],
        "review-status-store-unreadable"
    );
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn an_issue_the_store_never_held_is_unavailable() {
    let (_root, context, _receipt) = low_change();
    crate::transport::store::forget_scratch_store();

    let failure =
        review_status(&named(&context), 42).expect_err("an unknown issue was answered as empty");

    assert_eq!(
        failure.envelope()["reason"],
        "review-status-issue-not-in-store"
    );
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn an_unreadable_pull_request_is_unavailable() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let unreadable = json!({"matches": "pr view", "stderr": "gh: boom", "status": 1});

    let failure = status_over(&context, &comments, &[unreadable])
        .expect_err("an unreadable pull request was answered");

    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn an_unreadable_lane_is_unavailable() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());
    answers[2] = json!({"matches": "check-runs", "stderr": "gh: boom", "status": 1});

    let failure =
        status_over(&context, &comments, &answers).expect_err("an unreadable lane was answered");

    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn a_status_reads_no_comment_and_no_label() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    stored(&context, &comments);
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());
    let bin = crate::test_env::scripted_gh();
    let directory = tempfile::tempdir().expect("a log directory");
    let log = directory.path().join("gh.log");

    crate::test_env::with_logging_gh(
        bin.path(),
        &crate::test_env::answers(&answers),
        &log,
        || review_status(&named(&context), 42),
    )
    .expect("the status answers");

    let calls = std::fs::read_to_string(&log).expect("the log");
    assert!(calls.contains("pr view"), "{calls}");
    assert!(calls.contains("check-runs"), "{calls}");
    for line in calls.lines() {
        for forbidden in ["issue", "comments", "label"] {
            assert!(!line.contains(forbidden), "{forbidden} read: {line}");
        }
    }
}

#[test]
fn a_command_named_before_a_publication_landed_is_refused_as_stale() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());
    let (next, _) = next_of(&context, &comments, &answers);
    assert_eq!(next["operation"], "record_computed_verdict");
    let named_receipt = ReviewTransaction {
        epoch: next["arguments"]["epoch"]
            .as_str()
            .expect("an epoch")
            .to_owned(),
        pr: next["arguments"]["pr"].as_u64().expect("a pr"),
        head: next["arguments"]["head"]
            .as_str()
            .expect("a head")
            .to_owned(),
        base: next["arguments"]["base"]
            .as_str()
            .expect("a base")
            .to_owned(),
        digest: next["arguments"]["digest"]
            .as_str()
            .expect("a digest")
            .to_owned(),
    };
    let successor = ReviewTransaction {
        epoch: op(2),
        ..receipt.clone()
    };
    let after = [
        live_claim("claude-a"),
        published(&receipt),
        published(&successor),
    ];
    stored(&context, &after);
    let bin = crate::test_env::scripted_gh();
    let script =
        crate::test_env::answers(&[crate::test_env::issue_view_answer(42, stdout_with(&after))]);
    let operation_id = op(9);

    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        record_computed_verdict(&context, &computed_request(&operation_id, &named_receipt))
    })
    .expect_err("a stale answer was executed");

    assert_eq!(
        failure.envelope()["reason"],
        "published-transaction-mismatch"
    );
    assert_eq!(class_of_failure(&failure), RefusalClass::Stale);
}

#[test]
fn a_publication_named_before_another_landed_is_refused_as_stale() {
    let world = republish_world();
    let first_head = world.commit("two.txt", 1);
    let first = world.receipt('1', &first_head);
    let (_root, context) = context_with_no_repository();
    let comments = [
        live_claim("claude-a"),
        published(&first),
        computed_by("codex-b", &first, "rejected"),
    ];
    let (next, _) = next_of(&context, &comments, &[open(&first)]);
    assert_eq!(next["operation"], "publish_review");
    let expected = next["arguments"]["expected_epoch"]
        .as_str()
        .expect("an expected epoch")
        .to_owned();
    let second_head = world.commit("two.txt", 2);
    let second = world.receipt('2', &second_head);
    world.commit("two.txt", 3);

    let (answer, _) = world.publish_expecting(
        &[published(&first), published(&second)],
        &second_head,
        Some(&expected),
    );
    let failure = answer.expect_err("a stale answer was executed");

    assert_eq!(failure.envelope()["reason"], "expected-epoch-moved");
    assert_eq!(class_of_failure(&failure), RefusalClass::Stale);
}

#[test]
fn a_publication_whose_pull_request_was_never_confirmed_is_unavailable_not_a_state() {
    let (_root, context, receipt) = low_change();
    let unconfirmed = wrote(
        "3",
        super::super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("unconfirmed", "true"),
            ],
        )
        .expect("a receipt marker"),
    );

    let failure = status_over(
        &context,
        &[live_claim("claude-a"), unconfirmed],
        &[open(&receipt)],
    )
    .expect_err("an unconfirmed publication was answered as a state");

    assert_eq!(failure.envelope()["reason"], "publication-unconfirmed");
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

/// A duplicate `unconfirmed` publication sharing the epoch of a confirmed,
/// escalated one is refused as unavailable before the escalation answer, rather
/// than answered `escalated` with a recover the gate would refuse. Mutation:
/// reading the escalation before the unconfirmed check answers `escalated` and
/// reddens the assertion.
#[test]
fn an_unconfirmed_duplicate_over_an_escalation_is_refused_not_answered() {
    let (_root, context, receipt) = low_change();
    let duplicate = wrote(
        "3",
        super::super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("unconfirmed", "true"),
            ],
        )
        .expect("a receipt marker"),
    );

    let failure = status_over(
        &context,
        &[
            live_claim("claude-a"),
            published(&receipt),
            computed_by("codex-b", &receipt, "escalated"),
            duplicate,
        ],
        &[open(&receipt)],
    )
    .expect_err("an unconfirmed duplicate was answered as a state");

    assert_eq!(failure.envelope()["reason"], "publication-unconfirmed");
    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

#[test]
fn an_escalation_that_a_refuter_cannot_settle_is_not_collected() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("inferential"),
            Some("introduced"),
        ),
        recorded_finding_on(&receipt, 7, "f-2", "severe", Some("insufficient"), None),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["kind"], "execute", "{answer}");
    assert_eq!(next["operation"], "record_computed_verdict");
}

#[test]
fn a_pull_request_whose_base_moved_off_the_receipt_is_a_coded_stop() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];
    let moved = ReviewTransaction {
        base: "9".repeat(40),
        ..receipt
    };

    let (next, _) = next_of(&context, &comments, &[open(&moved)]);

    assert_eq!(next["reason_code"], "pull-request-moved");
}

#[test]
fn the_repository_is_read_from_the_origin_remote_when_none_is_named() {
    let (_root, context, _receipt) = low_change();
    let unnamed = Context {
        repo: None,
        ..context
    };
    stored(&unnamed, &[live_claim("claude-a")]);

    let answer = review_status(&unnamed, 42).expect("the origin names the repository");

    assert_eq!(answer["next"]["reason_code"], "no-publication");
}

#[test]
fn a_repository_that_cannot_be_named_without_github_is_unavailable() {
    let (_root, context, _receipt) = low_change();
    git(&context.repo_dir, &["remote", "remove", "origin"]);
    let unnamed = Context {
        repo: None,
        ..context
    };
    stored(&unnamed, &[live_claim("claude-a")]);

    let failure = review_status(&unnamed, 42).expect_err("no repository could be named");

    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

fn recorded_over(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Value, Failure> {
    stored(context, comments);
    let bin = crate::test_env::scripted_gh();
    let mut script = vec![crate::test_env::issue_view_answer(
        42,
        stdout_with(comments),
    )];
    script.extend(lane_green());
    let operation_id = op(9);
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&script), || {
        record_computed_verdict(context, &computed_request(&operation_id, receipt))
    })
}

fn record_finding_over(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> (Result<Value, Failure>, Vec<ownership::Comment>) {
    stored(context, comments);
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(comments)),
        json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let operation_id = op(9);
    let finding = FindingReview {
        issue: 42,
        run_id: "claude-a",
        reviewer: "codex-b",
        operation_id: &operation_id,
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        id: "f-1",
        class: "warning",
        evidence: "observed the defect directly",
        impact: "wrong output on the happy path",
        parent: "",
        origin: "",
        evidence_class: "",
        causal_disposition: "",
        location: "",
        lens: "",
        now: "2026-08-31T00:00:00Z",
    };
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let recorded = record_review_finding(context, &finding);
        let timeline = store_timeline(context, 42);
        (recorded, timeline)
    })
}

fn fresh_approved(receipt: &ReviewTransaction) -> Vec<ownership::Comment> {
    vec![
        live_claim("claude-a"),
        published(receipt),
        recorded_lens_report_for(receipt, 3, RELIABILITY_LENS, None),
        stored_computed_verdict(receipt, "medium", "approved", "[]"),
    ]
}

#[test]
fn a_stale_verdict_with_a_finding_after_it_names_a_publication_that_is_admitted() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim("claude-a"),
        published(&first),
        computed_by("codex-b", &first, "approved"),
        recorded_finding_on(
            &first,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];
    let context = context_at(&world.work);
    let (next, answer) = next_of(&context, &comments, &[open(&first)]);
    assert_eq!(next["operation"], "publish_review", "{answer}");
    assert_eq!(next["reason_code"], "verdict-stale");
    let expected = next["arguments"]["expected_epoch"].as_str().expect("epoch");
    world.commit("two.txt", 2);

    let (published, _) = world.publish_expecting(&comments, &head, Some(expected));

    published.expect("the publication the answer named was refused");
}

/// The stale approval's stop names the person's two ways out: the finding the
/// verdict did not weigh, which is the cause that admits the publication, or
/// closing the pull request and publishing afresh. The first is run here and
/// admitted; the second is the person's own GitHub action. Mutation: dropping
/// the finding from the timeline leaves the publication refused
/// `approved-republish-without-cause`.
#[test]
fn a_tier_that_moved_with_no_finding_names_a_finding_that_admits_the_publication() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim("claude-a"),
        published(&first),
        stored_computed_verdict(&first, "medium", "approved", "[]"),
    ];
    let context = context_at(&world.work);
    let (next, answer) = next_of(&context, &comments, &[open(&first)]);
    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "verdict-stale");
    assert_eq!(next["operation"], Value::Null);
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains("record_review_finding"),
        "{continuation}"
    );
    assert!(
        continuation.contains("close the pull request"),
        "{continuation}"
    );
    world.commit("two.txt", 2);

    let refused = refusal_of(
        world
            .publish_expecting(&comments, &head, Some(&first.epoch))
            .0,
    );
    assert_eq!(refused["reason"], "approved-republish-without-cause");

    let (recorded, with_finding) = record_finding_over(&named(&context), &comments, &first);
    let recorded = recorded.expect("the finding the continuation named was refused");
    assert_eq!(recorded["reused_existing_finding"], false, "{recorded}");

    let (published, _) = world.publish_expecting(&with_finding, &head, Some(&first.epoch));

    published.expect("the finding the continuation named did not admit the publication");
}

#[test]
fn a_late_lens_report_that_escalates_has_no_command_and_re_recording_is_refused() {
    let (_root, context, receipt) = medium_change();
    let mut comments = reported_and_recorded(&receipt, "medium");
    comments.push(recorded_lens_report_for(
        &receipt,
        4,
        "review-risk",
        Some("risk"),
    ));
    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);
    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["operation"], Value::Null);

    let refused = recorded_over(&named(&context), &comments, &receipt).expect_err("re-recorded");

    assert_eq!(
        refused.envelope()["reason"],
        "computed-verdict-already-recorded"
    );
}

#[test]
fn a_legacy_approval_with_no_marker_is_answered_with_a_recording_that_is_admitted() {
    let (_root, context, receipt) = medium_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        legacy_accepted(&receipt),
    ];
    let mut answers = vec![open(&receipt)];
    answers.extend(lane_green());
    let (next, answer) = next_of(&context, &comments, &answers);
    assert_eq!(next["operation"], "record_computed_verdict", "{answer}");

    recorded_over(&named(&context), &comments, &receipt).expect("the named recording was refused");
}

/// A moved pull request over a legacy approval with no computed marker answers
/// the recording the marker is owed, not the moved fall-through, and the
/// recording it names is admitted. Mutation: reading the moved branch without
/// the approval's own refusal leaves `pull-request-moved` and reddens the first
/// assertion.
#[test]
fn a_legacy_approval_over_a_moved_pull_request_is_recorded_not_answered_moved() {
    let (_root, context, receipt) = medium_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        legacy_accepted(&receipt),
    ];
    let moved = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt.clone()
    };
    let mut answers = vec![open(&moved)];
    answers.extend(lane_green());
    let (next, answer) = next_of(&context, &comments, &answers);
    assert_eq!(next["operation"], "record_computed_verdict", "{answer}");

    recorded_over(&named(&context), &comments, &receipt).expect("the named recording was refused");
}

#[test]
fn a_legacy_approval_over_a_ledger_that_now_rejects_is_recorded_not_republished() {
    let (_root, context, receipt) = medium_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        recorded_lens_report_for(&receipt, 3, RELIABILITY_LENS, None),
        legacy_accepted(&receipt),
        recorded_finding_on(
            &receipt,
            6,
            "f-1",
            "severe",
            Some("deterministic"),
            Some("introduced"),
        ),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["operation"], "record_computed_verdict", "{answer}");
}

#[test]
fn a_closed_pull_request_over_a_recovered_escalation_is_published_afresh() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "escalated"),
        recovery_of(&receipt),
    ];

    let (next, _) = next_of(&context, &comments, &[pull(&receipt, "CLOSED", false)]);

    assert_eq!(next["reason_code"], "pull-request-closed");
}

#[test]
fn an_approval_whose_tier_cannot_be_read_is_unavailable_not_a_state() {
    let (_root, context, receipt) = low_change();
    let unreadable = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt
    };
    let comments = [
        live_claim("claude-a"),
        published(&unreadable),
        computed_by("codex-b", &unreadable, "approved"),
    ];

    let failure = status_over(&context, &comments, &[open(&unreadable)])
        .expect_err("an unreadable tier was answered");

    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

/// A moved pull request over a rejected receipt whose one correction is unspent
/// names `publish_review`, and the publication it names is admitted. Mutation:
/// restoring the old continuation, which named no operation, reddens the
/// assertion that the continuation names `publish_review`.
#[test]
fn a_pull_request_that_moved_over_a_rejected_receipt_names_a_publication_that_is_admitted() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim("claude-a"),
        published(&first),
        computed_by("codex-b", &first, "rejected"),
    ];
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "pull-request-moved");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("publish_review"), "{continuation}");

    let (published, _) = world.publish_expecting(&comments, &moved_head, Some(&first.epoch));

    published.expect("the publication the continuation named was refused");
}

/// A moved pull request over a genuinely unjudged receipt names `publish_review`,
/// and the publication it names is admitted: an unjudged parent is the "neither"
/// case, spending no correction. Mutation: making the gate refuse a publication
/// over an unjudged parent reddens the admission assertion.
#[test]
fn a_pull_request_that_moved_over_an_unjudged_receipt_names_a_publication_that_is_admitted() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim("claude-a"),
        published(&first),
        recorded_lens_report_for(&first, 3, RELIABILITY_LENS, None),
        recorded_finding_on(&first, 4, "f-1", "warning", None, None),
    ];
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "pull-request-moved");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("publish_review"), "{continuation}");

    let (published, _) = world.publish_expecting(&comments, &moved_head, Some(&first.epoch));

    published.expect("the publication the continuation named was refused");
}

/// A moved pull request over a lineage whose one correction is spent names
/// `escalate_review`, and that escalation is admitted: the continuation does not
/// send an agent at a publication the gate refuses `correction-already-spent`.
#[test]
fn a_pull_request_that_moved_over_a_spent_correction_names_an_escalation_that_is_admitted() {
    let (_root, mut context) = context_with_no_repository();
    context.config = corrections_limit_row("1");
    let (receipt, mut comments) = review_world_with_spent_correction();
    comments.push(computed_by("codex-b", &receipt, "rejected"));
    let moved = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("escalate_review"), "{continuation}");

    comments[0] = live_claim("codex-b");
    let escalation = ReviewEscalation {
        issue: 42,
        run_id: "codex-b",
        epoch: &receipt.epoch,
        pr: receipt.pr,
        head: &receipt.head,
        base: &receipt.base,
        digest: &receipt.digest,
        escalated_to: "asanabrial",
        reason: "the correction was spent",
        now: "2026-08-31T00:00:00Z",
    };
    let directory = tempfile::tempdir().expect("a directory for the store");
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(42, stdout_with(&comments)),
        json!({"matches": "issue comment 42", "stdout": "", "status": 0}),
    ]);
    let answer = crate::transport::store::with_store_at(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            Ledger::open(&context, 42, RECLAIM_NOW).expect("the first touch imports the issue");
        });
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            escalate_review(&context, &escalation)
        })
    });

    assert_eq!(
        answer.expect("the escalation the continuation named was refused")["terminal_state"],
        "escalated"
    );
}

/// The moved branch that names `escalate_review` over a spent correction also
/// says the publication is refused `correction-already-spent`; attempting it
/// pins that refusal rather than trusting the sentence.
#[test]
fn a_pull_request_that_moved_over_a_spent_correction_refuses_the_publication() {
    let world = limited_to_one_correction(republish_world());
    let original_head = world.commit("two.txt", 1);
    let correction_head = world.commit("two.txt", 2);
    let moved_head = world.commit("two.txt", 3);
    let original = world.receipt('1', &original_head);
    let correction = world.receipt('2', &correction_head);
    let comments = [
        live_claim(REPUBLISH_RUN),
        published(&original),
        computed_by(REPUBLISH_RUN, &original, "rejected"),
        published_with_parent(REPUBLISH_RUN, &correction, &original, "delta-1-2"),
        computed_by(REPUBLISH_RUN, &correction, "rejected"),
    ];
    let mut context = context_at(&world.work);
    context.config = corrections_limit_row("1");
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..correction.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("escalate_review"), "{continuation}");

    let refused = refusal_of(
        world
            .publish_expecting(&comments, &moved_head, Some(&correction.epoch))
            .0,
    );
    assert_eq!(refused["reason"], "correction-already-spent", "{refused}");
}

/// A moved pull request over a fresh approval with no cause names the finding
/// that admits the publication, not the refused publication itself. The same
/// publication without the finding is still refused
/// `approved-republish-without-cause`, which is the gate not widened.
#[test]
fn a_pull_request_that_moved_over_an_approved_receipt_names_a_finding_that_admits_the_publication()
{
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = fresh_approved(&first);
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains("record_review_finding"),
        "{continuation}"
    );
    assert!(continuation.contains("publish_review"), "{continuation}");

    let refused = refusal_of(
        world
            .publish_expecting(&comments, &moved_head, Some(&first.epoch))
            .0,
    );
    assert_eq!(refused["reason"], "approved-republish-without-cause");

    let (recorded, with_finding) = record_finding_over(&named(&context), &comments, &first);
    let recorded = recorded.expect("the finding the continuation named was refused");
    assert_eq!(recorded["reused_existing_finding"], false, "{recorded}");

    let (published, _) = world.publish_expecting(&with_finding, &moved_head, Some(&first.epoch));

    published.expect("the finding the continuation named did not admit the publication");
}

/// A base-only advance over a fresh approval is the state the moved continuation
/// names `publish_review` for, and that publication is admitted. Mutation:
/// restoring the old continuation's blanket refusal leaves the base-advance
/// publication refused and reddens the last assertion.
#[test]
fn a_base_only_advance_over_an_approved_receipt_names_a_publication_that_is_admitted() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = fresh_approved(&first);
    git(&world.work, &["checkout", "-q", "main"]);
    git(
        &world.work,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "the base advances",
        ],
    );
    git(&world.work, &["push", "-q", "origin", "main"]);
    let base_after = head_of(&world.work);
    git(&world.work, &["checkout", "-q", REPUBLISH_BRANCH]);
    git(&world.work, &["merge", "--no-edit", "main"]);
    let merge_head = head_of(&world.work);
    let context = context_at(&world.work);
    let moved = ReviewTransaction {
        head: merge_head.clone(),
        base: base_after,
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("publish_review"), "{continuation}");

    let (published, _) = world.publish_expecting(&comments, &merge_head, Some(&first.epoch));

    published.expect("the base advance publication the continuation named was refused");
}

/// A stale approval over a moved pull request answers the stale stop, not a
/// moved stop: the continuation names the finding that admits the publication
/// and the person, and the finding it names is admitted at the moved head.
/// Mutation: leaving the moved branch to read the approval as fresh reddens the
/// `verdict-stale` assertion.
#[test]
fn a_stale_approval_over_a_moved_pull_request_answers_the_stale_stop() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim("claude-a"),
        published(&first),
        stored_computed_verdict(&first, "medium", "approved", "[]"),
    ];
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "verdict-stale");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains("record_review_finding"),
        "{continuation}"
    );
    assert!(continuation.contains("person decides"), "{continuation}");

    let (recorded, with_finding) = record_finding_over(&named(&context), &comments, &first);
    recorded.expect("the finding the continuation named was refused");

    let (published, _) = world.publish_expecting(&with_finding, &moved_head, Some(&first.epoch));

    published.expect("the finding the continuation named did not admit the publication");
}

/// A moved pull request over an approved receipt that a finding has answered
/// reaches the moved fall-through, which names `publish_review`, and that
/// publication is admitted. This is the default the rejected-unspent test does
/// not exercise.
#[test]
fn a_moved_approved_receipt_answered_by_a_finding_names_a_publication_that_is_admitted() {
    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let mut comments = fresh_approved(&first);
    comments.push(recorded_finding_on(&first, 6, "f-1", "warning", None, None));
    let context = context_at(&world.work);
    let moved_head = world.commit("two.txt", 2);
    let moved = ReviewTransaction {
        head: moved_head.clone(),
        ..first.clone()
    };

    let (next, answer) = next_of(&context, &comments, &[open(&moved)]);
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("publish_review"), "{continuation}");

    let (published, _) = world.publish_expecting(&comments, &moved_head, Some(&first.epoch));

    published.expect("the publication the fall-through named was refused");
}

/// A merged pull request is delivered; the continuation hands the holder the post-merge tail
/// `closing::FOLLOW_UP` states, rather than assigning the close to a person with no command.
#[test]
fn a_merged_pull_request_names_the_post_merge_tail() {
    let (_root, context, receipt) = low_change();
    let comments = [live_claim("claude-a"), published(&receipt)];

    let (next, _) = next_of(&context, &comments, &[pull(&receipt, "MERGED", false)]);

    assert_eq!(next["kind"], "stop");
    assert_eq!(next["reason_code"], "delivered");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(
        continuation.contains(crate::transport::closing::FOLLOW_UP),
        "{continuation}"
    );
    assert!(continuation.contains("`release`"), "{continuation}");
    assert!(
        !continuation.contains("no Estigia command applies"),
        "{continuation}"
    );
}

/// A released receipt names the merge the holder runs, bound to that receipt's pull request and
/// head, because the delivery binding has the run merge under `Delivery authorisation`.
#[test]
fn a_released_receipt_names_the_exact_merge_command() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let (next, _) = next_of(&context, &comments, &[pull(&receipt, "OPEN", false)]);

    assert_eq!(next["kind"], "stop");
    assert_eq!(next["reason_code"], "released");
    let continuation = next["continuation"].as_str().expect("a continuation");
    let merge = format!(
        "gh pr {} {} --merge --match-head-commit {}",
        "merge", receipt.pr, receipt.head
    );
    assert!(continuation.contains(&merge), "{continuation}");
    assert!(
        continuation.contains("Delivery authorisation"),
        "{continuation}"
    );
    assert!(
        continuation.contains(&receipt.base),
        "the ask does not name the reviewed base: {continuation}"
    );
    assert!(!continuation.contains("A person merges"), "{continuation}");
}

/// `skill/policies/blind-judges.md`, `skill/bindings/github/operations.md`,
/// `docs/review-status.md`, `review_status` and the gate agree that a standing escalation is the
/// person's decision: the policy names the person's recover rather than a refused correction, and
/// the gate admits a publication over the escalation while its correction is unspent, which the
/// policy's definition states. Mutation: restoring a "never corrected" sentence in `skill/SKILL.md`
/// reddens the first assertion; making the gate refuse the unspent escalation reddens the
/// publication crossing.
#[test]
fn the_contract_the_document_and_the_code_agree_on_an_unrecovered_escalation() {
    let skill = include_str!("../../../../skill/SKILL.md");
    let flat = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !flat.contains("never corrected"),
        "the skill still claims a standing escalation is never corrected"
    );
    assert!(
        !flat.contains("may be corrected like a rejected one"),
        "the skill still lets an agent correct an unrecovered escalation"
    );

    let judges = include_str!("../../../../skill/policies/blind-judges.md");
    let judges_flat = judges.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        judges_flat.contains("`rejected` or `escalated`"),
        "blind judges no longer defines a correction as the gate counts it"
    );
    assert!(
        judges_flat.contains(
            "A receipt that stands escalated while its lineage still has a correction left may be corrected like a rejected one"
        ),
        "blind judges no longer states when an escalation may be corrected"
    );
    assert!(
        judges_flat.contains("person's command"),
        "blind judges no longer states that recovery is the person's command"
    );
    assert!(
        judges_flat.contains("It mints a **successor lineage**"),
        "blind judges no longer states the recovered escalation's successor"
    );
    assert!(
        judges_flat.contains(
            "recovering is not a release, and `release_ci` still needs an approved computed verdict on the successor's receipt"
        ),
        "blind judges no longer states that recovering releases nothing"
    );

    let operations = include_str!("../../../../skill/bindings/github/operations.md");
    let operations_flat = operations.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        operations_flat.contains(
            "An agent whose receipt stands escalated with its lineage's corrections spent, or handed to a person by `escalate_review`, tells the person that command and waits; while a correction is unspent it may still be corrected"
        ),
        "the operations companion no longer states the agent's escalation route"
    );
    assert!(
        operations_flat
            .contains("`release_ci` still needs an approved computed verdict on its own receipt"),
        "the operations companion no longer states that recovery releases nothing"
    );

    let world = republish_world();
    let head = world.commit("two.txt", 1);
    let first = world.receipt('1', &head);
    let comments = [
        live_claim(REPUBLISH_RUN),
        published(&first),
        computed_by(REPUBLISH_RUN, &first, "escalated"),
    ];
    let context = context_at(&world.work);
    let (next, answer) = next_of(&context, &comments, &[open(&first)]);
    assert_eq!(next["reason_code"], "escalated", "{answer}");
    let published = world.publish(&comments, &head);
    published.expect("the gate refused a publication over an unspent escalation");

    let document = include_str!("../../../../docs/review-status.md");
    let flat_document = document.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat_document.contains(
            "the person's `estigia recover <issue> --reason \"<why>\"` command, never an agent's"
        ),
        "the document no longer agrees that an unrecovered escalation is the person's recover"
    );
    assert!(
        flat_document
            .contains("a recovered escalation over a moved pull request names its successor"),
        "the document no longer states the recovered-escalation precedence over a moved pull request"
    );
}

#[derive(Clone, Copy)]
enum Runner {
    Alive,
    Answered,
    Dead,
}

struct Planted {
    epoch: String,
    name: &'static str,
    deadline_at: u64,
    runner: Runner,
}

fn planted_for(receipt: &ReviewTransaction, runner: Runner, deadline_at: u64) -> Planted {
    Planted {
        epoch: receipt.epoch.clone(),
        name: RELIABILITY_LENS,
        deadline_at,
        runner,
    }
}

const FAR: u64 = 4_000_000_000;

fn status_with_job(
    context: &Context,
    receipt: &ReviewTransaction,
    planted: &Planted,
) -> (Value, Value) {
    let comments = [live_claim("claude-a"), published(receipt)];
    crate::transport::store::forget_scratch_store();
    let dir = tempfile::tempdir().expect("a job directory");
    let lock = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join("lock"))
        .expect("a lock");
    match planted.runner {
        Runner::Alive => lock.lock().expect("the runner holds its lock"),
        Runner::Answered => crate::paths::replace_atomically(
            &dir.path().join("result.json"),
            r#"{"format":1,"answer_hex":"6162"}"#,
        )
        .expect("a result"),
        Runner::Dead => {}
    }
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(&comments),
    )]);
    let held = crate::reviewer::Receipt {
        epoch: planted.epoch.clone(),
        pr: receipt.pr,
        head: receipt.head.clone(),
        base: receipt.base.clone(),
        digest: receipt.digest.clone(),
        payload_digest: String::new(),
    };
    let offered = json!([]);
    let dir_text = dir.path().to_string_lossy().into_owned();
    let new = crate::transport::store::NewJob {
        id: "job-1",
        run_id: "claude-a",
        name: planted.name,
        receipt: &held,
        offered: &offered,
        attempt: None,
        started_at: 1,
        deadline_at: planted.deadline_at,
        dir: &dir_text,
    };
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let ledger = Ledger::open(&named(context), 42, RECLAIM_NOW).expect("the issue is imported");
        ledger
            .transact(|tx| tx.insert_job(&new))
            .expect("the job is recorded");
    });
    let answer = {
        let bin = crate::test_env::scripted_gh();
        crate::test_env::with_scripted_gh(
            bin.path(),
            &crate::test_env::answers(&[open(receipt)]),
            || review_status(&named(context), 42),
        )
    }
    .unwrap_or_else(|failure| panic!("status refused: {}", failure.envelope()));
    drop(lock);
    (answer["next"].clone(), answer)
}

#[test]
fn a_running_job_is_not_offered_for_collection() {
    let (_root, context, receipt) = medium_change();

    let (next, answer) = status_with_job(
        &context,
        &receipt,
        &planted_for(&receipt, Runner::Alive, FAR),
    );

    assert_eq!(next["kind"], "collect", "{answer}");
    assert_eq!(next["reason_code"], "lens-report-missing");
    assert_eq!(
        next["inputs"],
        json!([{
            "name": RELIABILITY_LENS,
            "capture": null,
            "running": {"job": "job-1", "deadline_at": FAR},
        }])
    );
}

#[test]
fn every_input_running_says_when_to_ask_again() {
    let (_root, context, receipt) = medium_change();

    let (next, _) = status_with_job(
        &context,
        &receipt,
        &planted_for(&receipt, Runner::Alive, FAR),
    );

    assert_eq!(next["kind"], "collect");
    assert_eq!(next["ask_after"], FAR);
}

#[test]
fn an_input_that_can_be_collected_asks_for_no_waiting() {
    let (_root, context, receipt) = medium_change();

    let (next, _) = status_with_job(
        &context,
        &receipt,
        &planted_for(&receipt, Runner::Answered, FAR),
    );

    assert!(next.get("ask_after").is_none(), "{next}");
}

#[test]
fn a_finished_job_is_collected_by_name_and_job() {
    let (_root, context, receipt) = medium_change();

    let (next, answer) = status_with_job(
        &context,
        &receipt,
        &planted_for(&receipt, Runner::Answered, FAR),
    );

    assert_eq!(
        next["inputs"],
        json!([{"name": RELIABILITY_LENS, "capture": "collect_review_input", "job": "job-1"}]),
        "{answer}"
    );
}

#[test]
fn a_job_whose_runner_died_is_collected_to_abandon_it() {
    let (_root, context, receipt) = medium_change();

    let (next, _) = status_with_job(
        &context,
        &receipt,
        &planted_for(&receipt, Runner::Dead, FAR),
    );

    assert_eq!(
        next["inputs"],
        json!([{"name": RELIABILITY_LENS, "capture": "collect_review_input", "job": "job-1"}])
    );
}

#[test]
fn a_job_still_alive_past_its_deadline_and_grace_is_collected_to_end_it() {
    let (_root, context, receipt) = medium_change();

    let (next, _) = status_with_job(&context, &receipt, &planted_for(&receipt, Runner::Alive, 1));

    assert_eq!(
        next["inputs"],
        json!([{"name": RELIABILITY_LENS, "capture": "collect_review_input", "job": "job-1"}])
    );
    assert!(next.get("ask_after").is_none(), "{next}");
}

#[test]
fn a_job_of_another_epoch_holds_nothing() {
    let (_root, context, receipt) = medium_change();
    let planted = Planted {
        epoch: op(9),
        ..planted_for(&receipt, Runner::Alive, FAR)
    };

    let (next, _) = status_with_job(&context, &receipt, &planted);

    assert_eq!(
        next["inputs"],
        json!([{"name": RELIABILITY_LENS, "capture": "launch_review_input"}])
    );
    assert!(next.get("ask_after").is_none(), "{next}");
}

#[test]
fn every_escalation_cause_has_a_plain_sentence_naming_what_the_person_decides() {
    use super::super::computed::Escalation;
    let expected = [
        (
            "evidence-class-missing",
            "without saying how it is evidenced",
        ),
        (
            "causal-disposition-missing",
            "whether this change or the base caused it",
        ),
        ("insufficient-evidence", "insufficient evidence"),
        (
            "causal-disposition-unknown",
            "cause of the severe finding `f-1` is unknown",
        ),
        ("no-refuter-result", "no refuter's answer"),
        ("lens-focus-disagreement", "different focuses"),
        (
            "validator-delta-carries-upstream",
            "history was rewritten or its merge with the moved base did not integrate cleanly, so its fix cannot be isolated from the base; decide whether to accept the change without a targeted validation",
        ),
        (
            "original_criteria_failed",
            "did not fix the parent's severe findings",
        ),
        (
            "correction_regression_failed",
            "broke something on the paths it touched",
        ),
        ("validator_inconclusive", "three attempts"),
    ];
    for (reason, phrase) in expected {
        let why = Escalation {
            id: "f-1".to_owned(),
            reason: reason.to_owned(),
        }
        .why();
        assert!(why.contains(phrase), "{reason}: {why}");
        assert!(why.contains("decide"), "{reason} names no decision: {why}");
    }
}

#[test]
fn an_explicit_escalation_stops_with_the_run_that_handed_it_over_and_its_reason() {
    let (_root, context, receipt) = low_change();
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        escalated_receipt_by("claude-a", &"5".repeat(32), "Alexis", &receipt),
    ];

    let (next, answer) = next_of(&context, &comments, &[open(&receipt)]);

    assert_eq!(next["reason_code"], "escalated", "{answer}");
    let why = next["why"].as_str().expect("a plain reason");
    assert!(
        why.contains("Alexis") && why.contains("the correction was spent"),
        "{why}"
    );
}

#[test]
fn the_computed_verdict_answer_carries_the_why_beside_each_reason_code() {
    use super::super::computed::{ComputedOutcome, ComputedVerdict, Escalation};
    let verdict = ComputedVerdict {
        outcome: ComputedOutcome::Escalated,
        open: Vec::new(),
        escalated: vec![Escalation {
            id: "validator".to_owned(),
            reason: "validator-delta-carries-upstream".to_owned(),
        }],
        follow_ups: Vec::new(),
        missing_roles: Vec::new(),
    };

    let json = verdict.to_json();

    assert_eq!(
        json["escalated"][0]["reason"],
        "validator-delta-carries-upstream"
    );
    assert!(
        json["escalated"][0]["why"]
            .as_str()
            .is_some_and(|why| why.contains("cannot be isolated from the base")),
        "{json}"
    );
}

fn claim_over(context: &Context, comments: &[ownership::Comment], pr_answers: &[Value]) -> Value {
    let mut timeline = vec![explaining_projection()];
    timeline.extend_from_slice(comments);
    let mut answers = vec![
        crate::test_env::issue_view_answer(
            42,
            issue_payload(
                &timeline,
                &["fixture"],
                &["dev:claude", "status:in-progress"],
            ),
        ),
        json!({"matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0}),
    ];
    answers.extend_from_slice(pr_answers);
    crate::transport::store::forget_scratch_store();
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
        claim(&named(context), &acquisition(42, "claude-a")).expect("the claim wins")
    })
}

fn receipt_json(receipt: &ReviewTransaction) -> Value {
    json!({
        "epoch": receipt.epoch,
        "pr": receipt.pr,
        "head": receipt.head,
        "base": receipt.base,
        "digest": receipt.digest,
    })
}

#[test]
fn a_claim_over_a_current_approved_publication_carries_its_receipt() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let answer = claim_over(&context, &comments, &[pull(&receipt, "OPEN", false)]);

    assert_eq!(
        answer["review_transaction"],
        receipt_json(&receipt),
        "{answer}"
    );
}

#[test]
fn a_claim_restores_nothing_when_the_head_has_moved() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let moved = ReviewTransaction {
        head: "f".repeat(40),
        ..receipt
    };

    let answer = claim_over(&context, &comments, &[pull(&moved, "OPEN", false)]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

#[test]
fn a_claim_restores_nothing_for_a_rejected_escalated_or_closed_publication() {
    for outcome in ["rejected", "escalated"] {
        let (_root, context, receipt) = low_change();
        let comments = [
            published(&receipt),
            computed_by("codex-b", &receipt, outcome),
        ];

        let answer = claim_over(&context, &comments, &[pull(&receipt, "OPEN", false)]);

        assert!(
            answer.get("review_transaction").is_none(),
            "{outcome}: {answer}"
        );
    }
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let answer = claim_over(&context, &comments, &[pull(&receipt, "MERGED", false)]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

fn gate_over(context: &Context, state_root: &std::path::Path) -> crate::harness::GateContext {
    crate::harness::GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: context.repo_dir.join("skill"),
        repo_dir: context.repo_dir.clone(),
        state_root: state_root.to_path_buf(),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn merge_verdict(
    gate: &crate::harness::GateContext,
    run: &crate::harness::session::Run,
    receipt: &ReviewTransaction,
    reported: &ReviewTransaction,
) -> Option<crate::outcome::Refusal> {
    let command = format!(
        "gh pr merge {} --merge --match-head-commit {}",
        receipt.pr, receipt.head
    );
    let (action, _) = crate::harness::classify("Bash", &json!({ "command": command }));
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[pull(reported, "OPEN", false)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        crate::harness::stale_verdict(
            &action,
            run,
            &gate.repo_dir,
            &crate::harness::live_pr_head_reader(gate),
        )
    })
}

fn sworn_over(gate: &crate::harness::GateContext, answer: &Value) -> crate::harness::session::Run {
    let tool = crate::harness::mcp::tools::TOOLS
        .iter()
        .find(|tool| tool.name == "claim")
        .expect("the claim tool is listed");
    let mut run = crate::harness::session::Run::new("claude-a".to_owned());
    crate::harness::mcp::apply_effect(tool, &json!({"issue": 42}), Some(answer), &mut run, gate);
    run
}

#[test]
fn a_restored_receipt_lets_the_merge_through_on_the_reviewed_head_and_refuses_a_moved_one() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let answer = claim_over(&context, &comments, &[pull(&receipt, "OPEN", false)]);
    let state = tempfile::tempdir().expect("a state root");
    let gate = gate_over(&context, state.path());

    let run = sworn_over(&gate, &answer);

    assert_eq!(run.review_transaction.as_ref(), Some(&receipt), "{answer}");
    assert!(
        merge_verdict(&gate, &run, &receipt, &receipt).is_none(),
        "a merge on the reviewed head was refused after the claim restored its receipt"
    );
    let moved = ReviewTransaction {
        head: "f".repeat(40),
        ..receipt.clone()
    };
    let refusal =
        merge_verdict(&gate, &run, &receipt, &moved).expect("a merge on a moved head was admitted");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
}

#[test]
fn a_claim_whose_pull_request_cannot_be_read_still_succeeds_and_restores_nothing() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let unreadable = json!({"matches": "pr view", "stderr": "gh: boom", "status": 1});

    let answer = claim_over(&context, &comments, &[unreadable]);

    assert_eq!(answer["ok"], true, "{answer}");
    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

#[test]
fn a_claim_restores_nothing_when_the_base_has_moved() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];
    let moved = ReviewTransaction {
        base: "f".repeat(40),
        ..receipt
    };

    let answer = claim_over(&context, &comments, &[pull(&moved, "OPEN", false)]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

#[test]
fn a_claim_restores_nothing_over_a_closed_pull_request() {
    let (_root, context, receipt) = low_change();
    let comments = [
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let answer = claim_over(&context, &comments, &[pull(&receipt, "CLOSED", false)]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

#[test]
fn a_claim_restores_nothing_over_an_unconfirmed_publication() {
    let (_root, context, receipt) = low_change();
    let unconfirmed = wrote(
        "3",
        super::super::super::markers::render(
            "published",
            &[
                ("run-id", "claude-a"),
                ("epoch", &receipt.epoch),
                ("pr", &receipt.pr.to_string()),
                ("head", &receipt.head),
                ("base", &receipt.base),
                ("digest", &receipt.digest),
                ("unconfirmed", "true"),
            ],
        )
        .expect("a receipt marker"),
    );
    let comments = [unconfirmed, computed_by("codex-b", &receipt, "approved")];

    let answer = claim_over(&context, &comments, &[pull(&receipt, "OPEN", false)]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

#[test]
fn a_claim_with_no_publication_carries_no_receipt() {
    let (_root, context, _receipt) = low_change();

    let answer = claim_over(&context, &[], &[]);

    assert!(answer.get("review_transaction").is_none(), "{answer}");
}

fn pull_over_base(
    receipt: &ReviewTransaction,
    base: &str,
    draft: bool,
    mergeable: &str,
    merge_state: &str,
) -> Value {
    json!({
        "matches": "pr view",
        "stdout": json!({
            "number": receipt.pr,
            "url": "https://github.com/o/r/pull/7",
            "headRefOid": receipt.head,
            "baseRefOid": base,
            "state": "OPEN",
            "isDraft": draft,
            "mergeable": mergeable,
            "mergeStateStatus": merge_state,
        })
        .to_string(),
        "status": 0,
    })
}

fn advance_base(context: &Context) -> String {
    git(
        &context.repo_dir,
        &["commit", "--quiet", "--allow-empty", "-m", "main advances"],
    );
    head_of(&context.repo_dir)
}

fn unrelated_base(context: &Context) -> String {
    git(
        &context.repo_dir,
        &["checkout", "-q", "--orphan", "unrelated"],
    );
    git(
        &context.repo_dir,
        &["commit", "--quiet", "--allow-empty", "-m", "unrelated root"],
    );
    let root = head_of(&context.repo_dir);
    git(&context.repo_dir, &["checkout", "-q", "-f", "main"]);
    root
}

fn approved(receipt: &ReviewTransaction) -> [ownership::Comment; 3] {
    [
        live_claim("claude-a"),
        published(receipt),
        computed_by("codex-b", receipt, "approved"),
    ]
}

#[test]
fn a_base_that_advanced_cleanly_leaves_an_approved_receipt_released_for_the_merge() {
    let (_root, context, receipt) = low_change();
    let advanced = advance_base(&context);

    let (next, answer) = next_of(
        &context,
        &approved(&receipt),
        &[pull_over_base(
            &receipt,
            &advanced,
            false,
            "MERGEABLE",
            "CLEAN",
        )],
    );

    assert_eq!(next["kind"], "stop", "{answer}");
    assert_eq!(next["reason_code"], "released", "{answer}");
    assert_eq!(answer["receipt"]["base"], receipt.base);
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("advanced"), "{continuation}");
    assert!(continuation.contains(&advanced), "{continuation}");
}

#[test]
fn a_claim_restores_the_receipt_over_a_base_that_advanced_cleanly_and_the_merge_is_admitted() {
    let (_root, context, receipt) = low_change();
    let advanced = advance_base(&context);
    let answer = claim_over(
        &context,
        &[
            published(&receipt),
            computed_by("codex-b", &receipt, "approved"),
        ],
        &[pull_over_base(
            &receipt,
            &advanced,
            false,
            "MERGEABLE",
            "CLEAN",
        )],
    );
    let state = tempfile::tempdir().expect("a state root");
    let gate = gate_over(&context, state.path());

    let run = sworn_over(&gate, &answer);

    assert_eq!(run.review_transaction.as_ref(), Some(&receipt), "{answer}");
    let seen = ReviewTransaction {
        base: advanced,
        ..receipt.clone()
    };
    assert!(
        merge_verdict(&gate, &run, &receipt, &seen).is_none(),
        "a merge on the reviewed head over an advanced base was refused"
    );
}

#[test]
fn a_base_the_clone_does_not_hold_leaves_no_live_receipt_and_is_no_failure() {
    let (_root, context, receipt) = low_change();
    let absent = "7".repeat(40);
    let seen = pull_over_base(&receipt, &absent, false, "MERGEABLE", "CLEAN");
    stored(
        &context,
        &[
            published(&receipt),
            computed_by("codex-b", &receipt, "approved"),
        ],
    );
    let bin = crate::test_env::scripted_gh();

    let live =
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&[seen]), || {
            current_review_transaction(&named(&context), 42)
        });

    assert!(matches!(live, Ok(None)), "{live:?}");
}

#[test]
fn a_base_that_advanced_with_a_conflict_is_not_the_live_receipt() {
    for (mergeable, merge_state) in [
        ("CONFLICTING", "DIRTY"),
        ("CONFLICTING", "CLEAN"),
        ("MERGEABLE", "DIRTY"),
    ] {
        let (_root, context, receipt) = low_change();
        let advanced = advance_base(&context);
        let seen = pull_over_base(&receipt, &advanced, false, mergeable, merge_state);

        let (next, answer) = next_of(&context, &approved(&receipt), std::slice::from_ref(&seen));
        assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");

        let claimed = claim_over(
            &context,
            &[
                published(&receipt),
                computed_by("codex-b", &receipt, "approved"),
            ],
            &[seen],
        );
        assert!(
            claimed.get("review_transaction").is_none(),
            "{mergeable}/{merge_state}: {claimed}"
        );
    }
}

#[test]
fn a_mergeability_github_is_still_computing_is_refused_with_a_word_that_says_so() {
    for (mergeable, merge_state) in [
        ("UNKNOWN", "UNKNOWN"),
        ("UNKNOWN", "CLEAN"),
        ("MERGEABLE", "UNKNOWN"),
    ] {
        let (_root, context, receipt) = low_change();
        let advanced = advance_base(&context);
        let seen = pull_over_base(&receipt, &advanced, false, mergeable, merge_state);

        let (next, answer) = next_of(&context, &approved(&receipt), std::slice::from_ref(&seen));
        assert_eq!(next["kind"], "stop", "{answer}");
        assert_eq!(next["reason_code"], "mergeability-unknown", "{answer}");
        let continuation = next["continuation"].as_str().expect("a continuation");
        assert!(continuation.contains("still computing"), "{continuation}");

        let claimed = claim_over(
            &context,
            &[
                published(&receipt),
                computed_by("codex-b", &receipt, "approved"),
            ],
            &[seen],
        );
        assert!(
            claimed.get("review_transaction").is_none(),
            "{mergeable}/{merge_state}: {claimed}"
        );
    }
}

#[test]
fn a_base_that_does_not_descend_from_the_reviewed_base_is_not_the_live_receipt() {
    let (_root, context, receipt) = low_change();
    let unrelated = unrelated_base(&context);
    let seen = pull_over_base(&receipt, &unrelated, false, "MERGEABLE", "CLEAN");

    let (next, answer) = next_of(&context, &approved(&receipt), std::slice::from_ref(&seen));
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");

    let claimed = claim_over(
        &context,
        &[
            published(&receipt),
            computed_by("codex-b", &receipt, "approved"),
        ],
        &[seen],
    );
    assert!(claimed.get("review_transaction").is_none(), "{claimed}");
}

#[test]
fn a_moved_head_is_not_the_live_receipt_whatever_the_base_and_the_mergeability() {
    let (_root, context, receipt) = low_change();
    let advanced = advance_base(&context);
    let moved = ReviewTransaction {
        head: "9".repeat(40),
        ..receipt.clone()
    };
    let seen = pull_over_base(&moved, &advanced, false, "MERGEABLE", "CLEAN");

    let (next, answer) = next_of(&context, &approved(&receipt), std::slice::from_ref(&seen));
    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");

    let claimed = claim_over(
        &context,
        &[
            published(&receipt),
            computed_by("codex-b", &receipt, "approved"),
        ],
        &[seen],
    );
    assert!(claimed.get("review_transaction").is_none(), "{claimed}");
}

#[test]
fn a_draft_whose_base_advanced_is_not_released_to_ci_over_the_old_base() {
    let (_root, context, receipt) = low_change();
    let advanced = advance_base(&context);

    let (next, answer) = next_of(
        &context,
        &approved(&receipt),
        &[pull_over_base(
            &receipt,
            &advanced,
            true,
            "MERGEABLE",
            "CLEAN",
        )],
    );

    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
}

#[test]
fn a_receipt_that_is_not_approved_is_not_carried_over_an_advanced_base() {
    let (_root, context, receipt) = low_change();
    let advanced = advance_base(&context);

    let (next, answer) = next_of(
        &context,
        &[live_claim("claude-a"), published(&receipt)],
        &[pull_over_base(
            &receipt,
            &advanced,
            false,
            "MERGEABLE",
            "CLEAN",
        )],
    );

    assert_eq!(next["reason_code"], "pull-request-moved", "{answer}");
}

#[test]
fn a_base_the_clone_does_not_hold_is_unavailable_not_a_clean_advance() {
    let (_root, context, receipt) = low_change();
    let seen = pull_over_base(&receipt, &"9".repeat(40), false, "MERGEABLE", "CLEAN");

    let failure = status_over(&context, &approved(&receipt), &[seen])
        .expect_err("an unreadable ancestry was answered");

    assert_eq!(class_of_failure(&failure), RefusalClass::Unavailable);
}

fn released_continuation_under(lane: Value) -> String {
    let (_root, _context, receipt) = low_change();
    let pull = json!({"baseRefOid": receipt.base});
    released_continuation(&receipt, &pull, &lane)
}

#[test]
fn a_released_receipt_does_not_wait_for_ci_where_the_lane_says_none_will_run() {
    for state in ["not-consulted", "disabled", "never-ran"] {
        let continuation = released_continuation_under(json!({"state": state}));
        assert!(
            continuation.contains("no CI will run") && continuation.contains(state),
            "{state}: {continuation}"
        );
        assert!(!continuation.contains("When CI is green"), "{continuation}");
    }
}

#[test]
fn a_released_receipt_waits_for_ci_where_a_lane_will_run_or_cannot_be_read() {
    for state in ["green", "running", "absent", "red", "unreadable"] {
        let continuation = released_continuation_under(json!({"state": state}));
        assert!(
            continuation.contains("When CI is green"),
            "{state}: {continuation}"
        );
        assert!(!continuation.contains("no CI will run"), "{continuation}");
    }
}

#[test]
fn review_status_reads_the_lane_into_the_released_continuation() {
    let (_root, context, receipt) = low_change();
    let off = Context {
        config: vec![(
            "Publication lane".to_owned(),
            "off".to_owned(),
            Source::Written,
        )],
        ..context
    };
    let comments = [
        live_claim("claude-a"),
        published(&receipt),
        computed_by("codex-b", &receipt, "approved"),
    ];

    let (next, answer) = next_of(&off, &comments, &[pull(&receipt, "OPEN", false)]);

    assert_eq!(next["reason_code"], "released", "{answer}");
    let continuation = next["continuation"].as_str().expect("a continuation");
    assert!(continuation.contains("no CI will run"), "{continuation}");
    assert!(!continuation.contains("When CI is green"), "{continuation}");
}
