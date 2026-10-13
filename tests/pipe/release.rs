use super::*;

#[test]
fn a_blank_run_id_is_refused_before_anything_is_read() {
    // `clap` checks that a flag is present, not that it says anything, so
    // `--run-id ""` satisfied it and went all the way to the tracker. What it
    // would have recorded is a claim under an empty id — exactly "a claim the
    // gate will never match", which is the reason that flag's own help text
    // gives for asking rather than guessing.
    //
    // Run as a process because the fault is `clap`'s to raise, before any of
    // this crate's code sees the arguments: the rule the rest of it states as
    // *everything knowable from the command line is settled before anything on
    // disk is read*.
    let home = tempfile::tempdir().expect("a temporary home");
    for blank in ["", "   "] {
        for arguments in [
            vec![
                "claim",
                "1",
                "--run-id",
                blank,
                "--horizon",
                "2026-12-31T23:00Z",
            ],
            vec!["release", "--run-id", blank],
            vec!["gate", "Write", "--run-id", blank],
        ] {
            let verb = arguments[0];
            let (_, stderr, ok) = run(home.path(), &arguments, "");
            assert!(!ok, "`{verb}` took a blank run id");
            assert!(
                stderr.contains("cannot be blank"),
                "`{verb}` refused a blank run id for some other reason: {stderr}"
            );
            // And it says where a real one comes from, because the operator is
            // usually an agent that was handed one at `SessionStart`.
            assert!(
                stderr.contains("SessionStart"),
                "`{verb}` does not say where a run id comes from: {stderr}"
            );
        }
    }
}

#[test]
fn a_run_pointer_nobody_can_read_is_not_a_run_that_holds_nothing() {
    // `session::load` keeps these apart on purpose — a pointer that is present
    // and unparseable sets `unreadable`, because "an unknown is not clearance"
    // and the gate has to fail closed on it. `release` read them back as one
    // and answered `holds no issue`, which is a statement of fact about
    // something Estigia does not know: the tracker may still show the issue
    // held by that run, and an operator told there was nothing to put down
    // would leave it there.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // A run that never swore anything.
    let (_, stderr, ok) = run(home.path(), &["release", "--run-id", "never-claimed"], "");
    assert!(!ok);
    assert!(
        stderr.contains("nothing-held"),
        "a run with no pointer is not an unreadable one: {stderr}"
    );

    // A run whose pointer is there and will not parse.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    std::fs::write(runs.join("broken.json"), "not json").expect("a damaged pointer");
    let (_, stderr, ok) = run(home.path(), &["release", "--run-id", "broken"], "");
    assert!(!ok);
    assert!(
        stderr.contains("run-pointer-unreadable"),
        "an unreadable pointer was reported as nothing held: {stderr}"
    );
    // It names the file, because removing it is the operator's move and they
    // cannot make it without knowing which one.
    assert!(
        stderr.contains("broken.json"),
        "it does not say which pointer: {stderr}"
    );
    // And no command, because none of Estigia's would discharge it.
    assert!(
        !stderr.contains("run: estigia"),
        "it names a command that changes nothing: {stderr}"
    );
}

/// The run `status` names is the run `release` answers to.
///
/// A pointer whose file name and whose `run_id` disagree is one `session::load`
/// already calls unreadable — deliberately, because a pointer that does not
/// name the run it is filed under is not one that run may act on. `holdings`
/// read the same directory by **content** and did not ask, so `status` listed
/// the identity written inside.
///
/// The two then answered differently about the same run, and `status`'s own
/// hint made it worse: it says `estigia release --run-id <id>` puts one down,
/// beside an id that command answers *holds no issue* for. A message naming a
/// command that does not discharge is the one thing the ratchet forbids, and
/// this one names it with the argument filled in.
#[test]
fn the_run_status_names_is_the_run_release_answers_to() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("a state directory");
    // The floor: a pointer whose name and content agree is listed, so this is
    // not a change that stops `status` reporting holdings.
    std::fs::write(
        runs.join("claude-aaaa1111.json"),
        r#"{"run_id":"claude-aaaa1111","issue":12,"verified_at":1785988000}"#,
    )
    .expect("an honest pointer");
    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        said.contains("claude-aaaa1111"),
        "an ordinary holding stopped being reported: {said}"
    );

    // And one that disagrees with itself.
    std::fs::write(
        runs.join("claude-bbbb2222.json"),
        r#"{"run_id":"claude-cccc3333","issue":99,"verified_at":1785988000}"#,
    )
    .expect("a pointer that disagrees with itself");
    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        !said.contains("claude-cccc3333"),
        "`status` names a run that no command answers to: {said}"
    );
    assert!(
        said.contains("cannot be read"),
        "the pointer that disagrees with itself is not reported at all: {said}"
    );
}

/// `release` does not report a release the transport did not perform.
///
/// `unassign` is two calls. The first is **discovery**: it answers
/// `write_performed: false` with the epoch it found and *"repeat unassign with
/// --target-operation and the same operation ID"*. This verb threw that answer
/// away and printed `released: <run> no longer holds #<issue>`.
///
/// Measured on the installed binary: after that sentence the pointer still held
/// the issue — correctly, the harness reads `write_performed` and keeps it — and
/// the next write went through with `allow — issue #12 was verified inside the
/// renewal window`. The gate was right and the message was wrong, in the one
/// command whose whole job is putting a claim down.
#[test]
fn release_does_not_report_a_release_the_transport_did_not_perform() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join("claude-abcd1234.json");
    std::fs::write(
        &pointer,
        serde_json::to_string(&serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": "in-progress",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
        }))
        .expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // A world that answers the timeline and nothing else, so the commit half of
    // the release cannot land. What must not happen is being told it did.
    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &["release", "--run-id", "claude-abcd1234"],
        "",
    );
    let told = format!("{said}{err}");
    assert!(
        !told.contains("no longer holds"),
        "a release the transport did not perform was reported as done:\n{told}"
    );
    assert!(
        !ok,
        "a release that did not happen exited as though it had:\n{told}"
    );

    // And the claim is still held, which is the half the gate got right all
    // along — so the message was the only thing lying.
    let after = std::fs::read_to_string(&pointer).expect("the pointer survives");
    assert!(
        after.contains("\"issue\":12") || after.contains("\"issue\": 12"),
        "the pointer stopped holding the issue nobody released:\n{after}"
    );
}

/// The phantom pointer the reconciliation was filed against, cleared end to end: a run
/// pointer on disk naming an issue the tracker's own timeline has no
/// acquisition event for — the shape `several-runs-hold-this-checkout` names
/// `estigia release --run-id <run-id>` to clear, and the one D4(b) actually
/// had to reach.
///
/// `plan_release` raises `nothing-to-unassign` for exactly this timeline —
/// this run appears in neither `ownership.live` nor `ownership.stale` — and
/// that refusal used to propagate out of `release` unhandled: the pointer
/// stayed on disk, and the command the several-holder refusal names did not
/// clear the state it described. `release`'s discovery call now recognises
/// that one code and forgets the pointer instead of repeating the refusal.
#[test]
fn releasing_a_phantom_pointer_forgets_it_and_says_so() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join("claude-phantom4a2b1c3d.json");
    std::fs::write(
        &pointer,
        serde_json::to_string(&serde_json::json!({
            "run_id": "claude-phantom4a2b1c3d",
            "issue": 12,
            "revision": 1,
            "state": "in-progress",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
        }))
        .expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer_nobody_claimed(),
        &["release", "--run-id", "claude-phantom4a2b1c3d"],
        "",
    );
    let told = format!("{said}{err}");
    assert!(ok, "clearing a phantom pointer was refused:\n{told}");
    assert!(
        told.contains("no epoch") && told.contains("forgotten"),
        "the command did not say what it did to the pointer:\n{told}"
    );
    assert!(
        !pointer.exists(),
        "the pointer the tracker holds no epoch for is still on disk after release"
    );
}

/// The negative half: a tracker read that fails must leave the pointer alone.
///
/// `nothing-to-unassign` is one specific, successful answer from the
/// timeline — not the shape of every refusal `unassign` can raise. A `gh`
/// that will not run answers `read-failed` instead, which `release` returns
/// untouched, and the local pointer — the one piece of state a person can
/// still read when the tracker cannot be reached at all — has to survive
/// that exactly as it did before D4(b).
#[test]
fn a_release_the_tracker_could_not_be_asked_about_forgets_nothing() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join("claude-phantom5a2b1c3d.json");
    std::fs::write(
        &pointer,
        serde_json::to_string(&serde_json::json!({
            "run_id": "claude-phantom5a2b1c3d",
            "issue": 34,
            "revision": 1,
            "state": "in-progress",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
        }))
        .expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // The fake process's own catch-all: no entry matches, so `gh` exits `0`
    // with nothing on stdout, and `gh issue view` returning nothing is itself
    // a read failure — `gh_json` refuses `None` bodies as unparseable rather
    // than reading absence as an answer.
    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        "[]",
        &["release", "--run-id", "claude-phantom5a2b1c3d"],
        "",
    );
    let told = format!("{said}{err}");
    assert!(
        !ok,
        "a release over an unreachable tracker was not refused:\n{told}"
    );
    assert!(
        !told.contains("forgotten") && !told.contains("no longer holds"),
        "a failed read was reported as though something had been cleared:\n{told}"
    );
    assert!(
        pointer.exists(),
        "a read that failed still took the pointer away:\n{told}"
    );
}

/// Final elected round: `run_tool`'s `unassign` sender pushes
/// `--runtime` from the validated `NamedRun.runtime`
/// (`run_tool` in `src/harness/mcp/mod.rs`) the same way the claim sender does — pinned
/// here the same way, over the comment the release write actually posts.
/// `ESTIGIA_FAKE_BODY_LOG` is what makes the body legible: every write path in
/// `claim.rs` deletes its staged file the instant `gh` returns, so nothing
/// outside the call itself ever saw what it said until `fake_process` started
/// copying it out first.
#[test]
fn a_non_claude_run_id_releases_under_its_own_runtime_not_the_constant() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let bodies = tempfile::tempdir().expect("a body-log directory");
    let body_log = bodies.path().join("bodies.log");
    let run_id = "codex-abcd1234";

    let pointer = home
        .join(".estigia")
        .join("runs")
        .join(format!("{run_id}.json"));
    std::fs::create_dir_all(pointer.parent().expect("the runs directory")).expect("it exists");
    std::fs::write(
        &pointer,
        serde_json::to_string(&serde_json::json!({
            "run_id": run_id,
            "issue": 12,
            "revision": 1,
            "state": "in-progress",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
        }))
        .expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    let marker = estigia::transport::markers::render(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "codex"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &"a".repeat(32)),
        ],
    )
    .expect("a claim marker");
    let held = serde_json::json!({
        "state": "OPEN",
        "assignees": [{"login": "fixture"}],
        "labels": [{"name": "status:in-progress"}, {"name": "dev:codex"}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
        }],
    });
    let script = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": held.to_string(), "status": 0 },
    ]))
    .expect("the script serialises");

    let output = tracker_command(home, repo, bin, &script)
        .env("ESTIGIA_FAKE_BODY_LOG", &body_log)
        .args(["release", "--run-id", run_id])
        .output()
        .expect("estigia release runs");
    let _ = output;

    let posted = std::fs::read_to_string(&body_log).unwrap_or_default();
    assert!(posted.is_empty(), "a release posts no comment: {posted}");
    let stored = local_entries(home)
        .into_iter()
        .filter(|body| body.contains("issue-flow: unassign"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stored.contains("runtime=codex"),
        "a non-Claude run id's own release did not carry its own runtime: {stored}"
    );
    assert!(
        !stored.contains("runtime=claude"),
        "a non-Claude run id's release recorded the constant instead of its own runtime: {stored}"
    );
}
