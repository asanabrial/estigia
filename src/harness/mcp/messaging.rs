//! `send_message`, `list_sessions`, `feed` and `where`: the tools answered from Estigia's own store.
//!
//! The mailbox is [`crate::harness::mailbox`]; this file is the door an agent reaches it
//! through, and the consent that door adds. A person sending from a terminal is never asked;
//! an agent is, and with nobody to ask the send is refused and nothing is recorded. The
//! reasoning is in `docs/sessions.md`, and the address vocabulary both verbs share is
//! [`super::address`].

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::{NamedRun, Silent, ToolFailure, address};
use crate::cli::sessions::Listed;
use crate::harness::{GateContext, exchange, handoff, inbox, mailbox, session};
use crate::identity::Identity;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store;

pub(super) const ALLOW_ONCE: &str = "allow_once";
pub(super) const ALLOW_SESSION: &str = "allow_session";
pub(super) const DENY: &str = "deny";

/// Why no answer came back from the client.
pub(super) enum Unasked {
    /// Nobody answered before the deadline.
    Unanswered,
    /// The question could not be put or its answer could not be read.
    Unreachable(String),
}

/// Something that can put one question to the person and report the client's answer.
pub(super) trait Elicit {
    /// The `result` of the client's response, `null` when it answered with an error.
    fn elicit(&mut self, message: &str, schema: Value) -> Result<Value, Unasked>;

    /// Records the progress token the current call carried.
    fn announce(&mut self, _token: Option<Value>) {}

    /// Caps how long a question to the person may wait for the rest of this call.
    fn bound(&mut self, _limit: Duration) {}

    /// Tells the client a blocked call is alive, when the call carried a progress token.
    fn beat(&mut self, _count: u64, _message: &str) {}
}

/// What the server knows about the person it could ask.
pub(super) struct Consent<'a> {
    pub(super) declared: bool,
    pub(super) grants: &'a mut BTreeSet<String>,
    pub(super) client: &'a mut dyn Elicit,
}

pub(super) fn text_result(text: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false,
    })
}

pub(crate) fn needs_a_person(sender: &str) -> Refusal {
    Refusal::not_started(
        "message-needs-a-person",
        format!(
            "{sender} asked to send a message and this client declared no way to ask the person, \
             so nothing was recorded"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the person sending it themselves with `estigia sessions send`, or a client that \
             supports elicitation",
        ),
    )
}

pub(crate) fn not_allowed(sender: &str, recipient: &str) -> Refusal {
    Refusal::not_started(
        "message-not-allowed",
        format!(
            "the person did not allow {sender} to send to {recipient}, or did not answer, so \
             nothing was recorded"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the person's own decision; ask them rather than asking again",
        ),
    )
}

pub(crate) fn question_failed(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-question-failed",
        format!(
            "the question could not be put to the person or answered: {detail}; nothing was recorded"
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "a client that is still connected; the person was never asked, so this is not their answer",
        ),
    )
}

pub(crate) fn reply_unknown(id: i64) -> Refusal {
    Refusal::not_started(
        "message-reply-unknown",
        format!(
            "this machine's mailbox holds no message {id}, so there is no sender to answer; \
             nothing was sent"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the id the delivered message carried, or `to` to name a recipient directly",
        ),
    )
}

pub(crate) fn reply_has_no_sender(id: i64) -> Refusal {
    Refusal::not_started(
        "message-reply-has-no-sender",
        format!(
            "message {id} was sent by a person, not by a session, so there is no run to answer; \
             nothing was sent"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`to` to name the recipient the answer belongs to",
        ),
    )
}

pub(crate) fn reply_sender_gone(run_id: &str) -> Refusal {
    Refusal::not_started(
        "message-reply-sender-gone",
        format!(
            "the sender of that message (`{run_id}`) is no longer in this machine's registry, so \
             there is nowhere to deliver an answer; nothing was sent"
        ),
        Resolution::run("estigia sessions"),
    )
}

fn refused(refusal: Refusal) -> ToolFailure {
    ToolFailure::Refused(Box::new(refusal))
}

/// Every `flow dev <n>` instruction a message carries.
///
/// Read from the words, because the instruction is prose the recipient follows
/// rather than a field the sender declares. A phrase quoted as something *not*
/// to run reads the same here; the refusal is what the person answers.
pub(super) fn flow_issued(text: &str) -> Vec<u64> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut found: Vec<u64> = Vec::new();
    for window in words.windows(3) {
        let verb = window[0].trim_start_matches('/');
        if !verb.eq_ignore_ascii_case("flow") || !window[1].eq_ignore_ascii_case("dev") {
            continue;
        }
        let digits = window[2].trim_start_matches('#');
        let end = digits
            .find(|character: char| !character.is_ascii_digit())
            .unwrap_or(digits.len());
        if let Ok(number) = digits[..end].parse::<u64>()
            && !found.contains(&number)
        {
            found.push(number);
        }
    }
    found
}

/// The refusal a message that sends a session after another issue gets.
///
/// A run holds one issue; telling it to work a second would leave two runs
/// believing they own the same session's work. Nothing is recorded, so the
/// sender can address the session that should take it instead.
pub(crate) fn flow_conflict(recipient: &Listed, text: &str) -> Option<Refusal> {
    let held = recipient.issue.clone()?;
    let conflicting: Vec<u64> = flow_issued(text)
        .into_iter()
        .filter(|number| !matches!(&held, Identity::Number(held) if held == number))
        .collect();
    if conflicting.is_empty() {
        return None;
    }
    let named = conflicting
        .iter()
        .map(|number| format!("#{number}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(Refusal::not_started(
        "message-recipient-holds-another-issue",
        format!(
            "{} holds {held} and this message tells it `flow dev` {named}; nothing was sent, \
             because a session that holds one issue cannot be told to work another",
            recipient.line()
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!(
                "a session holding {named}, or that issue released from {} before this one is \
                 handed over",
                recipient.spoken()
            ),
        ),
    ))
}

fn malformed(detail: String) -> ToolFailure {
    ToolFailure::Malformed(detail)
}

fn resolved(context: &GateContext, said: &str, repo_dir: Option<&str>) -> Result<Listed, Refusal> {
    let narrowed = address::resolve_in(&context.state_root, said, repo_dir)?;
    address::one(narrowed, said)
}

fn mailbox_unreadable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-store-unavailable",
        format!("the mailbox could not be read: {detail}"),
        Resolution::no_command(NoCommandReason::WorldAction, "read the mailbox again"),
    )
}

fn reply_recipient(context: &GateContext, id: i64) -> Result<Listed, Refusal> {
    let opened = store::open_for_read(&store::beside(&context.state_root))
        .map_err(|failure| mailbox_unreadable(&failure.detail()))?
        .ok_or_else(|| reply_unknown(id))?;
    let message = opened
        .message(id)
        .map_err(|failure| mailbox_unreadable(&failure.detail()))?
        .ok_or_else(|| reply_unknown(id))?;
    let sender = message
        .sender_run_id
        .ok_or_else(|| reply_has_no_sender(id))?;
    crate::cli::sessions::entries(&context.state_root)?
        .into_iter()
        .find(|entry| entry.run_id == sender)
        .ok_or_else(|| reply_sender_gone(&sender))
}

fn question(sender: &str, row: &Listed, text: &str) -> String {
    format!(
        "{sender} wants to send a message to {}:\n\n{text}\n\n\
         The recipient reads it as another session's request, not as yours.",
        row.line()
    )
}

fn requested_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "choice": {
                "type": "string",
                "title": "Send this message?",
                "enum": [ALLOW_ONCE, ALLOW_SESSION, DENY],
                "enumNames": [
                    "Allow once",
                    "Allow for this session",
                    "Deny",
                ],
            }
        },
        "required": ["choice"],
    })
}

fn choice_of(response: &Value) -> &str {
    if response.get("action").and_then(Value::as_str) != Some("accept") {
        return DENY;
    }
    match response.pointer("/content/choice").and_then(Value::as_str) {
        Some(ALLOW_ONCE) => ALLOW_ONCE,
        Some(ALLOW_SESSION) => ALLOW_SESSION,
        _ => DENY,
    }
}

fn wake_word(wake: Option<&str>) -> &'static str {
    match wake {
        Some("poll") => "the recipient polls for mail while idle",
        Some("stop-waiter") => "the recipient parks a Stop waiter between turns",
        _ => "this session recorded no idle wake, so delivery waits for its next hook point",
    }
}

fn row_json(row: &Listed) -> Value {
    let mut answer = json!({
        "name": row.spoken(),
        "run_id": row.run_id,
        "runtime": row.runtime,
        "repo": row.repo_dir.as_deref().map(crate::cli::sessions::basename),
        "repo_dir": row.repo_dir,
        "issue": row.issue,
        "state": row.state,
        "silent_for": row.silent_for,
        "waiting": row.waiting,
        "session_id": row.session_id,
        "wake": row.wake,
        "build": row.build.as_deref().filter(|build| *build != crate::build_info::VERSION),
    });
    if let Some(pane) = &row.pane {
        answer["pane"] = json!(pane);
    }
    if !row.handoffs.is_empty() {
        answer["handoffs"] = json!(row.handoffs);
    }
    answer
}

fn consented(
    consent: &mut Consent<'_>,
    sender: &str,
    prompt: &str,
    target: &str,
) -> Result<(), ToolFailure> {
    if consent.grants.contains(sender) {
        return Ok(());
    }
    if !consent.declared {
        return Err(refused(needs_a_person(sender)));
    }
    let answer = consent.client.elicit(prompt, requested_schema());
    let answer = match answer {
        Ok(answer) => answer,
        Err(Unasked::Unanswered) => Value::Null,
        Err(Unasked::Unreachable(detail)) => return Err(refused(question_failed(&detail))),
    };
    match choice_of(&answer) {
        ALLOW_ONCE => {}
        ALLOW_SESSION => {
            consent.grants.insert(sender.to_owned());
        }
        _ => return Err(refused(not_allowed(sender, target))),
    }
    Ok(())
}

pub(super) fn send_message(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    let sender = named.map_or("", |run| run.id);
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("`text` is a string; send_message needs it".to_owned()))?
        .trim_end();
    let repo_dir = match arguments.get("repo_dir") {
        None | Some(Value::Null) => None,
        Some(Value::String(dir)) if !dir.trim().is_empty() => Some(dir.as_str()),
        Some(_) => {
            return Err(malformed(
                "`repo_dir` is the directory the recipient must be running in, as a path"
                    .to_owned(),
            ));
        }
    };
    let kind = match arguments.get("type") {
        None | Some(Value::Null) => None,
        Some(Value::String(kind)) => Some(kind.as_str()),
        Some(_) => return Err(malformed("`type` is a string".to_owned())),
    };
    let count = |name: &str| match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .filter(|number| *number >= 1)
            .map(Some)
            .ok_or_else(|| malformed(format!("`{name}` is a whole number from 1"))),
    };
    let mut shape = mailbox::Shape {
        kind,
        thread: count("thread")?,
        in_reply_to: None,
        expires_in: count("expires_in")?.and_then(|seconds| u64::try_from(seconds).ok()),
    };
    if let Some(issue) = count("handoff")? {
        let outcome = match arguments.get("outcome") {
            None | Some(Value::Null) => None,
            Some(Value::String(outcome)) => Some(outcome.as_str()),
            Some(_) => return Err(malformed("`outcome` is a string".to_owned())),
        };
        let completion = handoff::Completion {
            issue: u64::try_from(issue).unwrap_or_default(),
            kind: kind.unwrap_or("note"),
            outcome,
            text,
        };
        return completed(arguments, context, sender, &completion, consent);
    }
    if arguments.get("broadcast").and_then(Value::as_bool) == Some(true) {
        return broadcast(arguments, context, sender, text, repo_dir, &shape, consent);
    }
    let recipient = match (arguments.get("reply_to"), arguments.get("to")) {
        (Some(reply), None) => {
            if repo_dir.is_some() {
                return Err(malformed(
                    "`repo_dir` narrows `to`; a reply is addressed to the sender of the message \
                     being answered, which the caller does not choose"
                        .to_owned(),
                ));
            }
            let id = reply.as_i64().ok_or_else(|| {
                malformed("`reply_to` is the number of the message being answered".to_owned())
            })?;
            shape.in_reply_to = Some(id);
            reply_recipient(context, id).map_err(refused)?
        }
        (None, Some(to)) => {
            let to = to
                .as_str()
                .ok_or_else(|| malformed("`to` is an address string".to_owned()))?;
            let said = address::spoken(to);
            resolved(context, &said, repo_dir).map_err(refused)?
        }
        (Some(_), Some(_)) => {
            return Err(malformed(
                "give `to` or `reply_to`, never both: `reply_to` answers the session that sent a \
                 message, `to` names a recipient"
                    .to_owned(),
            ));
        }
        (None, None) => {
            return Err(malformed(
                "send_message needs `to` (an address) or `reply_to` (the message being answered)"
                    .to_owned(),
            ));
        }
    };
    mailbox::admissible(
        &context.state_root,
        Some(sender),
        &recipient.run_id,
        text,
        &shape,
    )
    .map_err(refused)?;
    if let Some(conflict) = flow_conflict(&recipient, text) {
        return Err(refused(conflict));
    }

    let Some(consent) = consent else {
        return Err(refused(needs_a_person(sender)));
    };
    consented(
        consent,
        sender,
        &question(sender, &recipient, text),
        &recipient.run_id,
    )?;

    let receipt = mailbox::send_shaped(
        &context.state_root,
        Some(sender),
        &recipient.run_id,
        text,
        &shape,
    )
    .map_err(refused)?;
    Ok(text_result(
        json!({
            "message": receipt.id,
            "thread": receipt.thread,
            "type": kind.unwrap_or("note"),
            "to": row_json(&recipient),
            "delivery": "queued for the recipient's next hook point",
            "wake": wake_word(recipient.wake.as_deref()),
            "recipient_ended": receipt.recipient_ended,
        })
        .to_string(),
    ))
}

fn sender_of(context: &GateContext, run_id: &str) -> Result<Listed, Refusal> {
    crate::cli::sessions::entries(&context.state_root)?
        .into_iter()
        .find(|entry| entry.run_id == run_id)
        .ok_or_else(|| reply_sender_gone(run_id))
}

fn completed(
    arguments: &Value,
    context: &GateContext,
    run_id: &str,
    completion: &handoff::Completion<'_>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    if ["to", "reply_to", "broadcast"]
        .iter()
        .any(|name| arguments.get(*name).is_some_and(|value| !value.is_null()))
    {
        return Err(malformed(
            "`handoff` reports to the run that handed the issue over; give neither `to`,              `reply_to` nor `broadcast` with it"
                .to_owned(),
        ));
    }
    let duty = handoff::completable(&context.state_root, run_id, completion).map_err(refused)?;
    let recipient = sender_of(context, &duty.sender).map_err(refused)?;
    let Some(consent) = consent else {
        return Err(refused(needs_a_person(run_id)));
    };
    consented(
        consent,
        run_id,
        &question(run_id, &recipient, completion.text),
        &recipient.run_id,
    )?;
    let reported = handoff::complete(&context.state_root, run_id, completion).map_err(refused)?;
    let (message, mail_failure) = match &reported.mail {
        Ok(id) => (json!(id), Value::Null),
        Err(detail) => (Value::Null, json!(detail)),
    };
    Ok(text_result(
        json!({
            "handoff": completion.issue,
            "outcome": reported.outcome,
            "to": row_json(&recipient),
            "message": message,
            "mail_failure": mail_failure,
        })
        .to_string(),
    ))
}

fn broadcast_question(sender: &str, members: &[Listed], text: &str) -> String {
    format!(
        "{sender} wants to send a message to {} sessions:\n{}\n\n{text}\n\n\
         Each recipient reads it as another session's request, not as yours.",
        members.len(),
        address::Narrowed::lines(members)
    )
}

fn broadcast(
    arguments: &Value,
    context: &GateContext,
    sender: &str,
    text: &str,
    repo_dir: Option<&str>,
    shape: &mailbox::Shape<'_>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    if arguments
        .get("reply_to")
        .is_some_and(|value| !value.is_null())
    {
        return Err(malformed(
            "a broadcast has no single sender to answer; give `to`, not `reply_to`".to_owned(),
        ));
    }
    let to = arguments
        .get("to")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("`to` is the group a broadcast goes to".to_owned()))?;
    let mut narrowed =
        address::group(&context.state_root, &address::spoken(to), repo_dir).map_err(refused)?;
    narrowed.matched.retain(|member| member.run_id != sender);
    if narrowed.matched.is_empty() {
        return Err(refused(address::no_live_session(to, &narrowed)));
    }
    let members = narrowed.matched;
    for member in &members {
        mailbox::admissible(
            &context.state_root,
            Some(sender),
            &member.run_id,
            text,
            shape,
        )
        .map_err(refused)?;
        if let Some(conflict) = flow_conflict(member, text) {
            return Err(refused(conflict));
        }
    }
    let Some(consent) = consent else {
        return Err(refused(needs_a_person(sender)));
    };
    consented(
        consent,
        sender,
        &broadcast_question(sender, &members, text),
        &format!("the {} sessions matching `{to}`", members.len()),
    )?;
    let mut reached = Vec::new();
    let mut failed = Vec::new();
    for member in &members {
        match mailbox::send_shaped(
            &context.state_root,
            Some(sender),
            &member.run_id,
            text,
            shape,
        ) {
            Ok(receipt) => {
                let mut row = row_json(member);
                row["message"] = json!(receipt.id);
                row["thread"] = json!(receipt.thread);
                row["wake"] = json!(wake_word(member.wake.as_deref()));
                reached.push(row);
            }
            Err(refusal) => {
                failed.push(json!({ "run_id": member.run_id, "reason": refusal.code }));
            }
        }
    }
    Ok(text_result(
        json!({
            "broadcast": to,
            "type": shape.kind.unwrap_or("note"),
            "members": members.len(),
            "reached": reached,
            "failed": failed,
            "delivery": "queued for each member's next hook point",
        })
        .to_string(),
    ))
}

pub(super) fn inbox(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
) -> Result<Value, ToolFailure> {
    let run =
        named.ok_or_else(|| malformed("`run_id` is this run's id; inbox needs it".to_owned()))?;
    let text = |name: &str| match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(malformed(format!("`{name}` is a string"))),
    };
    let number = |name: &str| match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .filter(|number| *number >= 1)
            .map(Some)
            .ok_or_else(|| malformed(format!("`{name}` is a whole number from 1"))),
    };
    let request = inbox::Request {
        sent: arguments
            .get("sent")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        kind: text("type")?,
        thread: number("thread")?,
        from: text("from")?,
        state: text("state")?,
        limit: number("limit")?.and_then(|limit| u64::try_from(limit).ok()),
        read: number("read")?.into_iter().collect(),
        ack: number("ack")?.into_iter().collect(),
    };
    inbox::run(&context.state_root, run.id, &request)
        .map(|answer| text_result(answer.to_string()))
        .map_err(refused)
}

fn timeout_of(arguments: &Value, default: u64) -> Result<Duration, ToolFailure> {
    match arguments.get("timeout_seconds") {
        None | Some(Value::Null) => Ok(Duration::from_secs(default)),
        Some(value) => value
            .as_u64()
            .filter(|seconds| (1..=exchange::MAX_SECONDS).contains(seconds))
            .map(Duration::from_secs)
            .ok_or_else(|| {
                malformed(format!(
                    "`timeout_seconds` is a whole number from 1 to {}",
                    exchange::MAX_SECONDS
                ))
            }),
    }
}

fn choices_of(arguments: &Value) -> Result<Vec<String>, ToolFailure> {
    let invalid = || malformed("`choices` is a list of distinct, non-empty strings".to_owned());
    let Some(listed) = arguments.get("choices").filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let mut choices: Vec<String> = Vec::new();
    for choice in listed.as_array().ok_or_else(invalid)? {
        let choice = choice
            .as_str()
            .map(str::trim)
            .filter(|choice| !choice.is_empty());
        let choice = choice.ok_or_else(invalid)?;
        if choices.iter().any(|held| held == choice) {
            return Err(invalid());
        }
        choices.push(choice.to_owned());
    }
    Ok(choices)
}

pub(super) fn ask(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    let asker = named
        .ok_or_else(|| malformed("`run_id` is this run's id; ask needs it".to_owned()))?
        .id;
    let text = arguments
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("`text` is a string; ask needs it".to_owned()))?;
    let to = arguments
        .get("to")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("`to` is an address string; ask needs it".to_owned()))?;
    let choices = choices_of(arguments)?;
    let timeout = timeout_of(arguments, exchange::DEFAULT_ASK_SECONDS)?;
    let started = Instant::now();
    let recipient = resolved(context, &address::spoken(to), None).map_err(refused)?;
    let body = exchange::question_body(text, &choices);
    let shape = mailbox::Shape {
        kind: Some("question"),
        ..mailbox::Shape::default()
    };
    mailbox::admissible(
        &context.state_root,
        Some(asker),
        &recipient.run_id,
        &body,
        &shape,
    )
    .map_err(refused)?;
    if recipient.run_id == asker {
        return Err(refused(exchange::ask_to_self(asker)));
    }
    if let Some(conflict) = flow_conflict(&recipient, text) {
        return Err(refused(conflict));
    }
    let Some(consent) = consent else {
        return Err(refused(needs_a_person(asker)));
    };
    consent.client.bound(timeout);
    consented(
        consent,
        asker,
        &question(asker, &recipient, &body),
        &recipient.run_id,
    )?;

    let question = exchange::Question {
        asker,
        recipient: &recipient.run_id,
        text,
        choices: &choices,
    };
    let remaining = timeout.saturating_sub(started.elapsed());
    let posed = exchange::pose(&context.state_root, &question, remaining).map_err(refused)?;
    let client = &mut *consent.client;
    let waiting = format!("waiting for an answer from {}", recipient.spoken());
    let outcome = exchange::awaiting(
        &context.state_root,
        &question,
        &posed,
        started + timeout,
        &mut exchange::Heartbeat::every_ten_seconds(|count| client.beat(count, &waiting)),
    )
    .map_err(refused)?;
    let mut answer = json!({
        "question": posed.question,
        "thread": posed.thread,
        "to": row_json(&recipient),
        "waited_seconds": started.elapsed().as_secs(),
    });
    exchange::describe(&mut answer, &outcome);
    Ok(text_result(answer.to_string()))
}

pub(super) fn wait(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    let run =
        named.ok_or_else(|| malformed("`run_id` is this run's id; wait needs it".to_owned()))?;
    let text = |name: &str| match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(malformed(format!("`{name}` is a string"))),
    };
    let kind = text("type")?;
    if let Some(kind) = kind.filter(|kind| !inbox::KINDS.contains(kind)) {
        return Err(refused(mailbox::type_unknown(kind)));
    }
    let thread = match arguments.get("thread") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_i64()
                .filter(|number| *number >= 1)
                .ok_or_else(|| malformed("`thread` is a whole number from 1".to_owned()))?,
        ),
    };
    let timeout = timeout_of(arguments, exchange::DEFAULT_WAIT_SECONDS)?;
    let filter = exchange::Filter {
        kind,
        thread,
        from: text("from")?,
    };
    let started = Instant::now();
    let mut silent = Silent;
    let client: &mut dyn Elicit = match consent {
        Some(consent) => &mut *consent.client,
        None => &mut silent,
    };
    let found = exchange::wait(
        &context.state_root,
        run.id,
        &filter,
        started + timeout,
        &mut exchange::Heartbeat::every_ten_seconds(|count| {
            client.beat(count, "waiting for a matching message");
        }),
    )
    .map_err(refused)?;
    let mut answer = match found {
        Some(message) => json!({ "outcome": "message", "message": inbox::shown(&message) }),
        None => json!({ "outcome": "timed_out", "waited_seconds": started.elapsed().as_secs() }),
    };
    if run.runtime == "codex" {
        answer["wake"] = json!(
            "Codex cannot be woken when idle: this wait held the turn open and covers only it; \
             mail that arrives after it returns waits for the next hook point"
        );
    }
    Ok(text_result(answer.to_string()))
}

pub(super) fn list_sessions(
    arguments: &Value,
    context: &GateContext,
) -> Result<Value, ToolFailure> {
    let paged = [
        "status",
        "runtime",
        "run",
        "session_id",
        "repo_dir",
        "limit",
        "cursor",
        "fields",
    ]
    .iter()
    .any(|name| arguments.get(*name).is_some());
    let asked = match arguments.get("address") {
        None | Some(Value::Null) => None,
        Some(Value::String(asked)) => Some(asked.as_str()),
        Some(_) => {
            return Err(malformed(
                "`address` is the string a person would say, such as `opencode@estigia`, `#96` \
                 or `estigia-2`"
                    .to_owned(),
            ));
        }
    };
    if let Some(asked) = asked {
        if paged {
            return Err(malformed(
                "`address` answers the live registry by what a person calls a session; the page \
                 selectors answer the whole registry. Give one or the other."
                    .to_owned(),
            ));
        }
        let narrowed = address::group(&context.state_root, asked, None).map_err(refused)?;
        let closest: Vec<Value> = if narrowed.matched.is_empty() {
            narrowed.closest.iter().map(row_json).collect()
        } else {
            Vec::new()
        };
        return Ok(text_result(
            json!({
                "asked": asked,
                "matched": narrowed.matched.len(),
                "live": narrowed.matched.iter().map(row_json).collect::<Vec<_>>(),
                "closest": closest,
                "live_count": narrowed.live,
            })
            .to_string(),
        ));
    }
    if paged {
        return crate::cli::sessions::page(arguments, &context.state_root, session::now_seconds())
            .map(|page| text_result(page.to_string()))
            .map_err(refused);
    }
    address::around(&context.state_root)
        .map(|answer| text_result(answer.to_string()))
        .map_err(refused)
}

pub(super) fn feed(arguments: &Value, context: &GateContext) -> Result<Value, ToolFailure> {
    let mut selectors = arguments.clone();
    if let Some(fields) = selectors.as_object_mut()
        && let Some(repository) = fields.remove("repository")
    {
        fields.insert("repo".to_owned(), repository);
    }
    crate::cli::feed::page(&selectors, &context.state_root, session::now_seconds())
        .map(|page| text_result(page.to_string()))
        .map_err(refused)
}

pub(super) fn standing(arguments: &Value, context: &GateContext) -> Result<Value, ToolFailure> {
    let reference = arguments
        .get("reference")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("`reference` is a string; where needs it".to_owned()))?;
    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    crate::cli::standing::read(reference, &transport, &context.state_root)
        .map(|answer| text_result(answer.to_string()))
        .map_err(refused)
}
