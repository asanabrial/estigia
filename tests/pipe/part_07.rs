use super::*;

/// `publish_review` omits the payload's text by default, keeps every other
/// receipt field either way, and never truncates the text it is asked for.
///
/// The answer used to carry the payload unconditionally, and an
/// diff of a mid-sized change exceeded what the MCP surface could return, so the
/// publishing run could not read its own receipt back. `inline_payload: true`
/// is the only way to ask for the text now; `payload_bytes` and
/// `payload_digest` are measured against a diff computed independently in this
/// test, not against the function under test, so a truncation or a digest that
/// stopped matching its own bytes could not hide behind a self-comparison.
#[test]
fn publish_review_carries_the_payload_only_when_asked_and_never_truncates_it() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-inline-payload";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]);
    git(&["push", "-q", "origin", "main"]);

    git(&["checkout", "-q", "-b", branch]);
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]);
    git(&["push", "-q", "-u", "origin", branch]);
    let head = sha("HEAD");
    let base_sha = sha("origin/main");

    // The independent measurement: the exact bytes `review_payload` derives,
    // computed here — with the same pinned flags — rather than trusted from
    // the answer.
    let expected_payload = String::from_utf8(fenced_ambient_diff(repo, &base_sha, &head))
        .expect("the diff is UTF-8 in this fixture");
    let expected_digest = estigia::transport::ownership::sha256_hex(expected_payload.as_bytes());
    assert!(
        !expected_payload.is_empty(),
        "the fixture change produced no diff to measure against"
    );

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let timeline = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:in-progress"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!(
                "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
                 horizon=2099-01-01T00:00Z op-id={} -->\n",
                "a".repeat(32)
            ),
        }],
    })
    .to_string();
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline, "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // Three calls against the one fixture: the default answer, the same
    // publication asked to carry the text inline, and the same publication
    // asked to bring the tier and the first `review_status` back with it.
    // None of the pushes moves anything — the branch is already at `head` —
    // so all three reach the identical reused PR and receipt derivation, and
    // only the requested extras differ.
    for (inline, with_review) in [(false, false), (true, false), (false, true)] {
        record_cause_over_latest_receipt(home, 12);
        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let mut arguments = serde_json::json!({
            "issue": 12,
            "run_id": run_id,
            "branch": branch,
            "base": "main",
            "pr_title": "Something",
            "pr_body_file": body.display().to_string(),
            "worktree": repo.display().to_string(),
        });
        if inline {
            arguments["inline_payload"] = serde_json::json!(true);
        }
        if with_review {
            arguments["with_review"] = serde_json::json!(true);
        }
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "publish_review", "arguments": arguments }
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert_eq!(
            response["result"]["isError"], false,
            "inline_payload={inline}: the publication did not land: {text}"
        );
        let answer: serde_json::Value =
            serde_json::from_str(&text).expect("the publication answer is JSON");
        let answer_object = answer.as_object().expect("the answer is a JSON object");

        // Every receipt field the brief requires stays present whichever way
        // `inline_payload` was asked.
        for field in [
            "epoch",
            "pr",
            "head",
            "base",
            "digest",
            "payload_bytes",
            "payload_digest",
        ] {
            assert!(
                answer_object.contains_key(field) && !answer[field].is_null(),
                "inline_payload={inline}: the answer dropped `{field}`: {text}"
            );
        }
        // `parent` moved to `review_transaction` along with the rest of the
        // lineage, the topology and the autoclose verdict — see
        // `docs/review-cost.md`. This is a first publication, so it never had
        // a parent to carry either way; the point here is that the key itself
        // is gone from `publish_review`'s own answer.
        assert!(
            !answer_object.contains_key("parent"),
            "inline_payload={inline}: `parent` moved to review_transaction and must not ride \
             publish_review's own answer any more: {text}"
        );
        assert_eq!(
            answer["payload_bytes"].as_u64(),
            Some(expected_payload.len() as u64),
            "inline_payload={inline}: payload_bytes does not match the diff measured \
             independently: {text}"
        );
        assert_eq!(
            answer["payload_digest"].as_str(),
            Some(expected_digest.as_str()),
            "inline_payload={inline}: payload_digest does not match the diff measured \
             independently: {text}"
        );

        let next = answer["next"].as_str().unwrap_or_default();
        if inline {
            assert_eq!(
                answer["payload"].as_str(),
                Some(expected_payload.as_str()),
                "inline_payload=true: the payload is not byte-identical to the diff measured \
                 independently — a truncated or altered payload is worse than none: {text}"
            );
            assert_eq!(
                next, INLINE_PAYLOAD_NEXT,
                "inline_payload=true: the `next` sentence changed. It is pinned whole, for the \
                 same reason as its sibling below."
            );
        } else {
            assert!(
                !answer_object.contains_key("payload"),
                "inline_payload=false (the default): the answer carried `payload` text nobody \
                 asked for, at the cost this issue exists to remove: {text}"
            );
            assert!(
                !next.contains("hand each judge the `payload`"),
                "inline_payload=false (the default): `next` tells the caller to hand on a \
                 payload this answer did not carry — a dead end named as an instruction: {next}"
            );
            // Pinned whole, because a `str::contains` cannot decide whether a
            // sentence offers a route to the payload text. Two panels rejected
            // two attempts at deciding it that way: first a guard spelled around
            // the wording it policed, then a blocklist of tokens that both
            // `publication` — which does not contain `publish` — and
            // `--inline-payload` — which does not contain `inline_payload` —
            // walk straight past. This is contract text a caller acts on, so any
            // edit reddens here and the judgement happens at the edit rather
            // than in a substring somebody guessed.
            assert_eq!(
                next, OMITTED_PAYLOAD_NEXT,
                "inline_payload=false (the default): the `next` sentence changed. It is pinned \
                 whole. If the change is deliberate, update the constant here and say in the \
                 same edit that the new wording still offers no route to the payload text — a \
                 second publication mints a new epoch and voids every verdict recorded against \
                 this receipt."
            );
        }

        // The merged reads: `with_review` brings the tier and the first
        // `review_status` back with the publication, so the `review_tier` and
        // `review_status` calls that always follow it are not a second round
        // trip. The tier is measured here against the range the answer itself
        // names, and `review_status` carries its own forecast.
        if with_review {
            assert_eq!(
                answer["tier"]["level"].as_str(),
                Some("medium"),
                "with_review: the answer carried no tier for base..head: {text}"
            );
            assert!(
                answer["tier"]["forecast"].is_string(),
                "with_review: the tier carried no forecast: {text}"
            );
            // `review_status` reads the local store and the repository, so a
            // fixture whose origin is a bare path can answer it only as a
            // reported read failure. Either the first status (with its
            // forecast and next step) or that failure is present — never
            // neither, and never a refusal of a publication whose push landed.
            if answer["review_status"].is_object() {
                assert!(
                    answer["review_status"]["forecast"]["horizon"].is_string(),
                    "with_review: the review_status carried no forecast: {text}"
                );
                assert!(
                    answer["review_status"]["next"]["kind"].is_string(),
                    "with_review: the review_status carried no next step: {text}"
                );
            } else {
                assert!(
                    answer["review_reads_unavailable"]["reason"].is_string(),
                    "with_review: neither a review_status nor a named read failure came back: {text}"
                );
            }
        } else {
            assert!(
                !answer_object.contains_key("tier"),
                "without with_review the answer carried a tier nobody asked for: {text}"
            );
            assert!(
                !answer_object.contains_key("review_status"),
                "without with_review the answer carried a review_status nobody asked for: {text}"
            );
        }
    }
}

/// A declared `topology: inline` is recorded whatever the changed-file count:
/// nothing refuses it, with or without a reason, and a departure from `inline`
/// is recorded as typed.
#[test]
fn publish_review_records_a_bare_inline_declaration_over_any_file_count() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-topology4a2b1c3d";
    let branch = "fix/13-topology-threshold";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    std::fs::write(repo.join("one.txt"), "base one\n").expect("the base file");
    std::fs::write(repo.join("two.txt"), "base two\n").expect("the base file");
    git(&["add", "one.txt", "two.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]);
    git(&["push", "-q", "origin", "main"]);

    git(&["checkout", "-q", "-b", branch]);
    std::fs::write(repo.join("one.txt"), "changed one\n").expect("the change");
    std::fs::write(repo.join("two.txt"), "changed two\n").expect("the change");
    git(&["add", "one.txt", "two.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "two files changed",
    ]);
    git(&["push", "-q", "-u", "origin", branch]);
    let head = sha("HEAD");
    let base_sha = sha("origin/main");

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let timeline = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:in-progress"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!(
                "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
                 horizon=2099-01-01T00:00Z op-id={} -->\n",
                "a".repeat(32)
            ),
        }],
    })
    .to_string();
    let pr = serde_json::json!({
        "number": 13,
        "url": "https://github.com/o/r/pull/13",
        "headRefOid": head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline, "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // One call per declared topology, against the one fixture: the branch
    // never moves, so every call reaches the identical reused PR.
    // `temp` isolates each call's own staged scratch files — the `published`
    // comment `stage`s before posting it — so a successful call's own marker
    // can be read back afterward the same way `staged_receipt` reads it
    // elsewhere: `topology`/`topology_reason` moved off the answer to that
    // marker (`review_transaction` is what answers them now), and this is
    // the only copy that survives the call.
    let call = |arguments: serde_json::Value, log: &std::path::Path| -> (bool, String) {
        record_cause_over_latest_receipt(home, 13);
        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let _ = std::fs::remove_file(log);
        let temp = log.with_extension("temp");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(13));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "publish_review", "arguments": arguments }
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_LOG", log)
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        (
            response["result"]["isError"].as_bool().unwrap_or(false),
            text,
        )
    };

    let arguments = |extra: &[(&str, &str)]| {
        let mut value = serde_json::json!({
            "issue": 13,
            "run_id": run_id,
            "branch": branch,
            "base": "main",
            "pr_title": "Something",
            "pr_body_file": body.display().to_string(),
            "worktree": repo.display().to_string(),
        });
        let object = value.as_object_mut().expect("the arguments are an object");
        for (key, val) in extra {
            object.insert((*key).to_owned(), serde_json::json!(val));
        }
        value
    };

    let bare_log = trace.path().join("bare-calls.log");
    let (bare_is_error, bare) = call(arguments(&[("topology", "inline")]), &bare_log);
    assert!(
        !bare_is_error,
        "a bare `topology: inline` must be recorded, not refused: {bare}"
    );
    assert!(
        !bare.contains("topology-inline-over-threshold"),
        "the retired file-count refusal returned: {bare}"
    );
    let bare_receipt = staged_receipt(home, 13);
    assert!(
        bare_receipt.contains("topology=inline") && !bare_receipt.contains("topology-reason="),
        "the staged receipt does not carry the bare declaration: {bare_receipt}"
    );

    let reasoned_log = trace.path().join("reasoned-calls.log");
    let (reasoned_is_error, reasoned) = call(
        arguments(&[
            ("topology", "inline"),
            (
                "topology_reason",
                "one mechanical rename split across two files",
            ),
        ]),
        &reasoned_log,
    );
    assert!(
        !reasoned_is_error,
        "a stated reason must be accepted without argument: {reasoned}"
    );
    // `topology`/`topology_reason` moved to `review_transaction` — see
    // `docs/review-cost.md` — so what a declared topology actually recorded is
    // checked on the staged marker, `review_transaction`'s own evidence.
    let reasoned_receipt = staged_receipt(home, 13);
    assert!(
        reasoned_receipt.contains("topology=inline")
            && reasoned_receipt.contains(
                "topology-reason=one%20mechanical%20rename%20split%20across%20two%20files"
            ),
        "the staged receipt does not carry the declared topology and its reason: \
         {reasoned_receipt}"
    );

    let delegated_log = trace.path().join("delegated-calls.log");
    let (delegated_is_error, delegated) =
        call(arguments(&[("topology", "developer:1")]), &delegated_log);
    assert!(
        !delegated_is_error,
        "a departure from `inline` needs no reason at all: {delegated}"
    );
    let delegated_receipt = staged_receipt(home, 13);
    assert!(
        delegated_receipt.contains("topology=developer:1"),
        "the staged receipt does not carry the declared topology: {delegated_receipt}"
    );
    assert!(
        !delegated_receipt.contains("topology-reason="),
        "no reason was given and none should be recorded: {delegated_receipt}"
    );

    let cased_log = trace.path().join("cased-calls.log");
    let (cased_is_error, cased) = call(arguments(&[("topology", "Inline")]), &cased_log);
    assert!(
        !cased_is_error,
        "a capitalised `Inline` must be recorded: {cased}"
    );
    assert!(
        staged_receipt(home, 13).contains("topology=Inline"),
        "the declaration is kept as typed"
    );

    let padded_log = trace.path().join("padded-calls.log");
    let (padded_is_error, padded) = call(arguments(&[("topology", "inline  ")]), &padded_log);
    assert!(
        !padded_is_error,
        "a padded `inline  ` must be recorded: {padded}"
    );
    let padded_receipt = staged_receipt(home, 13);
    assert!(
        padded_receipt.contains("topology=inline -->"),
        "the padded declaration was not recorded trimmed: {padded_receipt}"
    );

    let raised_log = trace.path().join("raised-calls.log");
    let (raised_is_error, raised) = call(
        arguments(&[
            ("raise", "authorization"),
            ("raise_reason", "a new caller reaches this path"),
            ("focus", "risk"),
            ("focus_reason", "failure is the point"),
        ]),
        &raised_log,
    );
    assert!(!raised_is_error, "a valid raise must publish: {raised}");
    let raised_receipt = staged_receipt(home, 13);
    for expected in [
        "raise=authorization",
        "raise-reason=a%20new%20caller%20reaches%20this%20path",
        "focus=risk",
        "focus-reason=failure%20is%20the%20point",
        "computed-level=medium",
    ] {
        assert!(
            raised_receipt.contains(expected),
            "the staged receipt lacks `{expected}`: {raised_receipt}"
        );
    }

    let refused_log = trace.path().join("refused-calls.log");
    let (refused_is_error, refused) = call(
        arguments(&[("raise_reason", "no item named")]),
        &refused_log,
    );
    assert!(
        refused_is_error && refused.contains("risk-raise-reason-without-raise"),
        "a reason with no raise was not refused with its own reason: {refused}"
    );
}

/// The lane refusal refuses, and the two lenient states publish — through the
/// real server, over the wire the transport actually uses.
///
/// **The step nothing held.** The classification is measured pure
/// (`only_a_permission_refusal_stops_a_publication_that_could_not_start_its_lane`)
/// and the position is measured in the call log (the test above), but what joins
/// them — that `Forbidden` returns `Err` and that nothing else does — was
/// measured by no test at all. Both directions were mutated and both stayed
/// green: deleting the whole refusal block, and making `Absent` refuse beside
/// `Forbidden`. The second is the one that matters most, because it breaks every
/// consumer repository that has no `ci.yml` while looking like a tighter gate.
///
/// Three rows, one per state that can reach the join:
///
/// - **`HTTP 403` refuses**, `publication-lane-forbidden`, and refuses *before*
///   the epoch marker: nothing is posted to the timeline. The envelope carries
///   `world: "committed"`, because the push has already happened and a bare stop
///   would send somebody looking for a branch that is on the remote.
/// - **`HTTP 404` publishes**, reporting `absent`. Estigia does not parse
///   consumer YAML and cannot tell a workflow that is not there from one it may
///   not see, so the ambiguous code is on the permissive side by construction.
/// - **A dispatch that does not answer publishes**, reporting `unknown`. An
///   unknown result is not clearance, but it is not a reason to refuse a
///   publication either — `record_review_verdict` re-reads the world.
///
/// The state word is asserted for each, not only the refusal: only `started` was
/// ever pinned, so a wrong word in `absent` or `unknown` was invisible, and the
/// note beside it is what tells a resuming run whether to go looking for checks.
#[test]
fn only_a_forbidden_dispatch_refuses_a_publication_and_the_other_states_report() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-lane-states";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]);
    git(&["push", "-q", "origin", "main"]);

    git(&["checkout", "-q", "-b", branch]);
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]);
    git(&["push", "-q", "-u", "origin", branch]);
    let head = sha("HEAD");
    let base_sha = sha("origin/main");

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let timeline = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": "status:in-progress"}],
        "comments": [
            {
                "id": "IC_1",
                "createdAt": "2026-01-01T00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!(
                    "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
                     horizon=2099-01-01T00:00Z op-id={} -->\n",
                    "a".repeat(32)
                ),
            },
        ],
    })
    .to_string();
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });

    // The three states that can reach the join, as `gh workflow run` answers
    // them. `Started` is the fourth and the test above holds it.
    let states: [(&str, &str, Option<&str>); 3] = [
        (
            "forbidden",
            "gh: HTTP 403: Resource not accessible by personal access token",
            None,
        ),
        ("absent", "gh: HTTP 404: Not Found", Some("absent")),
        (
            "unknown",
            "dial tcp: lookup api.github.com: no such host",
            Some("unknown"),
        ),
    ];

    for (label, said, reported) in states {
        record_cause_over_latest_receipt(home, 12);
        let answers = serde_json::to_string(&serde_json::json!([
            // First, and narrowly: the dispatch is the only call whose answer
            // this test varies, and an entry placed after a broader `matches`
            // would never be reached.
            { "matches": "workflow run", "stderr": said, "status": 1 },
            { "matches": "issue view", "stdout": timeline, "status": 0 },
            { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
            { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
            { "matches": "json body", "stdout": serde_json::json!({"body": "names nothing"}).to_string(), "status": 0 },
            { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
            {
                "matches": "api graphql",
                "stdout": serde_json::json!({
                    "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                    } } } },
                }).to_string(),
                "status": 0,
            },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "publish_review", "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }}
        })
        .to_string();

        let log = trace.path().join(format!("{label}.log"));
        let temp = trace.path().join(format!("temp-{label}"));
        std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_LOG", &log)
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();

        // Every row dispatches. A publication that stopped asking would pass
        // the two lenient rows below without ever reaching the branch they are
        // here to hold.
        assert!(
            calls.contains("workflow run ci.yml"),
            "the {label} row never started a publication lane at all, so the state it reports is \
             not the state it measured: {calls}"
        );

        let Some(state) = reported else {
            assert_eq!(
                response["result"]["isError"], true,
                "a token that may not start workflows published anyway, so this epoch has a lane \
                 nobody can start: {text}"
            );
            assert!(
                text.contains("publication-lane-forbidden"),
                "the forbidden dispatch refused for the wrong reason: {text}"
            );
            // Before the marker, and this is where that is worth more than a
            // position in the log: the timeline is empty, so no reviewer can be
            // bound to an epoch whose lane never ran.
            assert!(
                !calls.contains("issue comment"),
                "the forbidden dispatch recorded a review transaction anyway: {calls}"
            );
            // The push did happen. Asserted through the sentence the harness
            // renders rather than through the key, because that is what the
            // reader sees: `world: "committed"` becomes this line, and a
            // refusal carrying no world at all renders **no such line** — so
            // dropping the key from the envelope reddens here.
            assert!(
                text.contains("What happened to the world: the write landed"),
                "the forbidden refusal reads as though nothing was written, which sends somebody \
                 looking for a branch that is already on the remote: {text}"
            );
            // Naming a dead end is worse than naming nothing, and the appended
            // clause about the rewritten pull request must not have replaced
            // the one command that clears this.
            assert!(
                text.contains("gh auth refresh -s workflow"),
                "the forbidden refusal names no command that clears it: {text}"
            );
            continue;
        };

        // The direction that breaks every consumer with no `ci.yml`. Absence is
        // the ordinary condition of a repository that has not adopted the lane,
        // and an unknown dispatch is not an answer that anything downstream
        // trusts — neither is a reason to refuse a publication.
        assert_eq!(
            response["result"]["isError"], false,
            "the {label} publication lane refused a publication, which breaks every repository \
             that has no dispatchable lane: {text}"
        );
        let answered: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|error| panic!("{label}: {error}: {text}"));
        assert_eq!(
            answered["publication_lane"], state,
            "the {label} row did not report `{state}`, so a wrong state word would be invisible: \
             {text}"
        );
        assert!(
            any_receipt_staged(home, 12),
            "the {label} row published without recording a receipt: {calls}"
        );
        // And the receipt says the same thing the answer did. One call, two
        // sentences, and they are read by different people.
        let receipt = staged_receipt(home, 12);
        assert!(
            !receipt.contains("was started against this head"),
            "the {label} receipt promises a lane run this publication did not start: {receipt}"
        );
        assert!(
            !receipt.contains("CI remains blocked while the PR is draft"),
            "the {label} receipt still carries the sentence the publication lane made false: \
             {receipt}"
        );
    }
}

/// A verdict is not banked for a head whose publication lane has not cleared it,
/// and absence of a lane is not a refusal.
///
/// This is the half that makes the change mechanical. Dispatching a lane and
/// asking whoever writes the review prompt to look at it first is a rule that
/// depends on somebody remembering, which is the failure an analysis concluded
/// against. `record_computed_verdict` is the single writer of a verdict marker, so
/// one refusal here covers every route to one — including the handoff route,
/// where a different run entirely records the verdict.
///
/// Four states, driven through the real server against a scripted `gh`:
///
/// - **no check runs at all proceeds.** First, and on its own line, because it
///   is what keeps every repository that has no dispatchable lane working. A
///   later change that turned absence into a refusal would break every consumer,
///   and this is the assertion that would say so.
/// - a failed conclusion refuses, naming the lane and the run;
/// - a queued or running lane refuses, naming the run to watch — `in_progress`
///   is not `success`, and this is the branch most easily written the wrong way
///   round;
/// - a listing that cannot be read refuses as a **read failure**. An unknown
///   result is not clearance, and reporting it as a red lane would send somebody
///   to fix a test that never ran.
///
/// One last row poses the narrowing rather than a state of the lane: the same
/// red listing, recorded as a **rejection**, which succeeds and never reads the
/// lane at all. That is what stops the gate from inventing a deadlock — a
/// reviewer who may not reject red bytes cannot resolve the handoff that lets
/// the author fix them.
///
/// Every refusal is checked against the call log for a comment, because a
/// verdict that refuses and writes anyway has recorded the thing it refused.
/// Every acceptance is checked against the same log for the **whole path** the
/// gate issued, because a lane read against the wrong ref is a gate that decides
/// nothing while answering `ok`.
#[test]
fn a_verdict_waits_for_the_publication_lane_and_absence_of_one_is_not_a_refusal() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let publisher = "claude-abcd1234";
    let reviewer = "gemini-c0ffee00";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    // The severe finding a rejection rests on: without it the computed verdict
    // is `approved` and the rejection case would measure the lane it is meant
    // to show is never read.
    let found = comment(
        "IC_finding",
        "2026-08-14T10:30:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", reviewer),
                ("reviewer", reviewer),
                ("op-id", &"b".repeat(32)),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("id", "lane-fixture-defect"),
                ("class", "severe"),
                ("evidence-class", "deterministic"),
                ("causal-disposition", "introduced"),
                (
                    "evidence",
                    "the fixture's own defect, so the rejection has a subject",
                ),
                (
                    "impact",
                    "without it the rejection cases could not be recorded at all",
                ),
            ],
        ),
    );
    let timeline = |rejecting: bool| -> String {
        let mut comments = vec![held.clone(), publication.clone()];
        if rejecting {
            comments.push(found.clone());
        }
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "comments": comments,
        })
        .to_string()
    };

    let check_run = |status: &str, conclusion: serde_json::Value| {
        serde_json::json!({
            "name": "check (ubuntu-latest)",
            "status": status,
            "conclusion": conclusion,
            "html_url": "https://github.com/o/r/actions/runs/31865504912/job/99",
        })
    };
    let listing = |runs: Vec<serde_json::Value>| {
        serde_json::json!({"total_count": runs.len(), "check_runs": runs}).to_string()
    };

    // The four states, each as the answer the check-run listing gives. `status`
    // is the exit code of the scripted `gh`, so the last one is a call that
    // could not be made at all rather than one that answered emptily.
    let none = listing(vec![]);
    let failed = listing(vec![check_run("completed", serde_json::json!("failure"))]);
    let unfinished = listing(vec![check_run("in_progress", serde_json::Value::Null)]);
    // One page of several, which is the unreadable shape that answers `ok`. The
    // call below it is one `gh` could not make at all. Both are failed reads and
    // they fail in different places, so both are posed: thirty green lanes read
    // as the whole of a head is how a red thirty-first becomes invisible.
    let partial = serde_json::json!({
        "total_count": 9,
        "check_runs": [check_run("completed", serde_json::json!("success"))],
    })
    .to_string();
    // The last row is the narrowing rather than another state of the lane: the
    // same red listing, recorded as a **rejection**, which must succeed. A
    // rejection over a red head is the route out — refusing it would leave the
    // reviewer holding a claim over bytes they have already judged, with no way
    // to resolve the handoff that lets the author fix and republish. Widening
    // the gate to every outcome left this file green until this row existed.
    let states: [(&str, &str, i64, &str, Option<&str>); 6] = [
        ("absent", none.as_str(), 0, "accepted", None),
        (
            "red",
            failed.as_str(),
            0,
            "accepted",
            Some("publication-lane-red"),
        ),
        (
            "running",
            unfinished.as_str(),
            0,
            "accepted",
            Some("publication-lane-unfinished"),
        ),
        (
            "partial",
            partial.as_str(),
            0,
            "accepted",
            Some("read-failed"),
        ),
        ("unreadable", "", 1, "accepted", Some("read-failed")),
        ("rejected-over-red", failed.as_str(), 0, "rejected", None),
    ];

    for (label, answer, status, outcome, refusal) in states {
        let log = trace.path().join(format!("{label}.log"));
        forget_store(home);
        let mut script = vec![serde_json::json!({
            "matches": "issue view", "stdout": timeline(outcome == "rejected"), "status": 0,
        })];
        // The lane's answer is scripted against **the receipt's own head**, not
        // against the words `check-runs`. A stand-in that answers any path
        // containing that substring answers a read of `main` just as happily,
        // and `main` is green by construction — so the two one-token mutations
        // that make the gate read the default branch (`{head}` written as `main`
        // in the `format!`, and `verdict.base` passed where `verdict.head`
        // belongs) both left this file green. With the head in the matcher they
        // find no entry, and an unmatched call is empty output, which is a
        // failed read rather than a cleared lane.
        let lane = format!("commits/{head}/check-runs");
        script.extend([
            serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
            serde_json::json!({ "matches": lane, "stdout": answer, "status": status }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{reviewer}.json")));
        let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "record_computed_verdict", "arguments": {
                "issue": 12,
                "run_id": reviewer,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
            "worktree": repo.display().to_string(),
            }}
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env(
                "ESTIGIA_FAKE_COUNT",
                trace.path().join(format!("{label}-count.json")),
            )
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();

        // The path the gate issued, whole, read out of the call log. The
        // fixture above proves the answer came from the right question; this
        // proves the question was asked at all and asked with every part of it
        // intact. `per_page=100` is inside the same string deliberately: it is
        // what makes a listing longer than one page a failed read, and shrinking
        // it — 100 to 5, 100 to 3 — changed nothing any test could see.
        let issued = format!("commits/{head}/check-runs?per_page=100");
        if outcome == "accepted" {
            assert!(
                calls.contains(&issued),
                "the lane was not read for the receipt's own head ({label}); nothing in the call \
                 log spells `{issued}`: {calls}"
            );
        } else {
            // The other half of the narrowing, and the reason it is measured
            // here rather than stated in `skill/bindings/github.md` alone: a
            // rejection does not consult the lane at all, so a gate widened to
            // every outcome shows up as an extra call before it shows up as a
            // refusal.
            assert!(
                !calls.contains("check-runs"),
                "a rejected verdict read the publication lane ({label}): {calls}"
            );
        }

        let Some(reason) = refusal else {
            // The state that must never refuse. A repository with no
            // dispatchable lane has no check runs on any head.
            assert_eq!(
                response["result"]["isError"], false,
                "a head with no check runs was refused, which breaks every repository that has \
                 no lane: {text}"
            );
            assert!(
                local_entries(home)
                    .iter()
                    .any(|body| body.contains("issue-flow: review-computed-verdict")),
                "the verdict was accepted and never recorded: {calls}"
            );
            continue;
        };
        assert_eq!(
            response["result"]["isError"], true,
            "the {label} lane cleared a verdict: {text}"
        );
        assert!(
            text.contains(reason),
            "the {label} lane refused for the wrong reason: {text}"
        );
        assert!(
            !calls.contains("issue comment"),
            "a refused verdict still wrote to the timeline ({label}): {calls}"
        );
        // The command that clears it. Naming a dead end is worse than naming
        // nothing, so each refusal carries the one thing to run next.
        match label {
            "red" => assert!(
                text.contains("gh run view 31865504912 --log-failed"),
                "the red refusal names no way to read the failure: {text}"
            ),
            "running" => assert!(
                text.contains("gh run watch 31865504912 --exit-status"),
                "the unfinished refusal names no run to watch: {text}"
            ),
            // A failed read is neither of those, and saying so is the point:
            // the harness renders it as *write nothing and retry the read*
            // rather than as a lane that answered.
            _ => assert!(
                text.contains("failed read and not a green lane"),
                "an unreadable listing was reported as a lane that answered: {text}"
            ),
        }
    }
}

/// A run whose jobs all report `steps: []` never executed a step,
/// and is not evidence of a defect the way a run that ran and failed is.
/// `gh run view --log-failed` reads nothing for the first shape because there
/// is nothing it ran to log — commonly a billing failure on the account
/// running Actions — and before this existed `record_review_verdict` answered
/// both shapes with the identical `publication-lane-red` refusal, sending an
/// operator to a command that could never clear it.
///
/// Both shapes are reproduced from recorded payloads a real repository would
/// answer for a completed, non-green run: real, non-empty `steps` for the
/// job that ran and failed, and `steps: []` for the one that never started.
/// The four rows below are what decides this issue is fixed:
///
/// - `ran-and-failed`, no evidence: refuses `publication-lane-red`, unchanged.
/// - `ran-and-failed`, evidence supplied anyway: **still** refuses
///   `publication-lane-red` — the invariant the issue says must not change.
///   A caller cannot pass `--never-ran-evidence-file` to talk its way past a
///   lane that genuinely ran and failed.
/// - `never-ran`, no evidence: refuses the distinct `publication-lane-never-ran`,
///   whose action names the annotation and asks for the evidence file rather
///   than a log that cannot exist.
/// - `never-ran`, evidence supplied: `accepted`, and the marker records both
///   the classification and the evidence verbatim.
#[test]
fn a_never_ran_lane_is_told_apart_from_one_that_ran_and_failed() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let publisher = "claude-abcd1234";
    let reviewer = "gemini-c0ffee00";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    // The verdict this call is about to write, exactly as the readback would
    // see it — same technique as the lane test above: the operation id is the
    // one the server itself mints from the arguments, so a script that
    // starts answering with this comment once the write has happened lets the
    // accepted row's readback find the marker it just posted.
    let operation_id = estigia::transport::claim::review_operation_id(
        "review-computed-verdict",
        &[reviewer, &epoch, "7", &head, &base, &digest],
    );
    let recorded = comment(
        "IC_verdict",
        "2026-08-14T11:00:00Z",
        marker(
            "review-computed-verdict",
            &[
                ("run-id", reviewer),
                ("op-id", &operation_id),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("level", "low"),
                ("outcome", "approved"),
                ("open", "[]"),
                ("escalated", "[]"),
                ("follow-ups", "[]"),
                ("missing", "[]"),
                ("classification", "never-ran"),
                ("never-ran-evidence", "billing evidence, as posted"),
            ],
        ),
    );
    let timeline = |written: bool| -> String {
        let mut comments = vec![held.clone(), publication.clone()];
        if written {
            comments.push(recorded.clone());
        }
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "comments": comments,
        })
        .to_string()
    };

    // A completed, non-green run, with the numeric check-run `id` a real
    // listing carries and this classification needs to reach the annotations
    // endpoint. `run_id` (in the URL) is the workflow run `gh run view` and
    // `gh run watch` take; `id` is the check run itself, and the two are
    // different numbers on a real repository.
    let check_run = serde_json::json!({
        "id": 555000111_u64,
        "name": "check (ubuntu-latest)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/31865504912/job/99",
    });
    let listing = serde_json::json!({"total_count": 1, "check_runs": [check_run]}).to_string();

    // The two shapes this test exists to tell apart. Both are a completed,
    // non-green run — the check-run listing above cannot distinguish them at
    // all, which is exactly the issue: the discriminator is a second read.
    let ran_and_failed = serde_json::json!({
        "total_count": 1,
        "jobs": [{"steps": [{"name": "pytest", "conclusion": "failure"}]}],
    })
    .to_string();
    let never_ran = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]}).to_string();
    let annotation = serde_json::json!([{
        "message": "This check suite has not been set up: the account has no remaining \
                     included minutes for GitHub Actions.",
    }])
    .to_string();

    let evidence = trace.path().join("evidence.txt");
    std::fs::write(
        &evidence,
        "Checked the account's Actions billing page directly: the spending limit is $0 and \
         every job this week shows zero minutes used.",
    )
    .expect("the evidence file is writable");

    let jobs_path = "actions/runs/31865504912/jobs";
    let annotations_path = "check-runs/555000111/annotations";
    let lane_path = format!("commits/{head}/check-runs");

    // (label, jobs answer, evidence file supplied, expected refusal reason or
    // `None` for an acceptance).
    let rows: [(&str, &str, bool, Option<&str>); 4] = [
        (
            "ran-and-failed-no-evidence",
            &ran_and_failed,
            false,
            Some("publication-lane-red"),
        ),
        // The invariant: evidence changes nothing for a lane that genuinely
        // ran and failed.
        (
            "ran-and-failed-with-evidence",
            &ran_and_failed,
            true,
            Some("publication-lane-red"),
        ),
        (
            "never-ran-no-evidence",
            &never_ran,
            false,
            Some("publication-lane-never-ran"),
        ),
        ("never-ran-with-evidence", &never_ran, true, None),
    ];

    for (label, jobs_answer, give_evidence, refusal) in rows {
        let log = trace.path().join(format!("{label}.log"));
        // Three reads precede the one write a refusal never reaches, and every
        // read after that write has to show the comment it posted — the same
        // shape the lane test above scripts, for the same reason.
        let mut script = vec![];
        for before in 1..=3 {
            script.push(serde_json::json!({
                "matches": "issue view", "nth": before, "stdout": timeline(false), "status": 0,
            }));
        }
        script.extend([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(true), "status": 0 }),
            serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
            serde_json::json!({ "matches": lane_path, "stdout": listing, "status": 0 }),
            serde_json::json!({ "matches": jobs_path, "stdout": jobs_answer, "status": 0 }),
            serde_json::json!({ "matches": annotations_path, "stdout": annotation, "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        let script = serde_json::to_string(&script).expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{reviewer}.json")));
        let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let mut arguments = serde_json::json!({
            "issue": 12,
            "run_id": reviewer,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "worktree": repo.display().to_string(),
        });
        if give_evidence {
            arguments["never_ran_evidence_file"] =
                serde_json::json!(evidence.display().to_string());
        }
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "record_computed_verdict", "arguments": arguments }
        })
        .to_string();
        // Read-only, takes no evidence and refuses nothing — the second half
        // this row exists for: the same head, read through the advisory tool
        // every run is told to consult before ever reaching
        // `record_computed_verdict` at all, so a fix reachable only through the
        // write is a fix `SKILL.md` step 5 routes an obedient run around.
        let temp = trace.path().join(format!("temp-{label}"));
        std::fs::create_dir_all(&temp).expect("a temporary directory for the child");
        let mut child = tracker_command(home, repo, bin, &script)
            .arg("mcp")
            .env(
                "ESTIGIA_FAKE_COUNT",
                trace.path().join(format!("{label}-count.json")),
            )
            .env("ESTIGIA_FAKE_LOG", &log)
            .env("TMPDIR", &temp)
            .env("TMP", &temp)
            .env("TEMP", &temp)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::{BufRead, Write};
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let mut stdout = std::io::BufReader::new(child.stdout.take().expect("stdout is piped"));
        let read_line = |stdout: &mut std::io::BufReader<std::process::ChildStdout>| -> String {
            let mut line = String::new();
            stdout.read_line(&mut line).expect("the server answers");
            assert!(!line.trim().is_empty(), "the server answered nothing");
            line
        };
        writeln!(stdin, "{request}").expect("the verdict request is written");
        let response: serde_json::Value = serde_json::from_str(read_line(&mut stdout).trim())
            .unwrap_or_else(|_| panic!("the MCP response is not JSON"));
        drop(stdin);
        child.wait().expect("the MCP server exits");
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();

        assert!(
            calls.contains(jobs_path),
            "{label}: the lane was never re-classified against its jobs: {calls}"
        );

        let Some(reason) = refusal else {
            assert_eq!(
                response["result"]["isError"], false,
                "{label}: a never-ran lane with evidence was refused: {text}"
            );
            let payload: serde_json::Value =
                serde_json::from_str(&text).unwrap_or_else(|_| panic!("{text}"));
            assert_eq!(payload["outcome"], "approved", "{label}: {text}");
            let staged = recorded_entries(home, 12)
                .into_iter()
                .rev()
                .find(|body| body.contains("issue-flow: review-computed-verdict"))
                .unwrap_or_else(|| panic!("{label}: no computed verdict was recorded"));
            let posted = estigia::transport::markers::parse(&staged)
                .into_iter()
                .find(|marker| {
                    marker.get("kind").map(String::as_str) == Some("review-computed-verdict")
                })
                .unwrap_or_else(|| panic!("{label}: no computed marker in {staged}"));
            assert_eq!(
                posted.get("classification").map(String::as_str),
                Some("never-ran"),
                "{label}: the approval does not record the classification: {staged}"
            );
            assert!(
                posted
                    .get("never-ran-evidence")
                    .is_some_and(|evidence| evidence.contains("spending limit")),
                "{label}: the approval does not carry the evidence verbatim: {staged}"
            );
            continue;
        };
        assert_eq!(
            response["result"]["isError"], true,
            "{label}: the lane cleared a verdict it should have refused: {text}"
        );
        assert!(
            text.contains(reason),
            "{label}: refused for the wrong reason: {text}"
        );
        assert!(
            !calls.contains("issue comment"),
            "{label}: a refused verdict still wrote to the timeline: {calls}"
        );
        if reason == "publication-lane-never-ran" {
            assert!(
                !text.contains("--log-failed"),
                "{label}: the never-ran refusal still tells the caller to read a log that \
                 cannot exist: {text}"
            );
            assert!(
                text.contains("--never-ran-evidence-file"),
                "{label}: the never-ran refusal does not name what to supply instead: {text}"
            );
            assert!(
                text.contains("spending limit") || text.contains("included minutes"),
                "{label}: the never-ran refusal does not name the annotation: {text}"
            );
        } else {
            assert!(
                text.contains("gh run view 31865504912 --log-failed"),
                "{label}: the red refusal names no way to read the failure: {text}"
            );
        }
    }
}

/// A blind judge's severe finding on a third round: `read_publication_lane`
/// returned `Red` as soon as one completed, non-green check run existed, so a
/// head carrying that red beside a still-`queued` check run never reached the
/// `Running` scan below it. A never-ran classification then cleared with
/// evidence supplied, banking a verdict against a lane one of whose runs had
/// not answered at all.
///
/// A head with one completed red check run whose jobs all report `steps: []`
/// (never-ran), plus a second check run still `queued`, with
/// `--never-ran-evidence-file` supplied. Must refuse `publication-lane-unfinished`
/// rather than clear, and the advisory `publication_lane` must answer `running`
/// for the identical head rather than `never-ran`.
#[test]
fn an_unfinished_sibling_check_run_refuses_a_never_ran_clearance() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let reviewer = "gemini-c0ffee00";
    let publisher = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    let timeline = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:review"}],
        "comments": [held, publication],
    })
    .to_string();

    let never_ran_run = serde_json::json!({
        "id": 800100_u64,
        "name": "check (never-ran)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/800000001/job/1",
    });
    let queued_run = serde_json::json!({
        "id": 800200_u64,
        "name": "check (still-queued)",
        "status": "queued",
        "conclusion": serde_json::Value::Null,
        "html_url": "https://github.com/o/r/actions/runs/800000002/job/1",
    });
    let listing = serde_json::json!({
        "total_count": 2,
        "check_runs": [never_ran_run, queued_run],
    })
    .to_string();
    let never_ran_jobs = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]}).to_string();
    let annotation =
        serde_json::json!([{"message": "no minutes remaining on this account"}]).to_string();

    let evidence = trace.path().join("evidence.txt");
    std::fs::write(
        &evidence,
        "Checked the account's Actions billing page directly: quota exhausted this week.",
    )
    .expect("the evidence file is writable");

    let lane_path = format!("commits/{head}/check-runs");
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline, "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": lane_path, "stdout": listing, "status": 0 },
        { "matches": "actions/runs/800000001/jobs", "stdout": never_ran_jobs, "status": 0 },
        { "matches": "check-runs/800100/annotations", "stdout": annotation, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs_dir = home.join(".estigia").join("runs");
    let _ = std::fs::remove_file(runs_dir.join(format!("{reviewer}.json")));
    let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs_dir, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "record_computed_verdict", "arguments": {
            "issue": 12,
            "run_id": reviewer,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "worktree": repo.display().to_string(),
            "never_ran_evidence_file": evidence.display().to_string(),
        }}
    })
    .to_string();

    let log = trace.path().join("unfinished-sibling.log");
    let mut child = tracker_command(home, repo, bin, &script)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::{BufRead, Write};
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = std::io::BufReader::new(child.stdout.take().expect("stdout is piped"));
    let read_line = |stdout: &mut std::io::BufReader<std::process::ChildStdout>| -> String {
        let mut line = String::new();
        stdout.read_line(&mut line).expect("the server answers");
        assert!(!line.trim().is_empty(), "the server answered nothing");
        line
    };
    writeln!(stdin, "{request}").expect("the verdict request is written");
    let response: serde_json::Value = serde_json::from_str(read_line(&mut stdout).trim())
        .unwrap_or_else(|_| panic!("the MCP response is not JSON"));
    drop(stdin);
    child.wait().expect("the MCP server exits");

    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let calls = std::fs::read_to_string(&log).unwrap_or_default();

    assert!(
        calls.contains("actions/runs/800000001/jobs"),
        "the never-ran run's jobs were never read: {calls}"
    );
    assert_eq!(
        response["result"]["isError"], true,
        "a head with an unfinished sibling check run cleared a verdict with evidence supplied: \
         {text}"
    );
    assert!(
        text.contains("publication-lane-unfinished"),
        "the unfinished sibling was not refused as unfinished: {text}"
    );
    assert!(
        !text.contains("publication-lane-never-ran"),
        "a lane with an unfinished sibling was cleared for evidence, which never-ran's own \
         refusal would invite: {text}"
    );
    assert!(
        !calls.contains("issue comment"),
        "a refused verdict still wrote to the timeline: {calls}"
    );
}

/// A judge's own severe finding on a first round: `read_publication_lane`
/// returned on the **first** completed non-green check run, so a head carrying
/// two of them — one whose run never executed a step and one that genuinely
/// ran and failed — was accepted when the never-ran one happened to be listed
/// first and refused when the order was reversed. The gate's answer depended
/// on GitHub's listing order.
///
/// Both orderings of the same two check runs are posed here, evidence
/// supplied both times, and both must refuse `publication-lane-red` naming
/// the run that ran — never `publication-lane-never-ran`, and never an
/// acceptance. The advisory `publication_lane` is asked the same question in
/// both orderings too, through the shared classifier, and must agree.
#[test]
fn a_head_with_two_failing_runs_refuses_on_either_listing_order() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let reviewer = "gemini-c0ffee00";
    let publisher = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    let timeline = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:review"}],
        "comments": [held, publication],
    })
    .to_string();

    // Two check runs, two distinct workflow runs, one never-ran and one that
    // genuinely ran and failed. Different `run_url`s so they resolve to
    // different `gh run view` ids — the whole point of "distinct workflow
    // runs only".
    let never_ran_run = serde_json::json!({
        "id": 9001_u64,
        "name": "check (never-ran)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/500000001/job/1",
    });
    let ran_and_failed_run = serde_json::json!({
        "id": 9002_u64,
        "name": "check (ran-and-failed)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/500000002/job/2",
    });

    let never_ran_jobs = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]}).to_string();
    let ran_jobs = serde_json::json!({
        "total_count": 1,
        "jobs": [{"steps": [{"name": "pytest", "conclusion": "failure"}]}],
    })
    .to_string();
    let annotation =
        serde_json::json!([{"message": "no minutes remaining on this account"}]).to_string();

    let evidence = trace.path().join("evidence.txt");
    std::fs::write(
        &evidence,
        "Checked the account's Actions billing page directly: quota exhausted this week.",
    )
    .expect("the evidence file is writable");

    let lane_path = format!("commits/{head}/check-runs");

    for (label, runs) in [
        (
            "never-ran-listed-first",
            vec![never_ran_run.clone(), ran_and_failed_run.clone()],
        ),
        (
            "ran-and-failed-listed-first",
            vec![ran_and_failed_run, never_ran_run],
        ),
    ] {
        let listing =
            serde_json::json!({"total_count": runs.len(), "check_runs": runs}).to_string();
        let log = trace.path().join(format!("{label}.log"));
        let script = serde_json::to_string(&serde_json::json!([
            { "matches": "issue view", "stdout": timeline, "status": 0 },
            { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
            { "matches": lane_path, "stdout": listing, "status": 0 },
            { "matches": "actions/runs/500000001/jobs", "stdout": never_ran_jobs, "status": 0 },
            { "matches": "actions/runs/500000002/jobs", "stdout": ran_jobs, "status": 0 },
            { "matches": "check-runs/9001/annotations", "stdout": annotation, "status": 0 },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");

        let runs_dir = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs_dir.join(format!("{reviewer}.json")));
        let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs_dir, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "record_computed_verdict", "arguments": {
                "issue": 12,
                "run_id": reviewer,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
            "worktree": repo.display().to_string(),
                "never_ran_evidence_file": evidence.display().to_string(),
            }}
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &script)
            .arg("mcp")
            .env(
                "ESTIGIA_FAKE_COUNT",
                trace.path().join(format!("{label}-count.json")),
            )
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::{BufRead, Write};
        let mut stdin = child.stdin.take().expect("stdin is piped");
        let mut stdout = std::io::BufReader::new(child.stdout.take().expect("stdout is piped"));
        let read_line = |stdout: &mut std::io::BufReader<std::process::ChildStdout>| -> String {
            let mut line = String::new();
            stdout.read_line(&mut line).expect("the server answers");
            assert!(!line.trim().is_empty(), "the server answered nothing");
            line
        };
        writeln!(stdin, "{request}").expect("the verdict request is written");
        let response: serde_json::Value = serde_json::from_str(read_line(&mut stdout).trim())
            .unwrap_or_else(|_| panic!("the MCP response is not JSON"));
        drop(stdin);
        child.wait().expect("the MCP server exits");

        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();

        assert!(
            calls.contains("actions/runs/500000001/jobs")
                && calls.contains("actions/runs/500000002/jobs"),
            "{label}: not every distinct workflow run on the head was classified: {calls}"
        );
        assert_eq!(
            response["result"]["isError"], true,
            "{label}: a head with a genuinely-failed run beside a never-ran one cleared a verdict \
             even with evidence supplied: {text}"
        );
        assert!(
            text.contains("publication-lane-red"),
            "{label}: refused for the wrong reason, listing order decided the outcome: {text}"
        );
        assert!(
            !text.contains("publication-lane-never-ran"),
            "{label}: the mixed head was told apart as never-ran: {text}"
        );
        assert!(
            text.contains("check (ran-and-failed)") || text.contains("500000002"),
            "{label}: the refusal does not name the run that actually ran: {text}"
        );
    }
}

/// No test drove a blank `--never-ran-evidence-file` through the actual
/// never-ran path before this: `blank-never-ran-evidence` existed only in
/// `claim.rs` and in the binding, unreached by anything that could notice it
/// stop firing. A file that is empty, nothing but whitespace, or filler such
/// as `n/a` — the check was widened from `is_empty` to `says_nothing` —
/// must be told apart from the argument being absent: absent refuses
/// `publication-lane-never-ran`, blank or filler refuses `blank-never-ran-evidence`.
#[test]
fn a_blank_or_filler_never_ran_evidence_file_is_told_apart_from_no_file_at_all() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let reviewer = "gemini-c0ffee00";
    let publisher = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    let timeline = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:review"}],
        "comments": [held, publication],
    })
    .to_string();

    let check_run = serde_json::json!({
        "id": 900100_u64,
        "name": "check (never-ran)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/900000001/job/1",
    });
    let listing = serde_json::json!({"total_count": 1, "check_runs": [check_run]}).to_string();
    let never_ran_jobs = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]}).to_string();
    let annotation =
        serde_json::json!([{"message": "no minutes remaining on this account"}]).to_string();

    let evidence = trace.path().join("never-ran-evidence.txt");

    let lane_path = format!("commits/{head}/check-runs");
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline, "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": lane_path, "stdout": listing, "status": 0 },
        { "matches": "actions/runs/900000001/jobs", "stdout": never_ran_jobs, "status": 0 },
        { "matches": "check-runs/900100/annotations", "stdout": annotation, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs_dir = home.join(".estigia").join("runs");
    let _ = std::fs::remove_file(runs_dir.join(format!("{reviewer}.json")));
    let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs_dir, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "record_computed_verdict", "arguments": {
            "issue": 12,
            "run_id": reviewer,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "worktree": repo.display().to_string(),
            "never_ran_evidence_file": evidence.display().to_string(),
        }}
    })
    .to_string();

    use std::io::Write;
    for (label, contents) in [("blank", "   \n\t"), ("filler", "n/a")] {
        std::fs::write(&evidence, contents).expect("the evidence file is writable");

        let log = trace.path().join(format!("{label}-never-ran-evidence.log"));
        let mut child = tracker_command(home, repo, bin, &script)
            .arg("mcp")
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "{label}: the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let calls = std::fs::read_to_string(&log).unwrap_or_default();

        assert_eq!(
            response["result"]["isError"], true,
            "{label}: a {contents:?} --never-ran-evidence-file cleared the head: {text}"
        );
        // The reason is rendered as its own parenthesised line, distinct from
        // the detail prose that also names `publication-lane-never-ran` as the
        // *other* case's reason — a plain substring check on the whole text
        // would pass on that mention alone.
        assert!(
            text.contains("(blank-never-ran-evidence)"),
            "{label}: a {contents:?} evidence file was not told apart from an absent one: {text}"
        );
        assert!(
            !text.contains("(publication-lane-never-ran)"),
            "{label}: a {contents:?} evidence file was refused as though none had been supplied \
             at all: {text}"
        );
        assert!(
            !calls.contains("issue comment"),
            "{label}: a refused verdict still wrote to the timeline: {calls}"
        );
    }
}

/// A judge's own severe finding on a first round: `never_ran_signal`
/// read the jobs listing with no `--paginate` and no `per_page`, so a run
/// with more jobs than one default page answered from one page, and
/// `jobs_report_never_ran`'s `.all()` never consulted `total_count` — a
/// partial page of `steps: []` jobs was read as proof the whole run
/// never executed, exactly the shape `read_publication_lane`'s own doc
/// comment calls "the failure this crate is named for". The jobs read now
/// carries `per_page=100`, matching the sibling check-runs read, so this
/// poses 101 against a page of 100.
///
/// A single red check run, with evidence supplied, whose jobs endpoint
/// answers `total_count: 101` and one hundred jobs, every one reporting
/// `steps: []`. The end-to-end path must refuse `publication-lane-red` —
/// never accept, and never `publication-lane-never-ran` — because a partial
/// read proves nothing about the job it did not see.
#[test]
fn a_partial_jobs_page_refuses_rather_than_clearing_the_head() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let reviewer = "gemini-c0ffee00";
    let publisher = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let (base, head) = low_delivery(repo);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let held = comment(
        "IC_claim",
        "2026-08-14T09:00:00Z",
        marker(
            "claim",
            &[
                ("run-id", reviewer),
                ("runtime", "gemini"),
                ("horizon", "2099-01-01T00:00Z"),
                ("op-id", &"a".repeat(32)),
            ],
        ),
    );
    let publication = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    let timeline = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:review"}],
        "comments": [held, publication],
    })
    .to_string();

    let check_run = serde_json::json!({
        "id": 7001_u64,
        "name": "check (matrix)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/600000001/job/1",
    });
    let listing = serde_json::json!({"total_count": 1, "check_runs": [check_run]}).to_string();
    // 101 jobs exist; only 100 arrived on this page, every one reporting no
    // steps at all — the partial-page shape.
    let partial_jobs = serde_json::json!({
        "total_count": 101,
        "jobs": (0..100).map(|_| serde_json::json!({"steps": []})).collect::<Vec<_>>(),
    })
    .to_string();

    let evidence = trace.path().join("evidence.txt");
    std::fs::write(&evidence, "Checked the billing page; nothing conclusive.")
        .expect("the evidence file is writable");

    let lane_path = format!("commits/{head}/check-runs");
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline, "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": lane_path, "stdout": listing, "status": 0 },
        { "matches": "actions/runs/600000001/jobs", "stdout": partial_jobs, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs_dir = home.join(".estigia").join("runs");
    let _ = std::fs::remove_file(runs_dir.join(format!("{reviewer}.json")));
    let mut run = estigia::harness::session::Run::new(reviewer.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs_dir, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "record_computed_verdict", "arguments": {
            "issue": 12,
            "run_id": reviewer,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "worktree": repo.display().to_string(),
            "never_ran_evidence_file": evidence.display().to_string(),
        }}
    })
    .to_string();

    let log = trace.path().join("partial-page.log");
    let mut child = tracker_command(home, repo, bin, &script)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LOG", &log)
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
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let calls = std::fs::read_to_string(&log).unwrap_or_default();

    assert!(
        calls.contains("actions/runs/600000001/jobs?per_page=100"),
        "the jobs listing was not read with per_page=100, matching the check-runs read: {calls}"
    );
    assert_eq!(
        response["result"]["isError"], true,
        "a partial jobs page cleared the head even with evidence supplied: {text}"
    );
    assert!(
        text.contains("publication-lane-red"),
        "a partial jobs page was not refused as an ordinary red lane: {text}"
    );
    assert!(
        !text.contains("publication-lane-never-ran"),
        "a partial jobs page was told apart as never-ran, which is exactly what a partial read \
         cannot prove: {text}"
    );
}

/// `review_status` carries the publication lane and the correction
/// count, driven through the real MCP server so the argument-to-flag wiring is
/// crossed.
///
/// The lane is read for the latest publication's own head, in the three states
/// a run consults before launching a panel — `never-ran` with its annotation
/// and its "Launch" route, `running` with its wait, and a genuinely `red` lane
/// with "do not launch". The corrections are the one lineage correction the
/// timeline spent, read from the same answer: the count that used to be a tool
/// of its own.
#[test]
fn review_status_answers_the_lane_and_the_correction_state_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let publisher = "claude-abcd1234";
    let reviewer = "codex-b";

    // Three real commits: the tier the receipt's own base..head reads needs the
    // objects, and a correction lineage needs three distinct heads.
    let base = checked_git_stdout(repo, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let commit = |content: &str| -> String {
        std::fs::write(repo.join("notes.md"), content).expect("a file to commit");
        checked_git(repo, &["add", "notes.md"]);
        checked_git(
            repo,
            &[
                "-c",
                "user.email=nobody@example.invalid",
                "-c",
                "user.name=nobody",
                "commit",
                "--quiet",
                "-m",
                "notes",
            ],
        );
        checked_git_stdout(repo, &["rev-parse", "HEAD"])
            .trim()
            .to_owned()
    };
    let head1 = commit("A line of prose, reworded.\n");
    let head2 = commit("A line of prose, reworded once more.\n");
    let head3 = commit("A line of prose, reworded a third time.\n");

    let epoch1 = "1".repeat(32);
    let epoch2 = "2".repeat(32);
    let epoch3 = "3".repeat(32);
    let digest1 = "1".repeat(64);
    let digest2 = "2".repeat(64);
    let digest3 = "3".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let comments = vec![
        comment(
            "IC_1",
            "2026-08-14T09:00:00Z",
            marker(
                "published",
                &[
                    ("run-id", publisher),
                    ("epoch", &epoch1),
                    ("pr", "7"),
                    ("head", &head1),
                    ("base", &base),
                    ("digest", &digest1),
                ],
            ),
        ),
        comment(
            "IC_2",
            "2026-08-14T09:10:00Z",
            marker(
                "review-verdict",
                &[
                    ("run-id", reviewer),
                    ("reviewer", reviewer),
                    ("op-id", &"a".repeat(32)),
                    ("epoch", &epoch1),
                    ("pr", "7"),
                    ("head", &head1),
                    ("base", &base),
                    ("digest", &digest1),
                    ("outcome", "accepted"),
                ],
            ),
        ),
        // A new candidate over an approved parent: a lineage that spends no
        // correction.
        comment(
            "IC_3",
            "2026-08-14T09:20:00Z",
            marker(
                "published",
                &[
                    ("run-id", publisher),
                    ("epoch", &epoch2),
                    ("pr", "7"),
                    ("head", &head2),
                    ("base", &base),
                    ("digest", &digest2),
                    ("parent", &epoch1),
                    ("parent-pr", "7"),
                    ("parent-head", &head1),
                    ("parent-base", &base),
                    ("parent-digest", &digest1),
                    ("delta", "delta-1-2"),
                ],
            ),
        ),
        comment(
            "IC_4",
            "2026-08-14T09:30:00Z",
            marker(
                "review-verdict",
                &[
                    ("run-id", reviewer),
                    ("reviewer", reviewer),
                    ("op-id", &"b".repeat(32)),
                    ("epoch", &epoch2),
                    ("pr", "7"),
                    ("head", &head2),
                    ("base", &base),
                    ("digest", &digest2),
                    ("outcome", "rejected"),
                ],
            ),
        ),
        // The correction: the parent's own latest verdict is `rejected`.
        comment(
            "IC_5",
            "2026-08-14T09:40:00Z",
            marker(
                "published",
                &[
                    ("run-id", publisher),
                    ("epoch", &epoch3),
                    ("pr", "7"),
                    ("head", &head3),
                    ("base", &base),
                    ("digest", &digest3),
                    ("parent", &epoch2),
                    ("parent-pr", "7"),
                    ("parent-head", &head2),
                    ("parent-base", &base),
                    ("parent-digest", &digest2),
                    ("delta", "delta-2-3"),
                ],
            ),
        ),
    ];
    seed_store_with(home, &[(12, "review", comments)]);

    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head3,
        "baseRefOid": base,
        "state": "OPEN",
        "isDraft": false,
    });
    let issue = serde_json::json!({
        "state": "open",
        "repository_url": "https://api.github.com/repos/o/r",
    });
    let repo_view = "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}";
    let never_ran_run = serde_json::json!({
        "id": 700100_u64,
        "name": "check (never-ran)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/700000001/job/1",
    });
    let queued_run = serde_json::json!({
        "id": 700200_u64,
        "name": "check (still-queued)",
        "status": "queued",
        "conclusion": serde_json::Value::Null,
        "html_url": "https://github.com/o/r/actions/runs/700000002/job/1",
    });
    let ran_run = serde_json::json!({
        "id": 700300_u64,
        "name": "check (ran-and-failed)",
        "status": "completed",
        "conclusion": "failure",
        "html_url": "https://github.com/o/r/actions/runs/700000003/job/1",
    });
    let never_ran_jobs = serde_json::json!({"total_count": 1, "jobs": [{"steps": []}]}).to_string();
    let ran_jobs = serde_json::json!({
        "total_count": 1,
        "jobs": [{"steps": [{"name": "pytest", "conclusion": "failure"}]}],
    })
    .to_string();
    let annotation =
        serde_json::json!([{"message": "no minutes remaining on this account"}]).to_string();
    let lane_path = format!("commits/{head3}/check-runs");

    let rows: [(&str, Vec<serde_json::Value>, &str, &str); 3] = [
        (
            "never-ran",
            vec![never_ran_run.clone()],
            "never-ran",
            "Launch",
        ),
        (
            "running",
            vec![never_ran_run, queued_run],
            "running",
            "wait",
        ),
        ("red", vec![ran_run], "red", "do not launch"),
    ];
    for (label, runs, state, action) in rows {
        let listing =
            serde_json::json!({"total_count": runs.len(), "check_runs": runs}).to_string();
        let script = serde_json::to_string(&serde_json::json!([
            // The comments endpoint is named before the issue endpoint: a
            // substring match on `issues/12` alone would answer the comments
            // read with the issue object and break the sync.
            { "matches": "api repos/{owner}/{repo}/issues/12/comments", "stdout": "[]", "status": 0 },
            { "matches": "api repos/{owner}/{repo}/issues/12", "stdout": issue.to_string(), "status": 0 },
            { "matches": "pr view", "stdout": pr.to_string(), "status": 0 },
            { "matches": "repo view", "stdout": repo_view, "status": 0 },
            { "matches": lane_path, "stdout": listing, "status": 0 },
            { "matches": "actions/runs/700000001/jobs", "stdout": never_ran_jobs, "status": 0 },
            { "matches": "actions/runs/700000003/jobs", "stdout": ran_jobs, "status": 0 },
            { "matches": "check-runs/700100/annotations", "stdout": annotation, "status": 0 },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "review_status", "arguments": { "issue": 12 } }
        })
        .to_string();

        let mut child = tracker_command(home, repo, bin, &script)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "{label}: the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert_ne!(
            response["result"]["isError"], true,
            "{label}: review_status refused: {text}"
        );
        let answer: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|_| panic!("{label}: {text}"));
        assert_eq!(answer["lane"]["state"], state, "{label}: {text}");
        assert!(
            answer["lane"]["action"]
                .as_str()
                .is_some_and(|said| said.contains(action)),
            "{label}: the lane action does not say {action:?}: {text}"
        );
        if state == "never-ran" {
            assert!(
                answer["lane"]["annotation"]
                    .as_str()
                    .is_some_and(|said| said.contains("minutes")),
                "{label}: the never-ran lane carries no annotation: {text}"
            );
        }
        assert_eq!(answer["corrections"]["corrections"], 1, "{label}: {text}");
        assert_eq!(
            answer["corrections"]["correction_limit"], 3,
            "{label}: {text}"
        );
        assert_eq!(
            answer["corrections"]["correction_spent"], false,
            "{label}: {text}"
        );
    }
}

/// A `review_status` sub-read that fails is embedded, never
/// aborts the answer.
///
/// The lane read already failed that way; the correction read used to abort
/// with `?`, hiding the `next` and `forecast` the caller actually asked for.
/// The two are made to fail the same way, and this poses the correction read
/// failing while the answer's own read succeeds: the comments sync answers an
/// object where a list belongs.
#[test]
fn review_status_embeds_a_failed_sub_read_instead_of_aborting_the_answer() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let publisher = "claude-abcd1234";

    let base = checked_git_stdout(repo, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    std::fs::write(repo.join("notes.md"), "A line of prose, reworded.\n").expect("a file");
    checked_git(repo, &["add", "notes.md"]);
    checked_git(
        repo,
        &[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            "notes",
        ],
    );
    let head = checked_git_stdout(repo, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let epoch = "e".repeat(32);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comments = vec![serde_json::json!({
        "id": "IC_1",
        "createdAt": "2026-08-14T09:00:00Z",
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    })];
    seed_store_with(home, &[(12, "review", comments)]);

    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head,
        "baseRefOid": base,
        "state": "OPEN",
        "isDraft": false,
    });
    let issue = serde_json::json!({
        "state": "open",
        "repository_url": "https://api.github.com/repos/o/r",
    });
    let green_run = serde_json::json!({
        "id": 710100_u64,
        "name": "check (green)",
        "status": "completed",
        "conclusion": "success",
        "html_url": "https://github.com/o/r/actions/runs/710000001/job/1",
    });
    let listing = serde_json::json!({"total_count": 1, "check_runs": [green_run]}).to_string();
    let lane_path = format!("commits/{head}/check-runs");
    let script = serde_json::to_string(&serde_json::json!([
        // An object where the sync expects a list of comments: the correction
        // read cannot answer, while every read before it can.
        { "matches": "api repos/{owner}/{repo}/issues/12/comments", "stdout": "{}", "status": 0 },
        { "matches": "api repos/{owner}/{repo}/issues/12", "stdout": issue.to_string(), "status": 0 },
        { "matches": "pr view", "stdout": pr.to_string(), "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": lane_path, "stdout": listing, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "review_status", "arguments": { "issue": 12 } }
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &script)
        .arg("mcp")
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
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        response["result"]["isError"], true,
        "a failed correction read aborted the whole answer: {text}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("{text}"));
    // The answer the caller asked for survives.
    assert!(
        answer["next"]["kind"].is_string(),
        "the failed sub-read hid the next step: {text}"
    );
    assert!(
        answer["forecast"]["horizon"].is_string(),
        "the failed sub-read hid the forecast: {text}"
    );
    // The lane, which answered, is embedded whole; the corrections, which did
    // not, are embedded as the read failure it is.
    assert_eq!(answer["lane"]["state"], "green", "{text}");
    assert!(
        answer["corrections"]["reason"].is_string(),
        "a failed correction read was not embedded as its own failure: {text}"
    );
}

/// `review_tier` reads the repair's own narrow range when
/// `parent_head` is given, rather than the whole `base..head`.
#[test]
fn review_tier_reads_the_repair_range_named_by_parent_head_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let base = checked_git_stdout(repo, &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let commit = |content: &str| -> String {
        std::fs::write(repo.join("notes.md"), content).expect("a file to commit");
        checked_git(repo, &["add", "notes.md"]);
        checked_git(
            repo,
            &[
                "-c",
                "user.email=nobody@example.invalid",
                "-c",
                "user.name=nobody",
                "commit",
                "--quiet",
                "-m",
                "notes",
            ],
        );
        checked_git_stdout(repo, &["rev-parse", "HEAD"])
            .trim()
            .to_owned()
    };
    let parent_head = commit("A line of prose, reworded.\n");
    let head = commit("A line of prose, reworded once more.\n");

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "review_tier", "arguments": {
            "base": base,
            "head": head,
            "parent_head": parent_head,
            "worktree": repo.display().to_string(),
        }}
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, "[]")
        .arg("mcp")
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
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        response["result"]["isError"], true,
        "review_tier refused a repair round: {text}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("{text}"));
    assert_eq!(
        answer["range"], "repair",
        "parent_head did not select the repair's own narrow range: {text}"
    );
    assert_eq!(
        answer["base"], parent_head,
        "the repair range is not parent_head..head: {text}"
    );
    assert!(
        answer["level"].is_string(),
        "the repair range carried no level: {text}"
    );
}

/// `target` classifies base movement when `recorded_base` is
/// given, rather than answering the delivery target.
#[test]
fn target_classifies_base_movement_when_recorded_base_is_given_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let origin = tempfile::tempdir().expect("a bare origin");
    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    let sha = |what: &str| -> String {
        checked_git_stdout(repo, &["rev-parse", what])
            .trim()
            .to_owned()
    };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    std::fs::write(repo.join("base.txt"), "base\n").expect("the base file");
    git(&["add", "base.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]);
    git(&["push", "-q", "origin", "main"]);
    let recorded_base = sha("HEAD");

    // The delivery head: a branch off the recorded base.
    git(&["checkout", "-q", "-b", "fix/12"]);
    std::fs::write(repo.join("head.txt"), "head\n").expect("the head file");
    git(&["add", "head.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "the change under review",
    ]);
    let head = sha("HEAD");

    // The base advances under the review, touching a disjoint path.
    git(&["checkout", "-q", "main"]);
    std::fs::write(repo.join("later.txt"), "later\n").expect("the later file");
    git(&["add", "later.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "a later base commit",
    ]);
    git(&["push", "-q", "origin", "main"]);
    let current_base = sha("HEAD");
    git(&["checkout", "-q", "fix/12"]);

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "target", "arguments": {
            "base": "main",
            "recorded_base": recorded_base,
            "worktree": repo.display().to_string(),
        }}
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, "[]")
        .arg("mcp")
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
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_ne!(
        response["result"]["isError"], true,
        "target refused a base-movement read: {text}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("{text}"));
    assert_eq!(
        answer["recorded_base"], recorded_base,
        "recorded_base did not select the base-movement answer: {text}"
    );
    assert_eq!(answer["current_base"], current_base, "{text}");
    assert_eq!(answer["head"], head, "{text}");
    assert!(
        answer["movement"].is_string(),
        "the base-movement answer carried no classification: {text}"
    );
    assert!(
        !answer
            .as_object()
            .is_some_and(|object| object.contains_key("manifest")),
        "recorded_base answered the delivery target instead: {text}"
    );
}
