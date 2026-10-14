use std::path::Path;

use serde::Serialize;

use crate::harness::{mailbox, session};
use crate::identity::Identity;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, SessionRow};

pub(super) const STALE_AFTER_SECONDS: u64 = 6 * 60 * 60;

#[derive(Serialize)]
struct Listed {
    run_id: String,
    runtime: String,
    session_id: Option<String>,
    status: &'static str,
    issue: Option<Identity>,
    state: Option<String>,
    last_seen: Option<u64>,
    silent_for: Option<u64>,
    repo_dir: Option<String>,
    worktree: Option<String>,
    ended_at: Option<u64>,
    end_reason: Option<String>,
    waiting: u64,
    undelivered_sent: u64,
}

fn status_of(ended: bool, last_seen: Option<u64>, now: Option<u64>) -> &'static str {
    if ended {
        return "ended";
    }
    match (last_seen, now) {
        (Some(seen), Some(now)) if now.saturating_sub(seen) <= STALE_AFTER_SECONDS => "live",
        (Some(_), Some(_)) => "stale",
        _ => "unknown",
    }
}

fn listed(rows: Vec<SessionRow>, holdings: Vec<session::Run>, now: Option<u64>) -> Vec<Listed> {
    let mut pointers: std::collections::BTreeMap<String, session::Run> = holdings
        .into_iter()
        .map(|run| (run.run_id.clone(), run))
        .collect();
    let mut entries: Vec<Listed> = Vec::new();
    for row in rows {
        let pointer = pointers.remove(&row.run_id);
        let last_seen = pointer
            .as_ref()
            .and_then(|run| run.verified_at)
            .map_or(row.last_seen, |verified| verified.max(row.last_seen));
        entries.push(Listed {
            status: status_of(row.ended_at.is_some(), Some(last_seen), now),
            issue: pointer.as_ref().and_then(|run| run.issue.clone()),
            state: pointer.as_ref().and_then(|run| run.state.clone()),
            worktree: pointer
                .as_ref()
                .and_then(|run| run.worktree.as_ref())
                .map(|path| path.display().to_string()),
            last_seen: Some(last_seen),
            silent_for: now.map(|now| now.saturating_sub(last_seen)),
            run_id: row.run_id,
            runtime: row.runtime,
            session_id: Some(row.session_id),
            repo_dir: row.repo_dir,
            ended_at: row.ended_at,
            end_reason: row.end_reason,
            waiting: 0,
            undelivered_sent: 0,
        });
    }
    for (run_id, run) in pointers {
        let runtime = match session::run_id_shape(&run_id) {
            session::RunIdShape::Named { runtime } | session::RunIdShape::Nameless { runtime } => {
                runtime.to_owned()
            }
            _ => "unknown".to_owned(),
        };
        entries.push(Listed {
            run_id,
            runtime,
            session_id: None,
            status: "unregistered",
            issue: run.issue,
            state: run.state,
            last_seen: run.verified_at,
            silent_for: run
                .verified_at
                .zip(now)
                .map(|(seen, now)| now.saturating_sub(seen)),
            repo_dir: run.repo_dir.map(|path| path.display().to_string()),
            worktree: run.worktree.map(|path| path.display().to_string()),
            ended_at: None,
            end_reason: None,
            waiting: 0,
            undelivered_sent: 0,
        });
    }
    entries.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then(a.run_id.cmp(&b.run_id)));
    entries
}

pub(super) fn unreadable(detail: &str) -> Refusal {
    Refusal::unavailable(
        "sessions-store-unreadable",
        format!("the session registry could not be read: {detail}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "read the registry again; a missing list is not an empty one",
        ),
    )
}

pub(super) fn sessions(json: bool) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let (rows, waiting, undelivered) =
        match store::open_for_read(&store::beside(&root)).map_err(|f| unreadable(&f.detail()))? {
            Some(opened) => (
                opened.sessions().map_err(|f| unreadable(&f.detail()))?,
                opened
                    .messages_waiting_for()
                    .map_err(|f| unreadable(&f.detail()))?,
                opened
                    .messages_undelivered_from()
                    .map_err(|f| unreadable(&f.detail()))?,
            ),
            None => Default::default(),
        };
    let mut entries = listed(rows, session::holdings(&root), session::now_seconds());
    for entry in &mut entries {
        entry.waiting = waiting.get(&entry.run_id).copied().unwrap_or_default();
        entry.undelivered_sent = undelivered.get(&entry.run_id).copied().unwrap_or_default();
    }
    if json {
        super::print_json(&serde_json::json!({
            "sessions": entries,
            "stale_after": STALE_AFTER_SECONDS,
        }));
        return Ok(());
    }
    if entries.is_empty() {
        say!("no session has registered on this machine");
    }
    for entry in &entries {
        let held = match (&entry.issue, &entry.state) {
            (Some(Identity::Number(number)), Some(state)) => format!("#{number} {state}"),
            (Some(issue), Some(state)) => format!("{issue} {state}"),
            (Some(Identity::Number(number)), None) => format!("#{number}"),
            (Some(issue), None) => issue.to_string(),
            _ => "-".to_owned(),
        };
        let seen = entry
            .silent_for
            .map_or_else(|| "never seen".to_owned(), session::age);
        let mut mail = String::new();
        if entry.waiting > 0 {
            mail.push_str(&format!("  {} waiting to be delivered", entry.waiting));
        }
        if entry.undelivered_sent > 0 {
            mail.push_str(&format!(
                "  {} sent and not delivered",
                entry.undelivered_sent
            ));
        }
        say!(
            "{}  {}  {}  {}  last seen {}{}",
            entry.run_id,
            entry.status,
            entry.runtime,
            held,
            seen,
            mail
        );
    }
    Ok(())
}

pub(super) fn source_unreadable(path: &Path, error: &str) -> Refusal {
    Refusal::unavailable(
        "message-source-unreadable",
        format!(
            "the message file {} could not be read: {error}",
            path.display()
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a readable text file, or the text given on the command line",
        ),
    )
}

pub(super) fn send(
    recipient: &str,
    text: Option<&str>,
    file: Option<&Path>,
    sender: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let body = match (text, file) {
        (Some(text), _) => text.to_owned(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|error| source_unreadable(path, &error.to_string()))?,
        (None, None) => String::new(),
    };
    let root = session::state_root(None)?;
    let receipt = mailbox::send(&root, sender, recipient, body.trim_end())?;
    if json {
        super::print_json(&serde_json::json!({
            "message": receipt.id,
            "recipient": recipient,
            "sender": sender,
            "recipient_ended": receipt.recipient_ended,
        }));
        return Ok(());
    }
    say!(
        "recorded message {} for {recipient}; it is delivered at that session's next hook point",
        receipt.id
    );
    if receipt.recipient_ended {
        say!(
            "{recipient} has ended, so this message stays undelivered and is listed against your session in `estigia sessions`"
        );
    }
    Ok(())
}

pub(super) fn deliver(runtime: &str, session_id: &str, json: bool) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let text = mailbox::deliver(&root, &session::run_id(runtime, session_id));
    if json {
        super::print_json(&serde_json::json!({ "messages": text }));
    } else if let Some(text) = text {
        say!("{text}");
    }
    Ok(())
}
