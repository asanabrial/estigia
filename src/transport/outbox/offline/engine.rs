use super::*;
use crate::transport::store::{OutboxRow, OutboxState};

const LINKED: &str = "linkedBranches";

fn rows() -> Vec<OutboxRow> {
    Store::open(&store_path().expect("a path"))
        .expect("opens")
        .outbox(&repo())
        .expect("reads")
}

fn state_of(kind: OutboxKind) -> Vec<OutboxState> {
    rows()
        .into_iter()
        .filter(|row| row.kind == kind)
        .map(|row| row.state)
        .collect()
}

fn comments_answer(issue: u64, operations: &[String]) -> serde_json::Value {
    let bodies: Vec<serde_json::Value> = operations
        .iter()
        .map(|op| {
            serde_json::json!({
                "node_id": format!("IC_{op}"),
                "id": 1,
                "created_at": "2026-01-01T20:00:01Z",
                "user": { "login": "fixture" },
                "body": format!("A note.\n<!-- estigia-outbox:{op} -->\n"),
            })
        })
        .collect();
    serde_json::json!({
        "matches": format!("issues/{issue}/comments"),
        "stdout": serde_json::Value::Array(bodies).to_string(),
        "status": 0,
    })
}

fn ops_of(kind: OutboxKind) -> Vec<String> {
    rows()
        .into_iter()
        .filter(|row| row.kind == kind)
        .map(|row| row.op_id)
        .collect()
}

impl Rig {
    fn flushed(&self, answers: &[serde_json::Value]) -> Result<serde_json::Value, Failure> {
        let script = crate::test_env::answers(answers);
        let bin = crate::test_env::scripted_gh();
        let runs = self.directory.path().join("runs");
        crate::test_env::with_logging_gh(bin.path(), &script, &self.log, || {
            crate::harness::operation_window::with_now_override(Some(noon), || {
                outbox::flush::run(&self.context, &runs, NOW)
            })
        })
    }

    fn asked(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn comment_writes(&self) -> Vec<String> {
        self.writes()
            .into_iter()
            .filter(|line| line.starts_with("issue comment"))
            .collect()
    }
}

fn view(issue: u64, label: &str) -> serde_json::Value {
    crate::test_env::issue_view_answer(
        issue,
        serde_json::json!({
            "state": "OPEN",
            "title": "A probe",
            "labels": [{ "name": format!("status:{label}") }],
            "comments": [],
        }),
    )
}

fn queue_a_note(rig: &Rig, issue: &str) {
    rig.closed(
        "comment",
        &["--issue", issue, "--body-file", &rig.path("note.md")],
    )
    .expect("a comment while closed");
}

#[test]
fn a_flush_while_closed_sends_nothing_and_keeps_the_queue() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let failure = rig.closed("flush", &[]).expect_err("a flush while closed");

    assert_eq!(failure.envelope()["reason"], "git-operation-window-denied");
    assert_eq!(rig.asked(), "");
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Queued]);
}

#[test]
fn the_queue_is_sent_in_the_order_it_was_made() {
    let rig = rig();
    queue_a_note(&rig, "12");
    rig.closed("transition", &["--issue", "12", "--to", "review"])
        .expect("a transition");
    queue_a_note(&rig, "12");
    let notes = ops_of(OutboxKind::Comment);

    let report = rig
        .flushed(&[comments_answer(12, &notes), view(12, "review")])
        .expect("a flush");

    assert_eq!(report["sent"], 3, "{report}");
    assert_eq!(report["remaining"], 0, "{report}");
    let verbs: Vec<String> = rig
        .writes()
        .iter()
        .filter(|line| line.starts_with("issue comment") || line.starts_with("issue edit"))
        .map(|line| {
            line.split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(verbs, ["issue comment", "issue edit", "issue comment"]);
    assert!(rows().iter().all(|row| row.state == OutboxState::Sent));
}

#[test]
fn a_created_issue_takes_its_number_before_the_next_write_on_it() {
    let rig = rig();
    let provisional = created(&rig);
    queue_a_note(&rig, &provisional.to_string());
    let notes = ops_of(OutboxKind::Comment);

    let report = rig
        .flushed(&[
            serde_json::json!({
                "matches": "issue create",
                "stdout": "https://github.com/fixture/repo/issues/77",
                "status": 0,
            }),
            comments_answer(77, &notes),
            view(77, "ready"),
        ])
        .expect("a flush");

    assert_eq!(report["sent"], 3, "{report}");
    let asked = rig.asked();
    assert!(asked.contains("issue comment 77"), "{asked}");
    assert!(
        !asked.contains(&provisional.to_string()),
        "GitHub was asked about the provisional number: {asked}"
    );
    assert_eq!(rig.state(77).as_deref(), Some("ready"));
    assert_eq!(rig.state(provisional), None);
    assert!(rows().iter().all(|row| row.issue == 77));
}

#[test]
fn a_create_for_another_repository_while_closed_is_queued_there_and_filed_by_a_flush_from_it() {
    let mut rig = rig();
    rig.context.repo = Some("other/target".to_owned());
    let home = tempfile::tempdir().expect("a home");
    let _destination = destination(home.path(), "other/target", "enabled 09:00-17:00");
    with_home(home.path(), || {
        let provisional = created(&rig);
        let target = Repo {
            host: "github.com".to_owned(),
            owner: "other".to_owned(),
            name: "target".to_owned(),
        };
        let held = |repo: &Repo| -> Vec<(OutboxKind, u64)> {
            Store::open(&store_path().expect("a path"))
                .expect("opens")
                .outbox(repo)
                .expect("reads")
                .iter()
                .map(|row| (row.kind, row.issue))
                .collect()
        };

        assert_eq!(
            held(&target),
            vec![
                (OutboxKind::Create, provisional),
                (OutboxKind::Reconverge, provisional)
            ]
        );
        assert_eq!(held(&repo()), Vec::new());

        rig.context.repo = Some("fixture/repo".to_owned());
        let elsewhere = rig.flushed(&[]).expect("a flush from the other checkout");
        assert_eq!(elsewhere["sent"], 0, "{elsewhere}");
        assert_eq!(rig.writes(), Vec::<String>::new());
        assert_eq!(held(&target).len(), 2);

        rig.context.repo = Some("other/target".to_owned());
        let report = rig
            .flushed(&[
                serde_json::json!({
                    "matches": "issue create",
                    "stdout": "https://github.com/other/target/issues/41",
                    "status": 0,
                }),
                view(41, "ready"),
            ])
            .expect("a flush from the target checkout");
        assert_eq!(report["remaining"], 0, "{report}");
        assert!(
            rig.writes()
                .iter()
                .any(|line| line.starts_with("issue create")),
            "the flush from the target checkout filed nothing: {:?}",
            rig.writes()
        );
        assert!(
            held(&target).iter().all(|(_, issue)| *issue == 41),
            "{:?}",
            held(&target)
        );
    });
}

#[test]
fn an_ambiguous_create_stops_the_queue_with_the_rest_still_queued() {
    let rig = rig();
    let provisional = created(&rig);
    queue_a_note(&rig, &provisional.to_string());

    let failure = rig
        .flushed(&[serde_json::json!({
            "matches": "issue create",
            "stderr": "HTTP 502",
            "status": 1,
        })])
        .expect_err("a create that failed");

    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "outbox-pending", "{envelope}");
    assert_eq!(envelope["stopped_at"]["kind"], "create", "{envelope}");
    assert_eq!(envelope["stopped_at"]["issue"], provisional, "{envelope}");
    assert_eq!(envelope["remaining"], 3, "{envelope}");
    assert_eq!(state_of(OutboxKind::Create), vec![OutboxState::Sending]);
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Queued]);
    assert_eq!(rig.comment_writes(), Vec::<String>::new());
}

#[test]
fn a_create_left_sending_is_adopted_by_its_title_and_not_filed_twice() {
    let rig = rig();
    let provisional = created(&rig);
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let first = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(first.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    let report = rig
        .flushed(&[
            serde_json::json!({
                "matches": "issue list",
                "stdout": serde_json::json!([{
                    "number": 77,
                    "title": "A probe",
                    "url": "https://github.com/fixture/repo/issues/77",
                    "createdAt": "2026-01-01T20:00:05Z",
                }]).to_string(),
                "status": 0,
            }),
            view(77, "ready"),
        ])
        .expect("a flush");

    assert_eq!(report["sent"], 2, "{report}");
    assert!(!rig.asked().contains("issue create"), "{}", rig.asked());
    assert_eq!(rig.state(77).as_deref(), Some("ready"));
    assert_eq!(rig.state(provisional), None);
}

fn sending_create(rig: &Rig) {
    created(rig);
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let first = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(first.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
}

fn listing(created_at: Option<&str>) -> serde_json::Value {
    let mut item = serde_json::json!({
        "number": 77,
        "title": "A probe",
        "url": "https://github.com/fixture/repo/issues/77",
    });
    if let Some(at) = created_at {
        item["createdAt"] = serde_json::json!(at);
    }
    serde_json::json!({
        "matches": "issue list",
        "stdout": serde_json::json!([item]).to_string(),
        "status": 0,
    })
}

fn filed(number: u64) -> serde_json::Value {
    serde_json::json!({
        "matches": "issue create",
        "stdout": format!("https://github.com/fixture/repo/issues/{number}\n"),
        "status": 0,
    })
}

#[test]
fn a_create_left_sending_does_not_adopt_an_issue_older_than_the_row() {
    let rig = rig();
    sending_create(&rig);

    let report = rig
        .flushed(&[
            listing(Some("2025-12-31T09:00:00Z")),
            filed(78),
            view(78, "ready"),
        ])
        .expect("a flush");

    assert_eq!(report["sent"], 2, "{report}");
    assert!(rig.asked().contains("issue create"), "{}", rig.asked());
    assert_eq!(rig.state(77), None);
    assert_eq!(rig.state(78).as_deref(), Some("ready"));
}

#[test]
fn a_create_left_sending_does_not_adopt_an_issue_without_a_creation_time() {
    let rig = rig();
    sending_create(&rig);

    let report = rig
        .flushed(&[listing(None), filed(78), view(78, "ready")])
        .expect("a flush");

    assert_eq!(report["sent"], 2, "{report}");
    assert_eq!(rig.state(77), None);
}

#[test]
fn a_create_left_sending_does_not_adopt_an_issue_the_store_already_holds() {
    let rig = rig();
    created(&rig);
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    held.import(
        &repo(),
        77,
        &serde_json::json!({
            "body": "an unrelated issue",
            "labels": [{ "name": "status:done" }],
            "comments": [],
        }),
        NOW,
    )
    .expect("an issue the store already holds");
    let first = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(first.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    let mut outcomes = Vec::new();
    for answers in [
        vec![
            listing(Some("2026-01-01T20:00:05Z")),
            filed(78),
            view(78, "ready"),
        ],
        vec![],
    ] {
        outcomes.push(rig.flushed(&answers).is_ok());
    }

    assert_eq!(outcomes, [true, true]);
    assert_eq!(rig.state(77).as_deref(), Some("done"));
    assert_eq!(rig.state(78).as_deref(), Some("ready"));
}

fn assert_unread_search_files_nothing(stdout: &str) {
    let rig = rig();
    sending_create(&rig);

    let outcome = rig.flushed(&[serde_json::json!({
        "matches": "issue list",
        "stdout": stdout,
        "status": 0,
    })]);

    assert!(outcome.is_err(), "{outcome:?}");
    assert!(
        !rig.asked().contains("issue create"),
        "{stdout:?} was read as no such issue: {}",
        rig.asked()
    );
    assert_eq!(state_of(OutboxKind::Create), vec![OutboxState::Sending]);
}

#[test]
fn a_title_search_that_answers_a_non_list_does_not_file_the_create_again() {
    assert_unread_search_files_nothing("{\"message\":\"not a list\"}");
}

#[test]
fn an_empty_title_search_answer_does_not_file_the_create_again() {
    assert_unread_search_files_nothing("");
}

#[test]
fn a_comment_left_sending_that_already_landed_is_not_posted_again() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let row = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(row.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    let report = rig
        .flushed(&[comments_answer(12, &[row.op_id])])
        .expect("a flush");

    assert_eq!(report["sent"], 1, "{report}");
    assert_eq!(rig.comment_writes(), Vec::<String>::new());
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Sent]);
}

#[test]
fn a_comment_left_sending_that_cannot_be_read_back_stops_before_posting() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let row = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(row.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    let failure = rig
        .flushed(&[serde_json::json!({
            "matches": "issues/12/comments",
            "stderr": "HTTP 502",
            "status": 1,
        })])
        .expect_err("a readback that failed");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert_eq!(rig.comment_writes(), Vec::<String>::new());
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Sending]);
}

#[test]
fn a_reconverge_that_cannot_be_read_back_stops_the_queue() {
    let rig = rig();
    rig.closed("transition", &["--issue", "12", "--to", "review"])
        .expect("a transition");
    queue_a_note(&rig, "12");

    let failure = rig
        .flushed(&[view(12, "in-progress")])
        .expect_err("a readback that disagrees");

    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "outbox-pending", "{envelope}");
    assert_eq!(envelope["stopped_at"]["kind"], "reconverge", "{envelope}");
    assert_eq!(state_of(OutboxKind::Reconverge), vec![OutboxState::Sending]);
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Queued]);
    assert_eq!(rig.comment_writes(), Vec::<String>::new());
}

#[test]
fn a_first_attempt_that_learns_nothing_goes_back_to_the_queue() {
    let rig = rig();
    created(&rig);
    let held = Store::open(&store_path().expect("a path")).expect("opens");
    held.set_meta(
        &crate::transport::commands::pending_create_key(&repo(), "a-probe", "A probe"),
        "A probe",
    )
    .expect("a mark");
    drop(held);

    let failure = rig
        .flushed(&[serde_json::json!({
            "matches": "issue list",
            "stderr": "HTTP 502",
            "status": 1,
        })])
        .expect_err("a search that failed");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert!(!rig.asked().contains("issue create"), "{}", rig.asked());
    assert_eq!(state_of(OutboxKind::Create), vec![OutboxState::Queued]);
    assert!(rows()[0].detail.is_some());
}

#[test]
fn a_write_already_sent_once_never_goes_back_to_the_queue() {
    let rig = rig();
    let provisional = created(&rig);
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let first = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(first.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    let failure = rig
        .flushed(&[serde_json::json!({
            "matches": "issue list",
            "stderr": "HTTP 502",
            "status": 1,
        })])
        .expect_err("a search that failed");

    assert_eq!(failure.envelope()["stopped_at"]["issue"], provisional);
    assert_eq!(state_of(OutboxKind::Create), vec![OutboxState::Sending]);
}

#[test]
fn a_comment_gh_refused_stays_sending_because_the_refusal_cannot_say_nothing_landed() {
    let rig = rig();
    queue_a_note(&rig, "12");

    let failure = rig
        .flushed(&[serde_json::json!({
            "matches": "issue comment",
            "stderr": "gh: not logged in",
            "status": 4,
        })])
        .expect_err("a comment that was refused");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert_eq!(rig.comment_writes().len(), 1);
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Sending]);
}

#[test]
fn a_queued_link_is_made_and_read_back() {
    let mut rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    rig.closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("a start while closed");
    rig.script = crate::test_env::answers(&[]);

    let linked = serde_json::json!({
        "data": { "repository": { "issue": { LINKED: {
            "nodes": [{ "ref": { "name": "fix-12" } }],
            "pageInfo": { "hasNextPage": false },
        } } } }
    });
    let report = rig
        .flushed(&[
            serde_json::json!({
                "matches": "repo view",
                "stdout": serde_json::json!({ "owner": { "login": "fixture" }, "name": "repo" })
                    .to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": LINKED,
                "stdout": linked.to_string(),
                "status": 0,
            }),
        ])
        .expect("a flush");

    assert_eq!(report["sent"], 1, "{report}");
    assert!(rig.asked().contains("issue develop 12"), "{}", rig.asked());
    assert_eq!(state_of(OutboxKind::Link), vec![OutboxState::Sent]);
}

#[cfg(unix)]
#[test]
fn a_flush_does_not_start_while_a_live_process_holds_the_lease() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let seconds = crate::harness::session::now_seconds().unwrap_or_default();
    held.take_outbox_lease(&repo(), 1, seconds, seconds + 600, |_| true)
        .expect("taken");
    drop(held);

    let failure = rig.flushed(&[]).expect_err("a flush behind a lease");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert_eq!(rig.asked(), "");
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    held.drop_outbox_lease(&repo(), 1).expect("dropped");
}

#[cfg(unix)]
#[test]
fn a_flush_for_another_repository_does_not_refuse_this_ones_write() {
    let rig = rig();
    let other = Repo {
        host: "github.com".to_owned(),
        owner: "fixture".to_owned(),
        name: "other".to_owned(),
    };
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    crate::transport::outbox::queue_in(
        &mut held,
        &other,
        5,
        OutboxKind::Comment,
        &serde_json::json!({ "body": "elsewhere" }),
        NOW,
    )
    .expect("a write queued for another repository");
    let seconds = crate::harness::session::now_seconds().unwrap_or_default();
    held.take_outbox_lease(&other, 1, seconds, seconds + 600, |_| true)
        .expect("taken");
    drop(held);

    let outcome = rig.open("transition", &["--issue", "12", "--to", "review"]);
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    held.drop_outbox_lease(&other, 1).expect("dropped");
    if let Err(failure) = outcome {
        assert_ne!(failure.envelope()["reason"], "outbox-pending");
    }
}

#[test]
fn a_reconverge_replaced_while_queued_keeps_the_ownership_it_carried() {
    let rig = rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim");
    rig.closed("transition", &["--issue", "12", "--to", "review"])
        .expect("a transition");

    let waiting: Vec<_> = rows()
        .into_iter()
        .filter(|row| row.kind == OutboxKind::Reconverge && row.state == OutboxState::Queued)
        .collect();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].payload["ownership"], true);
}

#[test]
fn a_queued_claim_projects_the_assignee_and_runtime_label_from_the_store() {
    let rig = rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim while closed");
    let bare = crate::test_env::issue_view_answer(
        12,
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{ "name": "status:in-progress" }],
            "comments": [],
        }),
    );
    let user = serde_json::json!({
        "matches": "api user",
        "stdout": "{\"login\":\"fixture-user\"}",
        "status": 0,
    });

    let failure = rig
        .flushed(&[user, bare])
        .expect_err("a projection the fixture never shows landing");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert_eq!(failure.envelope()["stopped_at"]["kind"], "reconverge");
    let edits: Vec<String> = rig
        .writes()
        .into_iter()
        .filter(|line| line.starts_with("issue edit 12"))
        .collect();
    assert!(
        edits
            .iter()
            .any(|line| line.contains("--add-assignee @me")
                && line.contains("--add-label dev:claude")),
        "no edit projected the owner from the store: {edits:?}"
    );
}

fn listed(rig: &Rig, state: &str) -> serde_json::Value {
    rig.closed(
        "list-state",
        &["--state", state, "--run-id", "claude-a", "--limit", "50"],
    )
    .expect("a listing while closed")
}

fn listed_numbers(answer: &serde_json::Value) -> Vec<u64> {
    answer["partitions"]
        .as_object()
        .into_iter()
        .flat_map(|partitions| partitions.values())
        .flat_map(|bucket| bucket.as_array().into_iter().flatten())
        .filter_map(|entry| entry["number"].as_u64())
        .collect()
}

#[test]
fn an_issue_created_while_closed_is_listed_as_provisional_in_the_state_the_store_gives_it() {
    let rig = rig();
    let provisional = created(&rig);

    let ready = listed(&rig, "ready");
    assert_eq!(listed_numbers(&ready), vec![provisional], "{ready}");
    assert!(
        ready.to_string().contains("\"provisional\":true"),
        "the listing does not mark the issue as not yet on GitHub: {ready}"
    );
    assert!(ready.to_string().contains("A probe"), "{ready}");
    assert_eq!(rig.writes(), Vec::<String>::new());

    rig.closed(
        "transition",
        &["--issue", &provisional.to_string(), "--to", "review"],
    )
    .expect("a transition while closed");
    assert_eq!(listed_numbers(&listed(&rig, "ready")), Vec::<u64>::new());
}

#[test]
fn a_listing_with_no_provisional_issue_carries_no_trace_of_one() {
    let rig = rig();
    let ready = listed(&rig, "ready");
    assert_eq!(listed_numbers(&ready), Vec::<u64>::new());
    assert!(!ready.to_string().contains("provisional"), "{ready}");
}

#[test]
fn the_identity_of_an_issue_created_while_closed_is_listed_so_it_is_not_filed_twice() {
    let rig = rig();
    let provisional = created(&rig);

    let answer = rig
        .closed("list-state", &["--identities"])
        .expect("a listing while closed");

    assert_eq!(
        answer["identities"],
        serde_json::json!([{
            "number": provisional,
            "identity": "a-probe",
            "provisional": true,
        }]),
        "{answer}"
    );
}

fn publish_flags(body: &str) -> Vec<&str> {
    vec![
        "--issue",
        "12",
        "--run-id",
        "claude-a",
        "--branch",
        "fix-12",
        "--base",
        "main",
        "--pr-title",
        "A probe",
        "--pr-body-file",
        body,
    ]
}

#[test]
fn a_write_while_open_sends_what_the_closed_window_queued_first() {
    let mut rig = rig();
    queue_a_note(&rig, "12");
    rig.script = crate::test_env::answers(&[
        comments_answer(12, &ops_of(OutboxKind::Comment)),
        view(12, "review"),
    ]);

    rig.open("transition", &["--issue", "12", "--to", "review"])
        .expect("a transition behind a queue that drains");

    let verbs: Vec<String> = rig
        .writes()
        .iter()
        .filter(|line| line.starts_with("issue comment") || line.starts_with("issue edit"))
        .map(|line| {
            line.split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    assert_eq!(verbs, ["issue comment", "issue edit"]);
    assert!(rows().iter().all(|row| row.state == OutboxState::Sent));
}

#[test]
fn a_write_while_open_is_refused_when_the_queue_does_not_drain() {
    let mut rig = rig();
    queue_a_note(&rig, "12");
    rig.script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issue comment",
        "stderr": "HTTP 502",
        "status": 1,
    })]);

    let failure = rig
        .open("transition", &["--issue", "12", "--to", "review"])
        .expect_err("a transition behind a queue that stalled");

    assert_eq!(failure.envelope()["reason"], "outbox-pending");
    assert!(
        !rig.writes()
            .iter()
            .any(|line| line.starts_with("issue edit")),
        "{:?}",
        rig.writes()
    );
    assert_eq!(rig.state(12).as_deref(), None);
}

#[test]
fn an_open_write_with_an_empty_queue_asks_nothing_extra() {
    let rig = rig();
    let _ = rig.open("transition", &["--issue", "12", "--to", "review"]);

    assert!(!rig.asked().contains("issue comment"), "{}", rig.asked());
    assert_eq!(rows(), Vec::new());
}

#[test]
fn a_publication_refused_while_closed_says_what_is_queued_and_what_runs_it() {
    let mut rig = rig();
    rig.closed("claim", &claim_flags("12", "claude-a", FIRST))
        .expect("a claim while closed");
    queue_a_note(&rig, "12");
    rig.script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-a",
        12,
        "in-progress",
    )]);
    let failure = rig
        .closed("publish-review", &publish_flags(&rig.path("body.md")))
        .expect_err("a publication while closed");

    let envelope = failure.envelope();
    assert_eq!(
        envelope["reason"], "git-operation-window-denied",
        "{envelope}"
    );
    let action = envelope["action"].as_str().expect("an action");
    assert!(
        action.contains("work stays uncommitted in the worktree"),
        "{action}"
    );
    assert!(action.contains("2 writes are queued"), "{action}");
    assert!(
        action.contains("`estigia flush` runs when the window opens"),
        "{action}"
    );
}

#[test]
fn a_denial_with_nothing_queued_keeps_its_old_text() {
    let window = crate::config::GitOperationWindow::parse("enabled 09:00-17:00").expect("parses");
    let failure = crate::harness::operation_window::with_now_override(Some(night), || {
        crate::transport::branch::schedule_denial(&window)
    });

    let envelope = failure.envelope();
    assert_eq!(
        envelope["reason"], "git-operation-window-denied",
        "{envelope}"
    );
    assert!(
        !envelope["action"]
            .as_str()
            .unwrap_or_default()
            .contains("queued"),
        "{envelope}"
    );
}

#[test]
fn the_queue_can_be_read_while_the_window_is_closed_without_asking_github() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let report = rig
        .closed("flush", &["--read-only"])
        .expect("a status while closed");

    assert_eq!(report["window"], "closed", "{report}");
    assert_eq!(report["remaining"], 1, "{report}");
    assert_eq!(report["writes"][0]["kind"], "comment", "{report}");
    assert_eq!(report["writes"][0]["state"], "queued", "{report}");
    assert_eq!(rig.asked(), "");
}

fn not_linked_answers() -> Vec<serde_json::Value> {
    let unlinked = serde_json::json!({
        "data": { "repository": { "issue": { LINKED: {
            "nodes": [],
            "pageInfo": { "hasNextPage": false },
        } } } }
    });
    vec![
        serde_json::json!({
            "matches": "issue develop",
            "stderr": "a branch named 'fix-12' already exists",
            "status": 1,
        }),
        serde_json::json!({
            "matches": "repo view",
            "stdout": serde_json::json!({ "owner": { "login": "fixture" }, "name": "repo" })
                .to_string(),
            "status": 0,
        }),
        serde_json::json!({ "matches": LINKED, "stdout": unlinked.to_string(), "status": 0 }),
    ]
}

fn queued_link_rig() -> Rig {
    let rig = repository_rig();
    let root = rig.path("worktrees/<branch>");
    rig.closed("start-branch", &start_flags("12", "fix-12", &root))
        .expect("a start while closed");
    rig
}

#[test]
fn a_link_gh_cannot_make_is_settled_with_the_reason_and_does_not_hold_the_queue() {
    let rig = queued_link_rig();

    let report = rig
        .flushed(&not_linked_answers())
        .expect("the flush completes");

    assert_eq!(report["sent"], 1, "{report}");
    assert_eq!(report["remaining"], 0, "{report}");
    assert_eq!(report["not_linked"][0]["issue"], 12, "{report}");
    assert!(
        report["not_linked"][0]["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("already exists"),
        "{report}"
    );
    assert_eq!(state_of(OutboxKind::Link), vec![OutboxState::Sent]);
    let again = rig
        .flushed(&[])
        .expect("a second flush has nothing to send");
    assert_eq!(again["sent"], 0, "{again}");
}

#[test]
fn a_link_left_sending_that_gh_cannot_make_is_settled_and_never_requeued() {
    let rig = queued_link_rig();
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let row = held
        .outbox(&repo())
        .expect("reads")
        .into_iter()
        .find(|row| row.kind == OutboxKind::Link)
        .expect("a link row");
    held.mark_outbox(row.seq, OutboxState::Sending, None, NOW)
        .expect("marks");
    drop(held);

    let mut answers = not_linked_answers();
    let sidebar = answers.pop().expect("the sidebar answer");
    answers.push(sidebar.clone());
    answers.push(sidebar);
    let report = rig.flushed(&answers).expect("the flush completes");

    assert_eq!(report["sent"], 1, "{report}");
    assert_eq!(state_of(OutboxKind::Link), vec![OutboxState::Sent]);
}

#[cfg(unix)]
fn engine_step(
    rig: &Rig,
    step: impl FnOnce(
        &outbox::flush::Engine,
        &mut Store,
        &OutboxRow,
    ) -> Result<outbox::flush::Step, Failure>,
    sending: bool,
) -> outbox::flush::Step {
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let mut row = held.next_outbox(&repo()).expect("reads").expect("a row");
    if sending {
        held.mark_outbox(row.seq, OutboxState::Sending, None, NOW)
            .expect("marked");
        row = held.next_outbox(&repo()).expect("reads").expect("a row");
    }
    let seconds = crate::harness::session::now_seconds().unwrap_or_default();
    held.take_outbox_lease(&repo(), 1, seconds, seconds + 600, |_| true)
        .expect("taken");
    let runs = rig.directory.path().join("runs");
    let owner = repo();
    let engine = outbox::flush::Engine::new(&rig.context, &runs, &owner, NOW);
    let script = crate::test_env::answers(&[]);
    let bin = crate::test_env::scripted_gh();
    let outcome = crate::test_env::with_logging_gh(bin.path(), &script, &rig.log, || {
        step(&engine, &mut held, &row)
    })
    .expect("a step");
    held.drop_outbox_lease(&repo(), 1).expect("dropped");
    outcome
}

#[cfg(unix)]
#[test]
fn a_flusher_that_lost_its_lease_does_not_mark_the_row_sent() {
    let rig = rig();
    queue_a_note(&rig, "12");

    let outcome = engine_step(&rig, |engine, held, row| engine.sent(held, row), true);

    assert!(matches!(outcome, outbox::flush::Step::Stopped(_)));
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Sending]);
}

#[cfg(unix)]
#[test]
fn a_flusher_that_lost_its_lease_sends_nothing() {
    let rig = rig();
    queue_a_note(&rig, "12");

    let outcome = engine_step(&rig, |engine, held, row| engine.step(held, row), false);

    assert!(matches!(outcome, outbox::flush::Step::Stopped(_)));
    assert_eq!(rig.comment_writes(), Vec::<String>::new());
    assert_eq!(state_of(OutboxKind::Comment), vec![OutboxState::Queued]);
}

#[test]
fn the_comment_readback_does_not_filter_by_the_local_clock() {
    let rig = rig();
    queue_a_note(&rig, "12");
    let mut held = Store::open(&store_path().expect("a path")).expect("opens");
    let row = held.next_outbox(&repo()).expect("reads").expect("a row");
    held.mark_outbox(row.seq, OutboxState::Sending, None, NOW)
        .expect("marked");
    drop(held);

    rig.flushed(&[comments_answer(12, &[row.op_id])])
        .expect("a flush");

    assert!(
        !rig.asked().contains("since="),
        "the readback asked GitHub to filter by the local clock: {}",
        rig.asked()
    );
}

#[test]
fn a_write_the_flush_makes_gives_up_before_the_lease_runs_out() {
    let how = outbox::flush::bounded_write();

    assert!(how.writes);
    let limit = how.timeout.expect("a deadline");
    assert!(limit < std::time::Duration::from_secs(outbox::flush::LEASE_SECONDS));
}

impl Rig {
    fn flushed_labelled(
        &self,
        answers: &[serde_json::Value],
    ) -> Result<serde_json::Value, Failure> {
        let script = crate::test_env::answers(answers);
        let bin = crate::test_env::scripted_gh();
        let runs = self.directory.path().join("runs");
        crate::test_env::with_labelled_logging_gh(
            bin.path(),
            &script,
            self.labels.as_ref().expect("a labelled rig"),
            &self.log,
            || {
                crate::harness::operation_window::with_now_override(Some(noon), || {
                    outbox::flush::run(&self.context, &runs, NOW)
                })
            },
        )
    }
}

#[test]
fn a_queued_handoff_projects_its_label_and_marks_the_row_sent() {
    let rig = labelled_rig();
    rig.closed("handoff", &handoff_flags())
        .expect("a handoff while closed");
    assert_eq!(state_of(OutboxKind::Reconverge), vec![OutboxState::Queued]);

    let report = rig
        .flushed_labelled(&[
            serde_json::json!({
                "matches": "view 12 --json labels",
                "labels": true,
                "status": 0,
            }),
            view(12, "in-progress"),
        ])
        .expect("a flush");
    assert_eq!(report["sent"], 1, "{report}");
    assert!(
        rig.asked().contains("--add-label dev:opencode"),
        "the flush did not project the label: {}",
        rig.asked()
    );
    assert!(
        landed_labels(&rig)
            .iter()
            .any(|label| label == "dev:opencode"),
        "the label did not land"
    );
    assert_eq!(state_of(OutboxKind::Reconverge), vec![OutboxState::Sent]);
}

#[test]
fn a_flushed_handoff_marks_its_store_row_landed() {
    let rig = labelled_rig();
    Store::open(&store_path().expect("a path"))
        .expect("opens")
        .record_handoff(
            &rig.context.repo_dir.display().to_string(),
            12,
            "opencode",
            Some("claude-a"),
            "deferred",
            NOW,
        )
        .expect("the row the tool writes after the transport queues");

    rig.closed("handoff", &handoff_flags())
        .expect("a handoff while closed");
    rig.flushed_labelled(&[
        serde_json::json!({
            "matches": "view 12 --json labels",
            "labels": true,
            "status": 0,
        }),
        view(12, "in-progress"),
    ])
    .expect("a flush");

    let landed = Store::open(&store_path().expect("a path"))
        .expect("opens")
        .handoffs()
        .expect("the handoffs")
        .into_iter()
        .find(|row| row.issue == 12 && row.runtime == "opencode")
        .expect("the handoff row");
    assert_eq!(landed.projection, "landed");
}
