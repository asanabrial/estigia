use super::*;

const FILED_ELSEWHERE: &str = "A finding about another repository";

fn call(
    home: &std::path::Path,
    repo: &std::path::Path,
    bin: &std::path::Path,
    answers: &str,
    logs: &[(&str, &std::path::Path)],
    name: &str,
    arguments: serde_json::Value,
) -> (bool, String) {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string();
    let mut command = tracker_command(home, repo, bin, answers);
    command.arg("mcp");
    for (variable, path) in logs {
        command.env(variable, path);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (
        response["result"]["isError"] == true,
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    )
}

fn filing(body: &std::path::Path, repo: &str) -> serde_json::Value {
    serde_json::json!({
        "identity": "cross-repo-finding",
        "title": FILED_ELSEWHERE,
        "body_file": body.to_string_lossy(),
        "priority": "priority:low",
        "domain": "general",
        "runtime": "claude",
        "run_id": "claude-abcd1234",
        "repo": repo,
    })
}

/// A session in `o/r` files into `o/b` through `create`, and `o/b`'s own queue sees the issue.
///
/// Turn it off by dropping the `repo` override in `run_tool` (`src/harness/mcp/mod.rs`) and this
/// reddens on the pinned `GH_REPO`, and on the store row that lands under `o/r`.
#[test]
fn create_with_a_repo_files_in_that_repository_and_leaves_the_issue_unclaimed() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let (log, env_log) = (trace.path().join("calls.log"), trace.path().join("env.log"));
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");

    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "issue create", "stdout": "https://github.com/o/b/issues/7\n", "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let (failed, text) = call(
        home,
        repo,
        bin,
        &answers,
        &[
            ("ESTIGIA_FAKE_LOG", &log),
            ("ESTIGIA_FAKE_ENV_LOG", &env_log),
        ],
        "create",
        filing(&body, "o/b"),
    );
    assert!(
        !failed,
        "the filing in another repository was refused: {text}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the answer is JSON: {text}"));
    assert_eq!(answer["issue"], 7, "{text}");
    for label in [
        "status:ready",
        "priority:low",
        "domain:general",
        "analyst:claude",
    ] {
        assert!(
            answer["labels"]
                .as_array()
                .is_some_and(|labels| labels.iter().any(|found| found == label)),
            "the filed issue lacks `{label}`: {text}"
        );
    }

    let calls = std::fs::read_to_string(&log).expect("the call log is readable");
    assert!(calls.contains("issue create"), "nothing was filed: {calls}");
    let pins = std::fs::read_to_string(&env_log).expect("the environment log is readable");
    let pinned: Vec<&str> = pins
        .lines()
        .filter(|line| line.starts_with("GH_REPO="))
        .collect();
    assert!(
        !pinned.is_empty() && pinned.iter().all(|line| *line == "GH_REPO=o/b"),
        "a call of the filing was aimed somewhere other than o/b:\n{pins}"
    );

    let connection = rusqlite::Connection::open(home.join(".estigia").join("state.db"))
        .expect("the store was created");
    let held = |owner_name: &str| -> (Option<String>, Option<String>, i64) {
        connection
            .query_row(
                "SELECT issues.state, issues.identity, \
                 (SELECT count(*) FROM entries WHERE entries.repo_id = repos.id AND entries.issue = 7) \
                 FROM issues JOIN repos ON repos.id = issues.repo_id \
                 WHERE repos.owner || '/' || repos.name = ?1 AND issues.number = 7",
                [owner_name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("{owner_name} does not hold issue 7: {error}"))
    };
    assert_eq!(
        held("o/b"),
        (
            Some("ready".to_owned()),
            Some("cross-repo-finding".to_owned()),
            0
        ),
        "the issue was not recorded unclaimed under o/b"
    );
    let under_a: i64 = connection
        .query_row(
            "SELECT count(*) FROM issues JOIN repos ON repos.id = issues.repo_id \
             WHERE repos.owner = 'o' AND repos.name = 'r'",
            [],
            |row| row.get(0),
        )
        .expect("the store counts");
    assert_eq!(
        under_a, 0,
        "the issue was recorded under the session's own repository"
    );

    let elsewhere = tracker_rig();
    name_origin(elsewhere.repo.path(), "git@github.com:o/b.git");
    let listing = serde_json::json!([[{
        "number": 7,
        "title": FILED_ELSEWHERE,
        "labels": [{"name": "status:ready"}, {"name": "domain:general"}, {"name": "priority:low"}],
        "created_at": "2026-09-01T00:00:00Z",
    }]]);
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "issues?state=open", "stdout": listing.to_string(), "status": 0},
    ]))
    .expect("the fake tracker script serialises");
    let (failed, text) = call(
        home,
        elsewhere.repo.path(),
        bin,
        &answers,
        &[],
        "list_state",
        serde_json::json!({"state": "ready", "run_id": "claude-abcd1234"}),
    );
    assert!(!failed, "o/b's queue read failed: {text}");
    let queue: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue is JSON: {text}"));
    assert!(
        queue["unadmitted"]
            .as_array()
            .is_none_or(|unadmitted| unadmitted.is_empty())
            && text.contains(FILED_ELSEWHERE),
        "o/b's queue does not hold the issue as admitted and ready: {text}"
    );
}
