use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::{mailbox, session};
use crate::identity::Identity;
use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::transport::store::{self, SessionRow};

pub(crate) const STALE_AFTER_SECONDS: u64 = 6 * 60 * 60;
pub(crate) const STATUSES: &[&str] = &["live", "stale", "ended", "unregistered", "unknown"];
pub(crate) const DEFAULT_LIMIT: u64 = 50;
pub(crate) const MAX_LIMIT: u64 = 500;
pub(crate) const PAGE_VERSION: u64 = 1;
pub(crate) const CURSOR_TTL_SECONDS: u64 = 15 * 60;
pub(crate) const MAX_CURSOR_BYTES: usize = 4096;

#[derive(Serialize)]
pub(crate) struct Listed {
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

/// The selectors a bounded call filters the snapshot by, and the values a
/// cursor is bound to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Selectors {
    pub(crate) status: Option<String>,
    pub(crate) runtime: Option<String>,
    pub(crate) run: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) repo_dir: Option<String>,
}

impl Selectors {
    fn of(arguments: &Value) -> Self {
        let field = |name: &str| {
            arguments
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        Self {
            status: field("status"),
            runtime: field("runtime"),
            run: field("run"),
            session_id: field("session_id"),
            repo_dir: field("repo_dir"),
        }
    }

    fn matches(&self, entry: &Listed) -> bool {
        let word = |wanted: &Option<String>, found: &str| {
            wanted.as_deref().is_none_or(|wanted| wanted == found)
        };
        let text = |wanted: &Option<String>, found: &Option<String>| {
            wanted
                .as_deref()
                .is_none_or(|wanted| found.as_deref() == Some(wanted))
        };
        word(&self.status, entry.status)
            && word(&self.runtime, &entry.runtime)
            && word(&self.run, &entry.run_id)
            && text(&self.session_id, &entry.session_id)
            && text(&self.repo_dir, &entry.repo_dir)
    }
}

/// The position a page stopped at, and the snapshot, selectors and store it
/// belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Cursor {
    pub(crate) version: u64,
    pub(crate) selectors: Selectors,
    pub(crate) taken_at: u64,
    pub(crate) last_started_at: u64,
    pub(crate) last_run_id: String,
    pub(crate) store: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    crate::paths::hex_digest(hasher.finalize())
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|pair| Some(hex_nibble(pair[0])? << 4 | hex_nibble(pair[1])?))
        .collect()
}

/// The opaque token that names a cursor: the payload in hex, then a digest of
/// it.
pub(crate) fn cursor_token(cursor: &Cursor) -> String {
    let payload = serde_json::to_vec(cursor).unwrap_or_default();
    format!(
        "{}.{}",
        crate::paths::hex_digest(&payload),
        sha256_hex(&payload)
    )
}

fn cursor_of(token: &str) -> Result<Cursor, Refusal> {
    if token.len() > MAX_CURSOR_BYTES {
        return Err(cursor_too_large(token.len()));
    }
    let (payload, digest) = token
        .split_once('.')
        .ok_or_else(|| cursor_malformed("it carries no `.`"))?;
    if digest.contains('.') {
        return Err(cursor_malformed("it carries more than one `.`"));
    }
    let payload =
        unhex(payload).ok_or_else(|| cursor_malformed("its payload is not hexadecimal"))?;
    if sha256_hex(&payload) != digest {
        return Err(cursor_tampered());
    }
    serde_json::from_slice(&payload).map_err(|error| cursor_malformed(&error.to_string()))
}

pub(super) fn cursor_too_large(bytes: usize) -> Refusal {
    Refusal::not_started(
        "sessions-cursor-too-large",
        format!(
            "the session cursor is {bytes} bytes, past the {MAX_CURSOR_BYTES} a cursor can be; \
             nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a cursor exactly as `next_cursor` returned it",
        ),
    )
}

pub(super) fn cursor_malformed(detail: &str) -> Refusal {
    Refusal::not_started(
        "sessions-cursor-malformed",
        format!("the session cursor could not be read: {detail}; nothing was listed"),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a cursor exactly as `next_cursor` returned it, or no cursor at all to start again",
        ),
    )
}

pub(super) fn cursor_tampered() -> Refusal {
    Refusal::not_started(
        "sessions-cursor-tampered",
        "the session cursor does not match its own check, so it was edited after it was issued; \
         nothing was listed",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "a cursor exactly as `next_cursor` returned it, unedited",
        ),
    )
}

pub(super) fn cursor_mismatch(what: &str) -> Refusal {
    Refusal::not_started(
        "sessions-cursor-mismatch",
        format!(
            "the session cursor was issued for other {what}, so it does not continue this call; \
             nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "the selectors and version the cursor was issued with, or no cursor to start again",
        ),
    )
}

pub(super) fn cursor_store_replaced() -> Refusal {
    Refusal::stale(
        "sessions-cursor-store-replaced",
        "the local store has been replaced since the session cursor was issued, so the snapshot \
         it names is gone; nothing was listed",
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "a fresh call without the cursor",
        ),
    )
}

pub(super) fn cursor_expired() -> Refusal {
    Refusal::stale(
        "sessions-cursor-expired",
        format!(
            "the session cursor is older than the {CURSOR_TTL_SECONDS} seconds a cursor stays \
             valid, so its snapshot may have moved; nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "a fresh call without the cursor",
        ),
    )
}

pub(super) fn limit_too_large(limit: u64) -> Refusal {
    Refusal::not_started(
        "sessions-limit-too-large",
        format!(
            "`limit` is {limit} and the most this tool answers is {MAX_LIMIT}; nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!("a `limit` of {MAX_LIMIT} or fewer"),
        ),
    )
}

pub(super) fn limit_invalid(value: &Value) -> Refusal {
    Refusal::not_started(
        "sessions-limit-invalid",
        format!(
            "`limit` is {value} and a page size is a positive whole number of rows, at most \
             {MAX_LIMIT}; nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!(
                "a `limit` between 1 and {MAX_LIMIT}, or no `limit` for the default {DEFAULT_LIMIT}"
            ),
        ),
    )
}

pub(super) fn unregistered_not_bounded() -> Refusal {
    Refusal::not_started(
        "sessions-unregistered-not-bounded",
        "a bounded page's snapshot is the registry rows, which carry the start time a snapshot \
         freezes, and a pointer-only run is not one of them, so `status=unregistered` matches \
         nothing here; the full list — `estigia sessions`, or `list_sessions` with no options — \
         still shows those runs",
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`list_sessions` with no options, or `estigia sessions`, for the pointer-only runs",
        ),
    )
}

fn clock_unreadable() -> Refusal {
    Refusal::unavailable(
        "clock-unreadable",
        "the clock could not be read, so no snapshot of the sessions could be taken",
        Resolution::no_command(NoCommandReason::WorldAction, "a working system clock"),
    )
}

/// The registry rows, keyed by their immutable start, and the pointer-only runs
/// beside them. Only the first carries the creation value a snapshot freezes.
fn registry_and_pointers(
    rows: Vec<SessionRow>,
    holdings: Vec<session::Run>,
    now: Option<u64>,
) -> (Vec<(u64, Listed)>, Vec<Listed>) {
    let mut pointers: std::collections::BTreeMap<String, session::Run> = holdings
        .into_iter()
        .map(|run| (run.run_id.clone(), run))
        .collect();
    let mut registry: Vec<(u64, Listed)> = Vec::new();
    for row in rows {
        let pointer = pointers.remove(&row.run_id);
        let last_seen = pointer
            .as_ref()
            .and_then(|run| run.verified_at)
            .map_or(row.last_seen, |verified| verified.max(row.last_seen));
        registry.push((
            row.started_at,
            Listed {
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
            },
        ));
    }
    let mut unregistered: Vec<Listed> = Vec::new();
    for (run_id, run) in pointers {
        let runtime = match session::run_id_shape(&run_id) {
            session::RunIdShape::Named { runtime } | session::RunIdShape::Nameless { runtime } => {
                runtime.to_owned()
            }
            _ => "unknown".to_owned(),
        };
        unregistered.push(Listed {
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
    (registry, unregistered)
}

fn listed(rows: Vec<SessionRow>, holdings: Vec<session::Run>, now: Option<u64>) -> Vec<Listed> {
    let (mut entries, unregistered) = registry_and_pointers(rows, holdings, now);
    entries.extend(unregistered.into_iter().map(|entry| (0, entry)));
    entries.sort_by(|a, b| {
        b.1.last_seen
            .cmp(&a.1.last_seen)
            .then(a.1.run_id.cmp(&b.1.run_id))
    });
    entries.into_iter().map(|(_, listed)| listed).collect()
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

type Mail = std::collections::BTreeMap<String, u64>;

fn read(root: &Path) -> Result<(Vec<SessionRow>, Mail, Mail), Refusal> {
    match store::open_for_read(&store::beside(root)).map_err(|f| unreadable(&f.detail()))? {
        Some(opened) => Ok((
            opened.sessions().map_err(|f| unreadable(&f.detail()))?,
            opened
                .messages_waiting_for()
                .map_err(|f| unreadable(&f.detail()))?,
            opened
                .messages_undelivered_from()
                .map_err(|f| unreadable(&f.detail()))?,
        )),
        None => Ok(Default::default()),
    }
}

fn mail(entries: &mut [Listed], waiting: &Mail, undelivered: &Mail) {
    for entry in entries {
        entry.waiting = waiting.get(&entry.run_id).copied().unwrap_or_default();
        entry.undelivered_sent = undelivered.get(&entry.run_id).copied().unwrap_or_default();
    }
}

pub(crate) fn entries(root: &Path) -> Result<Vec<Listed>, Refusal> {
    let (rows, waiting, undelivered) = read(root)?;
    let mut entries = listed(rows, session::holdings(root), session::now_seconds());
    mail(&mut entries, &waiting, &undelivered);
    Ok(entries)
}

/// One bounded page of the session registry, or the refusal that says why not.
pub(crate) fn page(arguments: &Value, root: &Path) -> Result<Value, Refusal> {
    let now = session::now_seconds().ok_or_else(clock_unreadable)?;
    let selectors = Selectors::of(arguments);
    if selectors.status.as_deref() == Some("unregistered") {
        return Err(unregistered_not_bounded());
    }
    let limit = match arguments.get("limit") {
        None => DEFAULT_LIMIT,
        Some(value) => {
            let Some(limit) = value.as_u64().filter(|limit| *limit >= 1) else {
                return Err(limit_invalid(value));
            };
            if limit > MAX_LIMIT {
                return Err(limit_too_large(limit));
            }
            limit
        }
    };
    let store_id = store::store_identity(&store::beside(root));
    let (taken_at, after) = match arguments.get("cursor").and_then(Value::as_str) {
        Some(token) => {
            let cursor = cursor_of(token)?;
            if cursor.version != PAGE_VERSION {
                return Err(cursor_mismatch("version"));
            }
            if cursor.selectors != selectors {
                return Err(cursor_mismatch("selectors"));
            }
            if cursor.store != store_id {
                return Err(cursor_store_replaced());
            }
            if now.saturating_sub(cursor.taken_at) > CURSOR_TTL_SECONDS {
                return Err(cursor_expired());
            }
            (
                cursor.taken_at,
                Some((cursor.last_started_at, cursor.last_run_id)),
            )
        }
        None => (now, None),
    };

    let (rows, waiting, undelivered) = read(root)?;
    let (mut snapshot, _) = registry_and_pointers(rows, session::holdings(root), Some(taken_at));
    snapshot.retain(|(started_at, _)| *started_at <= taken_at);
    snapshot.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.run_id.cmp(&b.1.run_id)));
    for (_, entry) in &mut snapshot {
        entry.waiting = waiting.get(&entry.run_id).copied().unwrap_or_default();
        entry.undelivered_sent = undelivered.get(&entry.run_id).copied().unwrap_or_default();
    }

    let matching: Vec<&(u64, Listed)> = snapshot
        .iter()
        .filter(|(_, entry)| selectors.matches(entry))
        .collect();
    let start = after.as_ref().map_or(0, |(started_at, run_id)| {
        matching.partition_point(|entry| {
            (entry.0, entry.1.run_id.as_str()) <= (*started_at, run_id.as_str())
        })
    });
    let page: Vec<&(u64, Listed)> = matching
        .iter()
        .skip(start)
        .take(limit as usize)
        .copied()
        .collect();
    let returned = page.len();
    let more = matching.len() > start + returned;
    let next_cursor = if more {
        page.last().map(|last| {
            cursor_token(&Cursor {
                version: PAGE_VERSION,
                selectors: selectors.clone(),
                taken_at,
                last_started_at: last.0,
                last_run_id: last.1.run_id.clone(),
                store: store_id.clone(),
            })
        })
    } else {
        None
    };
    let sessions: Vec<&Listed> = page.iter().map(|entry| &entry.1).collect();
    Ok(serde_json::json!({
        "version": PAGE_VERSION,
        "taken_at": taken_at,
        "stale_after": STALE_AFTER_SECONDS,
        "limit": limit,
        "returned": returned,
        "matched": matching.len(),
        "more": more,
        "next_cursor": next_cursor,
        "sessions": sessions,
    }))
}

pub(super) fn sessions(json: bool) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let entries = entries(&root)?;
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

pub(super) fn deliver(
    runtime: &str,
    session_id: &str,
    peek: bool,
    through: Option<i64>,
    json: bool,
) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    let run = session::run_id(runtime, session_id);
    let (text, last) = if peek {
        match mailbox::peek(&root, &run) {
            Some((text, last)) => (Some(text), Some(last)),
            None => (None, None),
        }
    } else {
        let text = match through {
            Some(through) => mailbox::deliver_through(&root, &run, through),
            None => mailbox::deliver(&root, &run),
        };
        (text, None)
    };
    if json {
        super::print_json(&serde_json::json!({ "messages": text, "through": last }));
    } else if let Some(text) = text {
        say!("{text}");
    }
    Ok(())
}
