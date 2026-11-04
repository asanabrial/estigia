use std::path::Path;
use std::sync::{Arc, Mutex};

use super::{collect_with, launch_with, linear_key_in, status};
use crate::outcome::Refusal;
use crate::reviewer::job::{OnThread, Spawner};
use crate::reviewer::{Adapter, Invocation};
use crate::transport::Context;
use crate::transport::pull_review::{ClaimRequest, claim};
use crate::transport::store::{Store, store_path};

const RUN: &str = "claude-abcd1234";
const NOW: &str = "2026-11-04T10:00:00Z";
const HORIZON: &str = "2099-11-04T18:00Z";
const LENS: &str = "review-reliability";

struct Rig {
    root: tempfile::TempDir,
    head: String,
    base: String,
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

fn rig(topic_file: &str, bytes: usize) -> Rig {
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
    std::fs::write(
        repo.join("estigia.generated.toml"),
        "[[generated]]\npath = \"Cargo.lock\"\ncommand = [\"true\"]\n",
    )
    .expect("a declaration");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "base"]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "-b", "topic"]);
    std::fs::write(repo.join(topic_file), "x\n".repeat(bytes / 2)).expect("a change");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "topic"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "--quiet", "main"]);
    std::fs::write(repo.join("target-gained.txt"), "only on the target\n").expect("a file");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "the target moved"]);
    let tip = git(&repo, &["rev-parse", "HEAD"]);
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
        base,
        tip,
        context,
    }
}

fn answers(rig: &Rig) -> String {
    crate::test_env::answers(&[
        serde_json::json!({
            "matches": "--json state,isCrossRepository",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "isCrossRepository": false,
                "author": { "login": "a-teammate" },
                "headRefOid": rig.head,
                "headRefName": "topic",
                "baseRefName": "main",
                "baseRefOid": rig.tip,
            })
            .to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "api user",
            "stdout": serde_json::json!({ "login": "the-reviewer" }).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "--json headRefName,closingIssuesReferences",
            "stdout": serde_json::json!({
                "headRefName": "topic",
                "closingIssuesReferences": [{ "number": 12 }],
            })
            .to_string(),
            "status": 0,
        }),
        crate::test_env::issue_view_answer(
            12,
            serde_json::json!({ "body": "THE TRACKER ISSUE TEXT" }),
        ),
    ])
}

fn gh<T>(rig: &Rig, body: impl FnOnce() -> T) -> T {
    let log = rig.root.path().join("gh.log");
    crate::test_env::with_logging_gh(
        crate::test_env::scripted_gh().path(),
        &answers(rig),
        &log,
        body,
    )
}

fn claimed(rig: &Rig) -> serde_json::Value {
    let trees = rig.root.path().join("trees/<repo>/<branch>");
    let trees = trees.to_string_lossy().into_owned();
    gh(rig, || {
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
    .expect("the claim")
}

type Seen = Arc<Mutex<Vec<Vec<u8>>>>;

struct Stub {
    by_role: Vec<(String, Vec<u8>)>,
    seen: Seen,
}

impl Adapter for Stub {
    fn review(&self, invocation: &Invocation) -> Result<Vec<u8>, Refusal> {
        self.seen
            .lock()
            .expect("the lock")
            .push(invocation.prompt.clone());
        let answer = self
            .by_role
            .iter()
            .find(|(role, _)| *role == invocation.role)
            .map_or_else(
                || br#"{"findings":[]}"#.to_vec(),
                |(_, bytes)| bytes.clone(),
            );
        Ok(answer)
    }
}

fn spawner(by_role: &[(&str, &str)], seen: &Seen) -> impl Spawner + use<> {
    let by_role: Vec<(String, Vec<u8>)> = by_role
        .iter()
        .map(|(role, answer)| ((*role).to_owned(), answer.as_bytes().to_vec()))
        .collect();
    let seen = Arc::clone(seen);
    OnThread(move || {
        Ok(Box::new(Stub {
            by_role: by_role.clone(),
            seen: Arc::clone(&seen),
        }) as Box<dyn Adapter>)
    })
}

fn collect(rig: &Rig) -> serde_json::Value {
    gh(rig, || collect_with(&rig.context, RUN, 10, &|_| {})).expect("the collection")
}

fn severe(id: &str, evidence_class: &str) -> String {
    serde_json::json!({ "findings": [{
        "id": id,
        "class": "severe",
        "evidence": "change.txt:1 does the wrong thing",
        "impact": "the wrong thing happens to whoever runs it",
        "location": "change.txt:1",
        "evidence_class": evidence_class,
        "causal_disposition": "introduced",
    }]})
    .to_string()
}

#[test]
fn the_panel_reads_the_merge_base_through_the_head_and_nothing_the_target_gained() {
    let rig = rig("change.txt", 20);
    let held = claimed(&rig);
    assert_eq!(held["merge_base"], rig.base.as_str());
    assert_eq!(held["base_tip"], rig.tip.as_str());
    let seen = Seen::default();
    let launched = gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_000)
    })
    .expect("the launch");
    assert_eq!(launched["criteria"], "fixed-and-tracker");
    let answer = collect(&rig);
    assert_eq!(answer["verdict"], "approved", "{answer}");
    assert_eq!(answer["head"], rig.head.as_str());
    let prompts = seen.lock().expect("the lock");
    let prompt = String::from_utf8_lossy(&prompts[0]);
    assert!(prompt.contains("change.txt"));
    assert!(
        !prompt.contains("target-gained"),
        "the moved target inflated the diff"
    );
}

#[test]
fn the_tracker_issue_is_criteria_and_the_pull_request_description_is_never_asked_for() {
    let rig = rig("change.txt", 20);
    claimed(&rig);
    let seen = Seen::default();
    gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_000)
    })
    .expect("the launch");
    collect(&rig);
    let prompts = seen.lock().expect("the lock");
    assert!(String::from_utf8_lossy(&prompts[0]).contains("THE TRACKER ISSUE TEXT"));
    let log = std::fs::read_to_string(rig.root.path().join("gh.log")).expect("the log");
    for line in log.lines().filter(|line| line.contains("pr view")) {
        assert!(
            !line.contains("body"),
            "a read asked for the description: {line}"
        );
    }
}

#[test]
fn a_deterministic_severe_finding_requests_changes() {
    let rig = rig("change.txt", 20);
    claimed(&rig);
    let seen = Seen::default();
    let answer = severe("wrong", "deterministic");
    gh(&rig, || {
        launch_with(
            &rig.context,
            RUN,
            &spawner(&[(LENS, &answer)], &seen),
            1_000,
        )
    })
    .expect("the launch");
    assert_eq!(collect(&rig)["verdict"], "changes-requested");
}

#[test]
fn an_inferential_severe_finding_goes_to_the_refuter_before_any_verdict() {
    let rig = rig("change.txt", 20);
    claimed(&rig);
    let seen = Seen::default();
    let answer = severe("maybe", "inferential");
    let refuted =
        serde_json::json!({ "outcomes": [{ "id": "maybe", "outcome": "refuted" }] }).to_string();
    let lens_then_refuter = spawner(&[(LENS, &answer), ("review-refuter", &refuted)], &seen);
    gh(&rig, || {
        launch_with(&rig.context, RUN, &lens_then_refuter, 1_000)
    })
    .expect("the lens");
    let waiting = collect(&rig);
    assert_eq!(waiting["verdict"], "incomplete", "{waiting}");
    assert_eq!(waiting["next"]["names"][0], "refutation-batch");
    gh(&rig, || {
        launch_with(&rig.context, RUN, &lens_then_refuter, 1_001)
    })
    .expect("the refuter");
    assert_eq!(collect(&rig)["verdict"], "approved");
}

#[test]
fn a_change_over_the_ceiling_answers_that_no_review_ran_and_no_verdict() {
    let rig = rig(
        "big.txt",
        crate::transport::target::REVIEW_PAYLOAD_CEILING + 1024,
    );
    claimed(&rig);
    let answer = gh(&rig, || status(&rig.context, RUN)).expect("the status");
    assert_eq!(answer["reviewed"], false);
    assert_eq!(answer["verdict"], serde_json::Value::Null);
    assert_eq!(answer["reason"], "review-payload-too-large");
    assert!(
        answer["detail"]
            .as_str()
            .expect("a detail")
            .contains("no review ran")
    );
    let collected = collect(&rig);
    assert_eq!(collected["reviewed"], false, "{collected}");
    assert_eq!(collected["verdict"], serde_json::Value::Null);
    assert_eq!(collected["next"]["kind"], "stop", "{collected}");
    let seen = Seen::default();
    let failure = gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_000)
    })
    .expect_err("no launch over the ceiling");
    assert!(
        failure
            .envelope()
            .to_string()
            .contains("review-payload-too-large")
    );
    assert!(seen.lock().expect("the lock").is_empty());
}

#[test]
fn a_declared_generated_file_still_reaches_the_reader() {
    let rig = rig("Cargo.lock", 20);
    let withheld =
        crate::transport::target::review_payload(&rig.context.repo_dir, &rig.base, &rig.head)
            .expect("the ordinary payload");
    assert!(
        !withheld.text.contains("@@"),
        "the base's declaration did not withhold the file, so this test measures nothing"
    );
    claimed(&rig);
    let seen = Seen::default();
    gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_000)
    })
    .expect("the launch");
    collect(&rig);
    let prompts = seen.lock().expect("the lock");
    let prompt = String::from_utf8_lossy(&prompts[0]);
    assert!(
        prompt.contains("@@"),
        "the generated file's hunks were withheld"
    );
}

#[test]
fn an_answer_that_will_not_parse_leaves_the_slot_to_launch_again() {
    let rig = rig("change.txt", 20);
    claimed(&rig);
    let seen = Seen::default();
    gh(&rig, || {
        launch_with(
            &rig.context,
            RUN,
            &spawner(&[(LENS, "not json")], &seen),
            1_000,
        )
    })
    .expect("the launch");
    let answer = collect(&rig);
    assert_eq!(answer["verdict"], "incomplete");
    assert_eq!(answer["next"]["kind"], "launch");
    gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_001)
    })
    .expect("the second launch");
    assert_eq!(collect(&rig)["verdict"], "approved");
}

#[test]
fn releasing_forgets_the_reviewers_answers() {
    let rig = rig("change.txt", 20);
    claimed(&rig);
    let seen = Seen::default();
    gh(&rig, || {
        launch_with(&rig.context, RUN, &spawner(&[], &seen), 1_000)
    })
    .expect("the launch");
    collect(&rig);
    let store = Store::open(&store_path().expect("a path")).expect("the store");
    let epoch = store
        .pull_review_of(RUN)
        .expect("a read")
        .expect("a row")
        .epoch;
    assert!(!store.pull_review_slots(&epoch).expect("a read").is_empty());
    crate::transport::pull_review::release(&rig.context, RUN).expect("the release");
    assert!(store.pull_review_slots(&epoch).expect("a read").is_empty());
}

#[test]
fn a_key_is_read_from_the_branch_only_where_it_stands_alone() {
    assert_eq!(
        linear_key_in("feature/abc-102-add-a-thing", "ABC"),
        Some("ABC-102".to_owned())
    );
    assert_eq!(linear_key_in("xabc-102", "ABC"), None);
    assert_eq!(linear_key_in("abc-thing", "ABC"), None);
}
