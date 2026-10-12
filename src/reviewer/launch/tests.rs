use std::cell::RefCell;

use super::*;
use crate::reviewer::{Receipt, ReviewerCode, Role};
use crate::transport::ownership::sha256_hex;

const DIFF: &str = "diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1 +1 @@\n-old\n+new\n";
const ANSWER: &str = r#"{"findings":[{"id":"f1","class":"severe","evidence":"x.rs:1 drops it","impact":"loses a write","location":"x.rs:1","evidence_class":"deterministic","causal_disposition":"introduced"}]}"#;

struct Stub {
    answer: Result<Vec<u8>, Refusal>,
    seen: RefCell<Vec<Invocation>>,
}

impl Stub {
    fn answering(answer: Result<Vec<u8>, Refusal>) -> Self {
        Self {
            answer,
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl Adapter for Stub {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        self.seen.borrow_mut().push(invocation.clone());
        self.answer.clone()
    }
}

fn receipt() -> Receipt {
    Receipt {
        epoch: "e1".to_owned(),
        pr: 42,
        head: "h".repeat(40),
        base: "b".repeat(40),
        digest: "manifest-digest".to_owned(),
        payload_digest: sha256_hex(DIFF.as_bytes()),
    }
}

fn run(stub: &Stub, payload: &[u8]) -> Result<RoleResult, Refusal> {
    let receipt = receipt();
    launch(
        stub,
        &PromptInput {
            role: Role::Risk,
            receipt: &receipt,
            payload,
            criteria: "the criteria",
            findings: &[],
        },
        Some("a-model".to_owned()),
    )
}

#[test]
fn a_valid_lens_answer_round_trips_to_findings() {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let RoleResult::Findings(found) = run(&stub, DIFF.as_bytes()).expect("an answer") else {
        panic!("a lens answers with findings");
    };
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "f1");

    let seen = stub.seen.borrow();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].role, "review-risk");
    assert_eq!(seen[0].receipt, receipt());
    assert_eq!(seen[0].model.as_deref(), Some("a-model"));
    assert!(String::from_utf8_lossy(&seen[0].prompt).contains("x.rs"));
}

#[test]
fn an_adapter_refusal_passes_through_under_its_own_code() {
    let stub = Stub::answering(Err(ReviewerCode::Deadline.refusal("too slow")));
    let refusal = run(&stub, DIFF.as_bytes()).expect_err("a refusal");
    assert_eq!(refusal.code, "reviewer-deadline");
}

#[test]
fn a_malformed_answer_is_reviewer_result_malformed() {
    let stub = Stub::answering(Ok(b"looks fine to me".to_vec()));
    let refusal = run(&stub, DIFF.as_bytes()).expect_err("a refusal");
    assert_eq!(refusal.code, "reviewer-result-malformed");
}

#[test]
fn a_payload_that_is_not_the_published_one_refuses_before_the_adapter_runs() {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let refusal = run(&stub, b"some other diff").expect_err("a refusal");
    assert_eq!(refusal.code, "reviewer-payload-mismatch");
    assert!(stub.seen.borrow().is_empty(), "the adapter was called");
}

fn recorded(stub: &Stub, verdict: Recording) -> (Result<RoleResult, Refusal>, Vec<RoleResult>) {
    let receipt = receipt();
    let mut seen = Vec::new();
    let outcome = launch_recorded(
        stub,
        &PromptInput {
            role: Role::Risk,
            receipt: &receipt,
            payload: DIFF.as_bytes(),
            criteria: "the criteria",
            findings: &[],
        },
        None,
        &mut |answer| {
            seen.push(answer.clone());
            verdict.clone()
        },
    );
    (outcome, seen)
}

fn refusal_named(code: &'static str) -> Refusal {
    Refusal::not_started(
        code,
        "the recorder's message",
        crate::outcome::Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "the recorder's way out",
        ),
    )
}

fn rejected_by_the_recorder() -> Refusal {
    refusal_named("finding-location-outside-the-change")
}

fn passes_through(refusal: Refusal) {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let (outcome, seen) = recorded(&stub, Recording::Refused(refusal.clone()));
    assert_eq!(outcome.expect_err("a refusal"), refusal);
    assert_eq!(seen.len(), 1);
}

#[test]
fn an_answer_the_recorder_records_is_returned() {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let (outcome, seen) = recorded(&stub, Recording::Recorded);
    let returned = outcome.expect("the recorded answer");
    let expected = crate::reviewer::result::parse(Role::Risk, ANSWER.as_bytes()).expect("parses");
    assert_eq!(seen, vec![expected.clone()]);
    assert_eq!(returned, expected);
}

#[test]
fn an_answer_the_recorder_rejects_is_an_unusable_answer_that_names_why() {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let (outcome, seen) = recorded(&stub, Recording::AnswerRejected(rejected_by_the_recorder()));
    let refusal = outcome.expect_err("a refusal, never the answer");
    assert_eq!(refusal.code, "reviewer-result-malformed");
    assert!(
        refusal
            .message
            .contains("finding-location-outside-the-change")
            && refusal.message.contains("the recorder's message"),
        "{}",
        refusal.message
    );
    assert_eq!(seen.len(), 1);
}

#[test]
fn a_rejection_whose_write_may_have_landed_is_not_called_an_unusable_answer() {
    let stub = Stub::answering(Ok(ANSWER.as_bytes().to_vec()));
    let landed = Refusal {
        outcome: crate::outcome::MutationOutcome::Unknown,
        ..rejected_by_the_recorder()
    };
    assert_eq!(landed.class, crate::outcome::RefusalClass::Refused);
    let (outcome, _) = recorded(&stub, Recording::AnswerRejected(landed.clone()));
    assert_eq!(outcome.expect_err("a refusal"), landed);
}

#[test]
fn a_person_s_stop_is_not_called_an_unusable_answer() {
    let stop = refusal_named("control-message");
    assert_eq!(stop.class, crate::outcome::RefusalClass::Refused);
    passes_through(stop);
}

#[test]
fn a_closed_issue_is_not_called_an_unusable_answer() {
    let closed = refusal_named("issue-not-open");
    assert_eq!(closed.class, crate::outcome::RefusalClass::Refused);
    passes_through(closed);
}

#[test]
fn a_recorder_whose_write_may_have_landed_is_not_called_an_unusable_answer() {
    passes_through(Refusal {
        outcome: crate::outcome::MutationOutcome::Unknown,
        class: crate::outcome::RefusalClass::Unknown,
        ..rejected_by_the_recorder()
    });
}

#[test]
fn a_recorder_that_could_not_read_its_surface_is_not_called_an_unusable_answer() {
    passes_through(Refusal {
        class: crate::outcome::RefusalClass::Unavailable,
        ..rejected_by_the_recorder()
    });
}

#[test]
fn a_recorder_that_found_the_receipt_moved_is_not_called_an_unusable_answer() {
    let moved = Refusal::stale(
        "published-transaction-mismatch",
        "the publication moved",
        crate::outcome::Resolution::no_command(
            crate::outcome::NoCommandReason::WorldAction,
            "re-read the receipt",
        ),
    );
    assert_eq!(moved.class, crate::outcome::RefusalClass::Stale);
    assert!(moved.outcome.is_clean());
    passes_through(moved);
}

#[test]
fn a_clean_refusal_classed_unknown_is_not_called_an_unusable_answer() {
    passes_through(Refusal {
        class: crate::outcome::RefusalClass::Unknown,
        ..rejected_by_the_recorder()
    });
}

#[test]
fn a_refused_launch_never_reaches_the_recorder() {
    let stub = Stub::answering(Ok(b"looks fine to me".to_vec()));
    let (outcome, seen) = recorded(&stub, Recording::Recorded);
    assert_eq!(
        outcome.expect_err("a refusal").code,
        "reviewer-result-malformed"
    );
    assert!(seen.is_empty());
}
