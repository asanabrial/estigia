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

fn rendered_table() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("scripts/measure-transcripts.py"))
        .arg("--projects")
        .arg(root.join("tests/fixtures/usage/projects"))
        .output()
        .expect("python3 runs the measurement script");
    assert!(
        output.status.success(),
        "{:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
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

#[test]
fn the_readable_table_names_the_unknown_class_and_the_floor() {
    let table = rendered_table();
    assert!(table.contains("usage classes"), "{table}");
    assert!(
        table.contains("cache_read unknown"),
        "a class no record carried is shown as unknown, not as a zero:\n{table}"
    );
    assert!(
        table.contains("records missing a counter: 1"),
        "the table states the floor it is reporting under:\n{table}"
    );
    assert!(
        table.contains("sessions naming several issues: 1"),
        "the attribution limit is stated where the figures are:\n{table}"
    );
}

#[test]
fn each_context_reports_its_turns_models_effort_and_tool_calls() {
    let report = measured();
    let repo = report
        .pointer("/by_repo/estigia")
        .expect("the estigia fixture project is measured");
    assert_eq!(
        repo.pointer("/turn_profile/orchestrator"),
        Some(&serde_json::json!({
            "model_turns": 3,
            "served_models": {"claude-opus-5-5": 3},
            "requested_models": {"claude-opus-5-5": 2, "unknown": 1},
            "efforts": {"medium": 2, "unknown": 1},
            "tool_calls": 2
        })),
        "one turn per message identity, a retried fragment's tool call counted once, \
         and a field a record omits counted under unknown"
    );
    assert_eq!(
        repo.pointer("/turn_profile/reviewers"),
        Some(&serde_json::json!({
            "model_turns": 1,
            "served_models": {"claude-sonnet-5-5": 1},
            "requested_models": {"unknown": 1},
            "efforts": {"high": 1},
            "tool_calls": 0
        }))
    );
    assert_eq!(number(repo, "/subagents_launched"), 1);
}

#[test]
fn the_usage_classes_are_split_by_the_model_that_answered() {
    let report = measured();
    assert_eq!(
        report.pointer("/overall/usage_by_model"),
        Some(&serde_json::json!({
            "claude-opus-5-5": {"input": 7, "cache_read": 300, "cache_creation": 7, "output": 11},
            "claude-sonnet-5-5": {"input": 1, "cache_read": 0, "cache_creation": 2, "output": 3},
            "unknown": {"input": 5, "cache_read": null, "cache_creation": null, "output": null}
        })),
        "a record naming no model is measured under unknown, and its missing classes stay null"
    );
}

#[test]
fn each_session_is_listed_with_its_wall_clock_span() {
    let report = measured();
    let sessions = report
        .pointer("/sessions")
        .and_then(Value::as_array)
        .expect("the per-session rows");
    assert_eq!(sessions.len(), 2);
    let alpha = sessions
        .iter()
        .find(|row| row["path"] == "-Users-asanabrial-repo-estigia/alpha.jsonl")
        .expect("the alpha session row");
    assert_eq!(
        alpha["wall_clock_seconds"],
        serde_json::json!(100.0),
        "first record to last, the subagent's transcript included"
    );
    assert_eq!(alpha["subagents_launched"], serde_json::json!(1));
    assert_eq!(alpha["issue"], serde_json::json!(148));
    let thin = sessions
        .iter()
        .find(|row| row["path"] == "-Users-asanabrial-repo-thin/only.jsonl")
        .expect("the thin session row");
    assert!(
        thin["wall_clock_seconds"].is_null(),
        "a session with no timestamp has an unknown span, not a zero one: {thin}"
    );
}

#[test]
fn a_malformed_transcript_is_measured_rather_than_crashing_the_run() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("scripts/measure-transcripts.py"))
        .arg("--projects")
        .arg(root.join("tests/fixtures/usage/malformed"))
        .arg("--json")
        .output()
        .expect("python3 runs the measurement script");
    assert!(
        output.status.success(),
        "a naive timestamp beside an aware one, or a content element that is not an object, \
         stopped the measurement: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("one JSON object");
    let session = &report["sessions"][0];
    assert_eq!(
        session["wall_clock_seconds"],
        serde_json::json!(30.0),
        "a stamp with no offset is read as UTC"
    );
    assert_eq!(
        session["turn_profile"]["orchestrator"]["tool_calls"],
        serde_json::json!(1),
        "the stray element is skipped and the tool call beside it is still counted"
    );
}
