use super::*;

/// `review_payload_membership` over the real MCP server. The test above
/// scripts a tracker and no repository; this needs both, since the operation
/// rebuilds the payload from the checkout the call runs against rather than
/// reading stored bytes.
///
/// The expected payload and its digest are computed here with a bare `git
/// diff` under the same pinned flags `pinned_diff_args` uses — never read
/// from the function under test — the same discipline
/// `publish_review_carries_the_payload_only_when_asked_and_never_truncates_it`
/// states at its own doc comment, so a defect in `review_payload` itself
/// could not hide behind a self-comparison.
#[test]
fn review_payload_membership_crosses_a_real_candidate_over_the_real_mcp_server() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
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

    std::fs::write(repo.join("kept.txt"), "one\ntwo\nthree\n").expect("the base file");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "add kept.txt",
    ]);
    let base = sha("HEAD");

    std::fs::write(repo.join("kept.txt"), "one\nTWO\nthree\n").expect("the change");
    git(&["add", "kept.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "change kept.txt",
    ]);
    let head = sha("HEAD");

    // The independent measurement: the exact bytes `review_payload` derives,
    // computed here rather than trusted from the answer.
    let expected_payload = String::from_utf8(fenced_ambient_diff(repo, &base, &head))
        .expect("the diff is UTF-8 in this fixture");
    assert!(
        !expected_payload.is_empty(),
        "the fixture change produced no diff to measure against"
    );
    let expected_digest = estigia::transport::ownership::sha256_hex(expected_payload.as_bytes());

    let trace = tempfile::tempdir().expect("a directory for the candidate file");
    let candidate = trace.path().join("candidate.diff");
    std::fs::write(&candidate, &expected_payload).expect("the candidate file");

    let run_id = "claude-abcd1234";
    let epoch = "1".repeat(32);
    let marker = estigia::transport::markers::render(
        "published",
        &[
            ("run-id", run_id),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &"1".repeat(64)),
            ("payload-digest", &expected_digest),
        ],
    )
    .expect("a published marker");
    let answers = serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": "status:review"}],
            "comments": [{
                "id": "IC_1",
                "createdAt": "2026-08-14T09:00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": marker,
            }],
        }).to_string(),
        "status": 0,
    }]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "review_read",
            "arguments": {
                "issue": 12,
                "view": "membership",
                "candidate": candidate.display().to_string(),
                "worktree": repo.display().to_string(),
            }
        }
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
        "review_read (membership) refused over the real wire: {response}"
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    let answered: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|_| panic!("the tool answer is JSON: {text}"));
    assert_eq!(answered["ok"], true, "{answered}");
    assert_eq!(answered["payload_digest"], expected_digest, "{answered}");
    assert_eq!(answered["omitted"], 0, "{answered}");
}

#[test]
fn a_repair_past_the_frozen_correction_budget_is_refused() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000001";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    for (label, correction_lines, expect_refusal) in [
        ("within-budget", 1u64, false),
        ("exceeds-budget", 10u64, true),
    ] {
        record_cause_over_latest_receipt(home, 20);
        let branch = format!("fix/20-budget-{label}");
        g(&["checkout", "--quiet", "-b", &branch, "main"]);
        // The original candidate: four added lines. `floor_two(4) == 2`.
        std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
        g(&["add", "feature.txt"]);
        commit("the original candidate");
        let parent_head = sha("HEAD");

        let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
        for n in 0..correction_lines {
            feature.push_str(&format!("extra{n}\n"));
        }
        std::fs::write(repo.join("feature.txt"), feature).expect("the repair");
        g(&["add", "feature.txt"]);
        commit("the repair");
        let head = sha("HEAD");
        g(&["push", "--quiet", "-u", "origin", &branch]);

        let parent_epoch = "7".repeat(32);
        let parent_digest = "8".repeat(64);
        let claimed = serde_json::json!({
            "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true, "includesCreatedEdit": false,
            "body": format!(
                "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
                 horizon=2099-01-01T00:00Z op-id={} -->\n",
                "a".repeat(32)
            ),
        });
        let earlier = serde_json::json!({
            "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
            "viewerDidAuthor": true, "includesCreatedEdit": false,
            "body": format!(
                "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
                 pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
                 epoch={parent_epoch} -->\n"
            ),
        });
        let earlier_cause = republish_cause(
            run_id,
            &parent_head,
            &base_sha,
            &parent_digest,
            &parent_epoch,
        );
        let timeline = serde_json::json!({
            "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
            "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
        });
        let pr = serde_json::json!({
            "number": 7, "url": "https://github.com/o/r/pull/7",
            "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
        });

        let (failed, text, _temp) = correction_budget_publish(
            home,
            repo,
            bin,
            trace.path(),
            run_id,
            20,
            &branch,
            &timeline,
            &pr,
            label,
        );

        if expect_refusal {
            assert!(
                failed,
                "{label}: {correction_lines} correction lines over a budget of 2 published \
                 anyway: {text}"
            );
            assert!(
                text.contains("correction-budget-exceeded"),
                "{label}: the publication refused for some other reason: {text}"
            );
        } else {
            assert!(
                !failed,
                "{label}: a repair inside its budget was refused: {text}"
            );
            let receipt = staged_receipt(home, 20);
            assert!(
                receipt.contains("budget-original-lines=4 budget=2 budget-cumulative=1"),
                "{label}: the accepted publication's own receipt does not carry the frozen \
                 budget it should have computed: {receipt}"
            );
        }
    }
}

/// `original_head` below is fabricated and unresolvable — if the code
/// bootstrapped instead of reading the parent's frozen state back, this
/// would fail loudly, not silently.
#[test]
fn a_repair_within_budget_alone_is_refused_by_the_accumulated_total() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000002";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/21-budget-cumulative";
    g(&["checkout", "--quiet", "-b", branch, "main"]);

    // Content the frozen genesis paths below name, so this round's diff
    // stays admissible.
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\ne0\n")
        .expect("the first repair's own content");
    g(&["add", "feature.txt"]);
    commit("the first repair");
    let first_repair_head = sha("HEAD");

    // The second repair, under test: two more lines. Alone, `2 <= 2` is
    // inside the frozen budget; only the accumulated `1 + 2 = 3` is not.
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    feature.push_str("e1\ne2\n");
    std::fs::write(repo.join("feature.txt"), feature).expect("the second repair");
    g(&["add", "feature.txt"]);
    commit("the second repair");
    let head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    let original_epoch = "1".repeat(32);
    let original_head = "f".repeat(40);
    let original_digest = "1".repeat(64);
    let first_repair_epoch = "2".repeat(32);
    let first_repair_digest = "2".repeat(64);
    let first_repair_delta = "3".repeat(64);
    // A fake, opaque tree id — never resolved, only carried forward on the
    // marker.
    let genesis_tree = "4".repeat(40);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let original_publication = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={original_head} base={base_sha} digest={original_digest} \
             epoch={original_epoch} -->\n"
        ),
    });
    let first_repair_publication = serde_json::json!({
        "id": "IC_3", "createdAt": "2026-01-01T02:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={first_repair_head} base={base_sha} digest={first_repair_digest} \
             epoch={first_repair_epoch} parent={original_epoch} parent-pr=7 \
             parent-head={original_head} parent-base={base_sha} \
             parent-digest={original_digest} delta={first_repair_delta} \
             budget-candidate-tree={first_repair_head} budget-genesis-tree={genesis_tree} \
             budget-genesis-paths=feature.txt budget-original-lines=4 budget=2 \
             budget-cumulative=1 -->\n"
        ),
    });
    let first_repair_publication_cause = republish_cause(
        run_id,
        &first_repair_head,
        &base_sha,
        &first_repair_digest,
        &first_repair_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, original_publication, first_repair_publication, first_repair_publication_cause[0].clone(), first_repair_publication_cause[1].clone(), first_repair_publication_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        21,
        branch,
        &timeline,
        &pr,
        "budget-cumulative",
    );

    assert!(
        failed,
        "a second repair adding 2 lines, on top of a first repair's own 1, was not refused even \
         though the running total (1 + 2 = 3) exceeds the frozen budget of 2: {text}"
    );
    assert!(
        text.contains("correction-budget-exceeded"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        text.contains("cumulative total to 3 against a budget of 2"),
        "the refusal did not carry the parent marker's own cumulative total forward: {text}"
    );
}

/// An unmerged base advance never reaches the correction budget at all —
/// `publish_review` is refused earlier, before the payload is
/// materialised, because the two-dot diff `main`'s advance against a branch
/// that never merged it would carry `mainfeature.txt` as this repair's own
/// deletion. Before that refusal existed, this same fixture published
/// successfully and the budget froze from the candidate's own size (`4`/`2`)
/// rather than the advance's `204`/`102` — `docs/honesty.md` says why that
/// budget mechanism's own reasoning is still correct, and unreachable through
/// this call only because the newer, earlier refusal answers first.
#[test]
fn an_unmerged_base_advance_is_refused_before_the_correction_budget_ever_runs() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000004";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/22-budget-unmerged-advance";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");

    g(&["checkout", "--quiet", "main"]);
    let advance: String = (0..50).map(|n| format!("main{n}\n")).collect();
    std::fs::write(repo.join("mainfeature.txt"), advance).expect("the base's own advance");
    g(&["add", "mainfeature.txt"]);
    commit("main advances far past the frozen budget");
    g(&["push", "--quiet", "origin", "main"]);
    let moved_base_sha = sha("main");

    g(&["checkout", "--quiet", branch]);
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    feature.push_str("extra0\n");
    std::fs::write(repo.join("feature.txt"), feature).expect("the repair");
    g(&["add", "feature.txt"]);
    commit("the repair");
    let head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    // `baseRefOid` reports the base branch's **current** tip regardless of
    // whether the branch has merged it.
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": moved_base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        22,
        branch,
        &timeline,
        &pr,
        "budget-unmerged-advance",
    );

    assert!(
        failed,
        "a repair whose base advanced past what the branch was rebased onto, unmerged, was \
         accepted — the two-dot diff this would have published carried mainfeature.txt's 50 \
         lines as this repair's own deletion: {text}"
    );
    assert!(
        text.contains("publication-base-not-ancestor"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        !any_receipt_staged(home, 22),
        "a receipt was staged despite the refusal, so something was written before it fired"
    );
}

/// The caller's own fields — `run-id`, `topology` and
/// `topology-reason` — are the only part of the published comment that no
/// constant bounds, and a value that renders past GitHub's 65,536-character cap
/// is refused **before any remote write** — not after the branch is pushed and
/// GitHub refuses the comment.
#[test]
fn an_oversize_topology_is_refused_before_any_remote_write() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-topology00000007";
    let branch = "fix/307-topology-bound";
    let g = |args: &[&str]| checked_git(repo, args);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = correction_budget_sha(repo, "main");

    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the candidate");
    g(&["add", "feature.txt"]);
    commit("the candidate");
    let head = correction_budget_sha(repo, "HEAD");

    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    // The measurement counts the value twice — percent-encoded on the marker and
    // escaped in the prose — so 40,000 characters render far past the budget
    // left by the 65,536 cap, the 8,192 fixed overhead, the 32,768 manifest
    // ceiling and the lane's measured width.
    let oversize = "x".repeat(40_000);
    let label = "topology-oversize";
    let (failed, text, _temp) = publication_attempt(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        307,
        "publish_review",
        branch,
        &timeline,
        &pr,
        label,
        Some(&oversize),
        None,
    );

    assert!(failed, "an oversize topology was accepted: {text}");
    assert!(
        text.contains("publication-comment-too-large"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        !any_receipt_staged(home, 307),
        "a receipt was staged despite the refusal"
    );
    // The remote is unchanged: the fake `gh` log names only the read
    // `verify_claim` made. Any create, edit, ready or workflow call is a write,
    // and none may appear before the refusal.
    let calls = std::fs::read_to_string(trace.path().join(format!("{label}.log")))
        .expect("the fake tracker call log");
    for write in [
        "pr create",
        "pr edit",
        "pr ready",
        "pr comment",
        "workflow run",
    ] {
        assert!(
            !calls.contains(write),
            "the refusal let a remote write through: {write:?} is in {calls:?}"
        );
    }
}

/// The recovery `publication-base-not-ancestor` names must clear
/// the block on a repair round. A rebase followed by the ordinary
/// `publish_review` push is a non-fast-forward once the branch is on the
/// remote, so the refusal now names `republish_review`; this drives the whole
/// path — a repair publication refused, a rebase, then the republish that must
/// succeed rather than refuse again. During a correction the
/// refusal names only the merge, so the rebase here is taken without its advice.
#[test]
fn the_base_advance_refusal_sends_a_repair_round_to_the_merge_and_republish_still_clears_a_rebase()
{
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-recover4a2b1c3d";
    let issue = 305u64;
    let branch = "fix/305-base-recovery";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };
    let remote_head = || -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["rev-parse", &format!("refs/heads/{branch}")])
            .output()
            .expect("the origin is readable");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let is_ancestor = |ancestor: &str, descendant: &str| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["merge-base", "--is-ancestor", ancestor, descendant])
            .status()
            .expect("git runs")
            .success()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    // The parent publication's own branch, actually pushed, so the lease the
    // republish takes below is against real remote bytes.
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    // `main` advances on a path the branch never touches, so replaying the
    // branch's own commit onto it is conflict-free.
    g(&["checkout", "--quiet", "main"]);
    std::fs::create_dir_all(repo.join("docs")).expect("the docs directory");
    std::fs::write(repo.join("docs").join("other.md"), "Ordinary prose.\n")
        .expect("main's own advance");
    g(&["add", "docs/other.md"]);
    commit("main advances on a path the branch never touches");
    g(&["push", "--quiet", "origin", "main"]);
    let advanced_base_sha = sha("main");

    // The publication reads the head from the checkout, so the repair attempt
    // below runs from the branch rather than from `main`.
    g(&["checkout", "--quiet", branch]);

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });

    // The repair attempt before the recovery: the branch still sits on
    // `parent_head`, so `main`'s advanced tip is not an ancestor of it.
    let pr_before = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": parent_head, "baseRefOid": advanced_base_sha,
        "state": "OPEN", "isDraft": true,
    });
    let (failed, text, _temp) = publication_attempt(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        issue,
        "publish_review",
        branch,
        &timeline,
        &pr_before,
        "recovery-before",
        None,
        None,
    );
    assert!(
        failed,
        "a repair whose base advanced, before any rebase, was accepted: {text}"
    );
    assert!(
        text.contains("publication-base-not-ancestor"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        text.contains("git -C <worktree> merge --no-edit origin/main")
            && !text.contains("republish_review")
            && text.contains("validator-delta-carries-upstream"),
        "a repair round must be sent to the merge and told why a rebase is not offered: {text}"
    );
    assert_eq!(
        remote_head(),
        parent_head,
        "the refusal moved the remote despite saying nothing was pushed"
    );
    assert!(
        !any_receipt_staged(home, issue),
        "a receipt was staged despite the refusal, so something was written before it fired"
    );

    g(&["rebase", &advanced_base_sha]);
    let rebased_head = sha("HEAD");
    assert_ne!(
        rebased_head, parent_head,
        "the rebase produced no new commit, so the test proves nothing"
    );
    assert!(
        is_ancestor(&advanced_base_sha, &rebased_head),
        "the rebase did not make the advanced base an ancestor of the head"
    );

    // The next call must clear the block, not refuse again.
    let pr_after = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": rebased_head, "baseRefOid": advanced_base_sha,
        "state": "OPEN", "isDraft": true,
    });
    let (failed, text, _temp) = publication_attempt(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        issue,
        "republish_review",
        branch,
        &timeline,
        &pr_after,
        "recovery-after",
        None,
        None,
    );
    assert!(
        !failed,
        "the named recovery did not clear the block, it refused again: {text}"
    );
    assert!(
        !text.contains("publication-base-not-ancestor"),
        "the recovery landed on the same refusal again: {text}"
    );
    assert_eq!(
        remote_head(),
        rebased_head,
        "the rewritten branch did not reach the remote: {text}"
    );
}

#[test]
fn an_unmerged_base_advance_pushed_from_elsewhere_is_refused_once_the_fetch_moves_the_ref() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let other = tempfile::tempdir().expect("a second working directory");
    let run_id = "claude-elsewh4a2b1c3d";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--initial-branch=main", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/317-advance-pushed-from-elsewhere";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");

    assert!(
        Command::new("git")
            .args(["clone", "--quiet"])
            .arg(origin.path())
            .arg(other.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    let og = |args: &[&str]| checked_git(other.path(), args);
    let other_sha = |what: &str| correction_budget_sha(other.path(), what);
    let advance: String = (0..50).map(|n| format!("main{n}\n")).collect();
    std::fs::write(other.path().join("mainfeature.txt"), advance).expect("the base's own advance");
    og(&["add", "mainfeature.txt"]);
    og(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "main advances far past the frozen budget",
    ]);
    og(&["push", "--quiet", "origin", "main"]);
    let moved_base_sha = other_sha("HEAD");

    g(&["checkout", "--quiet", branch]);
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    feature.push_str("extra0\n");
    std::fs::write(repo.join("feature.txt"), feature).expect("the repair");
    g(&["add", "feature.txt"]);
    commit("the repair");
    let head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    let remote_tracking_before = sha("refs/remotes/origin/main");
    assert_eq!(
        remote_tracking_before, base_sha,
        "the publishing repository's origin/main is not at the base this fixture pushed, so the \
         stale-ref shape it exists to cover was never set up"
    );

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    // `baseRefOid` reports the base branch's **current** tip regardless of
    // whether the branch has merged it.
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": moved_base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        317,
        branch,
        &timeline,
        &pr,
        "advance-pushed-from-elsewhere",
    );

    assert_eq!(
        sha("refs/remotes/origin/main"),
        moved_base_sha,
        "publish_review's own fetch did not move the stale remote-tracking ref, so this fixture no \
         longer sets up the shape it exists to cover: {text}"
    );
    assert!(
        failed,
        "a base advance pushed from a different checkout was accepted where the same advance \
         pushed from the publishing repository is refused: {text}"
    );
    assert!(
        text.contains("publication-base-not-ancestor"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        !any_receipt_staged(home, 317),
        "a receipt was staged despite the refusal, so something was written before it fired"
    );
}

#[test]
fn a_first_publication_is_not_budgeted() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000007";
    let g = |args: &[&str]| checked_git(repo, args);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);

    let branch = "fix/23-budget-first-publication";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    // Large enough that a defect which ran the arithmetic anyway would
    // refuse it against a stray budget like 200.
    let large: String = (0..2000).map(|n| format!("line{n}\n")).collect();
    std::fs::write(repo.join("feature.txt"), large).expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    g(&["push", "--quiet", "-u", "origin", branch]);
    let head = correction_budget_sha(repo, "HEAD");

    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": correction_budget_sha(repo, "main"),
        "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        23,
        branch,
        &timeline,
        &pr,
        "budget-first-publication",
    );

    assert!(!failed, "a first publication was refused: {text}");
    let receipt = staged_receipt(home, 23);
    for field in [
        "budget=",
        "budget-original-lines=",
        "budget-cumulative=",
        "budget-refrozen=",
    ] {
        assert!(
            !receipt.contains(field),
            "a first publication carried a `{field}` field, so it was budgeted against \
             something it never repairs: {receipt}"
        );
    }
    assert!(
        receipt.contains("budget-candidate-tree="),
        "a first publication must still record its own candidate tree, on the same terms \
         `topology` and `payload-digest` already do: a caller's value, written to the marker, \
         and read back by nothing — every parent tree this transport compares is recomputed \
         from the immutable `parent-head` a receipt already binds, never read off this field: \
         {receipt}"
    );
}

#[test]
fn a_repair_on_a_new_pull_request_is_not_charged_against_the_old_ones_budget() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000008";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    // The abandoned PR #7: a small original candidate, `floor_two(4) == 2`.
    // Never actually published through this fixture — only its own
    // `published` marker is recorded on the issue timeline, the way a real
    // one already sitting there would be.
    g(&["checkout", "--quiet", "-b", "fix/24-pr7", "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("PR #7's own candidate");
    g(&["add", "feature.txt"]);
    commit("PR #7's own candidate");
    let pr7_head = sha("HEAD");
    g(&["checkout", "--quiet", "main"]);

    // A genuinely second change on the same issue: a fresh branch cut from
    // the same, unmoved `main` — so `merge_base(this_base, this_head)` is an
    // ancestor of `pr7_head`, the ordinary case rather than the exotic one —
    // that happens to touch the same filename with unrelated content.
    let branch = "fix/24-pr8";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    let unrelated: String = (0..50).map(|n| format!("other-line-{n}\n")).collect();
    std::fs::write(repo.join("feature.txt"), unrelated).expect("PR #8's own candidate");
    g(&["add", "feature.txt"]);
    commit("PR #8's own candidate, sharing PR #7's file name and nothing else");
    g(&["push", "--quiet", "-u", "origin", branch]);
    let pr8_head = sha("HEAD");

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let pr7_published = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={pr7_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let pr7_published_cause =
        republish_cause(run_id, &pr7_head, &base_sha, &parent_digest, &parent_epoch);
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, pr7_published, pr7_published_cause[0].clone(), pr7_published_cause[1].clone(), pr7_published_cause[2].clone()],
    });
    // PR #8: a different pull request number than the one the issue timeline
    // last recorded a publication against.
    let pr = serde_json::json!({
        "number": 8, "url": "https://github.com/o/r/pull/8",
        "headRefOid": pr8_head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        24,
        branch,
        &timeline,
        &pr,
        "budget-crosses-pull-request",
    );

    assert!(
        !failed,
        "a first publication on a new pull request was refused against a different pull \
         request's frozen budget: {text}"
    );
    let receipt = staged_receipt(home, 24);
    for field in ["budget=", "budget-original-lines=", "budget-cumulative="] {
        assert!(
            !receipt.contains(field),
            "a publication on a new pull request carried a `{field}` field frozen from a \
             different pull request's own candidate: {receipt}"
        );
    }
}

/// A local, **untracked** `.git/info/attributes` — the shape no `git status`
/// ever lists — cannot mark every path `binary` and suppress the budget's own
/// count, because the budget's tree-to-tree measurement runs in an isolated
/// `GIT_DIR` with no attribute source of its own.
#[test]
fn a_local_gitattributes_binary_override_cannot_silence_the_budget() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000009";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/26-budget-gitattributes";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    // The original candidate: four added lines. `floor_two(4) == 2`.
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");

    // Local and untracked: a real `.git/info/attributes`, the file
    // `clean_status`'s `--untracked-files=all` never sees because it is
    // inside the admin directory rather than the tree.
    let git_dir_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--path-format=absolute", "--git-dir"])
        .output()
        .expect("git answers");
    let git_dir = std::path::PathBuf::from(String::from_utf8_lossy(&git_dir_output.stdout).trim());
    std::fs::create_dir_all(git_dir.join("info")).expect("the admin directory's info directory");
    std::fs::write(git_dir.join("info").join("attributes"), "* -diff\n")
        .expect("a local, untracked attributes file");

    // Ten lines, past the frozen budget of 2 — the same "exceeds" shape
    // `a_repair_past_the_frozen_correction_budget_is_refused` uses.
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    for n in 0..10u64 {
        feature.push_str(&format!("extra{n}\n"));
    }
    std::fs::write(repo.join("feature.txt"), feature).expect("the repair");
    g(&["add", "feature.txt"]);
    commit("the repair");
    let head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        26,
        branch,
        &timeline,
        &pr,
        "budget-gitattributes",
    );

    assert!(
        failed,
        "a ten-line repair over a budget of 2 published anyway, with `* -diff` written to a \
         local, untracked `.git/info/attributes` marking every path binary and unmeasured: \
         {text}"
    );
    assert!(
        text.contains("correction-budget-exceeded"),
        "the publication refused for some other reason: {text}"
    );
}

#[test]
fn a_side_branch_repair_merged_back_is_counted_in_full_and_refused() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000003";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/24-budget-side-branch";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");

    // Merged back with `--no-ff`, which a first-parent-only diff cannot walk.
    g(&["switch", "--quiet", "-c", "scratch"]);
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    for n in 0..500 {
        feature.push_str(&format!("extra{n}\n"));
    }
    std::fs::write(repo.join("feature.txt"), feature).expect("the side-branch repair");
    g(&["add", "feature.txt"]);
    commit("a 500-line repair, authored on a side branch");
    g(&["switch", "--quiet", branch]);
    g(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "merge",
        "--quiet",
        "--no-ff",
        "--no-edit",
        "scratch",
    ]);
    let head = sha("HEAD");
    g(&["branch", "-D", "scratch"]);
    g(&["push", "--quiet", "-u", "origin", branch]);

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        24,
        branch,
        &timeline,
        &pr,
        "budget-side-branch",
    );

    assert!(
        failed,
        "a 500-line repair authored on a side branch and merged back with --no-ff published \
         anyway — the escape the fifth attempt's `--first-parent` walk left open: {text}"
    );
    assert!(
        text.contains("correction-budget-exceeded"),
        "the publication refused for some other reason: {text}"
    );
    assert!(
        text.contains("500 changed lines"),
        "the merge was not counted in full — `--first-parent` would have answered 0 here: {text}"
    );
}

#[derive(Clone, Copy, Debug)]
enum Integration {
    Merge,
    Rebase,
}

struct MovedBaseRound<'a> {
    integration: Integration,
    parent_cumulative: Option<u64>,
    conflicting: bool,
    rename: bool,
    swapped: bool,
    ambient_config: bool,
    repair: &'a [(&'a str, usize)],
}

/// One repair round over a base that advanced past the parent publication's:
/// the branch's `feature.txt` is the 20-line original candidate (budget 10), and
/// main's advance adds a 200-line `moved.txt`. Answers whether the publication
/// refused, its answer text, and the staged receipt when it did not.
fn moved_base_round(round: &MovedBaseRound) -> (bool, String, String) {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000005";
    let identity = [
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
    ];
    let g = |args: &[&str]| checked_git(repo, args);
    let with_identity = |args: &[&str]| {
        let mut all = identity.to_vec();
        all.extend_from_slice(args);
        checked_git(repo, &all)
    };
    let sha = |what: &str| correction_budget_sha(repo, what);
    let lines = |prefix: &str, count: usize| -> String {
        (0..count).map(|n| format!("{prefix}{n}\n")).collect()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    std::fs::write(repo.join("shared.txt"), lines("s", 10)).expect("the shared file");
    g(&["add", "kept.txt", "shared.txt"]);
    with_identity(&["commit", "--quiet", "-m", "base content"]);
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/25-moved-base";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), lines("a", 20)).expect("the original candidate");
    g(&["add", "feature.txt"]);
    if round.conflicting || round.rename {
        let shared = format!("branch\n{}", lines("s", 10).split_once('\n').unwrap().1);
        std::fs::write(repo.join("shared.txt"), shared).expect("the branch's shared edit");
        g(&["add", "shared.txt"]);
    }
    with_identity(&["commit", "--quiet", "-m", "the original candidate"]);
    let parent_head = sha("HEAD");

    if round.swapped {
        g(&["checkout", "--quiet", "--orphan", "other"]);
        g(&["rm", "-rf", "--quiet", "."]);
        std::fs::write(repo.join("moved.txt"), lines("m", 200)).expect("the swapped base");
        g(&["add", "moved.txt"]);
        with_identity(&["commit", "--quiet", "-m", "an unrelated line of history"]);
        g(&["branch", "-f", "main", "other"]);
    } else {
        g(&["checkout", "--quiet", "main"]);
        std::fs::write(repo.join("moved.txt"), lines("m", 200)).expect("the base's own advance");
        g(&["add", "moved.txt"]);
        if round.conflicting {
            let shared = format!("main\n{}", lines("s", 10).split_once('\n').unwrap().1);
            std::fs::write(repo.join("shared.txt"), shared).expect("main's shared edit");
            g(&["add", "shared.txt"]);
        }
        if round.rename {
            g(&["mv", "shared.txt", "shared_renamed.txt"]);
        }
        with_identity(&["commit", "--quiet", "-m", "main advances"]);
    }
    g(&["push", "--quiet", "--force", "origin", "main"]);
    let moved_base_sha = sha("main");

    g(&["checkout", "--quiet", branch]);
    let resolve = || {
        std::fs::write(
            repo.join("shared.txt"),
            lines("s", 10).replacen("s0", "resolved", 1),
        )
        .expect("the author's resolution");
        g(&["add", "shared.txt"]);
    };
    match (round.integration, round.conflicting) {
        (Integration::Merge, false) => {
            let unrelated: &[&str] = if round.swapped {
                &["--allow-unrelated-histories"]
            } else {
                &[]
            };
            let mut args = vec!["merge", "--quiet", "--no-edit"];
            args.extend_from_slice(unrelated);
            args.push("main");
            with_identity(&args);
        }
        (Integration::Merge, true) => {
            assert!(
                !Command::new("git")
                    .arg("-C")
                    .arg(repo)
                    .args(identity)
                    .args(["merge", "--quiet", "--no-edit", "main"])
                    .output()
                    .expect("git runs")
                    .status
                    .success(),
                "the fixture's merge did not conflict"
            );
            resolve();
            with_identity(&["commit", "--quiet", "--no-edit"]);
        }
        (Integration::Rebase, false) => {
            with_identity(&["rebase", "--quiet", "main"]);
        }
        (Integration::Rebase, true) => {
            unreachable!("only the merge shape is driven through a conflict")
        }
    }
    for (path, count) in round.repair {
        let mut content = std::fs::read_to_string(repo.join(path)).expect("the repaired file");
        content.push_str(&lines("r", *count));
        std::fs::write(repo.join(path), content).expect("the repair");
        g(&["add", path]);
    }
    if !round.repair.is_empty() {
        with_identity(&["commit", "--quiet", "-m", "the repair"]);
    }
    if round.ambient_config {
        g(&["config", "merge.conflictStyle", "diff3"]);
        g(&["config", "merge.pin.driver", "true"]);
        let git_dir = checked_git_stdout(repo, &["rev-parse", "--absolute-git-dir"]);
        let info = std::path::Path::new(git_dir.trim()).join("info");
        std::fs::create_dir_all(&info).expect("the info directory");
        std::fs::write(info.join("attributes"), "shared.txt merge=pin\n")
            .expect("the local attributes");
    }
    let head = sha("HEAD");
    g(&["push", "--quiet", "--force", "-u", "origin", branch]);

    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let frozen_lines = if round.conflicting || round.rename {
        22
    } else {
        20
    };
    let budget_fields = round
        .parent_cumulative
        .map_or_else(String::new, |cumulative| {
            format!(
                " budget-candidate-tree={parent_head} budget-genesis-tree={} \
             budget-genesis-paths=feature.txt budget-original-lines={frozen_lines} \
             budget={} budget-cumulative={cumulative}",
                "4".repeat(40),
                frozen_lines / 2,
            )
        });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={} epoch={}{budget_fields} -->\n",
            "8".repeat(64),
            "7".repeat(32),
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &"8".repeat(64),
        &"7".repeat(32),
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": moved_base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        25,
        branch,
        &timeline,
        &pr,
        "moved-base",
    );
    let receipt = if failed {
        String::new()
    } else {
        staged_receipt(home, 25)
    };
    (failed, text, receipt)
}

const BOTH_INTEGRATIONS: [Integration; 2] = [Integration::Merge, Integration::Rebase];

fn plain_round<'a>(
    integration: Integration,
    parent_cumulative: Option<u64>,
    repair: &'a [(&'a str, usize)],
) -> MovedBaseRound<'a> {
    MovedBaseRound {
        integration,
        parent_cumulative,
        conflicting: false,
        rename: false,
        swapped: false,
        ambient_config: false,
        repair,
    }
}

#[test]
fn a_moved_base_is_not_charged_to_the_repair_and_keeps_the_frozen_budget() {
    for integration in BOTH_INTEGRATIONS {
        let (failed, text, receipt) =
            moved_base_round(&plain_round(integration, None, &[("feature.txt", 3)]));
        assert!(
            !failed,
            "{integration:?}: a 3-line repair was refused: {text}"
        );
        assert!(
            receipt.contains("budget-parent=integrated"),
            "{integration:?}: the round was not recorded as measured against an integrated \
             parent: {receipt}"
        );
        assert!(
            !receipt.contains("budget-refrozen"),
            "{integration:?}: the base advance re-froze the lineage: {receipt}"
        );
        assert_eq!(
            receipt.contains("delta-parent=integrated"),
            matches!(integration, Integration::Merge),
            "{integration:?}: only a merged base is integrated for the validator: {receipt}"
        );
        assert!(
            receipt.contains("budget-original-lines=20 budget=10")
                && receipt.contains("budget-genesis-paths=feature.txt")
                && receipt.contains("budget-cumulative=3"),
            "{integration:?}: the base's own 200 lines were charged, or the genesis moved: \
             {receipt}"
        );
    }
}

#[test]
fn a_moved_base_still_refuses_a_repair_over_the_frozen_budget() {
    for integration in BOTH_INTEGRATIONS {
        let (failed, text, _) =
            moved_base_round(&plain_round(integration, None, &[("feature.txt", 11)]));
        assert!(
            failed,
            "{integration:?}: an 11-line repair against a budget of 10: {text}"
        );
        assert!(
            text.contains("correction-budget-exceeded") && text.contains("11 changed lines"),
            "{integration:?}: {text}"
        );
        assert!(
            text.contains("against the tree git merges the parent head and the new base into"),
            "{integration:?}: an integrated round's budget refusal described the wrong \
             measurement: {text}"
        );
    }
}

#[test]
fn a_moved_base_carries_the_parents_cumulative_instead_of_resetting_it() {
    for integration in BOTH_INTEGRATIONS {
        let (failed, text, receipt) =
            moved_base_round(&plain_round(integration, Some(6), &[("feature.txt", 3)]));
        assert!(
            !failed,
            "{integration:?}: 6 + 3 is inside the budget of 10: {text}"
        );
        assert!(
            receipt.contains("budget-cumulative=9"),
            "{integration:?}: the parent's cumulative was not carried: {receipt}"
        );
        let (failed, text, _) =
            moved_base_round(&plain_round(integration, Some(6), &[("feature.txt", 5)]));
        assert!(
            failed,
            "{integration:?}: 6 + 5 exceeds the budget of 10: {text}"
        );
        assert!(
            text.contains("cumulative total to 11 against a budget of 10"),
            "{integration:?}: a base advance cleared what the lineage had spent: {text}"
        );
    }
}

#[test]
fn a_moved_base_refuses_a_path_the_advance_changed_but_the_merge_did_not_conflict_on() {
    for integration in BOTH_INTEGRATIONS {
        // `moved.txt` is the base's own advance, merged cleanly: the author has
        // no honest reason to touch it, so it is outside the round's scope even
        // though the advance changed it.
        let (failed, text, _) =
            moved_base_round(&plain_round(integration, None, &[("moved.txt", 2)]));
        assert!(
            failed
                && text.contains("correction-outside-reviewed-scope")
                && text.contains("integrated_paths"),
            "{integration:?}: a path the advance changed but the merge did not conflict on was \
             admitted: {text}"
        );
        assert!(
            text.contains("tree git merges the parent head and the new base into")
                && !text.contains("changed outside the reviewed candidate's own paths"),
            "{integration:?}: the refusal described the pre-integration measurement: {text}"
        );

        let (failed, text, _) =
            moved_base_round(&plain_round(integration, None, &[("kept.txt", 1)]));
        assert!(
            failed
                && text.contains("correction-outside-reviewed-scope")
                && text.contains("integrated_paths")
                && text.contains("kept.txt"),
            "{integration:?}: a path outside genesis and outside the integration: {text}"
        );
    }
}

#[test]
fn a_moved_base_admits_a_path_the_merge_conflicted_on() {
    // `parent_cumulative: Some(0)` makes the parent carry a recorded genesis of
    // `feature.txt` alone, so `shared.txt` — the path the merge conflicted on —
    // is admitted by the conflict, not by the genesis.
    let (failed, text, receipt) = moved_base_round(&MovedBaseRound {
        integration: Integration::Merge,
        parent_cumulative: Some(0),
        conflicting: true,
        rename: false,
        swapped: false,
        ambient_config: false,
        repair: &[("shared.txt", 1)],
    });
    assert!(
        !failed,
        "a repair to a path the merge conflicted on was refused: {text}"
    );
    assert!(
        receipt.contains("budget-genesis-paths=feature.txt")
            && receipt.contains("budget-cumulative=7"),
        "the conflict resolution and the repair were not measured: {receipt}"
    );
}

/// The advance renamed `shared.txt` to `shared_renamed.txt`; the branch's own
/// genesis modified `shared.txt`, so git applies that modification to the
/// destination and the merge does not conflict. The repair touches the
/// destination, which is neither a genesis path nor a conflicted path, so it
/// is admitted only by the rename-destination clause.
#[test]
fn a_moved_base_admits_the_destination_of_a_rename_the_advance_made() {
    let (failed, text, receipt) = moved_base_round(&MovedBaseRound {
        integration: Integration::Merge,
        parent_cumulative: None,
        conflicting: false,
        rename: true,
        swapped: false,
        ambient_config: false,
        repair: &[("shared_renamed.txt", 1)],
    });
    assert!(
        !failed,
        "a repair to the destination of a rename the advance made was refused: {text}"
    );
    assert!(
        receipt.contains("budget-cumulative=1"),
        "the repair to the rename destination was not measured: {receipt}"
    );
}

#[test]
fn a_conflict_resolution_is_counted_as_the_authors_lines() {
    let (failed, text, receipt) = moved_base_round(&MovedBaseRound {
        integration: Integration::Merge,
        parent_cumulative: None,
        conflicting: true,
        rename: false,
        swapped: false,
        ambient_config: false,
        repair: &[],
    });
    assert!(!failed, "{text}");
    assert!(
        receipt.contains("budget-cumulative=6") && receipt.contains("budget-parent=integrated"),
        "the five conflict-marker lines and the one resolved line were not measured: {receipt}"
    );
    assert!(
        !receipt.contains("delta-parent=integrated"),
        "a merge that conflicted was recorded as cleanly integrated: {receipt}"
    );
}

#[test]
fn a_repositorys_own_merge_configuration_does_not_change_the_charge() {
    let (failed, text, receipt) = moved_base_round(&MovedBaseRound {
        integration: Integration::Merge,
        parent_cumulative: None,
        conflicting: true,
        rename: false,
        swapped: false,
        ambient_config: true,
        repair: &[],
    });
    assert!(!failed, "{text}");
    assert!(
        receipt.contains("budget-cumulative=6"),
        "a local diff3 conflict style or merge driver changed the charged lines: {receipt}"
    );
}

#[test]
fn a_swapped_base_still_refreezes_the_lineage() {
    let (failed, text, receipt) = moved_base_round(&MovedBaseRound {
        integration: Integration::Merge,
        parent_cumulative: Some(6),
        conflicting: false,
        rename: false,
        swapped: true,
        ambient_config: false,
        repair: &[("feature.txt", 1)],
    });
    assert!(!failed, "{text}");
    assert!(
        receipt.contains("budget-refrozen=base-advanced")
            && !receipt.contains("budget-parent=integrated"),
        "a base swapped for another line of history is not an advance of this one: {receipt}"
    );
}

#[test]
fn a_merged_advance_is_recorded_as_read_whole_and_a_rebased_one_as_rewritten() {
    for (integration, reason) in [
        (Integration::Merge, "delta-range-reason=base-advanced"),
        (Integration::Rebase, "delta-range-reason=history-rewritten"),
    ] {
        let (failed, text, receipt) =
            moved_base_round(&plain_round(integration, None, &[("feature.txt", 1)]));
        assert!(!failed, "{text}");
        assert!(receipt.contains(reason), "{integration:?}: {receipt}");
    }
}

#[test]
fn admission_refuses_a_path_outside_genesis_and_accepts_a_companion_test() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000006";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    std::fs::write(
        repo.join("CHANGELOG.md"),
        "# Changelog\n\n## Unreleased\n\n## Older\n",
    )
    .expect("the base changelog");
    g(&["add", "kept.txt", "CHANGELOG.md"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    for (label, added_path, added_content, expect_refusal) in [
        ("outside-genesis", "src/other.rs", "fn other() {}\n", true),
        (
            "companion-test",
            "tests/companion.rs",
            "fn companion_test() {}\n",
            false,
        ),
    ] {
        let branch = format!("fix/26-budget-admission-{label}");
        g(&["checkout", "--quiet", "-b", &branch, "main"]);
        std::fs::create_dir_all(repo.join("src")).expect("the src directory");
        std::fs::write(repo.join("src/feature.rs"), "fn feature() {\n}\n")
            .expect("the original candidate's own module");
        std::fs::write(
            repo.join("CHANGELOG.md"),
            "# Changelog\n\n## Unreleased\n- entry\n\n## Older\n",
        )
        .expect("the original candidate's own changelog entry");
        g(&["add", "src/feature.rs", "CHANGELOG.md"]);
        commit("the original candidate");
        let parent_head = sha("HEAD");

        std::fs::create_dir_all(repo.join(std::path::Path::new(added_path).parent().unwrap()))
            .expect("the added path's directory");
        std::fs::write(repo.join(added_path), added_content).expect("the repair's own addition");
        g(&["add", added_path]);
        commit("the repair");
        let head = sha("HEAD");
        g(&["push", "--quiet", "-u", "origin", &branch]);

        let parent_epoch = "7".repeat(32);
        let parent_digest = "8".repeat(64);
        let claimed = serde_json::json!({
            "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true, "includesCreatedEdit": false,
            "body": format!(
                "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
                 horizon=2099-01-01T00:00Z op-id={} -->\n",
                "a".repeat(32)
            ),
        });
        let earlier = serde_json::json!({
            "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
            "viewerDidAuthor": true, "includesCreatedEdit": false,
            "body": format!(
                "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
                 pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
                 epoch={parent_epoch} -->\n"
            ),
        });
        let earlier_cause = republish_cause(
            run_id,
            &parent_head,
            &base_sha,
            &parent_digest,
            &parent_epoch,
        );
        let timeline = serde_json::json!({
            "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
            "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
        });
        let pr = serde_json::json!({
            "number": 7, "url": "https://github.com/o/r/pull/7",
            "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
        });

        let (failed, text, _temp) = correction_budget_publish(
            home,
            repo,
            bin,
            trace.path(),
            run_id,
            26,
            &branch,
            &timeline,
            &pr,
            label,
        );

        if expect_refusal {
            assert!(
                failed,
                "{label}: a correction adding {added_path:?}, outside the reviewed genesis and \
                 not an admissible companion test, published anyway: {text}"
            );
            assert!(
                text.contains("correction-outside-reviewed-scope"),
                "{label}: the publication refused for some other reason: {text}"
            );
            assert!(
                text.contains(added_path),
                "{label}: the refusal did not name the offending path: {text}"
            );
        } else {
            assert!(
                !failed,
                "{label}: a correction adding only a companion test under `tests/` was refused: \
                 {text}"
            );
        }
    }
}

/// A second pull request publishing on the same issue must not divert the
/// budget on the *first* pull request's own next repair onto PR #8's
/// unrelated genesis. The issue's latest publication is PR #8's once it
/// lands, so a parent selected by validating that single candidate against
/// `this_pr`, rather than by finding PR #7's own latest, binds PR #7's
/// repair to a genesis it never touched.
#[test]
fn a_repair_interleaved_with_a_second_pull_requests_publication_is_still_budgeted() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000010";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    // PR #7's own original candidate: `floor_two(4) == 2`. Never actually
    // published through this fixture — only its own `published` marker sits
    // on the issue timeline, the way one a real earlier round wrote would.
    let branch = "fix/27-pr7";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("PR #7's own candidate");
    g(&["add", "feature.txt"]);
    commit("PR #7's own candidate");
    let pr7_original_head = sha("HEAD");

    // PR #7's own repair: well past the frozen budget of 2, on the same
    // genesis path, so admission passes and only the budget can refuse it.
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    for n in 0..20 {
        feature.push_str(&format!("extra{n}\n"));
    }
    std::fs::write(repo.join("feature.txt"), feature).expect("PR #7's own repair");
    g(&["add", "feature.txt"]);
    commit("PR #7's own repair, twenty lines past its frozen budget");
    let pr7_repair_head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    // PR #8's own, unrelated candidate: a real, resolvable commit on a
    // sibling branch cut from the same `main` — the receipt naming a real
    // head is what makes this fixture reach the routing defect rather than
    // a resolution failure on a hash nothing ever wrote.
    g(&["checkout", "--quiet", "-b", "fix/27-pr8", "main"]);
    std::fs::write(repo.join("other.txt"), "x\n").expect("PR #8's own candidate");
    g(&["add", "other.txt"]);
    commit("PR #8's own candidate, an unrelated file on an unrelated branch");
    let pr8_head = sha("HEAD");
    g(&["checkout", "--quiet", branch]);

    let pr7_epoch = "7".repeat(32);
    let pr7_digest = "8".repeat(64);
    let pr8_epoch = "9".repeat(32);
    let pr8_digest = "a".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let pr7_published = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={pr7_original_head} base={base_sha} digest={pr7_digest} \
             epoch={pr7_epoch} -->\n"
        ),
    });
    // PR #8, on a wholly unrelated branch never checked out here, publishes
    // *after* PR #7 — so it is the issue's latest publication by the time
    // PR #7's own repair reads the timeline.
    let pr8_published = serde_json::json!({
        "id": "IC_3", "createdAt": "2026-01-01T02:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=8 head={pr8_head} base={base_sha} digest={pr8_digest} epoch={pr8_epoch} -->\n"
        ),
    });
    let pr8_published_cause =
        republish_cause(run_id, &pr8_head, &base_sha, &pr8_digest, &pr8_epoch);
    let pr7_published_cause = republish_cause(
        run_id,
        &pr7_original_head,
        &base_sha,
        &pr7_digest,
        &pr7_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, pr7_published, pr7_published_cause[0].clone(), pr7_published_cause[1].clone(), pr7_published_cause[2].clone(), pr8_published, pr8_published_cause[0].clone(), pr8_published_cause[1].clone(), pr8_published_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": pr7_repair_head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        27,
        branch,
        &timeline,
        &pr,
        "budget-interleaved-pull-request",
    );

    assert!(
        failed,
        "a twenty-line repair on PR #7, published after PR #8 published on the same issue, was \
         not refused against PR #7's own frozen budget of 2: {text}"
    );
    assert!(
        text.contains("correction-budget-exceeded"),
        "the publication refused for some other reason than the correction budget: {text}"
    );
}

/// The original candidate's own changed-path manifest, frozen as
/// `budget-genesis-paths` on the published marker, is bounded before anything
/// remote moves. Before this refusal existed, a manifest this
/// large would have been assembled only after `push_to_origin`, with the
/// branch already pushed and the pull request already a draft.
#[test]
fn a_frozen_genesis_manifest_too_large_to_record_is_refused_before_the_push() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-budget00000011";
    let g = |args: &[&str]| checked_git(repo, args);
    let sha = |what: &str| correction_budget_sha(repo, what);
    let commit = |message: &str| {
        g(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--quiet",
            "-m",
            message,
        ]);
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    g(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    g(&["branch", "-M", "main"]);
    std::fs::write(repo.join("kept.txt"), "root\n").expect("the base file");
    g(&["add", "kept.txt"]);
    commit("base content");
    g(&["push", "--quiet", "-u", "origin", "main"]);
    let base_sha = sha("main");

    let branch = "fix/28-budget-genesis-overflow";
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    // The original candidate: 350 paths, each carrying enough bytes outside
    // the marker's safe set (`!`, percent-encoded to three characters
    // apiece) that the joined, percent-encoded manifest alone clears
    // GENESIS_MANIFEST_CEILING (32 KiB), before the `\u{1f}` separators
    // between them are even counted.
    let unsafe_run = "!".repeat(30);
    let candidate_path = |n: u32| format!("g{n:04}{unsafe_run}.txt");
    for n in 0..350u32 {
        std::fs::write(repo.join(candidate_path(n)), "x\n")
            .expect("one path of the oversized candidate");
    }
    g(&["add", "."]);
    commit("the original candidate, an oversized manifest");
    let parent_head = sha("HEAD");

    // The repair: one line appended to an already-reviewed path, well inside
    // any budget `floor_two` could compute from ~350 original lines — the
    // point of this fixture is the manifest ceiling, not the line-count one.
    let first = candidate_path(0);
    let mut content = std::fs::read_to_string(repo.join(&first)).expect("the first path");
    content.push_str("y\n");
    std::fs::write(repo.join(&first), content).expect("the repair");
    g(&["add", &first]);
    commit("the repair");
    let head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    let parent_epoch = "7".repeat(32);
    let parent_digest = "8".repeat(64);
    let claimed = serde_json::json!({
        "id": "IC_1", "createdAt": "2026-01-01T00:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={run_id} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->\n",
            "a".repeat(32)
        ),
    });
    let earlier = serde_json::json!({
        "id": "IC_2", "createdAt": "2026-01-01T01:00Z",
        "viewerDidAuthor": true, "includesCreatedEdit": false,
        "body": format!(
            "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
             pr=7 head={parent_head} base={base_sha} digest={parent_digest} \
             epoch={parent_epoch} -->\n"
        ),
    });
    let earlier_cause = republish_cause(
        run_id,
        &parent_head,
        &base_sha,
        &parent_digest,
        &parent_epoch,
    );
    let timeline = serde_json::json!({
        "state": "OPEN", "assignees": [], "labels": [{"name": "status:in-progress"}],
        "comments": [claimed, earlier, earlier_cause[0].clone(), earlier_cause[1].clone(), earlier_cause[2].clone()],
    });
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base_sha, "state": "OPEN", "isDraft": true,
    });

    let label = "budget-genesis-overflow";
    let (failed, text, _temp) = correction_budget_publish(
        home,
        repo,
        bin,
        trace.path(),
        run_id,
        28,
        branch,
        &timeline,
        &pr,
        label,
    );

    assert!(
        failed,
        "a repair whose parent's own original candidate touched 350 oversized paths was \
         accepted despite a manifest too large to record on the published marker: {text}"
    );
    assert!(
        text.contains("genesis-manifest-too-large"),
        "the publication refused for some other reason: {text}"
    );

    let log = trace.path().join(format!("{label}.log"));
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    for wrote in ["pr create", "pr edit", "pr ready"] {
        assert!(
            !calls.contains(wrote),
            "`{wrote}` reached the remote before the manifest ceiling refused: {calls}"
        );
    }
    assert!(
        !any_receipt_staged(home, 28),
        "a receipt was staged despite the refusal, so something was written before it fired"
    );
}

/// A successor's `start_branch` is refused by name, not by git's own text.
///
/// Driven as a process because the defect is in the wiring: the unit test beside
/// `branch_pinned_elsewhere` proves the decision, and would stay green if the
/// call into `start_branch` were deleted. This is the half that cannot.
#[test]
fn a_successor_is_told_which_checkout_holds_the_branch() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let origin = tempfile::tempdir().expect("a bare origin");
    let trees = tempfile::tempdir().expect("a directory for worktrees");
    let run_id = "claude-beef5678";
    let branch = "fix-12-inherited";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success()),
        "the bare origin was not created"
    );
    git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string(),
    ]);
    git(&["branch", "-M", "main"]);
    assert!(
        git(&["push", "-q", "origin", "main"]),
        "the base did not push"
    );

    // The predecessor: a checkout holding the branch at a path this run's own
    // template will never compute, because the template is keyed to the run id.
    let predecessor = trees.path().join("left-behind-by-another-run");
    assert!(
        git(&["branch", branch, "main"]),
        "the inherited branch was not created"
    );
    assert!(
        checked_git(
            repo,
            &[
                "worktree",
                "add",
                "--quiet",
                "--",
                &predecessor.display().to_string(),
                branch,
            ],
        ),
        "the predecessor's checkout was not made"
    );

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:in-progress"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!("Claimed by {run_id}.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        { "matches": "issue develop", "stdout": "", "status": 1 },
        {
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "start_branch",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "expect_state": "in-progress",
                "worktree_root": trees.path().display().to_string(),
            },
        },
    })
    .to_string();

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::{BufRead, Write};
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let mut stdout = std::io::BufReader::new(child.stdout.take().expect("stdout is piped"));
    writeln!(stdin, "{request}").expect("the request is written");
    let mut line = String::new();
    stdout.read_line(&mut line).expect("the server answers");
    drop(stdin);
    let _ = child.wait();

    let answered: serde_json::Value =
        serde_json::from_str(line.trim()).unwrap_or_else(|_| panic!("not JSON-RPC: {line:?}"));
    let text = answered["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        answered["result"]["isError"], true,
        "a branch another checkout holds was not refused at all: {text}"
    );
    assert!(
        text.contains("branch-checked-out-elsewhere"),
        "the refusal did not name itself; git's own text is not a reason a caller can \
         branch on: {text}"
    );
    assert!(
        text.contains("left-behind-by-another-run"),
        "the refusal did not say which checkout holds the branch: {text}"
    );
}

/// Filing with a priority on no declared scale is accepted, and the queue read names it.
///
/// `create` attaches whatever `priority` it is given — `scale:high` here, the parameter's own
/// example text with `scale` where the scale's name belongs — and answers `priority_checked:
/// false` rather than refusing. `list_state` names the filed item under `unchecked` instead of
/// returning it looking rankable.
///
/// Turn it off by dropping the `unchecked` naming in `list_state` and this reddens on the
/// `unchecked` assertion; drop `priority_checked` in `create` and it reddens there.
#[test]
fn a_filed_item_with_an_undeclared_scale_is_named_by_the_queue_read() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");

    let listing = serde_json::to_string(&serde_json::json!([
        {
            "number": 101,
            "title": "filed on no scale",
            "labels": [{"name": "status:ready"}, {"name": "domain:general"}, {"name": "scale:high"}],
            "created_at": "2026-09-01T00:00:00Z",
        },
        {
            "number": 102,
            "title": "filed on the declared scale",
            "labels": [{"name": "status:ready"}, {"name": "domain:general"}, {"name": "priority:medium"}],
            "created_at": "2026-09-02T00:00:00Z",
        },
    ]))
    .expect("the queue listing serialises");
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "label create", "stdout": "", "status": 0},
        {"matches": "issue create", "stdout": "https://github.com/o/r/issues/101\n", "status": 0},
        {"matches": "issues?state=open", "stdout": serde_json::json!([serde_json::from_str::<serde_json::Value>(&listing).unwrap()]).to_string(), "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let filed: serde_json::Value = {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "create",
                "arguments": {
                    "identity": "a-probe",
                    "title": "filed on no scale",
                    "body_file": body.to_string_lossy(),
                    "priority": "scale:high",
                    "domain": "general",
                    "runtime": "opencode",
                    "run_id": "opencode-test-1a2b3c4d",
                    "state": "ready",
                },
            },
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
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the filed answer is JSON: {text}"))
    };
    assert_eq!(
        filed["ok"], true,
        "filing with an undeclared scale was refused instead of recorded: {filed}"
    );
    assert_eq!(
        filed["priority_checked"], false,
        "the filing did not record that its scale was unchecked: {filed}"
    );

    let queue: serde_json::Value = {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_state",
                "arguments": {"state": "ready", "run_id": "opencode-test-1a2b3c4d"},
            },
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
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"))
    };
    let named: Vec<u64> = queue["unchecked"]
        .as_array()
        .unwrap_or_else(|| panic!("the queue read named nothing unchecked: {queue}"))
        .iter()
        .filter_map(|entry| entry["number"].as_u64())
        .collect();
    assert_eq!(
        named,
        vec![101],
        "the queue read did not name exactly the item filed on no scale: {queue}"
    );
}

/// An empty value after the colon — `priority:` — is no value at all. `create` answers
/// `priority_checked: false` for it, and `list_state` names it under `unchecked` rather than
/// returning it looking rankable, exactly as for a filing on a scale the transport cannot read.
///
/// Before the fix `priority_scale_checked` kept only the scale and discarded everything after the
/// colon, so this filing was answered `priority_checked: true` while the queue read named it
/// unchecked. Restore that split and this reddens on the `priority_checked` assertion.
#[test]
fn an_empty_priority_value_is_answered_unchecked_and_named_by_the_queue_read() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let body = trace.path().join("body.md");
    std::fs::write(&body, conforming_body()).expect("the issue body is written");

    let listing = serde_json::to_string(&serde_json::json!([
        {
            "number": 105,
            "title": "filed with an empty priority value",
            "labels": [{"name": "status:ready"}, {"name": "domain:general"}, {"name": "priority:"}],
            "created_at": "2026-09-05T00:00:00Z",
        },
    ]))
    .expect("the queue listing serialises");
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "label create", "stdout": "", "status": 0},
        {"matches": "issue create", "stdout": "https://github.com/o/r/issues/105\n", "status": 0},
        {"matches": "issues?state=open", "stdout": serde_json::json!([serde_json::from_str::<serde_json::Value>(&listing).unwrap()]).to_string(), "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let filed: serde_json::Value = {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "create",
                "arguments": {
                    "identity": "a-probe",
                    "title": "filed with an empty priority value",
                    "body_file": body.to_string_lossy(),
                    "priority": "priority:",
                    "domain": "general",
                    "runtime": "opencode",
                    "run_id": "opencode-test-1a2b3c4d",
                    "state": "ready",
                },
            },
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
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the filed answer is JSON: {text}"))
    };
    assert_eq!(
        filed["ok"], true,
        "filing with an empty priority value was refused instead of recorded: {filed}"
    );
    assert_eq!(
        filed["priority_checked"], false,
        "an empty priority value was reported checked: {filed}"
    );

    let queue: serde_json::Value = {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_state",
                "arguments": {"state": "ready", "run_id": "opencode-test-1a2b3c4d"},
            },
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
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"))
    };
    let named: Vec<u64> = queue["unchecked"]
        .as_array()
        .unwrap_or_else(|| panic!("the queue read named nothing unchecked: {queue}"))
        .iter()
        .filter_map(|entry| entry["number"].as_u64())
        .collect();
    assert_eq!(
        named,
        vec![105],
        "the queue read did not name the item filed with an empty priority value: {queue}"
    );
}

/// A queue read with nothing unrankable carries no `unchecked` key at all.
///
/// The naming above is additive: an answer with no such items is byte-identical to the one from
/// before, so a caller diffing answers cannot mistake the new key for new drift.
#[test]
fn a_clean_queue_read_carries_no_unchecked_key() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    seed_store(home, &[(103, "ready")]);

    let listing = serde_json::to_string(&serde_json::json!([
        {
            "number": 103,
            "title": "rankable",
            "labels": [{"name": "status:ready"}, {"name": "domain:general"}, {"name": "priority:low"}],
            "created_at": "2026-09-03T00:00:00Z",
        },
    ]))
    .expect("the queue listing serialises");
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "issues?state=open", "stdout": serde_json::json!([serde_json::from_str::<serde_json::Value>(&listing).unwrap()]).to_string(), "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "list_state",
            "arguments": {"state": "ready", "run_id": "opencode-test-1a2b3c4d"},
        },
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
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let queue: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        queue["ok"], true,
        "the clean queue read did not answer ok: {queue}"
    );
    let general: Vec<u64> = queue["partitions"]["general"]
        .as_array()
        .unwrap_or_else(|| panic!("the clean queue read carried no `general` partition: {queue}"))
        .iter()
        .filter_map(|entry| entry["number"].as_u64())
        .collect();
    assert_eq!(
        general,
        vec![103],
        "the clean queue read did not carry item 103 in `general`: {queue}"
    );
    assert!(
        queue.get("unchecked").is_none(),
        "a clean queue read grew an `unchecked` key: {queue}"
    );
}

/// The queue read must ask the tracker for ascending creation order. The REST
/// issues endpoint answers newest-first by default, so a request with no
/// ordering term is cut to the newest `limit` and the transport then presents
/// that cut under an oldest-first heading. The argument is pinned on the wire,
/// not in source text: drop `sort=created&direction=asc` from `state_items` and
/// this reddens on the logged call.
#[test]
fn the_queue_read_asks_the_tracker_for_ascending_creation_order() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    seed_store(home, &[(101, "ready")]);
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("order.log");

    let listing = serde_json::json!([{
        "number": 101,
        "title": "queued",
        "labels": [{"name": "status:ready"}, {"name": "domain:general"}],
        "created_at": "2026-09-01T00:00:00Z",
    }])
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "issues?state=open", "stdout": serde_json::json!([serde_json::from_str::<serde_json::Value>(&listing).unwrap()]).to_string(), "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "list_state",
            "arguments": {"state": "ready", "run_id": "opencode-test-1a2b3c4d"},
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
    assert_eq!(response["result"]["isError"], false, "{response}");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.contains("issues?state=open&per_page=100&sort=created&direction=asc"),
        "the queue read did not ask for ascending creation order, so a cut answer holds the \
         newest issues under an oldest-first heading: {calls}"
    );
}

/// A truncated queue read returns the **oldest** matching issues, not the
/// newest. The fixture holds three issues and `limit` is two, and it answers
/// both orders the endpoint can give: oldest-first only when the request carries
/// the ordering term, newest-first otherwise. Drop the ordering term and the cut
/// keeps the newest two instead, moving the partition head from 301 to 302.
#[test]
fn a_truncated_queue_read_returns_the_oldest_issues() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    seed_store(home, &[(301, "ready"), (302, "ready"), (303, "ready")]);
    let trace = tempfile::tempdir().expect("a trace directory");
    let log = trace.path().join("truncated.log");

    let issue = |number: u64, created: &str| {
        serde_json::json!({
            "number": number,
            "title": format!("issue {number}"),
            "labels": [{"name": "status:ready"}, {"name": "domain:general"}],
            "created_at": created,
        })
    };
    let newest_first = serde_json::json!([
        issue(303, "2026-09-03T00:00:00Z"),
        issue(302, "2026-09-02T00:00:00Z"),
        issue(301, "2026-09-01T00:00:00Z"),
    ]);
    let oldest_first = serde_json::json!([
        issue(301, "2026-09-01T00:00:00Z"),
        issue(302, "2026-09-02T00:00:00Z"),
        issue(303, "2026-09-03T00:00:00Z"),
    ]);
    let page = |issues: &serde_json::Value| serde_json::json!([issues]).to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        {"matches": "direction=asc", "stdout": page(&oldest_first), "status": 0},
        {"matches": "issues?state=open", "stdout": page(&newest_first), "status": 0},
    ]))
    .expect("the fake tracker script serialises");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "list_state",
            "arguments": {"state": "ready", "run_id": "opencode-test-1a2b3c4d", "limit": 2},
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
    let queue: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        queue["at_limit"], true,
        "a cut answer did not say it was cut: {queue}"
    );
    let numbers: Vec<u64> = queue["partitions"]["general"]
        .as_array()
        .unwrap_or_else(|| panic!("the general partition is an array: {queue}"))
        .iter()
        .filter_map(|entry| entry["number"].as_u64())
        .collect();
    assert_eq!(
        numbers,
        vec![301, 302],
        "a truncated queue read returned the newest issues instead of the oldest: {queue}"
    );
}

/// The repository's schedule, end to end: `config set --repo` round-trips a
/// usable value through the repository override, and the gate denies the
/// protected operations when that row cannot be read as a schedule.
///
/// The gate half is clock-independent on purpose: an enabled value that does
/// not parse fails closed whatever time it is, so this measures the wiring —
/// the gate reading this checkout's own file — rather than the machine's
/// clock, which the unit tests pin with a fixed time instead.
#[test]
fn the_gate_denies_what_the_repository_schedule_forbids() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let pointer = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "state": "in-progress",
        "repo_dir": repo,
        "verified_at": now,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&pointer).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    let ask = |tool: &str, input: &str| {
        let (out, err, _) = run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer("in-progress"),
            &[
                "gate",
                tool,
                "--run-id",
                "claude-abcd1234",
                "--input",
                input,
            ],
            "",
        );
        format!("{out}{err}")
    };

    // What `config set --repo` writes, the gate reads: a usable value
    // round-trips through parsing, rendering, and the repository override.
    let (said, error, ok) = run_in(
        home,
        repo,
        &[
            "config",
            "set",
            "--repo",
            "Git operation window",
            "enabled 09:00-17:00",
        ],
        "",
    );
    assert!(ok, "the write was refused: {said}{error}");
    let repository_file = repo.join(".git").join("estigia").join("estigia.local.md");
    let written = std::fs::read_to_string(&repository_file).expect("the repository file");
    assert!(
        written.contains("enabled 09:00-17:00"),
        "the repository override does not carry the schedule: {written}"
    );
    let (listed, _, _) = run_in(home, repo, &["config", "list"], "");
    assert!(
        listed.contains("enabled 09:00-17:00"),
        "the read-back does not report the schedule: {listed}"
    );

    // A hand-edited row that does not parse fails closed for pushes and
    // commits alike, whatever time it is.
    let invalid = "| Setting | Value here | Skill default |\n|---|---|---|\n| Git operation window | enabled whenever | disabled |\n";
    std::fs::write(&repository_file, invalid).expect("an unreadable schedule");
    let push = ask("Bash", r#"{"command":"git push origin main"}"#);
    assert!(
        push.contains("git-operation-window-denied"),
        "an unparsable schedule waved a push through: {push}"
    );
    let commit = ask("Bash", r#"{"command":"git commit -m wip"}"#);
    assert!(
        commit.contains("git-operation-window-denied"),
        "an unparsable schedule waved a commit through: {commit}"
    );

    // Scoped to the protected operations: an ordinary edit still rides the
    // renewal window the pointer above established.
    let edit = ask("Edit", r#"{"file_path":"src/main.rs"}"#);
    assert!(
        edit.contains("allow"),
        "an unparsable schedule stopped an ordinary edit: {edit}"
    );

    // And switching the row off reopens the gate, which is what makes the
    // denials above about the schedule rather than the fixture.
    std::fs::write(
        &repository_file,
        invalid.replace("enabled whenever", "disabled"),
    )
    .expect("the schedule switched off");
    let reopened = ask("Bash", r#"{"command":"git push origin main"}"#);
    assert!(
        reopened.contains("allow"),
        "disabling the schedule did not reopen the gate: {reopened}"
    );
}

#[test]
fn the_edit_screen_refuses_without_a_terminal_before_reading_anything() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["config", "edit"], "");
    assert!(!ok, "config edit ran with no terminal");
    assert!(
        stderr.contains("no-terminal-for-the-screen"),
        "refused for the wrong reason: {stderr}"
    );
    assert!(
        stderr.contains("estigia config set"),
        "refused without naming what works: {stderr}"
    );
}
