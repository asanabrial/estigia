use std::path::Path;
use std::sync::{Arc, Mutex};

mod voice;

use super::{submit, submit_with};
use crate::outcome::Refusal;
use crate::reviewer::job::{OnThread, Spawner};
use crate::reviewer::{Adapter, Invocation};
use crate::transport::Context;
use crate::transport::pull_review::panel::{collect_with, launch_with};
use crate::transport::pull_review::{ClaimRequest, claim};
use crate::transport::store::{Store, store_path};

const RUN: &str = "claude-abcd1234";
const NOW: &str = "2026-11-04T10:00:00Z";
const HORIZON: &str = "2099-11-04T18:00Z";

struct Rig {
    root: tempfile::TempDir,
    head: String,
    tip: String,
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

fn rig() -> Rig {
    crate::transport::store::forget_scratch_store();
    let root = tempfile::tempdir().expect("a temporary root");
    let origin = root.path().join("origin.git");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&origin).expect("a remote");
    std::fs::create_dir_all(&repo).expect("a repository");
    git(&origin, &["init", "--quiet", "--bare", "-b", "main"]);
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    git(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    std::fs::write(repo.join("readme.txt"), "base\n").expect("a file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let tip = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "-b", "topic"]);
    std::fs::write(repo.join("change.txt"), "a change\n").expect("a change");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "topic"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "main"]);
    git(
        &repo,
        &[
            "push",
            "--quiet",
            "origin",
            "main",
            "topic",
            "topic:refs/pull/7/head",
        ],
    );
    git(&repo, &["branch", "--quiet", "-D", "topic"]);
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: repo,
        config: Vec::new(),
        repo: Some("acme/widget".to_owned()),
    };
    Rig {
        root,
        head,
        tip,
        context,
    }
}

fn view(rig: &Rig, head: &str, state: &str, base: &str) -> serde_json::Value {
    serde_json::json!({
        "matches": "pr view 7",
        "stdout": serde_json::json!({
            "state": state,
            "isCrossRepository": false,
            "author": { "login": "a-teammate" },
            "headRefOid": head,
            "headRefName": "topic",
            "baseRefName": base,
            "baseRefOid": rig.tip,
            "closingIssuesReferences": [],
        })
        .to_string(),
        "status": 0,
    })
}

fn user() -> serde_json::Value {
    serde_json::json!({
        "matches": "api user",
        "stdout": serde_json::json!({ "login": "the-reviewer" }).to_string(),
        "status": 0,
    })
}

fn gh<T>(rig: &Rig, entries: &[serde_json::Value], body: impl FnOnce() -> T) -> T {
    let log = rig.root.path().join("gh.log");
    crate::test_env::with_logging_gh(
        crate::test_env::scripted_gh().path(),
        &crate::test_env::answers(entries),
        &log,
        body,
    )
}

fn log(rig: &Rig) -> String {
    std::fs::read_to_string(rig.root.path().join("gh.log")).unwrap_or_default()
}

struct Stub(Vec<u8>);

impl Adapter for Stub {
    fn review(&self, _: &Invocation) -> Result<Vec<u8>, Refusal> {
        Ok(self.0.clone())
    }
}

fn spawner(answer: &str) -> impl Spawner + use<> {
    let answer = Arc::new(Mutex::new(answer.as_bytes().to_vec()));
    OnThread(move || {
        Ok(Box::new(Stub(answer.lock().expect("the lock").clone())) as Box<dyn Adapter>)
    })
}

struct Slow(Stub, std::time::Duration);

impl Adapter for Slow {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        std::thread::sleep(self.1);
        self.0.review(invocation)
    }
}

fn spawner_after(delay: std::time::Duration, answer: &str) -> impl Spawner + use<> {
    let answer = answer.as_bytes().to_vec();
    OnThread(move || Ok(Box::new(Slow(Stub(answer.clone()), delay)) as Box<dyn Adapter>))
}

// A review taken as far as a computed verdict. Answers `worktree`.
fn reviewed(rig: &Rig, answer: &str) -> std::path::PathBuf {
    reviewed_by(rig, &spawner(answer))
}

fn reviewed_by(rig: &Rig, spawner: &dyn Spawner) -> std::path::PathBuf {
    let trees = rig.root.path().join("trees/<repo>/<branch>");
    let trees = trees.to_string_lossy().into_owned();
    let open = view(rig, &rig.head, "OPEN", "main");
    let held = gh(rig, &[open.clone(), user()], || {
        claim(
            &rig.context,
            &ClaimRequest {
                pr: 7,
                run_id: RUN,
                horizon: HORIZON,
                worktree_root: Some(&trees),
                now: NOW,
            },
            None,
        )
    })
    .expect("the claim");
    let now = crate::harness::session::now_seconds().expect("a clock");
    gh(rig, std::slice::from_ref(&open), || {
        launch_with(&rig.context, RUN, spawner, now)
    })
    .expect("the launch");
    let collected = gh(rig, &[open], || {
        collect_with(&rig.context, RUN, 10, &|_| {})
    })
    .expect("the collection");
    assert_ne!(collected["verdict"], "incomplete", "{collected}");
    std::path::PathBuf::from(held["worktree"].as_str().expect("a worktree"))
}

fn no_reviews() -> serde_json::Value {
    serde_json::json!({ "matches": "--paginate --slurp", "stdout": "[[]]", "status": 0 })
}

fn posted(id: u64) -> serde_json::Value {
    serde_json::json!({
        "matches": "--method POST",
        "stdout": serde_json::json!({ "id": id }).to_string(),
        "status": 0,
    })
}

fn read_back(id: u64, commit: &str, state: &str) -> serde_json::Value {
    serde_json::json!({
        "matches": format!("reviews/{id}"),
        "stdout": serde_json::json!({
            "id": id,
            "commit_id": commit,
            "state": state,
            "html_url": "https://github.com/acme/widget/pull/7#pullrequestreview-1",
        })
        .to_string(),
        "status": 0,
    })
}

fn comments_on(id: u64, paths: &[&str]) -> serde_json::Value {
    let page: Vec<serde_json::Value> = paths
        .iter()
        .map(|path| serde_json::json!({ "path": path, "body": "text" }))
        .collect();
    serde_json::json!({
        "matches": format!("reviews/{id}/comments"),
        "stdout": serde_json::json!([page]).to_string(),
        "status": 0,
    })
}

fn request(rig: &Rig) -> serde_json::Value {
    let written = log(rig);
    let line = written
        .lines()
        .find(|line| line.contains("--method POST"))
        .expect("a review was posted");
    let (_, rest) = line.split_once("--input ").expect("sent with --input");
    let path = rest.split('\t').next().unwrap_or_default().trim();
    let document = std::fs::read_to_string(path).expect("the staged request");
    serde_json::from_str(&document).expect("a JSON request")
}

const CLEAN: &str = r#"{"findings":[]}"#;

fn mixed_answer() -> String {
    serde_json::json!({ "findings": [
        {
            "id": "wrong",
            "class": "severe",
            "evidence": "change.txt:1 does the wrong thing",
            "impact": "the wrong thing happens",
            "location": "change.txt:1",
            "evidence_class": "deterministic",
            "causal_disposition": "introduced",
        },
        {
            "id": "elsewhere",
            "class": "warning",
            "evidence": "readme.txt:1 is stale",
            "impact": "readers are misled",
            "location": "readme.txt:1",
        },
        {
            "id": "spans",
            "class": "warning",
            "evidence": "change.txt:1-2 spans past the hunk",
            "impact": "the range leaves the change",
            "location": "change.txt:1-2",
        },
    ]})
    .to_string()
}

fn severe_answer() -> String {
    serde_json::json!({ "findings": [{
        "id": "wrong",
        "class": "severe",
        "evidence": "change.txt:1 does the wrong thing",
        "impact": "the wrong thing happens",
        "location": "change.txt:1",
        "evidence_class": "deterministic",
        "causal_disposition": "introduced",
    }]})
    .to_string()
}

fn gone(rig: &Rig, worktree: &Path) {
    assert!(!worktree.exists(), "the worktree outlived the review");
    let store = Store::open(&store_path().expect("a path")).expect("the store");
    assert!(store.pull_review_of(RUN).expect("a read").is_none());
    assert!(
        !git(&rig.context.repo_dir, &["worktree", "list"]).contains("review-pr-7"),
        "the worktree is still registered"
    );
}

#[test]
fn a_clean_review_approves_carrying_the_reviewed_commit_and_reports_only_the_read_back() {
    let rig = rig();
    let worktree = reviewed(&rig, CLEAN);
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            no_reviews(),
            posted(41),
            read_back(41, &rig.head, "APPROVED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the submission");
    assert_eq!(answer["submitted"], true);
    assert_eq!(answer["event"], "APPROVE");
    assert_eq!(answer["commit_id"], rig.head.as_str());
    assert_eq!(answer["state"], "APPROVED");
    let sent = request(&rig);
    assert_eq!(sent["commit_id"], rig.head.as_str());
    assert_eq!(sent["event"], "APPROVE");
    gone(&rig, &worktree);
}

#[test]
fn a_reviewer_that_answers_after_the_first_poll_is_collected_not_abandoned() {
    let rig = rig();
    let worktree = reviewed_by(
        &rig,
        &spawner_after(std::time::Duration::from_millis(250), CLEAN),
    );
    assert!(worktree.exists(), "the delayed answer left no worktree");
}

#[test]
fn a_severe_finding_requests_changes_and_the_body_carries_it() {
    let rig = rig();
    let worktree = reviewed(&rig, &severe_answer());
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            comments_on(41, &["change.txt"]),
            no_reviews(),
            posted(41),
            read_back(41, &rig.head, "CHANGES_REQUESTED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the submission");
    assert_eq!(answer["event"], "REQUEST_CHANGES");
    let sent = request(&rig);
    assert_eq!(sent["event"], "REQUEST_CHANGES");
    let text = sent.to_string();
    assert!(text.contains("does the wrong thing"), "{text}");
    let body = sent["body"].as_str().expect("a body");
    for trace in ["<!--", "blind review", "`wrong`", "**severe**", &rig.head] {
        assert!(
            !body.contains(trace),
            "the review carries {trace:?}: {body}"
        );
    }
    gone(&rig, &worktree);
}

#[test]
fn a_finding_inside_a_hunk_is_an_inline_comment_and_the_rest_stay_in_the_body() {
    let rig = rig();
    let worktree = reviewed(&rig, &mixed_answer());
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            comments_on(41, &["change.txt"]),
            no_reviews(),
            posted(41),
            read_back(41, &rig.head, "CHANGES_REQUESTED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the submission");
    assert_eq!(answer["submitted"], true, "{answer}");
    let sent = request(&rig);
    assert_eq!(sent["commit_id"], rig.head.as_str());
    assert_eq!(sent["event"], "REQUEST_CHANGES");
    assert_eq!(
        sent["comments"],
        serde_json::json!([{
            "path": "change.txt",
            "line": 1,
            "side": "RIGHT",
            "body": "change.txt:1 does the wrong thing the wrong thing happens",
        }])
    );
    let body = sent["body"].as_str().expect("a body");
    assert!(!body.contains("does the wrong thing"), "{body}");
    assert!(body.contains("readme.txt:1"), "{body}");
    assert!(body.contains("change.txt:1-2"), "{body}");
    assert!(body.contains("1 posted inline"), "{body}");
    gone(&rig, &worktree);
}

#[test]
fn a_review_whose_comments_did_not_land_is_unconfirmed() {
    for listed in [
        &[][..],
        &["readme.txt"][..],
        &["change.txt", "change.txt"][..],
    ] {
        let rig = rig();
        let worktree = reviewed(&rig, &mixed_answer());
        let failure = gh(
            &rig,
            &[
                view(&rig, &rig.head, "OPEN", "main"),
                comments_on(41, listed),
                no_reviews(),
                posted(41),
                read_back(41, &rig.head, "CHANGES_REQUESTED"),
            ],
            || submit(&rig.context, RUN),
        )
        .expect_err("not confirmed");
        assert!(
            failure
                .envelope()
                .to_string()
                .contains("pull-review-submission-unconfirmed"),
            "{}",
            failure.envelope()
        );
        assert!(worktree.is_dir(), "the claim was put down");
    }
}

#[test]
fn an_approval_with_no_finding_sends_no_comments() {
    let rig = rig();
    reviewed(&rig, CLEAN);
    gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            no_reviews(),
            posted(41),
            read_back(41, &rig.head, "APPROVED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the submission");
    let sent = request(&rig);
    assert!(sent.get("comments").is_none(), "{sent}");
    assert_eq!(sent["event"], "APPROVE");
}

#[test]
fn another_accounts_review_of_the_same_commit_is_not_taken_for_this_one() {
    let rig = rig();
    reviewed(&rig, CLEAN);
    let theirs = serde_json::json!([[{
        "id": 40,
        "commit_id": rig.head,
        "user": { "login": "a-teammate" },
        "body": "",
    }]]);
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            serde_json::json!({
                "matches": "--paginate --slurp",
                "stdout": theirs.to_string(),
                "status": 0,
            }),
            posted(41),
            read_back(41, &rig.head, "APPROVED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the submission");
    assert_eq!(answer["review_id"], 41, "{answer}");
    assert!(log(&rig).contains("--method POST"), "nothing was sent");
}

#[test]
fn a_head_that_moved_submits_nothing_reads_head_moved_and_removes_the_worktree() {
    let rig = rig();
    let worktree = reviewed(&rig, CLEAN);
    let moved = "d".repeat(40);
    let answer = gh(
        &rig,
        &[view(&rig, &moved, "OPEN", "main"), no_reviews()],
        || submit(&rig.context, RUN),
    )
    .expect("an answer");
    assert_eq!(answer["submitted"], false);
    assert_eq!(answer["reason"], "head-moved");
    assert!(!log(&rig).contains("--method POST"), "something was sent");
    gone(&rig, &worktree);
}

#[test]
fn a_retarget_and_a_closed_pull_request_submit_nothing() {
    for (state, base, reason) in [
        ("OPEN", "another-base", "pull-review-retargeted"),
        ("MERGED", "main", "pull-review-not-open"),
    ] {
        let rig = rig();
        let worktree = reviewed(&rig, CLEAN);
        let answer = gh(
            &rig,
            &[view(&rig, &rig.head, state, base), no_reviews()],
            || submit(&rig.context, RUN),
        )
        .expect("an answer");
        assert_eq!(answer["reason"], reason);
        assert_eq!(answer["submitted"], false);
        assert!(!log(&rig).contains("--method POST"), "something was sent");
        gone(&rig, &worktree);
    }
}

#[test]
fn a_verdict_with_a_lens_unanswered_is_refused_and_keeps_the_claim() {
    let rig = rig();
    let trees = rig.root.path().join("trees/<repo>/<branch>");
    let trees = trees.to_string_lossy().into_owned();
    let open = view(&rig, &rig.head, "OPEN", "main");
    let held = gh(&rig, &[open, user()], || {
        claim(
            &rig.context,
            &ClaimRequest {
                pr: 7,
                run_id: RUN,
                horizon: HORIZON,
                worktree_root: Some(&trees),
                now: NOW,
            },
            None,
        )
    })
    .expect("the claim");
    let failure = gh(&rig, &[], || submit(&rig.context, RUN)).expect_err("nothing answered");
    assert!(
        failure
            .envelope()
            .to_string()
            .contains("pull-review-incomplete"),
        "{}",
        failure.envelope()
    );
    assert!(std::path::Path::new(held["worktree"].as_str().expect("a path")).is_dir());
    assert!(!log(&rig).contains("--method POST"));
}

#[test]
fn a_read_back_that_disagrees_is_unconfirmed_and_keeps_the_claim_for_the_retry() {
    let rig = rig();
    let worktree = reviewed(&rig, CLEAN);
    let failure = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            no_reviews(),
            posted(41),
            read_back(41, &"e".repeat(40), "APPROVED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect_err("not confirmed");
    assert!(
        failure
            .envelope()
            .to_string()
            .contains("pull-review-submission-unconfirmed"),
        "{}",
        failure.envelope()
    );
    assert!(
        worktree.is_dir(),
        "the claim was put down with its outcome unknown"
    );
}

#[test]
fn a_retry_finds_the_review_a_lost_answer_left_and_sends_no_second_one() {
    let rig = rig();
    reviewed(&rig, CLEAN);
    let existing = serde_json::json!([[{
        "id": 41,
        "commit_id": rig.head,
        "user": { "login": "the-reviewer" },
        "body": "",
    }]]);
    let answer = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            serde_json::json!({
                "matches": "--paginate --slurp",
                "stdout": existing.to_string(),
                "status": 0,
            }),
            read_back(41, &rig.head, "APPROVED"),
        ],
        || submit(&rig.context, RUN),
    )
    .expect("the retry");
    assert_eq!(answer["submitted"], true);
    assert!(
        !log(&rig).contains("--method POST"),
        "a second verdict was sent"
    );
}

#[test]
fn a_retry_after_the_head_moved_answers_for_the_review_that_landed_rather_than_nothing_sent() {
    for (state, base, moved) in [
        ("OPEN", "main", true),
        ("MERGED", "main", false),
        ("OPEN", "another-base", false),
    ] {
        let rig = rig();
        let worktree = reviewed(&rig, CLEAN);
        let live = if moved {
            "d".repeat(40)
        } else {
            rig.head.clone()
        };
        let existing = serde_json::json!([[{
            "id": 41,
            "commit_id": rig.head,
            "user": { "login": "the-reviewer" },
            "body": "",
        }]]);
        let answer = gh(
            &rig,
            &[
                view(&rig, &live, state, base),
                serde_json::json!({
                    "matches": "--paginate --slurp",
                    "stdout": existing.to_string(),
                    "status": 0,
                }),
                read_back(41, &rig.head, "APPROVED"),
            ],
            || submit(&rig.context, RUN),
        )
        .expect("the retry");
        assert_eq!(answer["submitted"], true, "{answer}");
        assert_eq!(answer["commit_id"], rig.head.as_str());
        assert_eq!(answer["head_moved_during_submission"], moved);
        assert!(
            !log(&rig).contains("--method POST"),
            "a second verdict was sent"
        );
        gone(&rig, &worktree);
    }
}

#[test]
fn an_unreadable_search_for_the_first_review_is_unconfirmed_and_sends_nothing() {
    let rig = rig();
    let worktree = reviewed(&rig, CLEAN);
    let failure = gh(
        &rig,
        &[
            view(&rig, &rig.head, "OPEN", "main"),
            serde_json::json!({ "matches": "--paginate --slurp", "stdout": "", "stderr": "HTTP 502", "status": 1 }),
        ],
        || submit(&rig.context, RUN),
    )
    .expect_err("not confirmed");
    assert!(
        failure
            .envelope()
            .to_string()
            .contains("pull-review-submission-unconfirmed"),
        "{}",
        failure.envelope()
    );
    assert!(!log(&rig).contains("--method POST"));
    assert!(
        worktree.is_dir(),
        "the claim was put down with its outcome unknown"
    );
}

#[test]
fn an_escalated_verdict_sends_nothing_and_puts_the_claim_down() {
    let rig = rig();
    let insufficient = serde_json::json!({ "findings": [{
        "id": "unsure",
        "class": "severe",
        "evidence": "change.txt:1 may do the wrong thing",
        "impact": "the wrong thing may happen",
        "location": "change.txt:1",
        "evidence_class": "insufficient",
        "causal_disposition": "introduced",
    }]})
    .to_string();
    let worktree = reviewed(&rig, &insufficient);
    let answer = gh(&rig, &[], || submit(&rig.context, RUN)).expect("an answer");
    assert_eq!(answer["submitted"], false);
    assert_eq!(answer["reason"], "pull-review-escalated");
    assert!(!log(&rig).contains("--method POST"));
    gone(&rig, &worktree);
}

#[test]
fn a_change_over_the_ceiling_is_never_submitted_clean() {
    let rig = rig();
    let repo = rig.context.repo_dir.clone();
    git(&repo, &["checkout", "--quiet", "-b", "big"]);
    std::fs::write(
        repo.join("big.txt"),
        "x\n".repeat(crate::transport::target::REVIEW_PAYLOAD_CEILING),
    )
    .expect("a large change");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "big"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "main"]);
    git(
        &repo,
        &[
            "push",
            "--quiet",
            "origin",
            "big:refs/heads/topic",
            "big:refs/pull/7/head",
            "--force",
        ],
    );
    let trees = rig.root.path().join("trees/<repo>/<branch>");
    let trees = trees.to_string_lossy().into_owned();
    let open = view(&rig, &head, "OPEN", "main");
    let held = gh(&rig, &[open, user()], || {
        claim(
            &rig.context,
            &ClaimRequest {
                pr: 7,
                run_id: RUN,
                horizon: HORIZON,
                worktree_root: Some(&trees),
                now: NOW,
            },
            None,
        )
    })
    .expect("the claim");
    #[cfg(unix)]
    let mut reviewer = {
        use std::os::unix::process::CommandExt;
        let store = Store::open(&store_path().expect("a path")).expect("the store");
        let epoch = store
            .pull_review_of(RUN)
            .expect("a read")
            .expect("a row")
            .epoch;
        let job = rig.root.path().join("job");
        std::fs::create_dir_all(&job).expect("a job directory");
        store
            .insert_pull_review_slot(&epoch, "review-risk", &job.to_string_lossy(), 0)
            .expect("a slot");
        let child = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .process_group(0)
            .spawn()
            .expect("a reviewer");
        store
            .mark_pull_review_slot_spawned(&epoch, "review-risk", child.id())
            .expect("the pid");
        child
    };

    let answer = gh(&rig, &[], || submit(&rig.context, RUN)).expect("an answer");
    assert_eq!(answer["submitted"], false);
    assert_eq!(answer["reason"], "review-payload-too-large");

    #[cfg(unix)]
    {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match reviewer.try_wait() {
                Ok(Some(_)) => break,
                other => {
                    assert!(
                        std::time::Instant::now() < until,
                        "a refusal left the reviewer running: {other:?}"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }
    }
    assert!(!log(&rig).contains("--method POST"));
    gone(&rig, Path::new(held["worktree"].as_str().expect("a path")));
}

#[test]
fn only_a_whole_range_inside_one_hunk_is_a_site() {
    let mut changed = |_: &str| vec![3..=5, 9..=9];
    let inside =
        |location: &str, changed: &mut dyn FnMut(&str) -> Vec<std::ops::RangeInclusive<u32>>| {
            super::site_in_change(Some(location), changed).is_some()
        };
    assert!(inside("a.rs:4", &mut changed));
    assert!(inside("a.rs:3-5", &mut changed));
    assert!(!inside("a.rs:5-6", &mut changed), "partly inside");
    assert!(!inside("a.rs:5-9", &mut changed), "across two hunks");
    assert!(!inside("a.rs", &mut changed), "no line");
    assert!(super::site_in_change(None, &mut changed).is_none());
}

#[test]
fn a_range_comment_names_both_ends_on_the_right_side() {
    let site = crate::transport::claim::parse_finding_location("a.rs:3-5").expect("a range");
    let comment = super::Comment {
        site,
        text: "t".to_owned(),
    };
    assert_eq!(
        comment.json(),
        serde_json::json!({
            "path": "a.rs", "line": 5, "start_line": 3,
            "side": "RIGHT", "start_side": "RIGHT", "body": "t",
        })
    );
}
