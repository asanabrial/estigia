use super::*;

const PUBLISHED: &str = "2026-11-03T10:00:00Z";

fn published_at() -> i64 {
    crate::transport::ownership::parse_stamp(PUBLISHED).expect("a stamp")
}

fn quiet() -> Pr {
    let mut pr = Pr::open();
    pr.updated = Some(PUBLISHED);
    pr
}

fn workflow_files(names: &[&str]) -> String {
    let object = if names.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::json!({ "entries": names.iter().map(|name| serde_json::json!({ "name": name })).collect::<Vec<_>>() })
    };
    serde_json::json!({ "data": { "repository": { "object": object } } }).to_string()
}

fn script(pr: &Pr, workflows: &[&str]) -> String {
    crate::test_env::answers(&[
        serde_json::json!({ "matches": "pr list", "stdout": r#"[{"number":7}]"# }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"owner":{"login":"example"},"name":"repo"}"#,
        }),
        serde_json::json!({ "matches": "expression=", "stdout": workflow_files(workflows) }),
        serde_json::json!({ "matches": "api graphql", "stdout": graphql(pr) }),
    ])
}

fn read(
    pr: &Pr,
    workflows: &[&str],
    declared: &str,
    seconds_after_publication: i64,
    wait_seconds: u64,
) -> (serde_json::Value, Vec<String>) {
    let (root, context) = rig();
    let bin = crate::test_env::scripted_gh();
    let log = root.path().join("gh.log");
    let stages = stages(declared);
    let answer = crate::test_env::with_logging_gh(bin.path(), &script(pr, workflows), &log, || {
        stages_with_sleep(
            &context,
            &Stages {
                branch: "issue-12",
                stages: &stages,
                ready_by: Party::Agent,
                merge_by: Party::Person,
                request: false,
                expected_head: Some("abc"),
                wait_seconds,
                now_secs: published_at() + seconds_after_publication,
            },
            &mut |_| {},
        )
    })
    .expect("a reading");
    (answer, logged(&log))
}

fn read_with_workflow_answer(mut answer: serde_json::Value) -> Result<serde_json::Value, Failure> {
    let (_root, context) = rig();
    let bin = crate::test_env::scripted_gh();
    answer["matches"] = serde_json::json!("expression=");
    let answers = crate::test_env::answers(&[
        serde_json::json!({ "matches": "pr list", "stdout": r#"[{"number":7}]"# }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"owner":{"login":"example"},"name":"repo"}"#,
        }),
        answer,
        serde_json::json!({ "matches": "api graphql", "stdout": graphql(&quiet()) }),
    ]);
    let stages = stages("none");
    crate::test_env::with_scripted_gh(bin.path(), &answers, || {
        stages_with_sleep(
            &context,
            &Stages {
                branch: "issue-12",
                stages: &stages,
                ready_by: Party::Agent,
                merge_by: Party::Person,
                request: false,
                expected_head: Some("abc"),
                wait_seconds: 5,
                now_secs: published_at() + 600,
            },
            &mut |_| {},
        )
    })
}

#[test]
fn an_unreadable_workflow_listing_refuses_and_never_reads_none() {
    for (case, answer) in [
        (
            "the call fails",
            serde_json::json!({ "status": 1, "stderr": "boom" }),
        ),
        (
            "the answer carries errors",
            serde_json::json!({
                "stdout": r#"{"data":{"repository":{"object":null}},"errors":[{"message":"no"}]}"#,
            }),
        ),
        (
            "the answer names no repository",
            serde_json::json!({ "stdout": r#"{"data":{"repository":null}}"# }),
        ),
    ] {
        let failure = read_with_workflow_answer(answer)
            .expect_err(&format!("{case}: an unreadable head was answered"));
        assert!(
            matches!(failure, Failure::Read(_)),
            "{case}: {}",
            failure.detail()
        );
    }
}

#[test]
fn only_a_yaml_entry_in_the_workflows_directory_is_a_workflow() {
    let (answer, _) = read(&quiet(), &["README.md", "ci.yml.disabled"], "none", 600, 0);
    assert_eq!(answer["ci"], "none", "{answer}");
    for file in ["ci.yml", "ci.yaml"] {
        let (answer, _) = read(&quiet(), &["README.md", file], "none", 600, 0);
        assert_eq!(answer["ci"], "missing", "{file}: {answer}");
    }
}

#[test]
fn an_empty_rollup_with_no_workflow_and_no_check_stage_reads_none_after_the_grace_period() {
    let (answer, _) = read(&quiet(), &[], "none", 121, 10);
    assert_eq!(answer["ci"], "none", "{answer}");
    assert_eq!(answer["wait_timed_out"], false, "{answer}");
    assert!(
        answer["ci_note"]
            .as_str()
            .is_some_and(|note| note.contains("no CI was seen")),
        "{answer}"
    );
}

#[test]
fn an_empty_rollup_inside_the_grace_period_is_still_waited_for_and_asks_nothing_about_workflows() {
    let (answer, calls) = read(&quiet(), &[], "none", 30, 5);
    assert_eq!(answer["ci"], "missing", "{answer}");
    assert_eq!(answer["wait_timed_out"], true, "{answer}");
    assert!(
        !calls.iter().any(|call| call.contains("expression=")),
        "{calls:?}"
    );
}

#[test]
fn the_grace_period_is_counted_through_the_wait() {
    let (answer, _) = read(&quiet(), &[], "none", 118, 10);
    assert_eq!(answer["ci"], "none", "{answer}");
    assert_eq!(answer["wait_timed_out"], false, "{answer}");
}

#[test]
fn an_empty_rollup_with_a_workflow_file_at_the_head_stays_missing_and_never_passes() {
    let (answer, calls) = read(&quiet(), &["ci.yml"], "none", 600, 5);
    assert_eq!(answer["ci"], "missing", "{answer}");
    assert_eq!(answer["wait_timed_out"], true, "{answer}");
    assert!(
        calls.iter().any(|call| call.contains("expression=")),
        "{calls:?}"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.contains("expression="))
            .count(),
        1,
        "the head's workflow files are read once, not on every poll: {calls:?}"
    );
}

#[test]
fn a_declared_check_stage_means_the_rollup_is_never_read_as_none() {
    let (answer, calls) = read(
        &quiet(),
        &[],
        "check:cubic@cubic-dev-ai[bot], review:any",
        600,
        0,
    );
    assert_eq!(answer["ci"], "missing", "{answer}");
    assert!(
        !calls.iter().any(|call| call.contains("expression=")),
        "{calls:?}"
    );
}

#[test]
fn a_reading_without_a_publication_time_is_not_none() {
    let (answer, _) = read(&Pr::open(), &[], "none", 600, 0);
    assert_eq!(answer["ci"], "missing", "{answer}");
}

#[test]
fn a_blocking_check_stage_that_has_not_registered_keeps_the_reading_pending() {
    let pr = quiet().check("unrelated", "COMPLETED", Some("SUCCESS"));
    let (answer, _) = read(&pr, &[], TEAM, 600, 5);
    assert_eq!(answer["ci"], "pending", "{answer}");
    assert_eq!(answer["wait_timed_out"], true, "{answer}");
}

#[test]
fn a_failed_reading_names_the_failing_checks() {
    let pr = quiet()
        .check("build", "COMPLETED", Some("FAILURE"))
        .check("lint", "COMPLETED", Some("SUCCESS"))
        .check("tests", "COMPLETED", Some("TIMED_OUT"));
    let (answer, _) = read(&pr, &[], "none", 0, 0);
    assert_eq!(answer["ci"], "failed", "{answer}");
    assert_eq!(
        answer["failing_checks"],
        serde_json::json!(["build", "tests"])
    );
}

#[test]
fn a_green_reading_names_no_failing_check() {
    let pr = quiet().check("build", "COMPLETED", Some("SUCCESS"));
    let (answer, _) = read(&pr, &[], "none", 0, 0);
    assert_eq!(answer["ci"], "passed", "{answer}");
    assert!(answer.get("failing_checks").is_none(), "{answer}");
}
