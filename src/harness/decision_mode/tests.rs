use serde_json::{Value, json};

use super::super::hook::{Event, Input, run};
use super::*;

fn context(mode: DecisionMode, state_root: &Path) -> GateContext {
    GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: std::path::PathBuf::from("estigia-no-skill-here"),
        repo_dir: std::path::PathBuf::from("estigia-no-repo-here"),
        state_root: state_root.to_path_buf(),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: mode,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: super::super::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn asked() -> Value {
    json!({"questions": [
        {
            "question": "Which storage should the cache use?",
            "header": "Storage",
            "multiSelect": false,
            "options": [
                {"label": "Memory", "description": "fast, lost on restart"},
                {"label": "SQLite (Recommended)", "description": "survives a restart"}
            ]
        },
        {
            "question": "Add a flag?",
            "header": "Flag",
            "multiSelect": false,
            "options": [
                {"label": "No", "description": ""},
                {"label": "Yes", "description": "behind a feature flag"}
            ]
        }
    ]})
}

fn input(tool_name: &str, tool_input: Value) -> Input {
    Input {
        session_id: "abcd1234".to_owned(),
        tool_name: tool_name.to_owned(),
        tool_input,
        ..Input::default()
    }
}

fn stop() -> Input {
    Input {
        session_id: "abcd1234".to_owned(),
        ..Input::default()
    }
}

fn state() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().expect("a state directory");
    let root = directory.path().join("runs");
    (directory, root)
}

#[test]
fn the_marked_option_is_the_answer_and_the_first_one_stands_in_for_a_missing_mark() {
    assert_eq!(
        recommended(&asked()),
        vec![
            TakenDecision {
                question: "Which storage should the cache use?".to_owned(),
                decision: "SQLite — survives a restart".to_owned(),
            },
            TakenDecision {
                question: "Add a flag?".to_owned(),
                decision: "No".to_owned(),
            },
        ]
    );
}

#[test]
fn a_question_under_auto_is_refused_and_names_the_recommended_answer() {
    let (_directory, root) = state();
    let answer = run(
        Event::PreToolUse,
        &input("AskUserQuestion", asked()),
        Some(&context(DecisionMode::Auto, &root)),
    );
    let output = &answer["hookSpecificOutput"];
    assert_eq!(output["permissionDecision"], "deny", "{answer}");
    let reason = output["permissionDecisionReason"]
        .as_str()
        .unwrap_or_default();
    assert!(reason.contains("question-decided-here"), "{reason}");
    assert!(reason.contains("SQLite — survives a restart"), "{reason}");
}

#[test]
fn a_question_under_ask_is_left_to_the_person() {
    let (_directory, root) = state();
    let answer = run(
        Event::PreToolUse,
        &input("AskUserQuestion", asked()),
        Some(&context(DecisionMode::Ask, &root)),
    );
    assert_eq!(answer, json!({}));
    assert_eq!(report(&root, "claude-abcd1234"), None);
}

#[test]
fn the_turn_ends_by_listing_each_decision_once() {
    let (_directory, root) = state();
    let auto = context(DecisionMode::Auto, &root);
    run(
        Event::PreToolUse,
        &input("AskUserQuestion", asked()),
        Some(&auto),
    );

    let first = run(Event::Stop, &stop(), Some(&auto));
    let message = first["systemMessage"].as_str().unwrap_or_default();
    assert!(
        message.contains("1. Which storage should the cache use?")
            && message.contains("→ SQLite — survives a restart")
            && message.contains("2. Add a flag?"),
        "{first}"
    );

    let second = run(Event::Stop, &stop(), Some(&auto));
    assert!(second.get("systemMessage").is_none(), "{second}");
}

#[test]
fn a_stop_that_blocks_keeps_the_decisions_for_the_next_one() {
    let (_directory, root) = state();
    let auto = context(DecisionMode::Auto, &root);
    assert!(record(&root, "claude-abcd1234", &recommended(&asked())));

    let blocked = super::super::hook::with_decisions(
        &auto,
        "claude-abcd1234",
        json!({"decision": "block", "reason": "keep going"}),
    );
    assert!(blocked.get("systemMessage").is_none(), "{blocked}");
    assert!(report(&root, "claude-abcd1234").is_some());
}

#[test]
fn another_tool_is_not_a_question() {
    let (_directory, root) = state();
    let auto = context(DecisionMode::Auto, &root);
    assert!(refuse(&auto, "claude-abcd1234", "Bash", &json!({"command": "ls"})).is_none());
}
