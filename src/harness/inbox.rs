use std::path::Path;

use serde_json::{Value, json};

use super::{mailbox, session};
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, Message, MessageListing};

pub(crate) const KINDS: &[&str] = &[
    "note",
    "question",
    "answer",
    "done",
    "blocked",
    "escalation",
];
pub(crate) const STATES: &[&str] = &["queued", "delivered", "read", "acked"];

#[derive(Default)]
pub(crate) struct Request<'a> {
    pub(crate) sent: bool,
    pub(crate) kind: Option<&'a str>,
    pub(crate) thread: Option<i64>,
    pub(crate) from: Option<&'a str>,
    pub(crate) state: Option<&'a str>,
    pub(crate) limit: Option<u64>,
    pub(crate) read: Vec<i64>,
    pub(crate) ack: Vec<i64>,
}

pub(crate) const DEFAULT_LIMIT: u64 = 50;
pub(crate) const MAX_LIMIT: u64 = 200;

pub(crate) fn state_unknown(state: &str) -> Refusal {
    Refusal::not_started(
        "inbox-state-unknown",
        format!(
            "`{state}` is not a message state; the states are {}",
            STATES.join(", ")
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "one of the listed states",
        ),
    )
}

pub(crate) fn needs_a_run() -> Refusal {
    Refusal::not_started(
        "inbox-needs-a-run",
        "an inbox belongs to a run, and this call named none and the environment carries none",
        Resolution::run("estigia inbox --session-id <session-id> --runtime <runtime>"),
    )
}

pub(crate) fn not_yours(ids: &[i64]) -> Refusal {
    let named = ids
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Refusal::not_started(
        "inbox-message-not-yours",
        format!(
            "message {named} is not in this run's inbox, so it cannot be read or acknowledged \
             by it; nothing was marked"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the ids this run's own `inbox` lists",
        ),
    )
}

fn unavailable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-store-unavailable",
        format!("the inbox could not be read or marked: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "ask again; nothing was marked",
        ),
    )
}

pub(crate) fn shown(message: &Message) -> Value {
    json!({
        "id": message.id,
        "thread": message.thread,
        "type": message.kind,
        "in_reply_to": message.in_reply_to,
        "from": message.sender_run_id,
        "to": message.recipient_run_id,
        "state": message.state(),
        "sent_at": message.created_at,
        "delivered_at": message.delivered_at,
        "read_at": message.read_at,
        "acked_at": message.acked_at,
        "expires_at": message.expires_at,
        "body": message.body,
    })
}

/// Marks what `request` names for `run_id`, then lists. Listing stamps nothing;
/// only `read` and `ack` do, and only on mail addressed to `run_id`.
pub(crate) fn run(
    state_root: &Path,
    run_id: &str,
    request: &Request<'_>,
) -> Result<Value, Refusal> {
    if let Some(kind) = request.kind.filter(|kind| !KINDS.contains(kind)) {
        return Err(mailbox::type_unknown(kind));
    }
    if let Some(state) = request.state.filter(|state| !STATES.contains(state)) {
        return Err(state_unknown(state));
    }
    let path = store::beside(state_root);
    let marking = !request.read.is_empty() || !request.ack.is_empty();
    if marking {
        mark(&path, run_id, request)?;
    }
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let reader = if marking {
        Some(store::Store::open(&path).map_err(|failure| unavailable(&failure.detail()))?)
    } else {
        store::open_for_read(&path).map_err(|failure| unavailable(&failure.detail()))?
    };
    let mut listed = match &reader {
        Some(reader) => reader
            .messages_listed(&MessageListing {
                recipient: (!request.sent).then_some(run_id),
                sender: if request.sent {
                    Some(run_id)
                } else {
                    request.from
                },
                kind: request.kind,
                thread: request.thread,
                state: request.state,
                limit: limit + 1,
            })
            .map_err(|failure| unavailable(&failure.detail()))?,
        None => Vec::new(),
    };
    let more = u64::try_from(listed.len()).is_ok_and(|held| held > limit);
    if more {
        listed.remove(0);
    }
    Ok(json!({
        "run_id": run_id,
        "box": if request.sent { "sent" } else { "received" },
        "read": request.read,
        "acked": request.ack,
        "limit": limit,
        "more": more,
        "messages": listed.iter().map(shown).collect::<Vec<_>>(),
    }))
}

fn mark(path: &Path, run_id: &str, request: &Request<'_>) -> Result<(), Refusal> {
    let opened = store::Store::open(path).map_err(|failure| unavailable(&failure.detail()))?;
    let mut foreign = Vec::new();
    for id in request.read.iter().chain(&request.ack) {
        let owned = opened
            .message(*id)
            .map_err(|failure| unavailable(&failure.detail()))?
            .is_some_and(|message| message.recipient_run_id == run_id);
        if !owned && !foreign.contains(id) {
            foreign.push(*id);
        }
    }
    if !foreign.is_empty() {
        return Err(not_yours(&foreign));
    }
    let now = session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))?;
    opened
        .messages_marked(run_id, &request.read, false, now)
        .and_then(|()| opened.messages_marked(run_id, &request.ack, true, now))
        .map_err(|failure| unavailable(&failure.detail()))
}

#[cfg(test)]
mod tests;
