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
pub(crate) const MAX_SELECTOR_BYTES: usize = 256;

/// An issue handed to a runtime, as the store recorded it and as a session of
/// that runtime in that repository is shown it.
#[derive(Serialize, Clone, PartialEq, Eq)]
pub(crate) struct Handoff {
    pub(crate) issue: u64,
    pub(crate) from_run_id: Option<String>,
    pub(crate) at: String,
    pub(crate) projection: String,
}

impl Handoff {
    fn of(row: &crate::transport::store::HandoffRow) -> Self {
        Self {
            issue: row.issue,
            from_run_id: row.from_run_id.clone(),
            at: row.created_at.clone(),
            projection: row.projection.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
pub(crate) struct Listed {
    pub(crate) run_id: String,
    pub(crate) name: Option<String>,
    pub(crate) runtime: String,
    pub(crate) session_id: Option<String>,
    pub(crate) status: &'static str,
    pub(crate) issue: Option<Identity>,
    pub(crate) state: Option<String>,
    pub(crate) last_seen: Option<u64>,
    pub(crate) silent_for: Option<u64>,
    pub(crate) repo_dir: Option<String>,
    pub(crate) worktree: Option<String>,
    pub(crate) ended_at: Option<u64>,
    pub(crate) end_reason: Option<String>,
    pub(crate) waiting: u64,
    pub(crate) undelivered_sent: u64,
    pub(crate) build: Option<String>,
    pub(crate) wake: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pane: Option<String>,
    #[serde(skip)]
    pub(crate) host_pid: Option<u64>,
    #[serde(skip)]
    pub(crate) host_name: Option<String>,
    #[serde(skip)]
    pub(crate) binary: Option<String>,
    #[serde(skip)]
    pub(crate) session_file: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) handoffs: Vec<Handoff>,
    #[serde(flatten)]
    pub(crate) reported: Option<Reported>,
}

/// What a run says about itself, or what its task checklist implies, shown only
/// when `fields` asks for it. Never evidence of anything.
#[derive(Serialize, Clone, Default)]
pub(crate) struct Reported {
    #[serde(skip_serializing_if = "Option::is_none")]
    status_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_age: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_state: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    self_reported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_derived: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    derived: Option<bool>,
}

impl Listed {
    /// The word a sender can use for this row: its short name, or its run id
    /// when the row predates names.
    pub(crate) fn spoken(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.run_id)
    }

    /// One line a person can read, and relay, without parsing.
    pub(crate) fn line(&self) -> String {
        let held = match (&self.issue, &self.state) {
            (Some(Identity::Number(number)), Some(state)) => format!("#{number} {state}"),
            (Some(issue), Some(state)) => format!("{issue} {state}"),
            (Some(Identity::Number(number)), None) => format!("#{number}"),
            (Some(issue), None) => issue.to_string(),
            _ => "nothing".to_owned(),
        };
        let repo = self
            .repo_dir
            .as_deref()
            .map_or_else(|| "-".to_owned(), |dir| basename(dir).to_owned());
        let seen = self.silent_for.map_or_else(
            || "never seen".to_owned(),
            |seconds| format!("seen {}", session::age(seconds)),
        );
        let build = self
            .build
            .as_deref()
            .filter(|build| *build != crate::build_info::VERSION)
            .map_or_else(String::new, |build| format!("  on {build}"));
        format!(
            "{} ({}), {} in {}, holds {}, {}{}",
            self.spoken(),
            self.run_id,
            self.runtime,
            repo,
            held,
            seen,
            build
        )
    }
}

pub(crate) fn basename(dir: &str) -> &str {
    Path::new(dir)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(dir)
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

    fn over_the_bound(&self) -> Option<(&'static str, usize)> {
        [
            ("status", &self.status),
            ("runtime", &self.runtime),
            ("run", &self.run),
            ("session_id", &self.session_id),
            ("repo_dir", &self.repo_dir),
        ]
        .into_iter()
        .find_map(|(name, value)| {
            value
                .as_ref()
                .filter(|value| value.len() > MAX_SELECTOR_BYTES)
                .map(|value| (name, value.len()))
        })
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
pub(crate) fn cursor_token<T: Serialize>(cursor: &T) -> String {
    let payload = serde_json::to_vec(cursor).unwrap_or_default();
    format!(
        "{}.{}",
        crate::paths::hex_digest(&payload),
        sha256_hex(&payload)
    )
}

pub(super) fn cursor_of<T: serde::de::DeserializeOwned>(token: &str) -> Result<T, Refusal> {
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
            "`limit` is {limit} and the most a page answers is {MAX_LIMIT}; nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!("a `limit` of {MAX_LIMIT} or fewer"),
        ),
    )
}

pub(super) fn selector_too_large(name: &str, bytes: usize) -> Refusal {
    Refusal::not_started(
        "sessions-selector-too-large",
        format!(
            "`{name}` is {bytes} bytes and a selector is at most {MAX_SELECTOR_BYTES}; nothing was \
             listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            format!("a `{name}` of {MAX_SELECTOR_BYTES} bytes or fewer"),
        ),
    )
}

pub(super) fn fields_unknown(asked: &str) -> Refusal {
    Refusal::not_started(
        "sessions-fields-unknown",
        format!(
            "`fields` is {asked}, and the only field set a session row can add is `status`; \
             nothing was listed"
        ),
        Resolution::no_command(
            NoCommandReason::OperatorKnowledge,
            "`fields` of `status`, or no `fields` at all",
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

pub(super) fn listing_flags_beside_send(flags: &[&str]) -> Refusal {
    let named = flags.join(", ");
    let (given, without) = if flags.len() == 1 {
        ("was", "it")
    } else {
        ("were", "them")
    };
    Refusal::not_started(
        "sessions-send-takes-no-listing-flag",
        format!(
            "{named} {given} given beside `send`, which takes no listing flag, so nothing was \
             sent; the same call without {without} delivers"
        ),
        Resolution::run("estigia sessions send <run-id> \"<text>\""),
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
                name: row.name,
                build: row.build,
                wake: row.wake,
                pane: row.pane,
                host_pid: row.host_pid,
                host_name: row.host_name,
                binary: row.binary,
                session_file: row.session_file,
                handoffs: Vec::new(),
                reported: None,
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
            name: None,
            build: None,
            wake: None,
            pane: None,
            host_pid: None,
            host_name: None,
            binary: None,
            session_file: None,
            handoffs: Vec::new(),
            reported: None,
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
type Handoffs = Vec<store::HandoffRow>;

fn read(root: &Path) -> Result<(Vec<SessionRow>, Mail, Mail, Handoffs), Refusal> {
    match store::open_for_read(&store::beside(root)).map_err(|f| unreadable(&f.detail()))? {
        Some(opened) => Ok((
            opened.sessions().map_err(|f| unreadable(&f.detail()))?,
            opened
                .messages_waiting_for()
                .map_err(|f| unreadable(&f.detail()))?,
            opened
                .messages_undelivered_from()
                .map_err(|f| unreadable(&f.detail()))?,
            opened.handoffs().map_err(|f| unreadable(&f.detail()))?,
        )),
        None => Ok(Default::default()),
    }
}

/// Whether the call asked for the self-reported status, or the refusal for any
/// other value.
pub(crate) fn wants_status(arguments: &Value) -> Result<bool, Refusal> {
    match arguments.get("fields") {
        None | Some(Value::Null) => Ok(false),
        Some(Value::String(asked)) if asked == "status" => Ok(true),
        Some(other) => Err(fields_unknown(&other.to_string())),
    }
}

fn attach_status(entries: &mut [Listed], root: &Path, now: u64) -> Result<(), Refusal> {
    let Some(opened) =
        store::open_for_read(&store::beside(root)).map_err(|f| unreadable(&f.detail()))?
    else {
        return Ok(());
    };
    let latest = opened
        .latest_statuses()
        .map_err(|f| unreadable(&f.detail()))?;
    let open = opened
        .first_open_tasks()
        .map_err(|f| unreadable(&f.detail()))?;
    for entry in entries {
        let ended = entry.status == "ended";
        entry.reported = match latest.get(&entry.run_id) {
            Some(recorded) => {
                let age = now.saturating_sub(recorded.set_at);
                Some(Reported {
                    status_text: Some(recorded.status.clone()),
                    phase: recorded.phase.clone(),
                    status_age: Some(age),
                    status_state: Some(if ended {
                        "ended"
                    } else if age > STALE_AFTER_SECONDS {
                        "stale"
                    } else {
                        "current"
                    }),
                    self_reported: Some(true),
                    ..Reported::default()
                })
            }
            None => match &entry.issue {
                Some(Identity::Number(number)) => open.get(number).map(|(first, total)| Reported {
                    status_derived: Some(format!("task {first} of {total}")),
                    derived: Some(true),
                    status_state: ended.then_some("ended"),
                    ..Reported::default()
                }),
                _ => None,
            },
        };
    }
    Ok(())
}

fn fill(entry: &mut Listed, waiting: &Mail, undelivered: &Mail, handoffs: &Handoffs) {
    entry.waiting = waiting.get(&entry.run_id).copied().unwrap_or_default();
    entry.undelivered_sent = undelivered.get(&entry.run_id).copied().unwrap_or_default();
    entry.handoffs = match entry.repo_dir.as_deref() {
        Some(dir) => handoffs
            .iter()
            .filter(|row| {
                row.runtime == entry.runtime
                    && crate::paths::same_directory(Path::new(&row.repo_dir), Path::new(dir))
            })
            .map(Handoff::of)
            .collect(),
        None => Vec::new(),
    };
}

fn mail(entries: &mut [Listed], waiting: &Mail, undelivered: &Mail, handoffs: &Handoffs) {
    for entry in entries {
        fill(entry, waiting, undelivered, handoffs);
    }
}

pub(crate) fn entries(root: &Path) -> Result<Vec<Listed>, Refusal> {
    let (rows, waiting, undelivered, handoffs) = read(root)?;
    let mut entries = listed(rows, session::holdings(root), session::now_seconds());
    mail(&mut entries, &waiting, &undelivered, &handoffs);
    Ok(entries)
}

pub(super) struct Page {
    version: u64,
    taken_at: u64,
    limit: u64,
    returned: usize,
    matched: usize,
    more: bool,
    next_cursor: Option<String>,
    sessions: Vec<Listed>,
}

impl Page {
    fn answer(&self) -> Value {
        serde_json::json!({
            "version": self.version,
            "taken_at": self.taken_at,
            "stale_after": STALE_AFTER_SECONDS,
            "limit": self.limit,
            "returned": self.returned,
            "matched": self.matched,
            "more": self.more,
            "next_cursor": self.next_cursor,
            "sessions": self.sessions,
        })
    }

    fn footer(&self) -> String {
        match self.next_cursor.as_deref() {
            Some(cursor) => format!(
                "{} of {} row(s) on this page; more remain — the next cursor is {cursor}",
                self.returned, self.matched
            ),
            None => format!(
                "{} of {} row(s) on this page; nothing more remains",
                self.returned, self.matched
            ),
        }
    }
}

/// One bounded page of the session registry, or the refusal that says why not.
pub(crate) fn page(arguments: &Value, root: &Path, now: Option<u64>) -> Result<Value, Refusal> {
    one_page(arguments, root, now).map(|page| page.answer())
}

pub(super) fn one_page(arguments: &Value, root: &Path, now: Option<u64>) -> Result<Page, Refusal> {
    let now = now.ok_or_else(clock_unreadable)?;
    let selectors = Selectors::of(arguments);
    let with_status = wants_status(arguments)?;
    if let Some((name, bytes)) = selectors.over_the_bound() {
        return Err(selector_too_large(name, bytes));
    }
    if selectors.status.as_deref() == Some("unregistered") {
        return Err(unregistered_not_bounded());
    }
    let limit = match arguments.get("limit").and_then(Value::as_u64) {
        Some(limit) if limit > MAX_LIMIT => return Err(limit_too_large(limit)),
        Some(limit) => limit,
        None => DEFAULT_LIMIT,
    };
    let store_id = store::store_identity(&store::beside(root));
    let (taken_at, after) = match arguments.get("cursor").and_then(Value::as_str) {
        Some(token) => {
            let cursor: Cursor = cursor_of(token)?;
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

    let (rows, waiting, undelivered, handoffs) = read(root)?;
    let (mut snapshot, _) = registry_and_pointers(rows, session::holdings(root), Some(taken_at));
    snapshot.retain(|(started_at, _)| *started_at <= taken_at);
    snapshot.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.run_id.cmp(&b.1.run_id)));
    for (_, entry) in &mut snapshot {
        fill(entry, &waiting, &undelivered, &handoffs);
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
    let mut sessions: Vec<Listed> = page.iter().map(|entry| entry.1.clone()).collect();
    if with_status {
        attach_status(&mut sessions, root, now)?;
    }
    Ok(Page {
        version: PAGE_VERSION,
        taken_at,
        limit,
        returned,
        matched: matching.len(),
        more,
        next_cursor,
        sessions,
    })
}

fn row_line(entry: &Listed, all: bool) -> String {
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
    let build = entry
        .build
        .as_deref()
        .filter(|build| *build != crate::build_info::VERSION)
        .map_or_else(String::new, |build| format!("  on {build}"));
    format!(
        "{}  {}  {}  {}  {}  last seen {}{}{}",
        entry.spoken(),
        entry.run_id,
        if all { entry.status } else { "live" },
        entry.runtime,
        held,
        seen,
        mail,
        build
    )
}

fn bounded_listing(
    json: bool,
    all: bool,
    limit: Option<u64>,
    cursor: Option<&str>,
    fields: Option<&str>,
    root: &Path,
) -> Result<(), Refusal> {
    let mut arguments = serde_json::Map::new();
    if let Some(fields) = fields {
        arguments.insert("fields".to_owned(), Value::from(fields));
    }
    if !all {
        arguments.insert("status".to_owned(), Value::from("live"));
    }
    if let Some(limit) = limit {
        arguments.insert("limit".to_owned(), Value::from(limit));
    }
    if let Some(cursor) = cursor {
        arguments.insert("cursor".to_owned(), Value::from(cursor));
    }
    let answer = one_page(&Value::Object(arguments), root, session::now_seconds())?;
    if json {
        say!("{}", answer.answer());
        return Ok(());
    }
    for entry in &answer.sessions {
        say!("{}", row_line(entry, all));
    }
    say!("{}", answer.footer());
    Ok(())
}

pub(super) fn sessions(
    json: bool,
    all: bool,
    limit: Option<u64>,
    cursor: Option<&str>,
    fields: Option<&str>,
) -> Result<(), Refusal> {
    let root = session::state_root(None)?;
    if limit.is_some() || cursor.is_some() {
        return bounded_listing(json, all, limit, cursor, fields, &root);
    }
    let with_status = wants_status(&serde_json::json!({ "fields": fields }))?;
    let mut entries = entries(&root)?;
    if with_status {
        let now = session::now_seconds().ok_or_else(clock_unreadable)?;
        attach_status(&mut entries, &root, now)?;
    }
    let shown: Vec<&Listed> = entries
        .iter()
        .filter(|entry| all || entry.status == "live")
        .collect();
    if json {
        super::print_json(&serde_json::json!({
            "sessions": shown,
            "stale_after": STALE_AFTER_SECONDS,
            "of": entries.len(),
            "all": all,
        }));
        return Ok(());
    }
    if entries.is_empty() {
        say!("no session has registered on this machine");
        return Ok(());
    }
    if shown.is_empty() {
        say!(
            "no session is live on this machine; `estigia sessions --all` lists the {} row(s) the \
             registry holds",
            entries.len()
        );
        return Ok(());
    }
    for entry in &shown {
        say!("{}", row_line(entry, all));
    }
    let behind: Vec<&str> = shown
        .iter()
        .filter(|entry| {
            entry
                .build
                .as_deref()
                .is_some_and(|build| build != crate::build_info::VERSION)
        })
        .map(|entry| entry.spoken())
        .collect();
    if !behind.is_empty() {
        say!(
            "{} registered with another build; this binary is {}. Restart that session, or the \
             MCP server it runs, to update it — a running session keeps the build it started \
             with.",
            behind.join(", "),
            crate::build_info::VERSION
        );
    }
    if !all {
        say!(
            "{} of {} rows are live; `estigia sessions --all` shows the rest",
            shown.len(),
            entries.len()
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
    shape: &mailbox::Shape<'_>,
    json: bool,
) -> Result<(), Refusal> {
    let body = match (text, file) {
        (Some(text), _) => text.to_owned(),
        (None, Some(path)) => std::fs::read_to_string(path)
            .map_err(|error| source_unreadable(path, &error.to_string()))?,
        (None, None) => String::new(),
    };
    let root = session::state_root(None)?;
    let said = crate::harness::mcp::address::spoken(recipient);
    let narrowed = crate::harness::mcp::address::resolve(&root, &said)?;
    let row = crate::harness::mcp::address::one(narrowed, &said)?;
    let body = body.trim_end();
    if let Some(conflict) = crate::harness::mcp::messaging::flow_conflict(&row, body) {
        return Err(conflict);
    }
    let receipt = mailbox::send_shaped(&root, sender, &row.run_id, body, shape)?;
    if json {
        super::print_json(&serde_json::json!({
            "message": receipt.id,
            "thread": receipt.thread,
            "recipient": row.run_id,
            "name": row.spoken(),
            "runtime": row.runtime,
            "repo": row.repo_dir.as_deref().map(basename),
            "sender": sender,
            "recipient_ended": receipt.recipient_ended,
        }));
        return Ok(());
    }
    say!(
        "recorded message {} for {}; it is delivered at that session's next hook point",
        receipt.id,
        row.line()
    );
    if receipt.recipient_ended {
        say!(
            "{} has ended, so this message stays undelivered and is listed against your session \
             in `estigia sessions`",
            row.spoken()
        );
    }
    Ok(())
}

pub(super) fn ask(
    recipient: &str,
    text: &str,
    choices: &[String],
    timeout: Option<u64>,
    asker: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let asker = asker.ok_or_else(crate::harness::exchange::needs_a_run_to_ask)?;
    let root = session::state_root(None)?;
    let said = crate::harness::mcp::address::spoken(recipient);
    let narrowed = crate::harness::mcp::address::resolve(&root, &said)?;
    let row = crate::harness::mcp::address::one(narrowed, &said)?;
    if let Some(conflict) = crate::harness::mcp::messaging::flow_conflict(&row, text) {
        return Err(conflict);
    }
    let timeout = std::time::Duration::from_secs(
        timeout.unwrap_or(crate::harness::exchange::DEFAULT_ASK_SECONDS),
    );
    let started = std::time::Instant::now();
    let question = crate::harness::exchange::Question {
        asker,
        recipient: &row.run_id,
        text,
        choices,
    };
    let posed = crate::harness::exchange::pose(&root, &question, timeout)?;
    let outcome = crate::harness::exchange::awaiting(
        &root,
        &question,
        &posed,
        started + timeout,
        &mut crate::harness::exchange::Heartbeat::every_ten_seconds(|_| {}),
    )?;
    let mut answer = serde_json::json!({
        "question": posed.question,
        "thread": posed.thread,
        "to": row.run_id,
        "waited_seconds": started.elapsed().as_secs(),
    });
    crate::harness::exchange::describe(&mut answer, &outcome);
    if json {
        super::print_json(&answer);
        return Ok(());
    }
    match &outcome {
        crate::harness::exchange::Outcome::Answered(message) => say!("{}", message.body),
        _ => say!(
            "{}: {}",
            answer["outcome"].as_str().unwrap_or_default(),
            answer["note"].as_str().unwrap_or_default()
        ),
    }
    Ok(())
}

pub(super) fn wait(
    filter: &crate::harness::exchange::Filter<'_>,
    timeout: Option<u64>,
    waiter: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let waiter = waiter.ok_or_else(crate::harness::exchange::needs_a_run_to_wait)?;
    let root = session::state_root(None)?;
    let timeout = std::time::Duration::from_secs(
        timeout.unwrap_or(crate::harness::exchange::DEFAULT_WAIT_SECONDS),
    );
    let found = crate::harness::exchange::wait(
        &root,
        waiter,
        filter,
        std::time::Instant::now() + timeout,
        &mut crate::harness::exchange::Heartbeat::every_ten_seconds(|_| {}),
    )?;
    let answer = match &found {
        Some(message) => serde_json::json!({
            "outcome": "message",
            "message": crate::harness::inbox::shown(message),
        }),
        None => serde_json::json!({ "outcome": "timed_out" }),
    };
    if json {
        super::print_json(&answer);
        return Ok(());
    }
    match found {
        Some(message) => say!(
            "{} {} thread {} from {}: {}",
            message.id,
            message.kind,
            message.thread,
            message.sender_run_id.as_deref().unwrap_or("a person"),
            message.body
        ),
        None => say!("timed_out: no matching message arrived"),
    }
    Ok(())
}

pub(super) fn print_inbox(answer: &serde_json::Value) {
    let messages = answer["messages"].as_array().map_or(&[][..], Vec::as_slice);
    if messages.is_empty() {
        say!(
            "no messages in the {} box",
            answer["box"].as_str().unwrap_or("received")
        );
    }
    for message in messages {
        let party = if answer["box"] == "sent" {
            "to"
        } else {
            "from"
        };
        say!(
            "{} {} [{}] thread {} {party} {}: {}",
            message["id"],
            message["type"].as_str().unwrap_or_default(),
            message["state"].as_str().unwrap_or_default(),
            message["thread"],
            message[party].as_str().unwrap_or("a person"),
            message["body"].as_str().unwrap_or_default()
        );
    }
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
