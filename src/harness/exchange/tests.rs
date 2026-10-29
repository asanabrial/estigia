use super::*;
use crate::harness::mailbox::{self, Shape};
use crate::transport::store::{self, SessionSeen, Store};

const ALPHA: &str = "claude-alpha0a1b2c3d";
const BRAVO: &str = "opencode-bravo0a1b2c3d";
const CHARLIE: &str = "pi-charlie0a1b2c3d";
const CODEX: &str = "codex-delta0a1b2c3d";
const SHORT: Duration = Duration::from_millis(300);
const LONG: Duration = Duration::from_secs(20);

struct Rig {
    directory: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let rig = Self {
            directory: tempfile::tempdir().expect("a temporary directory"),
        };
        let store = rig.store();
        for (run_id, runtime, wake) in [
            (ALPHA, "claude", Some("stop-waiter")),
            (BRAVO, "opencode", Some("poll")),
            (CHARLIE, "pi", Some("poll")),
            (CODEX, "codex", None),
        ] {
            store
                .session_seen(
                    &SessionSeen {
                        run_id,
                        runtime,
                        session_id: "host-session",
                        repo_dir: None,
                        wake,
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

    fn send(&self, from: &str, to: &str, body: &str, shape: &Shape<'_>) -> Result<i64, Refusal> {
        mailbox::send_shaped(&self.root(), Some(from), to, body, shape).map(|receipt| receipt.id)
    }

    fn ask(&self, to: &str, choices: &[&str], timeout: Duration) -> Outcome {
        let choices: Vec<String> = choices.iter().map(|choice| (*choice).to_owned()).collect();
        ask(
            &self.root(),
            &Question {
                asker: ALPHA,
                recipient: to,
                text: "A or B?",
                choices: &choices,
            },
            Instant::now() + timeout,
            &mut Heartbeat::silent(),
        )
        .expect("the ask is answered")
    }

    fn answer_when_asked(
        &self,
        from: &'static str,
        body: &'static str,
    ) -> std::thread::JoinHandle<()> {
        let root = self.root();
        std::thread::spawn(move || {
            let question = loop {
                let waiting = Store::open(&store::beside(&root))
                    .expect("the store opens")
                    .messages_listed(&store::MessageListing {
                        recipient: Some(from),
                        kind: Some("question"),
                        limit: 1,
                        ..store::MessageListing::default()
                    })
                    .expect("the mailbox reads");
                if let Some(question) = waiting.first() {
                    break question.id;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            mailbox::send_shaped(
                &root,
                Some(from),
                ALPHA,
                body,
                &Shape {
                    kind: Some("answer"),
                    in_reply_to: Some(question),
                    ..Shape::default()
                },
            )
            .expect("the answer is sent");
        })
    }

    fn outcome_of(&self, question: i64) -> Option<String> {
        self.store()
            .ask(question)
            .expect("reads")
            .and_then(|ask| ask.outcome)
    }
}

fn answer_to(question: i64) -> Shape<'static> {
    Shape {
        kind: Some("answer"),
        in_reply_to: Some(question),
        ..Shape::default()
    }
}

fn of_kind(kind: &'static str, thread: Option<i64>) -> Shape<'static> {
    Shape {
        kind: Some(kind),
        thread,
        ..Shape::default()
    }
}

#[test]
fn an_ask_returns_the_choice_another_run_answers() {
    let rig = Rig::new();
    let answering = rig.answer_when_asked(BRAVO, "B");
    let Outcome::Answered(answer) = rig.ask(BRAVO, &["A", "B"], LONG) else {
        panic!("the ask did not return the answer");
    };
    answering.join().expect("the answerer finishes");
    assert_eq!(answer.body, "B");
    assert_eq!(answer.kind, "answer");
    assert_eq!(answer.sender_run_id.as_deref(), Some(BRAVO));
    assert_eq!(answer.state(), "delivered");
    assert_eq!(
        rig.outcome_of(answer.in_reply_to.expect("a parent"))
            .as_deref(),
        Some("answered")
    );
}

#[test]
fn an_ask_nobody_answers_times_out_records_it_and_refuses_the_late_answer() {
    let rig = Rig::new();
    assert!(matches!(
        rig.ask(BRAVO, &["A", "B"], SHORT),
        Outcome::TimedOut
    ));
    let question = rig
        .store()
        .ask(1)
        .expect("reads")
        .expect("the ask was recorded");
    assert_eq!(question.outcome.as_deref(), Some("timed_out"));
    let refused = rig
        .send(BRAVO, ALPHA, "A", &answer_to(1))
        .expect_err("a late answer is refused");
    assert_eq!(refused.code, "message-answer-late");
    let listed = rig.store().messages_listed(&store::MessageListing {
        recipient: Some(ALPHA),
        limit: 10,
        ..store::MessageListing::default()
    });
    assert!(listed.expect("reads").is_empty());
}

#[test]
fn an_answer_outside_the_choices_or_from_another_run_is_refused_and_the_ask_stays_open() {
    let rig = Rig::new();
    let waiting = {
        let root = rig.root();
        std::thread::spawn(move || {
            ask(
                &root,
                &Question {
                    asker: ALPHA,
                    recipient: BRAVO,
                    text: "A or B?",
                    choices: &["A".to_owned(), "B".to_owned()],
                },
                Instant::now() + LONG,
                &mut Heartbeat::silent(),
            )
        })
    };
    while rig.store().ask(1).expect("reads").is_none() {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        rig.send(BRAVO, ALPHA, "C", &answer_to(1))
            .expect_err("not a choice")
            .code,
        "message-answer-not-a-choice"
    );
    assert_eq!(
        rig.send(CHARLIE, ALPHA, "A", &answer_to(1))
            .expect_err("not the addressee")
            .code,
        "message-answer-not-addressed"
    );
    assert_eq!(rig.outcome_of(1), None);
    rig.send(BRAVO, ALPHA, "A", &answer_to(1))
        .expect("a valid choice");
    let Outcome::Answered(answer) = waiting.join().expect("finishes").expect("answered") else {
        panic!("the valid answer did not end the ask");
    };
    assert_eq!(answer.body, "A");
}

#[test]
fn a_recipient_that_ends_while_asked_is_an_outcome_not_a_guess() {
    let rig = Rig::new();
    let ending = {
        let root = rig.root();
        std::thread::spawn(move || {
            let opened = Store::open(&store::beside(&root)).expect("opens");
            while opened.ask(1).expect("reads").is_none() {
                std::thread::sleep(Duration::from_millis(10));
            }
            opened
                .session_ended(BRAVO, Some("exit"), 2_000)
                .expect("ends");
        })
    };
    assert!(matches!(rig.ask(BRAVO, &["A"], LONG), Outcome::Ended));
    ending.join().expect("finishes");
    assert_eq!(rig.outcome_of(1).as_deref(), Some("ended"));
}

#[test]
fn a_recipient_with_no_idle_wake_is_told_so_instead_of_waited_on() {
    let rig = Rig::new();
    let started = Instant::now();
    assert!(matches!(
        rig.ask(CODEX, &["A", "B"], LONG),
        Outcome::NotWoken
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(rig.outcome_of(1), None);
    rig.send(CODEX, ALPHA, "A", &answer_to(1))
        .expect("a later answer is mail");
}

#[test]
fn a_question_cannot_be_put_to_oneself() {
    let rig = Rig::new();
    let refused = ask(
        &rig.root(),
        &Question {
            asker: ALPHA,
            recipient: ALPHA,
            text: "?",
            choices: &[],
        },
        Instant::now() + SHORT,
        &mut Heartbeat::silent(),
    )
    .expect_err("refused");
    assert_eq!(refused.code, "ask-to-self");
}

fn filter<'a>(kind: &'a str, thread: i64) -> Filter<'a> {
    Filter {
        kind: Some(kind),
        thread: Some(thread),
        from: None,
    }
}

#[test]
fn a_wait_returns_exactly_the_first_message_matching_type_and_thread() {
    let rig = Rig::new();
    let first = rig
        .send(BRAVO, ALPHA, "opening", &of_kind("note", None))
        .expect("sent");
    let other = rig
        .send(BRAVO, ALPHA, "elsewhere", &of_kind("done", None))
        .expect("sent");
    let wanted = rig
        .send(BRAVO, ALPHA, "finished", &of_kind("done", Some(first)))
        .expect("sent");
    let later = rig
        .send(BRAVO, ALPHA, "again", &of_kind("done", Some(first)))
        .expect("sent");
    let got = wait(
        &rig.root(),
        ALPHA,
        &filter("done", first),
        Instant::now() + SHORT,
        &mut Heartbeat::silent(),
    )
    .expect("reads")
    .expect("a match");
    assert_eq!((got.id, got.body.as_str()), (wanted, "finished"));
    let next = wait(
        &rig.root(),
        ALPHA,
        &filter("done", first),
        Instant::now() + SHORT,
        &mut Heartbeat::silent(),
    )
    .expect("reads")
    .expect("the second match");
    assert_eq!(next.id, later);
    assert!(
        wait(
            &rig.root(),
            ALPHA,
            &filter("done", first),
            Instant::now() + SHORT,
            &mut Heartbeat::silent()
        )
        .expect("reads")
        .is_none()
    );
    let untouched = rig.store().message(other).expect("reads").expect("kept");
    assert_eq!(untouched.state(), "queued");
}

#[test]
fn a_wait_blocks_until_a_match_arrives_and_beats_while_it_does() {
    let rig = Rig::new();
    let beats = std::cell::Cell::new(0_u64);
    let sender = {
        let root = rig.root();
        let observed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen = observed.clone();
        let handle = std::thread::spawn(move || {
            while !seen.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
            }
            mailbox::send_shaped(
                &root,
                Some(BRAVO),
                ALPHA,
                "ready",
                &of_kind("blocked", None),
            )
            .expect("sent");
        });
        (handle, observed)
    };
    let got = wait(
        &rig.root(),
        ALPHA,
        &Filter {
            kind: Some("blocked"),
            thread: None,
            from: Some(BRAVO),
        },
        Instant::now() + LONG,
        &mut Heartbeat {
            every: Duration::from_millis(40),
            beat: Box::new(|count| {
                beats.set(count);
                if count >= 2 {
                    sender.1.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }),
        },
    )
    .expect("reads")
    .expect("the message arrived");
    sender.0.join().expect("finishes");
    assert_eq!(got.body, "ready");
    assert!(beats.get() >= 2, "{}", beats.get());
}
