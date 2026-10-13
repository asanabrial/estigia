use super::*;

/// `gh pr merge` is judged by the PR head the tracker reports, not by the
/// invoking checkout's own `HEAD`: the launch checkout here never moves off
/// its own real head, which differs from the reviewed one, and the delivery
/// is still allowed once the PR head the tracker reports matches the receipt.
#[test]
fn a_gh_pr_merge_is_judged_by_the_pr_head_not_the_launch_checkouts_head() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let reviewed = "e".repeat(40);
    assert_ne!(
        reviewed, rig.head,
        "the synthetic reviewed head collides with the checkout's real head"
    );

    let pointer = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "revision": 1,
        "state": "review",
        "repo_dir": repo,
        "worktree": serde_json::Value::Null,
        "review_receipt": {
            "epoch": "a".repeat(32),
            "pr": 12,
            "head": reviewed,
            "base": "b".repeat(40),
            "digest": "c".repeat(64),
        },
        "reviewed_head": serde_json::Value::Null,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&pointer).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    let merge = |answers: &str| {
        let (out, err, _) = run_with_tracker(
            home,
            repo,
            bin,
            answers,
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                r#"{"command":"gh pr merge 12"}"#,
            ],
            "",
        );
        format!("{out}{err}")
    };

    // The PR head the tracker reports matches the reviewed one: allowed, even
    // though `repo` — the launch checkout every call above runs from — never
    // moves off its own real head, which differs from the reviewed one.
    let allowed = merge(
        &serde_json::to_string(&serde_json::json!([
            pr_view_entry(12, &reviewed),
            issue_view_entry("review"),
        ]))
        .expect("the script serialises"),
    );
    assert!(
        allowed.contains("allow"),
        "a PR head matching the reviewed one was refused: {allowed}"
    );

    // The PR head the tracker reports has moved: refused, naming the reviewed
    // head and the PR's new head.
    let moved = "d".repeat(40);
    let refused = merge(
        &serde_json::to_string(&serde_json::json!([
            pr_view_entry(12, &moved),
            issue_view_entry("review"),
        ]))
        .expect("the script serialises"),
    );
    assert!(
        refused.contains("verdict-bound-to-other-bytes"),
        "a PR head that moved past the reviewed one was allowed: {refused}"
    );
    assert!(
        refused.contains(&moved[..7]) && refused.contains(&reviewed[..7]),
        "the refusal does not name both heads: {refused}"
    );

    // `gh pr view` itself failed: refused with the transport's own unreadable
    // read, never `verdict-bound-to-other-bytes`.
    let unreadable = merge(
        &serde_json::to_string(&serde_json::json!([
            { "matches": "pr view 12", "status": 1, "stderr": "gh: pull request not found" },
            issue_view_entry("review"),
        ]))
        .expect("the script serialises"),
    );
    assert!(
        !unreadable.contains("verdict-bound-to-other-bytes"),
        "an unreadable PR head was still bound to other bytes: {unreadable}"
    );
    assert!(
        unreadable.contains("read-failed"),
        "an unreadable PR head was not refused as a failed read: {unreadable}"
    );
}

/// The floor a compound line's classification is measured against: a bare
/// `git push` from `in-progress` still answers the way it does today. Without
/// this floor, every assertion below could pass with nothing left to
/// distinguish a real fix from one that refuses everything.
#[test]
fn a_bare_push_from_in_progress_still_answers_as_today() {
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
    let (out, err, _) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Bash",
            "--json",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"git push"}"#,
        ],
        "",
    );
    let answered = format!("{out}{err}");
    assert!(
        answered.contains("\"decision\": \"allow\""),
        "a bare push from in-progress stopped riding the answer it always has: {answered}"
    );
}

/// `git push && gh pr merge 66` used to record only `git push`, which is not
/// on `DELIVERS` — so the whole apparatus that refuses a delivery was skipped
/// entirely and the merge went through. Ranked on the strictest fragment
/// present, this line now records `gh pr merge`, which reaches
/// `delivery-pr-unidentified`: the compound line disqualifies the literal
/// parser (the `&` byte) before any pull-request identity can be read, so the
/// question `stale_verdict` answers is not "does this line deliver" but
/// "which receipt does it name" — and it can name none.
#[test]
fn a_push_and_a_pr_merge_on_one_line_is_refused_from_in_progress() {
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
    let (out, err, _) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Bash",
            "--json",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"git push && gh pr merge 66"}"#,
        ],
        "",
    );
    let answered = format!("{out}{err}");
    assert!(
        !answered.contains("\"decision\": \"allow\""),
        "a push carrying a compound pull-request merge was allowed from in-progress: {answered}"
    );
    assert!(
        answered.contains("delivery-pr-unidentified"),
        "the compound push-then-merge line was refused for the wrong reason: {answered}"
    );
}

/// `git merge origin/main && gh pr merge 66` from `review`, holding a complete
/// receipt for pull request 54, used to record `git merge` — which compares
/// only the receipt head against `HEAD` and never reads a pull request
/// identity at all; a checkout at the reviewed head would have been allowed
/// to deliver pull request 66 on a verdict that named 54. Ranked on the
/// strictest fragment present, this line now records `gh pr merge` instead,
/// which reaches `delivery-pr-unidentified` rather than the head-only
/// comparison: the `&` byte is outside what `pr_merge_target` admits, so no
/// identity can be read and the refusal lands before any receipt is
/// consulted — stricter than a mismatch, and not the head-only allow the
/// defect used to produce.
#[test]
fn a_merge_and_a_mismatched_pr_merge_on_one_line_is_refused_from_review() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let pointer = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "revision": 1,
        "state": "review",
        "repo_dir": repo,
        "worktree": serde_json::Value::Null,
        "review_receipt": {
            "epoch": "a".repeat(32),
            "pr": 54,
            "head": rig.head,
            "base": "b".repeat(40),
            "digest": "c".repeat(64),
        },
        "reviewed_head": serde_json::Value::Null,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&pointer).expect("the pointer serialises"),
    )
    .expect("the pointer is written");
    let (out, err, _) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("review"),
        &[
            "gate",
            "Bash",
            "--json",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"git merge origin/main && gh pr merge 66"}"#,
        ],
        "",
    );
    let answered = format!("{out}{err}");
    assert!(
        !answered.contains("verdict-bound-to-other-bytes")
            && !answered.contains("\"decision\": \"allow\""),
        "a merge carrying a compound mismatched pull-request merge was allowed from review \
         because the head matched: {answered}"
    );
    assert!(
        answered.contains("delivery-pr-unidentified"),
        "the compound merge-then-merge line was refused for the wrong reason: {answered}"
    );
}

/// The floor a compound line's classification is measured against: a line
/// carrying exactly one irreversible fragment answers exactly as it does today,
/// in every tier. The multiplicity refusal must never fire on one fragment, so
/// each of these still reaches the answer it reached before this change — here
/// `control-surface-not-installed`, because no `estigia setup` ran.
#[test]
fn a_single_fragment_answers_as_today_in_every_tier() {
    for command in [
        "git push origin main",
        "git merge origin/main",
        "gh pr merge 12",
    ] {
        let home = tempfile::tempdir().expect("a temporary home");
        let here = tempfile::tempdir().expect("a launch directory");
        let run_id = "claude-abcd1234";
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

        let payload = serde_json::json!({ "command": command });
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
            "an uninstalled control surface was not refused for `{command}`: {out}{error}"
        );
        let value: serde_json::Value = serde_json::from_str(error.trim())
            .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
        assert_eq!(
            value["code"], "control-surface-not-installed",
            "a line carrying one fragment was answered differently for `{command}`: {value}"
        );
    }
}

/// `git merge origin/main && git tag v1` carries two tier-1 `IRREVERSIBLE`
/// fragments, so recording either one would decide the line on an answer that
/// was never about the other. It is refused, with a code naming the
/// multiplicity rather than any one fragment, and the refusal names both.
#[test]
fn a_line_with_two_same_tier_fragments_is_refused_naming_both() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let run_id = "claude-abcd1234";
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

    let payload = serde_json::json!({ "command": "git merge origin/main && git tag v1" });
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
        "a line carrying two same-tier fragments was not refused: {out}{error}"
    );
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(
        value["code"], "several-irreversible-fragments-on-one-line",
        "the same-tier pair was refused for the wrong reason: {value}"
    );
    let message = value["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("git merge") && message.contains("git tag"),
        "the refusal does not name both fragments on the line: {value}"
    );
    assert_eq!(
        value["observed"]["command"]["same_tier_fragments"],
        serde_json::json!(["git merge", "git tag"]),
        "the observed reading does not carry both tied fragments: {value}"
    );
}

/// The other half of the issue's example, and the tier that reaches no delivery
/// check at all: `git push && gh pr create` are both tier 0, so the refusal
/// must fire on the weakest tier too, not only where `DELIVERS` holds both.
#[test]
fn a_tier_zero_pair_is_refused_naming_both() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let run_id = "claude-abcd1234";
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

    let payload = serde_json::json!({ "command": "git push && gh pr create" });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run_in(
        home.path(),
        here.path(),
        &[
            "gate", "Bash", "--json", "--run-id", run_id, "--input", &payload,
        ],
        "",
    );
    assert!(!ok, "a tier-0 pair was not refused: {out}{error}");
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(
        value["code"], "several-irreversible-fragments-on-one-line",
        "the tier-0 pair was refused for the wrong reason: {value}"
    );
    let message = value["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("git push") && message.contains("gh pr create"),
        "the refusal does not name both fragments on the line: {value}"
    );
    assert_eq!(
        value["observed"]["command"]["same_tier_fragments"],
        serde_json::json!(["git push", "gh pr create"]),
        "the observed reading does not carry both tied fragments: {value}"
    );
}

/// The no-session door: `estigia gate` without `--run-id` classifies the line
/// too, so it must raise the same multiplicity refusal rather than dropping the
/// reading and adjudicating the first fragment.
#[test]
fn a_same_tier_pair_is_refused_without_a_run_id_too() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let payload = serde_json::json!({ "command": "git merge origin/main && git tag v1" });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run_in(
        home.path(),
        here.path(),
        &["gate", "Bash", "--json", "--input", &payload],
        "",
    );
    assert!(
        !ok,
        "a line carrying two same-tier fragments was not refused without a run id: {out}{error}"
    );
    let value: serde_json::Value = serde_json::from_str(error.trim())
        .unwrap_or_else(|parse_error| panic!("not JSON: {parse_error}: {out}{error}"));
    assert_eq!(
        value["code"], "several-irreversible-fragments-on-one-line",
        "the no-session door refused for the wrong reason: {value}"
    );
}

/// `git push` sits at index 0 and `git tag` at index 2, so without the tier
/// rank the earlier index would win the tiebreak.
#[test]
fn a_push_beside_a_tag_records_the_tag_the_stricter_check_reads() {
    let home = tempfile::tempdir().expect("a temporary home");
    let here = tempfile::tempdir().expect("a launch directory");
    let run_id = "claude-abcd1234";
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

    // No `estigia setup` ran, so this is refused `control-surface-not-installed`
    // before the tracker is ever asked — the same shape
    // `a_denied_delivery_reports_the_command_and_checkout_it_was_read_against`
    // uses to reach `observed.command` with nothing scripted.
    let payload = serde_json::json!({ "command": "git push && git tag v1" });
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
    assert_eq!(
        value["observed"]["command"]["matched"], "git tag",
        "the stricter fragment did not win the rank: {value}"
    );
}

/// A local fast-forward to the branch's tracked remote is preparation, not delivery.
///
/// The gate used to read every `git merge` as landing shared work, including the
/// exact `git merge --ff-only origin/main` used to bring an isolated worktree up
/// to date before continuing it. That command changes only this checkout and can
/// neither publish nor create a merge commit, but `in-progress` refused it with
/// the same answer as a pull-request merge.
#[test]
fn an_exact_local_fast_forward_is_allowed_without_widening_other_merges() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let git = |arguments: &[&str]| {
        checked_git(repo, arguments);
    };
    let branch = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--show-current"])
        .output()
        .expect("git names the fixture branch");
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_owned();
    assert!(!branch.is_empty(), "the fixture is not on a branch");

    // Put one commit on the local tracking ref while leaving the checked-out
    // branch at its clean parent. No network is involved.
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "upstream",
    ]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["reset", "--hard", "--quiet", "HEAD^"]);
    git(&["remote", "add", "origin", "https://example.invalid/o/r.git"]);
    git(&["config", &format!("branch.{branch}.remote"), "origin"]);
    git(&[
        "config",
        &format!("branch.{branch}.merge"),
        "refs/heads/main",
    ]);

    let pointer = |state: &str, reviewed_head: Option<&str>| {
        let pointer = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": state,
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
            "reviewed_head": reviewed_head,
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&pointer).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let gate = |state: &str, command: &str| {
        forget_store(home);
        let input = serde_json::json!({"command": command}).to_string();
        run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer(state),
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                &input,
            ],
            "",
        )
    };

    pointer("in-progress", None);
    let (allowed, refused, ok) = gate("in-progress", "git merge --ff-only origin/main");
    assert!(
        ok,
        "the safe local fast-forward was refused: {allowed}{refused}"
    );
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it: {allowed}"
    );

    // The host injects `GIT_EDITOR` into every shell it opens. `tracker_command`
    // strips every `GIT_*` name for determinism, which is exactly what kept this
    // suite green while the exemption was inert on that host. Put the variable
    // back and assert the same literal merge is still permitted: the criterion
    // is a test that does not scrub its environment.
    let input = serde_json::json!({"command": "git merge --ff-only origin/main"}).to_string();
    let (allowed, refused, ok) = run_with_tracker_and_env(
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
            &input,
        ],
        "",
        &[("GIT_EDITOR", "true")],
    );
    assert!(
        ok,
        "a host-injected editor disabled the safe local fast-forward: {allowed}{refused}"
    );
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it with GIT_EDITOR set: {allowed}"
    );

    pointer("in-progress", Some(&"0".repeat(40)));
    let (_, stale, ok) = gate("in-progress", "git merge --ff-only origin/main");
    assert!(!ok, "the local exception bypassed a stale verdict");
    assert!(
        stale.contains("verdict-bound-to-other-bytes"),
        "stale_verdict did not run first: {stale}"
    );
    pointer("in-progress", None);

    let (_, refused, ok) = gate("in-progress", "git merge --ff-only origin/other");
    assert!(!ok, "an untracked target was allowed");
    assert!(refused.contains("out-of-phase"), "wrong refusal: {refused}");
    assert!(
        refused.contains(
            "git merge: this step lands the work and issue #12 is in in-progress, where no verdict exists"
        ),
        "the existing out-of-phase message changed: {refused}"
    );
    assert!(
        refused.contains(
            "a review of this head. Publish the review target, move the issue to review, and deliver once somebody has answered"
        ),
        "the existing out-of-phase guidance changed: {refused}"
    );

    for state in ["analysis", "ready", "blocked"] {
        pointer(state, None);
        let (_, refused, ok) = gate(state, "git merge --ff-only origin/main");
        assert!(!ok, "the local exception widened into {state}");
        assert!(
            refused.contains("out-of-phase")
                && refused.contains(&format!(
                    "git merge: this step lands the work and issue #12 is in {state}, where no verdict exists"
                )),
            "{state} did not retain the existing refusal: {refused}"
        );
    }

    let ledger = decisions(home);
    assert!(
        ledger.lines().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).expect("a ledger entry");
            entry["verdict"] == "allow" && entry["subject"] == "git merge"
        }),
        "the allowed fast-forward is absent from the ledger: {ledger}"
    );
}

/// `git merge X` never moves `X`. Inside this run's own isolated checkout the
/// merge can only move the branch that checkout has out, so the delivery this
/// gate exists to refuse — landing work on trunk with no verdict — happens in
/// the *base* checkout, not here. Today no state lets the merge run at all: it
/// is a `DELIVERS` command, refused everywhere but `review`/`done`, and the
/// conflict it is meant to resolve cannot be produced from either — the
/// deadlock this test is named for.
///
/// Modelled on
/// [`an_exact_local_fast_forward_is_allowed_without_widening_other_merges`],
/// which the shipped exemption cannot cover: that one only recognises the
/// narrow `git merge --ff-only <target>` spelling, and a merge-commit case —
/// no `--ff-only` at all — is what a real integration usually is.
#[test]
fn an_integration_in_the_runs_own_checkout_is_allowed_from_in_progress() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let branch = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--show-current"])
        .output()
        .expect("git names the fixture branch");
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_owned();
    assert!(!branch.is_empty(), "the fixture is not on a branch");

    let pointer = |state: &str, worktree: Option<&std::path::Path>, reviewed_head: Option<&str>| {
        let pointer = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": state,
            "repo_dir": repo,
            "worktree": worktree,
            "reviewed_head": reviewed_head,
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&pointer).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let gate = |state: &str, command: &str| {
        forget_store(home);
        let input = serde_json::json!({"command": command}).to_string();
        run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer(state),
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                &input,
            ],
            "",
        )
    };

    // 1. The deadlock case, which is the point: the fixture repo IS the run's
    // own isolated checkout, `in-progress` is where the run holds the issue,
    // and this is the merge-commit spelling the shipped `--ff-only` exemption
    // cannot cover.
    pointer("in-progress", Some(repo), None);
    let (allowed, refused, ok) = gate("in-progress", "git merge origin/main");
    assert!(
        ok,
        "an integration in the run's own checkout was refused: {allowed}{refused}"
    );
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it: {allowed}"
    );

    // 2. The refusal that must survive: no worktree recorded, so the fixture
    // repo is standing in for the *base* checkout, not a run's own isolated
    // one. The rule is false here and the ordinary refusal — word for word —
    // stands.
    pointer("in-progress", None, None);
    let (_, refused, ok) = gate("in-progress", "git merge origin/main");
    assert!(!ok, "a merge in the base checkout was allowed");
    assert!(refused.contains("out-of-phase"), "wrong refusal: {refused}");
    assert!(
        refused.contains(
            "git merge: this step lands the work and issue #12 is in in-progress, where no verdict exists"
        ),
        "the existing out-of-phase message changed: {refused}"
    );
    assert!(
        refused.contains(
            "a review of this head. Publish the review target, move the issue to review, and deliver once somebody has answered"
        ),
        "the existing out-of-phase guidance changed: {refused}"
    );

    // 3. An integration spends no verdict, so a stale one does not stop it in
    // the run's own checkout; the same line anywhere else is still a delivery
    // and `stale_verdict` still reads it first.
    pointer("in-progress", Some(repo), Some(&"0".repeat(40)));
    let (allowed, refused, ok) = gate("in-progress", "git merge origin/main");
    assert!(
        ok && allowed.contains("allow"),
        "a stale verdict stopped an integration in the run's own checkout: {allowed}{refused}"
    );
    pointer("in-progress", None, Some(&"0".repeat(40)));
    let (_, stale, ok) = gate("in-progress", "git merge origin/main");
    assert!(
        !ok,
        "a stale verdict was spent outside the run's own checkout"
    );
    assert!(
        stale.contains("verdict-bound-to-other-bytes"),
        "stale_verdict did not run first: {stale}"
    );

    // 4. It does not widen into other states: the exemption is `in-progress`
    // only, and a state that holds no verdict still refuses the merge — with
    // its own code, and without sending the run to publish a review it cannot
    // reach from here.
    for state in ["analysis", "ready", "blocked"] {
        pointer(state, Some(repo), None);
        let (_, refused, ok) = gate(state, "git merge origin/main");
        assert!(!ok, "the integration exemption widened into {state}");
        assert!(
            refused.contains("integration-out-of-phase"),
            "{state} was not refused as an out-of-phase integration: {refused}"
        );
        assert!(
            !refused.contains("Publish"),
            "{state} sent an integration in the run's own checkout to publish: {refused}"
        );
    }
}

/// The case the integration exemption exists for, taken all the way through:
/// a real conflict, a real resolution, and a real finish — with `in-progress`
/// held the whole way, never traded for `review` to get the merge landed.
/// Both blind judges who found the severe defect this round repairs asked for
/// exactly this, end to end, and nothing in the suite posed it before.
///
/// Three gate calls, in order:
/// 1. `git merge origin/main` — allowed, and the fixture is built so this
///    really does conflict: `shared.txt` is a different line on each side of
///    a common ancestor, the same way
///    `local_fast_forward_proof_fails_closed_on_repository_state` builds a
///    `refs/remotes/origin/main` with no real remote at all.
/// 2. The resolution itself is an ordinary file write, gated `Routine` under
///    this run's live claim — not a second merge command, and nothing the
///    integration exemption has any say over.
/// 3. `git merge --continue` — allowed. This is the flag the literal reader
///    has to carry: a conflict is not resolved by any spelling that lacks
///    it, and it is the one step that would have broken had
///    `exact_integration_merge`'s flag list left it out.
#[test]
fn a_conflicting_integration_is_resolved_without_leaving_in_progress() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let git = |arguments: &[&str]| {
        checked_git(repo, arguments);
    };
    // In the repository, not per command. `git merge` asks for the committer
    // before it starts, so a runner carrying no global identity failed the
    // conflicting merge before it could write `MERGE_HEAD` - and the assertion
    // below read that as "did not conflict". Two of three CI platforms did.
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    let branch = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--show-current"])
        .output()
        .expect("git names the fixture branch");
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_owned();
    assert!(!branch.is_empty(), "the fixture is not on a branch");

    // A common ancestor, then the same line changed two different ways: one
    // commit becomes `refs/remotes/origin/main` with no real remote behind
    // it, the other stays on the branch this run's own checkout has out.
    std::fs::write(repo.join("shared.txt"), "base\n").expect("the shared file is written");
    git(&["add", "shared.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "shared file",
    ]);
    let base = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git reads HEAD")
            .stdout,
    )
    .trim()
    .to_owned();

    std::fs::write(repo.join("shared.txt"), "origin change\n")
        .expect("the origin side of the conflict is written");
    git(&["add", "shared.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "origin change",
    ]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["reset", "--hard", "--quiet", &base]);

    std::fs::write(repo.join("shared.txt"), "local change\n")
        .expect("the local side of the conflict is written");
    git(&["add", "shared.txt"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "local change",
    ]);

    let pointer = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "revision": 1,
        "state": "in-progress",
        "repo_dir": repo,
        "worktree": repo,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&pointer).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    let gate = |tool: &str, input: &serde_json::Value| {
        run_with_tracker(
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
                &input.to_string(),
            ],
            "",
        )
    };

    // 1. Allowed, and it really does conflict.
    let (allowed, refused, ok) = gate(
        "Bash",
        &serde_json::json!({"command": "git merge origin/main"}),
    );
    assert!(ok, "the integration was refused: {allowed}{refused}");
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it: {allowed}"
    );
    let merge = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge", "origin/main"])
        .output()
        .expect("git runs the merge");
    let complaint = String::from_utf8_lossy(&merge.stderr).trim().to_owned();
    assert!(
        !merge.status.success(),
        "the fixture did not actually conflict"
    );
    // Any failure exits non-zero, so the code alone cannot tell a conflict from
    // a fixture that never reached one. `MERGE_HEAD` is what only a real
    // conflict leaves, and git's own words are what make the next failure
    // diagnosable rather than a guess.
    assert!(
        repo.join(".git").join("MERGE_HEAD").is_file(),
        "the merge failed without leaving the checkout mid-merge, so it never conflicted - git said: {complaint}"
    );

    // 2. The resolution: a routine write, not a merge command.
    let (allowed, refused, ok) = gate(
        "Edit",
        &serde_json::json!({"file_path": repo.join("shared.txt").display().to_string()}),
    );
    assert!(ok, "resolving the conflict was refused: {allowed}{refused}");
    assert!(
        allowed.contains("allow"),
        "the gate did not allow the resolution: {allowed}"
    );
    std::fs::write(repo.join("shared.txt"), "resolved\n").expect("the conflict is resolved");
    git(&["add", "shared.txt"]);

    // 3. Allowed, and finishing it for real proves the flag was carried.
    let (allowed, refused, ok) = gate(
        "Bash",
        &serde_json::json!({"command": "git merge --continue"}),
    );
    assert!(ok, "finishing the merge was refused: {allowed}{refused}");
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it: {allowed}"
    );
    let finish = Command::new("git")
        .arg("-C")
        .arg(repo)
        .env("GIT_EDITOR", "true")
        .args(["merge", "--continue"])
        .output()
        .expect("git runs merge --continue");
    assert!(
        finish.status.success(),
        "the merge did not finish: {}",
        String::from_utf8_lossy(&finish.stderr)
    );
    assert!(
        !repo.join(".git").join("MERGE_HEAD").is_file(),
        "the checkout is still mid-merge"
    );

    // The pointer's own state was never touched by any of the three calls
    // above — `gate` never writes it, only a transition would — so this is a
    // sanity check that the fixture did not smuggle one in, not a proof by
    // itself.
    let after: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
        )
        .expect("the pointer is still readable"),
    )
    .expect("the pointer is still valid JSON");
    assert_eq!(
        after["state"], "in-progress",
        "the pointer left in-progress: {after}"
    );
}

/// CI release must refuse a receipt no computed verdict cleared, and must
/// refuse it *before* it touches the pull request.
///
/// The ordering assertion this replaces was source text only: a reviewer widened
/// the gate to `qualifying_review_verdict(..).or_else(|| Some(accepted))` — a
/// gate deciding nothing, which is the failure this crate exists to refuse — and
/// the whole suite stayed green, because the identifier was still textually
/// where the string test looked for it. This one drives the real server against
/// the fake tracker and watches for `pr ready` on the wire.
#[test]
fn ci_release_refuses_a_receipt_no_computed_verdict_cleared() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
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
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "release_ci",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "worktree": repo.display().to_string(),
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
    assert!(
        text.contains("qualifying-review-verdict-missing"),
        "an unreviewed receipt was refused for the wrong reason: {text}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("pr ready"),
        "CI release reached the pull request without a verdict: {calls}"
    );
}

/// The reproduction: the publisher's pointer is absent — it was
/// deleted at `SessionEnd`, as every run pointer is — while a *different* run
/// holds the live `review` claim after resuming the issue and supplies the
/// byte-exact latest receipt the tracker still records. Before the fix,
/// `recorded_receipt` raised `published-transaction-mismatch` on the publisher
/// field alone even though every other field matched; after it, the four
/// other re-verifications `release_ci` already performs are what stand
/// between this run and CI, and none of them names a publisher.
///
/// The digest is not invented: it is derived from the fixture checkout by the
/// same `expected_target` `release_ci` itself calls through `clean_target`,
/// so a receipt this test cannot actually match against the checkout would
/// fail this test rather than silently passing on a fixture value nothing
/// re-derives.
#[test]
fn release_ci_restores_a_receipt_for_a_run_that_did_not_publish_it() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let publisher = "claude-original1234";
    let resuming_run = "codex-resuming4a2b1c3d";
    let epoch = "e".repeat(32);

    // `tracker_rig` leaves an empty commit, and `expected_target` refuses an
    // empty tree as "not a delivery target" — so this checkout needs one real
    // file committed before its target is derivable.
    let base_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the fixture checkout's base");
    let base = String::from_utf8_lossy(&base_output.stdout)
        .trim()
        .to_owned();
    std::fs::write(repo.join("delivered.md"), "one\n").expect("a file to commit");
    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    git(&["add", "delivered.md"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "delivered",
    ]);
    let head_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the fixture checkout's head");
    let head = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_owned();

    let context = estigia::transport::Context {
        skill_dir: repo.to_path_buf(),
        repo_dir: repo.to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let target = estigia::transport::target::expected_target(&context, &base, None, None, false)
        .expect("the fixture checkout's own target is derivable");
    let digest = target["digest"]
        .as_str()
        .expect("the derived target carries a digest")
        .to_owned();

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
                    ("run-id", resuming_run),
                    ("runtime", "codex"),
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
                    ("run-id", publisher),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
    ];

    let pr_draft = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base, "state": "OPEN", "isDraft": true,
    });
    let pr_ready = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base, "state": "OPEN", "isDraft": false,
    });
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "pr view", "nth": 1, "stdout": pr_draft.to_string(), "status": 0 },
        { "matches": "pr ready", "stdout": "", "status": 0 },
        { "matches": "pr view", "nth": 2, "stdout": pr_ready.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(resuming_run.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );
    assert!(
        !runs.join(format!("{publisher}.json")).exists(),
        "the fixture accidentally left a pointer for the publisher"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "release_ci",
            "arguments": {
                "issue": 12,
                "run_id": resuming_run,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "worktree": repo.display().to_string(),
            }
        }
    })
    .to_string();

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
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
        "a run that did not publish, holding a live claim and the byte-exact latest receipt, was \
         refused: {text}"
    );
    let answered: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert_eq!(answered["ci_released"], true, "{text}");
    assert_eq!(answered["next"], "ci_status", "{text}");
    assert_eq!(answered["lane"]["state"], "unreadable", "{text}");
    assert_eq!(answered["basis"], "structural-readback", "{text}");

    let pointer: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(runs.join(format!("{resuming_run}.json")))
            .expect("the resuming run's pointer is readable"),
    )
    .expect("the pointer is valid JSON");
    assert_eq!(pointer["review_transaction"]["epoch"], epoch, "{pointer}");
    assert_eq!(pointer["review_transaction"]["head"], head, "{pointer}");
}

/// A `warning` on the receipt is informational, so delivery does not wait on
/// it: the receipt the gate refused while its warning stood
/// unanswered now reaches `gh pr ready` and reports `ci_released: true`.
///
/// Watched red before this change: the phase answered `disposition-missing`
/// and never touched the pull request — the gate this change retires.
///
/// The same identity also carries an earlier `severe` recording, the shape two
/// judges produce when one confirms what the other flagged: the identity is
/// still non-severe for its warning, and the receipt delivers on that class
/// alone.
#[test]
fn delivery_delivers_a_receipt_holding_an_unanswered_non_blocking_finding() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let reviewer = "codex-b";
    let epoch = "e".repeat(32);

    // The target must be derivable from the checkout, the same way the
    // neighbouring release test derives it, or a refusal here would prove a
    // fixture mismatch rather than the gate.
    let base_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the fixture checkout's base");
    let base = String::from_utf8_lossy(&base_output.stdout)
        .trim()
        .to_owned();
    std::fs::write(repo.join("delivered.md"), "one\n").expect("a file to commit");
    let git = |arguments: &[&str]| -> bool { checked_git(repo, arguments) };
    git(&["add", "delivered.md"]);
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "delivered",
    ]);
    let head_output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git names the fixture checkout's head");
    let head = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_owned();

    let context = estigia::transport::Context {
        skill_dir: repo.to_path_buf(),
        repo_dir: repo.to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let target = estigia::transport::target::expected_target(&context, &base, None, None, false)
        .expect("the fixture checkout's own target is derivable");
    let digest = target["digest"]
        .as_str()
        .expect("the derived target carries a digest")
        .to_owned();

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
    let claim = comment(
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
    );
    let publication = comment(
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
            ],
        ),
    );
    let severe = comment(
        "IC_severe",
        "2026-08-14T09:20:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", run_id),
                ("op-id", &"5".repeat(32)),
                ("reviewer", reviewer),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("id", "w-1"),
                ("class", "severe"),
                ("evidence", "observed the defect directly"),
                ("impact", "wrong output on the happy path"),
                ("evidence-class", "deterministic"),
                ("causal-disposition", "pre-existing"),
            ],
        ),
    );
    let warning = comment(
        "IC_warning",
        "2026-08-14T09:40:00Z",
        marker(
            "review-finding",
            &[
                ("run-id", run_id),
                ("op-id", &"3".repeat(32)),
                ("reviewer", reviewer),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("id", "w-1"),
                ("class", "warning"),
                (
                    "evidence",
                    "the sentence beside the fix overstates what the check covers",
                ),
                (
                    "impact",
                    "a reader trusts a guarantee the code does not make",
                ),
            ],
        ),
    );
    let pr_draft = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base, "state": "OPEN", "isDraft": true,
    });
    let pr_ready = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": head, "baseRefOid": base, "state": "OPEN", "isDraft": false,
    });
    let release_answers = |comments: Vec<serde_json::Value>| {
        serde_json::to_string(&serde_json::json!([
            {
                "matches": "issue view",
                "stdout": serde_json::json!({
                    "state": "OPEN",
                    "labels": [{"name": "status:review"}],
                    "comments": comments,
                }).to_string(),
                "status": 0,
            },
            { "matches": "pr view", "nth": 1, "stdout": pr_draft.to_string(), "status": 0 },
            { "matches": "pr ready", "stdout": "", "status": 0 },
            { "matches": "pr view", "nth": 2, "stdout": pr_ready.to_string(), "status": 0 },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises")
    };

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = |name: &str, arguments: serde_json::Value| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        })
        .to_string()
    };
    let release_request = request(
        "release_ci",
        serde_json::json!({
            "issue": 12,
            "run_id": run_id,
            "epoch": epoch,
            "pr": 7,
            "head": head,
            "base": base,
            "digest": digest,
            "worktree": repo.display().to_string(),
        }),
    );

    let call =
        |request: &str, answers: String, counts: &std::path::Path| -> (serde_json::Value, String) {
            let mut child = tracker_command(home, repo, bin, &answers)
                .arg("mcp")
                .env("ESTIGIA_FAKE_COUNT", counts)
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
            let response: serde_json::Value = serde_json::from_slice(&output.stdout)
                .unwrap_or_else(|_| {
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
            (response, format!("{text}\n--- calls ---\n{calls}"))
        };

    let before = vec![claim, publication, severe, warning];

    // The warning stands unanswered. Nothing about delivery waits on it, so the
    // run reaches the write that marks the pull request ready.
    let (delivered, transcript) = call(&release_request, release_answers(before), &count);
    assert_eq!(
        delivered["result"]["isError"], false,
        "a receipt holding a non-blocking finding was refused: {transcript}"
    );
    let released: serde_json::Value = serde_json::from_str(
        delivered["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default(),
    )
    .unwrap_or_else(|error| panic!("{error}: {transcript}"));
    assert_eq!(released["ci_released"], true, "{transcript}");
    assert!(
        transcript.contains("pr ready"),
        "delivery never marked the pull request ready: {transcript}"
    );
}

/// Neither acquisition route hands an unresolved handoff back to the run that
/// asked for the review.
///
/// This is the whole mechanism of the change, and it was wired by a source-order
/// assertion alone: replacing `require_review_eligibility(..)?` with
/// `let _ = require_review_eligibility(..)` in both `claim` and `reclaim` leaves
/// the identifiers exactly where a string test looks for them, and the suite
/// stayed green while a publishing run could take back the item it is forbidden
/// to review. That is a gate deciding nothing — the failure this crate exists to
/// refuse — so both routes are now driven through the real server.
#[test]
fn no_acquisition_route_returns_an_unresolved_handoff_to_its_requester_over_the_wire() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let run_id = "claude-abcd1234";
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
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
    let comments = vec![
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
                ],
            ),
        ),
        comment(
            "IC_handoff",
            "2026-08-14T10:00:00Z",
            marker(
                "review-handoff",
                &[
                    ("run-id", run_id),
                    ("target-op", &target),
                    ("op-id", &"f".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("authority", "ask"),
                    ("requested-at", "2026-08-14T10:00:00Z"),
                    ("deadline", "2026-08-14T10:30:00Z"),
                    ("blocker", "no independent context in this run"),
                    ("discharger", "another run records the verdict"),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // Both doors, one fixture: the requester is refused whichever it tries.
    for (tool, arguments) in [
        (
            "claim",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "horizon": "2099-01-01T00:00Z",
                "state": "review",
            }),
        ),
        (
            "claim",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "horizon": "2099-01-01T00:00Z",
                "state": "review",
                "takeover": true,
            }),
        ),
    ] {
        let log = trace.path().join(format!("{tool}.log"));
        let count = trace.path().join(format!("{tool}-count.json"));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        })
        .to_string();
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
        assert_eq!(response["result"]["isError"], true, "{tool}: {response}");
        assert!(
            text.contains("review-handoff-requester-excluded"),
            "{tool} did not refuse the requesting run: {text}"
        );
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !calls.contains("issue comment") && !calls.contains("issue edit"),
            "{tool} wrote to the tracker before refusing: {calls}"
        );
    }
}

/// A handoff names the latest receipt or it names nothing.
///
/// `handoff_review` carries this check twice — before the marker and before the
/// release — and neither copy was held: neutering both left the whole suite
/// green. It is load-bearing, not belt-and-braces. `review_eligibility` collects
/// handoffs only for the *latest* publication, so a handoff recorded against a
/// superseded receipt excludes nobody while the ownership epoch has already been
/// released — the publishing run is immediately eligible again for the item it
/// is the one run forbidden to review. That is this issue's own livelock, with
/// an audit trail.
#[test]
fn a_handoff_against_a_superseded_receipt_is_refused_before_anything_is_written() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let target = "a".repeat(32);
    let superseded = "e".repeat(32);
    let latest = "f".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
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
    let publication = |epoch: &str| {
        marker(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        )
    };
    // A live claim, and a republish after the epoch this request names.
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
                    ("op-id", &target),
                ],
            ),
        ),
        comment("IC_first", "2026-08-14T09:10:00Z", publication(&superseded)),
        comment("IC_second", "2026-08-14T09:20:00Z", publication(&latest)),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
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
                // The epoch a republish has already invalidated.
                "epoch": superseded,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": "no independent context in this run",
                "discharger": "another run records the verdict",
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
    assert!(
        text.contains("published-transaction-mismatch"),
        "a superseded receipt was refused for the wrong reason: {text}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "a handoff for a superseded receipt was written to the timeline: {calls}"
    );
    assert!(
        !calls.contains("issue edit"),
        "a handoff for a superseded receipt released ownership: {calls}"
    );
}

/// A run that does not hold the live claim cannot hand the review off.
///
/// This check was wired by a string search for `verify_claim(` and nothing else,
/// and it is not cosmetic: `plan_release` accepts a run found live **or** stale,
/// so without it a run whose horizon has passed could still write the immutable
/// handoff marker — which excludes the publisher from both acquisition routes
/// and from the queue — and release the epoch underneath whoever now holds it.
/// The state readback only fires after that write.
#[test]
fn a_run_that_lost_the_claim_cannot_hand_the_review_off() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let publisher = "claude-abcd1234";
    // Nobody else needs to hold it: an expired claim is enough.
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
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
    // The publisher's own claim, expired. This is the case that isolates the
    // check: `plan_release` accepts a run found live **or** stale, so it would
    // happily release this epoch, and an absent claim would have been refused by
    // `plan_release` alone — measuring nothing.
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", publisher),
                    ("runtime", "claude"),
                    ("horizon", "2020-01-01T00:00Z"),
                    ("op-id", &target),
                ],
            ),
        ),
        comment(
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
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "handoff_review",
            "arguments": {
                "issue": 12,
                "run_id": publisher,
                "target_operation": target,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": "no independent context in this run",
                "discharger": "another run records the verdict",
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(publisher.to_owned());
    run.issue = Some(estigia::identity::Identity::Number(12));
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

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
    assert_eq!(response["result"]["isError"], true, "{response}");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "a run that had lost the claim still wrote the handoff marker: {calls}"
    );
    assert!(
        !calls.contains("issue edit"),
        "a run that had lost the claim still released ownership: {calls}"
    );
}

/// The review queue is the third place the same exclusion is enforced, and it
/// hides the item from its requester without hiding it from anybody else. It
/// also refuses the whole queue when the open list cannot be read.
///
/// Both were promised in `bindings/github.md` and the tool description while
/// nothing measured either: pushing every candidate to `eligible` regardless of
/// eligibility, or treating an unreadable list as an empty one, each left the
/// suite green.
#[test]
fn the_review_queue_hides_a_handoff_from_its_requester_and_refuses_an_unreadable_list() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    name_origin_o_r(repo);
    let trace = tempfile::tempdir().expect("a trace directory");
    let requester = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
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
    let timeline = vec![
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", requester),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
        comment(
            "IC_handoff",
            "2026-08-14T10:00:00Z",
            marker(
                "review-handoff",
                &[
                    ("run-id", requester),
                    ("target-op", &"a".repeat(32)),
                    ("op-id", &"f".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("authority", "ask"),
                    ("requested-at", "2026-08-14T10:00:00Z"),
                    ("deadline", "2026-08-14T10:30:00Z"),
                    ("blocker", "no independent context in this run"),
                    ("discharger", "another run records the verdict"),
                ],
            ),
        ),
    ];
    let queue = serde_json::json!([{
        "number": 12,
        "title": "a published receipt awaiting review",
        "labels": [{"name": "status:review"}, {"name": "domain:general"}],
        "created_at": "2026-08-14T08:00:00Z",
    }])
    .to_string();
    // The candidate's timeline is the store's, recorded here once, so the queue
    // read decides from it without a GitHub read per candidate.
    seed_store_with(home, &[(12, "review", timeline)]);
    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "issues?state=open", "stdout": serde_json::json!([serde_json::from_str::<serde_json::Value>(&queue).unwrap()]).to_string(), "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let ask = |run_id: &str, answers: &str, label: &str| -> serde_json::Value {
        let log = trace.path().join(format!("{label}.log"));
        let count = trace.path().join(format!("{label}-count.json"));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_state",
                "arguments": { "state": "review", "run_id": run_id },
            },
        })
        .to_string();
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
        let output = child.wait_with_output().expect("the MCP server exits");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "the MCP response is not JSON: {}",
                String::from_utf8_lossy(&output.stdout)
            )
        })
    };

    let mine = ask(requester, &answers, "requester");
    let text = mine["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(mine["result"]["isError"], false, "{mine}");
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        answer["count"], 0,
        "the requesting run was offered its own unresolved handoff: {text}"
    );
    assert_eq!(answer["excluded_count"], 1, "{text}");
    assert_eq!(
        answer["excluded"][0]["reason"], "review-handoff-requester-excluded",
        "{text}"
    );

    let theirs = ask("codex-99999999", &answers, "distinct");
    let text = theirs["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(theirs["result"]["isError"], false, "{theirs}");
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        answer["count"], 1,
        "a distinct run could not see the handed-off item: {text}"
    );
    assert_eq!(answer["excluded_count"], 0, "{text}");

    // And the list itself, answering something that is not a list. A failed
    // `gh` call never reaches this — the transport refuses that already — so
    // the case left is a success whose body is not a queue, and treating it as
    // an empty one sends a run to triage believing there is no work.
    let no_list = serde_json::to_string(&serde_json::json!([
        { "matches": "issues?state=open", "stdout": "null", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let unread = ask("codex-99999999", &no_list, "no-list");
    assert_eq!(
        unread["result"]["isError"], true,
        "an unreadable queue was answered as an empty one: {unread}"
    );
}
