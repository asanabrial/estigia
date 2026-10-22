use std::path::PathBuf;

use crate::transport::store::{OutboxKind, Repo, Store, store_path};
use crate::transport::{Context, Failure, Source, dispatch::dispatch, outbox};

const NOW: &str = "2026-01-01T20:00:00Z";

const BODY: &str = "## Description for dumb humans\n\n> [!NOTE]\n> A probe.\n\n\
    ## Problem\nA probe.\n\n\
    ## Why it matters\nA probe.\n\n\
    ## Proposed approach\nA probe.\n\n\
    ## Where to start\nA probe.\n\n\
    ## Acceptance criteria\nA probe.\n\n\
    ## How to verify\nA probe.\n\n\
    ## Out of scope\nA probe.\n\n\
    ## Evidence\nA probe.\n";

fn night() -> Option<u16> {
    Some(20 * 60)
}

fn noon() -> Option<u16> {
    Some(12 * 60)
}

pub(super) struct Rig {
    directory: tempfile::TempDir,
    log: PathBuf,
    context: Context,
    script: String,
    labels: Option<PathBuf>,
}

pub(super) fn rig() -> Rig {
    let directory = tempfile::tempdir().expect("a directory");
    let log = directory.path().join("calls.log");
    crate::paths::replace_atomically(&directory.path().join("body.md"), BODY).expect("a body file");
    crate::paths::replace_atomically(&directory.path().join("note.md"), "A note.\n")
        .expect("a note file");
    git(directory.path(), &["init", "--quiet", "-b", "main"]);
    git(
        directory.path(),
        &["remote", "add", "origin", "git@github.com:fixture/repo.git"],
    );
    let context = Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: vec![(
            "Git operation window".to_owned(),
            "enabled 09:00-17:00".to_owned(),
            Source::Written,
        )],
        repo: Some("fixture/repo".to_owned()),
    };
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(
            12,
            serde_json::json!({
                "state": "OPEN",
                "labels": [{ "name": "status:in-progress" }],
                "comments": [],
            }),
        ),
        serde_json::json!({ "matches": "issues?state=open", "stdout": "[[]]", "status": 0 }),
    ]);
    Rig {
        directory,
        log,
        context,
        script,
        labels: None,
    }
}

/// A rig whose fake `gh` keeps one issue's labels across calls, so an edit can
/// be read back. The handoff projection is exactly that read-write-read.
pub(super) fn labelled_rig() -> Rig {
    let mut rig = rig();
    let labels = rig.directory.path().join("labels.json");
    crate::paths::replace_atomically(&labels, "[\"status:in-progress\"]").expect("the labels");
    rig.script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "view 12 --json labels",
            "labels": true,
            "status": 0,
        }),
        crate::test_env::issue_view_answer(
            12,
            serde_json::json!({
                "state": "OPEN",
                "labels": [{ "name": "status:in-progress" }],
                "comments": [],
            }),
        ),
    ]);
    rig.labels = Some(labels);
    rig
}

pub(super) fn repo() -> Repo {
    Repo {
        host: "github.com".to_owned(),
        owner: "fixture".to_owned(),
        name: "repo".to_owned(),
    }
}

/// The window rows a checkout carries in its own `estigia.local.md`.
pub(super) fn rows(window: &str) -> String {
    format!("| Setting | Value |\n|---|---|\n| Git operation window | {window} |\n")
}

/// A checkout of `name` on this machine, carrying `window` as its own row and
/// recorded among the checkouts that answer for themselves under `home`.
///
/// Its `origin` names the repository on GitHub, which is what lets the window
/// resolve a write aimed at `name` to this checkout's rows.
pub(super) fn destination(home: &std::path::Path, name: &str, window: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a destination checkout");
    git(directory.path(), &["init", "--quiet", "-b", "main"]);
    git(
        directory.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("git@github.com:{name}.git"),
        ],
    );
    let file = crate::skill::repository_config_path(directory.path());
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("the rows directory");
    crate::paths::replace_atomically(&file, &rows(window)).expect("the rows");
    crate::skill::remember_repository(home, directory.path());
    directory
}

/// Runs `body` with `home` as this machine's home, so a repository recorded
/// under it is one the window can find.
pub(super) fn with_home<T>(home: &std::path::Path, body: impl FnOnce() -> T) -> T {
    crate::test_env::with_home_env(home, home, home, body)
}

impl Rig {
    pub(super) fn path(&self, name: &str) -> String {
        self.directory.path().join(name).display().to_string()
    }

    fn call_at(
        &self,
        clock: fn() -> Option<u16>,
        operation: &str,
        flags: &[&str],
    ) -> Result<serde_json::Value, Failure> {
        self.call_when(clock, NOW, operation, flags)
    }

    fn call_when(
        &self,
        clock: fn() -> Option<u16>,
        now: &str,
        operation: &str,
        flags: &[&str],
    ) -> Result<serde_json::Value, Failure> {
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        let bin = crate::test_env::scripted_gh();
        let call = || {
            crate::harness::operation_window::with_now_override(Some(clock), || {
                dispatch(&self.context, operation, &flags, now)
            })
        };
        match &self.labels {
            Some(labels) => crate::test_env::with_labelled_logging_gh(
                bin.path(),
                &self.script,
                labels,
                &self.log,
                call,
            ),
            None => crate::test_env::with_logging_gh(bin.path(), &self.script, &self.log, call),
        }
    }

    pub(super) fn closed(
        &self,
        operation: &str,
        flags: &[&str],
    ) -> Result<serde_json::Value, Failure> {
        self.call_at(night, operation, flags)
    }

    pub(super) fn open(
        &self,
        operation: &str,
        flags: &[&str],
    ) -> Result<serde_json::Value, Failure> {
        self.call_at(noon, operation, flags)
    }

    pub(super) fn writes(&self) -> Vec<String> {
        crate::test_env::logged_writes(&self.log)
    }

    pub(super) fn queue(&self) -> Vec<(OutboxKind, u64)> {
        self.queue_for(&repo())
    }

    pub(super) fn queue_for(&self, repo: &Repo) -> Vec<(OutboxKind, u64)> {
        let store = Store::open(&store_path().expect("a path")).expect("opens");
        store
            .outbox(repo)
            .expect("reads")
            .iter()
            .map(|row| (row.kind, row.issue))
            .collect()
    }

    pub(super) fn state(&self, issue: u64) -> Option<String> {
        let store = Store::open(&store_path().expect("a path")).expect("opens");
        store.issue_state(&repo(), issue).expect("reads")
    }
}

fn created(rig: &Rig) -> u64 {
    let answer = rig
        .closed(
            "create",
            &[
                "--identity",
                "a-probe",
                "--title",
                "A probe",
                "--body-file",
                &rig.path("body.md"),
                "--domain",
                "transport",
                "--priority",
                "p2",
            ],
        )
        .expect("a create while closed");
    answer["issue"].as_u64().expect("an issue number")
}

#[test]
fn a_transition_while_closed_moves_the_store_and_queues_the_projection() {
    let rig = rig();
    let answer = rig
        .closed("transition", &["--issue", "12", "--to", "review"])
        .expect("a transition while closed");

    assert_eq!(answer["projection"], "deferred", "{answer}");
    assert!(answer["queued"].as_u64().is_some(), "{answer}");
    assert_eq!(rig.state(12).as_deref(), Some("review"));
    assert_eq!(rig.queue(), vec![(OutboxKind::Reconverge, 12)]);
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn two_transitions_while_closed_leave_one_queued_reconverge() {
    let rig = rig();
    rig.closed("transition", &["--issue", "12", "--to", "review"])
        .expect("the first");
    let last = rig
        .closed("transition", &["--issue", "12", "--to", "done"])
        .expect("the second");

    assert_eq!(rig.state(12).as_deref(), Some("done"));
    let live: Vec<_> = rig
        .queue()
        .into_iter()
        .filter(|(kind, _)| *kind == OutboxKind::Reconverge)
        .collect();
    assert_eq!(live, vec![(OutboxKind::Reconverge, 12)]);
    assert!(last["queued"].as_u64().is_some());
}

#[test]
fn a_transition_while_open_still_edits_the_labels() {
    let rig = rig();
    let _ = rig.open("transition", &["--issue", "12", "--to", "review"]);

    assert!(
        rig.writes().iter().any(|line| line.contains("issue edit")),
        "an open window did not project: {:?}",
        rig.writes()
    );
    assert_eq!(rig.queue(), Vec::new());
}

#[test]
fn a_comment_while_closed_is_queued_with_its_escaped_body() {
    let rig = rig();
    let answer = rig
        .closed(
            "comment",
            &["--issue", "12", "--body-file", &rig.path("note.md")],
        )
        .expect("a comment while closed");

    assert!(answer["queued"].as_u64().is_some(), "{answer}");
    assert_eq!(answer["kind"], "note");
    assert_eq!(rig.queue(), vec![(OutboxKind::Comment, 12)]);
    let store = Store::open(&store_path().expect("a path")).expect("opens");
    let row = store.next_outbox(&repo()).expect("reads").expect("a row");
    assert_eq!(row.payload["body"], "A note.\n");
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_create_while_closed_is_held_under_a_provisional_number() {
    let rig = rig();
    let answer = rig
        .closed(
            "create",
            &[
                "--identity",
                "a-probe",
                "--title",
                "A probe",
                "--body-file",
                &rig.path("body.md"),
                "--domain",
                "transport",
                "--priority",
                "p2",
            ],
        )
        .expect("a create while closed");

    let provisional = answer["issue"].as_u64().expect("a number");
    assert!(outbox::is_provisional(provisional), "{answer}");
    assert_eq!(answer["provisional"], true);
    assert_eq!(answer["url"], serde_json::Value::Null);
    assert_eq!(rig.state(provisional).as_deref(), Some("ready"));
    assert_eq!(
        rig.queue(),
        vec![
            (OutboxKind::Create, provisional),
            (OutboxKind::Reconverge, provisional)
        ]
    );
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_queued_create_remembers_what_it_will_file() {
    let rig = rig();
    created(&rig);
    let store = Store::open(&store_path().expect("a path")).expect("opens");
    let row = store.next_outbox(&repo()).expect("reads").expect("a row");

    assert_eq!(row.kind, OutboxKind::Create);
    assert_eq!(row.payload["title"], "A probe");
    assert_eq!(row.payload["body"], BODY);
    assert_eq!(row.payload["state"], "ready");
    let labels = row.payload["labels"].as_array().expect("labels");
    assert!(labels.iter().any(|label| label == "status:ready"));
    assert!(labels.iter().any(|label| label == "domain:transport"));
    assert!(labels.iter().any(|label| label == "p2"));
}

#[test]
fn a_comment_on_a_provisional_issue_is_queued_under_its_number() {
    let rig = rig();
    let provisional = created(&rig);
    rig.closed(
        "comment",
        &[
            "--issue",
            &provisional.to_string(),
            "--body-file",
            &rig.path("note.md"),
        ],
    )
    .expect("a comment on a provisional issue");

    assert!(rig.queue().contains(&(OutboxKind::Comment, provisional)));
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_transition_of_a_provisional_issue_moves_only_the_store() {
    let rig = rig();
    let provisional = created(&rig);
    let answer = rig
        .closed(
            "transition",
            &["--issue", &provisional.to_string(), "--to", "in-progress"],
        )
        .expect("a transition of a provisional issue");

    assert_eq!(answer["projection"], "deferred");
    assert_eq!(rig.state(provisional).as_deref(), Some("in-progress"));
    assert_eq!(rig.writes(), Vec::<String>::new());
}

const FIRST: &str = "0123456789abcdef0123456789abcdef";
const SECOND: &str = "fedcba9876543210fedcba9876543210";
const HORIZON: &str = "2099-01-01T00:00Z";

fn claim_flags<'a>(issue: &'a str, run: &'a str, operation: &'a str) -> Vec<&'a str> {
    vec![
        "--issue",
        issue,
        "--run-id",
        run,
        "--runtime",
        "claude",
        "--horizon",
        HORIZON,
        "--operation-id",
        operation,
    ]
}

#[test]
fn a_claim_while_closed_is_recorded_and_its_projection_queued() {
    let rig = rig();
    let answer = rig
        .closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim while closed");

    assert_eq!(answer["projection"], "deferred", "{answer}");
    assert!(answer["queued"].as_u64().is_some(), "{answer}");
    assert_eq!(rig.queue(), vec![(OutboxKind::Reconverge, 12)]);
    assert_eq!(rig.writes(), Vec::<String>::new());
    rig.closed(
        "verify-claim",
        &[
            "--issue",
            "12",
            "--run-id",
            "claude-a",
            "--expect-state",
            "in-progress",
        ],
    )
    .expect("the claim holds");
}

#[test]
fn a_release_while_closed_queues_its_projection_and_its_evidence() {
    let rig = rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim");
    let mut discover = claim_flags("12", "claude-a", SECOND);
    let found = rig
        .closed("unassign", &discover)
        .expect("a discovery while closed");
    let target = found["target_operation"].as_str().expect("an epoch");
    discover.extend(["--target-operation", target]);
    let note = rig.path("note.md");
    discover.extend(["--evidence-file", &note]);
    let released = rig
        .closed("unassign", &discover)
        .expect("a release while closed");

    assert_eq!(released["projection"], "deferred", "{released}");
    let kinds: Vec<_> = rig.queue().into_iter().map(|(kind, _)| kind).collect();
    assert!(kinds.contains(&OutboxKind::Comment), "{kinds:?}");
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_takeover_while_closed_queues_its_projection() {
    let rig = rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim");
    let mut takeover = claim_flags("12", "claude-b", SECOND);
    let note = rig.path("note.md");
    takeover.extend(["--takeover", "--force", "--reason-file", &note]);
    let found = rig.closed("claim", &takeover).expect("a discovery");
    let target = found["target_operation"].as_str().expect("an epoch");
    takeover.extend(["--target-operation", target]);
    let taken = rig.closed("claim", &takeover).expect("a takeover");

    assert_eq!(taken["projection"], "deferred", "{taken}");
    assert_eq!(rig.queue(), vec![(OutboxKind::Reconverge, 12)]);
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn an_issue_created_while_closed_can_be_worked_from_claim_to_release() {
    let rig = rig();
    let provisional = created(&rig).to_string();
    let issue = provisional.as_str();

    rig.closed("claim", &claim_flags(issue, "claude-a", FIRST))
        .expect("a claim of a provisional issue");
    rig.closed("transition", &["--issue", issue, "--to", "in-progress"])
        .expect("a transition");
    rig.closed(
        "task",
        &[
            "--issue",
            issue,
            "--run-id",
            "claude-a",
            "--id",
            "T1",
            "--text",
            "The first task.",
            "--route",
            "inline",
        ],
    )
    .expect("a task");
    rig.closed(
        "heartbeat",
        &[
            "--issue",
            issue,
            "--run-id",
            "claude-a",
            "--expect-state",
            "in-progress",
            "--body-file",
            &rig.path("note.md"),
        ],
    )
    .expect("a heartbeat");
    rig.closed(
        "comment",
        &["--issue", issue, "--body-file", &rig.path("note.md")],
    )
    .expect("a comment");
    rig.closed(
        "verify-claim",
        &[
            "--issue",
            issue,
            "--run-id",
            "claude-a",
            "--expect-state",
            "in-progress",
        ],
    )
    .expect("the claim holds");
    let found = rig
        .closed("unassign", &claim_flags(issue, "claude-a", SECOND))
        .expect("a discovery");
    let mut release = claim_flags(issue, "claude-a", SECOND);
    release.extend([
        "--target-operation",
        found["target_operation"].as_str().expect("an epoch"),
    ]);
    rig.closed("unassign", &release).expect("a release");

    assert_eq!(rig.writes(), Vec::<String>::new());
    let asked = std::fs::read_to_string(&rig.log).unwrap_or_default();
    assert!(
        !asked.contains(issue),
        "GitHub was asked about an issue it does not have: {asked}"
    );
}

fn git(at: &std::path::Path, arguments: &[&str]) {
    crate::transport::run(
        &["git"]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>(),
        Some(at),
        crate::transport::How::write(),
    )
    .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()));
}

fn repository_rig() -> Rig {
    let mut rig = rig();
    let origin = rig.directory.path().join("origin.git");
    let repo = rig.directory.path().join("repo");
    std::fs::create_dir_all(&origin).expect("a remote directory");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    git(&origin, &["init", "--quiet", "--bare", "-b", "main"]);
    git(&repo, &["init", "--quiet", "-b", "main"]);
    git(&repo, &["config", "user.email", "estigia@example.invalid"]);
    git(&repo, &["config", "user.name", "Estigia test"]);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:fixture/repo.git"],
    );
    git(
        &repo,
        &[
            "config",
            &format!("url.{}.insteadOf", origin.display()),
            "git@github.com:fixture/repo.git",
        ],
    );
    crate::paths::replace_atomically(&repo.join("one.txt"), "one\n").expect("content");
    git(&repo, &["add", "one.txt"]);
    git(&repo, &["commit", "--quiet", "-m", "one"]);
    git(&repo, &["push", "--quiet", "origin", "main"]);
    rig.context.repo_dir = repo;
    rig.script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-a",
        12,
        "in-progress",
    )]);
    rig
}

fn start_flags<'a>(issue: &'a str, branch: &'a str, root: &'a str) -> Vec<&'a str> {
    vec![
        "--issue",
        issue,
        "--run-id",
        "claude-a",
        "--branch",
        branch,
        "--base",
        "main",
        "--repo-name",
        "repo",
        "--worktree-root",
        root,
    ]
}

fn remote_has(rig: &Rig, branch: &str) -> bool {
    let answer = crate::transport::run(
        &[
            "git",
            "ls-remote",
            "--heads",
            "origin",
            &format!("refs/heads/{branch}"),
        ],
        Some(&rig.context.repo_dir),
        crate::transport::How::read(),
    )
    .expect("ls-remote answers");
    !answer.stdout.trim().is_empty()
}

#[test]
fn a_branch_started_while_closed_is_made_locally_and_its_link_queued() {
    let rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("a start while closed");

    assert_eq!(answer["natively_linked"], false, "{answer}");
    assert_eq!(answer["link_outcome"], "deferred", "{answer}");
    assert!(answer["link_queued"].as_u64().is_some(), "{answer}");
    assert!(
        std::path::Path::new(answer["worktree"].as_str().expect("a path")).is_dir(),
        "{answer}"
    );
    assert_eq!(rig.queue(), vec![(OutboxKind::Link, 12)]);
    assert!(!remote_has(&rig, "fix-12"), "the branch reached the remote");
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn starting_the_same_branch_twice_while_closed_queues_one_link() {
    let rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    rig.closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("the first start");
    let again = rig
        .closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("the second start");

    assert_eq!(again["resumed_existing_worktree"], true, "{again}");
    assert_eq!(rig.queue(), vec![(OutboxKind::Link, 12)]);
}

#[test]
fn a_branch_that_exists_locally_and_remotely_is_not_linked_while_closed() {
    let rig = repository_rig();
    let repo = rig.context.repo_dir.clone();
    git(&repo, &["branch", "fix-12", "HEAD"]);
    git(&repo, &["push", "--quiet", "origin", "--", "fix-12"]);
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("a start of a branch that is on both sides");

    assert_eq!(answer["link_outcome"], "deferred", "{answer}");
    assert_eq!(rig.queue(), vec![(OutboxKind::Link, 12)]);
    assert_eq!(rig.writes(), Vec::<String>::new());
    let asked = std::fs::read_to_string(&rig.log).unwrap_or_default();
    assert!(!asked.contains("develop"), "{asked}");
}

#[test]
fn a_provisional_issue_gets_a_branch_while_closed() {
    let mut rig = repository_rig();
    let provisional = created(&rig).to_string();
    rig.script = crate::test_env::answers(&[]);
    rig.closed("claim", &claim_flags(&provisional, "claude-a", FIRST))
        .expect("a claim");
    rig.closed(
        "transition",
        &["--issue", &provisional, "--to", "in-progress"],
    )
    .expect("a transition");
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .closed("start-branch", &start_flags(&provisional, "feat-x", &root))
        .expect("a start for a provisional issue");

    assert_eq!(answer["link_outcome"], "deferred", "{answer}");
    assert!(
        rig.queue()
            .contains(&(OutboxKind::Link, provisional.parse().expect("a number")))
    );
    assert_eq!(rig.writes(), Vec::<String>::new());
}

fn asked(rig: &Rig) -> String {
    std::fs::read_to_string(&rig.log).unwrap_or_default()
}

fn no_link(mut flags: Vec<&str>) -> Vec<&str> {
    flags.push("--no-link");
    flags
}

#[test]
fn a_branch_started_without_the_link_issues_no_develop_and_says_skipped() {
    let rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .open("start-branch", &no_link(start_flags("12", "fix-12", &root)))
        .expect("a start that skips the link");

    assert_eq!(answer["natively_linked"], false, "{answer}");
    assert_eq!(answer["link_outcome"], "skipped", "{answer}");
    assert!(answer.get("link_queued").is_none(), "{answer}");
    assert!(
        std::path::Path::new(answer["worktree"].as_str().expect("a path")).is_dir(),
        "{answer}"
    );
    assert!(!asked(&rig).contains("develop"), "{}", asked(&rig));
    assert_eq!(rig.writes(), Vec::<String>::new());
    assert_eq!(rig.queue(), Vec::<(OutboxKind, u64)>::new());
    assert!(!remote_has(&rig, "fix-12"), "the branch reached the remote");
}

#[test]
fn a_branch_started_without_the_link_while_closed_queues_no_link() {
    let rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .closed("start-branch", &no_link(start_flags("12", "fix-12", &root)))
        .expect("a start that skips the link while closed");

    assert_eq!(answer["natively_linked"], false, "{answer}");
    assert_eq!(answer["link_outcome"], "skipped", "{answer}");
    assert!(answer.get("link_queued").is_none(), "{answer}");
    assert_eq!(rig.queue(), Vec::<(OutboxKind, u64)>::new());
}

#[test]
fn a_branch_started_by_default_still_issues_develop_and_links() {
    let mut rig = repository_rig();
    let repo = rig.context.repo_dir.clone();
    git(&repo, &["branch", "fix-12", "HEAD"]);
    git(&repo, &["push", "--quiet", "origin", "--", "fix-12"]);
    rig.script = crate::test_env::answers(&[
        crate::test_env::live_holder_issue("claude-a", 12, "in-progress"),
        serde_json::json!({ "matches": "issue develop", "stdout": "", "status": 0 }),
    ]);
    let root = rig.path("worktrees/<branch>");
    let answer = rig
        .open("start-branch", &start_flags("12", "fix-12", &root))
        .expect("a default start");

    assert_eq!(answer["natively_linked"], true, "{answer}");
    assert_eq!(answer["link_outcome"], "created", "{answer}");
    assert!(asked(&rig).contains("issue develop 12"), "{}", asked(&rig));
}

mod aimed;
mod engine;

fn handoff_flags() -> Vec<&'static str> {
    vec!["--issue", "12", "--to", "opencode", "--run-id", "claude-a"]
}

fn landed_labels(rig: &Rig) -> Vec<String> {
    std::fs::read_to_string(rig.labels.as_ref().expect("a labelled rig"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[test]
fn a_handoff_while_open_projects_the_runtime_label_and_reads_it_back() {
    let rig = labelled_rig();
    let answer = rig
        .open("handoff", &handoff_flags())
        .expect("a handoff while open");

    assert_eq!(answer["projection"], "landed", "{answer}");
    assert_eq!(answer["to"], "opencode", "{answer}");
    assert!(
        rig.writes()
            .iter()
            .any(|line| line.contains("issue edit 12") && line.contains("--add-label dev:opencode")),
        "the label was not projected: {:?}",
        rig.writes()
    );
    assert!(
        landed_labels(&rig)
            .iter()
            .any(|label| label == "dev:opencode"),
        "the label did not land"
    );
    assert_eq!(rig.queue(), Vec::<(OutboxKind, u64)>::new());
}

#[test]
fn a_handoff_while_closed_queues_the_projection_and_projects_nothing() {
    let rig = labelled_rig();
    let answer = rig
        .closed("handoff", &handoff_flags())
        .expect("a handoff while closed");

    assert_eq!(answer["projection"], "deferred", "{answer}");
    assert!(answer["queued"].as_u64().is_some(), "{answer}");
    assert_eq!(rig.queue(), vec![(OutboxKind::Reconverge, 12)]);
    assert_eq!(rig.writes(), Vec::<String>::new());
}

#[test]
fn a_handoff_of_a_held_issue_is_refused_and_writes_nothing() {
    let mut rig = labelled_rig();
    rig.script = crate::test_env::answers(&[
        serde_json::json!({
            "matches": "view 12 --json labels",
            "labels": true,
            "status": 0,
        }),
        crate::test_env::live_holder_issue("claude-a", 12, "in-progress"),
    ]);
    let failure = rig
        .closed("handoff", &handoff_flags())
        .expect_err("a held issue is not handed over");

    assert_eq!(failure.envelope()["reason"], "handoff-issue-held");
    assert_eq!(rig.writes(), Vec::<String>::new());
    assert_eq!(rig.queue(), Vec::<(OutboxKind, u64)>::new());
}

#[test]
fn a_handoff_of_an_issue_claimed_only_in_the_store_is_refused() {
    let rig = labelled_rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim recorded in the store");
    let queued = rig.queue();
    let failure = rig
        .closed("handoff", &handoff_flags())
        .expect_err("the store's holder is a holder");

    assert_eq!(failure.envelope()["reason"], "handoff-issue-held");
    assert_eq!(failure.envelope()["holder"], "claude-a");
    assert_eq!(rig.queue(), queued);
}

#[test]
fn a_handoff_naming_an_unlabellable_runtime_writes_nothing() {
    let rig = labelled_rig();
    let failure = rig
        .closed(
            "handoff",
            &[
                "--issue",
                "12",
                "--to",
                "OpenCode Two",
                "--run-id",
                "claude-a",
            ],
        )
        .expect_err("a runtime a label cannot carry");

    assert_eq!(failure.envelope()["reason"], "handoff-runtime-unreadable");
    assert_eq!(rig.writes(), Vec::<String>::new());
    assert_eq!(rig.queue(), Vec::<(OutboxKind, u64)>::new());
}
