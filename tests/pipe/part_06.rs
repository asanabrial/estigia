use super::*;

/// The ordinary publication adjudicates its claim, proved by what reaches the remote.
///
/// The test whose **absence** was the finding, and it is about `publish_review`
/// rather than about anything this change added. Nothing anywhere asserted that
/// the ordinary publication refuses when the claim has moved: the only thing
/// holding that property was `honesty.rs` counting the string `verify_claim(` in
/// a function body, and a count cannot see a conditional.
///
/// Measured, five ways, each of which left the whole suite green at some point
/// in this change's history and is red now:
///
/// - the entry verification deleted;
/// - its result discarded with `let _ = …`, which keeps the token a count reads;
/// - the call moved behind `if matches!(push, Push::Leased { .. })`;
/// - the call moved behind `if matches!(push, Push::FastForward)` — the *other*
///   value of the same discriminant, which the first four of these tests missed
///   because they drove only the route that value keeps;
/// - the call moved below the draft conversion, so it still precedes the push.
///
/// The last two are why this runs both entry points and why the fixture has an
/// open pull request. With an empty `pr list` the assertions that no `pr edit`
/// or `pr ready` ran are **vacuous** — `ensure_draft` and `edit_pr` live behind
/// `reused`, so they could not run whatever the mutation. A verification moved
/// below them still stops the push, so the remote alone cannot see it.
///
/// The remote is the assertion that a mutated build cannot fake; the pull
/// request is the one that catches a refusal arriving too late. The message
/// assertion fires first of the three, and is there to prove the refusal is
/// about the claim rather than about something incidental.
#[test]
fn a_publication_refuses_at_entry_when_the_claim_has_moved() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-entry";

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

    // The branch is committed and **not** pushed. That is what makes the origin
    // able to answer the question: if the entry verification is gone, the
    // publication runs to completion and this ref appears there.
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

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    // Somebody else holds it, from the very first read — and the timeline also
    // carries a complete receipt, so `republish_review` gets past its own
    // receipt check and reaches the same entry verification. Without it that
    // route would refuse for an unrelated reason and prove nothing.
    let published_marker = format!(
        "<!-- issue-flow: published run-id=claude-99999999 pr=7 head={} base={} digest={} \
         epoch={} -->",
        "d".repeat(40),
        "b".repeat(40),
        "c".repeat(64),
        "a".repeat(32)
    );
    // **One open draft pull request**, not an empty list. An empty list made
    // `reused` `None`, so `edit_pr` was structurally unreachable and the
    // assertion below that no `pr edit` ran could not fail. With a pull request
    // present it does, and it is what catches an entry verification moved
    // *below* the draft conversion — the push assertion alone cannot see that,
    // because the renewal further down still stops the push.
    //
    // Exactly one of the three assertions bites, and saying "they" would be the
    // same overclaim this round is correcting. `pr create` cannot fire with
    // `reused` set, and `pr ready` cannot fire because this pull request is
    // already draft. That last one leaves a real gap: a verification placed
    // *between* `ensure_draft` and `edit_pr` would, against a **ready** pull
    // request, un-ready somebody else's before refusing, and this fixture
    // cannot see it. Named rather than fixed, because a second fixture for a
    // window one statement wide is more machinery than the risk earns.
    let open_pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": "e".repeat(40), "baseRefOid": "b".repeat(40),
        "state": "OPEN", "isDraft": true,
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
                    "body": format!(
                        "Claimed.\n\n<!-- issue-flow: claim run-id=claude-99999999 runtime=claude \
                         horizon=2099-01-01T00:00Z op-id={} -->\n{published_marker}\n",
                        "a".repeat(32)
                    ),
                }],
            }).to_string(),
            "status": 0,
        },
        { "matches": "pr list", "stdout": serde_json::json!([open_pr]).to_string(), "status": 0 },
        { "matches": "headRefOid", "stdout": open_pr.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // **Both** entry points. The first version drove `publish_review` only, and
    // the entry verification stayed removable from `republish_review` with the
    // whole suite green — the same defect one route further along, which is how
    // three of these rounds have gone. They share a body today; nothing says
    // they always will, and a test that covers one of two routes is a test that
    // stops holding the moment somebody splits them.
    for tool in ["publish_review", "republish_review"] {
        let runs = home.join(".estigia").join("runs");
        // Revision-guarded: the second pass needs a clean pointer.
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
            "params": { "name": tool, "arguments": {
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

        let log = trace.path().join(format!("entry-{tool}.log"));
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
            response["result"]["isError"], true,
            "{tool} went through under a claim another run holds: {response}"
        );
        assert!(
            text.contains("not-current-live-holder"),
            "something other than the claim refused {tool}, so this proves nothing about \
             adjudication: {text}"
        );

        // The remote, which a mutated build cannot fake.
        let refs = checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(refname)"]);
        assert!(
            !refs.contains(branch),
            "{tool} put the branch on the remote under a claim this run does not hold: {refs}"
        );
        // And the pull request, which the push assertion cannot see. A run whose
        // entry verification sits *below* the draft conversion still stops before
        // the push — and has already un-readied and re-described somebody else's
        // pull request by then.
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        for wrote in ["pr create", "pr edit", "pr ready"] {
            assert!(
                !calls.contains(wrote),
                "`{wrote}` ran under a claim this run does not hold, via {tool}: {calls}"
            );
        }
    }
}

/// A per-call working directory decides which holder answers for the call.
///
/// OpenCode's plugin launches the gate from the **project** directory, because
/// its plugin context carries a project and no session identity. When two runs
/// each hold an isolated worktree inside one base checkout, that directory is
/// the base — which both runs cover at equal depth, so `holders_of` returned
/// both and the call was refused `several-runs-hold-this-checkout`.
///
/// The refusal was correct about the directory it was given. What the call
/// actually carries is `args.workdir`, the directory the command will run in,
/// and nothing read it. Measured on 2026-08-16 with two live holders of this
/// repository: a `git commit` explicitly targeting one worktree was refused for
/// ambiguity between two runs that were not ambiguous at all, and the refusal
/// advised releasing one of them — which is the isolation both were using.
///
/// Driven as a process against the real binary, because the defect is in what
/// reaches `gate_context` from the outside. A unit test on `payload_cwd` cannot
/// see the base-checkout fallback that makes the wrong answer look right.
#[test]
fn a_per_call_working_directory_selects_the_holder_that_owns_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // One base checkout, two isolated worktrees inside it. Inside, because that
    // is the arrangement `holders_of` documents as the one that goes ambiguous:
    // work in A's worktree is covered exactly by A and from the base by B.
    let base = tempfile::tempdir().expect("a checkout");
    let worktree_a = base.path().join("wt-a");
    let worktree_b = base.path().join("wt-b");
    std::fs::create_dir_all(&worktree_a).expect("worktree a");
    std::fs::create_dir_all(&worktree_b).expect("worktree b");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    for (run_id, issue, worktree) in [
        ("claude-aaaa1111", 12u64, &worktree_a),
        ("opencode-bbbb2222", 34u64, &worktree_b),
    ] {
        let pointer = serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "state": "in-progress",
            "repo_dir": base.path(),
            "worktree": worktree,
            "verified_at": serde_json::Value::Null,
        });
        std::fs::write(
            runs.join(format!("{run_id}.json")),
            serde_json::to_string(&pointer).expect("a pointer serialises"),
        )
        .expect("the pointer writes");
    }

    // Both commands the report reproduced it with. The second matters on its
    // own: it is Estigia's own binary being refused, which is what showed the
    // false ambiguity lands before any delivery or Git-head validation.
    let ask_running = |command: &str, workdir: Option<&str>| {
        clear_decisions(home.path());
        let payload = match workdir {
            Some(named) => serde_json::json!({ "command": command, "workdir": named }),
            None => serde_json::json!({ "command": command }),
        };
        let payload = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, ok) = run_in(
            home.path(),
            base.path(),
            &["gate", "bash", "--input", &payload],
            "",
        );
        let wrote = decisions(home.path());
        (format!("{out}{error}"), ok, wrote)
    };

    for command in [
        "git commit -m fixture",
        "estigia config set \"Change size\" 400",
    ] {
        let ask = |workdir: Option<&str>| ask_running(command, workdir);

        // The floor. A call whose working directory really is the shared base is
        // genuinely ambiguous, and that refusal is not what this change touches:
        // if it stops firing, the rows below prove nothing. It is also the
        // project-context fallback, which a call carrying no per-call working
        // directory has to keep.
        let (said, ok, _) = ask(None);
        assert!(
            !ok && said.contains("several-runs-hold-this-checkout"),
            "the genuine ambiguity at the base checkout stopped being refused \
             for `{command}`, so nothing below is a measurement: {said}"
        );

        // Each worktree in turn, by absolute path.
        for (named, mine, theirs) in [
            (&worktree_a, "claude-aaaa1111", "opencode-bbbb2222"),
            (&worktree_b, "opencode-bbbb2222", "claude-aaaa1111"),
        ] {
            let (said, _, wrote) = ask(Some(&named.display().to_string()));
            assert!(
                !said.contains("several-runs-hold-this-checkout"),
                "`{command}` naming {} was refused as ambiguous between two runs \
                 that hold different worktrees: {said}",
                named.display()
            );
            // Not merely "some holder answered" — the right one. The ledger is
            // where the attribution is durable, and an answer that named the
            // other run would be the same defect wearing the opposite result.
            assert!(
                wrote.contains(mine),
                "the decision was not attributed to the run that owns {}: {wrote}",
                named.display()
            );
            assert!(
                !wrote.contains(theirs),
                "the decision named the run that does not own {}: {wrote}",
                named.display()
            );
        }

        // Relative, resolved the way OpenCode resolves it: against the directory
        // the call runs in, which is the one the plugin launched this process
        // in.
        let (said, _, wrote) = ask(Some("wt-a"));
        assert!(
            !said.contains("several-runs-hold-this-checkout"),
            "a relative working directory was not resolved for `{command}`, so \
             it selected nobody: {said}"
        );
        assert!(
            wrote.contains("claude-aaaa1111") && !wrote.contains("opencode-bbbb2222"),
            "a relative working directory reached the wrong holder: {wrote}"
        );

        // A directory that is not there. It resolves under the launch directory
        // like any other relative value, so the ordinary ambiguity refusal is
        // what it gets — the answer the same call gets carrying no key at all.
        //
        // The clamp is what holds this now, not the resolution: an unresolvable
        // value is placed before it is compared, and one that lands outside is
        // dropped in favour of the launch directory *or* admitted because a
        // pointer covers it — see `held_by_a_pointer`. Both belts are kept,
        // and the resolution is measured by the unit test rather than by
        // this row.
        let (said, ok, _) = ask(Some("no-such-worktree"));
        assert!(
            !ok,
            "a working directory that names nothing was allowed through: {said}"
        );
        assert!(
            said.contains("several-runs-hold-this-checkout"),
            "a working directory that names nothing escaped the checkout it was \
             resolved against, so no run held the call: {said}"
        );

        // The rows that decide whether reading this key at all is safe.
        //
        // `cwd` is set by the host. `workdir` is a **tool argument**, which on
        // every runtime here means it is written by the model, and a value that
        // names a real directory outside the checkout is not a value that names
        // nothing: it resolves, it is covered by no run, and the gate answers
        // `outside` and exits zero. The command still runs wherever it was
        // going to run. That is a payload steering a write out of the gate,
        // which is strictly worse than the false ambiguity being fixed here and
        // is the failure this crate exists to refuse — a widened gate that looks
        // exactly like working correctly.
        //
        // Every spelling below reaches the same place by a different road, and
        // each one was `several-runs-hold-this-checkout` before this alias was
        // read at all. Nothing may be allowed through.
        let parent = base
            .path()
            .parent()
            .expect("the fixture has somewhere above it")
            .display()
            .to_string();
        let climbed = worktree_a.join("..").join("..").display().to_string();
        // And the road that only opens when the path cannot be **opened**.
        // Comparison resolves both sides and falls back to the spelling when
        // resolution fails, so `..` past an existing component was never
        // cancelled: `wt-a/../../nope` still started with the launch directory,
        // was called inside, and was attributed to the holder of the component
        // it climbed *through*. Measured: not merely let past — allowed, exit
        // zero, under a claim the call had nothing to do with. A run holding one
        // worktree could borrow the other's authority by writing one `..`.
        let through_a = worktree_a.join("..").join("..").join("nope");
        let through_a = through_a.display().to_string();
        let deeper = worktree_a
            .join("..")
            .join("..")
            .join("..")
            .join("nope")
            .display()
            .to_string();
        for escape in [
            "..",
            parent.as_str(),
            climbed.as_str(),
            if cfg!(windows) { "C:\\Windows" } else { "/etc" },
            "wt-a/../../nope",
            through_a.as_str(),
            deeper.as_str(),
        ] {
            let (said, ok, _) = ask(Some(escape));
            assert!(
                !ok,
                "`{command}` steered the gate out of the claim with \
                 `workdir` = {escape:?}, and the write went through unjudged: {said}"
            );
            assert!(
                said.contains("several-runs-hold-this-checkout"),
                "`{command}` with `workdir` = {escape:?} was answered something \
                 other than the refusal the same call gets without the key, so \
                 the payload moved the decision: {said}"
            );
        }
    }

    // The same road, through a tool that carries no working directory of its
    // own. The key was once read for every gated tool rather than only for the
    // one whose host sends it, so a payload that merely included it steered any
    // of them — measured, on `write` and `edit`, before the read was narrowed to
    // Bash. Two things hold it shut now and this row is the floor under both:
    // the tool restriction, and the clamp behind it. Kept out of the loop above
    // because the argument shape differs.
    clear_decisions(home.path());
    let payload = serde_json::json!({
        "file_path": worktree_a.join("f.txt"),
        "workdir": "..",
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !said.contains("several-runs-hold-this-checkout"),
        "a write naming a file inside the claim was refused as ambiguous: {said}"
    );
    // The write is attributed to the run that owns the worktree its target
    // lands in — the fix — and the stray `workdir` in the
    // payload is Bash's key, read for Bash and ignored here.
    let write_ledger = decisions(home.path());
    assert!(
        write_ledger.contains("claude-aaaa1111") && !write_ledger.contains("opencode-bbbb2222"),
        "the write was not attributed to the run that owns the worktree its \
         target lands in: {write_ledger}"
    );

    // Genuine ambiguity survives for a write too, not only for Bash: a target
    // that lands at the shared base itself — neither worktree, both runs
    // cover it at equal depth — is still refused, the same answer the
    // identical call gets with no target at all.
    clear_decisions(home.path());
    let payload = serde_json::json!({ "file_path": base.path().join("f.txt") });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !ok && said.contains("several-runs-hold-this-checkout"),
        "a write target at the shared base was not refused as ambiguous between \
         the two runs that hold worktrees inside it: {said}"
    );

    // A cwd that names nowhere a claim covers must not answer
    // outside with exit zero. The three rows measured on 2026-08-16 against
    // the public verb, all `bash` — the tool whose only evidence *is* `cwd`
    // (via `workdir`), so discarding an uncovered `cwd` leaves it with none
    // and the answer has to match the identical payload carrying no `cwd` at
    // all: the genuine ambiguity at the shared base.
    let elsewhere = if cfg!(windows) { "C:\\Windows" } else { "/etc" };
    let through_a = worktree_a.join("..").join("..").join("nope");
    let through_a = through_a.display().to_string();
    for payload in [
        serde_json::json!({ "command": "git commit", "cwd": elsewhere }),
        serde_json::json!({ "tool_input": { "command": "git commit", "cwd": ".." } }),
        serde_json::json!({ "command": "git commit", "cwd": through_a }),
    ] {
        clear_decisions(home.path());
        let body = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, ok) = run_in(
            home.path(),
            base.path(),
            &["gate", "bash", "--input", &body],
            "",
        );
        let said = format!("{out}{error}");
        assert!(
            !ok && said.contains("several-runs-hold-this-checkout"),
            "a `bash` payload whose cwd names nowhere a claim covers was answered differently from the same call carrying no cwd: {said}"
        );
    }

    // A `write` is a different tool with different evidence: its `cwd` is
    // Bash's key and carries nothing about where the write lands, so an
    // uncovered `cwd` on a write payload must not touch the answer at all —
    // the write's own `file_path` still names a covered worktree, and that is
    // what decides. Discarding the bogus `cwd` and falling through to that
    // evidence is the fix, not a case of "no evidence
    // survives", so this is *not* the ambiguous-fallback group above: it is
    // driven separately, and checked the way the write attribution rows
    // above it are checked — by the ledger, not by the exit code, because a
    // single holder now genuinely owns the call and whatever that holder's
    // own tracker read answers on this machine is a different question from
    // the one this row measures.
    clear_decisions(home.path());
    let payload = serde_json::json!({
        "file_path": worktree_a.join("f.txt"),
        "cwd": elsewhere,
    });
    let body = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &body],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !said.contains("several-runs-hold-this-checkout"),
        "a `write` payload whose cwd names nowhere a claim covers was refused as \
         ambiguous instead of falling through to its own target: {said}"
    );
    let wrote = decisions(home.path());
    assert!(
        wrote.contains("claude-aaaa1111") && !wrote.contains("opencode-bbbb2222"),
        "a `write` payload whose cwd names nowhere a claim covers was not attributed \
         to the run whose worktree its target lands in: {wrote}"
    );

    // And the other door of the same branch, which nothing crossed before this
    // change restructured the line it lives on. A host's `cwd` is authoritative
    // and has to keep reaching the decision: discarding it entirely — always
    // adjudicating the process directory — left the whole suite green, while the
    // comment above that line cites a measured defect where the same call was
    // judged against two different repositories depending on which door it came
    // through. Sent as the hook nests it, because that nesting is why the door
    // exists at all.
    clear_decisions(home.path());
    let payload = serde_json::json!({
        "tool_input": { "cwd": worktree_b, "file_path": worktree_b.join("f.txt") },
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let wrote = decisions(home.path());
    assert!(
        !said.contains("several-runs-hold-this-checkout"),
        "a checkout the host named was discarded, so the call fell back to the \
         shared base and was refused as ambiguous: {said}"
    );
    assert!(
        wrote.contains("opencode-bbbb2222") && !wrote.contains("claude-aaaa1111"),
        "the decision was not attributed to the run that owns the checkout the \
         host named: {wrote}"
    );

    // The hook may run from anywhere, including a directory that is not the
    // base and not the worktree. A cwd naming the held checkout still has
    // to reach that holder; clamping it to the launch directory would drop
    // the name and re-open the ambiguity.
    let elsewhere_dir = tempfile::tempdir().expect("a directory the process was not launched in");
    clear_decisions(home.path());
    let payload = serde_json::json!({
        "cwd": worktree_a,
        "command": "git commit",
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        elsewhere_dir.path(),
        &["gate", "bash", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let wrote = decisions(home.path());
    assert!(
        !said.contains("several-runs-hold-this-checkout"),
        "a checkout the host named from outside the launch directory was discarded: {said}"
    );
    assert!(
        wrote.contains("claude-aaaa1111") && !wrote.contains("opencode-bbbb2222"),
        "a hook-route cwd launched from elsewhere did not reach the holder that owns it: {wrote}"
    );
}

/// A call carrying the host's session id is answered by the run it derives, not
/// by the holders of the directory it runs in.
///
/// OpenCode 2's `execute.before` event carries a session id and no run id, and
/// the module forwarded neither: every call was adjudicated by `holders_of`
/// against the directory the plugin was instantiated for. A session rooted at
/// the shared base is covered by every live run at equal depth, so its writes
/// were refused `several-runs-hold-this-checkout` — including writes naming no
/// checkout at all, measured on this machine on 2026-09-25 against two
/// issues with `~/.zshrc`. `--session-id` with `--runtime` is the other half of
/// `--run-id`: the derivation stays in the crate, pinned against
/// `session::run_id` here rather than against its text.
#[test]
fn a_call_carrying_a_session_id_is_not_refused_for_the_checkouts_holders() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let base = tempfile::tempdir().expect("a checkout");
    let worktree_a = base.path().join("wt-a");
    let worktree_b = base.path().join("wt-b");
    std::fs::create_dir_all(&worktree_a).expect("worktree a");
    std::fs::create_dir_all(&worktree_b).expect("worktree b");
    let repository_file = base.path().join(".git").join("estigia");
    std::fs::create_dir_all(&repository_file).expect("the repository's own directory");
    std::fs::write(
        repository_file.join("estigia.local.md"),
        "| Setting | Value here | Skill default |\n|---|---|---|\n\
         | Issue requirement | off | ask |\n",
    )
    .expect("the repository answers off");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    for (run_id, issue, worktree) in [
        ("claude-aaaa1111", 12u64, &worktree_a),
        ("opencode-bbbb2222", 34u64, &worktree_b),
    ] {
        let pointer = serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "state": "in-progress",
            "repo_dir": base.path(),
            "worktree": worktree,
            "verified_at": serde_json::Value::Null,
        });
        std::fs::write(
            runs.join(format!("{run_id}.json")),
            serde_json::to_string(&pointer).expect("a pointer serialises"),
        )
        .expect("the pointer writes");
    }

    let session = "ses_0123456789abcdef0123456789abcdef";
    let payload = serde_json::json!({ "file_path": base.path().join("scratch.txt") });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");

    // The floor: with no identity the shared base is genuinely ambiguous, and
    // the rows below are only a measurement while this still fires.
    let (out, error, ok) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !ok && said.contains("several-runs-hold-this-checkout"),
        "the identity-less floor stopped refusing, so this test measures nothing: {said}"
    );

    // The same call, carrying the session id the host's hook event sent. The
    // run it derives holds nothing here, so the answer is the stand-aside that
    // permits — and the two holders are never counted.
    let (out, error, ok) = run_in(
        home.path(),
        base.path(),
        &[
            "gate",
            "write",
            "--input",
            &payload,
            "--session-id",
            session,
            "--runtime",
            "opencode",
        ],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        ok && !said.contains("several-runs-hold-this-checkout"),
        "a call carrying the host's session id was still refused for the base \
         checkout's holders: {said}"
    );

    // An operator's exported fallback is not a second identity: a host that
    // forwards a session id keeps the identity road, which is the collision the
    // reviewers measured before this row existed.
    let (out, error, ok) = run_with_path(
        home.path(),
        base.path(),
        &[
            "gate",
            "write",
            "--input",
            &payload,
            "--session-id",
            session,
            "--runtime",
            "opencode",
        ],
        "",
        None,
        &[("ESTIGIA_RUN_ID", "claude-aaaa1111")],
    );
    let said = format!("{out}{error}");
    assert!(
        ok && !said.contains("several-runs-hold-this-checkout"),
        "an exported ESTIGIA_RUN_ID collided with the forwarded session id: {said}"
    );

    // Two identities, or half of one, are refused rather than ordered: a
    // precedence between them would pick a run nobody named.
    let derived = estigia::harness::session::run_id("opencode", session);
    for (extra, code) in [
        (
            vec![
                "--run-id",
                derived.as_str(),
                "--session-id",
                session,
                "--runtime",
                "opencode",
            ],
            "gate-identity-over-specified",
        ),
        (vec!["--session-id", session], "gate-identity-incomplete"),
        (vec!["--runtime", "opencode"], "gate-identity-incomplete"),
    ] {
        let mut arguments = vec!["gate", "write", "--input", payload.as_str()];
        arguments.extend(extra);
        let (out, error, ok) = run_in(home.path(), base.path(), &arguments, "");
        let said = format!("{out}{error}");
        assert!(
            !ok && said.contains(code),
            "the flag pair was not refused `{code}`: {said}"
        );
    }
}

/// `--json`'s `observed.checkout` names which candidate the resolution took,
/// and why one was dropped when one was — a structured account of a
/// refusal's stated cause, checked here against the checkout half rather than
/// against a rule that was wrong.
///
/// Three shapes, and they must not collapse into one another: nothing named
/// at all, a named candidate a holder does not cover, and a named candidate
/// the clamp refuses before any holder is even asked. The last of the three
/// is the case the issue was filed on — a `workdir` was named and thrown
/// away, and the first cut of this fix still answered `dropped: null` for it,
/// which is indistinguishable from nothing having been named.
#[test]
fn observed_checkout_names_its_source_and_what_it_dropped() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");

    let ask = |workdir: Option<&std::path::Path>| {
        let mut payload = serde_json::json!({ "command": "echo hola" });
        if let Some(workdir) = workdir {
            payload["workdir"] = serde_json::Value::String(workdir.display().to_string());
        }
        let payload = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, _) = run_in(
            home.path(),
            here.path(),
            &["gate", "Bash", "--json", "--input", &payload],
            "",
        );
        let said = format!("{out}{error}");
        serde_json::from_str::<serde_json::Value>(said.trim())
            .unwrap_or_else(|error| panic!("not JSON: {error}: {said}"))
    };

    // No `workdir` in the payload at all: nothing was there to narrow the
    // candidate, so the resolution never left the launch directory and has
    // nothing to report as dropped.
    let nothing_named = ask(None);
    assert_eq!(
        nothing_named["observed"]["checkout"]["source"], "launch-directory",
        "a call with no workdir did not resolve to the launch directory: {nothing_named}"
    );
    assert!(
        nothing_named["observed"]["checkout"]["dropped"].is_null(),
        "a call with nothing to drop reported a dropped candidate: {nothing_named}"
    );

    // A `workdir` that names a real directory inside the launch directory,
    // covered by no holder at all — narrowed, then dropped, and the
    // resolution falls back to the launch directory and says why.
    let candidate = here.path().join("sub");
    std::fs::create_dir_all(&candidate).expect("a directory inside the launch directory");
    let uncovered = ask(Some(&candidate));
    assert_eq!(
        uncovered["observed"]["checkout"]["source"], "launch-directory",
        "a workdir covered by no holder was not dropped back to the launch directory: {uncovered}"
    );
    let uncovered_dropped = first_dropped(&uncovered["observed"]["checkout"]["dropped"])
        .unwrap_or_else(|| panic!("a dropped candidate did not say why: {uncovered}"))
        .to_owned();

    // A `workdir` that names a real directory entirely **outside** the launch
    // directory — the exact case this issue was filed on. `narrowed_by_the_call`
    // clamps it to `None` before a holder is ever asked, and that `None` must
    // not read the same as "nothing was named": a reader who sees
    // `source: "launch-directory"` with `dropped: null` here has no way to
    // tell their `workdir` was considered and thrown away.
    let outside = tempfile::tempdir().expect("a directory outside the launch directory");
    let clamped = ask(Some(outside.path()));
    assert_eq!(
        clamped["observed"]["checkout"]["source"], "launch-directory",
        "a workdir outside the launch directory did not fall back to it: {clamped}"
    );
    let clamped_dropped = first_dropped(&clamped["observed"]["checkout"]["dropped"])
        .unwrap_or_else(|| {
            panic!("a workdir outside the launch directory was not reported as dropped: {clamped}")
        });

    // And the two rejections read differently: the clamp is a deliberate
    // refusal to trust a path outside the launch directory at all, where the
    // uncovered-holder case above it is a path that *was* trusted and simply
    // matched no claim. Collapsing them into one string was the other half of
    // what this issue measured.
    assert_ne!(
        clamped_dropped, uncovered_dropped,
        "a workdir outside the launch directory and a workdir no holder covers were reported \
         with the same dropped reason"
    );
}

/// `checkout_root_covering`'s two ways of answering `None` read
/// differently: zero holders is the opposite fact from several, and the
/// panel that rejected the first cut of this issue found the code answering
/// "uncovered by any holder" for both.
#[test]
fn observed_checkout_distinguishes_zero_holders_from_several_holders() {
    let home = tempfile::tempdir().expect("a temporary home");
    let base = tempfile::tempdir().expect("a checkout");
    let worktree_a = base.path().join("wt-a");
    let worktree_b = base.path().join("wt-b");
    std::fs::create_dir_all(&worktree_a).expect("worktree a");
    std::fs::create_dir_all(&worktree_b).expect("worktree b");

    let ask = |workdir: &std::path::Path| {
        let payload = serde_json::json!({
            "command": "echo hola",
            "workdir": workdir.display().to_string(),
        });
        let payload = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, _) = run_in(
            home.path(),
            base.path(),
            &["gate", "Bash", "--json", "--input", &payload],
            "",
        );
        let said = format!("{out}{error}");
        serde_json::from_str::<serde_json::Value>(said.trim())
            .unwrap_or_else(|error| panic!("not JSON: {error}: {said}"))
    };

    // Zero: nobody holds either worktree.
    let zero = ask(&worktree_a);
    let zero_dropped = first_dropped(&zero["observed"]["checkout"]["dropped"])
        .unwrap_or_else(|| panic!("an uncovered workdir was not reported as dropped: {zero}"))
        .to_owned();

    // Several: two runs, both covering `worktree_a` at the same depth — the
    // shape `holders_of`'s own doc calls out as still ambiguous.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    for (run_id, issue) in [("claude-aaaa1111", 1u64), ("claude-bbbb2222", 2u64)] {
        let pointer = serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "state": "in-progress",
            "repo_dir": worktree_a,
            "worktree": worktree_a,
            "verified_at": serde_json::Value::Null,
        });
        std::fs::write(
            runs.join(format!("{run_id}.json")),
            serde_json::to_string(&pointer).expect("a pointer serialises"),
        )
        .expect("the pointer writes");
    }
    let several = ask(&worktree_a);
    let several_dropped = first_dropped(&several["observed"]["checkout"]["dropped"])
        .unwrap_or_else(|| {
            panic!("a several-holder workdir was not reported as dropped: {several}")
        });

    assert_ne!(
        zero_dropped, several_dropped,
        "an uncovered workdir and one covered by several holders were reported with the same \
         dropped reason: {zero_dropped:?}"
    );
}

/// The same-directory exemption in `checkout_root_covering`: a single
/// holder's recorded root equal to the launch directory survives the drop.
/// The branch is observable through the reason, not the directory — `checkout_root_covering`
/// was made to report why it discarded a root, so dropping this
/// one would state, of the launch directory itself, that it was clamped
/// outside the launch directory.
#[test]
fn observed_checkout_does_not_clamp_a_holder_root_equal_to_the_launch_directory() {
    let home = tempfile::tempdir().expect("a temporary home");
    let launched = tempfile::tempdir().expect("the launch directory");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    let pointer = serde_json::json!({
        "run_id": "claude-equal1111",
        "issue": 157,
        "state": "in-progress",
        "repo_dir": launched.path(),
        "worktree": serde_json::Value::Null,
        "verified_at": serde_json::Value::Null,
    });
    std::fs::write(
        runs.join("claude-equal1111.json"),
        serde_json::to_string(&pointer).expect("a pointer serialises"),
    )
    .expect("the pointer writes");

    let payload = serde_json::json!({
        "command": "echo hola",
        "workdir": launched.path().display().to_string(),
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        launched.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let observed: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|error| panic!("not JSON: {error}: {said}"));

    let dropped = observed["observed"]["checkout"]["dropped"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !dropped
            .iter()
            .any(|reason| reason.as_str() == Some("clamped outside the launch directory")),
        "a holder whose recorded root is the launch directory itself was reported clamped \
         outside the launch directory: {observed}"
    );
}

/// A payload naming its command as an argv array is a live adapter shape —
/// Codex sends `bash`, `-lc`, then the command, all inside one array — not
/// "zero or several command keys". Exactly one key was there; its value was
/// simply not a string.
#[test]
fn observed_command_names_its_own_reason_for_an_argv_payload() {
    let home = tempfile::tempdir().expect("a temporary home");
    let payload = serde_json::json!({ "command": ["gh", "pr", "merge", "54", "--merge"] });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run(
        home.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    let _ = error;
    let value: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|error| panic!("not JSON: {error}: {out}"));
    let reason = value["observed"]["command"]["literal"]["unestablished"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("an argv command reported a literal instead of a reason: {value}")
        });
    assert!(
        reason.contains("argv"),
        "an argv payload's literal was not reported as an argv array: {reason:?}"
    );
    assert_ne!(
        reason, "the payload named zero or several command keys",
        "an argv payload with exactly one command key still reads as zero-or-several: {reason:?}"
    );
}

/// A denial through the ordinary claim road carries what the gate parsed —
/// both halves, on a `Deny` rather than only on `outside`/`allow` — and
/// `adjudicated` is checked against an actual value at least once.
#[test]
fn a_denied_delivery_reports_the_command_and_checkout_it_was_read_against() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let run_id = "claude-f5deny001";
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    let pointer = serde_json::json!({
        "run_id": run_id,
        "issue": 59,
        "state": "in-progress",
        "repo_dir": here.path(),
        "worktree": here.path(),
        "verified_at": serde_json::Value::Null,
    });
    std::fs::write(
        runs.join(format!("{run_id}.json")),
        serde_json::to_string(&pointer).expect("a pointer serialises"),
    )
    .expect("the pointer writes");

    // No `estigia setup` ran, so the contract is not installed and this is
    // refused `control-surface-not-installed` before the tracker is ever
    // asked — a `Deny` reached with nothing scripted.
    let payload = serde_json::json!({ "command": "gh pr merge 136 --merge 2>&1" });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run_in(
        home.path(),
        here.path(),
        &[
            "gate", "Bash", "--json", "--run-id", run_id, "--input", &payload,
        ],
        "",
    );
    assert!(
        !ok,
        "an uninstalled control surface was not refused: {out}{error}"
    );
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(
        value["code"], "control-surface-not-installed",
        "the wrong refusal answered: {value}"
    );

    let command = &value["observed"]["command"];
    assert_eq!(command["matched"], "gh pr merge", "{value}");
    assert_eq!(
        command["literal"], "gh pr merge 136 --merge 2>&1",
        "the raw command line, redirect included, was not carried: {value}"
    );
    assert_eq!(command["boundary_source"], "irreversible", "{value}");
    assert_eq!(command["disqualifying_byte"], ">", "{value}");

    let checkout = &value["observed"]["checkout"];
    assert_eq!(checkout["source"], "launch-directory", "{value}");
    // Both sides placed, because `adjudicated` is the spelling the gate itself
    // used — `launch_directory` is a bare `current_dir()` — and one directory
    // has more than one spelling on two of the three platforms this ships to.
    // macOS hands out `/var/folders/...` and reports `/private/var/folders/...`;
    // the Windows runner hands out `C:\Users\runneradmin\...` and reports the
    // 8.3 form `C:\Users\RUNNER~1\...`. Comparing the strings asserts whichever
    // spelling this machine happens to use, which is why this assertion has now
    // reddened the lane on both of them. Comparing what they place to asserts
    // the directory, which is the claim worth holding.
    let reported = checkout["adjudicated"]
        .as_str()
        .unwrap_or_else(|| panic!("the adjudicated checkout was not a string: {value}"));
    assert_eq!(
        estigia::paths::placed(std::path::Path::new(reported)),
        estigia::paths::placed(here.path()),
        "the gate adjudicated a different directory than the one it was launched in: {value}"
    );
}

/// A role refusal — the panel's strongest, independently found signal — now
/// carries what the gate had already read before it ever asked the role
/// question.
#[test]
fn a_role_refusal_carries_what_the_gate_read() {
    let home = tempfile::tempdir().expect("a temporary home");
    let payload = serde_json::json!({
        "command": "echo hola",
        "agent_type": "review-blind",
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run(
        home.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    assert!(
        !ok,
        "a review-blind role reaching for Bash was not refused: {out}{error}"
    );
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(value["code"], "tool-outside-declared-role", "{value}");
    assert!(
        value.get("observed").is_some(),
        "a role refusal reached an agent with the account this door built for it discarded: {value}"
    );
    assert!(
        value["observed"]["checkout"].is_object(),
        "a role refusal carries no checkout reading: {value}"
    );
}

/// A run pointer this machine cannot read is denied with the account the
/// gate had already built, and with the subject every other denial on this
/// road gets — `prefixed` is "the one place that has to", and this was the
/// one `Deny` `decide_observing` produced that bypassed it entirely.
#[test]
fn an_unreadable_run_pointer_carries_what_the_gate_read_and_is_subject_prefixed() {
    let home = tempfile::tempdir().expect("a temporary home");
    let run_id = "claude-corruptf5x";
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    std::fs::write(
        runs.join(format!("{run_id}.json")),
        "{\"run_id\": \"claude-corruptf5x\", \"issue\":",
    )
    .expect("a half-written pointer");

    let payload = serde_json::json!({ "file_path": "src/x.rs" });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run(
        home.path(),
        &[
            "gate", "Edit", "--json", "--run-id", run_id, "--input", &payload,
        ],
        "",
    );
    assert!(
        !ok,
        "an unreadable run pointer was not refused: {out}{error}"
    );
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(value["code"], "run-pointer-unreadable", "{value}");
    assert!(
        value["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("src/x.rs: ")),
        "the subject was not prefixed onto an unreadable-pointer refusal: {value}"
    );
    assert!(
        value.get("observed").is_some(),
        "an unreadable-pointer refusal reached an agent with its account discarded: {value}"
    );
}

/// A `cwd` the payload named and a `workdir` the same call named can each be
/// discarded for a different reason, and both are established facts — not
/// only the one written to the field last.
#[test]
fn observed_checkout_keeps_every_dropped_reason_not_only_the_last() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let elsewhere = tempfile::tempdir().expect("a directory outside the launch directory");
    let payload = serde_json::json!({
        "command": "echo hola",
        // Both keys a `bash` call can carry evidence in: `cwd` (the host's,
        // read by `payload_cwd`) and `workdir` (the call's own, read by
        // `narrowed_by_the_call`). Neither names anywhere a claim covers.
        "cwd": elsewhere.path().display().to_string(),
        "workdir": elsewhere.path().display().to_string(),
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        here.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let value: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {said}"));
    let dropped = value["observed"]["checkout"]["dropped"]
        .as_array()
        .unwrap_or_else(|| panic!("no dropped candidates were reported at all: {value}"));
    assert!(
        dropped.len() >= 2,
        "a payload cwd and a workdir both discarded reported only one reason between them: {value}"
    );
}

/// `CheckoutSource` names which of four candidates the resolution took, and
/// a mutation naming judge measured that only `launch-directory` was ever
/// checked: an unconditional `CheckoutSource::WriteTarget` at the
/// discriminator in `narrowed_by_the_call` left the whole suite green.
/// `payload-cwd`, `payload-workdir` and `write-target` each get their own
/// case here, driven through one held worktree so the resolution has
/// something to succeed against rather than fall back from.
#[test]
fn observed_checkout_reports_payload_cwd_payload_workdir_and_write_target() {
    let home = tempfile::tempdir().expect("a temporary home");
    let base = tempfile::tempdir().expect("a checkout");
    let worktree = base.path().join("wt-a");
    std::fs::create_dir_all(&worktree).expect("a worktree");

    let run_id = "claude-s3fixture01";
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    let pointer = serde_json::json!({
        "run_id": run_id,
        "issue": 1,
        "state": "in-progress",
        "repo_dir": worktree,
        "worktree": worktree,
        "verified_at": serde_json::Value::Null,
    });
    std::fs::write(
        runs.join(format!("{run_id}.json")),
        serde_json::to_string(&pointer).expect("a pointer serialises"),
    )
    .expect("the pointer writes");

    let ask = |tool: &str, payload: serde_json::Value| {
        let payload = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, _) = run_in(
            home.path(),
            base.path(),
            &["gate", tool, "--json", "--input", &payload],
            "",
        );
        let said = format!("{out}{error}");
        serde_json::from_str::<serde_json::Value>(said.trim())
            .unwrap_or_else(|error| panic!("not JSON: {error}: {said}"))
    };

    // `payload-workdir`: a Bash call whose `workdir` names the held worktree.
    let workdir_answer = ask(
        "Bash",
        serde_json::json!({
            "command": "echo hola",
            "workdir": worktree.display().to_string(),
        }),
    );
    assert_eq!(
        workdir_answer["observed"]["checkout"]["source"], "payload-workdir",
        "{workdir_answer}"
    );

    // `write-target`: a write tool whose target file lands inside the held
    // worktree — no `workdir`/`cwd` key at all, so only the target evidence
    // can be what selects it.
    let write_answer = ask(
        "Edit",
        serde_json::json!({ "file_path": worktree.join("f.txt").display().to_string() }),
    );
    assert_eq!(
        write_answer["observed"]["checkout"]["source"], "write-target",
        "{write_answer}"
    );

    // `payload-cwd`: a host-provided `cwd` naming the held worktree, read
    // before `workdir` ever is.
    let cwd_answer = ask(
        "Bash",
        serde_json::json!({
            "command": "echo hola",
            "cwd": worktree.display().to_string(),
        }),
    );
    assert_eq!(
        cwd_answer["observed"]["checkout"]["source"], "payload-cwd",
        "{cwd_answer}"
    );
}

/// A `workdir` that names a value the platform cannot even place is a third
/// way `narrowed_by_the_call` can discard a candidate, distinct from
/// "nothing named" and from "named and clamped outside the launch
/// directory" — the sibling of `context_the_payload_names`'s own "the
/// payload cwd could not be placed". An absolute path one `..` past its own
/// root is unplaceable on both platforms this ships to, portably: `placed`
/// refuses to pop past a prefix or root component.
#[test]
fn an_unplaceable_workdir_is_reported_as_dropped_rather_than_nothing_named() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    #[cfg(windows)]
    let unplaceable = "C:\\..";
    #[cfg(unix)]
    let unplaceable = "/..";
    let payload = serde_json::json!({
        "command": "echo hola",
        "workdir": unplaceable,
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        here.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let value: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {said}"));
    assert_eq!(
        value["observed"]["checkout"]["source"], "launch-directory",
        "an unplaceable workdir did not fall back to the launch directory: {value}"
    );
    let dropped = first_dropped(&value["observed"]["checkout"]["dropped"]).unwrap_or_else(|| {
        panic!("an unplaceable workdir was not reported as dropped at all: {value}")
    });
    assert!(
        dropped.contains("placed"),
        "an unplaceable workdir was not reported with its own reason: {dropped:?}"
    );
}

/// The sibling of the `workdir` case above: a write tool's target can be
/// named and unplaceable too, and `narrowed_by_the_call`'s write-target
/// closure used a `?` chain that let this fall through as "nothing named"
/// instead — reproduced against this crate's own build with
/// `estigia gate Edit --json --input '{"file_path":"C:\\..\\x.rs"}'`, which
/// answered byte-identical to a call naming no target at all.
#[test]
fn an_unplaceable_write_target_is_reported_as_dropped_rather_than_nothing_named() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    #[cfg(windows)]
    let unplaceable = "C:\\..\\x.rs";
    #[cfg(unix)]
    let unplaceable = "/../x.rs";
    let payload = serde_json::json!({ "file_path": unplaceable });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        here.path(),
        &["gate", "Edit", "--json", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let value: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {said}"));
    assert_eq!(
        value["observed"]["checkout"]["source"], "launch-directory",
        "an unplaceable write target did not fall back to the launch directory: {value}"
    );
    let dropped = first_dropped(&value["observed"]["checkout"]["dropped"]).unwrap_or_else(|| {
        panic!("an unplaceable write target was not reported as dropped at all: {value}")
    });
    assert!(
        dropped.contains("placed"),
        "an unplaceable write target was not reported with its own reason: {dropped:?}"
    );
}

/// One long-lived MCP server survives its own `start_branch`.
///
/// The whole of the defect, driven as a process because that is the shape the
/// defect has: an MCP server's directory is fixed for its lifetime, and no unit
/// test that constructs a `GateContext` per call can see a *second* call arrive
/// at the same one.
///
/// The precondition is not exotic. A `claim` whose tracker write lands and whose
/// readback fails returns `Err` before the pointer effect runs, so the run holds
/// the issue on the timeline and records no `repo_dir` — measured five times on
/// 2026-08-17 against the live tracker during a GitHub outage, and it is the
/// state the run that filed the issue was left in. From there the dispatch
/// guard's precondition — a run with no coverage has nothing to be outside of —
/// lets `start_branch` through, and the worktree it records becomes the run's
/// *only* covered directory. The next call from the same server is refused
/// `run-id-names-another-checkout`, and repeating it cannot help, because a
/// server's working directory does not change when a child command uses another.
///
/// Reverting `run.repo_dir.get_or_insert_with(..)` in `apply_effect` restores
/// the refusal and reddens the second half of this test.
#[test]
fn one_server_survives_the_isolation_it_created() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let origin = tempfile::tempdir().expect("a bare origin");
    let trees = tempfile::tempdir().expect("a directory for worktrees");
    let run_id = "claude-abcd1234";
    let branch = "fix-12-isolation";

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
        // Nonzero, so the sidebar read below decides — the ordinary shape when
        // the link cannot be made, and the one the live run answered with.
        { "matches": "issue develop", "stdout": "", "status": 1 },
        {
            "matches": "repo view",
            "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}",
            "status": 0,
        },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "linkedBranches": {
                    "nodes": [],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // The observed shape: the timeline holds the claim, the pointer holds no
    // checkout. Written field by field rather than through `claim`, because
    // reproducing it through `claim` would mean reproducing the outage.
    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    assert!(
        run.repo_dir.is_none(),
        "this fixture is only the defect's precondition while the checkout is absent"
    );
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = |id: u64, name: &str, arguments: serde_json::Value| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        })
        .to_string()
    };
    // Both down one pipe, into one process, in arrival order — which is the only
    // way to pose "the server that called `start_branch` becomes its own next
    // refusal".
    //
    // Written and read **one at a time** rather than both at once, so the pointer
    // can be inspected in between. Sending both and asserting only at the end
    // measured the pair and not the halves: the renewal fills the same field by
    // its own route, so removing the isolation's line left this test green while
    // the unit test beside it reddened. Two reviewers found that independently,
    // and the fix is where the assertion is taken rather than what it says.
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
    let answered = |line: &str| -> serde_json::Value {
        serde_json::from_str(line.trim())
            .unwrap_or_else(|_| panic!("not a JSON-RPC line: {line:?}"))
    };
    let text = |response: &serde_json::Value| -> String {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    let answer = |stdout: &mut std::io::BufReader<std::process::ChildStdout>| -> String {
        let mut line = String::new();
        stdout.read_line(&mut line).expect("the server answers");
        assert!(!line.trim().is_empty(), "the server answered nothing");
        line
    };

    writeln!(
        stdin,
        "{}",
        request(
            1,
            "start_branch",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "expect_state": "in-progress",
                "worktree_root": trees.path().display().to_string(),
            }),
        )
    )
    .expect("the isolation request is written");
    let isolated = answered(&answer(&mut stdout));
    assert_eq!(
        isolated["result"]["isError"],
        false,
        "the isolation itself did not happen, so the second call proves nothing: {}",
        text(&isolated)
    );

    // Here, and not at the end: this is the only moment at which the isolation is
    // the only thing that can have written the checkout down.
    let between = estigia::harness::session::load(&runs, run_id);
    assert!(
        between.worktree.is_some(),
        "the isolated checkout was not recorded at all"
    );
    assert!(
        between
            .covered()
            .any(|covered| estigia::paths::covers(covered, repo)),
        "isolation left the run covering {:?} and not the checkout it was adjudicated from, so \
         the next call from this same server is refused",
        between.covered().collect::<Vec<_>>()
    );

    writeln!(
        stdin,
        "{}",
        request(
            2,
            "verify_claim",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "expect_state": "in-progress",
            }),
        )
    )
    .expect("the renewal request is written");
    let renewed = answered(&answer(&mut stdout));
    drop(stdin);
    child.wait().expect("the MCP server exits");
    let said = text(&renewed);
    assert!(
        !said.contains("run-id-names-another-checkout"),
        "the server that created the worktree was refused in the checkout it is running in: {said}"
    );
    assert_eq!(
        renewed["result"]["isError"], false,
        "the renewal after isolation was refused: {said}"
    );

    // And the reason it survived: the pointer names both directories, which is
    // what `docs/honesty.md` says the gate covers.
    let after = estigia::harness::session::load(&runs, run_id);
    assert!(
        after.worktree.is_some(),
        "the isolated checkout was not recorded, so nothing was measured here"
    );
    assert!(
        after
            .covered()
            .any(|covered| estigia::paths::covers(covered, repo)),
        "isolation left the run covering {:?} and not the checkout it was adjudicated from",
        after.covered().collect::<Vec<_>>()
    );
}

/// A run that is already stranded gets itself back with one ordinary call.
///
/// The other half of it. Recording the claim's checkout at isolation
/// stops a run reaching this state; it does nothing for the ones already in it,
/// and there were several — the issue names two, and the run that fixed it made
/// a third by claiming through the same outage.
///
/// The state is the one the field reported: the timeline holds the claim, the
/// pointer holds a worktree and nothing that says where the claim was sworn. The
/// call is `verify_claim`, which is the one the contract already requires before
/// the first repository write, made from the checkout the server is running in —
/// no new verb, no restart, and no tracker *write*, which is what makes it
/// reachable during exactly the outage that causes the damage.
///
/// Two things are measured here and neither is enough alone: that the call is
/// not refused, and that the record it was measured against comes back complete.
/// A run allowed through on an empty pointer is a run whose repository writes are
/// still not measured against anything.
#[test]
fn a_stranded_run_recovers_from_the_checkout_it_is_running_in() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let stranded = tempfile::tempdir().expect("the worktree it was left holding");
    let run_id = "claude-abcd1234";

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
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.worktree = Some(stranded.path().to_path_buf());
    assert!(
        run.issue.is_none() && run.repo_dir.is_none(),
        "the fixture is only the stranded shape while the claim's checkout is absent"
    );
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "verify_claim",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "expect_state": "in-progress",
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
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let response: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let said = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        !said.contains("run-id-names-another-checkout"),
        "the stranded run was refused the one call the contract tells it to make: {said}"
    );
    assert_eq!(
        response["result"]["isError"], false,
        "the renewal a stranded run recovers with was refused: {said}"
    );

    // Allowed through is only half of it. A run readmitted on an empty record is
    // a run whose repository writes are still measured against nothing.
    let after = estigia::harness::session::load(&runs, run_id);
    assert_eq!(
        after.issue,
        Some(estigia::identity::Identity::Number(12)),
        "the run came back holding nothing, so nothing downstream is gated"
    );
    assert_eq!(
        after.worktree.as_deref(),
        Some(stranded.path()),
        "recovering cost the run the isolated checkout it had already been given"
    );
    assert!(
        after
            .covered()
            .any(|covered| estigia::paths::covers(covered, repo)),
        "the record still does not cover the checkout the server is running in: {:?}",
        after.covered().collect::<Vec<_>>()
    );
}

/// The two ways of asking what governs give one answer.
///
/// `estigia config list` with no agent named answers *what governs here*, and
/// the gate decides in one root for the whole machine. Those were two different
/// files: the command walked the declared adapter order and answered from the
/// first configured one — the shared neutral root — while the gate had picked
/// the root holding the operator's own `estigia.local.md`. Measured on the
/// machine where it was found: `Blind judges` read back `single` from one command
/// and `two blind` from the other, on the same machine, one flag apart.
#[test]
fn what_governs_reads_the_same_whether_or_not_an_agent_is_named() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    for agent in ["agents", "claude-code"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "setup failed: {stderr}");
    }

    // Their own file, beside the contract of the agent they configured, setting
    // a row neither installed table carries.
    std::fs::write(
        home.path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Model routing | developer=opus |\n",
    )
    .expect("the operator's own file");

    let rows = |arguments: &[&str]| {
        let (out, stderr, ok) = run(home.path(), arguments, "");
        assert!(ok, "`{arguments:?}` failed: {stderr}");
        out
    };
    let unnamed = rows(&["config", "list"]);
    let owner = rows(&["config", "list", "--agent", "claude-code"]);

    assert_eq!(
        unnamed, owner,
        "`config list` and `config list --agent claude-code` answer differently on a machine \
         whose gate decides in the Claude Code root"
    );
    assert!(
        unnamed.contains("developer"),
        "the row the operator wrote is not what the command reports:\n{unnamed}"
    );
}

#[test]
fn a_fact_about_this_machine_reaches_every_contract_and_cannot_be_set_per_agent() {
    // Two roots answered one machine-wide question differently and no command
    // made them agree. `Setting::scope()` has three values and both write paths
    // were written as though it had two: the plain form asked `elsewhere()`
    // only for `Scope::Everywhere`, so a machine row landed in the canonical
    // contract alone; and `config set --agent` refused an `Everywhere` row
    // while accepting a `Machine` one, which then could not be held — the
    // per-agent renderer intersects with `AGENT_SETTINGS` and drops it on the
    // way out.
    //
    // Measured before the fix, in a scratchpad home: `config set "Summary
    // language" Spanish` left `doctor` reporting `agents reads Summary language
    // English against Spanish`, and the per-agent form answered
    // `setting-shadowed-by-local-file` — blaming a file that need not exist.
    //
    // A person writes in one language across every checkout they have, which is
    // what `Scope::Machine`'s own doc comment says. So the row spreads like a
    // repository row, and cannot be given a different answer for one agent.
    let home = tempfile::tempdir().expect("a temporary home");
    for agent in ["claude-code", "codex"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "installing {agent} failed: {stderr}");
    }

    let (out, stderr, ok) = run(
        home.path(),
        &["config", "set", "Summary language", "Spanish"],
        "",
    );
    assert!(ok, "the machine write failed: {stderr}");
    assert!(
        out.contains("other installed contract"),
        "a machine row reached one contract and said nothing about the rest: {out}"
    );
    // And it says which kind of fact it just spread. The sentence was hardcoded
    // "a fact about this repository" and printed for both, so the tool answered
    // one question two ways in its own output — the row is about the machine
    // where it refuses, about the repository where it succeeds.
    assert!(
        out.contains("fact about this machine"),
        "the propagation sentence calls a machine row something else: {out}"
    );

    // `--repo` refuses it, and names a command that works. It used to send the
    // operator to `--agent`, which is the refusal one command later: a dead end,
    // which this repository's rules call worse than naming nothing.
    let checkout = tempfile::tempdir().expect("a temporary checkout");
    std::fs::create_dir(checkout.path().join(".git"))
        .expect("something that looks like a checkout");
    let (_, stderr, ok) = run_in(
        home.path(),
        checkout.path(),
        &["config", "set", "--repo", "Summary language", "English"],
        "",
    );
    assert!(
        !ok,
        "a machine row was written as a fact about one repository"
    );
    assert!(
        stderr.contains("setting-not-the-repositorys"),
        "the wrong refusal: {stderr}"
    );
    assert!(
        stderr.contains("this machine"),
        "the refusal still says a machine row is what one agent does: {stderr}"
    );
    assert!(
        !stderr.contains("`--agent <agent>` instead"),
        "the refusal points at the door that refuses it too: {stderr}"
    );

    // Every installed contract, not just the one that answered first. Read
    // through `config list --agent`, because that is the question a run asks.
    for slug in ["claude-code", "codex"] {
        let (out, _, _) = run(
            home.path(),
            &["config", "list", "--agent", slug, "--json"],
            "",
        );
        let rows: serde_json::Value = serde_json::from_str(&out).expect("config list prints JSON");
        let held = rows
            .as_array()
            .expect("an array of rows")
            .iter()
            .find(|row| row["setting"] == "Summary language")
            .map(|row| row["value"].as_str().unwrap_or_default().to_owned())
            .expect("the row is offered");
        assert_eq!(
            held, "Spanish",
            "{slug} still answers something else for a question about this machine"
        );
    }

    // And the divergence `doctor` exists to report is gone, rather than being
    // reported with no way out. This is the row added for it.
    let (out, stderr, _) = run(home.path(), &["doctor"], "");
    let report = format!("{out}{stderr}");
    assert!(
        !report.contains("Summary language"),
        "doctor still reports a machine-wide divergence after the one command that should clear it: {report}"
    );

    // The other half: the divergence cannot be created per agent either. Told
    // in the same breath which command does hold it, because a refusal that
    // does not name a way through is one an operator has to guess past.
    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "--agent",
            "codex",
            "Summary language",
            "English",
        ],
        "",
    );
    assert!(!ok, "a machine row was accepted for one agent");
    assert!(
        stderr.contains("setting-not-per-agent"),
        "the wrong refusal: {stderr}"
    );
    assert!(
        !stderr.contains("setting-shadowed-by-local-file"),
        "the refusal still blames a local override for a row that is simply not per-agent: {stderr}"
    );
    assert!(
        stderr.contains("estigia config set"),
        "the refusal does not name the command that holds the row: {stderr}"
    );

    // The refusal is a refusal, not a write it then complained about.
    let (out, _, _) = run(
        home.path(),
        &["config", "list", "--agent", "codex", "--json"],
        "",
    );
    let rows: serde_json::Value = serde_json::from_str(&out).expect("config list prints JSON");
    let held = rows
        .as_array()
        .expect("an array of rows")
        .iter()
        .find(|row| row["setting"] == "Summary language")
        .map(|row| row["value"].as_str().unwrap_or_default().to_owned())
        .expect("the row is offered");
    assert_eq!(
        held, "Spanish",
        "the refused per-agent write changed the answer anyway"
    );
}

/// Every publication starts the lane **once**, and starts it before the receipt
/// exists.
///
/// Two properties, and neither is provable from the source. The count is the one
/// this issue turns on: `CHANGELOG.md` records why CI does not start on a topic
/// push, open, synchronize or reopen — one run per push exposes the head before
/// the review barrier exists — so a fix that started a run per push would have
/// re-created exactly what the barrier refused, while looking like more
/// coverage. One run per **publication epoch** is a different number, and this
/// is where it is counted.
///
/// The ordering is the other. The dispatch sits between the push and the
/// `published` marker, so a token that may not start workflows refuses while no
/// epoch exists — rather than recording a receipt whose lane nobody could start,
/// which is a head judges get bound to and CI never answers for. Measured as a
/// position in the call log: the dispatch is before the comment that carries the
/// marker.
///
/// Both entry points, because they share a body today and nothing says they
/// always will.
#[test]
fn each_publication_starts_one_lane_run_and_records_its_receipt_locally() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-lane";

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
    // Pushed already, so both routes land: the ordinary push is a no-op
    // fast-forward and the leased one leases against the head the receipt below
    // names. What is being counted is the dispatch, not the push.
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
            {
                "id": "IC_2",
                "createdAt": "2026-01-01T01:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!(
                    "Published draft for review.\n\n<!-- issue-flow: published run-id={run_id} \
                     pr=7 head={head} base={base_sha} digest={} epoch={} -->\n",
                    "c".repeat(64),
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
    let answers = serde_json::to_string(&serde_json::json!([
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

    for tool in ["publish_review", "republish_review"] {
        let runs = home.join(".estigia").join("runs");
        // Revision-guarded: the second pass needs a clean pointer.
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
            "params": { "name": tool, "arguments": {
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

        let log = trace.path().join(format!("lane-{tool}.log"));
        // Where the receipt this call stages can be read back from. One
        // directory per entry point, because the staged name carries the
        // child's process id and two children would leave two files.
        let temp = trace.path().join(format!("temp-{tool}"));
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
        assert_eq!(
            response["result"]["isError"], false,
            "{tool} did not land: {text}"
        );
        // The wiring, not the renderer. Pinning `answer_text` alone left the
        // whole suite green when `run_tool` stopped calling it and rendered
        // indented itself, which is the regression that puts the cost back.
        assert_eq!(
            text.lines().count(),
            1,
            "{tool} answered with indentation the model pays for: {text}"
        );
        // Read the value, not the spelling it happens to wear: a substring
        // carrying the indented `": "` tested the renderer rather than the lane.
        let answered: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|error| panic!("{tool}: {error}: {text}"));
        assert_eq!(
            answered["publication_lane"], "started",
            "{tool} did not report that it started the lane: {text}"
        );

        let calls: Vec<String> = std::fs::read_to_string(&log)
            .expect("the call log")
            .lines()
            .map(str::to_owned)
            .collect();
        let dispatched: Vec<&String> = calls
            .iter()
            .filter(|line| line.contains("workflow run"))
            .collect();
        assert_eq!(
            dispatched.len(),
            1,
            "{tool} started {} lane runs for one publication epoch; one per push is the \
             cardinality the draft barrier refused: {calls:?}",
            dispatched.len()
        );
        assert!(
            dispatched[0].contains(&format!("workflow run ci.yml --ref {branch}")),
            "{tool} dispatched something other than this repository's lane against the published \
             branch: {}",
            dispatched[0]
        );

        assert!(
            any_receipt_staged(home, 12)
                && !calls.iter().any(|line| line.contains("issue comment")),
            "{tool} did not record the review transaction locally, or posted it: {calls:?}"
        );

        // And what that receipt **says**, which is the half no test read. The
        // answer above and the comment here come out of one call, and they used
        // to contradict each other: one reported a lane an accepted verdict
        // waits on, the other told every reviewer CI was blocked until the pull
        // request is readied. A reviewer who believes the comment never looks at
        // the lane, and reads its refusal as a malfunction.
        let receipt = staged_receipt(home, 12);
        assert!(
            !receipt.contains("CI remains blocked while the PR is draft"),
            "{tool} posted a receipt saying CI is blocked in the same call that started the \
             publication lane: {receipt}"
        );
        assert!(
            receipt.contains(
                "One run of `ci.yml` was started against this head as this receipt was recorded, \
                 and an accepted verdict cannot be recorded until it is green."
            ),
            "{tool} posted a receipt that does not say a verdict waits on the lane it started: \
             {receipt}"
        );
        assert!(
            receipt.contains("still waits for it to be marked ready"),
            "{tool} posted a receipt that drops the draft barrier, so it now reads as though the \
             ordinary pull-request-event lane were running too: {receipt}"
        );
    }
}

/// One publication epoch dispatches every active `.github/workflows/*` file the
/// listing names, not a hardcoded `ci.yml`, and the receipt records the width.
///
/// A third candidate without `workflow_dispatch` answers `HTTP 422` and is not
/// counted among the started set — discovery does not parse consumer YAML, so a
/// file without the trigger is told apart only when the dispatch is refused.
#[test]
fn a_publication_dispatches_every_discovered_lane_and_records_the_width() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-multi-lane";

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
    let workflows = serde_json::json!({
        "total_count": 4,
        "workflows": [
            { "state": "active", "path": ".github/workflows/ci.yml" },
            { "state": "active", "path": ".github/workflows/ci-api.yml" },
            { "state": "active", "path": ".github/workflows/release.yml" },
            { "state": "active", "path": "dynamic/copilot-swe-agent/copilot" },
            { "state": "disabled_manually", "path": ".github/workflows/old.yml" },
        ],
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "actions/workflows",
            "stdout": workflows,
            "status": 0
        },
        {
            "matches": "workflow run release.yml",
            "stderr": "gh: HTTP 422: Workflow does not have 'workflow_dispatch' trigger",
            "status": 1
        },
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

    let log = trace.path().join("multi-lane.log");
    let temp = trace.path().join("temp-multi");
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
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        response["result"]["isError"], false,
        "publish_review did not land: {text}"
    );
    let answered: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert_eq!(answered["publication_lane"], "started", "{text}");
    // `publication_lane_count`/`publication_lane_workflows` moved to
    // `review_transaction` — see `docs/review-cost.md`. What actually
    // started — and that a workflow without `workflow_dispatch` is not among
    // it — is checked on the staged marker below, the same evidence
    // `review_transaction` itself reads back.

    let calls = std::fs::read_to_string(&log).expect("the call log");
    assert!(
        calls.contains("actions/workflows"),
        "discovery never asked the workflows listing: {calls}"
    );
    assert!(
        calls.contains(&format!("workflow run ci.yml --ref {branch}"))
            && calls.contains(&format!("workflow run ci-api.yml --ref {branch}")),
        "both dispatchable lanes must run against the published branch: {calls}"
    );
    assert!(
        calls.contains("workflow run release.yml"),
        "a candidate without the trigger is still asked, so it can be classified Absent: {calls}"
    );
    assert!(
        !calls.contains("workflow run old.yml") && !calls.contains("copilot"),
        "disabled and non-.github/workflows paths must never be candidates: {calls}"
    );

    let receipt = staged_receipt(home, 12);
    assert!(
        receipt.contains(
            "2 runs (`ci-api.yml`, `ci.yml`) were started against this head as this receipt was \
             recorded, and an accepted verdict cannot be recorded until every one is green."
        ),
        "the receipt must name every started lane and the count: {receipt}"
    );
    assert!(
        receipt.contains("lanes=ci-api.yml%2Cci.yml") && receipt.contains("lane-count=2"),
        "the published marker must record the width: {receipt}"
    );
}

/// A Pi subagent is a child `pi` process with a session id of its own, so its
/// gate used to derive a run that holds nothing and let the write through even
/// when the parent's claim no longer stood.
///
/// The parent's claim is made unholdable by the stand-in `gh` (the issue is
/// closed), so the discriminating row is the same call with and without
/// `--parent-session-id`: without it the child is allowed as `nothing-sworn`,
/// with it the child is refused exactly as the parent is.
#[test]
fn a_pi_child_is_decided_as_its_parent_run_when_it_names_the_parent_session() {
    let rig = tracker_rig();
    let worktree = tempfile::tempdir().expect("a worktree");
    let runs = rig.home.path().join(".estigia").join("runs");
    let mut run =
        estigia::harness::session::Run::new(estigia::harness::session::run_id("pi", "PARENT"));
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(rig.repo.path().to_path_buf());
    run.worktree = Some(worktree.path().to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );
    let payload = serde_json::json!({
        "file_path": worktree.path().join("f.txt").display().to_string(),
        "content": "x",
    })
    .to_string();
    let answers = serde_json::to_string(&serde_json::json!([
        rest_listing(&[]),
        {
            "matches": "issues/12",
            "stdout": serde_json::json!({
                "state": "closed",
                "repository_url": "https://api.github.com/repos/o/r",
            }).to_string(),
            "status": 0,
        },
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "CLOSED",
                "labels": [],
                "comments": [],
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let ask = |identity: &[&str]| {
        let mut arguments = vec!["gate", "write", "--json", "--input", payload.as_str()];
        arguments.extend(identity);
        let ran = tracker_command(rig.home.path(), rig.repo.path(), rig.bin.path(), &answers)
            .args(&arguments)
            .output()
            .expect("the gate runs");
        let said = if ran.status.success() {
            ran.stdout
        } else {
            ran.stderr
        };
        let said: serde_json::Value =
            serde_json::from_slice(&said).expect("the gate answers in JSON");
        (ran.status.success(), said)
    };

    let (ok, parent) = ask(&["--session-id", "PARENT", "--runtime", "pi"]);
    assert!(
        !ok && parent["code"] == "issue-not-open",
        "the fixture does not make the parent's own write refused, so nothing below measures it: {parent}"
    );

    let (ok, unlinked) = ask(&["--session-id", "CHILD", "--runtime", "pi"]);
    assert!(
        ok && unlinked["aside"] == "nothing-sworn",
        "the child with no link to its parent is no longer unmeasured: {unlinked}"
    );

    let (ok, linked) = ask(&[
        "--session-id",
        "CHILD",
        "--runtime",
        "pi",
        "--parent-session-id",
        "PARENT",
    ]);
    assert!(
        !ok && linked["code"] == "issue-not-open",
        "the child named its parent session and was not decided as the parent is: {linked}"
    );

    let (ok, stranger) = ask(&[
        "--session-id",
        "CHILD",
        "--runtime",
        "pi",
        "--parent-session-id",
        "NO-SUCH-PARENT",
    ]);
    assert!(
        ok && stranger["aside"] == "nothing-sworn",
        "a parent that names no run was answered from somewhere else: {stranger}"
    );
}

#[test]
fn a_parent_session_id_is_refused_beside_a_run_id_without_a_runtime_and_when_blank() {
    let home = tempfile::tempdir().expect("a temporary home");
    let payload = "{}";
    for (extra, code) in [
        (
            vec![
                "--parent-session-id",
                "P",
                "--run-id",
                "pi-aaaa1111",
                "--runtime",
                "pi",
            ],
            "gate-identity-over-specified",
        ),
        (vec!["--parent-session-id", "P"], "gate-identity-incomplete"),
        (
            vec!["--parent-session-id", " ", "--runtime", "pi"],
            "cannot be blank",
        ),
    ] {
        let mut arguments = vec!["gate", "write", "--input", payload];
        arguments.extend(extra);
        let (out, error, ok) = run_in(home.path(), home.path(), &arguments, "");
        let said = format!("{out}{error}");
        assert!(
            !ok && said.contains(code),
            "the flags were not refused with `{code}`: {said}"
        );
    }
}

fn pi_session_run(rig: &TrackerRig, arguments: &[&str]) -> (String, String, bool) {
    let ran = tracker_command(rig.home.path(), rig.repo.path(), rig.bin.path(), "[]")
        .args(arguments)
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&ran.stdout).into_owned(),
        String::from_utf8_lossy(&ran.stderr).into_owned(),
        ran.status.success(),
    )
}

fn hold_issue_as(rig: &TrackerRig, session: &str, issue: u64) -> String {
    let runs = rig.home.path().join(".estigia").join("runs");
    let id = estigia::harness::session::run_id("pi", session);
    let mut run = estigia::harness::session::Run::new(id.clone());
    run.issue = Some(estigia::identity::Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(rig.repo.path().to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );
    id
}

#[test]
fn a_pi_session_start_prints_the_same_context_the_hook_gives_and_names_the_forked_from_claim() {
    let rig = tracker_rig();
    let parent_run = hold_issue_as(&rig, "PARENT", 12);
    let own_run = estigia::harness::session::run_id("pi", "FORK");

    let (plain, _, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "start",
            "--session-id",
            "PARENT",
            "--runtime",
            "pi",
        ],
    );
    assert!(ok, "{plain}");
    assert!(
        plain.contains(&format!("Run id `{parent_run}`")) && plain.contains("holds issue #12"),
        "{plain}"
    );
    assert!(
        !plain.contains("The session this one was started from"),
        "{plain}"
    );

    for reason in ["fork", "new"] {
        let (said, _, ok) = pi_session_run(
            &rig,
            &[
                "session",
                "start",
                "--session-id",
                "FORK",
                "--runtime",
                "pi",
                "--reason",
                reason,
                "--previous-session-id",
                "PARENT",
            ],
        );
        assert!(ok, "{said}");
        assert!(said.contains(&format!("Run id `{own_run}`")), "{said}");
        assert!(
            said.contains(&parent_run) && said.contains("#12") && said.contains("holds nothing"),
            "{reason}: {said}"
        );
        assert!(
            said.contains(&format!("estigia release --run-id {parent_run}")),
            "{reason}: the handover names no command: {said}"
        );
    }

    for reason in ["resume", "startup", "reload"] {
        let (said, _, ok) = pi_session_run(
            &rig,
            &[
                "session",
                "start",
                "--session-id",
                "FORK",
                "--runtime",
                "pi",
                "--reason",
                reason,
                "--previous-session-id",
                "PARENT",
            ],
        );
        assert!(ok && !said.contains(&parent_run), "{reason}: {said}");
    }

    let (said, _, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "start",
            "--session-id",
            "FORK",
            "--runtime",
            "pi",
            "--reason",
            "fork",
            "--previous-session-id",
            "NOBODY",
        ],
    );
    assert!(
        ok && !said.contains("The session this one was started from"),
        "{said}"
    );
    let moved =
        estigia::harness::session::load(&rig.home.path().join(".estigia").join("runs"), &own_run);
    assert!(moved.issue.is_none(), "a fork moved the claim: {moved:?}");
}

#[test]
fn a_pi_session_end_forgets_the_pointer_only_when_the_session_is_really_over() {
    let rig = tracker_rig();
    let runs = rig.home.path().join(".estigia").join("runs");
    let run = hold_issue_as(&rig, "ENDING", 12);

    for reason in ["fork", "new", "resume", "reload"] {
        let (said, _, ok) = pi_session_run(
            &rig,
            &[
                "session",
                "end",
                "--session-id",
                "ENDING",
                "--runtime",
                "pi",
                "--reason",
                reason,
            ],
        );
        assert!(ok, "{said}");
        assert!(
            estigia::harness::session::load(&runs, &run).issue.is_some(),
            "a session that was only replaced lost its pointer on `{reason}`"
        );
    }

    let (said, _, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "end",
            "--session-id",
            "ENDING",
            "--runtime",
            "pi",
            "--reason",
            "quit",
        ],
    );
    assert!(ok, "{said}");
    assert!(
        estigia::harness::session::load(&runs, &run).issue.is_none(),
        "a quit left the pointer behind"
    );
    let (_, _, again) = pi_session_run(
        &rig,
        &[
            "session",
            "end",
            "--session-id",
            "ENDING",
            "--runtime",
            "pi",
            "--reason",
            "quit",
        ],
    );
    assert!(again, "a second end was refused");
}

fn listed_sessions(rig: &TrackerRig) -> Vec<serde_json::Value> {
    let (said, stderr, ok) = pi_session_run(rig, &["sessions", "--json", "--all"]);
    assert!(ok, "{said}{stderr}");
    let parsed: serde_json::Value = serde_json::from_str(said.trim()).expect("sessions is JSON");
    parsed["sessions"].as_array().expect("a list").clone()
}

fn session_named<'a>(listed: &'a [serde_json::Value], run_id: &str) -> &'a serde_json::Value {
    listed
        .iter()
        .find(|row| row["run_id"] == run_id)
        .unwrap_or_else(|| panic!("{run_id} is not listed: {listed:?}"))
}

#[test]
fn sessions_lists_every_session_with_its_claim_and_marks_the_silent_and_the_ended() {
    let rig = tracker_rig();
    let start = |session: &str| {
        let (said, _, ok) = pi_session_run(
            &rig,
            &[
                "session",
                "start",
                "--session-id",
                session,
                "--runtime",
                "pi",
            ],
        );
        assert!(ok, "{said}");
    };
    start("alive");
    start("crashed");
    let crashed = hold_issue_as(&rig, "crashed", 12);
    let unregistered = hold_issue_as(&rig, "neverseen", 13);
    let alive = estigia::harness::session::run_id("pi", "alive");

    let listed = listed_sessions(&rig);
    let row = session_named(&listed, &alive);
    assert_eq!(row["status"], "live", "{row}");
    assert_eq!(row["runtime"], "pi", "{row}");
    assert!(row["issue"].is_null() && row["last_seen"].is_u64(), "{row}");
    assert_eq!(session_named(&listed, &crashed)["issue"], 12, "{listed:?}");
    assert_eq!(
        session_named(&listed, &unregistered)["status"],
        "unregistered"
    );

    rusqlite::Connection::open(rig.home.path().join(".estigia").join("state.db"))
        .expect("the store opens")
        .execute(
            "UPDATE sessions SET last_seen = 1 WHERE run_id = ?1",
            [&crashed],
        )
        .expect("the row ages");
    let listed = listed_sessions(&rig);
    let row = session_named(&listed, &crashed);
    assert_eq!(row["status"], "stale", "{row}");
    assert_eq!(row["issue"], 12, "a stale session was hidden: {row}");

    let (said, _, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "end",
            "--session-id",
            "alive",
            "--runtime",
            "pi",
            "--reason",
            "quit",
        ],
    );
    assert!(ok, "{said}");
    let listed = listed_sessions(&rig);
    assert_eq!(session_named(&listed, &alive)["status"], "ended");

    let (plain, _, ok) = pi_session_run(&rig, &["sessions", "--all"]);
    assert!(
        ok && plain.contains(&crashed) && plain.contains("stale") && plain.contains("#12"),
        "{plain}"
    );
}

#[test]
fn sessions_on_a_machine_that_never_registered_one_lists_none_and_creates_no_store() {
    let rig = tracker_rig();
    let (said, _, ok) = pi_session_run(&rig, &["sessions", "--json"]);
    assert!(ok, "{said}");
    let parsed: serde_json::Value = serde_json::from_str(said.trim()).expect("JSON");
    assert_eq!(parsed["sessions"], serde_json::json!([]));
    assert!(
        !rig.home.path().join(".estigia").join("state.db").exists(),
        "a read created the store"
    );
}

#[test]
fn the_cli_walks_a_bounded_session_registry_a_page_at_a_time_without_omissions_or_duplicates() {
    let rig = tracker_rig();
    let mut registered = Vec::new();
    for session in ["one", "two", "three", "four", "five"] {
        registered.push(pi_start(&rig, session));
    }
    registered.sort();

    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "--json"]);
    assert!(ok, "{said}{stderr}");
    let listed: serde_json::Value = serde_json::from_str(said.trim()).expect("sessions is JSON");
    assert_eq!(listed["of"], 5, "{listed}");
    assert_eq!(listed["all"], false, "{listed}");
    let rows = listed["sessions"].as_array().expect("a list").clone();
    assert_eq!(rows.len(), 5, "{rows:?}");
    let mut listed_ids: Vec<String> = rows
        .iter()
        .map(|row| row["run_id"].as_str().expect("a run id").to_owned())
        .collect();
    listed_ids.sort();
    assert_eq!(
        listed_ids, registered,
        "the unbounded list is not the registry it was taken from"
    );

    let (legacy, stderr, ok) = pi_session_run(&rig, &["sessions"]);
    assert!(ok, "{legacy}{stderr}");
    let mut said = legacy.lines();
    for row in &rows {
        let silence = row["silent_for"].as_u64().expect("a silence in seconds");
        assert!(
            silence < 60,
            "this listing was taken more than a minute after the fixture registered, so the age \
             bucket moved under the expectation: {row}"
        );
        let line = said.next().expect("a row line");
        assert_eq!(
            line,
            format!(
                "{}  {}  live  pi  -  last seen {}",
                row["name"].as_str().expect("a name"),
                row["run_id"].as_str().expect("a run id"),
                estigia::harness::session::age(silence),
            ),
            "the list without a page flag changed its fields"
        );
    }
    assert_eq!(
        said.next(),
        Some("5 of 5 rows are live; `estigia sessions --all` shows the rest"),
        "the list without a page flag changed its footer"
    );
    assert_eq!(
        said.next(),
        None,
        "the list without a page flag gained a line"
    );

    let mut walked: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut arguments = ["sessions", "--json", "--limit", "2"]
            .map(str::to_owned)
            .to_vec();
        if let Some(token) = &cursor {
            arguments.push("--cursor".to_owned());
            arguments.push(token.clone());
        }
        let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
        let (said, stderr, ok) = pi_session_run(&rig, &borrowed);
        assert!(ok, "{said}{stderr}");
        let page: serde_json::Value = serde_json::from_str(said.trim()).expect("a page is JSON");
        assert_eq!(
            serde_json::to_string(&page).expect("the page serializes"),
            said.trim(),
            "the page is not the compact JSON the tool's caller sees"
        );
        assert_eq!(page["version"], 1, "{page}");
        assert_eq!(page["limit"], 2, "{page}");
        assert_eq!(page["matched"], 5, "the snapshot moved: {page}");
        let rows = page["sessions"].as_array().expect("a list of rows");
        assert_eq!(page["returned"], rows.len(), "{page}");
        assert!(!rows.is_empty(), "a page carried no rows");
        walked.extend(
            rows.iter()
                .map(|row| row["run_id"].as_str().expect("a run id").to_owned()),
        );
        pages += 1;
        match page["next_cursor"].as_str() {
            Some(token) => cursor = Some(token.to_owned()),
            None => break,
        }
    }
    assert_eq!(pages, 3, "the page size did not hold: {walked:?}");
    assert_eq!(
        walked.len(),
        registered.len(),
        "the walk omitted a row: {walked:?}"
    );
    let mut unique = walked.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        walked.len(),
        "the walk repeated a row: {walked:?}"
    );
    assert_eq!(unique, registered, "the walk is not the registry");

    let mut prose: Vec<String> = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut arguments = ["sessions", "--limit", "2"].to_vec();
        if let Some(token) = &token {
            arguments.push("--cursor");
            arguments.push(token);
        }
        let (said, stderr, ok) = pi_session_run(&rig, &arguments);
        assert!(ok, "{said}{stderr}");
        let lines: Vec<&str> = said.lines().collect();
        let (footer, shown) = lines.split_last().expect("a footer");
        for line in shown {
            let run_id = line.split("  ").nth(1).expect("a run id on the row");
            assert!(
                registered.iter().any(|known| known == run_id),
                "a page showed a row the registry does not hold: {line}"
            );
            prose.push(run_id.to_owned());
        }
        match footer.strip_prefix("2 of 5 row(s) on this page; more remain — the next cursor is ")
        {
            Some(carried) => token = Some(carried.to_owned()),
            None => {
                assert_eq!(
                    *footer, "1 of 5 row(s) on this page; nothing more remains",
                    "the last page's footer does not say the walk is over"
                );
                break;
            }
        }
    }
    assert_eq!(prose, walked, "the prose pages and the JSON pages differ");
}

#[test]
fn a_bounded_session_call_keeps_each_refusal_the_page_answers() {
    let rig = tracker_rig();
    for session in ["one", "two", "three"] {
        pi_start(&rig, session);
    }
    let (said, stderr, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "end",
            "--session-id",
            "three",
            "--runtime",
            "pi",
            "--reason",
            "quit",
        ],
    );
    assert!(ok, "{said}{stderr}");
    let refused = |arguments: &[&str]| -> String {
        let (said, stderr, ok) = pi_session_run(&rig, arguments);
        assert!(!ok, "the call was answered: {said}{stderr}");
        format!("{said}{stderr}")
    };

    let above = refused(&["sessions", "--limit", "501"]);
    assert!(above.contains("sessions-limit-too-large"), "{above}");
    let zero = refused(&["sessions", "--limit", "0"]);
    assert!(zero.contains("--limit"), "{zero}");

    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "--json", "--limit", "1"]);
    assert!(ok, "{said}{stderr}");
    let page: serde_json::Value = serde_json::from_str(said.trim()).expect("JSON");
    assert_eq!(
        page["matched"], 2,
        "a page without `--all` did not ask for the live rows alone: {page}"
    );
    let token = page["next_cursor"].as_str().expect("a cursor").to_owned();

    let (payload, digest) = token.split_once('.').expect("a payload and a digest");
    let mut flipped = payload.to_owned();
    let first = flipped.as_bytes()[0];
    flipped.replace_range(0..1, if first == b'a' { "b" } else { "a" });
    let tampered = format!("{flipped}.{digest}");
    let edited = refused(&["sessions", "--limit", "1", "--cursor", &tampered]);
    assert!(edited.contains("sessions-cursor-tampered"), "{edited}");

    let mismatched = refused(&["sessions", "--all", "--limit", "1", "--cursor", &token]);
    assert!(
        mismatched.contains("sessions-cursor-mismatch"),
        "{mismatched}"
    );

    let malformed = refused(&["sessions", "--cursor", "not-a-cursor"]);
    assert!(
        malformed.contains("sessions-cursor-malformed"),
        "{malformed}"
    );

    let (said, stderr, ok) = pi_session_run(
        &rig,
        &["sessions", "--json", "--limit", "2", "--cursor", &token],
    );
    assert!(ok, "a page size may change mid-walk: {said}{stderr}");
    let page: serde_json::Value = serde_json::from_str(said.trim()).expect("JSON");
    assert_eq!(
        page["returned"], 1,
        "the second page did not finish the walk: {page}"
    );
    assert_eq!(page["next_cursor"], serde_json::Value::Null, "{page}");

    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "--all", "--json", "--limit", "1"]);
    assert!(ok, "{said}{stderr}");
    let page: serde_json::Value = serde_json::from_str(said.trim()).expect("JSON");
    assert_eq!(
        page["matched"], 3,
        "`--all` did not ask for every status: {page}"
    );
}

#[test]
fn a_sessions_send_refuses_every_listing_flag_before_it_writes() {
    let rig = tracker_rig();
    let recipient = pi_start(&rig, "recipient");
    let waiting = |rig: &TrackerRig| -> u64 {
        session_named(&listed_sessions(rig), &recipient)["waiting"]
            .as_u64()
            .expect("a waiting count")
    };

    for flag in [
        vec!["--all"],
        vec!["--limit", "5"],
        vec!["--cursor", "not-a-cursor"],
    ] {
        let mut arguments = vec!["sessions"];
        arguments.extend(flag.iter().copied());
        arguments.extend(["send", recipient.as_str(), "wake up"]);
        let (said, stderr, ok) = pi_session_run(&rig, &arguments);
        let refused = format!("{said}{stderr}");
        assert!(
            !ok,
            "`{}` beside `send` delivered the message: {refused}",
            flag[0]
        );
        assert!(
            refused.contains("sessions-send-takes-no-listing-flag"),
            "`{}` beside `send` answered something else: {refused}",
            flag[0]
        );
        assert!(
            refused.contains(flag[0]) && refused.contains("takes no listing flag"),
            "the refusal does not name the flag it refused and the rule: {refused}"
        );
        assert_eq!(
            waiting(&rig),
            0,
            "`{}` beside `send` wrote the message anyway",
            flag[0]
        );
    }

    let (said, stderr, ok) = pi_session_run(
        &rig,
        &[
            "sessions", "--all", "--limit", "5", "send", &recipient, "wake up",
        ],
    );
    let refused = format!("{said}{stderr}");
    assert!(!ok, "two listing flags beside `send` delivered: {refused}");
    assert!(
        refused.contains("--all, --limit") && refused.contains("were given"),
        "the refusal does not name both flags it refused: {refused}"
    );
    assert_eq!(
        waiting(&rig),
        0,
        "two listing flags wrote the message anyway"
    );

    let (said, stderr, ok) =
        pi_session_run(&rig, &["sessions", "send", &recipient, "wake up", "--all"]);
    assert!(
        !ok,
        "a listing flag after `send` was accepted: {said}{stderr}"
    );
    assert_eq!(
        waiting(&rig),
        0,
        "a trailing listing flag wrote the message"
    );

    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "send", &recipient, "wake up"]);
    assert!(
        ok,
        "the same call without a listing flag did not deliver: {said}{stderr}"
    );
    assert_eq!(
        waiting(&rig),
        1,
        "the send did not put the row in the mailbox"
    );

    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "--all", "--limit", "5"]);
    assert!(ok, "`--all` no longer lists: {said}{stderr}");
    assert!(
        said.contains(&recipient),
        "the listing door dropped the registered row: {said}"
    );
}

#[test]
fn a_bounded_session_page_answers_an_empty_registry_and_offers_no_cursor_with_nothing_left() {
    let idle = tracker_rig();
    let (said, stderr, ok) = pi_session_run(&idle, &["sessions", "--limit", "5"]);
    assert!(ok, "{said}{stderr}");
    assert_eq!(
        said.trim(),
        "0 of 0 row(s) on this page; nothing more remains",
        "an empty page said something else"
    );

    let one = tracker_rig();
    let only = pi_start(&one, "only");
    let (said, stderr, ok) = pi_session_run(&one, &["sessions", "--json", "--limit", "1"]);
    assert!(ok, "{said}{stderr}");
    let page: serde_json::Value = serde_json::from_str(said.trim()).expect("JSON");
    assert_eq!(page["returned"], 1, "{page}");
    assert_eq!(
        page["sessions"][0]["run_id"].as_str(),
        Some(only.as_str()),
        "the one row is not the one registered: {page}"
    );
    assert_eq!(
        page["more"], false,
        "a page with nothing after it said more remain: {page}"
    );
    assert!(
        page.get("next_cursor").is_some(),
        "a last page dropped `next_cursor` instead of spelling it null: {page}"
    );
    assert_eq!(
        page["next_cursor"],
        serde_json::Value::Null,
        "a page with nothing after it handed out a cursor: {page}"
    );
}

fn pi_start(rig: &TrackerRig, session: &str) -> String {
    let (said, _, ok) = pi_session_run(
        rig,
        &[
            "session",
            "start",
            "--session-id",
            session,
            "--runtime",
            "pi",
        ],
    );
    assert!(ok, "{said}");
    estigia::harness::session::run_id("pi", session)
}

fn send_from(rig: &TrackerRig, sender: &str, to: &str, text: &str) -> (String, String, bool) {
    pi_session_run(
        rig,
        &[
            "sessions",
            "send",
            to,
            text,
            "--session-id",
            sender,
            "--runtime",
            "pi",
        ],
    )
}

fn deliver_to(rig: &TrackerRig, session: &str) -> String {
    let (said, stderr, ok) = pi_session_run(
        rig,
        &[
            "session",
            "deliver",
            "--session-id",
            session,
            "--runtime",
            "pi",
        ],
    );
    assert!(ok, "{said}{stderr}");
    said
}

#[test]
fn a_sent_message_waits_visibly_for_both_ends_until_the_recipient_takes_it_once() {
    let rig = tracker_rig();
    let alice = pi_start(&rig, "alice");
    let bob = pi_start(&rig, "bob");

    let (said, stderr, ok) = send_from(&rig, "alice", &bob, "please look at the migration");
    assert!(ok, "{said}{stderr}");

    let listed = listed_sessions(&rig);
    assert_eq!(session_named(&listed, &bob)["waiting"], 1, "{listed:?}");
    assert_eq!(
        session_named(&listed, &alice)["undelivered_sent"],
        1,
        "{listed:?}"
    );

    let delivered = deliver_to(&rig, "bob");
    assert!(
        delivered.contains("please look at the migration") && delivered.contains(&alice),
        "{delivered}"
    );
    let listed = listed_sessions(&rig);
    assert_eq!(session_named(&listed, &bob)["waiting"], 0, "{listed:?}");
    assert_eq!(
        session_named(&listed, &alice)["undelivered_sent"],
        0,
        "{listed:?}"
    );
    assert_eq!(
        deliver_to(&rig, "bob").trim(),
        "",
        "a message was delivered twice"
    );
}

#[test]
fn a_peeked_message_stays_pending_until_it_is_taken_through_the_id_that_was_read() {
    let rig = tracker_rig();
    pi_start(&rig, "alice");
    let bob = pi_start(&rig, "bob");
    let (said, stderr, ok) = send_from(&rig, "alice", &bob, "wake up");
    assert!(ok, "{said}{stderr}");

    let deliver = |extra: &[&str]| {
        let mut args = vec![
            "--json",
            "session",
            "deliver",
            "--session-id",
            "bob",
            "--runtime",
            "pi",
        ];
        args.extend_from_slice(extra);
        let (said, stderr, ok) = pi_session_run(&rig, &args);
        assert!(ok, "{said}{stderr}");
        serde_json::from_str::<serde_json::Value>(&said).expect("json")
    };

    let peeked = deliver(&["--peek"]);
    assert!(
        peeked["messages"]
            .as_str()
            .unwrap_or("")
            .contains("wake up"),
        "{peeked}"
    );
    let through = peeked["through"].as_i64().expect("an id").to_string();
    assert_eq!(deliver(&["--peek"]), peeked, "peeking stamped the message");
    let listed = listed_sessions(&rig);
    assert_eq!(session_named(&listed, &bob)["waiting"], 1, "{listed:?}");

    deliver(&["--through", &through]);
    let listed = listed_sessions(&rig);
    assert_eq!(session_named(&listed, &bob)["waiting"], 0, "{listed:?}");
    assert_eq!(deliver(&["--peek"])["messages"], serde_json::Value::Null);
}

#[test]
fn a_message_for_a_session_nobody_registered_is_refused_and_records_nothing() {
    let rig = tracker_rig();
    pi_start(&rig, "alice");
    let (said, stderr, ok) = send_from(&rig, "alice", "pi-ghost", "hello?");
    assert!(!ok, "{said}");
    assert!(stderr.contains("message-no-live-session"), "{stderr}");
    let listed = listed_sessions(&rig);
    assert!(
        listed
            .iter()
            .all(|row| row["waiting"] == 0 && row["undelivered_sent"] == 0),
        "{listed:?}"
    );
}

#[test]
fn a_message_for_an_ended_session_is_kept_and_stays_visible_to_its_sender() {
    let rig = tracker_rig();
    let alice = pi_start(&rig, "alice");
    let bob = pi_start(&rig, "bob");
    let (said, _, ok) = pi_session_run(
        &rig,
        &[
            "session",
            "end",
            "--session-id",
            "bob",
            "--runtime",
            "pi",
            "--reason",
            "quit",
        ],
    );
    assert!(ok, "{said}");

    let (said, stderr, ok) = send_from(&rig, "alice", &bob, "too late");
    assert!(ok, "{said}{stderr}");
    assert!(said.contains("has ended"), "{said}");
    let listed = listed_sessions(&rig);
    assert_eq!(
        session_named(&listed, &alice)["undelivered_sent"],
        1,
        "{listed:?}"
    );
}

#[test]
fn an_empty_message_and_a_message_longer_than_a_row_holds_are_refused() {
    let rig = tracker_rig();
    pi_start(&rig, "alice");
    let bob = pi_start(&rig, "bob");
    let (_, stderr, ok) = send_from(&rig, "alice", &bob, "   ");
    assert!(!ok && stderr.contains("message-empty"), "{stderr}");
    let (_, stderr, ok) = send_from(&rig, "alice", &bob, &"x".repeat(9000));
    assert!(!ok && stderr.contains("message-too-long"), "{stderr}");
}

fn hook_payload(session: &str, repo: &std::path::Path, tool: &str, extra: &str) -> String {
    format!(
        "{{\"session_id\":\"{session}\",\"cwd\":\"{}\",\"tool_name\":\"{tool}\",\
         \"tool_input\":{{\"file_path\":\"{}\"}}{extra}}}",
        repo.display().to_string().replace('\\', "\\\\"),
        repo.join("scratch.txt")
            .display()
            .to_string()
            .replace('\\', "\\\\")
    )
}

#[test]
fn the_claude_code_mail_waiter_exits_two_with_the_message_once_and_zero_when_it_has_no_one_to_serve()
 {
    let rig = tracker_rig();
    let recipient = estigia::harness::session::run_id("claude", "recipient-1");
    let waiter = |payload: &str| {
        run_with_tracker(
            rig.home.path(),
            rig.repo.path(),
            rig.bin.path(),
            "[]",
            &[
                "hook",
                "stop",
                "--agent",
                "claude-code",
                "--dialect",
                "claude-code",
                "--wait-mail",
            ],
            payload,
        )
    };
    let (_, _, ok) = run_with_tracker(
        rig.home.path(),
        rig.repo.path(),
        rig.bin.path(),
        "[]",
        &["hook", "session-start", "--agent", "claude-code"],
        &format!(
            "{{\"session_id\":\"recipient-1\",\"cwd\":\"{}\"}}",
            rig.repo.path().display().to_string().replace('\\', "\\\\")
        ),
    );
    assert!(ok);
    let (said, stderr, ok) = pi_session_run(&rig, &["sessions", "send", &recipient, "wake up"]);
    assert!(ok, "{said}{stderr}");

    let cwd = rig.repo.path().display().to_string().replace('\\', "\\\\");
    let (out, stderr, ok) = waiter(&format!(
        "{{\"session_id\":\"recipient-1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\"}}"
    ));
    assert!(ok && out.is_empty() && stderr.is_empty(), "{out}{stderr}");
    let (out, stderr, ok) = waiter("{\"cwd\":\"/\"}");
    assert!(ok && out.is_empty() && stderr.is_empty(), "{out}{stderr}");
    assert_eq!(
        session_named(&listed_sessions(&rig), &recipient)["waiting"],
        1
    );

    let own = format!("{{\"session_id\":\"recipient-1\",\"cwd\":\"{cwd}\"}}");
    let (out, stderr, ok) = waiter(&own);
    assert!(
        !ok && out.is_empty(),
        "exit 2 was not the answer: {out}{stderr}"
    );
    assert!(stderr.contains("wake up"), "{stderr}");
    assert_eq!(
        session_named(&listed_sessions(&rig), &recipient)["waiting"],
        0
    );
}

#[test]
fn claude_code_and_codex_hooks_hand_a_waiting_message_to_the_agent_once() {
    for agent in ["claude-code", "codex"] {
        let rig = tracker_rig();
        let runtime = estigia::harness::session::runtime_of_agent(Some(agent));
        let recipient = estigia::harness::session::run_id(runtime, "recipient-1");
        let hook = |event: &str, payload: &str| {
            run_with_tracker(
                rig.home.path(),
                rig.repo.path(),
                rig.bin.path(),
                "[]",
                &["hook", event, "--agent", agent, "--dialect", "claude-code"],
                payload,
            )
        };
        let (_, _, ok) = hook(
            "session-start",
            &format!(
                "{{\"session_id\":\"recipient-1\",\"cwd\":\"{}\"}}",
                rig.repo.path().display().to_string().replace('\\', "\\\\")
            ),
        );
        assert!(ok);
        let listed = listed_sessions(&rig);
        assert_eq!(
            session_named(&listed, &recipient)["runtime"],
            runtime,
            "{agent}: the hook did not register the session: {listed:?}"
        );

        let (said, stderr, ok) = pi_session_run(
            &rig,
            &["sessions", "send", &recipient, "rebase before you push"],
        );
        assert!(ok, "{said}{stderr}");

        let (sub_agent, _, _) = hook(
            "pre-tool-use",
            &hook_payload(
                "recipient-1",
                rig.repo.path(),
                "Read",
                ",\"agent_type\":\"reader\"",
            ),
        );
        assert!(
            !sub_agent.contains("rebase before you push"),
            "{agent}: a sub-agent's call took the session's mail: {sub_agent}"
        );

        let (first, _, _) = hook(
            "pre-tool-use",
            &hook_payload("recipient-1", rig.repo.path(), "Read", ""),
        );
        let parsed: serde_json::Value = serde_json::from_str(first.trim()).expect("JSON");
        let context = parsed["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_else(|| panic!("{agent}: no additionalContext: {first}"));
        assert!(
            context.contains("rebase before you push"),
            "{agent}: {context}"
        );
        assert_eq!(
            parsed["hookSpecificOutput"]["hookEventName"], "PreToolUse",
            "{agent}: {first}"
        );
        assert!(
            parsed["hookSpecificOutput"]
                .get("permissionDecision")
                .is_none(),
            "{agent}: the mail decided a permission: {first}"
        );

        let (second, _, _) = hook(
            "pre-tool-use",
            &hook_payload("recipient-1", rig.repo.path(), "Read", ""),
        );
        assert_eq!(second.trim(), "{}", "{agent}: delivered twice: {second}");
        let listed = listed_sessions(&rig);
        assert_eq!(
            session_named(&listed, &recipient)["waiting"],
            0,
            "{agent}: {listed:?}"
        );
    }
}
