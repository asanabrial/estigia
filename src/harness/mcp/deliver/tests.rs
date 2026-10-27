use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use super::*;
use crate::test_env::{answers, scripted_gh, with_counting_logging_gh};

fn git(dir: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(arguments)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct World {
    _origin: tempfile::TempDir,
    work: tempfile::TempDir,
    base: String,
    head: String,
    merge: String,
}

fn commit(dir: &Path, file: &str, text: &str, message: &str) -> String {
    std::fs::create_dir_all(dir.join(file).parent().expect("a parent")).expect("a directory");
    std::fs::write(dir.join(file), text).expect("a file");
    git(dir, &["add", file]);
    git(dir, &["commit", "--quiet", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn world(head_file: &str, head_text: &str) -> World {
    world_over(&[], |dir| commit(dir, head_file, head_text, "topic"))
}

fn world_over(base_files: &[(&str, &str)], topic: impl FnOnce(&Path) -> String) -> World {
    let origin = tempfile::tempdir().expect("an origin");
    git(origin.path(), &["init", "--quiet", "--bare", "-b", "main"]);
    let work = tempfile::tempdir().expect("a checkout");
    let dir = work.path();
    git(dir, &["init", "--quiet", "-b", "main"]);
    git(dir, &["config", "user.email", "nobody@example.invalid"]);
    git(dir, &["config", "user.name", "nobody"]);
    let mut base = commit(dir, "a.txt", "one\n", "base");
    for (file, text) in base_files {
        base = commit(dir, file, text, "base file");
    }
    git(
        dir,
        &[
            "remote",
            "add",
            "origin",
            &origin.path().display().to_string(),
        ],
    );
    git(dir, &["push", "--quiet", "origin", "main"]);
    git(dir, &["checkout", "--quiet", "-b", "topic"]);
    let head = topic(dir);
    let tree = git(dir, &["rev-parse", "HEAD^{tree}"]);
    let merge = git(
        dir,
        &[
            "commit-tree",
            &tree,
            "-p",
            &base,
            "-p",
            &head,
            "-m",
            "merge",
        ],
    );
    World {
        _origin: origin,
        work,
        base,
        head,
        merge,
    }
}

fn advance_base(world: &World, file: &str, text: &str) {
    let elsewhere = tempfile::tempdir().expect("another clone");
    let origin = git(world.work.path(), &["remote", "get-url", "origin"]);
    git(elsewhere.path(), &["clone", "--quiet", &origin, "."]);
    git(
        elsewhere.path(),
        &["config", "user.email", "nobody@example.invalid"],
    );
    git(elsewhere.path(), &["config", "user.name", "nobody"]);
    commit(elsewhere.path(), file, text, "the base moves");
    git(elsewhere.path(), &["push", "--quiet", "origin", "main"]);
}

struct Fake {
    repo: PathBuf,
    scratch: tempfile::TempDir,
    receipt: Option<ReviewTransaction>,
    scripts: RefCell<HashMap<String, Vec<Result<Value, Value>>>>,
    calls: RefCell<Vec<String>>,
    decision: Result<(), Value>,
    merge_by_agent: bool,
    delivery_auto: bool,
    language: String,
    stamp: Option<String>,
    lane: Value,
    checks: Result<Option<Value>, Value>,
    now: u64,
    evidence: RefCell<String>,
}

const NOW: u64 = 1_800_000_000;

fn receipt_of(world: &World) -> ReviewTransaction {
    ReviewTransaction {
        epoch: "e".repeat(32),
        pr: 7,
        head: world.head.clone(),
        base: world.base.clone(),
        digest: "d".repeat(64),
    }
}

fn status(receipt: &ReviewTransaction, next: Value) -> Result<Value, Value> {
    Ok(json!({
        "ok": true,
        "receipt": {
            "epoch": receipt.epoch, "pr": receipt.pr, "head": receipt.head,
            "base": receipt.base, "digest": receipt.digest,
        },
        "next": next,
        "forecast": {"horizon": "partial"},
    }))
}

fn execute(operation: &str) -> Value {
    json!({"kind": "execute", "reason_code": "r", "operation": operation, "arguments": {}})
}

fn halt(reason: &str) -> Value {
    json!({"kind": "stop", "reason_code": reason, "operation": null, "continuation": "c"})
}

fn fake(world: &World) -> Fake {
    let receipt = receipt_of(world);
    let mut scripts: HashMap<String, Vec<Result<Value, Value>>> = HashMap::new();
    scripts.insert("verify_claim".into(), vec![Ok(json!({"ok": true}))]);
    scripts.insert(
        "review_status".into(),
        vec![
            status(&receipt, execute("record_computed_verdict")),
            status(&receipt, execute("release_ci")),
        ],
    );
    scripts.insert(
        "record_computed_verdict".into(),
        vec![Ok(json!({"ok": true, "outcome": "approved"}))],
    );
    scripts.insert("release_ci".into(), vec![Ok(json!({"ok": true}))]);
    scripts.insert(
        "review_read".into(),
        vec![Ok(json!({
            "ok": true,
            "computed_level": "medium",
            "focus": "risk",
            "outcome": "approved",
            "open": [],
            "follow_ups": [],
            "findings": [{"id": "line-replaced", "class": "warning"}],
        }))],
    );
    scripts.insert(
        "release".into(),
        vec![Ok(json!({"ok": true, "transition": {"to": "done"}}))],
    );
    Fake {
        repo: world.work.path().to_path_buf(),
        scratch: tempfile::tempdir().expect("a scratch"),
        receipt: Some(receipt),
        scripts: RefCell::new(scripts),
        calls: RefCell::new(Vec::new()),
        decision: Ok(()),
        merge_by_agent: true,
        delivery_auto: true,
        language: "English".into(),
        stamp: None,
        lane: json!({"state": "running"}),
        checks: Ok(Some(
            json!({"passed": true, "checks": [{"name": "suite", "exit": 0}]}),
        )),
        now: NOW,
        evidence: RefCell::new(String::new()),
    }
}

impl Hands for Fake {
    fn tool(&self, name: &str, arguments: Value) -> Result<Value, Value> {
        self.calls.borrow_mut().push(name.to_owned());
        if name == "release" {
            let file = arguments["evidence_file"]
                .as_str()
                .expect("an evidence file");
            *self.evidence.borrow_mut() =
                std::fs::read_to_string(file).expect("evidence is written");
        }
        let mut scripts = self.scripts.borrow_mut();
        let script = scripts
            .get_mut(name)
            .unwrap_or_else(|| panic!("no script for {name}"));
        if script.len() > 1 {
            script.remove(0)
        } else {
            script[0].clone()
        }
    }

    fn exec(&self, args: &[&str], how: How) -> Result<Output, Failure> {
        transport::run(args, Some(&self.repo), how)
    }

    fn merge_decision(&self, _pr: u64) -> Result<(), Value> {
        self.calls.borrow_mut().push("merge_decision".to_owned());
        self.decision.clone()
    }

    fn receipt(&self) -> Option<ReviewTransaction> {
        self.receipt.clone()
    }

    fn worktree(&self) -> Option<String> {
        None
    }

    fn settings(&self) -> Settings {
        Settings {
            merge_by_agent: self.merge_by_agent,
            delivery_auto: self.delivery_auto,
            summary_language: self.language.clone(),
        }
    }

    fn release_stamp(&self, _receipt: &ReviewTransaction) -> Result<Option<String>, Value> {
        Ok(self.stamp.clone())
    }

    fn lane(&self, _receipt: &ReviewTransaction) -> Value {
        self.lane.clone()
    }

    fn checks_receipt(&self, _receipt: &ReviewTransaction) -> Result<Option<Value>, Value> {
        self.checks.clone()
    }

    fn scratch(&self) -> PathBuf {
        self.scratch.path().to_path_buf()
    }

    fn now(&self) -> u64 {
        self.now
    }
}

fn request() -> Request {
    Request {
        issue: 218,
        run_id: "claude-abcd1234".into(),
        wait: Duration::ZERO,
        readback_pause: Duration::ZERO,
        spend: None,
        appendix: None,
    }
}

fn check(status: &str, conclusion: &str) -> Value {
    json!({"__typename": "CheckRun", "name": "ci", "status": status, "conclusion": conclusion})
}

fn pr_view(world: &World, rollup: Value) -> Value {
    json!({
        "matches": "statusCheckRollup",
        "stdout": json!({
            "statusCheckRollup": rollup,
            "mergeStateStatus": "CLEAN",
            "headRefOid": world.head,
            "baseRefOid": world.base,
            "baseRefName": "main",
        }).to_string(),
        "status": 0,
    })
}

fn merged_view(world: &World) -> Vec<Value> {
    vec![json!({
        "matches": "mergeCommit",
        "stdout": json!({
            "state": "MERGED",
            "mergedAt": "2026-10-25T10:00:00Z",
            "mergeCommit": {"oid": world.merge},
            "baseRefName": "main",
        }).to_string(),
        "status": 0,
    })]
}

fn issue_views() -> Vec<Value> {
    vec![
        json!({"matches": "issue view", "nth": 1, "stdout": "{\"state\":\"OPEN\"}", "status": 0}),
        json!({"matches": "issue view", "nth": 2, "stdout": "{\"state\":\"CLOSED\"}", "status": 0}),
    ]
}

fn green_script(world: &World) -> Vec<Value> {
    let mut script = vec![
        pr_view(world, json!([check("COMPLETED", "SUCCESS")])),
        json!({"matches": "pr merge", "stdout": "", "status": 0}),
    ];
    script.extend(merged_view(world));
    script.extend(issue_views());
    script
}

struct Ran {
    answer: Value,
    gh: Vec<String>,
}

fn drive(fake: &Fake, script: &[Value], request: &Request) -> Ran {
    let bin = scripted_gh();
    let dir = tempfile::tempdir().expect("a log directory");
    let (counts, log) = (dir.path().join("counts.json"), dir.path().join("log"));
    let answer = with_counting_logging_gh(bin.path(), &answers(script), &counts, &log, || {
        run(fake, request)
    });
    let gh = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default().to_owned())
        .collect();
    Ran { answer, gh }
}

fn merged_by_gh(ran: &Ran) -> bool {
    ran.gh.iter().any(|line| line.starts_with("pr merge"))
}

fn stopped(ran: &Ran) -> (&str, &str) {
    let stopped = &ran.answer["stopped"];
    assert_eq!(ran.answer["delivered"], false, "{}", ran.answer);
    (
        stopped["step"].as_str().unwrap_or_default(),
        stopped["reason"].as_str().unwrap_or_default(),
    )
}

fn called(fake: &Fake) -> Vec<String> {
    fake.calls.borrow().clone()
}

#[test]
fn a_clean_review_runs_to_done_and_reads_the_merge_back() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let ran = drive(&fake, &green_script(&world), &request());

    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    assert_eq!(ran.answer["ok"], true);
    assert_eq!(ran.answer["merged"]["sha"], world.merge);
    assert_eq!(
        ran.answer["merged"]["parents"],
        json!([world.base, world.head]),
        "the merge's second parent is the reviewed head"
    );
    assert_eq!(ran.answer["github_issue"]["state"], "CLOSED");
    assert_eq!(
        ran.answer["done"],
        json!([
            "record_computed_verdict",
            "release_ci",
            "merge",
            "release",
            "issue-close"
        ])
    );
    assert_eq!(
        called(&fake),
        [
            "verify_claim",
            "review_status",
            "record_computed_verdict",
            "review_status",
            "release_ci",
            "merge_decision",
            "review_read",
            "review_read",
            "review_read",
            "release",
        ]
    );
    let merge = ran
        .gh
        .iter()
        .find(|line| line.starts_with("pr merge"))
        .expect("the merge ran");
    assert_eq!(
        merge,
        &format!("pr merge 7 --merge --match-head-commit {}", world.head)
    );
    let evidence = fake.evidence.borrow();
    for needed in [
        "Delivered #218 through PR #7",
        &world.merge,
        &world.head,
        "Computed verdict: approved",
        "warning: line-replaced",
        "review-risk",
        "passed (suite exit 0)",
        "claude-abcd1234",
    ] {
        assert!(
            evidence.contains(needed),
            "{needed} missing from:\n{evidence}"
        );
    }
}

#[test]
fn a_run_with_no_complete_receipt_is_handed_back_first() {
    let world = world("b.txt", "two\n");
    let mut fake = fake(&world);
    fake.receipt = None;
    let ran = drive(&fake, &[], &request());
    assert_eq!(stopped(&ran), ("receipt", "receipt-missing"));
    assert_eq!(called(&fake), ["verify_claim"]);
}

#[test]
fn a_verdict_that_is_not_approved_hands_back_the_next_and_releases_nothing() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let receipt = receipt_of(&world);
    fake.scripts.borrow_mut().insert(
        "review_status".into(),
        vec![
            status(&receipt, execute("record_computed_verdict")),
            status(&receipt, execute("publish_review")),
        ],
    );
    fake.scripts.borrow_mut().insert(
        "record_computed_verdict".into(),
        vec![Ok(json!({"ok": true, "outcome": "rejected"}))],
    );
    let ran = drive(&fake, &[], &request());
    assert_eq!(stopped(&ran), ("review", "r"));
    assert_eq!(ran.answer["stopped"]["next"]["operation"], "publish_review");
    assert!(!called(&fake).contains(&"release_ci".to_owned()));
    assert!(ran.gh.is_empty(), "{:?}", ran.gh);
}

#[test]
fn running_inputs_answer_pending_and_write_nothing() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let receipt = receipt_of(&world);
    let collect = json!({
        "kind": "collect", "reason_code": "lens-report-missing", "operation": null,
        "arguments": {},
        "inputs": [{"name": "review-risk", "capture": null,
                    "running": {"job": "j", "deadline_at": NOW + 90}}],
        "ask_after": NOW + 90,
    });
    fake.scripts
        .borrow_mut()
        .insert("review_status".into(), vec![status(&receipt, collect)]);
    let ran = drive(&fake, &[], &request());
    assert_eq!(ran.answer["delivered"], false);
    assert_eq!(ran.answer["pending"]["step"], "review");
    assert_eq!(ran.answer["pending"]["ask_after"], NOW + 90);
    assert_eq!(called(&fake), ["verify_claim", "review_status"]);
}

#[test]
fn a_collect_that_needs_a_context_is_handed_back_not_waited_on() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let receipt = receipt_of(&world);
    let collect = json!({
        "kind": "collect", "reason_code": "lens-report-missing", "operation": null,
        "arguments": {},
        "inputs": [{"name": "review-risk", "capture": "launch_review_input"}],
    });
    fake.scripts
        .borrow_mut()
        .insert("review_status".into(), vec![status(&receipt, collect)]);
    let ran = drive(&fake, &[], &request());
    assert_eq!(stopped(&ran), ("review", "lens-report-missing"));
    assert!(ran.answer["stopped"]["next"]["inputs"].is_array());
}

#[test]
fn merge_by_person_and_ask_authorisation_hand_back_before_any_release() {
    let world = world("b.txt", "two\n");
    let mut by_person = fake(&world);
    by_person.merge_by_agent = false;
    let ran = drive(&by_person, &[], &request());
    assert_eq!(stopped(&ran), ("authorisation", "merge-by-person"));

    let mut asking = fake(&world);
    asking.delivery_auto = false;
    let ran = drive(&asking, &[], &request());
    assert_eq!(
        stopped(&ran),
        ("authorisation", "delivery-authorisation-ask")
    );

    for fake in [&by_person, &asking] {
        let calls = called(fake);
        assert!(!calls.contains(&"release_ci".to_owned()), "{calls:?}");
        assert!(!calls.contains(&"release".to_owned()), "{calls:?}");
    }
}

#[test]
fn red_ci_hands_back_naming_the_checks_and_does_not_merge() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let script = [pr_view(&world, json!([check("COMPLETED", "FAILURE")]))];
    let ran = drive(&fake, &script, &request());
    assert_eq!(stopped(&ran), ("ci", "ci-red"));
    assert_eq!(ran.answer["stopped"]["checks"], json!(["ci FAILURE"]));
    assert!(!merged_by_gh(&ran));
    assert!(!called(&fake).contains(&"merge_decision".to_owned()));
}

#[test]
fn a_running_check_answers_pending_and_a_second_call_resumes_without_repeating_a_write() {
    let world = world("b.txt", "two\n");
    let first = fake(&world);
    let running = [pr_view(&world, json!([check("IN_PROGRESS", "")]))];
    let ran = drive(&first, &running, &request());
    assert_eq!(ran.answer["pending"]["step"], "ci", "{}", ran.answer);
    assert_eq!(ran.answer["pending"]["ask_after"], NOW + ASK_AFTER_SECONDS);
    assert_eq!(
        ran.answer["done"],
        json!(["record_computed_verdict", "release_ci"])
    );

    let mut second = fake(&world);
    let receipt = receipt_of(&world);
    second.scripts.borrow_mut().insert(
        "review_status".into(),
        vec![status(&receipt, halt("released"))],
    );
    second.stamp = Some("2026-10-25T09:00:00Z".into());
    let ran = drive(&second, &green_script(&world), &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    let calls = called(&second);
    assert!(
        !calls.contains(&"record_computed_verdict".to_owned()),
        "{calls:?}"
    );
    assert!(!calls.contains(&"release_ci".to_owned()), "{calls:?}");
}

#[test]
fn an_empty_rollup_is_pending_inside_the_window_and_absent_past_it() {
    let world = world("b.txt", "two\n");
    let inside = fake(&world);
    let ran = drive(&inside, &[pr_view(&world, json!([]))], &request());
    assert_eq!(ran.answer["pending"]["step"], "ci", "{}", ran.answer);
    assert_eq!(ran.answer["pending"]["reason"], "checks-not-started");

    let mut past = fake(&world);
    let receipt = receipt_of(&world);
    past.scripts.borrow_mut().insert(
        "review_status".into(),
        vec![status(&receipt, halt("released"))],
    );
    past.stamp = Some(crate::harness::session::stamp_of(NOW - 601));
    let ran = drive(&past, &[pr_view(&world, json!([]))], &request());
    assert_eq!(stopped(&ran), ("ci", "ci-absent"));
    assert!(!merged_by_gh(&ran));

    let mut edge = fake(&world);
    edge.scripts.borrow_mut().insert(
        "review_status".into(),
        vec![status(&receipt, halt("released"))],
    );
    edge.stamp = Some(crate::harness::session::stamp_of(NOW - 599));
    let ran = drive(&edge, &[pr_view(&world, json!([]))], &request());
    assert_eq!(ran.answer["pending"]["reason"], "checks-not-started");
}

#[test]
fn a_moved_head_hands_back_and_does_not_merge() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let moved = json!({
        "matches": "statusCheckRollup",
        "stdout": json!({
            "statusCheckRollup": [check("COMPLETED", "SUCCESS")],
            "headRefOid": "f".repeat(40),
            "baseRefOid": world.base,
            "baseRefName": "main",
        }).to_string(),
        "status": 0,
    });
    let ran = drive(&fake, &[moved], &request());
    assert_eq!(stopped(&ran), ("ci", "head-moved"));
    assert!(!merged_by_gh(&ran));
}

#[test]
fn an_advanced_base_passes_when_it_merges_clean_and_hands_back_on_a_conflict() {
    let clean = world("b.txt", "two\n");
    advance_base(&clean, "c.txt", "elsewhere\n");
    let fake_clean = fake(&clean);
    let ran = drive(&fake_clean, &green_script(&clean), &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);

    let conflicting = world("a.txt", "topic\n");
    advance_base(&conflicting, "a.txt", "main\n");
    let fake_conflict = fake(&conflicting);
    let ran = drive(&fake_conflict, &green_script(&conflicting), &request());
    assert_eq!(stopped(&ran), ("base", "base-advanced"));
    assert!(!merged_by_gh(&ran));
    assert!(!called(&fake_conflict).contains(&"merge_decision".to_owned()));
}

#[test]
fn a_changed_manifest_version_hands_back_before_the_merge() {
    let cargo = world(
        "Cargo.toml",
        "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
    );
    let held = fake(&cargo);
    let ran = drive(&held, &green_script(&cargo), &request());
    assert_eq!(stopped(&ran), ("version", "version-changed"));
    assert_eq!(ran.answer["stopped"]["manifests"][0]["path"], "Cargo.toml");
    assert!(!merged_by_gh(&ran));

    let npm = world(
        "package.json",
        "{\"name\": \"x\", \"version\": \"2.0.0\"}\n",
    );
    let held = fake(&npm);
    let ran = drive(&held, &green_script(&npm), &request());
    assert_eq!(stopped(&ran), ("version", "version-changed"));
}

#[test]
fn a_refusal_from_the_merge_decision_hands_back_and_runs_no_merge() {
    let world = world("b.txt", "two\n");
    let mut fake = fake(&world);
    fake.decision = Err(json!({"code": "merge-without-ci-release", "message": "no release"}));
    let ran = drive(&fake, &green_script(&world), &request());
    assert_eq!(stopped(&ran), ("merge", "merge-without-ci-release"));
    assert!(!merged_by_gh(&ran));
    assert!(!called(&fake).contains(&"release".to_owned()));
}

#[test]
fn a_merge_github_refuses_hands_back_with_the_reason() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let script = [
        pr_view(&world, json!([check("COMPLETED", "SUCCESS")])),
        json!({"matches": "pr merge", "stderr": "the base branch policy prohibits the merge", "status": 1}),
        json!({"matches": "mergeCommit", "stdout": "{\"state\":\"OPEN\",\"mergeCommit\":null}", "status": 0}),
    ];
    let ran = drive(&fake, &script, &request());
    assert_eq!(stopped(&ran), ("merge", "merge-refused"));
    assert!(
        ran.answer["stopped"]["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("policy"),
        "{}",
        ran.answer
    );
    assert!(!called(&fake).contains(&"release".to_owned()));
}

#[test]
fn a_merge_whose_answer_was_lost_reads_as_landed_and_the_close_proceeds() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let mut script = vec![
        pr_view(&world, json!([check("COMPLETED", "SUCCESS")])),
        json!({"matches": "pr merge", "stderr": "connection reset", "status": 1}),
    ];
    script.extend(merged_view(&world));
    script.extend(issue_views());
    let ran = drive(&fake, &script, &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    assert_eq!(
        ran.gh
            .iter()
            .filter(|line| line.starts_with("pr merge"))
            .count(),
        1,
        "the merge was run once and read back, never re-run"
    );
    assert!(called(&fake).contains(&"release".to_owned()));
}

#[test]
fn a_merged_pull_request_resumes_at_the_close_without_a_second_merge() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    let receipt = receipt_of(&world);
    fake.scripts.borrow_mut().insert(
        "review_status".into(),
        vec![status(&receipt, halt("delivered"))],
    );
    let mut script = merged_view(&world);
    script.extend(issue_views());
    let ran = drive(&fake, &script, &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    assert!(!merged_by_gh(&ran));
    assert!(!called(&fake).contains(&"merge_decision".to_owned()));
}

#[test]
fn a_lane_with_no_ci_needs_the_appendix_and_carries_it_into_the_evidence() {
    let world = world("b.txt", "two\n");
    let mut without = fake(&world);
    without.lane = json!({"state": "never-ran", "annotation": "no minutes left"});
    let ran = drive(&without, &green_script(&world), &request());
    assert_eq!(stopped(&ran), ("ci", "appendix-required"));
    assert!(!merged_by_gh(&ran));

    let mut with = fake(&world);
    with.lane = json!({"state": "never-ran", "annotation": "no minutes left"});
    let given = Request {
        appendix: Some("cargo test: 4112 passed".into()),
        ..request()
    };
    let ran = drive(&with, &green_script(&world), &given);
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    let evidence = with.evidence.borrow();
    assert!(
        evidence.contains("## Appendix\n\ncargo test: 4112 passed"),
        "{evidence}"
    );
    assert!(evidence.contains("no CI ran"), "{evidence}");
    assert!(evidence.contains("no minutes left"), "{evidence}");
}

#[test]
fn a_close_that_stops_part_way_is_reported_as_the_release_reports_it() {
    let world = world("b.txt", "two\n");
    let fake = fake(&world);
    fake.scripts.borrow_mut().insert(
        "release".into(),
        vec![Ok(json!({
            "ok": false,
            "close_incomplete": {"step": "evidence", "failure": {"reason": "comment-body-invalid"}},
        }))],
    );
    let ran = drive(&fake, &green_script(&world), &request());
    assert_eq!(ran.answer["ok"], false);
    assert_eq!(ran.answer["delivered"], false);
    assert_eq!(ran.answer["close_incomplete"]["step"], "evidence");
    assert_eq!(ran.answer["merged"]["sha"], world.merge);
}

#[test]
fn the_summary_follows_the_operator_s_language_and_the_body_stays_english() {
    let world = world("b.txt", "two\n");
    let mut fake = fake(&world);
    fake.language = "Español".into();
    let ran = drive(&fake, &green_script(&world), &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    let evidence = fake.evidence.borrow();
    assert!(
        evidence.starts_with("Entregado #218 mediante el PR #7"),
        "{evidence}"
    );
    assert!(
        evidence.contains("- Computed verdict: approved"),
        "{evidence}"
    );
}

#[test]
fn a_rollup_is_read_the_way_the_hand_back_rules_name_it() {
    let rollup = |checks: Value| classify(&checks);
    assert_eq!(rollup(json!([])), Rollup::Empty);
    assert_eq!(rollup(Value::Null), Rollup::Empty);
    for conclusion in ["SUCCESS", "NEUTRAL", "SKIPPED"] {
        assert!(matches!(
            rollup(json!([check("COMPLETED", conclusion)])),
            Rollup::Green(_)
        ));
    }
    for conclusion in [
        "FAILURE",
        "CANCELLED",
        "TIMED_OUT",
        "ACTION_REQUIRED",
        "STARTUP_FAILURE",
        "STALE",
    ] {
        assert!(
            matches!(
                rollup(json!([check("COMPLETED", conclusion)])),
                Rollup::Red(_)
            ),
            "{conclusion}"
        );
    }
    for status in ["QUEUED", "IN_PROGRESS", "PENDING", "WAITING"] {
        assert!(matches!(
            rollup(json!([check(status, "")])),
            Rollup::Pending(_)
        ));
    }
    let mixed = rollup(json!([
        check("IN_PROGRESS", ""),
        check("COMPLETED", "FAILURE")
    ]));
    assert!(matches!(mixed, Rollup::Red(_)), "red outranks pending");
    let contexts = rollup(json!([
        {"__typename": "StatusContext", "context": "lint", "state": "SUCCESS"},
        {"__typename": "StatusContext", "context": "deploy", "state": "PENDING"},
    ]));
    assert_eq!(contexts, Rollup::Pending(vec!["deploy".into()]));
    assert!(matches!(
        rollup(json!([{"__typename": "StatusContext", "context": "x", "state": "ERROR"}])),
        Rollup::Red(_)
    ));
}

#[test]
fn a_manifest_version_is_the_package_one() {
    let cargo = "[workspace]\nversion = \"9\"\n\n[package]\nname = \"x\"\nversion = \"1.2.3\"\n\n[dependencies]\nversion = \"7\"\n";
    assert_eq!(cargo_version(cargo).as_deref(), Some("1.2.3"));
    assert_eq!(
        cargo_version("[package]\nversion.workspace = true\n").as_deref(),
        Some("workspace")
    );
    assert_eq!(cargo_version("[package]\nname = \"x\"\n"), None);
    assert_eq!(
        package_version("{\"version\": \"2.0.0\"}").as_deref(),
        Some("2.0.0")
    );
    assert_eq!(package_version("{\"name\": \"x\"}"), None);
    assert_eq!(package_version("not json"), None);
}

#[test]
fn the_live_merge_decision_is_the_gate_s_own_and_never_allows_a_run_that_holds_nothing() {
    let root = tempfile::tempdir().expect("a gate root");
    let repo = crate::test_env::repository_with_commit();
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: repo.path().to_path_buf(),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let live = Live {
        context: &context,
        run_id: "claude-abcd1234",
        transport: transport::Context::live(
            context.skill_root.clone(),
            context.repo_dir.clone(),
            None,
        ),
    };
    let action = Action::Boundary {
        command: "gh pr merge".to_owned(),
        pr: Some(7),
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: RunsIn::Session,
        target: CommitTarget::Checkout,
    };
    let direct = crate::harness::guard::decide_action(
        &context,
        &context.repo_dir,
        &action,
        Sensitivity::Boundary,
    );
    assert!(!matches!(direct, Decision::Allow(_)), "{direct:?}");
    let refused = live
        .merge_decision(7)
        .expect_err("no claim covers the merge");
    assert!(refused["code"].is_string(), "{refused}");
}

#[test]
fn a_lane_reading_not_consulted_still_reads_the_rollup_and_hands_back_red_ci() {
    let world = world("b.txt", "two\n");
    let mut held = fake(&world);
    held.lane = json!({"state": "not-consulted"});
    let script = [pr_view(&world, json!([check("COMPLETED", "FAILURE")]))];
    let ran = drive(&held, &script, &request());
    assert_eq!(stopped(&ran), ("ci", "ci-red"));
    assert!(!merged_by_gh(&ran));

    let running = [pr_view(&world, json!([check("IN_PROGRESS", "")]))];
    let ran = drive(&held, &running, &request());
    assert_eq!(ran.answer["pending"]["step"], "ci", "{}", ran.answer);

    let ran = drive(&held, &green_script(&world), &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
}

#[test]
fn an_empty_rollup_on_a_lane_not_consulted_delivers_without_waiting() {
    let world = world("b.txt", "two\n");
    let mut held = fake(&world);
    held.lane = json!({"state": "not-consulted"});
    let mut script = green_script(&world);
    script[0] = pr_view(&world, json!([]));
    let ran = drive(&held, &script, &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);

    let ran = drive(&fake(&world), &script, &request());
    assert_eq!(
        ran.answer["pending"]["reason"], "checks-not-started",
        "{}",
        ran.answer
    );
}

#[test]
fn a_merge_whose_every_readback_fails_is_unknown_not_refused() {
    let world = world("b.txt", "two\n");
    let held = fake(&world);
    let script = [
        pr_view(&world, json!([check("COMPLETED", "SUCCESS")])),
        json!({"matches": "pr merge", "stderr": "reset", "status": 1}),
        json!({"matches": "mergeCommit", "stderr": "boom", "status": 1}),
    ];
    let ran = drive(&held, &script, &request());
    assert_eq!(stopped(&ran), ("merge", "merge-unknown"));
    assert!(ran.answer["stopped"]["landed"].is_null());
    assert!(!called(&held).contains(&"release".to_owned()));
}

#[test]
fn an_unparsable_manifest_is_not_an_unchanged_version() {
    for (file, text) in [("Cargo.toml", "[package\n"), ("package.json", "{not json")] {
        let world = world(file, text);
        let ran = drive(&fake(&world), &green_script(&world), &request());
        assert_eq!(stopped(&ran), ("version", "version-unreadable"));
    }
}

#[test]
fn a_merge_tree_that_fails_is_unreadable_not_an_advanced_base() {
    let mut world = world("b.txt", "two\n");
    advance_base(&world, "c.txt", "elsewhere\n");
    world.head = "f".repeat(40);
    let ran = drive(&fake(&world), &green_script(&world), &request());
    assert_eq!(stopped(&ran), ("base", "base-unreadable"));
}

#[test]
fn a_checks_receipt_that_cannot_be_read_is_said_so_in_the_evidence() {
    let world = world("b.txt", "two\n");
    let mut held = fake(&world);
    held.checks = Err(json!({"code": "read-failed"}));
    drive(&held, &green_script(&world), &request());
    let evidence = held.evidence.borrow();
    assert!(
        evidence.contains("Declared checks receipt: could not be read"),
        "{evidence}"
    );
}

const CARGO: &str = "[package]\nname = \"x\"\nversion = \"1.0.0\"\n\n[dependencies]\na = \"1\"\nb = \"1\"\nc = \"1\"\nd = \"1\"\n";

fn manifest_moved(base: &[(&str, &str)], topic: impl FnOnce(&Path) -> String) -> Ran {
    let world = world_over(base, topic);
    drive(&fake(&world), &green_script(&world), &request())
}

#[test]
fn a_renamed_manifest_whose_version_changed_hands_back() {
    let ran = manifest_moved(&[("old/Cargo.toml", CARGO)], |dir| {
        std::fs::create_dir_all(dir.join("new")).expect("a directory");
        git(dir, &["mv", "old/Cargo.toml", "new/Cargo.toml"]);
        let bumped = CARGO.replace("1.0.0", "1.0.1");
        std::fs::write(dir.join("new/Cargo.toml"), bumped).expect("a bump");
        git(dir, &["add", "new/Cargo.toml"]);
        git(dir, &["commit", "--quiet", "-m", "rename and bump"]);
        git(dir, &["rev-parse", "HEAD"])
    });
    assert_eq!(stopped(&ran), ("version", "version-changed"));
    assert_eq!(
        ran.answer["stopped"]["manifests"][0]["path"],
        "new/Cargo.toml"
    );
}

#[test]
fn a_renamed_manifest_with_the_same_version_passes() {
    let ran = manifest_moved(&[("old/Cargo.toml", CARGO)], |dir| {
        std::fs::create_dir_all(dir.join("new")).expect("a directory");
        git(dir, &["mv", "old/Cargo.toml", "new/Cargo.toml"]);
        git(dir, &["commit", "--quiet", "-m", "rename"]);
        git(dir, &["rev-parse", "HEAD"])
    });
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
}

#[test]
fn an_added_manifest_with_a_version_and_a_deleted_one_both_hand_back() {
    let added = manifest_moved(&[], |dir| commit(dir, "Cargo.toml", CARGO, "add"));
    assert_eq!(stopped(&added), ("version", "version-changed"));
    assert_eq!(added.answer["stopped"]["manifests"][0]["from"], Value::Null);

    let deleted = manifest_moved(&[("Cargo.toml", CARGO)], |dir| {
        git(dir, &["rm", "--quiet", "Cargo.toml"]);
        git(dir, &["commit", "--quiet", "-m", "delete"]);
        git(dir, &["rev-parse", "HEAD"])
    });
    assert_eq!(stopped(&deleted), ("version", "version-changed"));
    assert_eq!(deleted.answer["stopped"]["manifests"][0]["to"], Value::Null);
}

#[test]
fn a_merge_that_answered_and_reads_open_is_unconfirmed_and_does_not_close() {
    let world = world("b.txt", "two\n");
    let held = fake(&world);
    let script = [
        pr_view(&world, json!([check("COMPLETED", "SUCCESS")])),
        json!({"matches": "pr merge", "stdout": "", "status": 0}),
        json!({"matches": "mergeCommit", "stdout": "{\"state\":\"OPEN\",\"mergeCommit\":null}", "status": 0}),
    ];
    let ran = drive(&held, &script, &request());
    assert_eq!(stopped(&ran), ("merge", "merge-unconfirmed"));
    assert!(!called(&held).contains(&"release".to_owned()));
}

#[test]
fn a_failing_issue_close_is_delivery_incomplete() {
    let world = world("b.txt", "two\n");
    let held = fake(&world);
    let mut script = green_script(&world);
    script.retain(|entry| entry["matches"] != "issue view");
    script.push(json!({"matches": "issue view", "stdout": "{\"state\":\"OPEN\"}", "status": 0}));
    script.push(json!({"matches": "issue close", "stderr": "denied", "status": 1}));
    let ran = drive(&held, &script, &request());
    assert_eq!(ran.answer["ok"], false, "{}", ran.answer);
    assert_eq!(ran.answer["delivered"], false);
    assert_eq!(ran.answer["delivery_incomplete"]["step"], "issue-close");
    assert!(called(&held).contains(&"release".to_owned()));
}

#[test]
fn an_already_closed_issue_is_not_closed_again() {
    let world = world("b.txt", "two\n");
    let mut script = green_script(&world);
    script.retain(|entry| entry["matches"] != "issue view");
    script.push(json!({"matches": "issue view", "stdout": "{\"state\":\"CLOSED\"}", "status": 0}));
    let ran = drive(&fake(&world), &script, &request());
    assert_eq!(ran.answer["delivered"], true, "{}", ran.answer);
    assert!(
        !ran.gh.iter().any(|line| line.starts_with("issue close")),
        "{:?}",
        ran.gh
    );
}

#[test]
fn an_unreadable_appendix_file_is_handed_back_before_anything_runs() {
    let root = tempfile::tempdir().expect("a root");
    let context = GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root: root.path().join("skill"),
        repo_dir: root.path().to_path_buf(),
        state_root: root.path().join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    };
    let named = NamedRun {
        id: "claude-abcd1234",
        runtime: "claude",
    };
    let missing = root.path().join("absent.md").display().to_string();
    let arguments = json!({"issue": 218, "run_id": "claude-abcd1234", "appendix_file": missing});
    let result = deliver(&arguments, &context, Some(&named)).expect("an answer");
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let answer: Value = serde_json::from_str(text).expect("JSON");
    assert_eq!(answer["stopped"]["step"], "appendix", "{answer}");
    assert_eq!(answer["stopped"]["reason"], "appendix-unreadable");
}

fn live_for<'a>(context: &'a GateContext, run_id: &'a str) -> Live<'a> {
    Live {
        context,
        run_id,
        transport: transport::Context::live(
            context.skill_root.clone(),
            context.repo_dir.clone(),
            None,
        ),
    }
}

fn shared_checkout_context(root: &Path, repo: &Path) -> GateContext {
    let skill_root = root.join("skill");
    std::fs::create_dir_all(&skill_root).expect("a skill root");
    std::fs::write(skill_root.join(crate::skill::CONTRACT), "the contract\n")
        .expect("the contract is installed");
    GateContext {
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        stand_down: None,
        skill_root,
        repo_dir: repo.to_path_buf(),
        state_root: root.join("state"),
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        decision_mode: crate::config::DecisionMode::Ask,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: crate::harness::RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn holder_receipt() -> crate::transport::claim::ReviewTransaction {
    crate::transport::claim::ReviewTransaction {
        epoch: "1".repeat(32),
        pr: 7,
        head: "a".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    }
}

fn store_holder(
    context: &GateContext,
    repo: &Path,
    run_id: &str,
    issue: u64,
    receipt: Option<crate::transport::claim::ReviewTransaction>,
) {
    let mut run = session::Run::new(run_id.to_owned());
    run.issue = Some(crate::identity::Identity::Number(issue));
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    run.review_transaction = receipt;
    session::store(&context.state_root, &run).expect("the pointer writes");
}

fn pr_head_answer(head: &str) -> Value {
    json!({
        "matches": "pr view 7",
        "stdout": json!({"headRefOid": head}).to_string(),
        "status": 0,
    })
}

#[test]
fn the_live_merge_decision_names_its_caller_when_another_live_run_holds_the_same_checkout() {
    let root = tempfile::tempdir().expect("a gate root");
    let repo = crate::test_env::repository_with_commit();
    let context = shared_checkout_context(root.path(), repo.path());
    store_holder(
        &context,
        repo.path(),
        "claude-caller000",
        12,
        Some(holder_receipt()),
    );
    store_holder(&context, repo.path(), "claude-bystander", 34, None);
    let bystander_before = session::load(&context.state_root, "claude-bystander");

    let bin = scripted_gh();
    let script = answers(&[
        crate::test_env::live_holder_issue("claude-caller000", 12, "in-progress"),
        crate::test_env::live_holder_issue("claude-bystander", 34, "in-progress"),
        pr_head_answer(&holder_receipt().head),
    ]);
    let refused = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        live_for(&context, "claude-caller000")
            .merge_decision(7)
            .expect_err("no CI release is recorded for this receipt")
    });

    assert_ne!(
        refused["code"], "several-runs-hold-this-checkout",
        "{refused}"
    );
    assert!(
        ["merge-without-ci-release", "read-failed"]
            .contains(&refused["code"].as_str().unwrap_or_default()),
        "{refused}"
    );
    let bystander_after = session::load(&context.state_root, "claude-bystander");
    assert_eq!(bystander_after.issue, bystander_before.issue);
    assert_eq!(bystander_after.state, bystander_before.state);
    assert!(bystander_after.review_transaction.is_none());
}

#[test]
fn the_live_merge_decision_refuses_a_caller_that_is_unclaimed_foreign_or_unreadable() {
    let root = tempfile::tempdir().expect("a gate root");
    let repo = crate::test_env::repository_with_commit();
    let context = shared_checkout_context(root.path(), repo.path());
    store_holder(
        &context,
        repo.path(),
        "claude-caller000",
        12,
        Some(holder_receipt()),
    );
    store_holder(&context, repo.path(), "claude-bystander", 34, None);

    let unclaimed = live_for(&context, "claude-nothing000")
        .merge_decision(7)
        .expect_err("a run that holds nothing is refused");
    assert_eq!(unclaimed["code"], "merge-not-covered", "{unclaimed}");

    let bin = scripted_gh();
    let script = answers(&[crate::test_env::live_holder_issue(
        "claude-caller000",
        12,
        "in-progress",
    )]);
    let foreign = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        live_for(&context, "claude-caller000")
            .merge_decision(8)
            .expect_err("a foreign PR is refused")
    });
    assert_eq!(foreign["code"], "delivery-pr-mismatch", "{foreign}");

    let stale = crate::test_env::with_scripted_gh(
        bin.path(),
        &answers(&[
            crate::test_env::live_holder_issue("claude-caller000", 12, "in-progress"),
            pr_head_answer(&"f".repeat(40)),
        ]),
        || {
            live_for(&context, "claude-caller000")
                .merge_decision(7)
                .expect_err("a moved head is refused")
        },
    );
    assert_eq!(stale["code"], "verdict-bound-to-other-bytes", "{stale}");

    std::fs::write(
        session::pointer_path(&context.state_root, "claude-garbled000"),
        "{not json",
    )
    .expect("a garbled pointer");
    let unreadable = live_for(&context, "claude-garbled000")
        .merge_decision(7)
        .expect_err("an unreadable pointer is refused");
    assert_eq!(unreadable["code"], "run-pointer-unreadable", "{unreadable}");
}
