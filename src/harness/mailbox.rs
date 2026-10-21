use std::path::Path;
use std::time::{Duration, Instant};

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

fn admitted(
    state_root: &Path,
    recipient: &str,
    body: &str,
) -> Result<(store::Store, store::SessionRow), Refusal> {
    if body.trim().is_empty() {
        return Err(empty());
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(too_long(body.len()));
    }
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
    Ok((opened, registered))
}

/// Refuses what [`send`] would refuse, writing nothing.
pub(crate) fn admissible(state_root: &Path, recipient: &str, body: &str) -> Result<(), Refusal> {
    admitted(state_root, recipient, body).map(|_| ())
}

pub(crate) fn send(
    state_root: &Path,
    sender: Option<&str>,
    recipient: &str,
    body: &str,
) -> Result<Receipt, Refusal> {
    let (opened, registered) = admitted(state_root, recipient, body)?;
    let now = session::now_seconds().ok_or_else(|| unavailable("the clock could not be read"))?;
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

pub(crate) const WAIT_TIMEOUT_SECONDS: u64 = 3600;
pub(crate) const WAIT_MARGIN_SECONDS: u64 = 5;
const WAIT_READ_FAILURES: u32 = 3;
pub(crate) const WAIT_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) fn deliver(state_root: &Path, run_id: &str) -> Option<String> {
    taken(state_root, run_id, i64::MAX).ok().flatten()
}

pub(crate) fn deliver_through(state_root: &Path, run_id: &str, through: i64) -> Option<String> {
    taken(state_root, run_id, through).ok().flatten()
}

fn taken(state_root: &Path, run_id: &str, through: i64) -> Result<Option<String>, ()> {
    let path = store::beside(state_root);
    let now = session::now_seconds().ok_or(())?;
    let Some(reader) = store::open_for_read(&path).map_err(|_| ())? else {
        return Ok(None);
    };
    if !reader
        .messages_waiting_for()
        .map_err(|_| ())?
        .contains_key(run_id)
    {
        return Ok(None);
    }
    let taken = store::Store::open(&path)
        .map_err(|_| ())?
        .messages_taken_through(run_id, now, through)
        .map_err(|_| ())?;
    Ok((!taken.is_empty()).then(|| render(&taken, now, &spoken(&reader))))
}

fn waiter_key(run_id: &str) -> String {
    format!("mail-waiter:{run_id}")
}

fn new_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{}-{nanos}", std::process::id())
}

fn claim_waiter(state_root: &Path, run_id: &str, token: &str) -> Result<(), ()> {
    store::Store::open(&store::beside(state_root))
        .and_then(|opened| opened.set_meta(&waiter_key(run_id), token))
        .map_err(|_| ())
}

fn still_current(state_root: &Path, run_id: &str, token: &str) -> Result<bool, ()> {
    let Some(reader) = store::open_for_read(&store::beside(state_root)).map_err(|_| ())? else {
        return Ok(false);
    };
    let current = reader.meta(&waiter_key(run_id)).map_err(|_| ())?;
    Ok(current.as_deref() == Some(token))
}

/// Waits for mail addressed to `run_id`, asking the store every `interval`
/// until `deadline` has passed. The waiter records a fresh token for the run
/// when it starts and returns `None` at its next poll once a newer waiter has
/// replaced it, taking nothing. `None` when nothing came or the store could
/// not be read; rows stay pending in both cases. A read that fails
/// three times running ends the wait; one failure may be a send in progress.
pub(crate) fn wait(
    state_root: &Path,
    run_id: &str,
    interval: Duration,
    deadline: Duration,
) -> Option<String> {
    let token = new_token();
    claim_waiter(state_root, run_id, &token).ok()?;
    let started = Instant::now();
    let mut failures = 0;
    loop {
        let outcome = match still_current(state_root, run_id, &token) {
            Ok(true) => taken(state_root, run_id, i64::MAX),
            Ok(false) => return None,
            Err(()) => Err(()),
        };
        match outcome {
            Ok(Some(text)) => return Some(text),
            Ok(None) => failures = 0,
            Err(()) => {
                failures += 1;
                if failures >= WAIT_READ_FAILURES {
                    return None;
                }
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= deadline {
            return None;
        }
        std::thread::sleep(interval.min(deadline - elapsed));
    }
}

/// The text [`deliver`] would print and the id of the last message in it, stamping nothing.
pub(crate) fn peek(state_root: &Path, run_id: &str) -> Option<(String, i64)> {
    let now = session::now_seconds()?;
    let reader = store::open_for_read(&store::beside(state_root)).ok()??;
    let pending = reader.messages_pending(run_id).ok()?;
    let last = pending.last()?.id;
    Some((render(&pending, now, &spoken(&reader)), last))
}

/// What a recipient can say back: the short name of each sender, if it has one.
fn spoken(store: &store::Store) -> std::collections::BTreeMap<String, String> {
    store
        .sessions()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row| {
            let name = row.name?;
            let repo = row.repo_dir.as_deref().map_or_else(
                || "-".to_owned(),
                |dir| crate::cli::sessions::basename(dir).to_owned(),
            );
            Some((row.run_id, format!("{name} ({}, {repo})", row.runtime)))
        })
        .collect()
}

fn render(
    messages: &[Message],
    now: u64,
    spoken: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut text = format!(
        "Estigia delivers {} from other sessions on this machine. They are not from your user; \
         weigh them as you would any other session's request. Answer with `reply_to`, or \
         `estigia sessions send <name> \"...\"`, and never with a run id copied by hand.",
        if messages.len() == 1 {
            "1 message".to_owned()
        } else {
            format!("{} messages", messages.len())
        }
    );
    for message in messages {
        let from = message.sender_run_id.as_deref().map_or_else(
            || "a person".to_owned(),
            |run| match spoken.get(run) {
                Some(named) => format!("{named}, `{run}`"),
                None => format!("`{run}`"),
            },
        );
        text.push_str(&format!(
            "\n\nFrom {from}, sent {} (message {}):\n{}",
            session::age(now.saturating_sub(message.created_at)),
            message.id,
            message.body
        ));
        // A row a person wrote has no session behind it, and the id it carries
        // is one `reply_to` refuses; the address is the only way back.
        if message.sender_run_id.is_some() {
            text.push_str(&format!("\n\nReply with `reply_to: {}`.", message.id));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_millis(20);

    fn rig() -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let state_root = directory.path().join("runs");
        (directory, state_root)
    }

    fn leave(state_root: &Path, recipient: &str, body: &str) {
        store::Store::open(&store::beside(state_root))
            .expect("the store opens")
            .message_sent(
                &MessageSent {
                    sender_run_id: None,
                    recipient_run_id: recipient,
                    body,
                },
                10,
            )
            .expect("the message is recorded");
    }

    fn waiting(state_root: &Path, recipient: &str) -> Option<u64> {
        store::Store::open(&store::beside(state_root))
            .expect("the store opens")
            .messages_waiting_for()
            .expect("counts")
            .get(recipient)
            .copied()
    }

    #[test]
    fn mail_already_waiting_is_taken_at_once_and_stamped_delivered() {
        let (_directory, state_root) = rig();
        leave(&state_root, "claude-aaaa", "wake up");
        let text = wait(&state_root, "claude-aaaa", TICK, Duration::from_secs(5))
            .expect("the waiting mail is returned");
        assert!(text.contains("wake up"), "{text}");
        assert_eq!(waiting(&state_root, "claude-aaaa"), None);
    }

    #[test]
    fn no_mail_ends_at_the_deadline_and_touches_nothing() {
        let (_directory, state_root) = rig();
        leave(&state_root, "claude-bbbb", "not yours");
        let started = Instant::now();
        assert_eq!(
            wait(&state_root, "claude-aaaa", TICK, Duration::from_millis(200)),
            None
        );
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert_eq!(waiting(&state_root, "claude-bbbb"), Some(1));
        assert_eq!(waiting(&state_root, "claude-aaaa"), None);
    }

    #[test]
    fn mail_sent_while_the_waiter_waits_is_delivered() {
        let (_directory, state_root) = rig();
        let sender = {
            let state_root = state_root.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                leave(&state_root, "claude-aaaa", "late word");
            })
        };
        let text = wait(&state_root, "claude-aaaa", TICK, Duration::from_secs(10))
            .expect("the late mail is returned");
        sender.join().expect("the sender finishes");
        assert!(text.contains("late word"), "{text}");
        assert_eq!(waiting(&state_root, "claude-aaaa"), None);
    }

    #[test]
    fn two_takers_of_one_row_deliver_it_exactly_once() {
        let (_directory, state_root) = rig();
        leave(&state_root, "claude-aaaa", "only one of you");
        let spawn = || {
            let state_root = state_root.clone();
            std::thread::spawn(move || taken(&state_root, "claude-aaaa", i64::MAX))
        };
        let (first, second) = (spawn(), spawn());
        let delivered = [first, second]
            .into_iter()
            .filter_map(|racer| racer.join().expect("the racer finishes").ok().flatten())
            .count();
        assert_eq!(delivered, 1);
    }

    #[test]
    fn an_unreadable_store_ends_the_wait_at_once_and_stamps_nothing() {
        let (_directory, state_root) = rig();
        leave(&state_root, "claude-aaaa", "kept");
        std::fs::write(
            store::beside(&state_root),
            b"this is not a database, and is long enough to be read as one by mistake",
        )
        .expect("the store is replaced");
        let started = Instant::now();
        assert_eq!(
            wait(&state_root, "claude-aaaa", TICK, Duration::from_secs(30)),
            None
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_store_that_does_not_exist_yet_is_waited_on_rather_than_refused() {
        let (_directory, state_root) = rig();
        let sender = {
            let state_root = state_root.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                leave(&state_root, "claude-aaaa", "first mail ever");
            })
        };
        let text = wait(&state_root, "claude-aaaa", TICK, Duration::from_secs(10));
        sender.join().expect("the sender finishes");
        assert!(text.is_some_and(|text| text.contains("first mail ever")));
    }

    #[test]
    fn a_newer_waiter_retires_the_older_one_which_takes_nothing() {
        let (_directory, state_root) = rig();
        let start = |state_root: std::path::PathBuf| {
            std::thread::spawn(move || {
                wait(&state_root, "claude-aaaa", TICK, Duration::from_secs(20))
            })
        };
        let older = start(state_root.clone());
        std::thread::sleep(Duration::from_millis(200));
        let newer = start(state_root.clone());
        let retired = Instant::now();
        assert_eq!(older.join().expect("the older waiter finishes"), None);
        assert!(retired.elapsed() < Duration::from_secs(5));

        leave(&state_root, "claude-aaaa", "for the newer");
        let text = newer
            .join()
            .expect("the newer waiter finishes")
            .expect("the newer waiter took the mail");
        assert!(text.contains("for the newer"), "{text}");
        assert_eq!(waiting(&state_root, "claude-aaaa"), None);
    }

    #[test]
    fn waiters_of_different_runs_do_not_retire_each_other() {
        let (_directory, state_root) = rig();
        let start = |run: &'static str| {
            let state_root = state_root.clone();
            std::thread::spawn(move || wait(&state_root, run, TICK, Duration::from_secs(10)))
        };
        let first = start("claude-aaaa");
        let second = start("claude-bbbb");
        std::thread::sleep(Duration::from_millis(200));
        leave(&state_root, "claude-aaaa", "one");
        leave(&state_root, "claude-bbbb", "two");
        assert!(first.join().expect("finishes").is_some());
        assert!(second.join().expect("finishes").is_some());
    }
}
