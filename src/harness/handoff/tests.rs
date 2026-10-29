use super::*;
use crate::harness::inbox;
use crate::transport::store::{SessionSeen, Store};
use serde_json::Value;

const SENDER: &str = "claude-send0a1b2c3d";
const RECIPIENT: &str = "opencode-recv0a1b2c3d";
const OTHER: &str = "opencode-othr0a1b2c3d";

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let rig = Self {
            directory: tempfile::tempdir().expect("a temporary directory"),
        };
        for (run_id, runtime) in [
            (SENDER, "claude"),
            (RECIPIENT, "opencode"),
            (OTHER, "opencode"),
        ] {
            rig.store()
                .session_seen(
                    &SessionSeen {
                        run_id,
                        runtime,
                        session_id: "host-session",
                        repo_dir: None,
                        wake: None,
                        pane: None,
                    },
                    1_000,
                )
                .expect("a session registers");
        }
        rig
    }

    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("runs")
    }

    fn repo(&self) -> std::path::PathBuf {
        self.directory.path().join("repo")
    }

    fn store(&self) -> Store {
        Store::open(&store::beside(&self.root())).expect("the store opens")
    }

    fn handed(&self, due_in: Option<u64>) {
        record(
            &self.root(),
            &self.repo(),
            &Handed {
                issue: 723,
                runtime: "opencode",
                from_run: SENDER,
                projection: "landed",
                at: "2026-10-22T21:00:00Z",
                due_in,
            },
        )
        .expect("the handoff is recorded");
    }

    fn accepted(&self, run_id: &str) -> bool {
        accept(&self.root(), &self.repo(), run_id, 723)
    }

    fn finished(
        &self,
        run_id: &str,
        kind: &str,
        outcome: Option<&str>,
    ) -> Result<Reported, Refusal> {
        complete(
            &self.root(),
            run_id,
            &Completion {
                issue: 723,
                kind,
                outcome,
                text: "merged and verified",
            },
        )
    }

    fn handoffs_seen_by_sender(&self) -> Vec<Value> {
        let seen = inbox::run(
            &self.root(),
            SENDER,
            &inbox::Request {
                sent: true,
                ..inbox::Request::default()
            },
        )
        .expect("the sent box answers");
        seen["handoffs"].as_array().cloned().unwrap_or_default()
    }

    fn mail_to_sender(&self) -> Vec<Value> {
        let seen = inbox::run(&self.root(), SENDER, &inbox::Request::default())
            .expect("the inbox answers");
        seen["messages"].as_array().cloned().unwrap_or_default()
    }
}

#[test]
fn an_accepted_handoff_is_closed_by_exactly_one_done_to_the_sender() {
    let rig = Rig::new();
    rig.handed(None);
    assert!(rig.accepted(RECIPIENT));

    let reported = rig
        .finished(RECIPIENT, "done", Some("succeeded"))
        .expect("the first done is taken");
    assert_eq!(reported.outcome, "succeeded");

    let mail = rig.mail_to_sender();
    assert_eq!(mail.len(), 1, "{mail:?}");
    assert_eq!(mail[0]["type"], "done");
    assert_eq!(mail[0]["from"], RECIPIENT);
    assert!(
        mail[0]["body"]
            .as_str()
            .is_some_and(|body| body.contains("#723") && body.contains("merged and verified")),
        "{mail:?}"
    );
}

#[test]
fn a_second_done_for_the_same_handoff_is_refused_and_sends_nothing() {
    let rig = Rig::new();
    rig.handed(None);
    rig.accepted(RECIPIENT);
    rig.finished(RECIPIENT, "done", Some("failed"))
        .expect("the first done is taken");

    for (kind, outcome) in [("done", Some("succeeded")), ("blocked", None)] {
        let refused = rig
            .finished(RECIPIENT, kind, outcome)
            .expect_err("a settled handoff takes no second word");
        assert_eq!(refused.code, "handoff-already-settled");
    }
    assert_eq!(rig.mail_to_sender().len(), 1);
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "failed");
}

#[test]
fn only_the_run_that_accepted_can_close_the_handoff() {
    let rig = Rig::new();
    rig.handed(None);

    let early = rig
        .finished(RECIPIENT, "done", Some("succeeded"))
        .expect_err("nobody accepted yet");
    assert_eq!(early.code, "handoff-not-accepted");

    assert!(rig.accepted(RECIPIENT));
    assert!(!rig.accepted(OTHER), "a second acceptance takes nothing");
    let other = rig
        .finished(OTHER, "done", Some("succeeded"))
        .expect_err("another run did not accept");
    assert_eq!(other.code, "handoff-not-accepted");
    assert!(rig.mail_to_sender().is_empty());
}

#[test]
fn a_blocked_closes_the_handoff_as_blocked() {
    let rig = Rig::new();
    rig.handed(None);
    rig.accepted(RECIPIENT);
    rig.finished(RECIPIENT, "blocked", None)
        .expect("a blocked is taken");

    assert_eq!(rig.mail_to_sender()[0]["type"], "blocked");
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "blocked");
}

#[test]
fn the_sender_sees_a_handoff_past_its_deadline_as_overdue_until_it_closes() {
    let rig = Rig::new();
    rig.handed(Some(0));
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "overdue");

    rig.accepted(RECIPIENT);
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "overdue");

    rig.finished(RECIPIENT, "done", Some("succeeded"))
        .expect("a late done is still taken");
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "succeeded");
}

#[test]
fn a_handoff_inside_its_deadline_is_not_overdue() {
    let rig = Rig::new();
    rig.handed(Some(3600));
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "offered");
    rig.accepted(RECIPIENT);
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "accepted");
}

#[test]
fn a_recipient_that_ends_without_done_or_blocked_is_reported_to_the_sender() {
    let rig = Rig::new();
    rig.handed(None);
    rig.accepted(RECIPIENT);
    rig.store()
        .session_ended(RECIPIENT, Some("exit"), 2_000)
        .expect("the session ends");

    let mail = rig.mail_to_sender();
    assert_eq!(mail.len(), 1, "{mail:?}");
    assert_eq!(mail[0]["type"], "escalation");
    assert_eq!(mail[0]["from"], RECIPIENT);
    assert!(
        mail[0]["body"]
            .as_str()
            .is_some_and(|body| body.contains("#723")),
        "{mail:?}"
    );
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "abandoned");

    rig.store()
        .session_ended(RECIPIENT, Some("exit"), 3_000)
        .expect("a second end is harmless");
    assert_eq!(rig.mail_to_sender().len(), 1);

    let late = rig
        .finished(RECIPIENT, "done", Some("succeeded"))
        .expect_err("the handoff already ended as abandoned");
    assert_eq!(late.code, "handoff-already-settled");
}

#[test]
fn a_recipient_that_closed_the_handoff_ends_without_a_report() {
    let rig = Rig::new();
    rig.handed(None);
    rig.accepted(RECIPIENT);
    rig.finished(RECIPIENT, "done", Some("succeeded"))
        .expect("done");
    rig.store()
        .session_ended(RECIPIENT, Some("exit"), 2_000)
        .expect("the session ends");

    assert_eq!(rig.mail_to_sender().len(), 1);
}

#[test]
fn a_run_that_never_accepted_ends_without_a_report() {
    let rig = Rig::new();
    rig.handed(None);
    rig.store()
        .session_ended(RECIPIENT, Some("exit"), 2_000)
        .expect("the session ends");

    assert!(rig.mail_to_sender().is_empty());
    assert_eq!(rig.handoffs_seen_by_sender()[0]["status"], "offered");
}
