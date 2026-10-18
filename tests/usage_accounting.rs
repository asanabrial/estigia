// An assertion that panics is the assertion working. See `tests/pipe.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The transcript accounting, over fixtures whose totals are known by hand.
//!
//! The classes are reported separately and an absent counter stays unknown: one
//! fixture omits three of the four, and the other repeats a message identity so
//! a retried fragment cannot be counted twice. `python3` is the interpreter the
//! measurement is written in, so its absence fails here rather than skipping.

#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn measured() -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("scripts/measure-transcripts.py"))
        .arg("--projects")
        .arg(root.join("tests/fixtures/usage/projects"))
        .arg("--json")
        .output()
        .expect("python3 runs the measurement script");
    assert!(
        output.status.success(),
        "{:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the measurement answers one JSON object")
}

fn number(report: &Value, pointer: &str) -> i64 {
    report
        .pointer(pointer)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("{pointer} is not a number in {report}"))
}

fn unknown(report: &Value, pointer: &str) {
    assert!(
        report.pointer(pointer).is_some_and(Value::is_null),
        "{pointer} is not null in {report}"
    );
}

fn classes(report: &Value, context: &str) -> Value {
    report
        .pointer(&format!("/usage/tokens/{context}"))
        .cloned()
        .unwrap_or_else(|| panic!("/usage/tokens/{context} is absent from {report}"))
}

#[test]
fn every_usage_class_is_totalled_for_the_context_that_carried_it() {
    let report = measured();
    let repo = report
        .pointer("/by_repo/estigia")
        .expect("the estigia fixture project is measured");
    assert_eq!(
        classes(repo, "orchestrator"),
        serde_json::json!({"input": 7, "cache_read": 300, "cache_creation": 7, "output": 11}),
        "the orchestrator's classes are the two records' own counters"
    );
    assert_eq!(
        classes(repo, "reviewers"),
        serde_json::json!({"input": 1, "cache_read": 0, "cache_creation": 2, "output": 3}),
        "a subagent's transcript is measured under the agentType its .meta.json records"
    );
    assert_eq!(
        number(repo, "/usage/records"),
        3,
        "three records carried usage"
    );
    assert_eq!(number(repo, "/usage/records_missing_a_counter"), 0);
    assert_eq!(number(repo, "/session_count"), 1);
}

#[test]
fn a_repeated_message_identity_is_counted_once() {
    let report = measured();
    let repo = report
        .pointer("/by_repo/estigia")
        .expect("the estigia fixture project is measured");
    assert_eq!(
        number(repo, "/context_input_tokens/orchestrator"),
        314,
        "3 + 100 + 7 and 4 + 200 + 0, not the retried fragment's 900s"
    );
    assert_eq!(
        classes(repo, "orchestrator")["input"],
        serde_json::json!(7),
        "a second record under one message id does not add to any class"
    );
    assert_eq!(
        repo.pointer("/per_issue_input_tokens"),
        Some(&serde_json::json!({"148": 314})),
        "the session's dominant issue carries its orchestrator context"
    );
    assert_eq!(number(repo, "/sessions_with_several_issues"), 1);
}

#[test]
fn a_counter_no_record_carried_is_unknown_and_never_zero() {
    let report = measured();
    let repo = report
        .pointer("/by_repo/thin")
        .expect("the thin fixture project is measured");
    assert_eq!(classes(repo, "orchestrator")["input"], serde_json::json!(5));
    for class in ["cache_read", "cache_creation", "output"] {
        unknown(repo, &format!("/usage/tokens/orchestrator/{class}"));
    }
    assert_eq!(number(repo, "/usage/records"), 1);
    assert_eq!(
        number(repo, "/usage/records_missing_a_counter"),
        1,
        "the one record lacking a counter is counted, so the total reads as a floor"
    );
    assert_eq!(number(repo, "/sessions_with_several_issues"), 0);
}

#[test]
fn the_overall_figures_are_the_two_projects_folded_together() {
    let report = measured();
    let overall = report.pointer("/overall").expect("the overall summary");
    assert_eq!(
        classes(overall, "orchestrator"),
        serde_json::json!({"input": 12, "cache_read": 300, "cache_creation": 7, "output": 11}),
        "a class one project observed is observed overall, and 5 more input joins it"
    );
    assert_eq!(
        classes(overall, "reviewers")["output"],
        serde_json::json!(3)
    );
    assert_eq!(number(overall, "/usage/records"), 4);
    assert_eq!(number(overall, "/usage/records_missing_a_counter"), 1);
}
