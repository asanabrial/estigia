use super::*;
use crate::config::GitOperationWindow;
use crate::transport::{Context, Failure, How, Source, run};

std::thread_local! {
    static AT: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

fn clock() -> Option<u16> {
    AT.with(|cell| Some(cell.get()))
}

fn at(minutes: u16, body: impl FnOnce()) {
    AT.with(|cell| cell.set(minutes));
    crate::harness::operation_window::with_now_override(Some(clock), body);
}

fn context(value: &str) -> Context {
    Context {
        skill_dir: std::path::PathBuf::from("/nonexistent-skill"),
        repo_dir: std::path::PathBuf::from("/nonexistent-repo"),
        config: vec![(
            "Git operation window".to_owned(),
            value.to_owned(),
            Source::Written,
        )],
        repo: None,
    }
}

fn closed() -> Mode {
    Mode::Closed(GitOperationWindow::parse("enabled 09:00-17:00").expect("a schedule"))
}

#[test]
fn a_disabled_window_is_open_at_any_hour() {
    at(23 * 60, || {
        assert!(matches!(state(&context("disabled")), Mode::Open));
    });
}

#[test]
fn an_enabled_window_is_closed_outside_its_range_and_open_inside() {
    at(20 * 60, || {
        assert!(matches!(
            state(&context("enabled 09:00-17:00")),
            Mode::Closed(_)
        ));
    });
    at(12 * 60, || {
        assert!(matches!(state(&context("enabled 09:00-17:00")), Mode::Open));
    });
}

#[test]
fn a_scoped_mode_answers_instead_of_the_clock_and_is_gone_afterwards() {
    at(20 * 60, || {
        let inside = with_mode(Mode::Open, || state(&context("enabled 09:00-17:00")));
        assert!(matches!(inside, Mode::Open));
        assert!(matches!(
            state(&context("enabled 09:00-17:00")),
            Mode::Closed(_)
        ));
    });
}

#[test]
fn a_scope_is_restored_when_the_body_panics() {
    let _ = std::panic::catch_unwind(|| with_mode(closed(), || panic!("inside")));
    assert!(scoped().is_none());
}

#[test]
fn gh_writes_are_classified_and_reads_are_not() {
    let writes: &[&[&str]] = &[
        &["gh", "issue", "create", "--title", "t"],
        &["gh", "issue", "comment", "12", "--body", "b"],
        &["gh", "issue", "edit", "12", "--add-label", "x"],
        &["gh", "-R", "o/r", "pr", "merge", "3"],
        &["gh", "pr", "create"],
        &["gh", "pr", "ready", "3"],
        &["gh", "issue", "develop", "12", "--name", "b"],
        &["gh", "label", "create", "x"],
        &["gh", "release", "create", "v1"],
        &["gh", "workflow", "run", "ci.yml"],
        &["gh", "api", "graphql", "-f", "query=mutation { x }"],
        &["gh", "api", "graphql", "--input", "m.json"],
        &["gh", "api", "graphql", "--input=m.json"],
        &["gh", "api", "graphql", "-F", "query=@m.graphql"],
        &["gh", "api", "graphql", "-Fquery=@m.graphql"],
        &["gh", "api", "graphql", "--field", "query=@-"],
        &["gh", "api", "graphql", "--field=query=@m.graphql"],
        &["gh", "api", "-X", "POST", "repos/o/r/issues"],
        &["gh", "api", "--method=PATCH", "repos/o/r/issues/1"],
        &["gh", "api", "repos/o/r/issues", "-f", "title=t"],
        &["gh", "api", "-XPOST", "repos/o/r/issues"],
        &["gh", "api", "-Xdelete", "repos/o/r/git/refs/heads/b"],
        &["gh", "api", "repos/o/r/issues", "-ftitle=t"],
        &["gh", "api", "-xput", "repos/o/r/pulls/7/merge"],
        &["gh", "api", "repos/o/r/issues", "-Fbody=@b.md"],
        &["gh", "api", "repos/o/r/issues", "--field=title=t"],
        &["gh", "api", "repos/o/r/issues", "--raw-field=title=t"],
        &["gh", "api", "repos/o/r/issues", "--input=body.json"],
        &["gh", "cache", "delete", "--all"],
        &["gh", "ssh-key", "add", "k.pub"],
        &["gh", "ssh-key", "delete", "1"],
        &["gh", "gpg-key", "add", "k.asc"],
        &["gh", "gpg-key", "delete", "1"],
        &["gh", "repo", "deploy-key", "add", "k.pub"],
        &["gh", "repo", "deploy-key", "delete", "1"],
        &[
            "gh",
            "-R",
            "o/r",
            "repo",
            "autolink",
            "create",
            "JIRA-",
            "https://x/<num>",
        ],
        &["gh", "repo", "autolink", "delete", "1"],
        &["git", "push", "origin", "b"],
        &["git", "-C", "/tmp/x", "push"],
        &["git", "-c", "a=b", "push", "--force"],
    ];
    for args in writes {
        assert!(is_outbound_write(args, How::tolerated()), "{args:?}");
    }
    let reads: &[&[&str]] = &[
        &["gh", "issue", "view", "12"],
        &["gh", "issue", "list"],
        &["gh", "issue", "develop", "--list", "12"],
        &["gh", "pr", "view", "3"],
        &["gh", "api", "graphql", "-f", "query=query { x }"],
        &["gh", "api", "repos/o/r/issues"],
        &[
            "gh",
            "api",
            "--method",
            "GET",
            "repos/o/r/issues",
            "-f",
            "state=open",
        ],
        &["gh", "label", "list"],
        &["gh", "cache", "list"],
        &["gh", "ssh-key", "list"],
        &["gh", "gpg-key", "list"],
        &["gh", "repo", "view"],
        &["gh", "repo", "deploy-key", "list"],
        &["gh", "repo", "autolink", "list"],
        &["gh", "repo", "autolink", "view", "1"],
        &["gh", "auth", "status"],
        &["gh", "browse"],
        &["git", "branch", "b"],
        &["git", "worktree", "add", "x", "b"],
        &["git", "ls-remote", "origin"],
        &["git", "fetch", "origin"],
    ];
    for args in reads {
        assert!(!is_outbound_write(args, How::tolerated()), "{args:?}");
    }
}

#[test]
fn a_call_declared_as_a_gh_write_is_one_whatever_it_spells() {
    assert!(is_outbound_write(&["gh", "frobnicate"], How::write()));
    assert!(!is_outbound_write(&["git", "frobnicate"], How::write()));
}

#[test]
fn while_closed_a_gh_write_is_refused_before_any_process_starts() {
    let directory = tempfile::tempdir().expect("a directory");
    let log = directory.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    let refused = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
        with_mode(closed(), || {
            run(
                &["gh", "issue", "comment", "12", "--body", "b"],
                None,
                How::write(),
            )
        })
    });
    match refused {
        Err(Failure::Stop(payload)) => {
            assert_eq!(payload["reason"], "outbox-bypass", "{payload}");
        }
        other => panic!("a closed window let a gh write through: {other:?}"),
    }
    assert!(
        std::fs::read_to_string(&log).unwrap_or_default().is_empty(),
        "the refused write still started gh"
    );
}

#[test]
fn while_closed_a_git_push_is_refused() {
    let refused = with_mode(closed(), || {
        run(&["git", "push", "origin", "b"], None, How::write())
    });
    assert!(
        matches!(&refused, Err(Failure::Stop(payload)) if payload["reason"] == "outbox-bypass"),
        "{refused:?}"
    );
}

#[test]
fn while_closed_a_read_is_not_refused() {
    let bin = crate::test_env::scripted_gh();
    let answered = crate::test_env::with_scripted_gh(bin.path(), "", || {
        with_mode(closed(), || {
            run(&["gh", "issue", "view", "12"], None, How::read())
        })
    });
    assert!(
        !matches!(&answered, Err(Failure::Stop(payload)) if payload["reason"] == "outbox-bypass"),
        "{answered:?}"
    );
}

#[test]
fn while_open_a_gh_write_reaches_gh() {
    let directory = tempfile::tempdir().expect("a directory");
    let log = directory.path().join("calls.log");
    let bin = crate::test_env::scripted_gh();
    crate::test_env::with_logging_gh(bin.path(), "", &log, || {
        let _ = with_mode(Mode::Open, || {
            run(
                &["gh", "issue", "comment", "12", "--body", "b"],
                None,
                How::write(),
            )
        });
    });
    assert!(
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("comment"),
        "an open window did not reach gh"
    );
}

#[test]
fn outside_any_scope_no_write_is_refused() {
    let directory = tempfile::tempdir().expect("a directory");
    let refused = run(
        &["git", "push", "nowhere"],
        Some(directory.path()),
        How::tolerated(),
    );
    assert!(
        !matches!(&refused, Err(Failure::Stop(payload)) if payload["reason"] == "outbox-bypass"),
        "{refused:?}"
    );
}
