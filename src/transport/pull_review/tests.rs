use std::path::{Path, PathBuf};

use super::{ClaimRequest, claim, release};
use crate::transport::Context;
use crate::transport::store::{Store, store_path};

const RUN: &str = "claude-abcd1234";
const OTHER: &str = "claude-ffff0000";
const NOW: &str = "2026-11-04T10:00:00Z";
const HORIZON: &str = "2026-11-04T18:00Z";

struct Rig {
    root: tempfile::TempDir,
    repo: PathBuf,
    origin: PathBuf,
    head: String,
    base: String,
    context: Context,
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let done = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        done.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&done.stderr)
    );
    String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

// `origin` is a bare repository holding `main` and `refs/pull/7/head`, the
// shape GitHub serves; the head commit is on no local branch of `repo`.
fn rig() -> Rig {
    crate::transport::store::forget_scratch_store();
    let root = tempfile::tempdir().expect("a temporary root");
    let origin = root.path().join("github.com-acme-widget.git");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&origin).expect("a remote directory");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    git(&origin, &["init", "--quiet", "--bare", "-b", "main"]);
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    git(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    git(&repo, &["commit", "--allow-empty", "--quiet", "-m", "base"]);
    git(&repo, &["push", "--quiet", "origin", "main"]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "-b", "topic"]);
    std::fs::write(repo.join("change.txt"), "the reviewed bytes\n").expect("a change");
    git(&repo, &["add", "change.txt"]);
    git(&repo, &["commit", "--quiet", "-m", "topic"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    git(
        &repo,
        &[
            "push",
            "--quiet",
            "origin",
            "HEAD:refs/pull/7/head",
            "HEAD:refs/heads/topic",
        ],
    );
    git(&repo, &["checkout", "--quiet", "main"]);
    git(&repo, &["branch", "--quiet", "-D", "topic"]);
    git(&repo, &["gc", "--quiet", "--prune=now"]);
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: repo.clone(),
        config: Vec::new(),
        repo: Some("acme/widget".to_owned()),
    };
    Rig {
        root,
        repo,
        origin,
        head,
        base,
        context,
    }
}

fn pr_view(rig: &Rig, overrides: serde_json::Value) -> serde_json::Value {
    let mut view = serde_json::json!({
        "state": "OPEN",
        "isCrossRepository": false,
        "author": { "login": "a-teammate" },
        "headRefOid": rig.head,
        "headRefName": "topic",
        "baseRefName": "main",
        "baseRefOid": rig.base,
    });
    for (key, value) in overrides.as_object().expect("an object") {
        view[key] = value.clone();
    }
    serde_json::json!({ "matches": "pr view 7", "stdout": view.to_string(), "status": 0 })
}

fn viewer() -> serde_json::Value {
    serde_json::json!({
        "matches": "api user",
        "stdout": serde_json::json!({ "login": "the-reviewer" }).to_string(),
        "status": 0,
    })
}

fn claiming(
    rig: &Rig,
    run_id: &str,
    view: serde_json::Value,
) -> Result<serde_json::Value, crate::transport::Failure> {
    let trees = rig.root.path().join("trees/<repo>/<branch>");
    let trees = trees.to_string_lossy().into_owned();
    let script = crate::test_env::answers(&[view, viewer()]);
    let log = rig.root.path().join("gh.log");
    crate::test_env::with_logging_gh(crate::test_env::scripted_gh().path(), &script, &log, || {
        claim(
            &rig.context,
            &ClaimRequest {
                pr: 7,
                run_id,
                horizon: HORIZON,
                worktree_root: Some(&trees),
                now: NOW,
            },
            None,
        )
    })
}

fn reason(failure: &crate::transport::Failure) -> String {
    match failure {
        crate::transport::Failure::Stop(answer) => {
            answer["reason"].as_str().unwrap_or_default().to_owned()
        }
        other => panic!("expected a stop, got {other:?}"),
    }
}

fn worktrees(repo: &Path) -> String {
    git(repo, &["worktree", "list", "--porcelain"])
}

#[test]
fn a_foreign_pull_request_yields_a_claim_and_a_detached_worktree() {
    let rig = rig();
    let answer = claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    assert_eq!(answer["pr"], 7);
    assert_eq!(answer["head"], rig.head.as_str());
    assert_eq!(answer["merge_base"], rig.base.as_str());
    assert_eq!(answer["base_tip"], rig.base.as_str());
    assert_eq!(answer["tracker_written"], false);
    let worktree = PathBuf::from(answer["worktree"].as_str().expect("a worktree"));
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), rig.head);
    assert!(worktree.join("change.txt").is_file());
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&worktree)
            .args(["symbolic-ref", "-q", "HEAD"])
            .output()
            .expect("git runs")
            .status
            .code()
            == Some(1),
        "the worktree is not detached"
    );
    assert!(worktrees(&rig.repo).contains("detached"));
}

#[test]
fn claiming_again_with_the_same_head_resumes_and_makes_nothing_new() {
    let rig = rig();
    claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let again = claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the renewal");
    assert_eq!(again["resumed_existing_worktree"], true);
}

#[test]
fn the_earliest_claim_wins_and_the_loser_makes_no_worktree() {
    let rig = rig();
    let won = claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let lost = claiming(&rig, OTHER, pr_view(&rig, serde_json::json!({}))).expect_err("held");
    assert_eq!(reason(&lost), "pull-review-held");
    assert!(PathBuf::from(won["worktree"].as_str().expect("a path")).is_dir());
    assert_eq!(worktrees(&rig.repo).matches("detached").count(), 1);
}

#[test]
fn a_claim_past_its_horizon_gives_way() {
    let rig = rig();
    claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let store = Store::open(&store_path().expect("a path")).expect("the store");
    let mut held = store.pull_review_of(RUN).expect("a read").expect("a row");
    held.run_id = OTHER.to_owned();
    let repo = crate::transport::claim::repo_of(&rig.context).expect("a repo");
    let now = held.horizon + 1;
    let taken = store.claim_pull_review(&repo, &held, now).expect("taken");
    assert_eq!(taken.run_id, OTHER);
    assert!(store.pull_review_of(RUN).expect("a read").is_none());
}

#[test]
fn a_head_that_moved_under_this_runs_claim_is_head_moved() {
    let rig = rig();
    claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let moved = git(
        &rig.origin,
        &[
            "commit-tree",
            "-m",
            "moved",
            &format!("{}^{{tree}}", rig.head),
            "-p",
            &rig.head,
        ],
    );
    git(&rig.origin, &["update-ref", "refs/pull/7/head", &moved]);
    let view = pr_view(&rig, serde_json::json!({ "headRefOid": moved }));
    let failure = claiming(&rig, RUN, view).expect_err("a moved head");
    assert_eq!(reason(&failure), "head-moved");
}

#[test]
fn a_head_the_fetch_cannot_bring_is_head_moved_and_nothing_is_made() {
    let rig = rig();
    let view = pr_view(&rig, serde_json::json!({ "headRefOid": "d".repeat(40) }));
    let failure = claiming(&rig, RUN, view).expect_err("an absent head");
    assert_eq!(reason(&failure), "head-moved");
    assert!(!worktrees(&rig.repo).contains("detached"));
}

#[test]
fn a_fork_a_closed_pull_request_and_this_accounts_own_stop_before_any_write() {
    for (overrides, expected) in [
        (
            serde_json::json!({ "isCrossRepository": true }),
            "pull-review-fork-refused",
        ),
        (
            serde_json::json!({ "state": "MERGED" }),
            "pull-review-not-open",
        ),
        (
            serde_json::json!({ "author": { "login": "the-reviewer" } }),
            "pull-review-own-pull-request",
        ),
    ] {
        let rig = rig();
        let failure = claiming(&rig, RUN, pr_view(&rig, overrides)).expect_err("a refusal");
        assert_eq!(reason(&failure), expected);
        assert!(!worktrees(&rig.repo).contains("detached"));
        let store = Store::open(&store_path().expect("a path")).expect("the store");
        assert!(store.pull_review_of(RUN).expect("a read").is_none());
    }
}

#[test]
fn an_unreadable_fork_field_is_not_read_as_this_repository() {
    let rig = rig();
    let view = pr_view(&rig, serde_json::json!({ "isCrossRepository": null }));
    let failure = claiming(&rig, RUN, view).expect_err("a refusal");
    assert_eq!(reason(&failure), "pull-review-fork-refused");
}

#[test]
fn the_reviewed_bytes_never_run_a_hook_when_the_worktree_is_made() {
    let rig = rig();
    git(&rig.repo, &["config", "core.hooksPath", "hooks"]);
    let hooks = rig.repo.join("hooks");
    std::fs::create_dir_all(&hooks).expect("hooks");
    let witness = rig.root.path().join("the-hook-ran");
    let hook = hooks.join("post-checkout");
    std::fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", witness.display())).expect("a hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }
    claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    assert!(!witness.exists(), "a repository hook ran on foreign bytes");
}

#[test]
fn releasing_removes_the_worktree_and_the_row_and_reads_nothing_from_github() {
    let rig = rig();
    let held = claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let worktree = PathBuf::from(held["worktree"].as_str().expect("a path"));
    let answer = release(&rig.context, RUN).expect("the release");
    assert_eq!(answer["released"], true);
    assert_eq!(answer["checkout"]["removed"], true);
    assert!(!worktree.exists());
    assert!(!worktrees(&rig.repo).contains("detached"));
    let store = Store::open(&store_path().expect("a path")).expect("the store");
    assert!(store.pull_review_of(RUN).expect("a read").is_none());
}

#[test]
fn a_leftover_worktree_reads_as_carrying_nothing_unpublished() {
    let rig = rig();
    let held = claiming(&rig, RUN, pr_view(&rig, serde_json::json!({}))).expect("the claim");
    let worktree = PathBuf::from(held["worktree"].as_str().expect("a path"));
    assert_eq!(
        git(
            &worktree,
            &["rev-list", "--count", "HEAD", "--not", "--remotes"]
        ),
        "0",
        "prune keeps a checkout whose head no remote-tracking ref reaches"
    );
}

#[test]
fn releasing_with_nothing_held_says_so() {
    let rig = rig();
    let answer = release(&rig.context, RUN).expect("the release");
    assert_eq!(answer["released"], false);
}
