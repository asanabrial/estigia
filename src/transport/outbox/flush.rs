//! Publishes the queue, oldest first, and stops at the first write it cannot
//! read back.
//!
//! The queue is one line: a write that stalls holds every write behind it,
//! because a later write may name the number an earlier one is still to be
//! given. A write is marked `sending` before it is sent and `sent` only after
//! its own readback, so a crash between the two leaves a row the next flush
//! resolves by reading, not by sending again.

use std::path::Path;

use crate::transport::store::{self, OutboxKind, OutboxRow, OutboxState, Repo, Store};
use crate::transport::window::{self, Mode};
use crate::transport::{Context, Failure, How, branch, claim, commands};

pub(super) const LEASE_SECONDS: u64 = 600;
const WRITE_SECONDS: u64 = 300;
const _: () = assert!(WRITE_SECONDS < LEASE_SECONDS);
const COMMENT_PAGE: usize = 100;
const COMMENT_PAGES: usize = 10;

const WRITES_TO_GITHUB: &[&str] = &[
    "ensure-states",
    "comment",
    "create",
    "transition",
    "claim",
    "unassign",
    "start-branch",
    "publish-review",
    "republish-review",
    "release-ci",
    "draft-pr",
    "handoff",
];

/// Whether `operation` writes to GitHub or publishes, so that a waiting queue
/// goes first.
pub(crate) fn writes_to_github(operation: &str) -> bool {
    WRITES_TO_GITHUB.contains(&operation)
}

/// Starts `executable flush` in its own process group with no terminal and
/// returns its process id without waiting for it. The lease in the store keeps
/// two flushers apart.
pub(crate) fn detach(executable: &Path) -> std::io::Result<u32> {
    let mut command = std::process::Command::new(executable);
    command
        .arg("flush")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    command.spawn().map(|child| child.id())
}

/// A write that gives up well inside the flush lease, so that a hung `gh` is
/// reported as an ambiguous write rather than outliving the lease it was made
/// under.
pub(crate) fn bounded_write() -> How {
    How::write_within(std::time::Duration::from_secs(WRITE_SECONDS))
}

/// What one queued write came to.
pub(super) enum Step {
    Sent,
    NotLinked(String),
    Stopped(String),
}

/// How a failed attempt leaves the row it was made for.
enum Fate {
    /// Nothing reached GitHub: the row goes back to the queue.
    Requeue,
    /// The write may have landed: the row stays `sending` until it is read.
    Unknown,
}

fn fate_of(failure: &Failure) -> Fate {
    match failure {
        Failure::Read(_) | Failure::RateLimited(_) | Failure::ConfigDefect(_) => Fate::Requeue,
        Failure::Write(_) | Failure::Timeout(_) => Fate::Unknown,
        Failure::Stop(payload) => {
            if payload.get("world").and_then(serde_json::Value::as_str) == Some("committed") {
                Fate::Unknown
            } else {
                Fate::Requeue
            }
        }
    }
}

fn describe(failure: &Failure) -> String {
    match failure {
        Failure::Stop(payload) | Failure::ConfigDefect(payload) => {
            let field = |name: &str| payload.get(name).and_then(serde_json::Value::as_str);
            let mut text = failure.detail();
            if let Some(more) = field("detail").or_else(|| field("message")) {
                text.push_str(": ");
                text.push_str(more);
            }
            text
        }
        other => other.detail(),
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    let reached = unsafe { libc::kill(pid, 0) } == 0;
    reached || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    true
}

fn marker_of(op_id: &str) -> String {
    format!("<!-- estigia-outbox:{op_id} -->")
}

fn pending(
    sent: usize,
    remaining: usize,
    stopped: Option<(&OutboxRow, &str)>,
    why: &str,
) -> Failure {
    let mut payload = serde_json::json!({
        "ok": false,
        "reason": "outbox-pending",
        "sent": sent,
        "remaining": remaining,
        "stopped_at": null,
        "detail": why,
        "action": "settle the write named in `stopped_at`, then run the flush again; nothing behind \
                   it is sent while it is unresolved",
    });
    if let Some((row, detail)) = stopped {
        payload["stopped_at"] = serde_json::json!({
            "seq": row.seq,
            "kind": row.kind.as_str(),
            "issue": row.issue,
            "detail": detail,
        });
    }
    if sent > 0 {
        payload["world"] = serde_json::json!("committed");
    }
    Failure::Stop(payload)
}

/// Publishes every unsent write for the repository in order.
///
/// Refuses while the git operation window is closed, and while another flush
/// holds the lease. Answers how many writes it sent; when a write stops it,
/// refuses `outbox-pending` naming that write and how many remain.
pub(crate) fn run(
    context: &Context,
    state_root: &Path,
    now: &str,
) -> Result<serde_json::Value, Failure> {
    if let Mode::Closed(window) = window::state(context) {
        return Err(branch::schedule_denial(&window));
    }
    let repo = claim::repo_of(context)?;
    let mut held = Store::open(&store::store_path()?)?;
    let pid = std::process::id();
    let seconds = crate::harness::session::now_seconds().unwrap_or_default();
    if let Some(other) =
        held.take_outbox_lease(&repo, pid, seconds, seconds + LEASE_SECONDS, process_alive)?
    {
        let remaining = unsent(&held, &repo)?;
        return Err(pending(
            0,
            remaining,
            None,
            &format!("another flush (process {other}) holds the queue"),
        ));
    }
    let outcome = drain(context, state_root, now, &mut held, &repo);
    let _ = held.drop_outbox_lease(&repo, pid);
    outcome
}

fn unsent(held: &Store, repo: &Repo) -> Result<usize, Failure> {
    Ok(held
        .outbox(repo)?
        .iter()
        .filter(|row| row.state != OutboxState::Sent)
        .count())
}

fn drain(
    context: &Context,
    state_root: &Path,
    now: &str,
    held: &mut Store,
    repo: &Repo,
) -> Result<serde_json::Value, Failure> {
    super::rekey_files(held, state_root)?;
    let engine = Engine::new(context, state_root, repo, now);
    let pid = std::process::id();
    let mut sent = 0;
    let mut not_linked = Vec::new();
    loop {
        let seconds = crate::harness::session::now_seconds().unwrap_or_default();
        if let Some(other) =
            held.take_outbox_lease(repo, pid, seconds, seconds + LEASE_SECONDS, process_alive)?
        {
            let remaining = unsent(held, repo)?;
            return Err(pending(
                sent,
                remaining,
                None,
                &format!("the flush lease passed to process {other}"),
            ));
        }
        let Some(row) = held.next_outbox(repo)? else {
            break;
        };
        match engine.step(held, &row)? {
            Step::Sent => sent += 1,
            Step::NotLinked(detail) => {
                sent += 1;
                not_linked.push(serde_json::json!({
                    "seq": row.seq,
                    "issue": row.issue,
                    "detail": detail,
                }));
            }
            Step::Stopped(detail) => {
                let remaining = unsent(held, repo)?;
                return Err(pending(sent, remaining, Some((&row, &detail)), &detail));
            }
        }
    }
    Ok(serde_json::json!({
        "ok": true,
        "sent": sent,
        "remaining": 0,
        "stopped_at": null,
        "not_linked": not_linked,
    }))
}

/// The queue as it stands, without sending any of it.
pub(crate) fn status(context: &Context) -> Result<serde_json::Value, Failure> {
    let closed = matches!(window::state(context), Mode::Closed(_));
    let window = if closed { "closed" } else { "open" };
    let Some(held) = store::open_for_read(&store::store_path()?)? else {
        return Ok(serde_json::json!({
            "ok": true, "window": window, "remaining": 0, "writes": [],
        }));
    };
    if held.unsent_outbox()? == 0 {
        return Ok(serde_json::json!({
            "ok": true, "window": window, "remaining": 0, "writes": [],
        }));
    }
    let repo = claim::repo_of(context)?;
    let writes: Vec<serde_json::Value> = held
        .outbox(&repo)?
        .into_iter()
        .filter(|row| row.state != OutboxState::Sent)
        .map(|row| {
            serde_json::json!({
                "seq": row.seq,
                "kind": row.kind.as_str(),
                "issue": row.issue,
                "state": row.state.as_str(),
                "enqueued_at": row.enqueued_at,
                "detail": row.detail,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "ok": true,
        "window": window,
        "remaining": writes.len(),
        "writes": writes,
    }))
}

/// Runs the queue before an operation that writes to GitHub, when the window is
/// open and a write is waiting. Answers `Ok` when nothing is left behind.
pub(crate) fn before_writing(context: &Context, now: &str) -> Result<(), Failure> {
    let waiting = match store::open_for_read(&store::store_path()?)? {
        Some(held) if held.unsent_outbox()? > 0 => {
            held.next_outbox(&claim::repo_of(context)?)?.is_some()
        }
        _ => false,
    };
    if !waiting {
        return Ok(());
    }
    let state_root = crate::harness::session::state_root(None)
        .map_err(|refusal| Failure::Read(refusal.message))?;
    run(context, &state_root, now).map(|_| ())
}

pub(super) struct Engine<'a> {
    context: &'a Context,
    state_root: &'a Path,
    repo: &'a Repo,
    now: &'a str,
}

impl<'a> Engine<'a> {
    pub(super) fn new(
        context: &'a Context,
        state_root: &'a Path,
        repo: &'a Repo,
        now: &'a str,
    ) -> Self {
        Self {
            context,
            state_root,
            repo,
            now,
        }
    }

    pub(super) fn step(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        if let Some(stopped) = self.lost_lease(held)? {
            return Ok(stopped);
        }
        match row.kind {
            OutboxKind::Create => self.create(held, row),
            OutboxKind::Comment => self.comment(held, row),
            OutboxKind::Reconverge => self.reconverge(held, row),
            OutboxKind::Link => self.link(held, row),
        }
    }

    fn lost_lease(&self, held: &mut Store) -> Result<Option<Step>, Failure> {
        let seconds = crate::harness::session::now_seconds().unwrap_or_default();
        Ok(held
            .take_outbox_lease(
                self.repo,
                std::process::id(),
                seconds,
                seconds + LEASE_SECONDS,
                process_alive,
            )?
            .map(|other| Step::Stopped(format!("the flush lease passed to process {other}"))))
    }

    fn begin(&self, held: &mut Store, row: &OutboxRow) -> Result<(), Failure> {
        if row.state == OutboxState::Queued {
            held.mark_outbox(row.seq, OutboxState::Sending, None, self.now)?;
        }
        Ok(())
    }

    pub(super) fn sent(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        if let Some(stopped) = self.lost_lease(held)? {
            return Ok(stopped);
        }
        held.mark_outbox(row.seq, OutboxState::Sent, None, self.now)?;
        Ok(Step::Sent)
    }

    fn settle(
        &self,
        held: &mut Store,
        row: &OutboxRow,
        failure: &Failure,
    ) -> Result<Step, Failure> {
        let detail = describe(failure);
        let state = match (fate_of(failure), row.state) {
            (Fate::Requeue, OutboxState::Queued) => OutboxState::Queued,
            _ => OutboxState::Sending,
        };
        held.mark_outbox(row.seq, state, Some(&detail), self.now)?;
        Ok(Step::Stopped(detail))
    }

    fn unknown(&self, held: &mut Store, row: &OutboxRow, detail: String) -> Result<Step, Failure> {
        held.mark_outbox(row.seq, OutboxState::Sending, Some(&detail), self.now)?;
        Ok(Step::Stopped(detail))
    }

    fn create(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        let text = |name: &str| {
            row.payload
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
        };
        let (title, body, state, identity) =
            (text("title"), text("body"), text("state"), text("identity"));
        let labels: Vec<String> = row
            .payload
            .get("labels")
            .and_then(serde_json::Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if row.state == OutboxState::Queued {
            let ensure = row
                .payload
                .get("ensure")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten();
            for pair in ensure {
                let (Some(name), Some(colour)) = (
                    pair.get(0).and_then(serde_json::Value::as_str),
                    pair.get(1).and_then(serde_json::Value::as_str),
                ) else {
                    continue;
                };
                if let Err(failure) = commands::ensure_label(self.context, name, colour) {
                    let detail = describe(&failure);
                    held.mark_outbox(row.seq, OutboxState::Queued, Some(&detail), self.now)?;
                    return Ok(Step::Stopped(detail));
                }
            }
        }
        self.begin(held, row)?;
        let pending = commands::pending_create_key(self.repo, identity, title);
        let (number, _) = match commands::file_or_adopt(
            self.context,
            held,
            self.repo,
            Some(&row.enqueued_at),
            &pending,
            title,
            body,
            state,
            &labels,
            row.state == OutboxState::Sending,
        ) {
            Ok(filed) => filed,
            Err(failure) => return self.settle(held, row, &failure),
        };
        match commands::issue_view(self.context, number, "title") {
            Ok(read) if read.get("title").and_then(serde_json::Value::as_str) == Some(title) => {}
            Ok(read) => {
                return self.unknown(
                    held,
                    row,
                    format!(
                        "issue #{number} was filed for {} and reads back titled {:?}",
                        row.issue,
                        read.get("title")
                    ),
                );
            }
            Err(failure) => {
                return self.unknown(
                    held,
                    row,
                    format!(
                        "issue #{number} was filed for {} and could not be read back: {}",
                        row.issue,
                        describe(&failure)
                    ),
                );
            }
        }
        if let Some(stopped) = self.lost_lease(held)? {
            return Ok(stopped);
        }
        if let Err(failure) = held.rekey(self.repo, row.issue, number, Some(row.seq)) {
            return self.unknown(
                held,
                row,
                format!(
                    "issue #{number} was filed for {}, and the local store refused to give it \
                     that number: {}",
                    row.issue,
                    failure.detail()
                ),
            );
        }
        let _ = held.set_meta(&pending, "");
        super::rekey_files(held, self.state_root)?;
        Ok(Step::Sent)
    }

    fn comment(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        if row.state == OutboxState::Sending {
            match self.comment_landed(row) {
                Ok(true) => return self.sent(held, row),
                Ok(false) => {}
                Err(failure) => return self.settle_unread(held, row, &failure),
            }
        }
        self.begin(held, row)?;
        let body = row
            .payload
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let staged = crate::paths::scratch_file(&format!("outbox-{}.md", row.op_id));
        crate::paths::replace_atomically(
            &staged,
            &format!("{}\n{}\n", body.trim_end(), marker_of(&row.op_id)),
        )
        .map_err(|error| Failure::Write(format!("the comment could not be staged: {error}")))?;
        let path = staged.display().to_string();
        let posted = crate::transport::run(
            &[
                "gh",
                "issue",
                "comment",
                &row.issue.to_string(),
                "--body-file",
                &path,
            ],
            Some(&self.context.repo_dir),
            bounded_write(),
        );
        let _ = std::fs::remove_file(&staged);
        if let Err(failure) = posted {
            return self.settle(held, row, &failure);
        }
        match self.comment_landed(row) {
            Ok(true) => self.sent(held, row),
            Ok(false) => self.unknown(
                held,
                row,
                format!(
                    "the comment on #{} was accepted and its marker was not read back",
                    row.issue
                ),
            ),
            Err(failure) => self.settle_unread(held, row, &failure),
        }
    }

    fn settle_unread(
        &self,
        held: &mut Store,
        row: &OutboxRow,
        failure: &Failure,
    ) -> Result<Step, Failure> {
        self.unknown(
            held,
            row,
            format!("the readback failed: {}", describe(failure)),
        )
    }

    fn comment_landed(&self, row: &OutboxRow) -> Result<bool, Failure> {
        let marker = marker_of(&row.op_id);
        for page in 1..=COMMENT_PAGES {
            let endpoint = format!(
                "repos/{{owner}}/{{repo}}/issues/{}/comments?per_page={COMMENT_PAGE}&page={page}",
                row.issue
            );
            let answer =
                crate::transport::gh_json(&["api", &endpoint], Some(&self.context.repo_dir))?
                    .ok_or_else(|| Failure::Read(format!("gh api {endpoint} returned nothing")))?;
            let comments = answer.as_array().ok_or_else(|| {
                Failure::Read(format!(
                    "gh api {endpoint} did not answer a list of comments"
                ))
            })?;
            if comments.iter().any(|comment| {
                comment
                    .get("body")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|body| body.contains(&marker))
            }) {
                return Ok(true);
            }
            if comments.len() < COMMENT_PAGE {
                return Ok(false);
            }
        }
        Err(Failure::Read(format!(
            "issue {} has {} or more comments, and the marker is in none of them",
            row.issue,
            COMMENT_PAGES * COMMENT_PAGE
        )))
    }

    fn reconverge(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        self.begin(held, row)?;
        match self.project(row) {
            Ok(()) => {
                if let Some(runtime) = row
                    .payload
                    .get(crate::transport::handoff::QUEUED_RUNTIME)
                    .and_then(serde_json::Value::as_str)
                {
                    held.mark_handoff_landed(row.issue, runtime)?;
                }
                self.sent(held, row)
            }
            Err(failure) => self.settle(held, row, &failure),
        }
    }

    fn project(&self, row: &OutboxRow) -> Result<(), Failure> {
        let ledger = store::Ledger::open(self.context, row.issue, self.now)?;
        if let Some(to) = ledger.state()? {
            commands::project_state(self.context, &ledger, row.issue, &to, None)?;
        }
        let ownership = row
            .payload
            .get("ownership")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if ownership {
            let event = claim::holding(&ledger.timeline()?, self.now).event;
            claim::converge_ownership_projection_from(
                self.context,
                row.issue,
                event,
                None,
                self.now,
                claim::Judge::Store(&ledger),
            )?;
        }
        if let Some(runtime) = row
            .payload
            .get(crate::transport::handoff::QUEUED_RUNTIME)
            .and_then(serde_json::Value::as_str)
        {
            crate::transport::handoff::require_unheld(
                self.context,
                row.issue,
                Some(runtime),
                self.now,
            )?;
            crate::transport::handoff::project_labels(self.context, row.issue, runtime)?;
        }
        Ok(())
    }

    fn link(&self, held: &mut Store, row: &OutboxRow) -> Result<Step, Failure> {
        let text = |name: &str| {
            row.payload
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
        };
        let (branch_name, base) = (text("branch"), text("base"));
        if row.state == OutboxState::Sending {
            match branch::is_branch_linked(self.context, row.issue, branch_name) {
                Ok(true) => return self.sent(held, row),
                Ok(false) => {}
                Err(failure) => return self.settle_unread(held, row, &failure),
            }
        }
        self.begin(held, row)?;
        let answer = match branch::develop_link(self.context, row.issue, branch_name, base) {
            Ok(answer) => answer,
            Err(failure) => return self.settle(held, row, &failure),
        };
        if answer.get("linked").and_then(serde_json::Value::as_bool) != Some(true) {
            let detail = format!(
                "`gh issue develop` did not link {branch_name} to #{}: {}",
                row.issue,
                answer
                    .get("detail")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("it gave no reason")
            );
            if let Some(stopped) = self.lost_lease(held)? {
                return Ok(stopped);
            }
            held.mark_outbox(row.seq, OutboxState::Sent, Some(&detail), self.now)?;
            return Ok(Step::NotLinked(detail));
        }
        match branch::is_branch_linked(self.context, row.issue, branch_name) {
            Ok(true) => self.sent(held, row),
            Ok(false) => self.unknown(
                held,
                row,
                format!(
                    "{branch_name} was linked to #{} and is not in its sidebar",
                    row.issue
                ),
            ),
            Err(failure) => self.settle_unread(held, row, &failure),
        }
    }
}
