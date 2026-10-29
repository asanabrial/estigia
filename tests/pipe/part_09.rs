use super::*;

/// Final elected round: the `handoff_review` sender pushes
/// `--runtime` from the validated `NamedRun.runtime`
/// (`run_tool` in `src/harness/mcp/mod.rs`) rather than the constant — and that value
/// reaches the timeline through the `unassign` write `handoff_review`
/// performs internally to release the current epoch. Adapted from
/// [`review_handoff_orders_evidence_release_and_pointer_clear`]'s own working
/// fixture, since `handoff_review` has no CLI subcommand and is reachable only
/// over MCP.
#[test]
fn a_non_claude_run_id_hands_off_review_under_its_own_runtime_not_the_constant() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let bodies = tempfile::tempdir().expect("a body-log directory");
    let body_log = bodies.path().join("bodies.log");
    let run_id = "codex-abcd1234";
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let blocker = "no independent context in this run";
    let discharger = "another run records the exact-receipt verdict";
    let operation = estigia::transport::claim::review_operation_id(
        "review-handoff",
        &[
            run_id, &target, &epoch, "7", &head, &base, &digest, blocker, discharger,
        ],
    );
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let claim = marker(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "codex"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &target),
        ],
    );
    let publication = marker(
        "published",
        &[
            ("run-id", run_id),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
        ],
    );
    let handoff = marker(
        "review-handoff",
        &[
            ("run-id", run_id),
            ("target-op", &target),
            ("op-id", &operation),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
            ("authority", "ask"),
            ("requested-at", "2026-08-14T10:00:00Z"),
            ("deadline", "2026-08-14T10:00:00Z"),
            ("blocker", blocker),
            ("discharger", discharger),
        ],
    );
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let before = vec![
        comment("IC_claim", "2026-08-14T09:00:00Z", claim),
        comment("IC_publication", "2026-08-14T09:10:00Z", publication),
    ];
    let mut after_handoff = before.clone();
    after_handoff.push(comment("IC_handoff", "2026-08-14T10:00:00Z", handoff));
    let issue = |state: &str, comments: &[serde_json::Value]| {
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": format!("status:{state}")}],
            "comments": comments,
        })
        .to_string()
    };
    // Unbounded rather than a precise `nth` count: this test cares only about
    // the runtime the release marker carries, which is recorded in the store.
    let mut bodies_script = vec![issue("review", &before); 6];
    bodies_script.extend((0..40).map(|_| issue("review", &after_handoff)));
    let mut answers = bodies_script
        .iter()
        .enumerate()
        .map(|(index, stdout)| {
            serde_json::json!({
                "matches": "issue view",
                "nth": index + 1,
                "stdout": stdout,
                "status": 0,
            })
        })
        .collect::<Vec<_>>();
    answers.push(serde_json::json!({
        "matches": "api user",
        "stdout": "{\"login\":\"fixture\"}",
        "status": 0,
    }));
    let script = serde_json::to_string(&answers).expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "handoff_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "target_operation": target,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": blocker,
                "discharger": discharger,
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let count = bodies.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &script)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_BODY_LOG", &body_log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let _ = child.wait_with_output().expect("the MCP server exits");

    let bodies = local_entries(home)
        .into_iter()
        .filter(|body| body.contains("issue-flow: unassign"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        bodies.contains("runtime=codex"),
        "a non-Claude run id's own handoff release did not carry its own runtime: {bodies}"
    );
    assert!(
        !bodies.contains("runtime=claude"),
        "a non-Claude run id's handoff release recorded the constant instead of its own \
         runtime: {bodies}"
    );
}

/// Author-elected round: the adopted-operation arm at
/// `src/transport/claim.rs:6411` checks `adopted_expected` against the
/// **live, already-persisted** marker `may_claim` adopts — not the caller's
/// own fresh id, which is why neither
/// [`a_non_claude_run_id_projects_its_own_runtime_not_the_constant`] nor
/// [`a_claim_renews_across_the_build_boundary`] reaches it: the first never
/// disagrees with its own minted marker, the second resumes its own
/// operation id directly and never calls `may_claim` at all. This drives a
/// genuinely first call — no pointer seeded, so `run_tool` mints a fresh
/// operation id of its own — against a live marker already on the timeline
/// under a *different* operation id, written carrying a runtime this run's
/// own id does not derive.
#[test]
fn a_claim_adopts_a_live_marker_under_the_runs_own_runtime() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let run_id = "codex-abcd1234";
    let horizon = "2099-01-01T00:00Z";
    let adopted_epoch = "2".repeat(32);

    // The live claim `may_claim` will adopt: a different, already-minted
    // operation id, with `runtime=claude` even though `run_id`'s own runtime
    // is `codex`. No pointer is seeded, so `run_tool` mints its own fresh id
    // and never matches this one directly — `operation_marker` for the fresh
    // id finds nothing, and `may_claim` finds this marker only by `run_id`.
    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &adopted_epoch),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:ready"}, {"name": "dev:claude"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-08-17T16:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": held.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the script serialises");

    let output = tracker_command(home, repo, bin, &script)
        .env("ESTIGIA_FAKE_LOG", &log)
        .args(["claim", "12", "--run-id", run_id, "--horizon", horizon])
        .output()
        .expect("estigia claim runs");
    assert!(
        output.status.success(),
        "adopting a live marker under this run's own (different) runtime was refused: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The person-facing road, end to end: `estigia claim` without `--json`
/// re-indents the transport answer for a terminal. The two unit tests on
/// `indented_for_a_person` invoke it directly and stay green if the call is
/// removed, so this drives the real binary and reads what a person is handed.
#[test]
fn a_claim_to_a_person_prints_the_transport_answer_indented() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let horizon = "2099-01-01T00:00Z";
    let epoch = "2".repeat(32);

    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &epoch),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:ready"}, {"name": "dev:claude"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-08-17T16:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": held.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the script serialises");

    let output = tracker_command(home, repo, bin, &script)
        .args(["claim", "12", "--run-id", run_id, "--horizon", horizon])
        .output()
        .expect("estigia claim runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "a claim a person asked for was refused: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The answer is the block before the `sworn:` line that follows it. The
    // sworn lines make the whole of stdout multi-line either way, so the
    // assertion has to read the transport answer itself.
    let answer = stdout.split("sworn:").next().unwrap_or_default();
    assert!(
        answer.lines().count() > 1,
        "the transport answer reached the terminal on the model's single line: {stdout}"
    );
}

/// Severe 1: a claim marker persisted under an operation id this
/// run already holds — `run_tool` reuses it rather than minting fresh, the
/// documented renewal in `run_tool` (`src/harness/mcp/mod.rs`) — must be readable
/// under the run's own current runtime even when the marker was written
/// carrying a different one.
///
/// The pointer is seeded by hand rather than built from a prior `estigia
/// claim` call: a genuine first write mints its operation id from the clock,
/// unknowable in advance, so a second call scripted to find it would adopt
/// it (`claim`'s adopted-id path — already covered by
/// [`a_non_claude_run_id_projects_its_own_runtime_not_the_constant`]) rather
/// than reuse it directly, which is the shape severe 1 is actually about.
#[test]
fn a_claim_renews_across_the_build_boundary() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let run_id = "codex-abcd1234";
    let epoch = "1".repeat(32);
    let horizon = "2099-01-01T00:00Z";

    // The pointer a prior successful `claim` under this run's own id would
    // have left: this issue, this operation id, nothing else needed.
    let runs_dir = home.join(".estigia").join("runs");
    std::fs::create_dir_all(&runs_dir).expect("the pointer directory");
    std::fs::write(
        runs_dir.join(format!("{run_id}.json")),
        serde_json::json!({
            "run_id": run_id,
            "issue": 12,
            "operation_id": epoch,
        })
        .to_string(),
    )
    .expect("the pointer seeds");

    // The marker a build that hard-coded the runtime would have written.
    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &epoch),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:ready"}, {"name": "dev:claude"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-08-17T16:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": held.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the script serialises");

    let output = tracker_command(home, repo, bin, &script)
        .env("ESTIGIA_FAKE_LOG", &log)
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--json",
        ])
        .output()
        .expect("the renewal runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "a renewal under this run's own current runtime was refused: {said}"
    );
    assert!(
        said.contains("\"reused_existing_claim\": true"),
        "the renewal did not reuse the operation id already on the pointer: {said}"
    );
}

/// A claim whose write lands and whose readback fails can be retried.
///
/// The pointer records the minted key and not the issue. The retry mints a
/// fresh key, finds this run already live, and used to refuse
/// `already-owned-by-different-operation`. Adopting the timeline epoch writes
/// no second comment and lets Swear complete the pointer. Revert the
/// `AlreadyWritten` arm in `may_claim` to restore that refusal.
#[test]
fn a_claim_retry_after_a_landed_write_whose_readback_failed_adopts_the_epoch() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let count = tempfile::NamedTempFile::new().expect("a count file");
    let log = tempfile::NamedTempFile::new().expect("a call log");
    let run_id = "claude-abcd1234";
    let empty = serde_json::json!({
        "state": "OPEN",
        "assignees": [],
        "labels": [{"name": "status:ready"}],
        "comments": [],
    });
    let first = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "nth": 1,
            "stdout": empty.to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "nth": 2,
            "stdout": empty.to_string(),
            "status": 0,
        },
        { "matches": "issue view", "stdout": "", "status": 3 },
        { "matches": "label create", "stdout": "", "status": 0 },
        { "matches": "issue comment", "stdout": "", "status": 0 },
    ]))
    .expect("the first script serialises");

    let mut first_cmd = tracker_command(home, repo, bin, &first);
    first_cmd.env("ESTIGIA_FAKE_COUNT", count.path());
    first_cmd.env("ESTIGIA_FAKE_LOG", log.path());
    let first_out = first_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            "2099-01-01T00:00Z",
            "--json",
        ])
        .output()
        .expect("the first claim runs");
    assert!(
        !first_out.status.success(),
        "the failed readback was treated as success: {}",
        String::from_utf8_lossy(&first_out.stderr)
    );

    let pointer = home
        .join(".estigia")
        .join("runs")
        .join(format!("{run_id}.json"));
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    assert!(
        recorded.get("issue").and_then(|v| v.as_u64()).is_none(),
        "the failed readback completed the pointer: {recorded}"
    );
    let epoch = recorded
        .get("operation_id")
        .and_then(|v| v.as_str())
        .expect("the mint was stored")
        .to_owned();

    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &epoch),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:ready"}, {"name": "dev:claude"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-08-17T16:00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let second = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": held.to_string(),
            "status": 0,
        },
        rest_listing(held["comments"].as_array().expect("comments")),
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the second script serialises");

    let before = std::fs::read_to_string(log.path()).unwrap_or_default();
    let comments_before = before.matches("issue comment").count();

    let mut second_cmd = tracker_command(home, repo, bin, &second);
    second_cmd.env("ESTIGIA_FAKE_LOG", log.path());
    let second_out = second_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            "2099-06-01T00:00Z",
            "--json",
        ])
        .output()
        .expect("the retry runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&second_out.stdout),
        String::from_utf8_lossy(&second_out.stderr)
    );
    assert!(
        second_out.status.success(),
        "the retry did not adopt the landed claim: {said}"
    );
    assert!(
        said.contains("reused_existing_claim") && said.contains("true"),
        "the retry did not say it adopted: {said}"
    );
    assert!(
        said.contains("2099-01-01T00:00Z") && !said.contains("2099-06-01T00:00Z"),
        "the retry reported the horizon it asked for rather than the one on the timeline: {said}"
    );

    let after = std::fs::read_to_string(log.path()).unwrap_or_default();
    assert_eq!(
        after.matches("issue comment").count(),
        comments_before,
        "the retry wrote a second claim comment: {after}"
    );

    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    assert_eq!(recorded.get("issue").and_then(|v| v.as_u64()), Some(12));
    assert_eq!(
        recorded.get("state").and_then(|v| v.as_str()),
        Some("in-progress")
    );
    assert!(
        recorded.get("repo_dir").and_then(|v| v.as_str()).is_some(),
        "the retry left repo_dir empty: {recorded}"
    );
}

/// A run whose own claim has lapsed cannot take the issue back through
/// `claim`, because the operation id `claim` reuses on retry names a **dead**
/// event rather than a live one to resume.
///
/// Reproduced with the run's own holding of the issue having come from a
/// `reclaim` rather than a `claim`, because that is the shape the issue's own
/// evidence measured: `reclaim`, reusing the id, correctly answers
/// `stale-self-requires-claim` (kept, deliberately — displacing yourself is
/// not a takeover). `claim`, reusing the very same id, used to answer
/// `operation-id-kind-conflict`: the marker that id names was written by
/// `reclaim`, and `claim` will not resume an id reserved for a different
/// kind. A run in this shape had no call left that worked.
///
/// Two processes, because the fix reissues under a **freshly minted** id and
/// that id cannot be predicted — it is derived from the clock and the process
/// id, not from anything this test can precompute. The first process poses
/// the lapsed self-claim and its retry; reading the pointer back afterwards is
/// how the fresh id is learned, exactly as
/// `a_claim_retry_after_a_landed_write_whose_readback_failed_adopts_the_epoch`
/// reads it back after its own first process. The second process poses a
/// timeline on which that write landed, and confirms `claim` adopts it rather
/// than writing a second comment.
///
/// Turn the harness's reissue off, or the transport's `claim-operation-
/// expired` check that now runs ahead of the kind and metadata checks that
/// would otherwise answer first, and the first process's assertions against
/// `operation-id-kind-conflict` and a rewritten pointer id go red without ever
/// reaching a second attempt.
#[test]
fn a_lapsed_self_claim_that_originated_as_a_reclaim_can_be_claimed_again() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let old_op = "a".repeat(32);
    let runs = home.join(".estigia").join("runs");

    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    run.operation_id = Some(old_op.clone());
    estigia::harness::session::store(&runs, &run).expect("the fixture pointer writes");

    let displaced_op = "b".repeat(32);
    let displaced_run = "claude-other00000000000000000000";
    let original_marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", displaced_run),
            ("runtime", "claude"),
            ("horizon", "2019-01-01T00:00Z"),
            ("op-id", &displaced_op),
        ],
    )
    .expect("the displaced run's own claim marker");
    let reclaim_marker = estigia::transport::markers::render(
        "reclaim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", "2020-01-01T00:00Z"),
            ("op-id", &old_op),
            ("from", displaced_run),
            ("from-op", &displaced_op),
        ],
    )
    .expect("a reclaim marker");
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    // Old enough that the reclaim it establishes (below) is itself long
    // expired by the time this test's two processes run, which is what makes
    // this run's own holding of the issue a lapsed self-claim rather than a
    // live one.
    let lapsed_comments = vec![
        comment(
            "IC_0",
            "2019-01-01T00:00:00Z",
            format!("Claimed by {displaced_run}.\n\n{original_marker}\n"),
        ),
        comment(
            "IC_1",
            "2019-06-01T00:00:00Z",
            format!("`{run_id}` reclaiming from `{displaced_run}`.\n\n{reclaim_marker}\n"),
        ),
    ];
    // `fixture` already assigned and `dev:claude` already on the issue, so the
    // second process's ownership projection — the assignee/label convergence
    // `claim` runs after `adjudicate` — finds nothing to change and never
    // needs the assignee edit this fake `gh` cannot read back, the same way
    // `a_claim_retry_after_a_landed_write_whose_readback_failed_adopts_the_epoch`
    // pre-settles its own second-phase fixture.
    let issue_view = |comments: &[serde_json::Value]| {
        serde_json::json!({
            "state": "OPEN",
            "assignees": [{"login": "fixture"}],
            "labels": [{"name": "status:in-progress"}, {"name": "dev:claude"}],
            "comments": comments,
        })
        .to_string()
    };

    let horizon = "2099-06-01T00:00Z";
    let count = tempfile::NamedTempFile::new().expect("a count file");
    let first_answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "assignees,labels",
            "nth": 1,
            "stdout": issue_view(&lapsed_comments),
            "status": 0,
        },
        {
            "matches": "assignees,labels",
            "nth": 2,
            "stdout": issue_view(&lapsed_comments),
            "status": 0,
        },
        { "matches": "assignees,labels", "stdout": "", "status": 3 },
        { "matches": "issue view", "stdout": issue_view(&lapsed_comments), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        { "matches": "label create", "stdout": "", "status": 0 },
        { "matches": "issue comment", "stdout": "", "status": 0 },
    ]))
    .expect("the first script serialises");

    // Logged, unlike the rest of this test's first process used to be — this
    // is the only process that exercises the reissue guard
    // (`should_reissue_claim`, `src/harness/mcp/mod.rs`), and without a log
    // there was nothing here that read what the reissue actually wrote,
    // only what it answered.
    let first_trace = tempfile::tempdir().expect("a trace directory");
    let first_log = first_trace.path().join("calls.log");
    let mut first_cmd = tracker_command(home, repo, bin, &first_answers);
    first_cmd.env("ESTIGIA_FAKE_COUNT", count.path());
    first_cmd.env("ESTIGIA_FAKE_LOG", &first_log);
    let first_output = first_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--json",
        ])
        .output()
        .expect("the first attempt runs");
    let out = String::from_utf8_lossy(&first_output.stdout).into_owned();
    let err = String::from_utf8_lossy(&first_output.stderr).into_owned();
    let ok = first_output.status.success();
    let said = format!("{out}{err}");
    assert!(
        !said.contains("operation-id-kind-conflict"),
        "the id reused from the lapsed reclaim still locks claim out: {said}"
    );
    assert!(
        !said.contains("stale-self-requires-claim"),
        "claim answered with reclaim's own refusal: {said}"
    );
    assert!(
        !ok,
        "the fixture's static `issue view` cannot show the write it just posted, so this \
         attempt has to read as an unconfirmed write rather than a success: {said}"
    );
    // The reissue-once guard's outcome, over the real process this test
    // otherwise only reads stdout from: the dead attempt under the old id is
    // refused before it writes, and the reissue under a fresh id records the
    // claim marker exactly once. `should_reissue_claim`'s own
    // `a_fresh_key_is_never_reissued_as_though_it_had_lost_a_race`
    // (`src/harness/mcp/tests.rs`) is what pins the guard's *conjunct*
    // directly — a key minted fresh for this call cannot itself already name
    // a dead event on the timeline, so no fixture built from real `gh`
    // answers can drive this process to the state that predicate's unit test
    // poses. This assertion is the guard's visible effect in the one process
    // that does reach it.
    let first_calls = std::fs::read_to_string(&first_log).unwrap_or_default();
    assert_eq!(
        comment_writes(&first_calls),
        0,
        "a claim posts no comment to GitHub: {first_calls}"
    );
    assert_eq!(
        local_entries(home)
            .iter()
            .filter(|body| body.contains("issue-flow: claim"))
            .count(),
        1,
        "the reissue must record exactly one claim marker in the store"
    );

    let pointer = runs.join(format!("{run_id}.json"));
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    let fresh_op = recorded
        .get("operation_id")
        .and_then(|v| v.as_str())
        .expect("a fresh key was minted and stored")
        .to_owned();
    assert_ne!(
        fresh_op, old_op,
        "the dead id from the lapsed reclaim was reused rather than replaced: {recorded}"
    );
    // The fixture's pointer already named this run as holding #12 in-progress
    // before either process ran — that is what makes this a *lapsed*
    // self-claim rather than a fresh one — so both survive this attempt
    // unchanged: an unconfirmed write is not read as `apply_effect`'s Swear,
    // which is the branch that would otherwise stamp `verified_at`.
    assert_eq!(recorded.get("issue").and_then(|v| v.as_u64()), Some(12));
    assert_eq!(
        recorded.get("state").and_then(|v| v.as_str()),
        Some("in-progress")
    );
    assert!(
        recorded
            .get("verified_at")
            .and_then(|v| v.as_u64())
            .is_none(),
        "an unconfirmed write was read as a verified renewal: {recorded}"
    );

    // The second process poses the world in which that write landed, so the
    // adoption path already proven by
    // `a_claim_retry_after_a_landed_write_whose_readback_failed_adopts_the_epoch`
    // is what confirms this run holds the issue again, under the fresh id.
    let landed_marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &fresh_op),
        ],
    )
    .expect("the landed claim marker");
    let mut landed_comments = lapsed_comments;
    landed_comments.push(comment(
        "IC_2",
        "2026-08-24T09:00:00Z",
        format!("Claimed by {run_id}, expect to report by {horizon}.\n\n{landed_marker}\n"),
    ));
    let second_answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": issue_view(&landed_comments), "status": 0 },
        rest_listing(&landed_comments),
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the second script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let mut second_cmd = tracker_command(home, repo, bin, &second_answers);
    second_cmd.env("ESTIGIA_FAKE_LOG", &log);
    let second_out = second_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--json",
        ])
        .output()
        .expect("the retry runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&second_out.stdout),
        String::from_utf8_lossy(&second_out.stderr)
    );
    assert!(
        second_out.status.success(),
        "the retry against the landed write did not succeed: {said}"
    );
    assert!(
        said.contains("reused_existing_claim") && said.contains("true"),
        "the retry did not adopt the landed claim: {said}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "the retry against a landed write posted a second claim comment: {calls}"
    );

    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    assert_eq!(
        recorded.get("issue").and_then(|v| v.as_u64()),
        Some(12),
        "the run does not read as holding the issue again: {recorded}"
    );
    assert_eq!(
        recorded.get("operation_id").and_then(|v| v.as_str()),
        Some(fresh_op.as_str()),
        "the pointer's operation id moved again on a call that only adopted: {recorded}"
    );
}

/// A run whose own claim lapsed and was then reclaimed and released by another
/// run can take the issue again.
///
/// The exception read `holding().stale` only, and `reduce_ownership`
/// truncates at the last takeover. Once a foreign `reclaim` closed the epoch
/// this run's acquisition sat in, that acquisition appeared in neither `live`
/// nor `stale`, so the reused id met the old lockout again: a `reclaim`-origin
/// marker answered `operation-id-kind-conflict`, a `claim`-origin one
/// `claim-metadata-mismatch`, and `reclaim` answered `nothing-to-reclaim`.
/// The dead-self check now reads the whole timeline, so this id answers
/// `claim-operation-expired` and the harness reissues under a fresh id.
///
/// The foreign takeover stays recorded: the reissue posts a `claim` and edits
/// nothing, so B's `reclaim` and its `unassign` are exactly where they were.
/// Turn the widened predicate back to `stale`-only and the first process's
/// assertions go red without reaching a second attempt.
#[test]
fn a_lapsed_self_claim_behind_a_foreign_reclaim_can_be_claimed_again() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let other_run = "claude-other00000000000000000000";
    let old_op = "a".repeat(32);
    let other_op = "b".repeat(32);
    let release_op = "c".repeat(32);
    let runs = home.join(".estigia").join("runs");

    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    run.operation_id = Some(old_op.clone());
    estigia::harness::session::store(&runs, &run).expect("the fixture pointer writes");

    let claim_marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", "2019-01-01T00:00Z"),
            ("op-id", &old_op),
        ],
    )
    .expect("this run's lapsed claim marker");
    let reclaim_marker = estigia::transport::markers::render(
        "reclaim",
        &[
            ("run-id", other_run),
            ("runtime", "claude"),
            ("horizon", "2020-01-01T00:00Z"),
            ("op-id", &other_op),
            ("from", run_id),
            ("from-op", &old_op),
        ],
    )
    .expect("the foreign takeover's marker");
    let release_marker = estigia::transport::markers::render(
        "unassign",
        &[
            ("run-id", other_run),
            ("runtime", "claude"),
            ("op-id", &release_op),
            ("target-op", &other_op),
        ],
    )
    .expect("the foreign takeover's own release marker");
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    // Old enough that this run's own claim has lapsed and the foreign takeover
    // that displaced it has itself been released by the time either process
    // runs — the shape in which the issue's lockout survives.
    let lapsed_comments = vec![
        comment(
            "IC_0",
            "2019-01-01T00:00:00Z",
            format!("Claimed by {run_id}.\n\n{claim_marker}\n"),
        ),
        comment(
            "IC_1",
            "2019-06-01T00:00:00Z",
            format!("`{other_run}` reclaiming from `{run_id}`.\n\n{reclaim_marker}\n"),
        ),
        comment(
            "IC_2",
            "2019-06-02T00:00:00Z",
            format!("`{other_run}` unassigning.\n\n{release_marker}\n"),
        ),
    ];
    let issue_view = |comments: &[serde_json::Value]| {
        serde_json::json!({
            "state": "OPEN",
            "assignees": [{"login": "fixture"}],
            "labels": [{"name": "status:in-progress"}, {"name": "dev:claude"}],
            "comments": comments,
        })
        .to_string()
    };

    let horizon = "2099-06-01T00:00Z";
    let first_answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": issue_view(&lapsed_comments), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        { "matches": "label create", "stdout": "", "status": 0 },
        { "matches": "issue comment", "stdout": "", "status": 0 },
    ]))
    .expect("the first script serialises");

    let first_trace = tempfile::tempdir().expect("a trace directory");
    let first_log = first_trace.path().join("calls.log");
    let mut first_cmd = tracker_command(home, repo, bin, &first_answers);
    first_cmd.env("ESTIGIA_FAKE_LOG", &first_log);
    let first_output = first_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--json",
        ])
        .output()
        .expect("the first attempt runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&first_output.stdout),
        String::from_utf8_lossy(&first_output.stderr)
    );
    assert!(
        !said.contains("operation-id-kind-conflict"),
        "the reused id still meets the kind conflict behind a foreign reclaim: {said}"
    );
    assert!(
        !said.contains("claim-metadata-mismatch"),
        "the reused id still meets the metadata mismatch behind a foreign reclaim: {said}"
    );
    let first_calls = std::fs::read_to_string(&first_log).unwrap_or_default();
    assert_eq!(
        comment_writes(&first_calls),
        0,
        "a claim posts no comment to GitHub: {first_calls}"
    );
    assert_eq!(
        local_entries(home)
            .iter()
            .filter(|body| body.contains("issue-flow: claim"))
            .count(),
        1,
        "the reissue must record exactly one claim marker in the store"
    );
    assert!(
        !first_calls.contains("issue edit"),
        "the reissue must not edit the foreign takeover away: {first_calls}"
    );

    let pointer = runs.join(format!("{run_id}.json"));
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is there"))
            .expect("the pointer is JSON");
    let fresh_op = recorded
        .get("operation_id")
        .and_then(|v| v.as_str())
        .expect("a fresh key was minted and stored")
        .to_owned();
    assert_ne!(
        fresh_op, old_op,
        "the dead id behind the foreign reclaim was reused rather than replaced: {recorded}"
    );

    // The world in which that write landed: this run holds the issue again under
    // the fresh id, with the foreign takeover still on the timeline behind it.
    let landed_marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &fresh_op),
        ],
    )
    .expect("the landed claim marker");
    let mut landed_comments = lapsed_comments;
    landed_comments.push(comment(
        "IC_3",
        "2026-08-24T09:00:00Z",
        format!("Claimed by {run_id}, expect to report by {horizon}.\n\n{landed_marker}\n"),
    ));
    let second_answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": issue_view(&landed_comments), "status": 0 },
        rest_listing(&landed_comments),
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the second script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let mut second_cmd = tracker_command(home, repo, bin, &second_answers);
    second_cmd.env("ESTIGIA_FAKE_LOG", &log);
    let second_out = second_cmd
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--json",
        ])
        .output()
        .expect("the retry runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&second_out.stdout),
        String::from_utf8_lossy(&second_out.stderr)
    );
    assert!(
        second_out.status.success(),
        "the retry against the landed write did not succeed: {said}"
    );
    assert!(
        said.contains("reused_existing_claim") && said.contains("true"),
        "the retry did not adopt the landed claim: {said}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "the retry against a landed write posted a second claim comment: {calls}"
    );
}

/// After a reclaim that named review, the gate must admit a write.
///
/// The gate sends `pointer.state` as expect-state (`harness::decide`). The
/// reclaim write is `reclaiming_review_leaves_the_pointer_reading_review`.
/// Together they go red if reclaim stamps in-progress again: this fixture
/// uses that stamp, and a review issue then refuses unexpected-state.
#[test]
fn a_reclaim_of_review_admits_a_gated_command_afterwards() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    estigia::harness::session::store(&home.join(".estigia"), &run).expect("the pointer writes");

    let payload = serde_json::json!({
        "command": format!("{}{}{}", "gh", " ", "pr merge 12 --merge"),
    });
    let payload = serde_json::to_string(&payload).expect("payload");
    let (out, error, _) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("review"),
        &["gate", "bash", "--input", &payload, "--run-id", run_id],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !said.contains("unexpected-state"),
        "the gate refused a write after a review reclaim stamp: {said}"
    );
}

/// `create` passes the caller's title to `gh issue create` unmodified.
///
/// The identity used to be prepended to the title with no reader anywhere in
/// the crate for it — `create --identity unreadable-titles --title "..."` used
/// to file `unreadable-titles: ...`. It no longer does: the recorded argv
/// carries the exact title the caller supplied, bounded by its neighbouring
/// flags so a prepend cannot hide inside a substring match, and the identity
/// is nowhere in it. This does not read the staged body file back — the
/// transport deletes it right after `gh issue create` returns, before this
/// test's process can see it — so it does not prove what the filed body holds.
/// `create_files_a_body_with_no_marker_and_records_the_identity_and_clause_locally`
/// below reads that body back.
///
/// Turn it off by restoring `let full_title = format!("{identity}: {title}");`
/// ahead of the `gh issue create` call and this reddens on the `--title`
/// assertion below.
#[test]
fn create_passes_the_callers_title_through_unmodified() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");

    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue create",
            "stdout": "https://github.com/o/r/issues/100\n",
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "create",
            "arguments": {
                "identity": "unreadable-titles",
                "title": "Issue and PR titles are unreadable at a glance",
                "body_file": body.to_string_lossy(),
                "priority": "priority:low",
                "domain": "general",
                "runtime": "claude",
                "run_id": "claude-abcd1234",
            },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
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

    let calls = std::fs::read_to_string(&log).expect("the call log is readable");
    assert!(
        calls.contains("issue create"),
        "the fixture never reached `gh issue create`, so this proves nothing: {calls}"
    );
    assert!(
        calls.contains("--title Issue and PR titles are unreadable at a glance --body-file"),
        "the recorded argv does not carry the caller's exact title, bounded by its \
         neighbouring flags: {calls}"
    );
    assert!(
        !calls.contains("--title unreadable-titles:"),
        "the identity was prepended to the title again: {calls}"
    );

    assert_eq!(
        response["result"]["isError"], false,
        "the issue was filed and the call reported a failure: {text}"
    );
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the answer is JSON: {text}"));
    assert_eq!(
        answer["issue"], 100,
        "the answer does not name the issue that was filed: {text}"
    );
}

/// `create` files the caller's body with no `issue-flow:` marker in it and
/// records the identity and the `blocked-until` clause in the local store.
///
/// `ESTIGIA_FAKE_BODY_LOG` reads the staged `--body-file` before
/// `create`'s own `std::fs::remove_file(&staged)` runs, so this crosses the
/// window the title test above could not.
///
/// Turn it off by composing an `analysis` marker into the body again, or by
/// dropping the store's identity or clause, and this reddens.
#[test]
fn create_files_a_body_with_no_marker_and_records_the_identity_and_clause_locally() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let bodies = tempfile::tempdir().expect("a body-log directory");
    let log = trace.path().join("calls.log");
    let body_log = bodies.path().join("bodies.log");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");

    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue create",
            "stdout": "https://github.com/o/r/issues/101\n",
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "create",
            "arguments": {
                "identity": "unreadable-titles",
                "title": "Issue and PR titles are unreadable at a glance",
                "body_file": body.to_string_lossy(),
                "priority": "priority:low",
                "domain": "general",
                "runtime": "claude",
                "run_id": "claude-abcd1234",
                "state": "blocked",
                "blocked_until": "7=done",
            },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LOG", &log)
        .env("ESTIGIA_FAKE_BODY_LOG", &body_log)
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
    assert_eq!(
        response["result"]["isError"], false,
        "the issue was filed and the call reported a failure: {text}"
    );

    let filed = std::fs::read_to_string(&body_log).expect("the body log is readable");
    assert!(
        !filed.contains("issue-flow:"),
        "the filed body carries a marker the store now holds: {filed}"
    );
    assert!(
        filed.contains("A probe."),
        "the filed body lost the caller's own text: {filed}"
    );

    let connection = rusqlite::Connection::open(home.join(".estigia").join("state.db"))
        .expect("the store was created");
    let identity: Option<String> = connection
        .query_row(
            "SELECT identity FROM issues WHERE number = 101",
            [],
            |row| row.get(0),
        )
        .expect("the filed issue is in the store");
    assert_eq!(
        identity.as_deref(),
        Some("unreadable-titles"),
        "the identity was not recorded in the store"
    );
    let clauses: Vec<String> = connection
        .prepare("SELECT body FROM blocked_clauses WHERE issue = 101")
        .expect("the clauses table exists")
        .query_map([], |row| row.get(0))
        .expect("the clauses read")
        .collect::<Result<_, _>>()
        .expect("every clause reads");
    assert_eq!(
        clauses,
        ["<!-- issue-flow: blocked-until encoding=pct issue=7 state=done -->"],
        "the clause was not recorded in the store"
    );
}

/// A `gh repo view` that fails mirrors nothing, and the label still moves.
///
/// The projects are read from the issue, and reading them starts with the
/// repository's identity — so an identity read that fails is the one failure
/// mode that turns the whole projection into a skip. It must stay a skip: an
/// identity nobody could read is not clearance to write to a project, and it is
/// not a failure of the transition either. Re-asking through `?` below the label
/// edit would report *nothing was written* over an edit that landed.
///
/// So the fixture fails `gh repo view` and asserts the three things that must
/// hold: the label lands, the projection says the question could not be asked,
/// and the call does not report a failure over the write it just made.
#[test]
fn an_unreadable_repository_identity_mirrors_nothing_and_the_label_still_moves() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let labels = trace.path().join("labels.json");
    let log = trace.path().join("calls.log");
    std::fs::write(&labels, serde_json::json!(["status:ready"]).to_string())
        .expect("the issue's labels are written");

    let answers = serde_json::to_string(&serde_json::json!([
        // The whole point of the fixture: the identity read fails, so the
        // projects can never be discovered.
        { "matches": "repo view", "stdout": "", "stderr": "gh: could not read the repository", "status": 1 },
        { "matches": "issue view", "labels": true, "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "transition",
            "arguments": { "issue": 12, "to": "done", "run_id": "claude-abcd1234" },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &labels)
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

    assert_eq!(
        response["result"]["isError"], false,
        "an identity read that failed reported a failure over a label edit that landed: {text}"
    );
    let held: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a list of names");
    assert!(
        held.iter().any(|name| name == "status:done"),
        "the authoritative label was not written because the identity read failed, leaving \
         {held:?}: {text}"
    );

    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the answer is JSON: {text}"));
    let skipped = answer["board"]["skipped"].as_str().unwrap_or_default();
    assert!(
        skipped.contains("identity could not be read"),
        "the projection did not say the question could not be asked: {text}"
    );
    let calls = std::fs::read_to_string(&log).expect("the call log is readable");
    assert!(
        !calls.contains("updateProjectV2ItemFieldValue"),
        "the mirror moved a card without knowing which repository it belongs to: {calls}"
    );
}

/// `skill/SKILL.md`'s delivery step named merge, tag, renew,
/// transition to `done`, record evidence and close — and never named release,
/// so a run that followed it exactly left `~/.estigia/runs/<id>.json` naming a
/// checkout, over an issue that same run had just closed. This drives the
/// step's tail end to end — a real `claim`, a real `transition --to done`
/// over MCP, and a real `release` against the issue once it is CLOSED —
/// rather than asserting on the prose the step now carries. What it proves is
/// narrower than the step's own new sentence: that `release` succeeds once
/// asked, against an issue this same delivery already closed. It does not
/// prove a delivery asks for it — nothing adjudicates that, which is
/// `docs/honesty.md`'s to say.
///
/// The run pointer is never written by this test: `claim` and `release` are
/// driven for real and the pointer is read back, the same idiom
/// `releasing_a_phantom_pointer_forgets_it_and_says_so` uses in the other
/// direction. Two adaptations follow from one fact this suite's fixture
/// cannot pose: `mint_operation_id` is nanosecond-seeded, so a static
/// `ESTIGIA_FAKE_ANSWERS` script cannot name the marker a fresh write would
/// carry. `claim` has a real door around it — a claim that finds this run
/// already holding the issue adopts the marker's own operation id rather than
/// minting one (`may_claim`'s `Attempt::AlreadyWritten`, the same route a run
/// that lost its pointer during an outage takes) — so the tracker here
/// already carries this run's marker and `claim` adopts it. `unassign` has no
/// such door, so its retry key is pinned on the pointer before `release`
/// runs, the same slot a retried release already reuses.
#[test]
fn a_delivery_that_follows_the_sequence_puts_its_own_claim_down() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let horizon = "2099-01-01T00:00Z";
    let claim_op = "a".repeat(32);
    let release_op = "b".repeat(32);
    let login = "fixture";

    let marker = |kind: &str, fields: &[(&str, &str)]| {
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

    // The claim marker this run's own `claim` call adopts rather than writes.
    // `ownership_epoch` returns the operation id verbatim whenever one is
    // recorded, so this is also the exact epoch `release` has to name later.
    let claim_marker = marker(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", horizon),
            ("op-id", &claim_op),
        ],
    );
    // A closure and not a value: the same comment is embedded in three
    // separate issue snapshots below, and rebuilding it each time reads
    // plainer than threading `.clone()` through all but the last.
    let claim_comment = || {
        comment(
            "IC_claim",
            "2026-01-01T00:00:00Z",
            format!("Claimed by {run_id}, expect to report by {horizon}.\n\n{claim_marker}\n"),
        )
    };

    // Claim stage: an OPEN issue whose timeline already carries this run's own
    // marker, and whose projection already agrees with it — the renewal shape
    // `may_claim`'s own comment documents from a real outage, adopted rather
    // than written a second time. `claim` still runs for real: it reads the
    // tracker, adjudicates, converges the projection, and the harness writes
    // the pointer this test reads back.
    let issue_open = serde_json::json!({
        "state": "OPEN",
        "assignees": [{ "login": login }],
        "labels": [{ "name": "status:in-progress" }, { "name": "dev:claude" }],
        "comments": [claim_comment()],
    });
    let answers_claim = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": issue_open.to_string(), "status": 0 },
        { "matches": "api user", "stdout": serde_json::json!({"login": login}).to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let claim_output = tracker_command(home, repo, bin, &answers_claim)
        .args(["claim", "12", "--run-id", run_id, "--horizon", horizon])
        .output()
        .expect("estigia claim runs");
    let claim_said = format!(
        "{}{}",
        String::from_utf8_lossy(&claim_output.stdout),
        String::from_utf8_lossy(&claim_output.stderr)
    );
    assert!(
        claim_output.status.success(),
        "driving claim failed: {claim_said}"
    );

    // This is the pointer the delivery will have to put down.
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join(format!("{run_id}.json"));
    assert!(
        pointer.exists(),
        "claim did not write the pointer it is supposed to hold: {claim_said}"
    );
    let pointer_after_claim: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is readable"))
            .expect("the pointer is JSON");
    assert_eq!(
        pointer_after_claim["issue"], 12,
        "the pointer claim wrote does not name the issue it claimed: {pointer_after_claim}"
    );

    // Transition to `done`, over MCP — there is no `estigia transition` CLI
    // subcommand, transitions are MCP-only.
    let label_dir = tempfile::tempdir().expect("a directory for the label store");
    let labels = label_dir.path().join("labels.json");
    std::fs::write(
        &labels,
        serde_json::json!(["status:in-progress"]).to_string(),
    )
    .expect("the issue's labels are written");
    let answers_transition = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "labels": true, "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "transition",
            "arguments": { "issue": 12, "to": "done", "from": "in-progress", "run_id": run_id },
        },
    })
    .to_string();
    let mut child = tracker_command(home, repo, bin, &answers_transition)
        .arg("mcp")
        .env("ESTIGIA_FAKE_LABELS", &labels)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    {
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
    }
    let transition_output = child.wait_with_output().expect("the MCP server exits");
    let transition_response: serde_json::Value = serde_json::from_slice(&transition_output.stdout)
        .unwrap_or_else(|_| {
            panic!(
                "the transition response is not JSON: {}",
                String::from_utf8_lossy(&transition_output.stdout)
            )
        });
    assert_eq!(
        transition_response["result"]["isError"], false,
        "the transition to done was refused: {transition_response}"
    );

    // `isError == false` alone holds no consequence: it is also what a
    // deleted transition stage would leave unasserted. Read the label store
    // the fake `gh issue edit` actually wrote back, so the assertion fails on
    // a transition that never ran rather than on something incidental further
    // down.
    let labels_after_transition: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a JSON array");
    assert!(
        labels_after_transition
            .iter()
            .any(|label| label == "status:done"),
        "the transition did not leave the issue carrying status:done: {labels_after_transition:?}"
    );
    assert!(
        !labels_after_transition
            .iter()
            .any(|label| label == "status:in-progress"),
        "the transition left status:in-progress on the issue instead of removing it: \
         {labels_after_transition:?}"
    );

    // Pin this run's release key rather than let the CLI mint a fresh one:
    // `mint_operation_id` is nanosecond-seeded, so a static fixture cannot
    // name the marker its own write would carry unless the key is fixed
    // first — the same idempotency slot a retried release already reuses
    // (`harness::mcp`'s `run.release_id`).
    let runs_dir = home.join(".estigia").join("runs");
    // A registered checkout this release must keep: the timeline carries no
    // publication of this run's, so the close is not recognisable as its own
    // delivery and the checkout is not the release's to remove.
    let kept_checkout = home.join("unrecognised-checkout");
    checked_git(repo, &["branch", "unrecognised-work", "HEAD"]);
    checked_git(
        repo,
        &[
            "worktree",
            "add",
            "--",
            kept_checkout.to_str().expect("the fixture path is UTF-8"),
            "unrecognised-work",
        ],
    );
    estigia::harness::session::update(&runs_dir, run_id, |run| {
        run.release_id = Some(release_op.clone());
        run.operation_id = Some(claim_op.to_string());
        run.worktree = Some(kept_checkout.clone());
    });

    // Release stage: the issue is now CLOSED, and the timeline still carries
    // this run's own claim marker. `unassign` reads
    // `assignees,labels,comments`, never `state`, so a release against an
    // issue this delivery just closed proceeds exactly as it would against an
    // open one. Two schedules: before `unassign`'s write,
    // the comments carry only the claim; from the write onward, they carry
    // the release too, so the write's own visibility check and the
    // projection repair that follows it both read back what the write
    // actually did.
    let unassign_marker = marker(
        "unassign",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("op-id", &release_op),
            ("target-op", &claim_op),
        ],
    );
    let unassign_comment = comment(
        "IC_unassign",
        "2026-01-02T00:00:00Z",
        format!("`{run_id}` releasing exact epoch `{claim_op}`.\n\n{unassign_marker}\n"),
    );
    let issue_closed_before = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment()],
    })
    .to_string();
    // The fixture's own reason to exist: a release against an issue this
    // delivery already closed. Flipping this literal to OPEN left the test
    // green, so pin the state the snapshot claims to carry before it is used.
    assert!(
        issue_closed_before.contains("\"state\":\"CLOSED\""),
        "the release-stage issue snapshot does not carry CLOSED: {issue_closed_before}"
    );
    let issue_closed_after = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment(), unassign_comment],
    })
    .to_string();
    // The write's own visibility check and the projection repair both read
    // this snapshot back. Flipping this literal to OPEN left the test green,
    // exactly like its `issue_closed_before` sibling above -- pin it too.
    assert!(
        issue_closed_after.contains("\"state\":\"CLOSED\""),
        "the post-release issue snapshot does not carry CLOSED: {issue_closed_after}"
    );
    let answers_release = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "nth": 1, "stdout": issue_closed_before, "status": 0 },
        { "matches": "issue view", "nth": 2, "stdout": issue_closed_before, "status": 0 },
        { "matches": "issue view", "stdout": issue_closed_after, "status": 0 },
        { "matches": "api user", "stdout": serde_json::json!({"login": login}).to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let release_output = tracker_command(home, repo, bin, &answers_release)
        .args(["release", "--run-id", run_id, "--json"])
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .output()
        .expect("estigia release runs");
    let release_stdout = String::from_utf8_lossy(&release_output.stdout).into_owned();
    let release_said = format!(
        "{release_stdout}{}",
        String::from_utf8_lossy(&release_output.stderr)
    );
    assert!(
        release_output.status.success(),
        "release against the closed issue was refused: {release_said}"
    );

    // The release reported success — a real tracker write, not the
    // `nothing-to-unassign` forget `releasing_a_phantom_pointer_forgets_it_
    // and_says_so` exercises in the other direction.
    let released: serde_json::Value = serde_json::from_str(release_stdout.trim())
        .unwrap_or_else(|_| panic!("the release answer is JSON: {release_said}"));
    assert_eq!(
        released["released"], true,
        "release did not report success against an issue this run's delivery had just closed: \
         {release_said}"
    );
    assert!(
        released.get("forgotten").is_none(),
        "release took the phantom-pointer forget path instead of a real tracker write: \
         {release_said}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(
        comment_writes(&calls),
        0,
        "a release posts no comment to GitHub: {calls}"
    );
    assert!(
        local_entries(home)
            .iter()
            .any(|body| body.contains("issue-flow: unassign")),
        "release reported success without the unassign marker reaching the store"
    );

    // The defect this reproduces: a delivery that followed the sequence and
    // reported success while still holding its own claim. `release` puts it
    // down for real, against the issue that delivery already closed.
    assert!(
        !pointer.exists(),
        "the run pointer survived a release the tracker accepted \u{2014} the exact phantom \
         holder this guards against"
    );

    // And the checkout stays: the timeline carries no publication of this
    // run's, so nothing here proves those bytes were delivered.
    assert_eq!(
        released["checkout"]["removed"], false,
        "a close the tracker does not recognise as this run's own delivery let its checkout be \
         removed: {release_said}"
    );
    // The ledger sentence says this path "names the reason", so read the reason
    // back rather than only the kept flag: a blank or wrong `detail` on this
    // path left the suite green until this assertion existed.
    assert!(
        released["checkout"]["detail"]
            .as_str()
            .is_some_and(|detail| !detail.trim().is_empty()),
        "the kept checkout named no reason: {release_said}"
    );
    assert!(
        kept_checkout.exists(),
        "the checkout was removed although the delivery was never proven: {release_said}"
    );
}

/// `release` is the one step the delivery path runs after the close,
/// and it is where the run that delivered puts its checkout down. This drives a
/// real `release` against an issue whose close the tracker recognises as this
/// run's own delivery — the same `delivery_close` proof `verify_claim`
/// uses — and asserts the worktree and the local branch are gone, with the
/// removal named in the answer. The sibling test above is the other half: a
/// close the tracker does not recognise leaves the checkout alone.
#[test]
fn releasing_a_delivered_issue_puts_its_checkout_and_branch_down() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let issue: u64 = 21;
    let branch = "fix/21-merged-branch-cleanup";
    let claim_op = "a".repeat(32);
    let release_op = "b".repeat(32);
    let epoch = "c".repeat(32);
    let digest = "d".repeat(64);
    let login = "fixture";
    let head = rig.head.clone();
    let base = rig.head.clone();
    let merge_sha = "f".repeat(40);

    let checkout = home.join("delivered-checkout");
    checked_git(repo, &["branch", branch, "HEAD"]);
    checked_git(
        repo,
        &[
            "worktree",
            "add",
            "--",
            checkout.to_str().expect("the fixture path is UTF-8"),
            branch,
        ],
    );
    assert!(
        worktree_registered(repo, &checkout),
        "the fixture checkout is not registered before the release"
    );

    let runs_dir = home.join(".estigia").join("runs");
    estigia::harness::session::update(&runs_dir, run_id, |run| {
        run.issue = Some(estigia::identity::Identity::Number(issue));
        run.state = Some("done".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        run.worktree = Some(checkout.clone());
        run.release_id = Some(release_op.clone());
    });

    let marker = |kind: &str, fields: &[(&str, &str)]| {
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
    let claim_comment = || {
        comment(
            "IC_claim",
            "2026-01-01T00:00:00Z",
            format!(
                "Claimed by {run_id}.\n\n{}\n",
                marker(
                    "claim",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("horizon", "2099-01-01T00:00Z"),
                        ("op-id", &claim_op),
                    ],
                )
            ),
        )
    };
    let published_comment = || {
        comment(
            "IC_publication",
            "2026-01-01T01:00:00Z",
            format!(
                "published.\n\n{}\n",
                marker(
                    "published",
                    &[
                        ("run-id", run_id),
                        ("epoch", &epoch),
                        ("pr", "77"),
                        ("head", &head),
                        ("base", &base),
                        ("digest", &digest),
                    ],
                )
            ),
        )
    };
    let unassign_comment = || {
        comment(
            "IC_unassign",
            "2026-01-02T00:00:00Z",
            format!(
                "releasing.\n\n{}\n",
                marker(
                    "unassign",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("op-id", &release_op),
                        ("target-op", &claim_op),
                    ],
                )
            ),
        )
    };
    let issue_before = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment(), published_comment()],
    })
    .to_string();
    let issue_after = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment(), published_comment(), unassign_comment()],
    })
    .to_string();
    let closing = serde_json::json!({
        "data": { "repository": { "issue": {
            "closedByPullRequestsReferences": {
                "nodes": [{
                    "number": 77,
                    "state": "MERGED",
                    "headRefName": branch,
                    "baseRefName": "main",
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null },
            },
        } } },
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "nth": 1, "stdout": issue_before, "status": 0 },
        { "matches": "issue view", "nth": 2, "stdout": issue_before, "status": 0 },
        { "matches": "issue view", "stdout": issue_after, "status": 0 },
        {
            "matches": "repo view",
            "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "n" }).to_string(),
            "status": 0,
        },
        { "matches": "api graphql", "stdout": closing, "status": 0 },
        {
            "matches": "pr view 77",
            "stdout": serde_json::json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
            "status": 0,
        },
        {
            "matches": format!("api repos/o/n/commits/{merge_sha}"),
            "stdout": serde_json::json!({ "parents": [{ "sha": base }, { "sha": head }] })
                .to_string(),
            "status": 0,
        },
        {
            "matches": "api user",
            "stdout": serde_json::json!({ "login": login }).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let output = tracker_command(home, repo, bin, &answers)
        .args(["release", "--run-id", run_id, "--json"])
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .output()
        .expect("estigia release runs");
    let release_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let release_said = format!(
        "{release_stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "release against a delivered issue was refused: {release_said}"
    );
    let released: serde_json::Value = serde_json::from_str(release_stdout.trim())
        .unwrap_or_else(|_| panic!("the release answer is JSON: {release_said}"));
    assert_eq!(
        released["checkout"]["removed"], true,
        "the release did not report putting the checkout down: {release_said}"
    );
    assert_eq!(
        released["checkout"]["branch"], branch,
        "the release named another branch: {release_said}"
    );
    assert!(
        !checkout.exists(),
        "the delivered checkout survived its release: {release_said}"
    );
    assert!(
        !worktree_registered(repo, &checkout),
        "the checkout stayed registered: {}",
        checked_git_stdout(repo, &["worktree", "list", "--porcelain"])
    );
    let branches = checked_git_stdout(repo, &["branch", "--list", branch]);
    assert!(
        branches.trim().is_empty(),
        "the local branch survived its release: {branches}"
    );
    assert!(
        !home
            .join(".estigia")
            .join("runs")
            .join(format!("{run_id}.json"))
            .exists(),
        "the run pointer survived the release"
    );
}

/// A delivery deletes every remote head its run recorded starting,
/// not only the pull request's own — the branch linked at `start_branch` is not
/// always the branch published, and GitHub's `delete_branch_on_merge` removes
/// only the latter. The checkout here is on the recorded branch while the pull
/// request's head names a different one, so a deletion that read the pull
/// request would leave the recorded head behind.
#[test]
fn a_delivery_deletes_a_recorded_remote_head_that_is_not_the_pull_requests() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let issue: u64 = 777;
    let recorded = "fix/777-recorded-head";
    let pr_head = "fix/777-pull-request-head";
    let claim_op = "a".repeat(32);
    let release_op = "b".repeat(32);
    let epoch = "c".repeat(32);
    let digest = "d".repeat(64);
    let login = "fixture";
    let head = rig.head.clone();
    let base = rig.head.clone();
    let merge_sha = "f".repeat(40);

    // A remote the recorded head is pushed to.
    let remote = tempfile::tempdir().expect("a remote directory");
    let remote_path = remote.path().join("remote.git");
    let remote_text = remote_path.to_str().expect("the fixture path is UTF-8");
    assert!(
        checked_git(repo, &["init", "--quiet", "--bare", remote_text]),
        "the bare remote is made"
    );
    assert!(
        checked_git(repo, &["remote", "add", "origin", remote_text]),
        "the remote is configured"
    );

    let checkout = home.join("recorded-checkout");
    checked_git(repo, &["branch", recorded, "HEAD"]);
    checked_git(
        repo,
        &[
            "worktree",
            "add",
            "--",
            checkout.to_str().expect("the fixture path is UTF-8"),
            recorded,
        ],
    );
    assert!(
        checked_git(repo, &["push", "--quiet", "origin", recorded]),
        "the recorded head is pushed"
    );
    let pushed = checked_git_stdout(repo, &["ls-remote", "--heads", "origin", recorded]);
    assert!(
        pushed.contains(recorded),
        "the fixture did not publish the recorded head: {pushed}"
    );

    let runs_dir = home.join(".estigia").join("runs");
    estigia::harness::session::update(&runs_dir, run_id, |run| {
        run.issue = Some(estigia::identity::Identity::Number(issue));
        run.state = Some("done".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        run.worktree = Some(checkout.clone());
        run.release_id = Some(release_op.clone());
    });

    let marker = |kind: &str, fields: &[(&str, &str)]| {
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
    let claim_comment = || {
        comment(
            "IC_claim",
            "2026-01-01T00:00:00Z",
            format!(
                "Claimed by {run_id}.\n\n{}\n",
                marker(
                    "claim",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("horizon", "2099-01-01T00:00Z"),
                        ("op-id", &claim_op),
                    ],
                )
            ),
        )
    };
    let branch_comment = || {
        comment(
            "IC_branch",
            "2026-01-01T00:30:00Z",
            format!(
                "started.\n\n{}\n",
                marker("branch", &[("run-id", run_id), ("branch", recorded)],)
            ),
        )
    };
    let published_comment = || {
        comment(
            "IC_publication",
            "2026-01-01T01:00:00Z",
            format!(
                "published.\n\n{}\n",
                marker(
                    "published",
                    &[
                        ("run-id", run_id),
                        ("epoch", &epoch),
                        ("pr", "777"),
                        ("head", &head),
                        ("base", &base),
                        ("digest", &digest),
                    ],
                )
            ),
        )
    };
    let unassign_comment = || {
        comment(
            "IC_unassign",
            "2026-01-02T00:00:00Z",
            format!(
                "releasing.\n\n{}\n",
                marker(
                    "unassign",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("op-id", &release_op),
                        ("target-op", &claim_op),
                    ],
                )
            ),
        )
    };
    let issue_before = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment(), branch_comment(), published_comment()],
    })
    .to_string();
    let issue_after = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [
            claim_comment(),
            branch_comment(),
            published_comment(),
            unassign_comment(),
        ],
    })
    .to_string();
    let closing = serde_json::json!({
        "data": { "repository": { "issue": {
            "closedByPullRequestsReferences": {
                "nodes": [{
                    "number": 777,
                    "state": "MERGED",
                    "headRefName": pr_head,
                    "baseRefName": "main",
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null },
            },
        } } },
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "nth": 1, "stdout": issue_before, "status": 0 },
        { "matches": "issue view", "nth": 2, "stdout": issue_before, "status": 0 },
        { "matches": "issue view", "stdout": issue_after, "status": 0 },
        {
            "matches": "repo view",
            "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "n" }).to_string(),
            "status": 0,
        },
        { "matches": "api graphql", "stdout": closing, "status": 0 },
        {
            "matches": "pr view 777",
            "stdout": serde_json::json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
            "status": 0,
        },
        {
            "matches": format!("api repos/o/n/commits/{merge_sha}"),
            "stdout": serde_json::json!({ "parents": [{ "sha": base }, { "sha": head }] })
                .to_string(),
            "status": 0,
        },
        {
            "matches": "api user",
            "stdout": serde_json::json!({ "login": login }).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let output = tracker_command(home, repo, bin, &answers)
        .args(["release", "--run-id", run_id, "--json"])
        .env("ESTIGIA_FAKE_COUNT", &count)
        .output()
        .expect("estigia release runs");
    let release_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let release_said = format!(
        "{release_stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "release against a delivered issue was refused: {release_said}"
    );
    let released: serde_json::Value = serde_json::from_str(release_stdout.trim())
        .unwrap_or_else(|_| panic!("the release answer is JSON: {release_said}"));

    let after = checked_git_stdout(repo, &["ls-remote", "--heads", "origin"]);
    assert!(
        !after.contains(recorded),
        "the recorded remote head survived the delivery: {after}\n{release_said}"
    );
    let remote_branches = released["checkout"]["remote_branches"]
        .as_array()
        .unwrap_or_else(|| panic!("the release named no remote heads: {release_said}"));
    assert!(
        remote_branches
            .iter()
            .any(|entry| entry["branch"] == recorded
                && entry["deleted"] == serde_json::json!(true)),
        "the remote deletion was not read back: {release_said}"
    );
}

/// The operator-invoked sweep. It lists a merged head, deletes
/// nothing until `--confirm`, and removes it when confirmed.
#[test]
fn prune_lists_a_merged_head_and_confirm_deletes_it() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let remote = tempfile::tempdir().expect("a remote directory");
    let remote_path = remote.path().join("remote.git");
    let remote_text = remote_path.to_str().expect("the fixture path is UTF-8");
    assert!(
        checked_git(repo, &["init", "--quiet", "--bare", remote_text]),
        "the bare remote is made"
    );
    assert!(
        checked_git(repo, &["remote", "add", "origin", remote_text]),
        "the remote is configured"
    );
    let base = checked_git_stdout(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned();
    assert!(
        checked_git(repo, &["push", "--quiet", "-u", "origin", &base]),
        "the base is published"
    );
    assert!(
        checked_git(repo, &["branch", "issue-9-merged", "HEAD"]),
        "the issue branch exists"
    );
    assert!(
        checked_git(repo, &["push", "--quiet", "origin", "issue-9-merged"]),
        "the issue branch is published"
    );
    assert!(
        checked_git(
            repo,
            &[
                "merge",
                "--quiet",
                "--no-ff",
                "-m",
                "merge",
                "issue-9-merged"
            ],
        ),
        "the issue branch is merged"
    );
    assert!(
        checked_git(repo, &["push", "--quiet", "origin", &base]),
        "the merged base is published"
    );

    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        "[]",
        &["prune", "--json", "--base", &base],
        "",
    );
    let told = format!("{said}{err}");
    assert!(ok, "prune was refused: {told}");
    let listed: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|_| panic!("the prune answer is JSON: {told}"));
    assert!(
        listed["swept"].as_array().is_some_and(|entries| entries
            .iter()
            .any(|entry| entry["name"] == "issue-9-merged"
                && entry["delete"] == serde_json::json!(true))),
        "the merged head was not listed for deletion: {told}"
    );
    let before = checked_git_stdout(repo, &["ls-remote", "--heads", "origin"]);
    assert!(
        before.contains("issue-9-merged"),
        "the listing deleted the head: {before}"
    );

    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        "[]",
        &["prune", "--json", "--confirm", "--base", &base],
        "",
    );
    let told = format!("{said}{err}");
    assert!(ok, "prune --confirm was refused: {told}");
    let after = checked_git_stdout(repo, &["ls-remote", "--heads", "origin"]);
    assert!(
        !after.contains("issue-9-merged"),
        "the merged head survived --confirm: {after}\n{told}"
    );
}

/// `estigia changes` is read-only and lists every checkout, the main one
/// included, with the uncommitted files git reports.
#[test]
fn changes_lists_a_dirty_orphan_checkout_and_writes_nothing() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::write(repo.join("forgotten.txt"), "one\ntwo\n").expect("a forgotten file");

    let (said, err, ok) = run_with_tracker(home, repo, bin, "[]", &["changes", "--json"], "");
    let told = format!("{said}{err}");
    assert!(ok, "changes was refused: {told}");
    let listed: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|_| panic!("the changes answer is JSON: {told}"));
    let trees = listed["worktrees"].as_array().expect("a worktrees array");
    let dirty = trees
        .iter()
        .find(|tree| {
            tree["files"]
                .as_array()
                .is_some_and(|files| files.iter().any(|file| file["path"] == "forgotten.txt"))
        })
        .unwrap_or_else(|| panic!("the dirty checkout was omitted: {told}"));
    assert_eq!(dirty["orphan"], serde_json::json!(true));
    assert_eq!(dirty["files"][0]["marker"], serde_json::json!("U"));
    assert!(
        repo.join("forgotten.txt").exists(),
        "the listing removed the file"
    );

    let (text, _, ok) = run_with_tracker(home, repo, bin, "[]", &["changes"], "");
    assert!(
        ok && text.contains("forgotten.txt") && text.contains("orphan"),
        "{text}"
    );
}

/// A linked checkout a run pointer names is listed as held, one no pointer
/// names as an orphan, and both carry what they hold uncommitted.
#[test]
fn changes_tells_a_held_linked_checkout_from_an_orphan_one() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let held = home.join("held-checkout");
    let orphan = home.join("orphan-checkout");
    for (branch, path) in [("issue-30-held", &held), ("issue-31-orphan", &orphan)] {
        checked_git(repo, &["branch", branch, "HEAD"]);
        checked_git(
            repo,
            &[
                "worktree",
                "add",
                "--",
                path.to_str().expect("the fixture path is UTF-8"),
                branch,
            ],
        );
    }
    std::fs::write(held.join("held-work.txt"), "a\nb\n").expect("held work");
    std::fs::write(orphan.join("orphan-work.txt"), "a\n").expect("orphan work");
    estigia::harness::session::update(
        &home.join(".estigia").join("runs"),
        "claude-held5678",
        |run| {
            run.issue = Some(estigia::identity::Identity::Number(30));
            run.state = Some("in-progress".to_owned());
            run.repo_dir = Some(repo.to_path_buf());
            run.worktree = Some(held.clone());
        },
    );

    let (said, err, ok) = run_with_tracker(home, repo, bin, "[]", &["changes", "--json"], "");
    let told = format!("{said}{err}");
    assert!(ok, "changes was refused: {told}");
    let listed: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|_| panic!("the changes answer is JSON: {told}"));
    let holding = |file: &str| {
        listed["worktrees"]
            .as_array()
            .expect("a worktrees array")
            .iter()
            .find(|tree| {
                tree["files"]
                    .as_array()
                    .is_some_and(|files| files.iter().any(|entry| entry["path"] == file))
            })
            .unwrap_or_else(|| panic!("the checkout holding {file} was omitted: {told}"))
    };
    let held_tree = holding("held-work.txt");
    assert_eq!(held_tree["orphan"], serde_json::json!(false), "{told}");
    assert_eq!(held_tree["run"], serde_json::json!("claude-held5678"));
    assert_eq!(held_tree["issue"], serde_json::json!(30));
    assert_eq!(held_tree["branch"], serde_json::json!("issue-30-held"));
    assert_eq!(held_tree["files"][0]["marker"], serde_json::json!("U"));
    assert_eq!(held_tree["files"][0]["added"], serde_json::json!(2));
    let orphan_tree = holding("orphan-work.txt");
    assert_eq!(orphan_tree["orphan"], serde_json::json!(true), "{told}");
    assert_eq!(orphan_tree["run"], serde_json::Value::Null);
    assert_eq!(orphan_tree["branch"], serde_json::json!("issue-31-orphan"));

    let (text, _, ok) = run_with_tracker(home, repo, bin, "[]", &["changes"], "");
    assert!(ok, "{text}");
    assert!(
        text.contains("[issue-30-held]  held by claude-held5678")
            && text.contains("[issue-31-orphan]  orphan"),
        "{text}"
    );
    assert!(
        held.join("held-work.txt").exists() && orphan.join("orphan-work.txt").exists(),
        "the listing removed a file"
    );
}

/// A release whose unassign marker is already on the
/// issue — the retried release — must retire the checkout too. Before this, the
/// resumed path answered without a `checkout` key and never attempted
/// retirement, so a first release that posted its marker and then failed before
/// cleanup left a delivered checkout with no record and no report.
#[test]
fn a_retried_release_puts_its_checkout_down_too() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let issue: u64 = 22;
    let branch = "fix/22-retried-release";
    let claim_op = "a".repeat(32);
    let release_op = "b".repeat(32);
    let epoch = "c".repeat(32);
    let digest = "d".repeat(64);
    let login = "fixture";
    let head = rig.head.clone();
    let base = rig.head.clone();
    let merge_sha = "f".repeat(40);

    let checkout = home.join("retried-checkout");
    checked_git(repo, &["branch", branch, "HEAD"]);
    checked_git(
        repo,
        &[
            "worktree",
            "add",
            "--",
            checkout.to_str().expect("the fixture path is UTF-8"),
            branch,
        ],
    );
    assert!(
        worktree_registered(repo, &checkout),
        "the fixture checkout is not registered before the release"
    );

    let runs_dir = home.join(".estigia").join("runs");
    estigia::harness::session::update(&runs_dir, run_id, |run| {
        run.issue = Some(estigia::identity::Identity::Number(issue));
        run.state = Some("done".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        run.worktree = Some(checkout.clone());
        run.release_id = Some(release_op.clone());
    });

    let marker = |kind: &str, fields: &[(&str, &str)]| {
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
    let claim_comment = || {
        comment(
            "IC_claim",
            "2026-01-01T00:00:00Z",
            format!(
                "Claimed by {run_id}.\n\n{}\n",
                marker(
                    "claim",
                    &[
                        ("run-id", run_id),
                        ("runtime", "claude"),
                        ("horizon", "2099-01-01T00:00Z"),
                        ("op-id", &claim_op),
                    ],
                )
            ),
        )
    };
    let published_comment = || {
        comment(
            "IC_publication",
            "2026-01-01T01:00:00Z",
            format!(
                "published.\n\n{}\n",
                marker(
                    "published",
                    &[
                        ("run-id", run_id),
                        ("epoch", &epoch),
                        ("pr", "77"),
                        ("head", &head),
                        ("base", &base),
                        ("digest", &digest),
                    ],
                )
            ),
        )
    };
    // The retry's whole premise: this run's unassign marker is already on the
    // issue, so `unassign` answers through `resumed_release` rather than writing.
    let unassign_comment = comment(
        "IC_unassign",
        "2026-01-02T00:00:00Z",
        format!(
            "releasing.\n\n{}\n",
            marker(
                "unassign",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("op-id", &release_op),
                    ("target-op", &claim_op),
                ],
            )
        ),
    );
    let issue_before = serde_json::json!({
        "state": "CLOSED",
        "assignees": [],
        "labels": [{ "name": "status:done" }],
        "comments": [claim_comment(), published_comment(), unassign_comment],
    })
    .to_string();
    let closing = serde_json::json!({
        "data": { "repository": { "issue": {
            "closedByPullRequestsReferences": {
                "nodes": [{
                    "number": 77,
                    "state": "MERGED",
                    "headRefName": branch,
                    "baseRefName": "main",
                }],
                "pageInfo": { "hasNextPage": false, "endCursor": null },
            },
        } } },
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": issue_before, "status": 0 },
        {
            "matches": "repo view",
            "stdout": serde_json::json!({ "owner": { "login": "o" }, "name": "n" }).to_string(),
            "status": 0,
        },
        { "matches": "api graphql", "stdout": closing, "status": 0 },
        {
            "matches": "pr view 77",
            "stdout": serde_json::json!({ "mergeCommit": { "oid": merge_sha } }).to_string(),
            "status": 0,
        },
        {
            "matches": format!("api repos/o/n/commits/{merge_sha}"),
            "stdout": serde_json::json!({ "parents": [{ "sha": base }, { "sha": head }] })
                .to_string(),
            "status": 0,
        },
        {
            "matches": "api user",
            "stdout": serde_json::json!({ "login": login }).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let output = tracker_command(home, repo, bin, &answers)
        .args(["release", "--run-id", run_id, "--json"])
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .output()
        .expect("estigia release runs");
    let release_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let release_said = format!(
        "{release_stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "the retried release was refused: {release_said}"
    );
    let released: serde_json::Value = serde_json::from_str(release_stdout.trim())
        .unwrap_or_else(|_| panic!("the release answer is JSON: {release_said}"));
    assert_eq!(
        released["checkout"]["removed"], true,
        "the retried release did not put the checkout down: {release_said}"
    );
    assert_eq!(
        released["checkout"]["branch"], branch,
        "the retried release named another branch: {release_said}"
    );
    assert!(
        !checkout.exists(),
        "the delivered checkout survived its retried release: {release_said}"
    );
    let branches = checked_git_stdout(repo, &["branch", "--list", branch]);
    assert!(
        branches.trim().is_empty(),
        "the local branch survived its retried release: {branches}"
    );
    assert!(
        !home
            .join(".estigia")
            .join("runs")
            .join(format!("{run_id}.json"))
            .exists(),
        "the run pointer survived the retried release"
    );
}

/// `review_findings` over the real MCP server, driven by stdio rather than
/// called in-process — the same reason `repair_rounds` gets one: a unit test
/// alone leaves the argument-to-flag wiring uncrossed, and `with_text` is a
/// boolean argument whose flag carries no value, a shape `--epoch`/`--id`
/// do not share.
///
/// One publication carrying two findings, a `suggestion` and a `severe`. The
/// call omits `--epoch`, so it answers the latest publication, and passes
/// `with_text: true` so the boolean-flag wiring this test exists for is
/// actually crossed — `blocking` distinguishes the two, and each finding's
/// `evidence`/`impact` arrive verbatim in the answer.
#[test]
fn review_findings_answers_the_findings_on_the_latest_publication_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";

    let epoch = "1".repeat(32);
    let head = "a".repeat(40);
    let base = "d".repeat(40);
    let digest = "1".repeat(64);

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
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
        comment(
            "IC_2",
            "2026-08-14T09:10:00Z",
            marker(
                "review-finding",
                &[
                    ("run-id", "codex-b"),
                    ("reviewer", "codex-b"),
                    ("op-id", &"a".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("id", "cosmetic-1"),
                    ("class", "suggestion"),
                    ("evidence", "a naming preference on line 12"),
                    ("impact", "readability only"),
                ],
            ),
        ),
        comment(
            "IC_3",
            "2026-08-14T09:20:00Z",
            marker(
                "review-finding",
                &[
                    ("run-id", "codex-b"),
                    ("reviewer", "codex-b"),
                    ("op-id", &"b".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("id", "off-by-one-1"),
                    ("class", "severe"),
                    ("evidence", "reproduced the off-by-one against the fixture"),
                    ("impact", "wrong output on the boundary case"),
                    ("evidence-class", "deterministic"),
                    ("causal-disposition", "introduced"),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "comments": comments,
        }).to_string(),
        "status": 0,
    }]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": "review_read", "arguments": { "issue": 12, "view": "findings", "with_text": true } }
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

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_ne!(
        response["result"]["isError"], true,
        "review_findings refused over the real wire: {response}"
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let answered: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"));
    assert_eq!(answered["ok"], true, "{answered}");
    assert_eq!(answered["selected"], "latest", "{answered}");
    assert_eq!(answered["receipt"]["epoch"], epoch, "{answered}");
    assert_eq!(answered["findings_count"], 2, "{answered}");
    assert_eq!(answered["text_included"], true, "{answered}");
    let findings = answered["findings"].as_array().expect("an array");
    assert_eq!(findings.len(), 2, "{answered}");
    let severe = findings
        .iter()
        .find(|finding| finding["id"] == "off-by-one-1")
        .unwrap_or_else(|| panic!("the severe finding is in the answer: {answered}"));
    assert_eq!(severe["blocking"], true, "{answered}");
    assert_eq!(
        severe["evidence"], "reproduced the off-by-one against the fixture",
        "{answered}"
    );
    assert_eq!(
        severe["impact"], "wrong output on the boundary case",
        "{answered}"
    );
    let suggestion = findings
        .iter()
        .find(|finding| finding["id"] == "cosmetic-1")
        .unwrap_or_else(|| panic!("the suggestion is in the answer: {answered}"));
    assert_eq!(suggestion["blocking"], false, "{answered}");
    assert_eq!(
        suggestion["evidence"], "a naming preference on line 12",
        "{answered}"
    );
    assert_eq!(suggestion["impact"], "readability only", "{answered}");
}

/// `estigia recover`, through the binary against the stand-in `gh`: the timeline is read from the
/// tracker's named repository, the role is asked of the repository that hosts the pull request,
/// and the person is shown the pinned tier and focus.
fn recovered_through_the_binary(
    host_row: &str,
    repo_view: bool,
    short: bool,
) -> (String, String, String) {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let (_, stderr, ok) = run(home, &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    for (row, value) in [
        ("Tracker", "github tracker-owner/tracker-repo"),
        ("Repository host", host_row),
    ] {
        let (_, stderr, ok) = run(home, &["config", "set", row, value], "");
        assert!(ok, "{row}: {stderr}");
    }

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
    let epoch = "1".repeat(32);
    let digest = "1".repeat(64);
    let head = rig.head.clone();
    let receipt = [
        ("epoch", epoch.as_str()),
        ("pr", "7"),
        ("head", head.as_str()),
        ("base", head.as_str()),
        ("digest", digest.as_str()),
    ];
    let mut published = vec![("run-id", "claude-abcd1234")];
    published.extend_from_slice(&receipt);
    let operation = "e".repeat(32);
    let mut computed = vec![("run-id", "claude-abcd1234"), ("op-id", operation.as_str())];
    computed.extend_from_slice(&receipt);
    computed.extend_from_slice(&[
        ("level", "low"),
        ("outcome", "escalated"),
        ("open", "[]"),
        (
            "escalated",
            r#"[{"id":"f-1","reason":"no-refuter-result"}]"#,
        ),
        ("follow-ups", "[]"),
        ("missing", "[]"),
    ]);
    let comments = vec![
        comment(
            "IC_1",
            "2026-08-14T09:00:00Z",
            marker("published", &published),
        ),
        comment(
            "IC_2",
            "2026-08-14T09:10:00Z",
            marker("review-computed-verdict", &computed),
        ),
    ];
    let mut script = vec![
        serde_json::json!({
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "collaborators/maintainer-1/permission",
            "stdout": serde_json::json!({ "permission": "write", "role_name": "maintain" }).to_string(),
            "status": 0,
        }),
    ];
    if short {
        script.push(serde_json::json!({
            "matches": "api user",
            "stdout": "{\"login\":\"maintainer-1\"}",
            "status": 0,
        }));
    }
    if repo_view {
        script.push(serde_json::json!({
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"fallback-owner\"},\"name\":\"fallback-repo\"}",
            "status": 0,
        }));
    }
    let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

    let logs = tempfile::tempdir().expect("a directory for the logs");
    let (calls, environment) = (logs.path().join("calls.log"), logs.path().join("env.log"));
    let long_form = [
        "recover",
        "7",
        "--epoch",
        &epoch,
        "--authorized-by",
        "maintainer-1",
        "--reason",
        "a person owns the rest",
    ];
    let short_form = ["recover", "7", "--reason", "a person owns the rest"];
    let output = tracker_command(home, repo, bin, &answers)
        .args(if short {
            &short_form[..]
        } else {
            &long_form[..]
        })
        .env("ESTIGIA_FAKE_LOG", &calls)
        .env("ESTIGIA_FAKE_ENV_LOG", &environment)
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "recover failed: {stdout}{stderr}");
    (
        stdout.into_owned(),
        std::fs::read_to_string(&calls).unwrap_or_default(),
        std::fs::read_to_string(&environment).unwrap_or_default(),
    )
}

#[test]
fn recover_binds_the_tracker_and_the_host_repositories_and_prints_the_pinned_line() {
    let (stdout, calls, environment) =
        recovered_through_the_binary("github host-owner/host-repo", false, false);
    assert!(stdout.contains("pinned: tier"), "{stdout}");
    assert!(
        calls.contains("repos/host-owner/host-repo/collaborators/maintainer-1/permission"),
        "the role was not asked of the host repository: {calls}"
    );
    assert!(
        environment.contains("GH_REPO=tracker-owner/tracker-repo"),
        "the timeline was not read from the tracker's repository: {environment}"
    );
}

#[test]
fn recover_needs_only_the_issue_and_a_reason() {
    let (stdout, calls, _) =
        recovered_through_the_binary("github host-owner/host-repo", false, true);
    assert!(stdout.contains("recovered: #7 epoch"), "{stdout}");
    assert!(stdout.contains("authorised by maintainer-1"), "{stdout}");
    assert!(
        calls.contains("api user"),
        "the login was not asked of gh: {calls}"
    );
    assert!(
        calls.contains("repos/host-owner/host-repo/collaborators/maintainer-1/permission"),
        "the resolved login was not role-checked: {calls}"
    );
}

#[test]
fn recover_asks_the_checkouts_own_repository_when_the_host_row_names_none() {
    let (stdout, calls, _) = recovered_through_the_binary("github", true, false);
    assert!(stdout.contains("pinned: tier"), "{stdout}");
    assert!(
        calls.contains("repos/fallback-owner/fallback-repo/collaborators/maintainer-1/permission"),
        "the role was not asked of the checkout's own repository: {calls}"
    );
}

#[test]
fn inspect_candidate_reads_a_correction_between_two_publications_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let (base, head) = commit_a_changed_line(repo, "kept.txt");
    let parent_epoch = "1".repeat(32);
    let epoch = "2".repeat(32);
    let parent_digest = "1".repeat(64);
    let digest = "2".repeat(64);

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
                    ("run-id", "claude-abcd1234"),
                    ("epoch", &parent_epoch),
                    ("pr", "7"),
                    ("head", &base),
                    ("base", &rig.head),
                    ("digest", &parent_digest),
                ],
            ),
        ),
        comment(
            "IC_2",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", "claude-abcd1234"),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("parent", &parent_epoch),
                    ("parent-pr", "7"),
                    ("parent-head", &base),
                    ("parent-base", &rig.head),
                    ("parent-digest", &parent_digest),
                    ("delta", "delta-1"),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "comments": comments,
        }).to_string(),
        "status": 0,
    }]))
    .expect("the fake tracker script serialises");

    let ask = |arguments: serde_json::Value| -> serde_json::Value {
        let mut arguments = arguments;
        arguments["view"] = serde_json::json!("inspect");
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "review_read", "arguments": arguments }
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
                    "the MCP response is not JSON: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        response["result"].clone()
    };
    let text_of = |result: &serde_json::Value| -> serde_json::Value {
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
    };

    let listed = ask(serde_json::json!({
        "issue": 12, "epoch": epoch, "request": "name-status"
    }));
    assert_ne!(
        listed["isError"], true,
        "refused over the real wire: {listed}"
    );
    let listed = text_of(&listed);
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["epoch"], epoch, "{listed}");
    let names = listed["text"].as_str().expect("text");
    assert!(
        names.contains("kept.txt") && names.starts_with('M'),
        "{listed}"
    );

    let before = ask(serde_json::json!({
        "issue": 12, "epoch": epoch, "request": "object", "path": "kept.txt", "side": "parent"
    }));
    assert_ne!(
        before["isError"], true,
        "refused over the real wire: {before}"
    );
    assert_eq!(text_of(&before)["text"], "before\n");

    let refused = ask(serde_json::json!({
        "issue": 12, "epoch": parent_epoch, "request": "name-status"
    }));
    assert_eq!(refused["isError"], true, "{refused}");
    let said = refused["content"][0]["text"].as_str().unwrap_or_default();
    assert!(said.contains("inspect-parent-missing"), "{refused}");
}

#[cfg(unix)]
fn collect_when_done(
    ask: &dyn Fn(&str, serde_json::Value) -> serde_json::Value,
    arguments: serde_json::Value,
) -> serde_json::Value {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut result = ask("collect_review_input", arguments.clone());
    while result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("capture-running")
        && std::time::Instant::now() < until
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
        result = ask("collect_review_input", arguments.clone());
    }
    result
}

#[cfg(unix)]
#[test]
fn launch_and_collect_review_input_run_the_offered_lens_and_record_it_over_the_real_mcp_server() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    let (base, head) = commit_a_changed_line(repo, "src/foo.rs");
    checked_git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/fixture/repo.git",
        ],
    );
    let run_id = "claude-abcd1234";
    let epoch = "2".repeat(32);
    let digest = "2".repeat(64);
    let payload = estigia::transport::target::review_payload(repo, &base, &head)
        .expect("the change has a payload");

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
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("payload-digest", &payload.digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "pr view",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": head,
                "baseRefOid": base,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "body": "The change keeps its one line.",
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "check-runs",
            "stdout": serde_json::json!({"total_count": 0, "check_runs": []}).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let reviewer = tempfile::tempdir().expect("a directory for the stand-in reviewer");
    let said = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": serde_json::json!({"findings": [{
            "id": "line-replaced",
            "class": "warning",
            "evidence": "src/foo.rs:1 replaced `before` with `after`",
            "impact": "a reader of the line sees a different word",
            "location": "src/foo.rs:1",
        }]}).to_string(),
    })
    .to_string();
    std::fs::write(reviewer.path().join("said.json"), said).expect("the canned answer");
    let launched = reviewer.path().join("prompt.txt");
    let release = reviewer.path().join("release");
    let script = format!(
        "#!/bin/sh\nn=0\nwhile [ ! -e '{}' ] && [ $n -lt 300 ]; do /bin/sleep 0.1; n=$((n+1)); done\n/bin/cat > '{}'\n/bin/cat '{}'\n",
        release.display(),
        launched.display(),
        reviewer.path().join("said.json").display()
    );
    let program = bin.join("claude");
    std::fs::write(&program, script).expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let ask = |tool: &str, arguments: serde_json::Value| -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
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
                    "the MCP response is not JSON: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        response["result"].clone()
    };
    let text_of = |result: &serde_json::Value| -> serde_json::Value {
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
    };

    let imported = ask(
        "verify_claim",
        serde_json::json!({"issue": 12, "run_id": run_id, "expect_state": "review"}),
    );
    assert_ne!(imported["isError"], true, "{imported}");

    let before = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(before["next"]["kind"], "collect", "{before}");
    let name = before["next"]["inputs"][0]["name"]
        .as_str()
        .expect("a lens is offered")
        .to_owned();
    assert_eq!(
        before["next"]["inputs"][0]["capture"], "launch_review_input",
        "{before}"
    );

    let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
    let mut stale = estigia::harness::session::Run::new(run_id.to_owned());
    stale.issue = Some(estigia::identity::Identity::Number(12));
    stale.state = Some("review".to_owned());
    stale.repo_dir = Some(repo.to_path_buf());
    assert!(estigia::harness::session::store(&runs, &stale).expect("the pointer is writable"));
    let verified_at = || -> serde_json::Value {
        let pointer = std::fs::read_to_string(runs.join(format!("{run_id}.json")))
            .expect("the run pointer is on disk");
        serde_json::from_str::<serde_json::Value>(&pointer).expect("a JSON pointer")["verified_at"]
            .clone()
    };
    assert!(verified_at().is_null(), "the fixture pointer was verified");

    let started = ask(
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert!(
        !launched.exists(),
        "a launch waited for the reviewer, which was still held back: {started}"
    );
    assert!(
        verified_at().is_number(),
        "a launch that started the review's reviewer did not renew the run's pointer: {started}"
    );
    assert_ne!(
        started["isError"], true,
        "refused over the real wire: {started}"
    );
    let started = text_of(&started);
    assert!(
        started["job"].is_string() && started["deadline_at"].is_number(),
        "{started}"
    );
    let while_running = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(while_running["next"]["kind"], "collect", "{while_running}");
    assert_eq!(
        while_running["next"]["inputs"][0],
        serde_json::json!({
            "name": name,
            "capture": null,
            "running": {"job": started["job"], "deadline_at": started["deadline_at"]},
        }),
        "{while_running}"
    );
    assert_eq!(
        while_running["next"]["ask_after"], started["deadline_at"],
        "{while_running}"
    );
    let early = ask(
        "collect_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert_eq!(early["isError"], true, "{early}");
    let said = early["content"][0]["text"].as_str().unwrap_or_default();
    assert!(said.contains("capture-running"), "{early}");
    std::fs::write(&release, "").expect("the reviewer is released");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut offered = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    while offered["next"]["inputs"][0]["capture"] != "collect_review_input"
        && std::time::Instant::now() < until
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
        offered = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    }
    assert_eq!(
        offered["next"]["inputs"][0],
        serde_json::json!({
            "name": name,
            "capture": "collect_review_input",
            "job": started["job"],
        }),
        "{offered}"
    );
    assert!(offered["next"].get("ask_after").is_none(), "{offered}");
    let captured = ask(
        "collect_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert_ne!(
        captured["isError"], true,
        "refused over the real wire: {captured}"
    );
    let captured = text_of(&captured);
    assert_eq!(captured["ok"], true, "{captured}");
    assert_eq!(captured["findings_recorded"], 1, "{captured}");
    let prompt = std::fs::read_to_string(&launched).expect("the reviewer was handed a prompt");
    assert!(
        prompt.contains("src/foo.rs") && prompt.contains("The change keeps its one line."),
        "the reviewer was not handed the diff and the criteria"
    );

    let after = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    let still_offered = after["next"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|input| input["name"] == name.as_str());
    assert!(
        !still_offered,
        "the lens was captured and is still offered: {after}"
    );
    assert_eq!(
        after["next"]["reason_code"], "verdict-computable",
        "{after}"
    );

    let again = ask(
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert_eq!(again["isError"], true, "{again}");
    let said = again["content"][0]["text"].as_str().unwrap_or_default();
    assert!(said.contains("capture-input-not-offered"), "{again}");
}

/// A `high` review licenses four lenses, and the batched `names`
/// argument launches every one of them in a single call and collects every one
/// of them in the next, rather than one call per slot. The review still reaches
/// `approved` end to end, and the call count drops from the eight a per-slot
/// path needs to two.
#[cfg(unix)]
#[test]
fn a_high_tier_review_launches_and_collects_every_offered_slot_in_batched_calls() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    // A hot-path token in the path is what reads the change as `high` and
    // licenses all four lenses, rather than the one a `medium` change gets.
    let (base, head) = commit_a_changed_line(repo, "src/auth.rs");
    checked_git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/fixture/repo.git",
        ],
    );
    let run_id = "claude-abcd1234";
    let epoch = "2".repeat(32);
    let digest = "2".repeat(64);
    let payload = estigia::transport::target::review_payload(repo, &base, &head)
        .expect("the change has a payload");

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
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("payload-digest", &payload.digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "pr view",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": head,
                "baseRefOid": base,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "body": "The change keeps its one line.",
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "check-runs",
            "stdout": serde_json::json!({"total_count": 0, "check_runs": []}).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let reviewer = tempfile::tempdir().expect("a directory for the stand-in reviewer");
    let said = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": serde_json::json!({"findings": []}).to_string(),
    })
    .to_string();
    std::fs::write(reviewer.path().join("said.json"), said).expect("the canned answer");
    let script = format!(
        "#!/bin/sh\n/bin/cat > /dev/null\n/bin/cat '{}'\n",
        reviewer.path().join("said.json").display()
    );
    let program = bin.join("claude");
    std::fs::write(&program, script).expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let ask = |tool: &str, arguments: serde_json::Value| -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
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
                    "the MCP response is not JSON: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        response["result"].clone()
    };
    let text_of = |result: &serde_json::Value| -> serde_json::Value {
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
    };

    let imported = ask(
        "verify_claim",
        serde_json::json!({"issue": 12, "run_id": run_id, "expect_state": "review"}),
    );
    assert_ne!(imported["isError"], true, "{imported}");

    let before = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(before["next"]["kind"], "collect", "{before}");
    let names: Vec<String> = before["next"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|input| {
            assert_eq!(input["capture"], "launch_review_input", "{before}");
            input["name"]
                .as_str()
                .expect("a lens is offered")
                .to_owned()
        })
        .collect();
    assert_eq!(
        names.len(),
        4,
        "a high tier did not offer its four lenses: {before}"
    );

    let mut advancing = 0;
    advancing += 1;
    let launched = ask(
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "names": names}),
    );
    assert_ne!(launched["isError"], true, "{launched}");
    let launched = text_of(&launched);
    assert_eq!(launched["ok"], true, "{launched}");
    assert_eq!(
        launched["launched"].as_array().map(Vec::len),
        Some(4),
        "the batched launch did not start all four lenses: {launched}"
    );

    advancing += 1;
    let collected = ask(
        "collect_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "names": names, "wait_seconds": 30}),
    );
    assert_ne!(collected["isError"], true, "{collected}");
    let collected = text_of(&collected);
    assert_eq!(
        collected["collected"].as_array().map(Vec::len),
        Some(4),
        "the batched collect did not record all four lenses: {collected}"
    );

    let per_slot = 2 * names.len();
    assert_eq!(
        advancing, 2,
        "the batched path took {advancing} calls, not one launch and one collect"
    );
    assert!(
        advancing < per_slot,
        "the batched path took {advancing} calls, no fewer than the {per_slot} a per-slot path needs"
    );

    let after = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(
        after["next"]["reason_code"], "verdict-computable",
        "{after}"
    );
    let bound = after["receipt"].clone();
    let recorded = ask(
        "record_computed_verdict",
        serde_json::json!({
            "issue": 12,
            "run_id": run_id,
            "epoch": bound["epoch"],
            "pr": bound["pr"],
            "head": bound["head"],
            "base": bound["base"],
            "digest": bound["digest"],
        }),
    );
    assert_ne!(recorded["isError"], true, "{recorded}");
    let recorded = text_of(&recorded);
    assert_eq!(recorded["outcome"], "approved", "{recorded}");
}

#[cfg(unix)]
#[test]
fn a_quota_failure_on_a_lens_records_nothing_and_leaves_release_ci_refused() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    let (base, head) = commit_a_changed_line(repo, "src/foo.rs");
    checked_git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/fixture/repo.git",
        ],
    );
    let run_id = "claude-abcd1234";
    let epoch = "2".repeat(32);
    let digest = "2".repeat(64);
    let payload = estigia::transport::target::review_payload(repo, &base, &head)
        .expect("the change has a payload");

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
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("payload-digest", &payload.digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "pr view",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": head,
                "baseRefOid": base,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "body": "The change keeps its one line.",
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        {
            "matches": "check-runs",
            "stdout": serde_json::json!({"total_count": 0, "check_runs": []}).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let program = bin.join("claude");
    std::fs::write(
        &program,
        "#!/bin/sh\n/bin/cat > /dev/null\necho 'Credit balance is too low' >&2\nexit 1\n",
    )
    .expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");

    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("calls.log");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let ask = |tool: &str, arguments: serde_json::Value| -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
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
                    "the MCP response is not JSON: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        response["result"].clone()
    };
    let text_of = |result: &serde_json::Value| -> serde_json::Value {
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
    };
    let refusal_of = |result: &serde_json::Value| -> String {
        assert_eq!(result["isError"], true, "{result}");
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };

    let imported = ask(
        "verify_claim",
        serde_json::json!({"issue": 12, "run_id": run_id, "expect_state": "review"}),
    );
    assert_ne!(imported["isError"], true, "{imported}");

    let before = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(before["next"]["kind"], "collect", "{before}");
    let name = before["next"]["inputs"][0]["name"]
        .as_str()
        .expect("a lens is offered")
        .to_owned();

    let started = ask(
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert_ne!(started["isError"], true, "{started}");
    let captured = collect_when_done(
        &ask,
        serde_json::json!({"issue": 12, "run_id": run_id, "name": name}),
    );
    assert!(
        refusal_of(&captured).contains("reviewer-unavailable"),
        "{captured}"
    );

    let after = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    let still_offered = after["next"]["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|input| input["name"] == name.as_str());
    assert!(
        still_offered,
        "a lens that never answered is no longer offered: {after}"
    );
    assert_ne!(after["next"]["operation"], "release_ci", "{after}");

    let receipt = serde_json::json!({
        "issue": 12,
        "run_id": run_id,
        "epoch": epoch,
        "pr": 7,
        "head": head,
        "base": base,
        "digest": digest,
    });
    let with = |extra: serde_json::Value| -> serde_json::Value {
        let mut arguments = receipt.clone();
        for (key, value) in extra.as_object().expect("an object") {
            arguments[key] = value.clone();
        }
        arguments
    };

    let computed = ask(
        "record_computed_verdict",
        with(serde_json::json!({"worktree": repo.display().to_string()})),
    );
    let said = refusal_of(&computed);
    assert!(
        !said.contains("\"approved\"") && said.contains("computed-verdict-incomplete"),
        "{computed}"
    );

    let released = ask(
        "release_ci",
        with(serde_json::json!({"worktree": repo.display().to_string()})),
    );
    assert!(
        refusal_of(&released).contains("qualifying-review-verdict-missing"),
        "{released}"
    );

    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("pr ready") && !calls.contains("issue comment"),
        "a quota failure wrote to the tracker or released the pull request: {calls}"
    );
}

#[cfg(unix)]
#[test]
fn launch_and_collect_review_input_run_the_offered_refuter_and_record_the_batch_over_the_real_mcp_server()
 {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    let (base, head) = commit_a_changed_line(repo, "src/foo.rs");
    checked_git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/fixture/repo.git",
        ],
    );
    let run_id = "claude-abcd1234";
    let epoch = "2".repeat(32);
    let digest = "2".repeat(64);
    let payload = estigia::transport::target::review_payload(repo, &base, &head)
        .expect("the change has a payload");

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
    let receipt = [
        ("epoch", epoch.as_str()),
        ("pr", "7"),
        ("head", head.as_str()),
        ("base", base.as_str()),
        ("digest", digest.as_str()),
    ];
    let with_receipt = |own: &[(&'static str, &str)]| -> Vec<(&'static str, String)> {
        own.iter()
            .map(|(key, value)| (*key, (*value).to_owned()))
            .chain(
                receipt
                    .iter()
                    .map(|(key, value)| (*key, (*value).to_owned())),
            )
            .collect()
    };
    let rendered = |kind, own: &[(&'static str, &str)]| {
        let owned = with_receipt(own);
        let borrowed: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
        marker(kind, &borrowed)
    };
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            rendered(
                "published",
                &[("run-id", run_id), ("payload-digest", &payload.digest)],
            ),
        ),
        comment(
            "IC_report",
            "2026-08-14T09:11:00Z",
            rendered(
                "review-lens-report",
                &[
                    ("run-id", run_id),
                    ("op-id", &"3".repeat(32)),
                    ("role", "review-reliability"),
                ],
            ),
        ),
        comment(
            "IC_finding",
            "2026-08-14T09:12:00Z",
            rendered(
                "review-finding",
                &[
                    ("run-id", run_id),
                    ("op-id", &"4".repeat(32)),
                    ("reviewer", "review-reliability"),
                    ("id", "line-replaced"),
                    ("class", "severe"),
                    ("evidence", "src/foo.rs:1 replaced `before` with `after`"),
                    ("impact", "a reader of the line sees a different word"),
                    ("evidence-class", "inferential"),
                    ("causal-disposition", "introduced"),
                    ("location", "src/foo.rs:1"),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "pr view",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": head,
                "baseRefOid": base,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "body": "The change keeps its one line.",
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "check-runs",
            "stdout": serde_json::json!({"total_count": 0, "check_runs": []}).to_string(),
            "status": 0,
        },
    ]))
    .expect("the fake tracker script serialises");

    let reviewer = tempfile::tempdir().expect("a directory for the stand-in reviewer");
    let said = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": serde_json::json!({"outcomes": [
            {"id": "line-replaced", "outcome": "corroborated"},
        ]}).to_string(),
    })
    .to_string();
    std::fs::write(reviewer.path().join("said.json"), said).expect("the canned answer");
    let launched = reviewer.path().join("prompt.txt");
    let script = format!(
        "#!/bin/sh\n/bin/cat > '{}'\n/bin/cat '{}'\n",
        launched.display(),
        reviewer.path().join("said.json").display()
    );
    let program = bin.join("claude");
    std::fs::write(&program, script).expect("the stand-in claude");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let ask = |tool: &str, arguments: serde_json::Value| -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
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
                    "the MCP response is not JSON: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        response["result"].clone()
    };
    let text_of = |result: &serde_json::Value| -> serde_json::Value {
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"))
    };

    let imported = ask(
        "verify_claim",
        serde_json::json!({"issue": 12, "run_id": run_id, "expect_state": "review"}),
    );
    assert_ne!(imported["isError"], true, "{imported}");

    let before = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_eq!(
        before["next"]["reason_code"], "refutation-missing",
        "{before}"
    );
    assert_eq!(
        before["next"]["inputs"][0]["capture"], "launch_review_input",
        "{before}"
    );
    assert_eq!(before["next"]["inputs"][0]["name"], "refutation-batch");

    let started = ask(
        "launch_review_input",
        serde_json::json!({"issue": 12, "run_id": run_id, "name": "refutation-batch"}),
    );
    assert_ne!(started["isError"], true, "{started}");
    let captured = collect_when_done(
        &ask,
        serde_json::json!({"issue": 12, "run_id": run_id, "name": "refutation-batch"}),
    );
    assert_ne!(
        captured["isError"], true,
        "refused over the real wire: {captured}"
    );
    let captured = text_of(&captured);
    assert_eq!(captured["ok"], true, "{captured}");
    assert_eq!(captured["outcomes_recorded"], 1, "{captured}");
    let prompt = std::fs::read_to_string(&launched).expect("the refuter was handed a prompt");
    assert!(
        prompt.contains("line-replaced") && prompt.contains("src/foo.rs"),
        "the refuter was not handed the candidate and the diff"
    );

    let after = text_of(&ask("review_status", serde_json::json!({"issue": 12})));
    assert_ne!(
        after["next"]["reason_code"], "refutation-missing",
        "{after}"
    );
    assert_eq!(
        after["next"]["reason_code"], "verdict-computable",
        "{after}"
    );
}

/// A `claim` can move the issue to a state in the same call: `transition_to`
/// runs the claim and then the `transition` the developer step always pairs
/// with it, against the real binary and the stateful stand-in `gh`.
///
/// The labels the stand-in actually wrote back are read, not the answer, so a
/// merged call that reported success without moving the issue fails here — the
/// same discipline
/// [`a_delivery_that_follows_the_sequence_puts_its_own_claim_down`] uses for
/// the separate transition. The pointer's own state is read back too: the merge
/// has to record the state it moved to, not the claim's default.
#[test]
fn a_claim_moves_the_issue_to_the_state_it_names() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run_id = "claude-abcd1234";
    let horizon = "2099-01-01T00:00Z";
    let claim_op = "a".repeat(32);
    let login = "fixture";

    let marker = |kind: &str, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    // A claim marker this run's own `claim` adopts rather than writes, the same
    // renewal shape the sibling test poses, so the fixture can name the exact
    // epoch the release later has to put down.
    let claim_comment = serde_json::json!({
        "id": "IC_claim",
        "createdAt": "2026-01-01T00:00:00Z",
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": format!(
            "Claimed by {run_id}, expect to report by {horizon}.\n\n{}\n",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", horizon),
                    ("op-id", &claim_op),
                ],
            ),
        ),
    });
    let issue_open = serde_json::json!({
        "state": "OPEN",
        "assignees": [{ "login": login }],
        "labels": [{ "name": "status:in-progress" }, { "name": "dev:claude" }],
        "comments": [claim_comment],
    });

    let label_dir = tempfile::tempdir().expect("a directory for the label store");
    let labels = label_dir.path().join("labels.json");
    std::fs::write(
        &labels,
        serde_json::json!(["status:in-progress"]).to_string(),
    )
    .expect("the issue's labels are written");

    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    // The claim reads the issue once; the transition then reads the labels,
    // edits, and reads them back. `nth` tells the claim's full read from the
    // transition's label-only ones; `labels: true` answers from the store so
    // the edit this same call made is what comes back.
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "body,labels,comments", "stdout": issue_open.to_string(), "status": 0 },
        { "matches": "assignees,labels", "stdout": issue_open.to_string(), "status": 0 },
        { "matches": "--json labels", "labels": true, "status": 0 },
        { "matches": "api user", "stdout": serde_json::json!({"login": login}).to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let output = tracker_command(home, repo, bin, &answers)
        .args([
            "claim",
            "12",
            "--run-id",
            run_id,
            "--horizon",
            horizon,
            "--transition-to",
            "done",
            "--json",
        ])
        .env("ESTIGIA_FAKE_LABELS", &labels)
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .output()
        .expect("estigia claim runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "the merged claim failed: {said}\ngh calls:\n{}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );

    let labels_after: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(&labels).expect("the label store is readable"),
    )
    .expect("the label store is a JSON array");
    assert!(
        labels_after.iter().any(|label| label == "status:done"),
        "the merged claim did not move the issue to done: {labels_after:?}"
    );
    assert!(
        !labels_after
            .iter()
            .any(|label| label == "status:in-progress"),
        "the merged claim left the old state label on the issue: {labels_after:?}"
    );

    let pointer = home
        .join(".estigia")
        .join("runs")
        .join(format!("{run_id}.json"));
    let pointer: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pointer).expect("the pointer is readable"))
            .expect("the pointer is JSON");
    assert_eq!(
        pointer["state"], "done",
        "the merged claim recorded a state other than the one it moved to: {pointer}"
    );
}

/// `deliver` is a tool the real server lists, takes through `run_tool`, and hands back from at the
/// receipt after its first step, the claim check, passed over the wire.
#[cfg(unix)]
#[test]
fn deliver_hands_back_at_the_receipt_over_the_real_mcp_server() {
    use std::io::Write;

    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    std::fs::create_dir_all(repo.join("src")).expect("a source directory");
    commit_a_changed_line(repo, "src/foo.rs");
    let run_id = "claude-abcd1234";
    let claim = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &"a".repeat(32)),
        ],
    )
    .expect("a protocol marker");
    let answers = serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "body": "The change keeps its one line.",
            "comments": [{
                "id": "IC_claim",
                "createdAt": "2026-08-14T09:00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": claim,
            }],
        }).to_string(),
        "status": 0,
    }]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(estigia::harness::session::store(&runs, &run).expect("the pointer is writable"));

    let respond = |tool: &str, arguments: serde_json::Value| -> serde_json::Value {
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments }
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        serde_json::from_slice(&output.stdout).expect("the MCP response is JSON")
    };

    let unknown = respond("never_called", serde_json::json!({}));
    assert!(
        unknown["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("deliver"),
        "the server does not name `deliver` among its tools: {unknown}"
    );

    let handed_back = respond(
        "deliver",
        serde_json::json!({"issue": 12, "run_id": run_id, "wait_seconds": 1}),
    );
    assert_ne!(handed_back["result"]["isError"], true, "{handed_back}");
    let text = handed_back["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let answer: serde_json::Value = serde_json::from_str(text).expect("the answer is JSON");
    assert_eq!(answer["delivered"], false, "{answer}");
    assert_eq!(answer["stopped"]["step"], "receipt", "{answer}");
    assert_eq!(answer["stopped"]["reason"], "receipt-missing", "{answer}");

    let too_long = respond(
        "deliver",
        serde_json::json!({"issue": 12, "run_id": run_id, "wait_seconds": 901}),
    );
    assert!(
        too_long["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("wait_seconds"),
        "{too_long}"
    );
}
