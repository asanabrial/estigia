use std::cell::RefCell;

use serde_json::{Value, json};

use super::{Inputs, Tracker, answer};
use crate::harness::session::{self, run_id};
use crate::transport::Context;
use crate::transport::store::Repo;

const NOW: u64 = 1_800_000_000;
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "2222222222222222222222222222222222222222";
const DIGEST: &str = "4444444444444444444444444444444444444444444444444444444444444444";
const MOVED: &str = "3333333333333333333333333333333333333333";

struct Fake {
    issue: Result<Value, String>,
    pull: Result<Value, String>,
    asked: RefCell<Vec<String>>,
}

impl Tracker for Fake {
    fn issue(&self, repo: &Repo, number: u64) -> Result<Value, String> {
        self.asked
            .borrow_mut()
            .push(format!("issue {}/{}#{number}", repo.owner, repo.name));
        self.issue.clone()
    }

    fn pull(&self, repo: &Repo, number: u64) -> Result<Value, String> {
        self.asked
            .borrow_mut()
            .push(format!("pull {}/{}#{number}", repo.owner, repo.name));
        self.pull.clone()
    }
}

fn tracker() -> Fake {
    Fake {
        issue: Ok(json!({"state": "open", "html_url": "https://github.com/o/b/issues/77"})),
        pull: Ok(json!({
            "number": 3, "url": "https://github.com/o/b/pull/3", "state": "OPEN",
            "headRefOid": MOVED, "baseRefOid": BASE,
        })),
        asked: RefCell::default(),
    }
}

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("runs")
    }

    fn store(&self) -> std::path::PathBuf {
        self.directory.path().join("state.db")
    }

    fn ask(&self, reference: &str, tracker: &Fake) -> Result<Value, crate::outcome::Refusal> {
        let context = Context {
            skill_dir: self.directory.path().to_path_buf(),
            repo_dir: self.directory.path().to_path_buf(),
            config: Vec::new(),
            repo: Some("o/a".to_owned()),
        };
        let root = self.root();
        answer(
            reference,
            &Inputs {
                context: &context,
                root: &root,
                now: Some(NOW),
                tracker,
            },
        )
    }

    fn sql(&self, script: &str) {
        rusqlite::Connection::open(self.store())
            .expect("the store opens")
            .execute_batch(script)
            .expect("the fixture seeds");
    }
}

fn checkout(directory: &std::path::Path, url: &str) {
    std::fs::create_dir_all(directory).expect("a checkout directory");
    for arguments in [
        vec!["init", "--quiet", "-b", "feature"],
        vec!["remote", "add", "origin", url],
    ] {
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(directory)
                .args(&arguments)
                .output()
                .is_ok_and(|output| output.status.success()),
            "git {arguments:?}"
        );
    }
}

fn rig() -> Rig {
    let rig = Rig {
        directory: tempfile::tempdir().expect("a temporary directory"),
    };
    drop(crate::transport::store::Store::open(&rig.store()).expect("the store opens"));
    let work = rig.directory.path().join("work-b");
    checkout(&work, "git@github.com:o/b.git");
    let pi = run_id("pi", "alpha");
    let claude = run_id("claude", "beta");
    let iso = |offset: u64| {
        format!(
            "strftime('%Y-%m-%dT%H:%M:%SZ', {}, 'unixepoch')",
            NOW - 1_000 + offset
        )
    };
    rig.sql(&format!(
        "INSERT INTO repos (id, host, owner, name) VALUES
           (1, 'github.com', 'o', 'a'), (2, 'github.com', 'o', 'b');
         INSERT INTO issues (repo_id, number, state, history) VALUES
           (1, 9, 'ready', 'native'), (2, 77, 'review', 'native');
         INSERT INTO entries (repo_id, issue, created_at, body, kind, run_id,
                              viewer_did_author, includes_created_edit, source)
           VALUES (2, 77, {e1}, '<!-- issue-flow: published run-id={pi} epoch=0123456789abcdef0123456789abcdef pr=3 head={HEAD} base={BASE} digest={DIGEST} -->',
                   'published', '{pi}', 1, 0, 'local');
         INSERT INTO transitions (repo_id, issue, at, run_id, from_state, to_state)
           VALUES (2, 77, {e1}, '{pi}', 'ready', 'in-progress'), (2, 77, {e2}, '{pi}', 'in-progress', 'review'),
                  (1, 9, {e2}, NULL, NULL, 'ready');
         INSERT INTO tasks (repo_id, issue, id, text, route, done, commit_id, at) VALUES
           (2, 77, 'T1', 'one', 'inline', 1, 'abc', 'x'), (2, 77, 'T2', 'two', 'inline', 0, NULL, 'x');
         INSERT INTO blocked_clauses (repo_id, issue, body, at) VALUES (2, 77, 'until #12 closes', 'x');
         INSERT INTO outbox (repo_id, issue, kind, payload, op_id, state, enqueued_at)
           VALUES (2, 77, 'comment', '{{}}', 'op-1', 'queued', 'x'), (2, 77, 'comment', '{{}}', 'op-2', 'sent', 'x');
         INSERT INTO sessions (run_id, runtime, session_id, repo_dir, started_at, last_seen)
           VALUES ('{pi}', 'pi', 'alpha', '{work}', {start}, {seen});
         INSERT INTO messages (sender_run_id, recipient_run_id, body, created_at)
           VALUES ('{claude}', '{pi}', 'to the holder', {m1}), ('{pi}', '{claude}', 'from the holder', {m2});",
        e1 = iso(10),
        e2 = iso(20),
        work = work.display(),
        start = NOW - 2_000,
        seen = NOW - 30,
        m1 = NOW - 500,
        m2 = NOW - 400,
    ));
    session::update(&rig.root(), &pi, |run| {
        run.issue = Some(crate::identity::Identity::Number(77));
        run.state = Some("review".to_owned());
        run.repo_dir = Some(work.clone());
        run.worktree = Some(work.clone());
    });
    rig
}

#[test]
fn a_number_only_another_repository_holds_answers_from_that_repository() {
    let fake = tracker();
    let said = rig().ask("77", &fake).expect("an answer");

    assert_eq!(said["scope"]["repo"], "o/b", "{said}");
    assert_eq!(said["scope"]["basis"], "candidate", "{said}");
    assert_eq!(said["workflow"]["state"], "review", "{said}");
    assert_eq!(said["issue"]["state"], "open", "{said}");
    assert_eq!(
        said["issue"]["url"], "https://github.com/o/b/issues/77",
        "{said}"
    );
}

#[test]
fn the_last_transitions_the_holder_the_checklist_and_the_clauses_come_with_it() {
    let said = rig().ask("o/b#77", &tracker()).expect("an answer");

    let moves: Vec<&Value> = said["transitions"]["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .collect();
    assert_eq!(moves.len(), 2, "{said}");
    assert_eq!(moves[1]["to"], "review");
    assert_eq!(said["holders"]["rows"][0]["runtime"], "pi", "{said}");
    assert_eq!(said["holders"]["rows"][0]["status"], "live", "{said}");
    assert_eq!(said["holders"]["rows"][0]["branch"], "feature", "{said}");
    assert_eq!(
        (
            said["tasks"]["done"].clone(),
            said["tasks"]["total"].clone()
        ),
        (json!(1), json!(2))
    );
    assert_eq!(
        said["blocked_until"]["clauses"],
        json!(["until #12 closes"]),
        "{said}"
    );
}

#[test]
fn the_verdict_is_shown_with_the_head_and_base_it_is_bound_to_and_the_live_head_beside_it() {
    let said = rig().ask("77", &tracker()).expect("an answer");

    assert_eq!(said["review"]["verdict"], "unjudged", "{said}");
    assert_eq!(said["review"]["head"], HEAD, "{said}");
    assert_eq!(said["review"]["base"], BASE, "{said}");
    assert_eq!(said["pull_request"]["number"], 3, "{said}");
    assert_eq!(said["pull_request"]["state"], "OPEN", "{said}");
    assert_eq!(said["pull_request"]["head"], MOVED, "{said}");
    assert_eq!(said["review"]["bound_to_live_head"], false, "{said}");
}

#[test]
fn messages_to_and_from_the_holder_and_queued_writes_are_listed() {
    let said = rig().ask("77", &tracker()).expect("an answer");

    assert_eq!(
        said["messages"]["rows"].as_array().map(Vec::len),
        Some(2),
        "{said}"
    );
    let queued = said["outbox"]["rows"].as_array().expect("rows");
    assert_eq!(queued.len(), 1, "a sent write was listed as queued: {said}");
    assert_eq!(queued[0]["status"], "queued, not landed");
}

#[test]
fn a_number_held_in_two_repositories_is_refused_with_both() {
    let rig = rig();
    rig.sql(
        "INSERT INTO issues (repo_id, number, state, history) VALUES (1, 77, 'ready', 'native');",
    );

    let refused = rig.ask("77", &tracker()).expect_err("two candidates");

    assert_eq!(refused.code, "ambiguous-issue");
    assert!(
        refused.message.contains("o/a") && refused.message.contains("o/b"),
        "{refused:?}"
    );
}

#[test]
fn a_number_nobody_holds_names_the_checkout_as_its_scope() {
    let said = rig().ask("123", &tracker()).expect("an answer");

    assert_eq!(said["scope"]["basis"], "checkout", "{said}");
    assert_eq!(said["scope"]["repo"], "o/a", "{said}");
    assert!(said["workflow"]["unavailable"].is_string(), "{said}");
}

#[test]
fn a_field_that_cannot_be_read_says_so_and_is_never_filled_from_a_nearby_state() {
    let fake = Fake {
        issue: Err("gh is not logged in".to_owned()),
        pull: Err("gh is not logged in".to_owned()),
        asked: RefCell::default(),
    };
    let said = rig().ask("77", &fake).expect("an answer");

    assert!(
        said["issue"]["unavailable"]
            .as_str()
            .is_some_and(|why| why.contains("not logged in")),
        "{said}"
    );
    assert!(said["pull_request"]["unavailable"].is_string(), "{said}");
    assert!(said["pull_request"].get("state").is_none(), "{said}");
    assert!(
        said["review"]["unavailable"].is_null(),
        "the recorded receipt is a store read: {said}"
    );
    assert_eq!(said["review"]["bound_to_live_head"], Value::Null, "{said}");
    assert_eq!(said["workflow"]["state"], "review", "{said}");
}

#[test]
fn a_store_that_does_not_exist_makes_every_store_field_unavailable_and_creates_nothing() {
    let rig = Rig {
        directory: tempfile::tempdir().expect("a directory"),
    };

    let said = rig.ask("o/b#77", &tracker()).expect("an answer");

    for field in [
        "workflow",
        "transitions",
        "holders",
        "tasks",
        "blocked_until",
        "messages",
        "outbox",
        "review",
    ] {
        assert!(said[field]["unavailable"].is_string(), "{field}: {said}");
    }
    assert!(!rig.store().exists(), "a read created the store");
}

#[test]
fn a_repository_the_store_never_saw_is_unavailable_and_not_empty() {
    let said = rig().ask("x/y#1", &tracker()).expect("an answer");

    assert!(said["tasks"]["unavailable"].is_string(), "{said}");
    assert!(said["tasks"].get("total").is_none(), "{said}");
}

#[test]
fn a_linear_key_is_answered_with_every_field_unavailable() {
    let fake = tracker();
    let said = rig().ask("eng-12", &fake).expect("an answer");

    assert_eq!(said["scope"]["tracker"], "linear", "{said}");
    assert!(
        said["workflow"]["unavailable"]
            .as_str()
            .is_some_and(|why| why.contains("Linear")),
        "{said}"
    );
    assert!(
        fake.asked.borrow().is_empty(),
        "a Linear key reached the GitHub tracker"
    );
}

#[test]
fn asking_writes_nothing_to_the_store_and_reads_no_claim() {
    let rig = rig();
    let before = std::fs::read(rig.store()).expect("the store reads");
    let wal = rig.directory.path().join("state.db-wal");

    rig.ask("77", &tracker()).expect("an answer");

    assert_eq!(std::fs::read(rig.store()).expect("the store reads"), before);
    assert!(!wal.exists() || std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0) == 0);
}
