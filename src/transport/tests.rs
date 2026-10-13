use super::*;

#[cfg(unix)]
#[test]
fn a_descendant_holding_the_pipe_has_its_group_killed_before_the_leader_is_reaped() {
    let started = std::time::Instant::now();
    let output = run(
        &["sh", "-c", "sleep 30 & exit 0"],
        None,
        How::write_within(Duration::from_secs(20)),
    )
    .expect("the leader exited cleanly");
    assert_eq!(output.status, 0);
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[cfg(unix)]
#[test]
fn process_arguments_preserve_non_utf8_path_bytes() {
    use std::os::unix::ffi::OsStringExt;

    let argument = std::ffi::OsString::from_vec(vec![b'p', b'a', b't', b'h', 0xff]);
    let output = run_os(
        &[
            std::ffi::OsString::from("sh"),
            std::ffi::OsString::from("-c"),
            std::ffi::OsString::from("printf '%s' \"$1\" | od -An -tx1"),
            std::ffi::OsString::from("sh"),
            argument,
        ],
        None,
        How::read(),
    )
    .expect("the process receives the path argument");
    assert_eq!(
        output.stdout.split_whitespace().collect::<String>(),
        "70617468ff"
    );
}

/// Only a `status:` label carries the workflow state.
///
/// Measured by mutation: answering `true` for every name left the whole suite
/// green. The consequence is not a misread — it is a **write**. Without
/// `--from`, `transition` removes "whatever stale state labels are there", so a
/// predicate that says yes to everything strips every label the issue carries:
/// `bug`, `priority:high`, whoever's triage, gone on the next move.
#[test]
fn only_a_status_label_carries_the_state() {
    for carried in ["status:in-progress", "status:done", "status:"] {
        assert!(
            super::commands::is_status_label(carried),
            "`{carried}` carries the state and was not read as one"
        );
    }
    for other in [
        "bug",
        "priority:high",
        "Status:done",
        " status:done",
        "statusy:done",
        "",
    ] {
        assert!(
            !super::commands::is_status_label(other),
            "`{other}` is not a state label and would be removed as a stale one"
        );
    }

    // And through the reader the removal actually uses, over the shape `gh`
    // answers with: the operator's own labels survive.
    let labels = serde_json::json!({
        "labels": [
            {"name": "bug"},
            {"name": "status:in-progress"},
            {"name": "priority:high"},
            {"name": "status:review"}
        ]
    });
    assert_eq!(
        super::commands::status_labels(&labels),
        vec!["status:in-progress".to_owned(), "status:review".to_owned()],
        "the labels a transition would strip are not only the state ones"
    );
}

#[test]
fn a_configured_value_is_found_by_prefix_and_first_row_wins() {
    // The Python's `cfg` matches a prefix, not a label, and answers with the
    // first row that matches. Copied deliberately — see `Context::get`.
    let context = Context {
        skill_dir: std::path::PathBuf::from("/skill"),
        repo_dir: std::path::PathBuf::from("/repo"),
        config: vec![
            (
                "delivery authorisation".to_owned(),
                "ask".to_owned(),
                super::Source::Written,
            ),
            (
                "delivery route".to_owned(),
                "direct".to_owned(),
                super::Source::Written,
            ),
            (
                "merge strategy".to_owned(),
                "merge commit".to_owned(),
                super::Source::Written,
            ),
        ],
        repo: None,
    };
    assert_eq!(context.get("merge strategy"), Some("merge commit"));
    assert_eq!(context.get("delivery"), Some("ask"), "the second row won");
    assert_eq!(context.get("nothing here"), None);
}

#[test]
fn a_missing_program_is_a_read_failure_and_never_a_write_one() {
    // The distinction the whole module exists to keep. A program that is not
    // there ran nothing, so nothing can have landed — reporting it as a write
    // would tell the caller to go and check for an effect that is impossible.
    let failure = run(&["estigia-no-such-program-exists"], None, How::write())
        .expect_err("a missing program cannot succeed");
    assert!(matches!(failure, Failure::Read(_)), "{failure:?}");
    assert_eq!(failure.code(), 3);
}

#[test]
fn an_unusable_working_directory_is_not_reported_as_a_missing_program() {
    let root = tempfile::tempdir().expect("a temporary root");
    let absent = root.path().join("not-a-directory");
    for how in [
        How::read(),
        How {
            timeout: Some(std::time::Duration::from_secs(1)),
            ..How::read()
        },
    ] {
        let failure = run(&["git", "--version"], Some(&absent), how)
            .expect_err("a process cannot start in an absent directory");
        let detail = failure.detail();

        assert!(
            detail.contains("could not start in") && detail.contains(&absent.display().to_string()),
            "the failure did not name its unusable working directory: {detail}"
        );
        assert!(
            !detail.contains("not found on PATH"),
            "an invalid working directory was blamed on PATH: {detail}"
        );
    }
}

#[cfg(unix)]
fn timed(script: &str, seconds: u64) -> Result<Output, Failure> {
    run(
        &["sh", "-c", script],
        None,
        How {
            timeout: Some(std::time::Duration::from_secs(seconds)),
            ..How::read()
        },
    )
}

#[cfg(unix)]
#[test]
fn a_timed_child_writing_past_the_pipe_buffer_is_read_in_full() {
    let output = timed("head -c 1048576 /dev/zero | tr '\\0' x", 20)
        .expect("output larger than a pipe buffer must not deadlock the wait");
    assert_eq!(output.stdout.len(), 1_048_576);
}

#[cfg(unix)]
#[test]
fn a_timed_child_reading_stdin_sees_end_of_file() {
    let output = timed("cat", 20).expect("stdin is closed, so cat ends at once");
    assert_eq!(output.stdout, "");
}

#[cfg(unix)]
#[test]
fn a_timed_childs_non_utf8_output_is_reported_lossily() {
    let output = timed("printf 'a\\377b'", 20).expect("the child exits 0");
    assert_eq!(output.stdout, "a\u{fffd}b");
    assert!(output.stdout_replaced);
}

#[cfg(unix)]
#[test]
fn a_grandchild_holding_the_pipe_does_not_hold_the_call() {
    let started = std::time::Instant::now();
    let output = timed("sleep 30 & echo hi", 20).expect("the shell exits 0");
    assert_eq!(output.stdout, "hi\n");
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[cfg(unix)]
#[test]
fn a_timeout_is_still_a_timeout() {
    let failure = timed("sleep 30", 1).expect_err("the child outlives its deadline");
    assert!(matches!(failure, Failure::Timeout(_)), "{failure:?}");
}

#[cfg(unix)]
fn failing_with(stderr: &str, how: How) -> Failure {
    let script = format!("echo '{stderr}' >&2; exit 1");
    run(&["sh", "-c", &script], None, how).expect_err("status 1 with check is a failure")
}

#[cfg(unix)]
#[test]
fn a_read_that_hit_a_rate_limit_is_named_and_says_when_it_resets_only_if_told() {
    for (stderr, kind) in [
        (
            "GraphQL: API rate limit already exceeded for user ID 1.",
            "primary",
        ),
        (
            "gh: API rate limit exceeded for user ID 1. (HTTP 403)",
            "primary",
        ),
        (
            "You have exceeded a secondary rate limit. Please wait a few minutes.",
            "secondary",
        ),
    ] {
        let failure = failing_with(stderr, How::read());
        assert!(matches!(failure, Failure::RateLimited(_)), "{failure:?}");
        assert_eq!(failure.code(), 3, "a rate limit is still a failed read");
        let envelope = failure.envelope();
        assert_eq!(envelope["reason"], "rate-limited");
        let detail = envelope["detail"].as_str().expect("a detail");
        assert!(detail.contains(kind), "{detail}");
        assert!(detail.contains("reset time unknown"), "{detail}");
        assert!(envelope["action"].as_str().is_some_and(|a| !a.is_empty()));
    }

    let told = failing_with(
        "API rate limit exceeded; resets at 2026-10-05T22:34:20Z",
        How::read(),
    );
    assert!(
        told.detail().contains("resets at 2026-10-05T22:34:20Z"),
        "{told:?}"
    );

    let other = failing_with("connection reset", How::read());
    assert!(matches!(other, Failure::Read(_)), "{other:?}");
    let write = failing_with("API rate limit exceeded", How::write());
    assert!(
        matches!(write, Failure::Write(_)),
        "a write that may have landed was reported as a read: {write:?}"
    );
}

#[test]
fn a_failing_write_and_a_failing_read_are_not_the_same_answer() {
    // Same command, same status, two different instructions to the caller.
    let program = if cfg!(windows) { "cmd" } else { "sh" };
    let args: Vec<&str> = if cfg!(windows) {
        vec![program, "/c", "exit 1"]
    } else {
        vec![program, "-c", "exit 1"]
    };

    let read = run(&args, None, How::read()).expect_err("status 1 with check is a failure");
    assert!(matches!(read, Failure::Read(_)), "{read:?}");
    assert_eq!(read.code(), 3);
    assert_eq!(read.envelope()["reason"], "read-failed");

    let write = run(&args, None, How::write()).expect_err("status 1 with check is a failure");
    assert!(matches!(write, Failure::Write(_)), "{write:?}");
    assert_eq!(write.code(), 5);
    assert_eq!(write.envelope()["reason"], "write-failed");

    // And tolerated: the same status, reported rather than raised.
    let answer = run(&args, None, How::tolerated()).expect("a tolerated status is an answer");
    assert_eq!(answer.status, 1);
}

/// The live context carries the operator's *files*, not only the contract.
///
/// `a_live_call_carries_the_operators_table` closed the level above this one:
/// the two live callers built the context with `config: Vec::new()`, so every
/// row resolved to nothing, silently. The fix read the **contract**. It stopped
/// there.
///
/// Two documents override that contract and both are the operator's own:
/// `estigia.local.md` beside it, and `.git/estigia/estigia.local.md` in a
/// repository that answers for itself. `estigia config list` layers both and
/// reports the result; `Context::live` read neither, so an operator who
/// configured a row in their own file rather than through the managed block was
/// read nothing.
///
/// The floor is the second half: with no override files, the contract's own
/// values still come through, or this would pass against a reader that answers
/// the operator's value to every question.
#[test]
fn a_live_call_carries_the_operators_own_files() {
    for (theirs, expected_worktree, expected_evidence) in [
        (true, "/work/trees", "measuring"),
        (false, "unset", "reading"),
    ] {
        let home = tempfile::tempdir().expect("a home");
        let skill = home.path().join("skill");
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");
        let repo = tempfile::tempdir().expect("a repository");

        if theirs {
            // Beside the contract: the file an operator writes when they do not
            // want their answers rewritten by a `sync`.
            std::fs::write(
                skill.join(crate::config::LOCAL_FILE),
                "<!-- estigia:config:start -->\n| Setting | Value here |\n|---|---|\n\
                 | Worktree location | /work/trees |\n<!-- estigia:config:end -->\n",
            )
            .expect("their file");
            // And in the repository, which is the other document `config list`
            // reports and `config set --repo` writes.
            let inside = crate::skill::repository_config_path(repo.path());
            std::fs::create_dir_all(inside.parent().expect("a directory")).expect("the directory");
            std::fs::write(
                &inside,
                "<!-- estigia:config:start -->\n| Setting | Value here |\n|---|---|\n\
                 | Evidence standard | measuring |\n<!-- estigia:config:end -->\n",
            )
            .expect("the repository's file");
        }

        // What the operator is told, through the reader every command uses.
        let told = crate::skill::installed_config_in(&skill, repo.path()).expect("it layers");
        assert_eq!(
            crate::config::Setting::Worktree.value_of(&told),
            expected_worktree,
            "the fixture does not pose what it says it does"
        );
        assert_eq!(
            crate::config::Setting::Evidence.value_of(&told),
            expected_evidence,
            "the fixture does not pose what it says it does"
        );

        // And what the transport that runs actually reads.
        let live = super::Context::live(skill.clone(), repo.path().to_path_buf(), None);
        assert_eq!(
            live.get("worktree location"),
            Some(expected_worktree),
            "the transport reads a worktree location the operator did not configure"
        );
        assert_eq!(
            live.get("evidence standard"),
            Some(expected_evidence),
            "the transport reads a merge strategy this repository did not choose"
        );
    }
}

/// A listing with no page metadata is a read that failed, not an empty answer.
///
/// The rule was written down at one of the two places that applied it — *an
/// absent `nodes` is not "no results": it is a response nobody can read, and
/// the empty list it would flatten to is the answer that grants clearance* —
/// and **nothing measured it**. Loosening either copy left the whole suite
/// green, which is how a sentence like that stops being true.
///
/// One reader now, so one test covers both callers: the project listing that
/// decides which projects an issue belongs to, and the closing-reference read
/// that decides whether an issue is already being closed by a pull request.
/// Both grant something on an empty list.
#[test]
fn a_connection_without_page_metadata_is_a_failed_read() {
    let whole = serde_json::json!({ "nodes": [], "pageInfo": { "hasNextPage": false } });
    let (nodes, page) = super::connection_page(&whole, "a listing").expect("a whole connection");
    assert!(
        nodes.is_empty() && page.is_object(),
        "the floor: a good page reads"
    );

    for broken in [
        serde_json::json!({ "nodes": [] }),
        serde_json::json!({ "pageInfo": { "hasNextPage": false } }),
        serde_json::json!({ "nodes": [], "pageInfo": "later" }),
        serde_json::json!({ "nodes": {}, "pageInfo": { "hasNextPage": false } }),
        serde_json::json!({}),
    ] {
        let refused = super::connection_page(&broken, "a listing");
        assert!(
            matches!(refused, Err(super::Failure::Read(_))),
            "a connection missing what it takes to be read was answered as an empty one: {broken}"
        );
    }
}

/// A comment edited after it was written cannot instruct or prove anything.
///
/// The tracker stamps a comment when it is **created**, and says separately
/// whether it has been edited since. So an edited comment's text is not the
/// text the timeline dated: whoever can edit it can put a stand-down, or a
/// claim's own operation marker, under a timestamp that was earned by something
/// else. `viewer_did_author && !includes_created_edit` is the whole of that
/// rule, and it decides two things — whether a control message stops this run,
/// and whether an operation this run claims to have performed is proven.
///
/// **Nothing measured it.** Turning the `&&` into `||` at either site — an
/// edited comment counting again — left the entire suite green, at both.
/// Measured by mutation, which is the only way an omission like this shows: the
/// rule is written, applied twice, and asserted nowhere.
#[test]
fn a_comment_edited_after_it_was_written_neither_instructs_nor_proves() {
    let at = "2026-08-01T10:00:00Z";
    // An `adjudication` naming this run: a control kind that is not a release,
    // which is what `control_after` looks for. A `standdown` is in
    // `RELEASE_KINDS` and this function walks past those on purpose — a run
    // that released an item and re-claimed it must not read its own release as
    // an order to stop.
    let order = crate::transport::markers::render(
        "adjudication",
        &[("run-id", "claude-b"), ("target", "claude-a")],
    )
    .expect("renders");
    let comment = |edited: bool, body: &str| crate::transport::ownership::Comment {
        id: Some("IC_1".to_owned()),
        created_at: at.to_owned(),
        body: body.to_owned(),
        viewer_did_author: true,
        includes_created_edit: edited,
    };

    // The floor: unedited, the control message is found. Without this the
    // assertion below passes against a reader that finds nothing at all.
    // Position matters: `control_after` reads only past the watermark, so the
    // acquisition sits first and the order after it.
    let honest = [comment(false, "just a note"), comment(false, &order)];
    assert!(
        super::claim::control_after(&honest, "claude-a", 0).is_some(),
        "an unedited adjudication no longer stops the run it names"
    );

    // And edited, it is not there to be obeyed. The timeline dated text that is
    // no longer the text.
    let tampered = [comment(false, "just a note"), comment(true, &order)];
    assert!(
        super::claim::control_after(&tampered, "claude-a", 0).is_none(),
        "a comment edited after the tracker stamped it was read as an instruction"
    );
}

/// A markerless comment never stops a run, whatever it says or names.
///
/// `control_after` used to read a markerless comment that named the run and held
/// one of a bounded phrase list as an order to stand down. The list could not
/// tell an instruction from a run's own record of its work — "adjudicate" is
/// this crate's own word for what `claim` does — so a run's markerless closing
/// note stopped the run that wrote it. Only a control-kind marker
/// instructs now.
#[test]
fn a_markerless_comment_never_stops_the_run() {
    let at = "2026-08-01T10:00:00Z";
    let comment = |body: &str| crate::transport::ownership::Comment {
        id: None,
        created_at: at.to_owned(),
        body: body.to_owned(),
        viewer_did_author: true,
        includes_created_edit: false,
    };
    let closing_note = "Delivered by claude-a: this run adjudicates its own ownership, and \
                        nothing here is an order to stand down.";
    let naming_the_run = [comment("a claim"), comment(closing_note)];
    assert!(
        super::claim::control_after(&naming_the_run, "claude-a", 0).is_none(),
        "a markerless note that names the run was read as an instruction to stop"
    );

    let naming_another = [comment("a claim"), comment("Delivered by claude-b.")];
    assert!(
        super::claim::control_after(&naming_another, "claude-a", 0).is_none(),
        "a markerless note naming another run stopped this one"
    );
}

/// A control-kind marker after the watermark still stops the run; a release-kind
/// marker and a control marker written before the watermark do not.
#[test]
fn only_a_control_marker_after_the_watermark_stops_the_run() {
    let at = "2026-08-01T10:00:00Z";
    let comment = |body: &str| crate::transport::ownership::Comment {
        id: None,
        created_at: at.to_owned(),
        body: body.to_owned(),
        viewer_did_author: true,
        includes_created_edit: false,
    };
    let adjudication = crate::transport::markers::render("adjudication", &[("run-id", "claude-a")])
        .expect("renders");
    let standdown =
        crate::transport::markers::render("standdown", &[("target", "claude-a")]).expect("renders");

    let after = [comment("a claim"), comment(&adjudication)];
    assert!(
        super::claim::control_after(&after, "claude-a", 0).is_some(),
        "a control-kind marker naming the run no longer stops it"
    );

    let release = [comment("a claim"), comment(&standdown)];
    assert!(
        super::claim::control_after(&release, "claude-a", 0).is_none(),
        "a release-kind marker was read as a control message"
    );

    let before = [comment(&adjudication), comment("a claim")];
    assert!(
        super::claim::control_after(&before, "claude-a", 1).is_none(),
        "a control message written before the watermark stopped the run"
    );
}

/// A review target is bound to an object id, not to whatever git printed.
///
/// `object()` runs `git rev-parse --verify` and takes the output as the thing a
/// review is bound to. What comes back is trusted only when it **looks like an
/// object id**, because a `rev-parse` that succeeds and prints a message, a ref
/// name or an empty line is a read that did not answer — and a review bound to
/// that text is bound to nothing.
///
/// The rule is asked directly, and `object` is held to calling it — a
/// `rev-parse` that succeeds and prints something else is not a state git can
/// be talked into.
#[test]
fn a_target_is_refused_unless_git_resolved_it_to_an_object() {
    for good in [
        "a66c9871f2d3c4b5a6978e9f0a1b2c3d4e5f6071",
        "de291f8",
        "0000000",
    ] {
        assert!(
            super::target::is_object_id(good),
            "{good:?} is an object id and was refused"
        );
    }
    for bad in [
        "",
        "HEAD",
        "main",
        "de291f",
        "fatal: ambiguous argument",
        "de291f8g",
    ] {
        assert!(
            !super::target::is_object_id(bad),
            "{bad:?} is not an object id and a review could have been bound to it"
        );
    }

    // And the one caller asks it, rather than deciding again on its own — which
    // is where it was before, unreachable to any test.
    let source = crate::source_text::code_of(include_str!("target.rs"));
    assert!(
        source.contains("if !is_object_id(&oid)"),
        "`object` no longer refuses through the rule this test measures"
    );
}

/// A flag that is there and says nothing is not an argument.
///
/// `need` asks whether a flag arrived; nothing asked whether it carried a word.
/// So `--runtime ""` reached `claim` as a present argument, and an empty
/// runtime is a marker naming nobody and a `dev:` label with nothing after the
/// colon — attached to a claimed issue. `claim.rs` draws exactly this
/// distinction one step later, in its own
/// words: *unresolved is not empty*.
///
/// Found by mutation: replacing the runtime the tool server stamps with an
/// empty string left the entire suite green.
///
/// Through the dispatcher, because that is the door every caller comes in by,
/// and both halves — the empty one refused, and a real one still accepted.
#[test]
fn an_argument_that_says_nothing_is_refused_before_anything_is_written() {
    let skill = tempfile::tempdir().expect("a skill directory");
    let repo = tempfile::tempdir().expect("a repository");
    let context = super::Context {
        skill_dir: skill.path().to_path_buf(),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let flags = |runtime: &str| {
        [
            "--issue",
            "12",
            "--run-id",
            "claude-aaaaaaaa",
            "--runtime",
            runtime,
            "--horizon",
            "2099-01-01T00:00Z",
            "--operation-id",
            "00000000000000000000000000000000",
        ]
        .iter()
        .map(|part| (*part).to_owned())
        .collect::<Vec<String>>()
    };

    for empty in ["", "   "] {
        let refused =
            super::dispatch::dispatch(&context, "claim", &flags(empty), "2026-08-01T10:00:00Z")
                .expect_err("an empty runtime is refused");
        let envelope = refused.envelope();
        assert_eq!(
            envelope.get("reason").and_then(serde_json::Value::as_str),
            Some("blank-argument"),
            "a runtime that says nothing was accepted as one: {envelope}"
        );
        assert_eq!(
            envelope.get("argument").and_then(serde_json::Value::as_str),
            Some("--runtime"),
            "the refusal does not name the flag: {envelope}"
        );
    }

    // And the five more that had the same hole. Each of these is carried into
    // the world as a name — `--to ""` writes `status:` with nothing after the
    // colon, `--run-id ""` claims an issue for nobody, `--branch ""` pushes a
    // ref with no name — and every one of them reached `gh` before this.
    for (operation, arguments, blank) in [
        ("transition", vec!["--issue", "12", "--to", ""], "--to"),
        ("list-state", vec!["--state", ""], "--state"),
        (
            "verify-claim",
            vec![
                "--issue",
                "12",
                "--run-id",
                "claude-a",
                "--expect-state",
                "",
            ],
            "--expect-state",
        ),
        (
            "start-branch",
            vec![
                "--issue", "12", "--run-id", "claude-a", "--branch", "", "--base", "main",
            ],
            "--branch",
        ),
    ] {
        let owned: Vec<String> = arguments.iter().map(|part| (*part).to_owned()).collect();
        let envelope =
            super::dispatch::dispatch(&context, operation, &owned, "2026-08-01T10:00:00Z")
                .expect_err("an argument that says nothing is refused")
                .envelope();
        assert_eq!(
            envelope.get("reason").and_then(serde_json::Value::as_str),
            Some("blank-argument"),
            "{operation} with an empty {blank} reached the world: {envelope}"
        );
        assert_eq!(
            envelope.get("argument").and_then(serde_json::Value::as_str),
            Some(blank),
            "the refusal does not name the flag: {envelope}"
        );
    }

    // The floor: a real runtime gets past this check. Without it the assertions
    // above would pass against a build that refuses every claim.
    let further =
        super::dispatch::dispatch(&context, "claim", &flags("claude"), "2026-08-01T10:00:00Z");
    let stopped_here = further.as_ref().err().is_some_and(|failure| {
        failure
            .envelope()
            .get("argument")
            .and_then(serde_json::Value::as_str)
            == Some("--runtime")
    });
    assert!(
        !stopped_here,
        "a real runtime was refused as though it said nothing"
    );
}

/// A repository nobody named is a read that did not answer.
///
/// `repo_identity` took the owner and the name out of `gh repo view` with
/// `unwrap_or_default()` and returned `Ok(("", ""))` — the sentence this crate
/// refuses everywhere else, *a failed read is not a failed answer*, committed
/// at the one place four callers get the repository from.
///
/// What they do with it: the project listing, the branch link and the
/// closing-PR connection query a repository named `""`,
/// and `start-branch` derives the worktree's own name from it when the flag is
/// absent — a checkout put somewhere unnamed. The closing-PR connection is the
/// expensive one: that set authorises post-merge claim renewal.
#[test]
fn a_repository_nobody_named_is_a_read_that_did_not_answer() {
    // The floor: a complete answer still reads.
    let whole = serde_json::json!({ "owner": { "login": "asanabrial" }, "name": "estigia" });
    assert_eq!(
        super::closing::identity_of(&whole).expect("a named repository"),
        ("asanabrial".to_owned(), "estigia".to_owned())
    );

    for silent in [
        serde_json::json!({}),
        serde_json::json!({ "name": "estigia" }),
        serde_json::json!({ "owner": { "login": "asanabrial" } }),
        serde_json::json!({ "owner": { "login": "" }, "name": "estigia" }),
        serde_json::json!({ "owner": { "login": "asanabrial" }, "name": "  " }),
        serde_json::json!({ "owner": "asanabrial", "name": "estigia" }),
    ] {
        assert!(
            matches!(
                super::closing::identity_of(&silent),
                Err(super::Failure::Read(_))
            ),
            "an answer that names no repository was read as one: {silent}"
        );
    }
}

/// `EnvMode::Replace`'s own filter, pure and process-free: no `std::env`
/// mutation, so no race against another test's own ambient environment. A
/// caller's `HOME` and `XDG_CONFIG_HOME` — the pair `core.attributesFile`'s
/// default resolves through — must not survive; `PATH`, matched
/// case-insensitively the way Windows itself treats it, must.
#[test]
fn essential_env_drops_home_and_keeps_path_case_insensitively() {
    let ambient = [
        ("HOME".to_owned(), "/home/caller".to_owned()),
        (
            "XDG_CONFIG_HOME".to_owned(),
            "/home/caller/.config".to_owned(),
        ),
        ("Path".to_owned(), "/usr/bin".to_owned()),
        ("LANG".to_owned(), "en_US.UTF-8".to_owned()),
    ];
    let kept = essential_env(ambient);
    assert!(
        kept.iter()
            .all(|(name, _)| !name.eq_ignore_ascii_case("HOME")
                && !name.eq_ignore_ascii_case("XDG_CONFIG_HOME")
                && !name.eq_ignore_ascii_case("LANG")),
        "a caller's own HOME, XDG_CONFIG_HOME or LANG survived the replacement: {kept:?}"
    );
    assert_eq!(
        kept,
        vec![("Path".to_owned(), "/usr/bin".to_owned())],
        "PATH, spelled as the caller spelled it, did not survive: {kept:?}"
    );
}
