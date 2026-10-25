//! Tests for the read-only Linear source, against a fake endpoint.
//!
//! The fake is a real local HTTP server, so the request `curl` actually sends
//! is what is measured — not a stand-in for `curl`. Every test asserts the one
//! thing this source promises: it reads title and description with a query and
//! sends no mutation at all.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use crate::identity::Identity;
use crate::test_env::with_linear_env;

use super::{Issue, read_issue};

/// A fake Linear endpoint that answers every request with one status and body,
/// recording each request body it is sent.
struct Endpoint {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    _thread: std::thread::JoinHandle<()>,
}

impl Endpoint {
    fn answering(status: u16, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = listener.local_addr().expect("an address").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let body = body.to_owned();
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let Some(request) = read_request(&mut stream) else {
                    continue;
                };
                // Any mutation is answered as a failure, so a source that sent
                // one would not read an issue at all.
                let (status, body) = if request.contains("mutation") {
                    (
                        400,
                        "{\"errors\":[{\"message\":\"a mutation is not admitted\"}]}",
                    )
                } else {
                    (status, body.as_str())
                };
                seen.lock().expect("the request log").push(request);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}/graphql"),
            requests,
            _thread: thread,
        }
    }

    fn in_turn(answers: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
        let port = listener.local_addr().expect("an address").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let answers: Vec<String> = answers.iter().map(|body| (*body).to_owned()).collect();
        let thread = std::thread::spawn(move || {
            for (turn, stream) in listener.incoming().enumerate() {
                let Ok(mut stream) = stream else { break };
                let Some(request) = read_request(&mut stream) else {
                    continue;
                };
                seen.lock().expect("the request log").push(request);
                let body = answers
                    .get(turn)
                    .or_else(|| answers.last())
                    .cloned()
                    .unwrap_or_default();
                let response = format!(
                    "HTTP/1.1 200\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self {
            url: format!("http://127.0.0.1:{port}/graphql"),
            requests,
            _thread: thread,
        }
    }

    fn sent(&self) -> Vec<String> {
        self.requests.lock().expect("the request log").clone()
    }
}

/// Reads one HTTP request's body, or `None` if the connection closed first.
fn read_request(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(at) = find(&buffer, b"\r\n\r\n") {
            break at + 4;
        }
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    while buffer.len() < header_end + length {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Some(String::from_utf8_lossy(&buffer[header_end..]).into_owned())
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
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

#[test]
fn a_query_reads_the_title_and_description_and_sends_no_mutation() {
    let endpoint = Endpoint::answering(
        200,
        "{\"data\":{\"issue\":{\"title\":\"Add the thing\",\"description\":\"As described\"}}}",
    );
    let read = with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .expect("the issue is read");
    assert_eq!(
        read,
        Issue {
            title: "Add the thing".to_owned(),
            description: "As described".to_owned(),
        }
    );
    let sent = endpoint.sent();
    assert_eq!(sent.len(), 1, "one request, and it is the read");
    assert!(
        sent[0].contains("ENG-123"),
        "the key was not sent: {}",
        sent[0]
    );
    assert!(
        sent[0].contains("query"),
        "the operation is not a query: {}",
        sent[0]
    );
    assert!(
        !sent[0].contains("mutation"),
        "a mutation was sent: {}",
        sent[0]
    );
}

#[test]
fn a_missing_credential_is_refused_before_any_request() {
    let endpoint = Endpoint::answering(200, "{}");
    let failure = with_linear_env(None, &endpoint.url, || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .expect_err("no credential");
    assert_eq!(a_reason(&failure), "linear-credential-missing");
    assert!(endpoint.sent().is_empty(), "a request was sent anyway");
}

#[test]
fn a_rejected_credential_is_named() {
    let endpoint = Endpoint::answering(401, "{\"errors\":[{\"message\":\"bad key\"}]}");
    let failure = with_linear_env(Some("lin_api_bad"), &endpoint.url, || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn an_unknown_key_is_named() {
    let endpoint = Endpoint::answering(200, "{\"data\":{\"issue\":null}}");
    let failure = with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        read_issue(&Identity::Key("ENG-999".to_owned()))
    })
    .expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
}

#[test]
fn an_unreachable_api_is_named() {
    let failure = with_linear_env(Some("lin_api_test"), "http://127.0.0.1:1/graphql", || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .expect_err("nothing is listening");
    assert_eq!(a_reason(&failure), "linear-unreachable");
}

#[test]
fn the_staged_config_carries_no_body_path_for_curl_to_unescape() {
    let staged = super::Staged::write("http://127.0.0.1:1/graphql", "lin_api_test", "{}")
        .expect("the request is staged");
    let config = std::fs::read_to_string(&staged.config).expect("the staged config");
    let body = staged.body.to_string_lossy().into_owned();
    drop(staged);
    assert!(
        !config.contains("data-binary") && !config.contains(&body),
        "the body path is in the curl config, where a backslash is unescaped: {config}"
    );
}

#[test]
fn the_body_argument_keeps_a_windows_path_verbatim() {
    let path = std::path::Path::new(r"C:\Users\ana\AppData\Local\Temp\body.json");
    assert_eq!(
        super::body_argument(path),
        r"@C:\Users\ana\AppData\Local\Temp\body.json"
    );
}

#[test]
fn the_curl_arguments_ignore_curlrc_and_carry_a_deadline() {
    let arguments = super::curl_arguments(
        std::path::Path::new("curl.conf"),
        std::path::Path::new("body.json"),
    );
    assert_eq!(arguments.first().map(String::as_str), Some("curl"));
    assert_eq!(arguments.get(1).map(String::as_str), Some("-q"));
    for flag in ["--connect-timeout", "--max-time"] {
        assert!(
            arguments.iter().any(|a| a == flag),
            "no {flag}: {arguments:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_staging_directory_that_already_exists_is_refused_not_reused() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!("estigia-linear-squat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).expect("a pre-created directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).expect("0755");
    let refused = super::create_private_dir(&dir);
    let mode = std::fs::metadata(&dir)
        .expect("the directory")
        .permissions()
        .mode()
        & 0o777;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(refused.is_err(), "a pre-existing directory was reused");
    assert_eq!(mode, 0o755, "the refused directory was altered");
}

#[cfg(unix)]
#[test]
fn a_fresh_staging_directory_is_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = std::env::temp_dir().join(format!("estigia-linear-fresh-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::create_private_dir(&dir).expect("a fresh directory");
    let mode = std::fs::metadata(&dir)
        .expect("the directory")
        .permissions()
        .mode()
        & 0o777;
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(mode, 0o700);
}

fn read_through(status: u16, body: &str) -> Result<Issue, crate::transport::Failure> {
    let endpoint = Endpoint::answering(status, body);
    with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
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
fn an_authentication_error_inside_a_200_answer_is_a_rejected_credential() {
    let failure = read_through(
        200,
        "{\"errors\":[{\"message\":\"Authentication required\",\
         \"extensions\":{\"type\":\"authentication error\"}}],\"data\":null}",
    )
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn an_authentication_error_inside_a_400_answer_is_a_rejected_credential() {
    let failure = read_through(
        400,
        "{\"errors\":[{\"message\":\"Invalid API key\",\
         \"extensions\":{\"type\":\"authentication error\"}}]}",
    )
    .expect_err("the credential was refused");
    assert_eq!(a_reason(&failure), "linear-credential-rejected");
}

#[test]
fn any_other_error_with_no_data_is_unreachable_and_names_the_message() {
    let failure = read_through(
        200,
        "{\"errors\":[{\"message\":\"rate limit exceeded\"}],\"data\":null}",
    )
    .expect_err("Linear answered an error");
    assert_eq!(a_reason(&failure), "linear-unreachable");
    assert!(
        a_detail(&failure).contains("rate limit exceeded"),
        "the message was dropped: {}",
        a_detail(&failure)
    );
}

#[test]
fn a_completed_or_canceled_issue_is_not_open() {
    for kind in ["completed", "canceled"] {
        let failure = read_through(
            200,
            &format!(
                "{{\"data\":{{\"issue\":{{\"title\":\"T\",\"description\":\"D\",\
                 \"state\":{{\"type\":\"{kind}\"}}}}}}}}"
            ),
        )
        .expect_err("a finished issue was read as workable");
        assert_eq!(a_reason(&failure), "issue-not-open", "{kind}");
    }
}

#[test]
fn a_started_issue_is_read_and_its_state_is_asked_for() {
    let endpoint = Endpoint::answering(
        200,
        "{\"data\":{\"issue\":{\"title\":\"T\",\"description\":\"D\",\
         \"state\":{\"type\":\"started\"}}}}",
    );
    let read = with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        read_issue(&Identity::Key("ENG-123".to_owned()))
    })
    .expect("an open issue is read");
    assert_eq!(read.title, "T");
    assert!(
        endpoint.sent()[0].contains("state"),
        "the query did not ask for the state: {}",
        endpoint.sent()[0]
    );
}

#[test]
fn a_staged_request_is_removed_when_it_goes_out_of_scope() {
    let staged = super::Staged::write("http://127.0.0.1:1/graphql", "lin_api_test", "{}")
        .expect("the request is staged");
    let dir = staged.config.parent().expect("a directory").to_path_buf();
    assert!(dir.is_dir());
    drop(staged);
    assert!(!dir.exists(), "the staging directory outlived its request");
}

#[test]
fn a_staging_that_fails_after_the_credential_is_written_leaves_nothing_behind() {
    let dir = std::env::temp_dir().join(format!("estigia-linear-failed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let failed = super::Staged::write_in(
        &dir,
        "http://127.0.0.1:1/graphql",
        "lin_api_secret",
        "{}",
        |_| Err(std::io::Error::other("narrowing failed")),
    );
    let left = dir.exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failed.is_err());
    assert!(!left, "a credential file was left on disk");
}

#[test]
fn a_not_found_error_beside_a_null_issue_is_an_unknown_key() {
    let failure = read_through(
        200,
        "{\"data\":{\"issue\":null},\"errors\":[{\"message\":\"Entity not found: Issue\",\
         \"extensions\":{\"type\":\"invalid input\"}}]}",
    )
    .expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
}

#[test]
fn a_not_found_error_with_no_data_is_an_unknown_key_naming_it() {
    let failure = read_through(
        200,
        "{\"errors\":[{\"message\":\"Entity not found: Issue\",\
         \"extensions\":{\"type\":\"invalid input\"}}]}",
    )
    .expect_err("no such issue");
    assert_eq!(a_reason(&failure), "linear-issue-unknown");
    assert!(
        a_detail(&failure).contains("ENG-123"),
        "{}",
        a_detail(&failure)
    );
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

fn moved(
    endpoint: &Endpoint,
    target: &str,
) -> Result<serde_json::Value, crate::transport::Failure> {
    with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        super::move_issue(&Identity::Key("ENG-123".to_owned()), target)
    })
}

#[test]
fn a_mapped_state_is_moved_with_one_mutation_and_read_back() {
    let endpoint = Endpoint::in_turn(&[&states_in("Todo"), ACCEPTED, &states_in("In Progress")]);
    let answer = moved(&endpoint, "in progress").expect("moved");
    assert_eq!(answer["moved"], true);
    assert_eq!(answer["from"], "Todo");
    assert_eq!(answer["state"], "In Progress");
    let sent = endpoint.sent();
    let mutations: Vec<&String> = sent
        .iter()
        .filter(|body| body.contains("mutation"))
        .collect();
    assert_eq!(mutations.len(), 1, "{sent:?}");
    assert!(mutations[0].contains("s-doing") && mutations[0].contains("uuid-123"));
}

#[test]
fn an_issue_already_in_the_state_is_not_moved() {
    let endpoint = Endpoint::in_turn(&[&states_in("In Review")]);
    let answer = moved(&endpoint, "In Review").expect("already there");
    assert_eq!(answer["moved"], false);
    assert!(!endpoint.sent().iter().any(|body| body.contains("mutation")));
}

#[test]
fn a_state_the_team_does_not_have_is_named_and_nothing_is_moved() {
    let endpoint = Endpoint::in_turn(&[&states_in("Todo")]);
    let failure = moved(&endpoint, "QA").expect_err("no such state");
    assert_eq!(a_reason(&failure), "linear-state-unknown");
    assert!(!endpoint.sent().iter().any(|body| body.contains("mutation")));
}

#[test]
fn a_move_that_does_not_read_back_is_not_reported_moved() {
    let endpoint = Endpoint::in_turn(&[&states_in("Todo"), ACCEPTED, &states_in("Todo")]);
    let failure = moved(&endpoint, "Done").expect_err("the read-back disagrees");
    assert_eq!(a_reason(&failure), "linear-move-not-read-back");
}

#[test]
fn a_refused_move_is_named_as_refused() {
    let endpoint = Endpoint::in_turn(&[
        &states_in("Todo"),
        "{\"data\":{\"issueUpdate\":null},\"errors\":[{\"message\":\"forbidden\"}]}",
    ]);
    let failure = moved(&endpoint, "Done").expect_err("Linear refused it");
    assert_eq!(a_reason(&failure), "linear-move-refused");
}

#[test]
fn follow_state_moves_only_a_mapped_state_of_a_linear_issue() {
    let names = vec![("review".to_owned(), "In Review".to_owned())];
    let answer = serde_json::json!({ "ok": true });
    let unmapped = super::follow_state(
        &names,
        Some(&Identity::Key("ENG-123".to_owned())),
        "done",
        answer.clone(),
    );
    assert_eq!(unmapped, answer, "a state nobody mapped moved something");
    let github = super::follow_state(&names, Some(&Identity::Number(7)), "review", answer.clone());
    assert_eq!(github, answer, "a GitHub issue was sent to Linear");

    let endpoint =
        Endpoint::in_turn(&[&states_in("In Progress"), ACCEPTED, &states_in("In Review")]);
    let followed = with_linear_env(Some("lin_api_test"), &endpoint.url, || {
        super::follow_state(
            &names,
            Some(&Identity::Key("ENG-123".to_owned())),
            "review",
            answer.clone(),
        )
    });
    assert_eq!(followed["ok"], true);
    assert_eq!(followed["linear_state"]["state"], "In Review");

    let failing = Endpoint::in_turn(&[
        &states_in("In Progress"),
        ACCEPTED,
        &states_in("In Progress"),
    ]);
    let reported = with_linear_env(Some("lin_api_test"), &failing.url, || {
        super::follow_state(
            &names,
            Some(&Identity::Key("ENG-123".to_owned())),
            "review",
            answer.clone(),
        )
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

#[test]
fn a_move_whose_answer_cannot_be_read_is_an_unknown_world() {
    let endpoint = Endpoint::in_turn(&[&states_in("Todo"), "not json"]);
    let failure = moved(&endpoint, "Done").expect_err("an unreadable answer to a sent move");
    assert_eq!(a_reason(&failure), "linear-move-unconfirmed");
    assert_eq!(failure.envelope()["world"], "unknown");
}

fn merged_pull_request() -> String {
    serde_json::json!({ "data": { "repository": { "pullRequest": {
        "number": 7, "state": "MERGED", "isDraft": false, "headRefOid": "abc",
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
    linear: &str,
) -> Result<serde_json::Value, crate::transport::Failure> {
    let bin = crate::test_env::scripted_gh();
    let answers = crate::test_env::answers(&[
        serde_json::json!({ "matches": "pr list", "stdout": r#"[{"number":7}]"# }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"owner":{"login":"example"},"name":"repo"}"#,
        }),
        serde_json::json!({ "matches": "api graphql", "stdout": merged_pull_request() }),
    ]);
    let flags: Vec<String> = ["--branch", "eng-123", "--issue", issue]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    with_linear_env(Some("lin_api_test"), linear, || {
        crate::test_env::with_scripted_gh(bin.path(), &answers, || {
            crate::transport::dispatch::dispatch(
                context,
                "review-stages",
                &flags,
                "2026-10-25T00:00:00Z",
            )
        })
    })
}

#[test]
fn reading_the_merge_through_dispatch_moves_the_linear_issue_to_its_done_name() {
    let root = tempfile::tempdir().expect("a root");
    let context = linear_repository(root.path());
    let endpoint = Endpoint::in_turn(&[&states_in("In Review"), ACCEPTED, &states_in("Done")]);
    let answer = stages_through_dispatch(&context, "ENG-123", &endpoint.url).expect("merged");
    assert_eq!(answer["position"], "merged");
    assert_eq!(answer["linear_state"]["state"], "Done", "{answer}");
    assert_eq!(
        endpoint
            .sent()
            .iter()
            .filter(|body| body.contains("mutation"))
            .count(),
        1
    );
}

#[test]
fn a_github_issue_read_merged_through_dispatch_sends_nothing_to_linear() {
    let root = tempfile::tempdir().expect("a root");
    let context = linear_repository(root.path());
    let endpoint = Endpoint::in_turn(&[&states_in("In Review")]);
    let answer = stages_through_dispatch(&context, "7", &endpoint.url).expect("merged");
    assert_eq!(answer["position"], "merged");
    assert!(answer.get("linear_state").is_none(), "{answer}");
    assert!(endpoint.sent().is_empty(), "{:?}", endpoint.sent());
}
