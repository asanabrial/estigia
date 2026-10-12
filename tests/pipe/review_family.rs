use super::*;

fn call_tool(
    rig: &TrackerRig,
    answers: &str,
    log: &std::path::Path,
    body_log: &std::path::Path,
    name: &str,
    arguments: serde_json::Value,
) -> (bool, String) {
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LOG", log)
        .env("ESTIGIA_FAKE_BODY_LOG", body_log)
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
    match response.get("error") {
        Some(error) => (true, error["message"].to_string()),
        None => (
            response["result"]["isError"] == true,
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ),
    }
}

fn filing_answers(listed: &str) -> String {
    serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue create",
            "stdout": "https://github.com/o/r/issues/101\n",
            "status": 0,
        },
        { "matches": "issue list", "stdout": listed, "status": 0 },
    ]))
    .expect("the fake tracker script serialises")
}

const FILED_TITLE: &str = "Issue and PR titles are unreadable at a glance";

/// A `create` that names an epoch records the `review` marker in the store and
/// files a body with no marker in it.
///
/// Turn it off by rendering the marker into the filed body again and this
/// reddens on the body log.
#[test]
fn create_with_an_epoch_files_no_review_marker_and_records_it_locally() {
    let rig = tracker_rig();
    name_origin_o_r(rig.repo.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");
    let (log, body_log) = (
        trace.path().join("calls.log"),
        trace.path().join("bodies.log"),
    );

    let (failed, text) = call_tool(
        &rig,
        &filing_answers("[]"),
        &log,
        &body_log,
        "create",
        serde_json::json!({
            "identity": "unreadable-titles",
            "title": FILED_TITLE,
            "body_file": body.to_string_lossy(),
            "priority": "priority:low",
            "domain": "general",
            "runtime": "claude",
            "run_id": "claude-abcd1234",
            "epoch": "0123456789abcdef0123456789abcdef",
            "findings": "finding-one",
        }),
    );
    assert!(!failed, "the issue was filed and the call failed: {text}");

    let filed = std::fs::read_to_string(&body_log).expect("the body log is readable");
    assert!(
        !filed.contains("issue-flow:"),
        "the filed body carries a marker the store now holds: {filed}"
    );
    let connection = rusqlite::Connection::open(rig.home.path().join(".estigia").join("state.db"))
        .expect("the store was created");
    let recorded: Vec<String> = connection
        .prepare("SELECT body FROM entries WHERE issue = 101 AND kind = 'review'")
        .expect("the entries table reads")
        .query_map([], |row| row.get(0))
        .expect("the entries read")
        .collect::<Result<_, _>>()
        .expect("every entry reads");
    assert_eq!(
        recorded.len(),
        1,
        "the review marker was not recorded: {recorded:?}"
    );
    assert!(
        recorded[0].contains("epoch=0123456789abcdef0123456789abcdef")
            && recorded[0].contains("findings=finding-one"),
        "the recorded marker lost an attribute: {recorded:?}"
    );
}

/// A store that refuses the admission after the issue was filed leaves the
/// identity unrecorded; the retry adopts the filed issue instead of filing a
/// second one.
///
/// Turn it off by dropping the pending record, or the title lookup, and the
/// retry files issue 102 and this reddens on the second `issue create`.
#[test]
fn a_create_whose_store_admission_failed_is_adopted_by_the_retry_not_filed_twice() {
    let rig = tracker_rig();
    name_origin_o_r(rig.repo.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");
    let arguments = serde_json::json!({
        "identity": "unreadable-titles",
        "title": FILED_TITLE,
        "body_file": body.to_string_lossy(),
        "priority": "priority:low",
        "domain": "general",
        "runtime": "claude",
        "run_id": "claude-abcd1234",
    });

    let store_path = rig.home.path().join(".estigia").join("state.db");
    drop(estigia::transport::store::Store::open(&store_path).expect("the store opens"));
    let connection = rusqlite::Connection::open(&store_path).expect("the store opens");
    connection
        .execute_batch(
            "CREATE TRIGGER refuse_admission BEFORE INSERT ON issues
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .expect("the trigger installs");

    let first_log = trace.path().join("first.log");
    let (failed, said) = call_tool(
        &rig,
        &filing_answers("[]"),
        &first_log,
        &trace.path().join("first-bodies.log"),
        "create",
        arguments.clone(),
    );
    assert!(failed, "a refused admission was reported as filed: {said}");
    assert!(
        said.contains("#101") && said.contains("adopts issue #101"),
        "the refusal does not name the filed issue and the retry: {said}"
    );

    connection
        .execute_batch("DROP TRIGGER refuse_admission")
        .expect("the trigger drops");
    let listed = serde_json::json!([{
        "number": 101,
        "title": FILED_TITLE,
        "url": "https://github.com/o/r/issues/101",
    }])
    .to_string();
    let second_log = trace.path().join("second.log");
    let (failed, said) = call_tool(
        &rig,
        &filing_answers(&listed),
        &second_log,
        &trace.path().join("second-bodies.log"),
        "create",
        arguments,
    );
    assert!(!failed, "the retry was refused: {said}");
    let answer: serde_json::Value = serde_json::from_str(&said).expect("the answer is JSON");
    assert_eq!(
        answer["issue"], 101,
        "the retry did not adopt the filed issue: {said}"
    );

    let calls = std::fs::read_to_string(&second_log).expect("the call log is readable");
    assert!(
        !calls.contains("issue create"),
        "the retry filed a duplicate: {calls}"
    );
    let identity: Option<String> = connection
        .query_row(
            "SELECT identity FROM issues WHERE number = 101",
            [],
            |row| row.get(0),
        )
        .expect("the adopted issue is in the store");
    assert_eq!(identity.as_deref(), Some("unreadable-titles"));
}

/// The `comment` tool posts a person's prose to GitHub and nothing else: no
/// marker rides in the body, and the kind and run-id it was called with decide
/// no text.
///
/// Turn it off by appending `markers::render(kind, ...)` to the body again and
/// this reddens.
#[test]
fn the_comment_tool_posts_the_prose_alone() {
    let rig = tracker_rig();
    name_origin_o_r(rig.repo.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let prose = trace.path().join("note.md");
    std::fs::write(&prose, "Found the cause in the retry loop.\n").expect("the prose is written");
    let (log, body_log) = (
        trace.path().join("calls.log"),
        trace.path().join("bodies.log"),
    );

    let (failed, said) = call_tool(
        &rig,
        "[]",
        &log,
        &body_log,
        "comment",
        serde_json::json!({
            "issue": 12,
            "body_file": prose.to_string_lossy(),
            "run_id": "claude-abcd1234",
            "kind": "diagnosis",
        }),
    );
    assert!(!failed, "the comment was refused: {said}");

    let posted = std::fs::read_to_string(&body_log).expect("the body log is readable");
    assert!(
        posted.ends_with("Found the cause in the retry loop.\n\n")
            && !posted.contains("issue-flow:")
            && !posted.contains("<!--"),
        "the posted comment is not the prose alone: {posted:?}"
    );
    let calls = std::fs::read_to_string(&log).expect("the call log is readable");
    assert_eq!(comment_writes(&calls), 1, "{calls}");
}

/// The stand-in `gh` refuses a body bound for GitHub that carries a marker, so
/// every test that drives it sweeps the bodies of the operations it runs.
#[test]
fn the_stand_in_gh_refuses_any_body_carrying_an_issue_flow_marker() {
    let rig = tracker_rig();
    let trace = tempfile::tempdir().expect("a trace directory");
    for (kind, arguments) in [
        ("issue comment", ["issue", "comment", "12"]),
        ("issue create", ["issue", "create", "--title"]),
        ("pr create", ["pr", "create", "--draft"]),
    ] {
        let staged = trace.path().join("body.md");
        std::fs::write(
            &staged,
            "prose\n\n<!-- issue-flow: claim encoding=pct run-id=claude-a -->\n",
        )
        .expect("the body is written");
        let output = Command::new(rig.bin.path().join("gh"))
            .args(arguments)
            .args(["--body-file"])
            .arg(&staged)
            .output()
            .expect("the stand-in runs");
        assert_eq!(
            output.status.code(),
            Some(97),
            "{kind}: the stand-in accepted a marker-bearing body: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
