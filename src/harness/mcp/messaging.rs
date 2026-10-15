//! `send_message` and `list_sessions`: the two tools answered from Estigia's own store.
//!
//! The mailbox is [`crate::harness::mailbox`]; this file is the door an agent reaches it
//! through, and the consent that door adds. A person sending from a terminal is never asked;
//! an agent is, and with nobody to ask the send is refused and nothing is recorded. The
//! reasoning is in `docs/sessions.md`.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{NamedRun, ToolFailure};
use crate::harness::{GateContext, mailbox, session};
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
}

/// What the server knows about the person it could ask.
pub(super) struct Consent<'a> {
    pub(super) declared: bool,
    pub(super) grants: &'a mut BTreeSet<String>,
    pub(super) client: &'a mut dyn Elicit,
}

fn text_result(text: String) -> Value {
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

pub(crate) fn holder_unreadable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-holder-unreadable",
        format!("the holder of the issue could not be read from the local store: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the store again; an unreadable holder is not no holder",
        ),
    )
}

pub(crate) fn issue_has_no_holder(issue: u64) -> Refusal {
    Refusal::not_started(
        "message-issue-has-no-holder",
        format!(
            "the local store shows no live holder of #{issue}, so there is no session to address"
        ),
        Resolution::run("estigia sessions"),
    )
}

fn refused(refusal: Refusal) -> ToolFailure {
    ToolFailure::Refused(Box::new(refusal))
}

fn issue_number(to: &str) -> Option<u64> {
    let digits = to.strip_prefix('#').unwrap_or(to);
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

fn holder_of(context: &GateContext, issue: u64) -> Result<String, Refusal> {
    let transport = crate::transport::Context::live(
        context.skill_root.clone(),
        context.repo_dir.clone(),
        context.tracker.named_repo(),
    );
    let repo = crate::transport::claim::repo_of(&transport)
        .map_err(|failure| holder_unreadable(&failure.detail()))?;
    let opened = store::open_for_read(&store::beside(&context.state_root))
        .map_err(|failure| holder_unreadable(&failure.detail()))?
        .ok_or_else(|| issue_has_no_holder(issue))?;
    let comments = opened
        .timeline(&repo, issue)
        .map_err(|failure| holder_unreadable(&failure.detail()))?;
    let now = session::stamp_of(
        session::now_seconds().ok_or_else(|| holder_unreadable("the clock could not be read"))?,
    );
    crate::transport::claim::holding(&comments, &now)
        .holder
        .ok_or_else(|| issue_has_no_holder(issue))
}

fn question(sender: &str, recipient: &str, held: Option<u64>, text: &str) -> String {
    let holds = held.map_or_else(String::new, |issue| format!(", which holds #{issue}"));
    format!(
        "{sender} wants to send a message to {recipient}{holds}:\n\n{text}\n\n\
         The recipient reads it as another session's request, not as yours."
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

pub(super) fn send_message(
    arguments: &Value,
    context: &GateContext,
    named: Option<&NamedRun<'_>>,
    consent: Option<&mut Consent<'_>>,
) -> Result<Value, ToolFailure> {
    let sender = named.map_or("", |run| run.id);
    let field = |name: &str| -> Result<&str, ToolFailure> {
        arguments.get(name).and_then(Value::as_str).ok_or_else(|| {
            ToolFailure::Malformed(format!("`{name}` is a string; send_message needs it"))
        })
    };
    let to = field("to")?.trim();
    let text = field("text")?.trim_end();

    let (recipient, held) = match issue_number(to) {
        Some(issue) => (holder_of(context, issue).map_err(refused)?, Some(issue)),
        None => {
            let held = match session::load(&context.state_root, to).issue {
                Some(Identity::Number(number)) => Some(number),
                _ => None,
            };
            (to.to_owned(), held)
        }
    };

    mailbox::admissible(&context.state_root, &recipient, text).map_err(refused)?;

    let Some(consent) = consent else {
        return Err(refused(needs_a_person(sender)));
    };
    if !consent.grants.contains(sender) {
        if !consent.declared {
            return Err(refused(needs_a_person(sender)));
        }
        let answer = consent.client.elicit(
            &question(sender, &recipient, held, text),
            requested_schema(),
        );
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
            _ => return Err(refused(not_allowed(sender, &recipient))),
        }
    }

    let receipt =
        mailbox::send(&context.state_root, Some(sender), &recipient, text).map_err(refused)?;
    Ok(text_result(
        json!({
            "message": receipt.id,
            "recipient": recipient,
            "sender": sender,
            "recipient_ended": receipt.recipient_ended,
        })
        .to_string(),
    ))
}

pub(super) fn list_sessions(context: &GateContext) -> Result<Value, ToolFailure> {
    let entries = crate::cli::sessions::entries(&context.state_root).map_err(refused)?;
    Ok(text_result(
        json!({
            "sessions": entries,
            "stale_after": crate::cli::sessions::STALE_AFTER_SECONDS,
        })
        .to_string(),
    ))
}
