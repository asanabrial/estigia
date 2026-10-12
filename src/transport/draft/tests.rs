use super::*;

const PR_URL: &str = "https://github.com/example/repo/pull/7";

struct Rig {
    root: tempfile::TempDir,
    context: Context,
    head: String,
}

fn git(at: &std::path::Path, arguments: &[&str]) -> String {
    run(
        &["git"]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>(),
        Some(at),
        How::write(),
    )
    .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()))
    .stdout
    .trim()
    .to_owned()
}

fn rig() -> Rig {
    let root = tempfile::tempdir().expect("a temporary root");
    let origin = root.path().join("origin.git");
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
    git(&repo, &["checkout", "--quiet", "-b", "issue-12"]);
    git(&repo, &["commit", "--allow-empty", "--quiet", "-m", "work"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: repo,
        config: Vec::new(),
        repo: None,
    };
    Rig {
        root,
        context,
        head,
    }
}

impl Rig {
    fn repo(&self) -> &std::path::Path {
        &self.context.repo_dir
    }

    fn body(&self, text: &str) -> std::path::PathBuf {
        let path = self.root.path().join("body.md");
        std::fs::write(&path, text).expect("the body file");
        path
    }

    fn on_origin(&self) -> bool {
        !git(self.repo(), &["ls-remote", "--heads", "origin", "issue-12"]).is_empty()
    }

    fn call(&self, title: &str, body_file: &std::path::Path) -> Result<serde_json::Value, Failure> {
        draft_with_sleep(
            &self.context,
            &Draft {
                branch: "issue-12",
                worktree: self.repo(),
                title,
                body_file,
            },
            &mut |_| {},
        )
    }
}

fn view(head: &str, draft: bool) -> serde_json::Value {
    serde_json::json!({
        "number": 7,
        "url": PR_URL,
        "headRefOid": head,
        "headRefName": "issue-12",
        "state": "OPEN",
        "isDraft": draft,
    })
}

fn script(existing: &serde_json::Value, viewed: &serde_json::Value) -> String {
    crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"defaultBranchRef":{"name":"main"}}"#,
        }),
        serde_json::json!({ "matches": "pr list", "stdout": existing.to_string() }),
        serde_json::json!({ "matches": "pr create", "stdout": PR_URL }),
        serde_json::json!({ "matches": "pr view", "stdout": viewed.to_string() }),
    ])
}

fn reason(failure: &Failure) -> String {
    failure
        .envelope()
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn logged(rig: &Rig, log: &std::path::Path) -> Vec<String> {
    let mine = rig
        .root
        .path()
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_owned();
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (call, at) = line.split_once('\t')?;
            at.contains(&mine).then(|| call.to_owned())
        })
        .collect()
}

#[test]
fn a_draft_is_created_and_read_back_and_the_tracker_is_never_written() {
    let rig = rig();
    let body = rig.body("What changed.\n");
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let answers = script(&serde_json::json!([]), &view(&rig.head, true));
    let answer =
        crate::test_env::with_logging_gh(bin.path(), &answers, &log, || rig.call("A title", &body))
            .expect("the draft is published");
    assert_eq!(answer["pr"], 7);
    assert_eq!(answer["url"], PR_URL);
    assert_eq!(answer["draft"], true);
    assert_eq!(answer["created"], true);
    assert_eq!(answer["head"], rig.head.as_str());
    assert!(rig.on_origin(), "the branch was not pushed");

    let calls = logged(&rig, &log);
    let created: Vec<&String> = calls
        .iter()
        .filter(|call| call.starts_with("pr create"))
        .collect();
    assert_eq!(created.len(), 1, "{calls:?}");
    assert!(created[0].contains("--draft"), "{created:?}");
    assert!(
        calls
            .iter()
            .all(|call| call.starts_with("pr ") || call.starts_with("repo view")),
        "a call other than a pull request or the default branch was made: {calls:?}"
    );
}

#[test]
fn the_body_is_written_as_given() {
    let rig = rig();
    let text = "Plain words, no receipt.\n\n- one\n- two";
    let body = rig.body(text);
    let bin = crate::test_env::scripted_gh();
    let sent = rig.root.path().join("sent.log");
    let answers = script(&serde_json::json!([]), &view(&rig.head, true));
    crate::test_env::with_body_log(bin.path(), &answers, &sent, || rig.call("A title", &body))
        .expect("the draft is published");
    let seen = std::fs::read_to_string(&sent).expect("the staged body was copied");
    let (_, staged) = seen.split_once('\n').expect("a header line and a body");
    assert_eq!(staged.trim_end_matches('\n'), text);
}

#[test]
fn a_closing_keyword_in_the_body_or_the_title_is_refused_before_anything_is_pushed() {
    for (title, text) in [("A title", "Closes #12"), ("Fixes #12 properly", "Words.")] {
        let rig = rig();
        let body = rig.body(text);
        let bin = crate::test_env::scripted_gh();
        let log = rig.root.path().join("gh.log");
        let answers = script(&serde_json::json!([]), &view(&rig.head, true));
        let failure =
            crate::test_env::with_logging_gh(bin.path(), &answers, &log, || rig.call(title, &body))
                .expect_err("a closing keyword was published");
        assert_eq!(reason(&failure), "closing-keyword-live");
        assert!(!rig.on_origin(), "the branch was pushed anyway");
        assert!(
            !logged(&rig, &log)
                .iter()
                .any(|call| call.starts_with("pr create")),
            "a pull request was created anyway"
        );
    }
}

#[test]
fn an_existing_open_draft_is_reported_and_no_second_one_is_made() {
    let rig = rig();
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let existing = serde_json::json!([view(&rig.head, true)]);
    let answers = script(&existing, &view(&rig.head, true));
    let answer =
        crate::test_env::with_logging_gh(bin.path(), &answers, &log, || rig.call("A title", &body))
            .expect("the retry reads the draft back");
    assert_eq!(answer["pr"], 7);
    assert_eq!(answer["created"], false);
    assert!(rig.on_origin(), "the branch was not pushed on the retry");
    assert!(
        !logged(&rig, &log)
            .iter()
            .any(|call| call.starts_with("pr create")),
        "a second pull request was created"
    );
}

#[test]
fn an_existing_open_pull_request_that_is_not_a_draft_is_refused_and_left_alone() {
    let rig = rig();
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let existing = serde_json::json!([view(&rig.head, false)]);
    let answers = script(&existing, &view(&rig.head, false));
    let failure =
        crate::test_env::with_logging_gh(bin.path(), &answers, &log, || rig.call("A title", &body))
            .expect_err("a ready pull request was taken over");
    assert_eq!(reason(&failure), "draft-pr-exists-ready");
    assert!(!rig.on_origin(), "the branch was pushed over a ready PR");
    assert!(
        !logged(&rig, &log)
            .iter()
            .any(|call| call.starts_with("pr create") || call.starts_with("pr ready")),
        "the ready pull request was written to"
    );
}

#[test]
fn a_read_back_that_never_agrees_says_the_push_landed_and_reports_no_pull_request() {
    let rig = rig();
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let elsewhere = "a".repeat(40);
    let answers = script(&serde_json::json!([]), &view(&elsewhere, true));
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &answers, || rig.call("A title", &body))
            .expect_err("an unconfirmed pull request was reported");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "draft-pr-readback-failed");
    assert_eq!(envelope["world"], "committed");
    let action = envelope["action"].as_str().expect("an action");
    assert!(
        action.contains("push landed") && action.contains("may exist"),
        "{action}"
    );
    assert!(envelope.get("url").is_none(), "{envelope}");
    assert!(rig.on_origin());
}

#[test]
fn a_read_back_that_errors_after_the_push_is_committed_and_says_the_push_landed() {
    let rig = rig();
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let answers = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "repo view",
            "stdout": r#"{"defaultBranchRef":{"name":"main"}}"#,
        }),
        serde_json::json!({ "matches": "pr list", "stdout": "[]" }),
        serde_json::json!({ "matches": "pr create", "stdout": PR_URL }),
        serde_json::json!({ "matches": "pr view", "stdout": "", "status": 1 }),
    ]);
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &answers, || rig.call("A title", &body))
            .expect_err("an erroring read-back was reported as nothing written");
    let envelope = failure.envelope();
    assert_eq!(envelope["world"], "committed", "{envelope}");
    let action = envelope["action"].as_str().expect("an action");
    assert!(action.contains("push landed"), "{action}");
    assert!(rig.on_origin());
}

#[test]
fn a_reused_draft_says_its_title_and_body_were_not_replaced() {
    let rig = rig();
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let existing = serde_json::json!([view(&rig.head, true)]);
    let answers = script(&existing, &view(&rig.head, true));
    let answer =
        crate::test_env::with_scripted_gh(bin.path(), &answers, || rig.call("A title", &body))
            .expect("the retry reads the draft back");
    assert_eq!(answer["title_and_body_applied"], false);
}

#[test]
fn a_checkout_that_is_not_on_the_recorded_branch_is_refused_before_anything_is_pushed() {
    let rig = rig();
    git(rig.repo(), &["checkout", "--quiet", "main"]);
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let answers = script(&serde_json::json!([]), &view(&rig.head, true));
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &answers, || rig.call("A title", &body))
            .expect_err("a draft was published from another branch");
    assert_eq!(reason(&failure), "draft-pr-branch-not-checked-out");
    assert!(!rig.on_origin());
}

#[test]
fn a_closing_keyword_in_a_commit_message_is_refused_before_anything_is_pushed() {
    let rig = rig();
    git(
        rig.repo(),
        &["commit", "--allow-empty", "--quiet", "-m", "Resolves #12"],
    );
    let body = rig.body("Words.");
    let bin = crate::test_env::scripted_gh();
    let answers = script(&serde_json::json!([]), &view(&rig.head, true));
    let failure =
        crate::test_env::with_scripted_gh(bin.path(), &answers, || rig.call("A title", &body))
            .expect_err("a commit that closes the issue was published");
    assert_eq!(reason(&failure), "closing-keyword-live");
    assert!(!rig.on_origin());
}

#[test]
fn two_drafts_of_one_branch_stage_to_distinct_files_and_one_cleanup_leaves_the_other() {
    let first = staged_name("issue-1/race");
    let second = staged_name("issue-1/race");
    assert_ne!(first, second);
    let first = stage("first", &first).expect("the first body is staged");
    let second = stage("second", &second).expect("the second body is staged");
    std::fs::remove_file(&first).expect("the first draft cleans up");
    assert_eq!(
        std::fs::read_to_string(&second).expect("the second body survives"),
        "second"
    );
    std::fs::remove_file(&second).expect("the second draft cleans up");
}
