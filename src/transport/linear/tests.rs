//! Tests for the Linear source, against a stand-in `linear` CLI.
//!
//! The stand-in is the `fake_process` example, so the argument list the source
//! actually spawns is what is measured, call by call, from its log.

use crate::identity::Identity;
use crate::transport::How;

use super::{Issue, QUERY, read_issue};

/// Runs `body` with a stand-in `linear` answering `entries`, and answers what
/// it returned beside every argument list the stand-in was asked, in order.
fn through<T>(entries: &[serde_json::Value], body: impl FnOnce() -> T) -> (T, Vec<String>) {
    through_programs(&["linear"], entries, body)
}

fn through_programs<T>(
    programs: &[&str],
    entries: &[serde_json::Value],
    body: impl FnOnce() -> T,
) -> (T, Vec<String>) {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let counts = scratch.path().join("counts.json");
    let log = scratch.path().join("calls.log");
    let answer = crate::test_env::with_counting_logging_gh(
        crate::test_env::fake_cli_directory(programs).path(),
        &crate::test_env::answers(entries),
        &counts,
        &log,
        body,
    );
    let calls = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default().to_owned())
        .collect();
    (answer, calls)
}

fn read_answering(entry: serde_json::Value) -> Result<Issue, crate::transport::Failure> {
    through(&[entry], || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .0
}

fn answering(stdout: &str) -> serde_json::Value {
    serde_json::json!({ "matches": "EstigiaIssue", "stdout": stdout })
}

fn failing(stdout: &str, stderr: &str) -> serde_json::Value {
    serde_json::json!({
        "matches": "EstigiaIssue",
        "status": 1,
        "stdout_exact": stdout,
        "stderr": stderr,
    })
}

fn a_reason(failure: &crate::transport::Failure) -> String {
    match failure {
        crate::transport::Failure::Stop(payload) => payload
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        other => panic!("expected a stop, got {other:?}"),
    }
}

fn a_detail(failure: &crate::transport::Failure) -> String {
    match failure {
        crate::transport::Failure::Stop(payload) => payload
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        other => panic!("expected a stop, got {other:?}"),
    }
}

#[test]
fn a_read_spawns_linear_api_with_the_query_and_sends_no_mutation() {
    let (read, calls) = through(
        &[answering(
            "{\"data\":{\"issue\":{\"title\":\"Add the thing\",\"description\":\"As described\"}}}",
        )],
        || read_issue(&Identity::Key("ENG-123".to_owned())),
    );
    assert_eq!(
        read.expect("the issue is read"),
        Issue {
            title: "Add the thing".to_owned(),
            description: "As described".to_owned(),
        }
    );
    assert_eq!(
        calls,
        vec![format!(
            "api --variables-json {{\"id\":\"ENG-123\"}} {QUERY}"
        )],
        "one call, and it is the read"
    );
    assert!(!calls[0].contains("mutation"), "a mutation was sent");
}

#[test]
fn a_missing_cli_is_named_as_missing() {
    let failure = super::request(
        "estigia-no-such-linear-cli",
        QUERY,
        &serde_json::json!({ "id": "ENG-123" }),
        How::read(),
    )
    .expect_err("no such program");
    assert_eq!(a_reason(&failure), "linear-cli-missing");
}

#[test]
fn a_cli_with_no_credential_is_a_missing_credential() {
    let failure = read_answering(failing(
        "",
        "✗ API request failed: No API key configured\n  Set LINEAR_API_KEY, add api_key to \
         .linear.toml, or run `linear auth login`.",
    ))
    .expect_err("no credential");
    assert_eq!(a_reason(&failure), "linear-credential-missing");
}

#[test]
fn an_authentication_error_on_stdout_is_a_rejected_credential() {
    let failure = read_answering(failing(
        "{\"errors\":[{\"message\":\"Authentication required, not authenticated\",\
         \"extensions\":{\"type\":\"authentication error\"}}]}",
        "",
    ))
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn an_authentication_error_on_stderr_is_a_rejected_credential() {
    let failure = read_answering(failing(
        "",
        "{\"errors\":[{\"message\":\"Invalid API key\",\
         \"extensions\":{\"type\":\"authentication error\"}}]}",
    ))
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn an_authentication_error_in_a_successful_exit_is_a_rejected_credential() {
    let failure = read_answering(answering(
        "{\"errors\":[{\"message\":\"Authentication required\",\
         \"extensions\":{\"type\":\"authentication error\"}}],\"data\":null}",
    ))
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn an_unknown_key_is_named() {
    let failure =
        read_answering(answering("{\"data\":{\"issue\":null}}")).expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
}

#[test]
fn a_not_found_error_from_a_failed_exit_is_an_unknown_key_naming_it() {
    let failure = read_answering(failing(
        "{\"errors\":[{\"message\":\"Entity not found: Issue\",\
         \"extensions\":{\"type\":\"invalid input\"}}],\"data\":null}",
        "",
    ))
    .expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
    assert!(
        a_detail(&failure).contains("ENG-123"),
        "{}",
        a_detail(&failure)
    );
}

#[test]
fn a_not_found_error_beside_a_null_issue_is_an_unknown_key() {
    let failure = read_answering(answering(
        "{\"data\":{\"issue\":null},\"errors\":[{\"message\":\"Entity not found: Issue\",\
         \"extensions\":{\"type\":\"invalid input\"}}]}",
    ))
    .expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
}

#[test]
fn any_other_error_with_no_data_is_unreachable_and_names_the_message() {
    let failure = read_answering(failing(
        "",
        "{\"errors\":[{\"message\":\"rate limit exceeded\"}],\"data\":null}",
    ))
    .expect_err("Linear answered an error");
    assert_eq!(a_reason(&failure), "linear-unreachable");
    assert!(
        a_detail(&failure).contains("rate limit exceeded"),
        "the message was dropped: {}",
        a_detail(&failure)
    );
}

#[test]
fn a_failed_exit_with_no_json_is_unreachable_and_names_the_cli_output() {
    let failure = read_answering(failing("", "✗ API request failed: fetch failed"))
        .expect_err("Linear could not be reached");
    assert_eq!(a_reason(&failure), "linear-unreachable");
    assert!(
        a_detail(&failure).contains("fetch failed"),
        "{}",
        a_detail(&failure)
    );
}

#[test]
fn a_successful_exit_that_is_not_json_is_unreachable() {
    let failure = read_answering(answering("not json")).expect_err("not Linear's answer");
    assert_eq!(a_reason(&failure), "linear-unreachable");
}

#[test]
fn a_completed_or_canceled_issue_is_not_open() {
    for kind in ["completed", "canceled"] {
        let failure = read_answering(answering(&format!(
            "{{\"data\":{{\"issue\":{{\"title\":\"T\",\"description\":\"D\",\
             \"state\":{{\"type\":\"{kind}\"}}}}}}}}"
        )))
        .expect_err("a finished issue was read as workable");
        assert_eq!(a_reason(&failure), "issue-not-open", "{kind}");
    }
}

#[test]
fn a_started_issue_is_read() {
    let read = read_answering(answering(
        "{\"data\":{\"issue\":{\"title\":\"T\",\"description\":\"D\",\
         \"state\":{\"type\":\"started\"}}}}",
    ))
    .expect("an open issue is read");
    assert_eq!(read.title, "T");
    assert!(QUERY.contains("state { type }"));
}

fn states_in(current: &str) -> String {
    serde_json::json!({ "data": { "issue": {
        "id": "uuid-123",
        "state": { "name": current },
        "team": { "states": { "nodes": [
            { "id": "s-todo", "name": "Todo" },
            { "id": "s-doing", "name": "In Progress" },
            { "id": "s-review", "name": "In Review" },
            { "id": "s-done", "name": "Done" },
        ]}},
    }}})
    .to_string()
}

const ACCEPTED: &str = "{\"data\":{\"issueUpdate\":{\"success\":true}}}";

/// The states read before a move, the move's answer, and the states read back.
fn a_move(before: &str, answer: serde_json::Value, after: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({ "matches": "EstigiaStates", "nth": 1, "stdout": states_in(before) }),
        serde_json::json!({ "matches": "EstigiaStates", "nth": 2, "stdout": states_in(after) }),
        answer,
    ]
}

fn accepted() -> serde_json::Value {
    serde_json::json!({ "matches": "EstigiaMove", "stdout": ACCEPTED })
}

fn moved(
    entries: &[serde_json::Value],
    target: &str,
) -> (
    Result<serde_json::Value, crate::transport::Failure>,
    Vec<String>,
) {
    through(entries, || {
        super::move_issue(&Identity::Key("ENG-123".to_owned()), target)
    })
}

fn mutations(calls: &[String]) -> Vec<&String> {
    calls
        .iter()
        .filter(|call| call.contains("mutation"))
        .collect()
}

#[test]
fn a_mapped_state_is_moved_with_one_mutation_and_read_back() {
    let (answer, calls) = moved(&a_move("Todo", accepted(), "In Progress"), "in progress");
    let answer = answer.expect("moved");
    assert_eq!(answer["moved"], true);
    assert_eq!(answer["from"], "Todo");
    assert_eq!(answer["state"], "In Progress");
    let sent = mutations(&calls);
    assert_eq!(sent.len(), 1, "{calls:?}");
    assert_eq!(
        sent[0],
        &format!(
            "api --variables-json {{\"id\":\"uuid-123\",\"stateId\":\"s-doing\"}} {}",
            super::MOVE_MUTATION
        )
    );
    assert_eq!(calls.len(), 3, "states, the move, states again: {calls:?}");
}

#[test]
fn an_issue_already_in_the_state_is_not_moved() {
    let (answer, calls) = moved(&a_move("In Review", accepted(), "In Review"), "In Review");
    assert_eq!(answer.expect("already there")["moved"], false);
    assert!(mutations(&calls).is_empty(), "{calls:?}");
}

#[test]
fn a_state_the_team_does_not_have_is_named_and_nothing_is_moved() {
    let (answer, calls) = moved(&a_move("Todo", accepted(), "Todo"), "QA");
    assert_eq!(
        a_reason(&answer.expect_err("no such state")),
        "linear-state-unknown"
    );
    assert!(mutations(&calls).is_empty(), "{calls:?}");
}

#[test]
fn a_move_that_does_not_read_back_is_not_reported_moved() {
    let (answer, _) = moved(&a_move("Todo", accepted(), "Todo"), "Done");
    assert_eq!(
        a_reason(&answer.expect_err("the read-back disagrees")),
        "linear-move-not-read-back"
    );
}

#[test]
fn a_refused_move_is_named_as_refused() {
    let refused = serde_json::json!({
        "matches": "EstigiaMove",
        "status": 1,
        "stdout_exact": "{\"data\":{\"issueUpdate\":null},\"errors\":[{\"message\":\"forbidden\"}]}",
    });
    let (answer, _) = moved(&a_move("Todo", refused, "Todo"), "Done");
    let failure = answer.expect_err("Linear refused it");
    assert_eq!(a_reason(&failure), "linear-move-refused");
    assert!(
        a_detail(&failure).contains("forbidden"),
        "{}",
        a_detail(&failure)
    );
}

#[test]
fn a_move_whose_answer_cannot_be_read_is_an_unknown_world() {
    let unreadable = serde_json::json!({
        "matches": "EstigiaMove",
        "status": 1,
        "stderr": "✗ API request failed: socket hang up",
    });
    let (answer, _) = moved(&a_move("Todo", unreadable, "Todo"), "Done");
    let failure = answer.expect_err("an unreadable answer to a sent move");
    assert_eq!(a_reason(&failure), "linear-move-unconfirmed");
    assert_eq!(failure.envelope()["world"], "unknown");
}

#[test]
fn follow_state_moves_only_a_mapped_state_of_a_linear_issue() {
    let names = vec![("review".to_owned(), "In Review".to_owned())];
    let answer = serde_json::json!({ "ok": true });
    let key = Identity::Key("ENG-123".to_owned());
    let (unmapped, calls) = through(&[], || {
        super::follow_state(&names, Some(&key), "done", answer.clone())
    });
    assert_eq!(unmapped, answer, "a state nobody mapped moved something");
    assert!(calls.is_empty(), "{calls:?}");
    let (github, calls) = through(&[], || {
        super::follow_state(&names, Some(&Identity::Number(7)), "review", answer.clone())
    });
    assert_eq!(github, answer, "a GitHub issue was sent to Linear");
    assert!(calls.is_empty(), "{calls:?}");

    let (followed, _) = through(&a_move("In Progress", accepted(), "In Review"), || {
        super::follow_state(&names, Some(&key), "review", answer.clone())
    });
    assert_eq!(followed["ok"], true);
    assert_eq!(followed["linear_state"]["state"], "In Review");

    let (reported, _) = through(&a_move("In Progress", accepted(), "In Progress"), || {
        super::follow_state(&names, Some(&key), "review", answer.clone())
    });
    assert_eq!(
        reported["linear_state"]["reason"], "linear-move-not-read-back",
        "a move that failed was not reported beside the answer"
    );
}

#[test]
fn the_states_follow_state_is_called_with_are_ones_state_names_maps() {
    for state in [super::STARTED, super::IN_REVIEW, super::DELIVERED] {
        assert!(
            crate::config::MAPPED_STATES.contains(&state),
            "{state} is moved to and `State names` cannot map it"
        );
    }
}

fn merged_pull_request() -> String {
    serde_json::json!({ "data": { "repository": { "pullRequest": {
        "number": 7, "state": "MERGED", "isDraft": false, "headRefOid": "abc", "headRefName": "eng-123",
        "mergeCommit": { "oid": "m1" }, "author": { "login": "alexis" },
        "reviewRequests": { "pageInfo": { "hasNextPage": false }, "nodes": [] },
        "latestReviews": { "pageInfo": { "hasNextPage": false }, "nodes": [] },
        "reviewThreads": { "pageInfo": { "hasNextPage": false }, "nodes": [] },
        "commits": { "nodes": [] },
    }}}})
    .to_string()
}

fn linear_repository(root: &std::path::Path) -> crate::transport::Context {
    let skill_dir = root.join("skill");
    let repo_dir = root.join("repo");
    std::fs::create_dir_all(&skill_dir).expect("a skill root");
    std::fs::create_dir_all(repo_dir.join(".git")).expect("a checkout");
    std::fs::write(
        skill_dir.join(crate::skill::CONTRACT),
        crate::skill::CONTRACT_CONTENTS,
    )
    .expect("a contract");
    let mut config = crate::config::Config::default();
    for (setting, value) in [
        (crate::config::Setting::Tracker, "linear ENG"),
        (crate::config::Setting::DeliveryPreset, "team"),
        (crate::config::Setting::StateNames, "done=Done"),
    ] {
        setting.apply(&mut config, value).expect("a row");
    }
    crate::setup::write_repository_configuration(
        &crate::skill::repository_config_path(&repo_dir),
        &config,
        &[
            crate::config::Setting::Tracker,
            crate::config::Setting::Workflow,
            crate::config::Setting::StateNames,
            crate::config::Setting::SelfReview,
            crate::config::Setting::ReadyBy,
            crate::config::Setting::CorrectionsAfterReady,
            crate::config::Setting::MergeBy,
        ],
    )
    .expect("the repository's file");
    crate::transport::Context {
        skill_dir,
        repo_dir,
        config: Vec::new(),
        repo: None,
    }
}

fn stages_through_dispatch(
    context: &crate::transport::Context,
    issue: &str,
) -> (
    Result<serde_json::Value, crate::transport::Failure>,
    Vec<String>,
) {
    let mut entries = vec![
        serde_json::json!({ "matches": "pr list", "stdout": r#"[{"number":7}]"# }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"owner":{"login":"example"},"name":"repo"}"#,
        }),
        serde_json::json!({ "matches": "api graphql", "stdout": merged_pull_request() }),
    ];
    entries.extend(a_move("In Review", accepted(), "Done"));
    let flags: Vec<String> = ["--branch", "eng-123", "--issue", issue]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    through_programs(&["gh", "linear"], &entries, || {
        crate::transport::dispatch::dispatch(
            context,
            "review-stages",
            &flags,
            "2026-10-25T00:00:00Z",
        )
    })
}

fn linear_calls(calls: &[String]) -> Vec<&String> {
    calls
        .iter()
        .filter(|call| call.starts_with("api --variables-json"))
        .collect()
}

#[test]
fn reading_the_merge_through_dispatch_moves_the_linear_issue_to_its_done_name() {
    let root = tempfile::tempdir().expect("a root");
    let context = linear_repository(root.path());
    let (answer, calls) = stages_through_dispatch(&context, "ENG-123");
    let answer = answer.expect("merged");
    assert_eq!(answer["position"], "merged");
    assert_eq!(answer["linear_state"]["state"], "Done", "{answer}");
    assert_eq!(mutations(&calls).len(), 1, "{calls:?}");
}

#[test]
fn a_github_issue_read_merged_through_dispatch_sends_nothing_to_linear() {
    let root = tempfile::tempdir().expect("a root");
    let context = linear_repository(root.path());
    let (answer, calls) = stages_through_dispatch(&context, "7");
    let answer = answer.expect("merged");
    assert_eq!(answer["position"], "merged");
    assert!(answer.get("linear_state").is_none(), "{answer}");
    assert!(linear_calls(&calls).is_empty(), "{calls:?}");
}
