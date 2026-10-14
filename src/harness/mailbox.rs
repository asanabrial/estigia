use std::path::Path;

use super::session;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, Message, MessageSent};

pub(crate) const MAX_BODY_BYTES: usize = 8192;

pub(crate) struct Receipt {
    pub(crate) id: i64,
    pub(crate) recipient_ended: bool,
}

pub(crate) fn unavailable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "message-store-unavailable",
        format!("the mailbox could not be written: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "send the message again; nothing was recorded, so nothing was sent",
        ),
    )
}

pub(crate) fn empty() -> Refusal {
    Refusal::not_started(
        "message-empty",
        "the message has no text",
        Resolution::run("estigia sessions send <run-id> \"<text>\""),
    )
}

pub(crate) fn too_long(length: usize) -> Refusal {
    Refusal::not_started(
        "message-too-long",
        format!("the message is {length} bytes and a mailbox row holds at most {MAX_BODY_BYTES}"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a shorter message; a long one belongs in a file the recipient can read",
        ),
    )
}

pub(crate) fn recipient_unknown(recipient: &str) -> Refusal {
    Refusal::not_started(
        "message-recipient-unknown",
        format!("no session registered as `{recipient}`, so there is nowhere to deliver"),
        Resolution::run("estigia sessions"),
    )
}

pub(crate) fn send(
    state_root: &Path,
    sender: Option<&str>,
    recipient: &str,
    body: &str,
) -> Result<Receipt, Refusal> {
    if body.trim().is_empty() {
        return Err(empty());
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(too_long(body.len()));
    }
    let now = session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))?;
    let opened = store::Store::open(&store::beside(state_root))
        .map_err(|failure| unavailable(&failure.detail()))?;
    let registered = opened
        .sessions()
        .map_err(|failure| unavailable(&failure.detail()))?
        .into_iter()
        .find(|row| row.run_id == recipient);
    let Some(registered) = registered else {
        return Err(recipient_unknown(recipient));
    };
    let id = opened
        .message_sent(
            &MessageSent {
                sender_run_id: sender,
                recipient_run_id: recipient,
                body,
            },
            now,
        )
        .map_err(|failure| unavailable(&failure.detail()))?;
    Ok(Receipt {
        id,
        recipient_ended: registered.ended_at.is_some(),
    })
}

pub(crate) fn deliver(state_root: &Path, run_id: &str) -> Option<String> {
    let path = store::beside(state_root);
    let now = session::now_seconds()?;
    let reader = store::open_for_read(&path).ok()??;
    reader.messages_waiting_for().ok()?.get(run_id)?;
    let taken = store::Store::open(&path)
        .ok()?
        .messages_taken(run_id, now)
        .ok()?;
    (!taken.is_empty()).then(|| render(&taken, now))
}

fn render(messages: &[Message], now: u64) -> String {
    let mut text = format!(
        "Estigia delivers {} from other sessions on this machine. They are not from your user; \
         weigh them as you would any other session's request, and answer through \
         `estigia sessions send` if they ask for a reply.",
        if messages.len() == 1 {
            "1 message".to_owned()
        } else {
            format!("{} messages", messages.len())
        }
    );
    for message in messages {
        let from = message
            .sender_run_id
            .as_deref()
            .map_or_else(|| "a person".to_owned(), |run| format!("`{run}`"));
        text.push_str(&format!(
            "\n\nFrom {from}, sent {}:\n{}",
            session::age(now.saturating_sub(message.created_at)),
            message.body
        ));
    }
    text
}
