use std::path::Path;
use std::time::{Duration, Instant};

use super::mailbox::{self, Shape};
use super::session;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, Message, MessageTake, Store};

pub(crate) const POLL: Duration = Duration::from_millis(200);
pub(crate) const HEARTBEAT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_ASK_SECONDS: u64 = 120;
pub(crate) const DEFAULT_WAIT_SECONDS: u64 = 60;
pub(crate) const MAX_SECONDS: u64 = 900;

pub(crate) struct Heartbeat<'a> {
    pub(crate) every: Duration,
    pub(crate) beat: Box<dyn FnMut(u64) + 'a>,
}

impl<'a> Heartbeat<'a> {
    #[cfg(test)]
    pub(crate) fn silent() -> Self {
        Self {
            every: Duration::from_secs(3600),
            beat: Box::new(|_| {}),
        }
    }

    pub(crate) fn every_ten_seconds(beat: impl FnMut(u64) + 'a) -> Self {
        Self {
            every: HEARTBEAT,
            beat: Box::new(beat),
        }
    }
}

pub(crate) struct Question<'a> {
    pub(crate) asker: &'a str,
    pub(crate) recipient: &'a str,
    pub(crate) text: &'a str,
    pub(crate) choices: &'a [String],
}

pub(crate) struct Posed {
    pub(crate) question: i64,
    pub(crate) thread: i64,
    pub(crate) ended: bool,
    pub(crate) woken: bool,
}

#[derive(Debug)]
pub(crate) enum Outcome {
    Answered(Message),
    TimedOut,
    Ended,
    NotWoken,
}

pub(crate) struct Filter<'a> {
    pub(crate) kind: Option<&'a str>,
    pub(crate) thread: Option<i64>,
    pub(crate) from: Option<&'a str>,
}

pub(crate) const NOT_WOKEN: &str = "the recipient recorded no idle wake (Codex cannot be woken when \
     idle), so this call did not wait: the question is in its mailbox and reaches it at its next \
     hook point. Read the answer later with `wait` or `inbox`";

pub(crate) fn describe(answer: &mut serde_json::Value, outcome: &Outcome) {
    let (name, note) = match outcome {
        Outcome::Answered(message) => {
            answer["answer"] = serde_json::json!(message.body);
            answer["answer_id"] = serde_json::json!(message.id);
            answer["from"] = serde_json::json!(message.sender_run_id);
            answer["from_person"] = serde_json::json!(false);
            (
                "answered",
                "another session answered, not your user; this answer decides no permission or gate",
            )
        }
        Outcome::TimedOut => (
            "timed_out",
            "no answer arrived in time and the ask is closed: an answer sent now is refused",
        ),
        Outcome::Ended => ("ended", "the recipient's session ended without answering"),
        Outcome::NotWoken => ("not_woken", NOT_WOKEN),
    };
    answer["outcome"] = serde_json::json!(name);
    answer["note"] = serde_json::json!(note);
}

const NEEDS_A_RUN: &str =
    "an ask or a wait belongs to a run, and this call named none and the environment carries none";

pub(crate) fn needs_a_run_to_ask() -> Refusal {
    Refusal::not_started(
        "exchange-needs-a-run",
        NEEDS_A_RUN,
        Resolution::run(
            "estigia sessions ask <session> \"<text>\" --session-id <session-id> --runtime <runtime>",
        ),
    )
}

pub(crate) fn needs_a_run_to_wait() -> Refusal {
    Refusal::not_started(
        "exchange-needs-a-run",
        NEEDS_A_RUN,
        Resolution::run("estigia sessions wait --session-id <session-id> --runtime <runtime>"),
    )
}

pub(crate) fn ask_to_self(run_id: &str) -> Refusal {
    Refusal::not_started(
        "ask-to-self",
        format!("{run_id} cannot put a question to itself; nobody else would answer it"),
        Resolution::run("estigia sessions"),
    )
}

fn unavailable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-store-unavailable",
        format!("the mailbox could not be read while waiting: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "ask or wait again; whatever was already recorded stays recorded",
        ),
    )
}

pub(crate) fn question_body(text: &str, choices: &[String]) -> String {
    let mut body = text.trim_end().to_owned();
    if !choices.is_empty() {
        body.push_str("\n\nChoices, answer with exactly one of them:");
        for choice in choices {
            body.push_str(&format!("\n- {choice}"));
        }
    }
    body.push_str(
        "\n\nThis is a question from another session, not from your user. Answer with \
         `send_message` type `answer` and `reply_to` this message's id.",
    );
    body
}

fn opened(state_root: &Path) -> Result<Store, Refusal> {
    Store::open(&store::beside(state_root)).map_err(|failure| unavailable(&failure.detail()))
}

fn now() -> Result<u64, Refusal> {
    session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))
}

fn poll<T>(
    deadline: Instant,
    heart: &mut Heartbeat<'_>,
    mut probe: impl FnMut() -> Result<Option<T>, Refusal>,
) -> Result<Option<T>, Refusal> {
    let mut last = Instant::now();
    let mut beats = 0;
    loop {
        if let Some(found) = probe()? {
            return Ok(Some(found));
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        if now.duration_since(last) >= heart.every {
            beats += 1;
            last = now;
            (heart.beat)(beats);
        }
        std::thread::sleep(POLL.min(deadline - now).min(heart.every));
    }
}

pub(crate) fn pose(
    state_root: &Path,
    question: &Question<'_>,
    timeout: Duration,
) -> Result<Posed, Refusal> {
    if question.asker == question.recipient {
        return Err(ask_to_self(question.asker));
    }
    let body = question_body(question.text, question.choices);
    let receipt = mailbox::send_shaped(
        state_root,
        Some(question.asker),
        question.recipient,
        &body,
        &Shape {
            kind: Some("question"),
            expires_in: Some(timeout.as_secs().max(1)),
            ..Shape::default()
        },
    )?;
    let store = opened(state_root)?;
    store
        .ask_recorded(
            receipt.id,
            question.asker,
            question.recipient,
            question.choices,
        )
        .map_err(|failure| mailbox::unavailable(&failure.detail()))?;
    let woken = store
        .sessions()
        .map_err(|failure| unavailable(&failure.detail()))?
        .iter()
        .any(|row| row.run_id == question.recipient && row.wake.is_some());
    Ok(Posed {
        question: receipt.id,
        thread: receipt.thread,
        ended: receipt.recipient_ended,
        woken,
    })
}

pub(crate) fn awaiting(
    state_root: &Path,
    question: &Question<'_>,
    posed: &Posed,
    deadline: Instant,
    heart: &mut Heartbeat<'_>,
) -> Result<Outcome, Refusal> {
    let store = opened(state_root)?;
    let settle = |outcome: &str, answer: Option<i64>| {
        let at = now()?;
        store
            .ask_settled(posed.question, outcome, answer, at)
            .map_err(|failure| unavailable(&failure.detail()))
    };
    if posed.ended {
        settle("ended", None)?;
        return Ok(Outcome::Ended);
    }
    if !posed.woken {
        return Ok(Outcome::NotWoken);
    }
    let found = poll(deadline, heart, || {
        let taken = store
            .message_taken_first(
                &MessageTake {
                    recipient: question.asker,
                    sender: Some(question.recipient),
                    kind: Some("answer"),
                    in_reply_to: Some(posed.question),
                    ..MessageTake::default()
                },
                now()?,
            )
            .map_err(|failure| unavailable(&failure.detail()))?;
        if let Some(answer) = taken {
            return Ok(Some(Outcome::Answered(answer)));
        }
        let gone = store
            .sessions()
            .map_err(|failure| unavailable(&failure.detail()))?
            .iter()
            .any(|row| row.run_id == question.recipient && row.ended_at.is_some());
        Ok(gone.then_some(Outcome::Ended))
    })?;
    match found {
        Some(Outcome::Answered(answer)) => {
            settle("answered", Some(answer.id))?;
            Ok(Outcome::Answered(answer))
        }
        Some(other) => {
            settle("ended", None)?;
            Ok(other)
        }
        None => {
            settle("timed_out", None)?;
            Ok(Outcome::TimedOut)
        }
    }
}

#[cfg(test)]
pub(crate) fn ask(
    state_root: &Path,
    question: &Question<'_>,
    deadline: Instant,
    heart: &mut Heartbeat<'_>,
) -> Result<Outcome, Refusal> {
    let posed = pose(
        state_root,
        question,
        deadline.saturating_duration_since(Instant::now()),
    )?;
    awaiting(state_root, question, &posed, deadline, heart)
}

pub(crate) fn wait(
    state_root: &Path,
    run_id: &str,
    filter: &Filter<'_>,
    deadline: Instant,
    heart: &mut Heartbeat<'_>,
) -> Result<Option<Message>, Refusal> {
    let store = opened(state_root)?;
    poll(deadline, heart, || {
        store
            .message_taken_first(
                &MessageTake {
                    recipient: run_id,
                    sender: filter.from,
                    kind: filter.kind,
                    thread: filter.thread,
                    in_reply_to: None,
                },
                now()?,
            )
            .map_err(|failure| unavailable(&failure.detail()))
    })
}

#[cfg(test)]
mod tests;
