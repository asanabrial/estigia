use super::*;
use crate::harness::mailbox::{self, Shape};
use crate::transport::store::{self, SessionSeen, Store};

const ALPHA: &str = "claude-alpha0a1b2c3d";
const BRAVO: &str = "codex-bravo0a1b2c3d";

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let rig = Self {
            directory: tempfile::tempdir().expect("a temporary directory"),
        };
        let store = rig.store();
        for (run_id, runtime) in [(ALPHA, "claude"), (BRAVO, "codex")] {
            store
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

    fn store(&self) -> Store {
        Store::open(&store::beside(&self.root())).expect("the store opens")
    }

    fn send(&self, from: &str, to: &str, body: &str, shape: &Shape<'_>) -> i64 {
        mailbox::send_shaped(&self.root(), Some(from), to, body, shape)
            .expect("the message is sent")
            .id
    }

    fn view(&self, run_id: &str, request: &Request<'_>) -> Value {
        run(&self.root(), run_id, request).expect("the inbox answers")
    }

    fn states(&self, run_id: &str, request: &Request<'_>) -> Vec<(i64, String)> {
        self.view(run_id, request)["messages"]
            .as_array()
            .expect("a list of messages")
            .iter()
            .map(|message| {
                (
                    message["id"].as_i64().expect("an id"),
                    message["state"].as_str().expect("a state").to_owned(),
                )
            })
            .collect()
    }
}

fn question<'a>() -> Shape<'a> {
    Shape {
        kind: Some("question"),
        ..Shape::default()
    }
}

#[test]
fn an_untyped_send_reads_as_a_note_that_starts_its_own_thread() {
    let rig = Rig::new();
    let id = rig.send(ALPHA, BRAVO, "plain words", &Shape::default());
    let seen = rig.view(BRAVO, &Request::default());
    let message = &seen["messages"][0];
    assert_eq!(message["id"], id);
    assert_eq!(message["type"], "note");
    assert_eq!(message["thread"], id);
    assert_eq!(message["state"], "queued");
    assert_eq!(message["from"], ALPHA);
    assert_eq!(message["body"], "plain words");
}

#[test]
fn a_row_written_before_the_columns_existed_reads_as_a_note() {
    let rig = Rig::new();
    rusqlite::Connection::open(store::beside(&rig.root()))
        .expect("the database opens")
        .execute(
            "INSERT INTO messages (sender_run_id, recipient_run_id, body, created_at)
             VALUES (NULL, ?1, 'from before', 5)",
            [BRAVO],
        )
        .expect("an old-shaped row is written");
    let seen = rig.view(BRAVO, &Request::default());
    let message = &seen["messages"][0];
    assert_eq!(message["type"], "note");
    assert_eq!(message["thread"], message["id"]);
    assert_eq!(message["in_reply_to"], Value::Null);
}

#[test]
fn a_reply_joins_the_thread_of_the_message_it_answers() {
    let rig = Rig::new();
    let asked = rig.send(ALPHA, BRAVO, "A or B?", &question());
    let answered = rig.send(
        BRAVO,
        ALPHA,
        "A",
        &Shape {
            kind: Some("answer"),
            in_reply_to: Some(asked),
            ..Shape::default()
        },
    );
    let seen = rig.view(
        ALPHA,
        &Request {
            thread: Some(asked),
            ..Request::default()
        },
    );
    assert_eq!(seen["messages"][0]["id"], answered);
    assert_eq!(seen["messages"][0]["thread"], asked);
    assert_eq!(seen["messages"][0]["in_reply_to"], asked);
    assert_eq!(seen["messages"][0]["type"], "answer");
}

#[test]
fn the_list_filters_by_type_thread_and_sender() {
    let rig = Rig::new();
    let first = rig.send(ALPHA, BRAVO, "one", &question());
    let second = rig.send(ALPHA, BRAVO, "two", &Shape::default());
    let third = rig.send(
        ALPHA,
        BRAVO,
        "three",
        &Shape {
            thread: Some(first),
            ..Shape::default()
        },
    );
    let ids = |request: Request<'_>| -> Vec<i64> {
        rig.states(BRAVO, &request)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    };
    assert_eq!(
        ids(Request {
            kind: Some("question"),
            ..Request::default()
        }),
        vec![first]
    );
    assert_eq!(
        ids(Request {
            thread: Some(first),
            ..Request::default()
        }),
        vec![first, third]
    );
    assert_eq!(
        ids(Request {
            from: Some(ALPHA),
            ..Request::default()
        }),
        vec![first, second, third]
    );
    assert!(
        ids(Request {
            from: Some(BRAVO),
            ..Request::default()
        })
        .is_empty()
    );
}

#[test]
fn listing_is_a_peek_that_stamps_nothing_and_leaves_delivery_to_the_hook() {
    let rig = Rig::new();
    let id = rig.send(ALPHA, BRAVO, "still pending", &Shape::default());
    let first = rig.view(BRAVO, &Request::default());
    let second = rig.view(BRAVO, &Request::default());
    assert_eq!(first, second);
    assert_eq!(first["messages"][0]["state"], "queued");
    let (text, last) = mailbox::peek(&rig.root(), BRAVO).expect("the mail is still pending");
    assert!(text.contains("still pending"), "{text}");
    assert_eq!(last, id);
    assert!(mailbox::deliver(&rig.root(), BRAVO).is_some());
}

#[test]
fn the_sender_sees_delivered_then_read_then_acked_and_each_is_the_recipients_act() {
    let rig = Rig::new();
    let id = rig.send(ALPHA, BRAVO, "do the thing", &Shape::default());
    let sent = Request {
        sent: true,
        ..Request::default()
    };
    assert_eq!(rig.states(ALPHA, &sent), vec![(id, "queued".to_owned())]);

    mailbox::deliver(&rig.root(), BRAVO).expect("the hook delivers it");
    assert_eq!(rig.states(ALPHA, &sent), vec![(id, "delivered".to_owned())]);
    assert_eq!(
        rig.states(BRAVO, &Request::default()),
        vec![(id, "delivered".to_owned())]
    );

    rig.view(
        BRAVO,
        &Request {
            read: vec![id],
            ..Request::default()
        },
    );
    assert_eq!(rig.states(ALPHA, &sent), vec![(id, "read".to_owned())]);

    rig.view(
        BRAVO,
        &Request {
            ack: vec![id],
            ..Request::default()
        },
    );
    assert_eq!(rig.states(ALPHA, &sent), vec![(id, "acked".to_owned())]);
}

#[test]
fn acking_mail_the_hook_never_delivered_leaves_it_out_of_the_pending_count() {
    let rig = Rig::new();
    let id = rig.send(ALPHA, BRAVO, "read it here instead", &Shape::default());
    rig.view(
        BRAVO,
        &Request {
            ack: vec![id],
            ..Request::default()
        },
    );
    assert_eq!(
        rig.store()
            .messages_waiting_for()
            .expect("counts")
            .get(BRAVO),
        None
    );
    assert!(mailbox::deliver(&rig.root(), BRAVO).is_none());
}

#[test]
fn reading_is_not_delivering() {
    let rig = Rig::new();
    let id = rig.send(ALPHA, BRAVO, "peeked", &Shape::default());
    rig.view(
        BRAVO,
        &Request {
            read: vec![id],
            ..Request::default()
        },
    );
    assert_eq!(
        rig.store()
            .messages_waiting_for()
            .expect("counts")
            .get(BRAVO),
        Some(&1)
    );
}

#[test]
fn the_state_filter_separates_what_still_needs_an_answer() {
    let rig = Rig::new();
    let kept = rig.send(ALPHA, BRAVO, "open", &Shape::default());
    let done = rig.send(ALPHA, BRAVO, "closed", &Shape::default());
    rig.view(
        BRAVO,
        &Request {
            ack: vec![done],
            ..Request::default()
        },
    );
    assert_eq!(
        rig.states(
            BRAVO,
            &Request {
                state: Some("queued"),
                ..Request::default()
            }
        ),
        vec![(kept, "queued".to_owned())]
    );
    assert_eq!(
        rig.states(
            BRAVO,
            &Request {
                state: Some("acked"),
                ..Request::default()
            }
        ),
        vec![(done, "acked".to_owned())]
    );
}

#[test]
fn a_run_cannot_read_or_ack_mail_addressed_to_another_and_nothing_is_marked() {
    let rig = Rig::new();
    let mine = rig.send(BRAVO, ALPHA, "to alpha", &Shape::default());
    let theirs = rig.send(ALPHA, BRAVO, "to bravo", &Shape::default());
    let refusal = run(
        &rig.root(),
        ALPHA,
        &Request {
            ack: vec![mine, theirs],
            ..Request::default()
        },
    )
    .expect_err("another run's mail is refused");
    assert_eq!(refusal.code, "inbox-message-not-yours");
    assert_eq!(
        rig.states(ALPHA, &Request::default()),
        vec![(mine, "queued".to_owned())]
    );
}

#[test]
fn an_unknown_type_or_thread_is_refused_before_anything_is_recorded() {
    let rig = Rig::new();
    let kind = mailbox::send_shaped(
        &rig.root(),
        Some(ALPHA),
        BRAVO,
        "x",
        &Shape {
            kind: Some("gossip"),
            ..Shape::default()
        },
    )
    .err()
    .expect("an unknown type is refused");
    assert_eq!(kind.code, "message-type-unknown");
    let thread = mailbox::send_shaped(
        &rig.root(),
        Some(ALPHA),
        BRAVO,
        "x",
        &Shape {
            thread: Some(99),
            ..Shape::default()
        },
    )
    .err()
    .expect("an unknown thread is refused");
    assert_eq!(thread.code, "message-thread-unknown");
    assert!(rig.states(BRAVO, &Request::default()).is_empty());
}

#[test]
fn an_expiry_is_recorded_and_shown_but_decides_nothing() {
    let rig = Rig::new();
    let id = rig.send(
        ALPHA,
        BRAVO,
        "stale soon",
        &Shape {
            expires_in: Some(60),
            ..Shape::default()
        },
    );
    let seen = rig.view(BRAVO, &Request::default());
    let message = &seen["messages"][0];
    assert_eq!(message["id"], id);
    let sent_at = message["sent_at"].as_u64().expect("a send time");
    assert_eq!(message["expires_at"], sent_at + 60);
}

#[test]
fn the_delivered_text_names_a_type_and_a_thread_only_when_there_is_something_to_name() {
    let rig = Rig::new();
    let asked = rig.send(ALPHA, BRAVO, "A or B?", &question());
    rig.send(
        ALPHA,
        BRAVO,
        "and also",
        &Shape {
            thread: Some(asked),
            ..Shape::default()
        },
    );
    rig.send(ALPHA, BRAVO, "plain", &Shape::default());
    let text = mailbox::deliver(&rig.root(), BRAVO).expect("delivered");
    assert!(
        text.contains(&format!("(message {asked}, question)")),
        "{text}"
    );
    assert!(text.contains(&format!(", thread {asked})")), "{text}");
    assert!(text.contains("A or B?") && text.contains("plain"), "{text}");
}
