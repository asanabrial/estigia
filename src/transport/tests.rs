use super::*;

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
            ("delivery authorisation".to_owned(), "ask".to_owned()),
            ("delivery route".to_owned(), "direct".to_owned()),
            ("project board".to_owned(), "acme/7".to_owned()),
        ],
        repo: None,
    };
    assert_eq!(context.get("project board"), Some("acme/7"));
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
/// row resolved to nothing and the board mirror was off for every run on the
/// machine, silently. The fix read the **contract**. It stopped there.
///
/// Two documents override that contract and both are the operator's own:
/// `estigia.local.md` beside it, and `.git/estigia/estigia.local.md` in a
/// repository that answers for itself. `estigia config list` layers both and
/// reports the result; `Context::live` read neither. Measured on one machine
/// with both files written:
///
/// | | `config list` | the transport |
/// |---|---|---|
/// | `Project board` | `acme/7` | `none` |
/// | `Merge strategy` | `rebase` | `merge commit` |
///
/// So the board mirror is off for everybody who configured the board in their
/// own file rather than through the managed block — which is the same outcome
/// the level above produced, one layer down, and reached the same way: the
/// previous fix covered the level above and was taken as complete.
///
/// The floor is the second half: with no override files, the contract's own
/// values still come through, or this would pass against a reader that answers
/// the operator's value to every question.
#[test]
fn a_live_call_carries_the_operators_own_files() {
    for (theirs, expected_board, expected_merge) in
        [(true, "acme/7", "rebase"), (false, "none", "merge commit")]
    {
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
                 | Project board | acme/7 |\n<!-- estigia:config:end -->\n",
            )
            .expect("their file");
            // And in the repository, which is the other document `config list`
            // reports and `config set --repo` writes.
            let inside = crate::skill::repository_config_path(repo.path());
            std::fs::create_dir_all(inside.parent().expect("a directory")).expect("the directory");
            std::fs::write(
                &inside,
                "<!-- estigia:config:start -->\n| Setting | Value here |\n|---|---|\n\
                 | Merge strategy | rebase |\n<!-- estigia:config:end -->\n",
            )
            .expect("the repository's file");
        }

        // What the operator is told, through the reader every command uses.
        let told = crate::skill::installed_config_in(&skill, repo.path()).expect("it layers");
        assert_eq!(
            crate::config::Setting::Board.value_of(&told),
            expected_board,
            "the fixture does not pose what it says it does"
        );
        assert_eq!(
            crate::config::Setting::Merge.value_of(&told),
            expected_merge,
            "the fixture does not pose what it says it does"
        );

        // And what the transport that runs actually reads.
        let live = super::Context::live(skill.clone(), repo.path().to_path_buf(), None);
        assert_eq!(
            live.get("project board"),
            Some(expected_board),
            "the transport reads a board the operator did not configure"
        );
        assert_eq!(
            live.get("merge strategy"),
            Some(expected_merge),
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
/// One reader now, so one test covers both callers: the board listing that
/// decides whether a project exists, and the closing-reference read that
/// decides whether an issue is already being closed by a pull request. Both
/// grant something on an empty list.
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

/// A review target is bound to an object id, not to whatever git printed.
///
/// `object()` runs `git rev-parse --verify` and takes the output as the thing a
/// review is bound to. What comes back is trusted only when it **looks like an
/// object id**, because a `rev-parse` that succeeds and prints a message, a ref
/// name or an empty line is a read that did not answer — and a review bound to
/// that text is bound to nothing.
///
/// Nothing measured it: replacing the whole shape check with *is it empty* left
/// the suite green. **And the first test written for it measured nothing
/// either** — it asked git for a ref that does not exist, which makes
/// `rev-parse` fail, so the refusal came from the run and never reached the
/// rule. The mutation stayed green through that test too, which is the only
/// reason it was caught.
///
/// So the rule is asked directly, and `object` is held to calling it.
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
    let source = include_str!("target.rs");
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
/// colon — attached to a claimed issue, and what the board is made to converge
/// on. `claim.rs` draws exactly this distinction one step later, in its own
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
/// What they do with it: the board listing queries an owner named `""`, the
/// branch link and the closing-PR connection query a repository named `""`,
/// and `start-branch` derives the worktree's own name from it when the flag is
/// absent — a checkout put somewhere unnamed. The closing-PR connection is the
/// expensive one: that set authorises post-merge claim renewal.
///
/// **Nothing measured it**: adding the refusal left the whole suite green.
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

#[test]
fn an_unreadable_review_candidate_timeline_makes_the_queue_unreadable() {
    assert!(
        super::commands::queue_comments(&serde_json::json!({"comments": []}), 12)
            .expect("an empty but readable timeline")
            .is_empty()
    );
    for unreadable in [
        serde_json::json!({}),
        serde_json::json!({"comments": null}),
        serde_json::json!({"comments": {}}),
    ] {
        assert!(
            matches!(
                super::commands::queue_comments(&unreadable, 12),
                Err(super::Failure::Read(_))
            ),
            "an unreadable candidate became eligible: {unreadable}"
        );
    }
}
