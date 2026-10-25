#![cfg(unix)]

use super::super::checks::{RunChecks, patience_for, run_checks_with};
use super::*;
use crate::transport::generated::checks::{commands_digest, declared_at};
use crate::transport::markers;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const NOW: &str = "2026-08-31T00:00:00Z";

fn put(path: &Path, text: &str) {
    crate::paths::replace_atomically(path, text).expect("a fixture file");
}

fn declaration_for(scripts: &[(&str, &str)]) -> String {
    scripts
        .iter()
        .map(|(name, script)| {
            format!(
                "[[check]]\nname = \"{name}\"\ncommand = [\"sh\", \"-c\", \"{}\"]\n\n",
                script.replace('\\', "\\\\").replace('"', "\\\"")
            )
        })
        .collect()
}

fn checked_change(declaration: Option<&str>) -> (tempfile::TempDir, Context, ReviewTransaction) {
    let root = tempfile::tempdir().expect("a directory");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:fixture/repo.git"],
    );
    put(&repo.join("notes.md"), "A line of prose.\n");
    if let Some(declaration) = declaration {
        put(&repo.join("estigia.generated.toml"), declaration);
    }
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = head_of(&repo);
    put(&repo.join("notes.md"), "A line of prose, reworded.\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "--quiet", "-m", "head"]);
    let context = Context {
        repo: Some("fixture/repo".to_owned()),
        ..context_at(&repo)
    };
    let mut receipt = ReviewTransaction {
        head: head_of(&repo),
        base,
        ..ledger_receipt()
    };
    receipt.digest = crate::transport::target::clean_target(&context, &receipt.base, None)
        .expect("the clean target")["digest"]
        .as_str()
        .expect("a digest")
        .to_owned();
    (root, context, receipt)
}

fn on_a_thread(dir: &Path) -> Result<u32, Failure> {
    let owned = dir.to_path_buf();
    let handle = std::thread::spawn(move || crate::transport::check_job::run(&owned));
    crate::reviewer::job::await_started(dir, || {
        handle.is_finished().then(|| "finished".to_owned())
    })?;
    Ok(std::process::id())
}

fn timeline_of(receipt: &ReviewTransaction) -> Vec<ownership::Comment> {
    vec![live_claim("claude-a"), published(receipt)]
}

fn request(wait_seconds: u64, base: &str) -> RunChecks<'_> {
    RunChecks {
        issue: 42,
        run_id: "claude-a",
        base: Some(base),
        worktree: None,
        wait_seconds,
        now: NOW,
    }
}

fn ask(
    context: &Context,
    comments: &[ownership::Comment],
    asking: &RunChecks<'_>,
    spawn: &dyn Fn(&Path) -> Result<u32, Failure>,
) -> Result<Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        42,
        stdout_with(comments),
    )]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        run_checks_with(context, asking, spawn)
    })
}

fn verdict(
    context: &Context,
    comments: &[ownership::Comment],
    receipt: &ReviewTransaction,
) -> Result<Value, Failure> {
    let bin = crate::test_env::scripted_gh();
    let mut answers = lane_read_answers();
    answers.insert(
        0,
        crate::test_env::issue_view_answer(42, stdout_with(comments)),
    );
    let operation_id = op(9);
    let asking = computed_request(&operation_id, receipt);
    crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&answers), || {
        record_computed_verdict(context, &asking)
    })
}

fn recorded_receipts() -> usize {
    crate::transport::store::local_bodies(42)
        .matches("issue-flow: checks-receipt ")
        .count()
}

fn counter(root: &Path) -> PathBuf {
    root.join("counter")
}

fn ran_times(counter: &Path) -> usize {
    std::fs::read_to_string(counter).map_or(0, |text| text.lines().count())
}

fn never(_: &Path) -> Result<u32, Failure> {
    panic!("a job was started");
}

#[test]
fn a_repository_that_declares_nothing_answers_so_records_nothing_and_clears_as_before() {
    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) = checked_change(None);
    let comments = timeline_of(&receipt);

    let answer = ask(&context, &comments, &request(0, &receipt.base), &never)
        .expect("nothing declared is an answer");
    assert_eq!(answer, json!({"ok": true, "declared": false}));
    assert_eq!(recorded_receipts(), 0);

    let cleared = verdict(&context, &comments, &receipt).expect("the verdict clears as before");
    assert_eq!(cleared["outcome"], "approved", "{cleared}");
}

#[test]
fn a_failing_check_records_passed_false_and_the_verdict_refuses_checks_failed() {
    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) = checked_change(Some(&declaration_for(&[
        ("fmt", "echo formatted"),
        ("tests", "echo broke >&2; exit 7"),
    ])));
    let comments = timeline_of(&receipt);

    let answer = ask(
        &context,
        &comments,
        &request(30, &receipt.base),
        &on_a_thread,
    )
    .expect("the job finishes inside the wait");
    assert_eq!(answer["state"], "finished", "{answer}");
    assert_eq!(answer["passed"], false, "{answer}");
    assert_eq!(answer["digest"], receipt.digest, "{answer}");
    assert_eq!(answer["checks"][0]["exit"], 0);
    assert_eq!(answer["checks"][1]["exit"], 7);
    assert!(
        answer["checks"][1]["tail"]
            .as_str()
            .is_some_and(|tail| tail.contains("broke")),
        "{answer}"
    );
    assert_eq!(recorded_receipts(), 1);
    let bodies = crate::transport::store::local_bodies(42);
    assert!(!bodies.contains("broke"), "a tail reached the marker");

    let failure = verdict(&context, &comments, &receipt).expect_err("a failed check clears");
    assert_eq!(reason(failure), "checks-failed");
}

#[test]
fn a_passing_check_records_a_receipt_and_the_verdict_clears() {
    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) =
        checked_change(Some(&declaration_for(&[("fmt", "echo formatted")])));
    let comments = timeline_of(&receipt);

    let refused = verdict(&context, &comments, &receipt).expect_err("no receipt yet");
    assert_eq!(reason(refused), "checks-receipt-missing");

    let answer = ask(
        &context,
        &comments,
        &request(30, &receipt.base),
        &on_a_thread,
    )
    .expect("the job finishes inside the wait");
    assert_eq!(answer["passed"], true, "{answer}");

    let cleared = verdict(&context, &comments, &receipt).expect("the verdict clears");
    assert_eq!(cleared["outcome"], "approved", "{cleared}");
}

#[test]
fn a_second_run_for_the_same_digest_runs_nothing() {
    crate::transport::store::forget_scratch_store();
    let counted = tempfile::tempdir().expect("a directory");
    let file = counter(counted.path());
    let (_root, context, receipt) = checked_change(Some(&declaration_for(&[(
        "once",
        &format!("echo ran >> {}", file.display()),
    )])));
    let comments = timeline_of(&receipt);

    let first = ask(
        &context,
        &comments,
        &request(30, &receipt.base),
        &on_a_thread,
    )
    .expect("the first run");
    assert_eq!(first["cached"], false, "{first}");
    let second = ask(&context, &comments, &request(30, &receipt.base), &never)
        .expect("the second run reads the receipt");
    assert_eq!(second["cached"], true, "{second}");
    assert_eq!(second["passed"], true, "{second}");
    assert_eq!(ran_times(&file), 1);
    assert_eq!(recorded_receipts(), 1);
}

#[test]
fn a_running_job_answers_running_in_the_wait_it_was_given_and_is_never_started_twice() {
    crate::transport::store::forget_scratch_store();
    let held = tempfile::tempdir().expect("a directory");
    let release = held.path().join("release");
    let (_root, context, receipt) = checked_change(Some(&declaration_for(&[(
        "slow",
        &format!("while [ ! -e {} ]; do sleep 0.05; done", release.display()),
    )])));
    let comments = timeline_of(&receipt);
    let starts = AtomicUsize::new(0);
    let counting = |dir: &Path| {
        starts.fetch_add(1, Ordering::SeqCst);
        on_a_thread(dir)
    };

    let began = std::time::Instant::now();
    let running = ask(&context, &comments, &request(1, &receipt.base), &counting)
        .expect("a running job is an answer");
    let waited = began.elapsed();
    assert_eq!(running["state"], "running", "{running}");
    assert!(running["ask_after"].as_u64().is_some(), "{running}");
    assert!(waited >= Duration::from_secs(1), "{waited:?}");
    assert!(waited < Duration::from_secs(20), "{waited:?}");

    let again = ask(&context, &comments, &request(0, &receipt.base), &counting)
        .expect("asking again is an answer");
    assert_eq!(again["state"], "running", "{again}");
    assert_eq!(again["job"], running["job"]);
    assert_eq!(starts.load(Ordering::SeqCst), 1, "a second job was started");
    assert_eq!(recorded_receipts(), 0);

    put(&release, "");
    let done = ask(&context, &comments, &request(30, &receipt.base), &counting)
        .expect("the finished job is collected");
    assert_eq!(done["state"], "finished", "{done}");
    assert_eq!(done["passed"], true, "{done}");
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}

#[test]
fn no_wait_is_longer_than_fifty_seconds() {
    assert_eq!(patience_for(0), Duration::ZERO);
    assert_eq!(patience_for(7), Duration::from_secs(7));
    assert_eq!(patience_for(50), Duration::from_secs(50));
    assert_eq!(patience_for(51), Duration::from_secs(50));
    assert_eq!(patience_for(u64::MAX), Duration::from_secs(50));
}

#[test]
fn a_receipt_for_another_digest_or_other_commands_is_a_missing_receipt() {
    let (_root, context, receipt) =
        checked_change(Some(&declaration_for(&[("fmt", "echo formatted")])));
    let declared = declared_at(&context.repo_dir, &receipt.base).expect("the declaration");
    let commands = commands_digest(&declared);
    let issued = |digest: &str, commands: &str, passed: &str| {
        wrote(
            "5",
            markers::render(
                "checks-receipt",
                &[
                    ("run-id", "claude-a"),
                    ("digest", digest),
                    ("commands", commands),
                    ("passed", passed),
                    ("results", "fmt=0:1"),
                ],
            )
            .expect("a receipt marker"),
        )
    };
    let with = |extra: ownership::Comment| {
        let mut comments = timeline_of(&receipt);
        comments.push(extra);
        release_refusal(&context, &receipt, &comments)
    };

    assert_eq!(
        with(issued(&"9".repeat(64), &commands, "true")),
        "checks-receipt-missing"
    );
    assert_eq!(
        with(issued(&receipt.digest, &"9".repeat(64), "true")),
        "checks-receipt-missing"
    );
    assert_eq!(
        with(issued(&receipt.digest, &commands, "false")),
        "checks-failed"
    );
    let mut comments = timeline_of(&receipt);
    comments.push(issued(&receipt.digest, &commands, "true"));
    release_basis_of(&context, &receipt, &comments).expect("a passing receipt for this target");

    let mut untrusted = issued(&receipt.digest, &commands, "true");
    untrusted.viewer_did_author = false;
    assert_eq!(with(untrusted), "checks-receipt-missing");
}

#[test]
fn the_release_gate_is_untouched_when_the_base_declares_no_checks() {
    let (_root, context, receipt) = checked_change(None);
    let comments = timeline_of(&receipt);
    release_basis_of(&context, &receipt, &comments).expect("no declaration, no new refusal");
}

#[test]
fn a_run_without_a_published_base_is_refused_not_guessed() {
    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) =
        checked_change(Some(&declaration_for(&[("fmt", "echo formatted")])));
    let asking = RunChecks {
        base: None,
        ..request(0, &receipt.base)
    };
    let failure = ask(&context, &timeline_of(&receipt), &asking, &never)
        .expect_err("no base, no target to bind");
    assert_eq!(reason(failure), "checks-base-unknown");
}

#[test]
fn a_dirty_tree_is_refused_before_anything_runs() {
    crate::transport::store::forget_scratch_store();
    let (_root, context, receipt) =
        checked_change(Some(&declaration_for(&[("fmt", "echo formatted")])));
    put(
        &context.repo_dir.join("notes.md"),
        "edited after publishing\n",
    );
    let failure = ask(
        &context,
        &timeline_of(&receipt),
        &request(0, &receipt.base),
        &never,
    )
    .expect_err("a dirty tree is not a target");
    assert!(!matches!(failure, Failure::Read(_)), "{}", failure.detail());
    assert_eq!(recorded_receipts(), 0);
}

#[test]
fn a_spawn_that_fails_is_the_answer_and_nothing_runs_in_the_foreground() {
    crate::transport::store::forget_scratch_store();
    let counted = tempfile::tempdir().expect("a directory");
    let file = counter(counted.path());
    let (_root, context, receipt) = checked_change(Some(&declaration_for(&[(
        "once",
        &format!("echo ran >> {}", file.display()),
    )])));
    let refusing = |_: &Path| -> Result<u32, Failure> {
        Err(Failure::Read("this process may not detach".to_owned()))
    };
    let failure = ask(
        &context,
        &timeline_of(&receipt),
        &request(0, &receipt.base),
        &refusing,
    )
    .expect_err("the failure is the answer");
    assert_eq!(failure.detail(), "this process may not detach");
    assert_eq!(ran_times(&file), 0);
    assert_eq!(recorded_receipts(), 0);
}
