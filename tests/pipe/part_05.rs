use super::*;

/// The compound handoff must make its evidence visible before it releases the
/// named epoch, and only final convergence may clear the local pointer.
#[test]
fn review_handoff_orders_evidence_release_and_pointer_clear() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
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
    let release =
        estigia::transport::claim::review_operation_id("review-handoff-release", &[&operation]);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let claim = marker(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
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
    let unassign = marker(
        "unassign",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("target-op", &target),
            ("op-id", &release),
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
    let mut after_release = after_handoff.clone();
    after_release.push(comment("IC_release", "2026-08-14T10:01:00Z", unassign));
    let issue = |state: &str, comments: &[serde_json::Value]| {
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": format!("status:{state}")}],
            "comments": comments,
        })
        .to_string()
    };
    let scripted = |final_state: &str| {
        let answers = serde_json::json!([
            {
                "matches": "view 12 --json body,labels,comments",
                "stdout": issue("review", &before),
                "status": 0,
            },
            {
                "matches": "view 12 --json labels",
                "nth": 1,
                "stdout": issue("review", &before),
                "status": 0,
            },
            {
                "matches": "view 12 --json labels",
                "nth": 2,
                "stdout": issue(final_state, &after_release),
                "status": 0,
            },
            {
                "matches": "view 12 --json assignees,labels",
                "stdout": issue("review", &after_release),
                "status": 0,
            },
            {
                "matches": "api user",
                "stdout": "{\"login\":\"fixture\"}",
                "status": 0,
            },
        ]);
        serde_json::to_string(&answers).expect("the fake tracker script serialises")
    };
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
    let put_pointer = || {
        forget_store(home);
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(estigia::identity::Identity::Number(12));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );
    };
    let invoke = |answers: &str| {
        let _ = std::fs::remove_file(&count);
        let _ = std::fs::remove_file(&log);
        let mut child = tracker_command(home, repo, bin, answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        child.wait_with_output().expect("the MCP server exits")
    };

    put_pointer();
    let output = invoke(&scripted("review"));
    assert!(output.status.success(), "the MCP process failed");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(response["result"]["isError"], false, "{response}");
    assert_eq!(
        estigia::harness::session::load(&runs, run_id).issue,
        None,
        "a converged handoff left the publishing run's pointer held"
    );
    let calls = std::fs::read_to_string(&log).expect("the fake tracker logged its calls");
    let calls: Vec<&str> = calls.lines().collect();
    assert_eq!(
        comment_writes(&calls.join("\n")),
        0,
        "the handoff and the release are recorded locally and post no comment: {calls:#?}"
    );
    assert!(
        local_entries(home)
            .iter()
            .any(|body| body.contains("issue-flow: review-handoff")),
        "the handoff marker was not recorded in the store"
    );
    assert!(
        local_entries(home)
            .iter()
            .any(|body| body.contains("issue-flow: unassign")),
        "the release marker was not recorded in the store"
    );

    put_pointer();
    let output = invoke(&scripted("in-progress"));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP refusal is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert_eq!(
        estigia::harness::session::load(&runs, run_id).issue,
        Some(estigia::identity::Identity::Number(12)),
        "a handoff that failed final state convergence cleared the run pointer"
    );
}

/// What the gate let through, named in the ledger by the real binary.
///
/// The record used to identify what Estigia stopped and not what it allowed: a
/// refusal is prefixed with its command, an allow was not. Measured under one
/// live claim, `git tag v1.0`, `gh release create v1.0` and `git push --force
/// origin main` left three identical lines — `tool=Bash verdict=allow
/// detail=issue #12 is held by claude-abcd1234`.
///
/// Through the binary, because the fix is one argument at a call site and
/// `note` already had its own test: passing `None` there left the whole suite
/// green, which is the shape this file exists to catch.
#[test]
fn the_ledger_names_the_boundary_the_gate_let_through() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "revision": 1,
        "state": "in-progress",
        "repo_dir": repo,
        "worktree": serde_json::Value::Null,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&run).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // The two irreversible steps that are **not** deliveries — publishing a
    // review target is how a run reaches review, so `in-progress` lets them
    // through. A `git tag` here would be refused as out-of-phase, which is the
    // first fixture this test had and the reason it is worth writing down.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Bash",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"git push --force origin main"}"#,
        ],
        "",
    );
    // And a write, whose subject is the file.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Write",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"file_path":"src/main.rs"}"#,
        ],
        "",
    );

    // A **different** boundary from the one above, and that is the whole of
    // why this catches anything: both doors sending `git push` made the
    // hook's line a duplicate of the gate's, so dropping the hook's subject
    // left the assertions passing on the other door's answer.
    //
    // And once through the agent's door, which is a different call site with
    // the same argument. Driving only `estigia gate` left the hook's own call
    // passing `None` with this test still green — two doors, one rule, and a
    // test that walked one of them.
    //
    // No session id, so the hook asks the checkout who holds it, which is the
    // same position a git hook is in.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "hook",
            "pre-tool-use",
            "--agent",
            "claude-code",
            "--dialect",
            "claude-code",
        ],
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "gh pr create --fill"},
            "cwd": repo,
        })
        .to_string(),
    );

    let ledger = decisions(home);
    let subjects: Vec<String> = ledger
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| entry.get("verdict").and_then(|v| v.as_str()) == Some("allow"))
        .map(|entry| {
            entry
                .get("subject")
                .and_then(|value| value.as_str())
                .unwrap_or("<unnamed>")
                .to_owned()
        })
        .collect();

    for wanted in ["git push", "gh pr create", "src/main.rs"] {
        assert!(
            subjects.iter().any(|subject| subject == wanted),
            "the ledger does not say {wanted} went through: {subjects:?} in {ledger}"
        );
    }
    // The floor: three steps, three distinguishable lines. Without it a record
    // that named the same thing every time would pass the assertions above.
    assert_eq!(
        subjects.len(),
        3,
        "expected one allowed line per step, got {subjects:?}"
    );
}

/// A gate whose entry this build cannot read stops saying `on`.
///
/// The other half of a fix that landed one round earlier in `doctor` and not
/// here. Two readers disagree by design: `is_gated` recognises the entry by
/// `hook pre-tool-use`, and `wiring::wire_in` requires the executable's own file
/// name to hold `estigia`. A copy renamed to anything else is plain to the first
/// and invisible to the second, so `registered` comes back empty and there is no
/// wire whose fault to report.
///
/// Measured: pointing `.claude/settings.json` at `…\ausente.exe` made `doctor`
/// answer `BROKEN gate` and `status` go on saying `gate on, tools on` about the
/// same machine. `status` is the one people read first — the argument this row
/// was built on — which makes it the half that mattered.
#[test]
fn a_gate_whose_entry_cannot_be_read_stops_saying_it_is_on() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    // The floor: a working install says `on`, or "it stops saying on" is a
    // sentence about a machine that never said it.
    let (before, _, _) = run(home.path(), &["status"], "");
    assert!(
        before.contains("gate on"),
        "a fresh install does not report its gate:\n{before}"
    );
    // And the other floor: the agents with no gate at all carry no fault.
    // A report that complains about everybody teaches people to ignore it,
    // which is the one thing a health report cannot survive.
    //
    // It guards the printed report and not the condition behind it: `status`
    // prints a fault only for an agent it lists as present, so dropping the
    // `is_gated` half of that condition changes nothing here. Said rather than
    // left looking measured.
    assert!(
        !before.contains("an entry is registered here"),
        "an agent with no gate registered was told its entry cannot be read:\n{before}"
    );

    let settings = home.path().join(".claude").join("settings.json");
    let text = std::fs::read_to_string(&settings).expect("the settings file was written");
    let renamed = text.replace("estigia.exe", "ausente.exe").replace(
        // The Unix spelling, so this measures the same thing on both platforms.
        //
        // Escaped, because the settings file is JSON and the command it holds is
        // a JSON string: what stands in the file after the path is `\"`, not
        // `"`. Written unescaped, this arm matched nothing on Linux and macOS —
        // the `.exe` arm above carried the test on Windows and the assertion
        // below caught the silence everywhere else, which is what it is for.
        "/estigia\\\"",
        "/ausente\\\"",
    );
    assert_ne!(renamed, text, "the fixture did not rename the executable");
    std::fs::write(&settings, &renamed).expect("their file");

    let (after, _, _) = run(home.path(), &["status"], "");
    assert!(
        !after.contains("gate on"),
        "a gate naming a binary this build cannot recognise is still reported on:\n{after}"
    );
    assert!(
        after.contains("REGISTERED BUT DEAD"),
        "the gate's state is no longer named:\n{after}"
    );
    assert!(
        after.contains("an entry is registered here"),
        "the report does not say what is wrong with it:\n{after}"
    );
    // And the command that rewrites the entry, which is what discharges it.
    assert!(
        after.contains("estigia setup claude-code"),
        "the fault is reported with no command that discharges it:\n{after}"
    );
}

/// A row this build refuses can still be set back to something it accepts.
///
/// The trap a tightening opens. A row whose value a build learned to refuse —
/// a `Worktree location` that is not absolute, here — then makes **every**
/// `config set` on that machine refuse the bad value, naming the file and no
/// command, including the one way out, which is setting the row back to `unset`.
///
/// The gate already keeps what parses for exactly this reason. The writer had no
/// such sentence, so a value the tool itself had accepted became a table nothing
/// could repair through the tool that wrote it.
#[test]
fn a_row_this_build_refuses_can_still_be_set_back_to_one_it_accepts() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // A row an older build would have written and this one refuses, put there
    // the way that build would have: straight into the table.
    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let text = std::fs::read_to_string(&contract).expect("the contract is installed");
    let broken = text.replace(
        "| Worktree location | unset |",
        "| Worktree location | ../relative |",
    );
    assert_ne!(broken, text, "the fixture did not reach the worktree row");
    std::fs::write(&contract, &broken).expect("their file");

    // The floor: this build really does refuse that value, or the test is about
    // a machine that was never in the state it describes.
    let (_, refused, ok) = run(
        home.path(),
        &["config", "set", "Worktree location", "../relative"],
        "",
    );
    assert!(
        !ok && refused.contains("worktree-location-not-absolute"),
        "the value this is about is not refused:\n{refused}"
    );

    // And the way out works.
    let (said, err, ok) = run(
        home.path(),
        &["config", "set", "Worktree location", "unset"],
        "",
    );
    assert!(
        ok,
        "a row this build refuses left the table unrepairable:\n{said}{err}"
    );
    let after = std::fs::read_to_string(&contract).expect("the contract survives");
    assert!(
        after.contains("| Worktree location | unset |"),
        "the row was reported set and the table does not hold it:\n{after}"
    );
    // And the rows beside it are still theirs — a bad row costs its own value
    // and nothing else.
    assert!(
        after.contains("| Tracker |"),
        "repairing one row cost the others:\n{after}"
    );

    // And the same way out with an agent named, which is a different reader:
    // `--agent` goes through `writable_config` and the line above through
    // `contract_of`. Both had the strict read and only one of them was walked.
    std::fs::write(&contract, &broken).expect("their file");
    let (said, err, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "--agent",
            "claude-code",
            // A row that **is** an agent's. `Worktree location` is written for
            // the machine and `--agent` refuses it by design, so asking for that
            // one here would measure the wrong refusal.
            "Delivery authorisation",
            "ask",
        ],
        "",
    );
    assert!(
        ok,
        "a bad worktree row stopped an agent's own row being written:\n{said}{err}"
    );
}

/// A closing keyword refuses **before** the branch reaches the remote.
///
/// It refused after: the push had landed and the pull request was open when
/// `publish_review` answered `closing-keyword-live`, whose outcome line reads
/// *"nothing was written"*. A run that believes it leaves an orphan branch and
/// an orphan pull request nobody knows exist, and the next call fails for an
/// unrelated reason — the operator debugging it starting from a false premise
/// supplied by the tool whose whole job is to be the honest one.
///
/// `Closes #n` in a commit message is readable locally, so the refusal costs
/// nothing where it belongs. The assertion is the acceptance criterion's own:
/// the remote is unchanged.
#[test]
fn a_closing_keyword_refuses_before_the_branch_reaches_the_remote() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-something";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success()),
        "the bare origin was not created"
    );
    let origin_url = origin.path().display().to_string();
    git(&["remote", "add", "origin", &origin_url]);
    git(&["branch", "-M", "main"]);
    // A tree with something in it: an empty one is not a delivery target, and
    // `clean_target` says so before anything about keywords is decided.
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file is written");
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
    assert!(
        git(&["push", "-q", "origin", "main"]),
        "the base did not push"
    );

    let body = trace.path().join("pr-body.md");

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    // Both shapes. The reused one is the field report's own — its pull request
    // already existed on the retry — and it is the one that stayed broken when
    // the scan merely moved above the push: `ensure_draft` and `edit_pr` write
    // to the live pull request before it.
    // Two shapes of pull-request list, and two sources of the keyword. The
    // reused shape is the field report's own — its pull request already existed
    // on the retry — and it is the one that stayed broken when the scan merely
    // moved above the push, because `ensure_draft` and `edit_pr` write to the
    // live pull request first. The source is a dimension because the body was
    // the half no test exercised, and the body is what that refusal published.
    for (listed, source, message, body_text) in [
        (
            "[]",
            "commit message",
            "make it better

Closes #12",
            "names nothing",
        ),
        (
            LISTED_PR,
            "commit message",
            "make it better

Closes #12",
            "names nothing",
        ),
        ("[]", "pr body", "make it better", "Closes #12"),
        (LISTED_PR, "pr body", "make it better", "Closes #12"),
        // The same sources naming another issue: the scan used to be handed
        // only the delivered issue, so `Closes #464` was invisible and GitHub
        // closed #464 on merge with nothing objecting.
        ("[]", "pr body", "make it better", "Closes #464"),
        (
            "[]",
            "commit message",
            "make it better

Closes #464",
            "names nothing",
        ),
    ] {
        // Rebuilt per case so exactly one source carries the keyword, which is
        // what makes each half of the scan load-bearing on its own.
        git(&["checkout", "-q", "-B", branch, "main"]);
        std::fs::write(
            repo.join("kept.txt"),
            format!(
                "{source}
"
            ),
        )
        .expect("the change");
        git(&["add", "kept.txt"]);
        assert!(
            git(&[
                "-c",
                "user.email=nobody@example.invalid",
                "-c",
                "user.name=nobody",
                "commit",
                "--quiet",
                "-m",
                message,
            ]),
            "the commit was not made"
        );
        std::fs::write(
            &body,
            format!(
                "{body_text}
"
            ),
        )
        .expect("the body is written");
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
                        "body": format!("Claimed.\n\n{claim}\n"),
                    }],
                }).to_string(),
                "status": 0,
            },
            { "matches": "pr list", "stdout": listed, "status": 0 },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        // The store is revision-guarded, so the second shape needs a clean pointer.
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
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "publish_review",
                "arguments": {
                    "issue": 12,
                    "run_id": run_id,
                    "branch": branch,
                    "base": "main",
                    "pr_title": "Something",
                    "pr_body_file": body.display().to_string(),
                    "worktree": repo.display().to_string(),
                }
            }
        })
        .to_string();

        let log = trace.path().join("calls.log");
        let count = trace.path().join("count.json");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
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

        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(
            text.contains("closing-keyword-live"),
            "the keyword was not what refused it: {text}"
        );
        assert!(
            text.contains("nothing was written"),
            "the pre-push refusal did not report the world untouched ({source}): {text}"
        );

        // The acceptance criterion, literally: the remote is unchanged.
        let refs = checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(refname)"]);
        assert!(
            !refs.contains(branch),
            "the branch reached the remote before the refusal: {refs}"
        );
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        for wrote in ["pr create", "pr edit", "pr ready"] {
            assert!(
                !calls.contains(wrote),
                "`{wrote}` reached the remote before the refusal ({listed}): {calls}"
            );
        }
    }
}

/// A refusal that arrives after the push says the write landed.
///
/// The transport had no way to say it: the outcome is derived from the exit
/// code, and a stop is `1` whether it refused before touching anything or after
/// pushing a branch and opening a pull request — so every refusal claimed
/// nothing had happened. The channel that fixes it was itself untested: renaming
/// the field in both producers left the whole suite green, which is this
/// repository's own definition of an untested fix.
///
/// Driven through the readback disagreement, which is a real post-push refusal:
/// the branch is on the remote and the pull request is open by the time the
/// head comes back wrong.
#[test]
fn a_refusal_after_the_push_reports_that_the_write_landed() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-published";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
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
        // No keyword: this run must reach the push.
        "a change that names no issue",
    ]);

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let (head_sha, base_sha) = (sha("HEAD"), sha("origin/main"));

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    // Both refusals that can arrive after the push, because each is its own
    // producer of the channel and only one of them was held: the readback
    // disagreeing, and a keyword arriving from the remote side of the pull
    // request — which the local precondition cannot see and does not claim to.
    // A third case whose honest answer is *unconfirmed* rather than *landed*:
    // the pull request is created and the list that follows does not show it.
    // That path is a `Failure::Read`, and the only thing keeping it from saying
    // `nothing was written` after a push is a `map_err` nothing tested.
    for (which, readback_head, remote_body, listed_after, landed) in [
        (
            "readback",
            "f".repeat(40),
            "names nothing",
            LISTED_SEVEN,
            true,
        ),
        ("remote keyword", head_sha, "Closes #12", LISTED_SEVEN, true),
        ("unconfirmed", "f".repeat(40), "names nothing", "[]", false),
    ] {
        let issue_comments = vec![serde_json::json!({
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed.\n\n{claim}\n"),
        })];
        record_cause_over_latest_receipt(home, 12);
        let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:in-progress"}],
                "comments": issue_comments,
            }).to_string(),
            "status": 0,
        },
        // Nothing open before; the freshly created one after. `published`
        // lists twice, and the second list is how it learns the number.
        { "matches": "pr list", "nth": 1, "stdout": "[]", "status": 0 },
        {
            "matches": "pr list",
            "stdout": listed_after,
            "status": 0,
        },
        { "matches": "pr create", "stdout": "https://github.com/o/r/pull/7\n", "status": 0 },
        // The readback, disagreeing: a head nobody published.
        {
            "matches": "headRefOid",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": readback_head,
                "baseRefOid": base_sha,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        // What the remote side of the pull request says, for the second case.
        // The local scan never sees this, which is why the later one stays.
        {
            "matches": "json body",
            "stdout": serde_json::json!({ "body": remote_body }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": "fix/12-published",
                        "baseRefName": "main",
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        // Revision-guarded: the second case needs a clean pointer.
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
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "publish_review",
                "arguments": {
                    "issue": 12,
                    "run_id": run_id,
                    "branch": branch,
                    "base": "main",
                    "pr_title": "Something",
                    "pr_body_file": body.display().to_string(),
                    "worktree": repo.display().to_string(),
                }
            }
        })
        .to_string();

        let log = trace.path().join("calls.log");
        let count = trace.path().join("count.json");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
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

        assert_eq!(response["result"]["isError"], true, "{response}");
        // The branch really did reach the remote, which is what makes the wording
        // load-bearing rather than decorative.
        let refs = checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(refname)"]);
        assert!(
            refs.contains(branch),
            "the {which} case never got as far as pushing, so it proves nothing"
        );
        // Every case: the sentence this issue exists to eliminate must not be
        // the answer once the branch is on the remote.
        assert!(
            !text.contains("nothing was written"),
            "the {which} refusal after the push claimed nothing was written: {text}"
        );
        if landed {
            assert!(
                text.contains("the write landed"),
                "the {which} refusal did not report the write as landed: {text}"
            );
        }
        assert!(
            !text.contains("nothing was written"),
            "the {which} refusal after the push claimed both: {text}"
        );
    }
}

/// A closing keyword naming **another** issue is refused after the push too.
///
/// The pre-push half of the scan is held by
/// [`a_closing_keyword_refuses_before_the_branch_reaches_the_remote`]. The
/// post-push half — `published`'s `names_another` arm, taken when a keyword
/// arrived from the remote side of a pull request this run did not write — was
/// held by nothing: the only pipe test that reaches the post-push
/// closing-keyword refusal
/// ([`a_refusal_after_the_push_reports_that_the_write_landed`]) drives a body
/// naming the delivered issue, so `names_this_issue` is true and the
/// `names_another` branch never runs. Forcing it to `false` left the suite
/// green.
///
/// The pull request is **reused**, listed before the push, so `published` reads
/// it back rather than creating one, and the keyword is on the remote body the
/// local scan never sees. The delivered issue is branch-linked, so the verdict's
/// own cause is `branch-link`; the refusal must still name
/// `closing-keyword-elsewhere`, the same cause the standalone
/// `check-closing-keywords` reports, because the remedy is read from the cause
/// and no edit removes a branch link.
#[test]
fn a_closing_keyword_naming_another_issue_refuses_after_the_push_on_a_reused_pr() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-reused-elsewhere";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
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
        // No keyword: the refusal must come from the remote body, after the push.
        "a change that names no issue",
    ]);

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "Refs #12.\n").expect("the body");

    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let (head_sha, base_sha) = (sha("HEAD"), sha("origin/main"));

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    // The reused pull request: listed before the push, so `published` reads it
    // back instead of creating one. Its remote body carries the keyword and its
    // branch is linked, so the assessment's own cause is `branch-link` while the
    // keyword names #464.
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": head_sha,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });
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
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        // `pr list` precedes `headRefOid` because the listing's own `--json`
        // names the field.
        { "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 },
        // `ensure_draft`'s read before the push and `read_back_pr`'s after it:
        // a draft at the pushed head.
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        // The remote body the local scan cannot see: a closing keyword naming
        // another issue.
        { "matches": "json body", "stdout": serde_json::json!({ "body": "Closes #464" }).to_string(), "status": 0 },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": branch,
                        "baseRefName": "main",
                    }],
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
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "publish_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }
        }
    })
    .to_string();

    let log = trace.path().join("calls.log");
    let count = trace.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
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

    // The branch really did reach the remote, which is what makes this the
    // post-push arm rather than the pre-push scan.
    let refs = checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(refname)"]);
    assert!(
        refs.contains(branch),
        "the branch never got as far as pushing, so this proves nothing: {refs}"
    );
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert!(
        text.contains("closing-keyword-live"),
        "the remote keyword was not what refused it: {text}"
    );
    assert!(
        text.contains("the write landed"),
        "the post-push refusal did not report the write as landed: {text}"
    );
    // The cause agrees with `check-closing-keywords`, whose own unit test pins
    // the same value: the remedy is read from the cause, and a `branch-link`
    // cannot be fixed by editing text.
    assert!(
        text.contains("closing-keyword-elsewhere"),
        "the post-push refusal disagreed with `check-closing-keywords` on why it fired: {text}"
    );
    assert!(
        !text.contains("branch-link"),
        "the post-push refusal carried the branch-link cause beside its keyword: {text}"
    );
}

/// A body this run cannot read refuses before it touches the remote.
///
/// The read was `if let Ok(..)`, which treats a file that is not there as a
/// body with no keyword in it — the sentence `keywords_in_commits` was rewritten
/// to refuse, three lines away in the same commit. Propagating it was claimed
/// and not held: reverting those three lines left the whole suite green, and the
/// reverted code reaches `ensure_draft`, runs `gh pr ready --undo` against the
/// live pull request, and *then* answers *nothing was written*.
///
/// The pull request here is deliberately not a draft, so `ensure_draft` has
/// something to do; that write is the one the revert performs and this refuses.
#[test]
fn an_unreadable_pr_body_refuses_before_the_remote_is_touched() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-unreadable";

    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
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
        // No keyword anywhere: the body being unreadable is the only fault.
        "a change that names no issue",
    ]);

    // Never written. That is the point.
    let body = trace.path().join("absent-body.md");

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
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        // Open, and **ready** — so `ensure_draft` has a write to perform. That
        // write is what the reverted read reaches before refusing.
        {
            "matches": "pr list",
            "stdout": serde_json::json!([{
                "number": 99,
                "url": "https://github.com/o/r/pull/99",
                "headRefOid": "0".repeat(40),
                "baseRefOid": "0".repeat(40),
                "isDraft": false,
            }]).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
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
        "params": {
            "name": "publish_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }
        }
    })
    .to_string();

    let log = trace.path().join("calls.log");
    let count = trace.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
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

    assert_eq!(response["result"]["isError"], true, "{response}");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    for wrote in ["pr ready", "pr edit", "pr create"] {
        assert!(
            !calls.contains(wrote),
            "`{wrote}` reached the remote before an unreadable body was refused: {calls}"
        );
    }
    let refs = checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(refname)"]);
    assert!(
        !refs.contains(branch),
        "the branch reached the remote before an unreadable body was refused"
    );
    // And the sentence is true, which is the whole subject of this issue.
    assert!(
        text.contains("nothing was written"),
        "the refusal did not say the world was untouched: {text}"
    );
}

/// `check-closing-keywords` refuses an unreadable commit range too.
///
/// The scan is one function now, and the function's own strictness is held —
/// but the *caller* was not. Restoring `assess_autoclose`'s old tolerant copy,
/// or dropping the `?`, left the whole suite green, so the sentence this change
/// put into the installed contract — *"both this scan and
/// `check-closing-keywords` refuse on it rather than continuing with an empty
/// list"* — could be made false with nothing objecting.
///
/// What tolerating it costs is not abstract: the assessment answers
/// `cause: "branch-link"` where the truth is `closing-keyword`, which points the
/// operator at the one cause no edit can undo.
#[test]
fn the_closing_keyword_check_refuses_a_range_it_cannot_read() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-unreadable-range";

    // A branch that exists, and a base whose remote-tracking ref does not, so
    // `origin/<base>..<branch>` is a range git cannot resolve.
    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    git(&["checkout", "-q", "-b", branch]);

    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": "fix/12-unreadable-range",
                        "baseRefName": "no-such-base",
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        // No keyword on the remote side, so the commit range is the only source
        // left and reading it is the only thing that can decide.
        {
            "matches": "json body",
            "stdout": "{\"body\":\"a body that names nothing\"}",
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
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
        "params": {
            "name": "check_closing_keywords",
            "arguments": { "issue": 12, "branch": branch, "base": "no-such-base" }
        }
    })
    .to_string();

    let log = trace.path().join("calls.log");
    let count = trace.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
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
        response["result"]["isError"], true,
        "an unreadable commit range was answered as an assessment: {text}"
    );
    assert!(
        !text.contains("branch-link"),
        "an unreadable range was reported as the cause no edit can undo: {text}"
    );
}

/// A closed issue still refuses a write inside the checkout it claimed.
///
/// This is the half that says what must **not** change, and nothing
/// held it: no fixture in this crate had ever put an issue in the closed state,
/// before or after the change. A judge measured what that costs by relaxing the
/// classification to `writes_outside_the_claim(..) || run.state == Some("done")`
/// — every routine repository write standing aside once the run carries the
/// state a delivery leaves it in, which is exactly what the issue puts out of
/// scope — and the whole suite stayed green.
///
/// So the tracker really answers `CLOSED` here, and the write really is inside
/// the claimed checkout.
#[test]
fn a_closed_issue_still_refuses_a_write_inside_the_checkout() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    // Derived the way the hook derives it. Stored under a guessed name the
    // pointer is one nothing loads, the gate stands aside for having sworn
    // nothing, and an empty answer looks exactly like the refusal working.
    let session = "closed-issue-fixture";
    let run_id = estigia::harness::session::run_id("claude", session);

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                // The state a delivery leaves behind, which is when the refusal
                // this test is about actually fires.
                "state": "CLOSED",
                "labels": [{"name": "status:done"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id);
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("done".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    // Inside the claimed checkout, and a file that does not exist yet — the
    // shape the classification is most likely to get wrong.
    let inside = repo.join("src").join("new.rs");

    let output = run_with_tracker(
        home,
        repo,
        bin,
        &answers,
        &["hook", "pre-tool-use"],
        &serde_json::json!({
            "session_id": session,
            "tool_name": "Write",
            "tool_input": { "file_path": inside.display().to_string() },
        })
        .to_string(),
    );
    let (stdout, _stderr, _ok) = output;
    assert!(
        stdout.contains("CLOSED") || stdout.contains("issue-not-open"),
        "a write inside the claimed checkout was not refused after the issue closed: {stdout}"
    );

    // And the half the issue is actually about, against the same closed tracker:
    // a write outside every covered checkout is answered without asking it. The
    // unit-level fixture for this never reaches the tracker at all — it stops at
    // `control-surface-not-installed` — so "with the issue in that state" was
    // carried only by the test's name until here.
    let (aside, _stderr, _ok) = run_with_tracker(
        home,
        repo,
        bin,
        &answers,
        &["hook", "pre-tool-use"],
        &serde_json::json!({
            "session_id": session,
            "tool_name": "Write",
            "tool_input": { "file_path": home.join("scratchpad").join("note.md").display().to_string() },
        })
        .to_string(),
    );
    // The empty object, not the absence of one word. A hook that stands aside
    // says nothing at all — the aside's code never reaches this surface — so
    // `{}` is the whole answer and anything else is some refusal. Asserting
    // `!contains("CLOSED")` would also be satisfied by
    // `control-surface-not-installed`, and since the stand-aside moved below
    // that refusal, an absence assertion would pass on the wrong one. The rig
    // installs the contract, so reaching `{}` here means the decision was taken
    // past it and without the tracker's answer.
    assert_eq!(
        aside.trim(),
        "{}",
        "a scratch note did not stand aside on the closed issue: {aside}"
    );
}

/// A rewritten branch republishes through the tool, and stops when the claim moves.
///
/// The two halves the unit tests on `push_to_origin` cannot reach, because both
/// are about what surrounds the push rather than about the push: that a run
/// never has to leave Estigia to land an amended branch, and that the renewal
/// standing immediately before the force-push actually refuses.
///
/// The second half is the one that decays quietly. Take that `verify_claim` out
/// and every unit test still passes — the lease is unchanged, the operation
/// still lands the rewritten branch, and the only thing lost is the answer to
/// *is this run still the holder* at the one boundary where the wrong answer
/// destroys history.
#[test]
fn a_republish_lands_a_rewritten_branch_and_stops_when_the_claim_moved() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-republish";

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
    let remote_head = || -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["rev-parse", &format!("refs/heads/{branch}")])
            .output()
            .expect("the origin is readable");
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

    // The first publication, as an ordinary push: this is the head the receipt
    // records and the head the lease will be taken against.
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
    let published_head = sha("HEAD");
    let base_sha = sha("origin/main");

    // The rewrite. After this the branch is no longer a descendant of what the
    // remote holds, which is exactly where the ordinary push gives up and a run
    // used to leave Estigia.
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "--amend",
        "-m",
        "the same change, amended",
    ]);
    let rewritten_head = sha("HEAD");
    assert_ne!(rewritten_head, published_head, "the amend rewrote nothing");
    assert_eq!(remote_head(), published_head, "the fixture did not publish");

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let claimed_by = |who: &str| {
        format!(
            "<!-- issue-flow: claim run-id={who} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->",
            "a".repeat(32)
        )
    };
    // The receipt the lease is taken from: a complete `published` marker naming
    // the head the remote actually holds.
    let receipt = format!(
        "<!-- issue-flow: published run-id={run_id} pr=7 head={published_head} base={base_sha} \
         digest={} epoch={} -->",
        "c".repeat(64),
        "a".repeat(32)
    );
    // `receipted` is what the third case turns off: an issue that has never been
    // published has no head to lease against, and the operation has to say so
    // rather than force over whatever the remote holds.
    let timeline = |holder: &str, receipted: bool| {
        let mut comments = vec![serde_json::json!({
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed.\n\n{}\n", claimed_by(holder)),
        })];
        if receipted {
            comments.push(serde_json::json!({
                "id": "IC_2",
                "createdAt": "2026-01-01T01:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!("Published draft for review.\n\n{receipt}\n"),
            }));
            comments.extend(republish_cause(
                run_id,
                &published_head,
                &base_sha,
                &"c".repeat(64),
                &"a".repeat(32),
            ));
        }
        serde_json::json!({
            "state": "OPEN",
            "labels": [{"name": "status:in-progress"}],
            "comments": comments,
        })
        .to_string()
    };
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": rewritten_head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });

    // `moved_in` is the store a second run writes to once the landing run has
    // edited the pull request: it claims the issue and releases this run's
    // epoch, so the renewal before the push — the last read of the store — finds
    // somebody else holding it.
    let second_run_writes = |store: &std::path::Path| {
        let foreign_claim = format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id=claude-99999999 runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->",
            "b".repeat(32)
        );
        let our_release = format!(
            "Released.\n\n<!-- issue-flow: unassign run-id={run_id} runtime=claude \
             op-id={} target-op={} -->",
            "d".repeat(32),
            "a".repeat(32)
        );
        serde_json::json!({
            "path": store.display().to_string(),
            "issue": 12,
            "at": "2098-01-01T00:00Z",
            "bodies": [foreign_claim, our_release],
        })
    };
    let answers = |moved_in: Option<&std::path::Path>, receipted: bool| {
        let mut script = vec![];
        if let Some(store) = moved_in {
            script.push(serde_json::json!({
                "matches": "pr edit",
                "store_append": second_run_writes(store),
                "stdout": "",
                "status": 0,
            }));
        }
        script.extend([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, receipted), "status": 0 }),
            serde_json::json!({ "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 }),
            serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
            serde_json::json!({ "matches": "json body", "stdout": serde_json::json!({"body": "names nothing"}).to_string(), "status": 0 }),
            serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
            serde_json::json!({
                "matches": "api graphql",
                "stdout": serde_json::json!({
                    "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                    } } } },
                }).to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        serde_json::to_string(&script).expect("the fake tracker script serialises")
    };

    let republish = |answers: &str, log: &std::path::Path| -> (bool, String) {
        forget_store(home);
        let runs = home.join(".estigia").join("runs");
        // Revision-guarded, so the second case needs a clean pointer.
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
            "params": { "name": "republish_review", "arguments": {
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
        let count = trace.path().join(format!(
            "{}.json",
            log.file_stem().unwrap_or_default().to_string_lossy()
        ));
        let mut child = tracker_command(home, repo, bin, answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", log)
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
        (
            response["result"]["isError"] == true,
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
    };

    // First half: the whole route, with the claim standing throughout.
    let landing = trace.path().join("landing.log");
    let (failed, text) = republish(&answers(None, true), &landing);
    assert!(!failed, "the republish did not land: {text}");
    let answered: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert!(
        answered["republished"] == true && text.contains(&published_head),
        "the answer does not say which route ran or what it leased against: {text}"
    );
    assert_eq!(
        remote_head(),
        rewritten_head,
        "the rewritten branch did not reach the remote"
    );

    // The renewal, found where it has to stand: after the last pull-request
    // listing — which is after the draft conversion and the body refresh — and
    // therefore between them and the push. Asserted on its own, because it is
    // the property, and because the denial below is only meaningful if there is
    // a call there to deny.
    let calls: Vec<String> = std::fs::read_to_string(&landing)
        .expect("the call log")
        .lines()
        .map(str::to_owned)
        .collect();
    let listed = calls
        .iter()
        .rposition(|line| line.contains("pr list"))
        .expect("the republish listed the open pull requests");
    assert!(
        calls[listed + 1..]
            .iter()
            .any(|line| line.contains("api repos/{owner}/{repo}/issues/12")),
        "no renewal stands between the pull-request listing and the force-push: {calls:#?}"
    );

    // Second half: everything identical, except that the renewal before the push
    // says somebody else holds it. Put the remote back first, so what this
    // measures is this run's push and not the one above.
    git(&[
        "push",
        "-q",
        "--force",
        "origin",
        &format!("{published_head}:{branch}"),
    ]);
    assert_eq!(remote_head(), published_head, "the remote was not restored");

    let denied = trace.path().join("denied.log");
    let (failed, text) = republish(
        &answers(Some(&home.join(".estigia").join("state.db")), true),
        &denied,
    );
    assert!(
        failed,
        "a republish went through after the claim moved to another run: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the force-push landed although the claim had moved: {text}"
    );
    // And it says which world it refused in. By this point `edit_pr` has replaced
    // the live pull request's title and body — the call log below proves it — so
    // `nothing was written` is false, and a run believing it leaves somebody
    // else's pull request re-described with nobody told to put it back. The
    // refusal reached here through `stop()`, whose envelope carries no `world`,
    // and the harness read the absence as *untouched*.
    assert!(
        std::fs::read_to_string(&denied)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr edit")),
        "the fixture never rewrote the pull request, so this proves nothing about the report"
    );
    assert!(
        !text.contains("nothing was written"),
        "the refusal reported an untouched world after rewriting the pull request: {text}"
    );
    assert!(
        text.contains("put it back"),
        "the refusal does not say what was left behind: {text}"
    );
    // And the renewal's **own** action survives beside it. The writes are
    // appended, never substituted: the one `Stop` that reaches this wrapper is
    // the renewal, and two of its five actions carry the only instruction the
    // run has — *claim it again* for a lapsed horizon, and acknowledge-and-drop
    // for a stand-down. The sibling wrapper destroyed its caller's action one
    // commit ago and turned the CI-exposure warning into an invitation; this is
    // the same shape, and it is held rather than assumed.
    assert!(
        text.contains("release only your own projection"),
        "the refusal replaced the renewal's own action instead of adding to it: {text}"
    );

    // Third: the refusal this operation exists to produce, driven the whole way
    // rather than at the level of `push_to_origin`. Somebody else moves the
    // remote after the receipt was recorded, so the lease refuses — and what is
    // asserted is what the *agent* is told, which no unit test can see. It was
    // the rarer path (the claim moving) that got a sentence naming the rewritten
    // pull request, while the designed outcome came back saying nothing about it.
    let other = trace.path().join("other");
    assert!(
        Command::new("git")
            .args(["clone", "--quiet", "--"])
            .arg(origin.path())
            .arg(&other)
            .output()
            .is_ok_and(|output| output.status.success())
    );
    let elsewhere = |arguments: &[&str]| checked_git(&other, arguments);
    elsewhere(&["checkout", "-q", branch]);
    std::fs::write(other.join("theirs.txt"), "theirs\n").expect("their change");
    elsewhere(&["add", "theirs.txt"]);
    elsewhere(&[
        "-c",
        "user.email=other@example.invalid",
        "-c",
        "user.name=Somebody else",
        "commit",
        "--quiet",
        "-m",
        "a commit this run never saw",
    ]);
    elsewhere(&["push", "-q", "origin", branch]);
    let theirs = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(&other)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git answers")
            .stdout,
    )
    .trim()
    .to_owned();
    assert_eq!(remote_head(), theirs, "the other run did not publish");

    let leased = trace.path().join("leased.log");
    let (failed, text) = republish(&answers(None, true), &leased);
    assert!(
        failed,
        "the lease let a republish over a commit the receipt never named: {text}"
    );
    assert_eq!(
        remote_head(),
        theirs,
        "the force-push destroyed a commit somebody else pushed: {text}"
    );
    assert!(
        !text.contains("nothing was written"),
        "the lease refusal reported an untouched world after rewriting the pull request: {text}"
    );
    // **Exactly** what happened, and nothing else. This fixture's pull request
    // is already draft — which is the normal state of a review target, since
    // `publish_review` drafts it and only `release_ci` makes it ready — so
    // `ensure_draft` writes nothing and only the edit did.
    //
    // The assertion used to be `contains("converted to draft")`, and it passed:
    // the refusal named a conversion nobody performed, and the test pinned the
    // falsehood so that fixing it would have gone red. An operator putting back
    // a draft conversion runs `gh pr ready`, which exposes the branch to CI —
    // the exact outcome `ensure_draft` exists to prevent.
    assert!(
        text.contains("had its title and body replaced"),
        "the lease refusal does not name the edit that did happen: {text}"
    );
    assert!(
        !text.contains("converted back to draft"),
        "the lease refusal claims a draft conversion over a pull request that was already draft: \
         {text}"
    );

    // Put the remote back for the last cases.
    git(&[
        "push",
        "-q",
        "--force",
        "origin",
        &format!("{published_head}:{branch}"),
    ]);

    // Fourth: the earliest refusal that can follow a write, and the one whose
    // report was left behind when the renewal's was fixed. The reused pull
    // request is READY, so `ensure_draft` un-readies it — and then its own
    // readback fails. A `gh pr view` that hits a rate limit is enough. The
    // refusal reached the agent as `Failure::Read`, which is exit 3 and reads
    // *the tracker could not be read; write nothing* — after a write.
    let ready_pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": rewritten_head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": false,
    });
    let unreadable = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // The readback `ensure_draft` makes, and nothing after it: this is the
        // first `pr view` of the operation, so failing it stops exactly there.
        { "matches": "headRefOid", "stdout": "", "status": 1 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let undone = trace.path().join("undone.log");
    let (failed, text) = republish(&unreadable, &undone);
    assert!(
        failed,
        "an unreadable draft readback was not a refusal: {text}"
    );
    assert!(
        std::fs::read_to_string(&undone)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr ready") && line.contains("--undo")),
        "the fixture never un-readied the pull request, so this proves nothing about the report"
    );
    assert!(
        !text.contains("nothing was written"),
        "the draft readback reported an untouched world after un-readying the PR: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );

    // Fourth-and-a-quarter: `--undo` returns zero and the pull request comes back
    // **still ready**. That is `draft-readback-failed`, and it is two things at
    // once — a report and a gate — neither of which anything measured.
    //
    // The gate: with its condition disabled the operation pushes the rewritten
    // head at a pull request CI is watching, which is the whole reason
    // `ensure_draft` runs before the push.
    //
    // The report: this refusal carries the only action that names that hazard.
    // A round of this change routed it through the shared wording, whose insert
    // **replaces** the action — so the warning was destroyed and what took its
    // place said the pull request *was converted back to draft* beside an
    // `observed` showing it was not, and invited an operator to put that back.
    // Putting it back is `gh pr ready`. The refusal would have talked somebody
    // into the exposure it exists to prevent.
    let still_ready = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // `--undo` is unmatched, so it "succeeds"; the readback disagrees.
        { "matches": "headRefOid", "stdout": ready_pr.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let stuck = trace.path().join("still-ready.log");
    let (failed, text) = republish(&still_ready, &stuck);
    assert!(
        failed,
        "a pull request that stayed ready was not refused: {text}"
    );
    assert!(
        std::fs::read_to_string(&stuck)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr ready") && line.contains("--undo")),
        "the fixture never attempted the un-ready, so this proves nothing"
    );
    assert!(
        text.contains("do not push") && text.contains("still ready"),
        "the refusal lost the action naming the CI exposure it exists to prevent: {text}"
    );
    for lie in ["converted back to draft", "put it back"] {
        assert!(
            !text.contains(lie),
            "the refusal said {lie:?} about a pull request it read back as still ready: {text}"
        );
    }
    assert!(
        !text.contains("nothing was written"),
        "the refusal reported an untouched world after `--undo` ran: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "a still-ready pull request did not stop the push: {text}"
    );

    // Fourth-and-a-half: the same window one call later. `ensure_draft` succeeds
    // — the pull request was ready and is now draft — and then `gh pr edit`
    // fails. That `?` was bare, so a transient failure there reported an
    // untouched world one statement after `un_readied` had been written to stop
    // exactly that. Removing the carrier left the whole suite green; this is
    // what measures it.
    let edit_fails = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // The draft readback succeeds, so `ensure_draft` returns having written.
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        { "matches": "pr edit", "stdout": "", "status": 1 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let edited = trace.path().join("edit-failed.log");
    let (failed, text) = republish(&edit_fails, &edited);
    assert!(failed, "a failing pr edit was not a refusal: {text}");
    let calls = std::fs::read_to_string(&edited).expect("the call log");
    assert!(
        calls.contains("pr ready") && calls.contains("--undo") && calls.contains("pr edit"),
        "the fixture did not un-ready and then attempt the edit: {calls}"
    );
    assert!(
        !text.contains("nothing was written"),
        "the failed edit reported an untouched world after un-readying the PR: {text}"
    );
    assert!(
        text.contains("the pull request was converted back to draft"),
        "the failed edit does not name the un-ready that did happen: {text}"
    );
    assert!(
        !text.contains("had its title"),
        "the failed edit claims an edit that did not land: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );

    // Fifth: an issue that has never been published has no head to lease
    // against. Forcing anyway would be the plain `--force` the issue rules out,
    // wearing a lease over whatever the remote happens to hold — so the refusal
    // is the guard, and until this case existed deleting it left the whole suite
    // green.
    let unpublished = trace.path().join("unpublished.log");
    let (failed, text) = republish(&answers(None, false), &unpublished);
    assert!(
        failed,
        "a republish went through with no recorded publication to lease against: {text}"
    );
    assert!(
        text.contains("published-transaction-missing"),
        "the missing receipt was not what refused it: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved although there was no receipt to lease against: {text}"
    );

    // Sixth, and the other direction of the same honesty: a refusal must not
    // claim a write that did not happen. There is a receipt, but **no open pull
    // request** — a previous one was closed — so `reused` is `None` and nothing
    // has been written to any pull request when the renewal denies the claim.
    //
    // Held because the flag was untested this way round. `rewrote_pr = true` as
    // a constant, and `= reused.is_some()`, both left the whole suite green, and
    // either would tell an operator that a pull request was converted to draft
    // and re-described when none was touched — sending them to undo a change
    // that does not exist. That is the mirror of the falsehood this operation
    // was fixed to stop telling, and it deserved the same measurement.
    let store_path = home.join(".estigia").join("state.db");
    let fresh_pr = || {
        serde_json::to_string(&serde_json::json!([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 }),
            // No open pull request: the reused path is never entered. The second
            // run writes to the store as the listing is answered, so the renewal
            // before the push is the one that finds it.
            serde_json::json!({ "matches": "pr list", "nth": 1, "stdout": "[]", "status": 0 }),
            serde_json::json!({
                "matches": "pr list", "nth": 2, "store_append": second_run_writes(&store_path),
                "stdout": "[]", "status": 0,
            }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]))
        .expect("the fake tracker script serialises")
    };
    let untouched = trace.path().join("untouched.log");
    let (failed, text) = republish(&fresh_pr(), &untouched);
    assert!(
        failed,
        "a republish went through after the claim moved to another run: {text}"
    );
    assert!(
        std::fs::read_to_string(&untouched)
            .expect("the call log")
            .lines()
            .all(|line| !line.contains("pr edit") && !line.contains("pr ready")),
        "the fixture rewrote a pull request, so this proves nothing about the untouched case"
    );
    // Every clause the report can emit, and the frame that carries them. Named
    // one by one rather than by a single phrase, because a single phrase is
    // exactly how this assertion died: the report's wording changed to
    // *"converted **back** to draft"*, the sibling case three was updated to
    // match and this one was not, so it forbade a substring the code could no
    // longer produce. A dead assertion is worse than none — measured, a build
    // that claimed both writes on every path, including this one where no pull
    // request exists at all, passed the entire suite.
    for lie in [
        "converted back to draft",
        "had its title",
        "put it back",
        "changed nothing",
    ] {
        assert!(
            !text.contains(lie),
            "the refusal said {lie:?} when no pull request was touched: {text}"
        );
    }
    // And the channel underneath the words. `world: committed` is what makes the
    // harness report a landed write, so a refusal that reaches here must not
    // carry it however the sentence is phrased.
    assert!(
        text.contains("nothing was written"),
        "the refusal did not report the untouched world it is actually in: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal with no pull request in play: {text}"
    );

    // Seventh: the pull request **was** ready, so `ensure_draft` un-readies it
    // and the edit follows. Now both writes happened, and the refusal has to
    // name both — which is the only case where the sentence the previous round
    // shipped was accurate. Holding the two cases together is the point: one
    // fixture proves the draft clause appears when it should, the other proves
    // it stays away when it should not, and a single boolean cannot satisfy
    // both.
    let was_ready = || {
        let ready = serde_json::json!({
            "number": 7, "url": "https://github.com/o/r/pull/7",
            "headRefOid": rewritten_head, "baseRefOid": base_sha,
            "state": "OPEN", "isDraft": false,
        });
        serde_json::to_string(&serde_json::json!([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 }),
            serde_json::json!({
                "matches": "pr list", "nth": 1,
                "stdout": serde_json::json!([ready]).to_string(), "status": 0,
            }),
            serde_json::json!({
                "matches": "pr list", "nth": 2, "store_append": second_run_writes(&store_path),
                "stdout": serde_json::json!([ready]).to_string(), "status": 0,
            }),
            // The readback `ensure_draft` makes after `--undo`: now a draft.
            serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]))
        .expect("the fake tracker script serialises")
    };
    let readied = trace.path().join("readied.log");
    let (failed, text) = republish(&was_ready(), &readied);
    assert!(
        failed,
        "the claim moved and the republish went through: {text}"
    );
    let calls = std::fs::read_to_string(&readied).expect("the call log");
    assert!(
        calls.contains("pr ready") && calls.contains("--undo") && calls.contains("pr edit"),
        "the fixture did not un-ready and edit, so this proves nothing about naming both: {calls}"
    );
    // The **joined** sentence, not two fragments of it. Two fragments are how
    // the ungrammatical frame survived: *"the pull request was had its title and
    // body replaced"* contains both of them. The unit test beside `describe()`
    // covers every combination; this is the one that proves the frame the MCP
    // server actually hands an agent is the assembled one.
    assert!(
        text.contains(
            "the pull request was converted back to draft and had its title and body replaced \
             before this refusal"
        ),
        "the refusal does not read as one sentence naming both writes: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );
}

/// After a rebase, `parent-head..head` also carries everything the
/// base gained in between, and every shipped surface named that range
/// unconditionally as the delta a re-review reads. This drives a real rebase
/// through `republish_review` and checks that the publication now records the
/// dishonesty instead of staying silent about it.
///
/// Composed from three already-proven pieces rather than invented: the rebase
/// construction mirrors `src/transport/tier/tests.rs`'s
/// `a_rebased_repairs_own_range_is_not_recoverable_so_it_answers_the_whole_change_instead`
/// (detach at the advanced base, recreate the branch's own commit as a new
/// object, keep the ancestry self-check so a fixture that silently failed to
/// rebase cannot pass this test while measuring nothing), the MCP rig is
/// `a_republish_lands_a_rewritten_branch_and_stops_when_the_claim_moved`'s, and
/// the fabricated lineage marker is `moved_base_round`'s
/// shape for a base that moved between the parent publication and this one.
#[test]
fn a_rebased_republish_records_history_rewritten_instead_of_the_parent_range() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-rangefix4a2b1c3d";
    let issue = 267u64;
    let branch = "fix/267-rebase-range";
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

    // The parent publication's own branch: one commit, actually pushed to
    // origin, so the force-with-lease below leases against real remote bytes.
    g(&["checkout", "--quiet", "-b", branch, "main"]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the original candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let parent_head = sha("HEAD");
    g(&["push", "--quiet", "-u", "origin", branch]);

    // `main` advances with passive prose, on a path the branch never touches,
    // so replaying the branch's own edits onto it is conflict-free.
    g(&["checkout", "--quiet", "main"]);
    std::fs::create_dir_all(repo.join("docs")).expect("the docs directory");
    std::fs::write(repo.join("docs").join("other.md"), "Ordinary prose.\n")
        .expect("main's own advance");
    g(&["add", "docs/other.md"]);
    commit("main advances on a path the branch never touches");
    g(&["push", "--quiet", "origin", "main"]);
    let advanced_base_sha = sha("main");

    // The rebase itself: the branch's own commit recreated as a new object on
    // top of the advanced base, plus the repair's own edit on top of that —
    // exactly what a real rebase followed by a real correction produces, and
    // why `parent_head` can never be this head's ancestor again.
    g(&["checkout", "--quiet", "--detach", &advanced_base_sha]);
    std::fs::write(repo.join("feature.txt"), "a\nb\nc\nd\n").expect("the replayed candidate");
    g(&["add", "feature.txt"]);
    commit("the original candidate");
    let mut feature = std::fs::read_to_string(repo.join("feature.txt")).expect("feature.txt");
    feature.push_str("extra0\n");
    std::fs::write(repo.join("feature.txt"), feature).expect("the repair");
    g(&["add", "feature.txt"]);
    commit("the repair");
    g(&["checkout", "--quiet", "-B", branch]);
    let rebased_head = sha("HEAD");

    assert_ne!(
        rebased_head, parent_head,
        "the rebase produced no new commit"
    );
    let is_ancestor = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge-base", "--is-ancestor", &parent_head, &rebased_head])
        .status()
        .expect("git runs")
        .success();
    assert!(
        !is_ancestor,
        "the fixture did not rebase away from parent_head; the test proves nothing"
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
    let pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": rebased_head, "baseRefOid": advanced_base_sha,
        "state": "OPEN", "isDraft": true,
    });

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, format!("Refs #{issue}.\n")).expect("the pull request body");

    let script = vec![
        serde_json::json!({ "matches": "issue view", "stdout": timeline.to_string(), "status": 0 }),
        serde_json::json!({ "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 }),
        serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
        serde_json::json!({ "matches": "json body", "stdout": serde_json::json!({"body": format!("Refs #{issue}.\n")}).to_string(), "status": 0 }),
        serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
        serde_json::json!({
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
    ];
    let answers = serde_json::to_string(&script).expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "republish_review", "arguments": {
            "issue": issue,
            "run_id": run_id,
            "branch": branch,
            "base": "main",
            "pr_title": "Something",
            "pr_body_file": body.display().to_string(),
            "worktree": repo.display().to_string(),
        }}
    })
    .to_string();

    let log = trace.path().join("republish.log");
    let temp = trace.path().join("republish-temp");
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
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let failed = response["result"]["isError"] == true;
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!failed, "the rebased republish did not land: {text}");
    // The answer itself is no longer parsed here: what it carries beyond the
    // identity fields moved to `review_transaction`, and what this row
    // proves is checked on the staged marker below.

    assert_eq!(
        remote_head(),
        rebased_head,
        "the rewritten branch did not reach the remote: {text}"
    );
    // `delta_range`/`delta_range_reason` moved to `review_transaction` — see
    // `docs/review-cost.md` — so what a rebased republish recorded is checked
    // on the staged marker below, the same evidence `review_transaction`
    // itself reads back.
    let receipt = staged_receipt(home, issue);
    assert!(
        !receipt.contains("delta-range="),
        "the staged marker recorded an honest range for a rewritten history: {receipt}"
    );
    assert!(
        receipt.contains("delta-range-reason=history-rewritten"),
        "the staged marker did not record why the parent range is not honest: {receipt}"
    );
}
