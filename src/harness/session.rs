//! Who this run is, and what it swore.
//!
//! The tracker is the only authority on whether a claim is live. What lives
//! here is strictly smaller: a **pointer** saying which issue to go and ask
//! about, and when the last answer arrived. Losing this file costs a round trip,
//! never a wrong answer — which is what keeps it from being a second source of
//! truth. `bindings/github.md` refuses to cache configuration for exactly that
//! reason, and the same rule applies here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::outcome::{NoCommandReason, Refusal, Resolution};
use crate::paths;
use crate::transport::store::{Record, Store};

/// The runtime prefix in a run-id, when the harness cannot name a better one.
pub const DEFAULT_RUNTIME: &str = "claude";

/// The agents whose run ids begin with a runtime word other than
/// [`DEFAULT_RUNTIME`], keyed by the slug their hook is registered under.
pub const AGENT_RUNTIMES: &[(&str, &str)] = &[("codex", "codex")];

/// The runtime word a hook registered by `agent` derives its run id with:
/// [`DEFAULT_RUNTIME`] for an agent not in [`AGENT_RUNTIMES`] and for a hook
/// that sent none.
pub fn runtime_of_agent(agent: Option<&str>) -> &'static str {
    agent
        .and_then(|agent| AGENT_RUNTIMES.iter().find(|(slug, _)| *slug == agent))
        .map_or(DEFAULT_RUNTIME, |(_, runtime)| runtime)
}

/// One run's pointer into the tracker.
///
/// Every field is either derived from the agent's own session or a note about
/// when Estigia last asked the tracker something. None of it is authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// `<runtime>-<session-prefix>`, minted once and reused for every write.
    pub run_id: String,
    /// The issue this run holds, once it has claimed one, as its tracker
    /// spells it.
    pub issue: Option<crate::identity::Identity>,
    /// How many times this pointer has been written: the "expected revision" a
    /// stale writer fails against, so two processes sharing one pointer cannot
    /// overwrite each other with old news.
    ///
    /// `#[serde(default)]` so a pointer written by an earlier build loads as
    /// revision zero rather than failing to parse.
    #[serde(default)]
    pub revision: u64,
    /// A pointer that is on disk and could not be read. Not serialised: it
    /// describes the read, not the run, and an unknown result is not clearance.
    #[serde(skip)]
    pub unreadable: bool,
    /// Why the read failed, when it did: the pointer path and the error, so
    /// the refusal can name the **file** the operator has to look at.
    #[serde(skip)]
    pub unreadable_reason: Option<String>,
    /// The workflow state the run believes the issue is in.
    ///
    /// Handed to `verify-claim --expect-state`, which is what turns "somebody
    /// moved this while I was working" into a stop rather than a surprise.
    pub state: Option<String>,
    /// The repository the claim is against, so a write in a different checkout
    /// is not measured against it.
    pub repo_dir: Option<PathBuf>,
    /// The isolated checkout this run works in, recorded because it is *where
    /// the work happens*. Written by `start_branch`, or by a `claim` or renewal
    /// that discovers this run **adopted** one.
    #[serde(default)]
    pub worktree: Option<PathBuf>,
    /// Unix seconds of the last answer from the tracker.
    pub verified_at: Option<u64>,
    /// The idempotency key this run minted for its claim: fresh per claim and
    /// stable across a retry.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// The operation id this run's **release** is reserved under — a slot of
    /// its own, and not the one above, because an operation id reserves one
    /// kind.
    #[serde(default)]
    pub release_id: Option<String>,
    /// The complete immutable review transaction this run may spend.
    ///
    /// A head identifies bytes only after the pull request lineage has been
    /// selected, so all five receipt fields are persisted together.
    #[serde(default, alias = "review_receipt")]
    pub review_transaction: Option<crate::transport::claim::ReviewTransaction>,
    /// The head `publish_review` bound the review target to, when it did — a
    /// verdict is bound to exact bytes, and the boundary that delivers on it
    /// checks those bytes. Only what this run published through Estigia's own
    /// tools.
    #[serde(default)]
    pub reviewed_head: Option<String>,
    /// The roles this run's delegated contexts actually ran as. **Observed, not
    /// declared**, **within this run only**. A set, so re-entering one role does
    /// not grow the file, and ordered, so two runs that saw the same roles
    /// write the same bytes.
    #[serde(default)]
    pub roles: BTreeSet<String>,
    /// Targets this run's own context wrote after a delegated context had
    /// already run at least once — `agent_type` genuinely absent on a call the
    /// gate allowed, while `roles` was non-empty. A set, for the reasons
    /// `roles` is one. The element is what `Action::subject` names, which is
    /// not always a path: a write tool names one, and a shell write names the
    /// matched fragment, the verb, or the tool. A write before this run's
    /// first delegated call, or on a host that never sends `agent_type` at
    /// all, is recorded as neither this run's own nor a delegate's;
    /// `docs/honesty.md` names that gap.
    #[serde(default)]
    pub inline_writes: BTreeSet<String>,
    /// This pointer is a local claim under `Workflow: assisted`: nothing on
    /// the tracker backs it, and it is read by [`crate::harness::assisted`],
    /// never by `verify-claim`.
    #[serde(default)]
    pub assisted: bool,
    /// The branch an assisted run was given, which its worktree must have
    /// checked out.
    #[serde(default)]
    pub branch: Option<String>,
}

impl Run {
    /// A run that has sworn nothing yet.
    pub fn new(run_id: String) -> Self {
        Self {
            run_id,
            issue: None,
            revision: 0,
            unreadable: false,
            unreadable_reason: None,
            state: None,
            repo_dir: None,
            worktree: None,
            verified_at: None,
            operation_id: None,
            release_id: None,
            roles: BTreeSet::new(),
            inline_writes: BTreeSet::new(),
            review_transaction: None,
            reviewed_head: None,
            assisted: false,
            branch: None,
        }
    }

    /// The issue number, when this run holds an issue and its tracker numbers
    /// issues. `None` for a Linear key: those operations are GitHub-only.
    pub fn issue_number(&self) -> Option<u64> {
        self.issue
            .as_ref()
            .and_then(crate::identity::Identity::number)
    }

    /// Whether the last verification is still inside the renewal window.
    ///
    /// Never consulted for an irreversible boundary — see
    /// [`crate::harness::Sensitivity`]. This only decides whether a routine
    /// write may ride on the answer the previous one already paid for.
    pub fn within_window(&self, window: Duration) -> bool {
        match (self.verified_at, now_seconds()) {
            // `checked_sub`, not `saturating_sub`: saturating floors at zero,
            // so a stamp **ahead of the clock** would stay inside the window
            // for as long as the run lived.
            (Some(verified), Some(now)) => now
                .checked_sub(verified)
                .is_some_and(|age| age < window.as_secs()),
            // No answer yet, or a clock that will not say what time it is. Both
            // mean "ask", which is the safe direction.
            _ => false,
        }
    }

    /// Records that the tracker answered just now.
    pub fn mark_verified(&mut self) {
        self.verified_at = now_seconds();
    }

    /// Every directory this run's oath covers.
    ///
    /// The checkout the claim was made in, and the isolated one the work is
    /// written in. `repository-delivery.md` requires the second to exist and to
    /// be somewhere else: *"Place each implementation checkout outside the base
    /// working tree."* A gate that knows only the first is a gate that watches
    /// an empty room.
    pub fn covered(&self) -> impl Iterator<Item = &PathBuf> {
        self.repo_dir.iter().chain(self.worktree.iter())
    }

    /// The directories whose writes this run answers for: the worktree alone for
    /// an assisted run, whose base checkout is the person's own and is never
    /// admitted to it, and [`Run::covered`] otherwise.
    pub fn lands_in(&self) -> Vec<&PathBuf> {
        if self.assisted {
            self.worktree.iter().collect()
        } else {
            self.covered().collect()
        }
    }
}

/// Mints the run identity from the agent's session:
/// `<runtime>-<session-prefix><digest>`, derived rather than random so that two
/// invocations of the hook inside one session agree without writing anything
/// down first. The digest is over the whole session id, not its first eight
/// characters — two runs sharing an identity share a **pointer**, and an oath
/// quietly transferred is the worst thing that can go wrong here. FNV-1a is
/// used because this value is persisted: a hasher that may move across Rust
/// releases is a pointer that stops matching its own run.
pub fn run_id(runtime: &str, session_id: &str) -> String {
    let prefix: String = session_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect();
    // An absent session has no identity to derive, and one that looked derived
    // would be worse: `run_in` already answers a missing session by asking the
    // checkout instead, and this name is what the ledger records it under.
    if prefix.is_empty() {
        return format!("{runtime}-{NAMELESS}");
    }
    format!("{runtime}-{prefix}{:08x}", fnv1a(session_id))
}

/// What a run id says when the session had no identity to derive one from.
pub const NAMELESS: &str = "unknown";

/// Whether this run id names a run at all: `<runtime>-unknown` is **a name two
/// runs share**, fine as a label and not fine as an identity, because an oath
/// is bound to one. One half of [`run_id_shape`]'s answer rather than a second
/// rule beside it.
pub fn is_nameless(run_id: &str) -> bool {
    matches!(run_id_shape(run_id), RunIdShape::Nameless { .. })
}

/// What a caller-supplied string is, held against the one shape [`run_id`]
/// mints: `<runtime>-<prefix><hash>` with a dashless tail, or
/// `<runtime>-unknown`.
///
/// One answer, decided here, for every caller that has to admit or refuse an
/// id from outside this crate. Earlier code refused the spellings it had measured one
/// disjunct at a time — no dash, an empty runtime, an alphabet — and each
/// round was complete for its own spelling and blind to the next. The class
/// only closes when the whole shape is decided in one place, and the runtime
/// a caller projects is read off this answer rather than split again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunIdShape<'a> {
    /// The id of a session: `<runtime>-<prefix><hash>`. This is the runtime
    /// the id names, and the only value anything should derive from it.
    Named {
        /// The runtime the validated id names.
        runtime: &'a str,
    },
    /// `<runtime>-unknown`: an id `run_id` mints when the session had no
    /// identity, and a name two runs share.
    Nameless {
        /// The runtime the validated id names.
        runtime: &'a str,
    },
    /// Nothing [`run_id`] mints; the variant says which part is not.
    Unminted(Unminted),
}

/// The part of an id that keeps it out of the minted shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unminted {
    /// No `-`: nothing before one names a runtime.
    NoRuntime,
    /// The part before the last `-` is not a runtime component: lowercase
    /// `a-z0-9` groups joined by single `-`, and nothing else.
    Runtime,
    /// The part after the last `-` is not `<prefix><hash>`: not the shape
    /// `minted_suffix` admits. This is the door a caller-composed
    /// `<runtime>-<session-prefix>` walks through when the host's session id
    /// carries a dash and the fragment after the last one is not itself the
    /// minted shape: split there, the runtime it yields is real-looking and
    /// wrong.
    Suffix,
}

/// The shape of a caller-supplied run id, decided once.
pub fn run_id_shape(run_id: &str) -> RunIdShape<'_> {
    let Some((runtime, suffix)) = run_id.rsplit_once('-') else {
        return RunIdShape::Unminted(Unminted::NoRuntime);
    };
    if runtime_component(runtime).is_err() {
        return RunIdShape::Unminted(Unminted::Runtime);
    }
    if suffix == NAMELESS {
        return RunIdShape::Nameless { runtime };
    }
    if minted_suffix(suffix) {
        return RunIdShape::Named { runtime };
    }
    RunIdShape::Unminted(Unminted::Suffix)
}

/// Whether `runtime` is a component [`run_id`] can carry: the check the
/// derived runtime and `create`'s declared one share.
pub fn runtime_component(runtime: &str) -> Result<(), Unminted> {
    crate::transport::worktree::run_component(runtime)
        .map(|_| ())
        .map_err(|_| Unminted::Runtime)
}

/// Whether `suffix` is what [`run_id`] appends: up to eight ASCII
/// alphanumerics, then eight lowercase hex digits. A tail of eight hex digits
/// alone is admitted too — the contract has callers spell the id from a
/// session prefix, and the digest is not checkable without the session id it
/// was derived from.
fn minted_suffix(suffix: &str) -> bool {
    let bytes = suffix.as_bytes();
    (8..=16).contains(&bytes.len())
        && bytes.iter().all(|byte| byte.is_ascii_alphanumeric())
        && bytes[bytes.len() - 8..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

/// The runtime a run id names: everything before the **last** `-`. Superseded
/// by [`run_id_shape`], which produces the runtime the gate uses from its own
/// split; nothing in production reaches this one, and it is retained for its
/// tests and as the record of the split rule. It falls back to the id itself
/// when there is no `-` at all: every id [`run_id`] mints has one, but the
/// string a caller hands this is not guaranteed to.
pub fn runtime_of(run_id: &str) -> &str {
    run_id
        .rsplit_once('-')
        .map_or(run_id, |(runtime, _)| runtime)
}

/// FNV-1a, 32-bit.
///
/// Written out rather than borrowed because this value is persisted — see the
/// note on [`run_id`] about hashers that are free to change.
fn fnv1a(text: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Where run pointers are kept.
pub fn state_root(home_override: Option<&Path>) -> Result<PathBuf, Refusal> {
    let home = match home_override {
        Some(path) => path.to_path_buf(),
        None => paths::home_dir().map_err(|error| {
            Refusal::not_started(
                "home-not-resolvable",
                format!("{error}"),
                Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "a HOME or USERPROFILE the process can read",
                ),
            )
        })?,
    };
    Ok(home.join(".estigia").join("runs"))
}

/// Where one run's pointer lives.
///
/// Public so a refusal can name the file: an operator told their pointer cannot
/// be read has nothing to act on until they know which one.
pub fn pointer_path(root: &Path, run_id: &str) -> PathBuf {
    root.join(format!("{}.json", run_file_name(run_id)))
}

/// The file name one run is stored under, sanitised so it cannot leave the
/// directory it is joined to.
fn run_file_name(run_id: &str) -> String {
    // Run-ids are minted from an alphanumeric filter above, so this cannot walk
    // out of the directory. Sanitised again anyway: the callers are public and
    // may not mint their argument the same way.
    let safe: String = run_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    // A name that maps to itself needs nothing else, and gets nothing else:
    // every run already on disk keeps the file it is in.
    if !run_id.is_empty()
        && run_id.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return safe;
    }
    // Anything else carries a digest of what was actually asked for, because
    // the name above **folds**: `claude/aaaa` and `claude_aaaa` were one file,
    // and on a case-insensitive filesystem so were `claude-aaaa1111` and
    // `Claude-AAAA1111`. Separated by `_`, which a name that maps to itself can
    // never contain, so a digested name cannot collide with an undigested one.
    format!("{safe}_{:08x}", fnv1a(run_id))
}

/// Reads this run's pointer, or a fresh one.
///
/// Only a pointer that is **not there** is a run that has sworn nothing. A
/// pointer that is there and cannot be read says a run under this name existed
/// and not what it swore, so it loads as *unreadable*, never as fresh.
pub fn load(root: &Path, run_id: &str) -> Run {
    let path = pointer_path(root, run_id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        // No pointer at all: this run has sworn nothing. `NotFound` is the only
        // read failure that may answer this, and even then only while the root
        // is a directory — Windows answers a path whose parent is not a
        // directory with `ERROR_PATH_NOT_FOUND`, which std maps to `NotFound`,
        // so absence is confirmed against the root rather than taken from the
        // error kind.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return match fs::metadata(root) {
                Ok(found) if !found.is_dir() => Run {
                    unreadable: true,
                    unreadable_reason: Some(format!(
                        "{}: the directory run pointers are kept in is not a directory, so no \
                         pointer under it can be read",
                        root.display()
                    )),
                    ..Run::new(run_id.to_owned())
                },
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Run {
                    unreadable: true,
                    unreadable_reason: Some(format!(
                        "{}: the directory run pointers are kept in cannot be read — {error}",
                        root.display()
                    )),
                    ..Run::new(run_id.to_owned())
                },
                _ => Run::new(run_id.to_owned()),
            };
        }
        // Present and unopenable, which is not the same as absent. The path and
        // the error ride along so the refusal a gate or tool builds can name
        // the file an operator has to look at.
        Err(error) => {
            return Run {
                unreadable: true,
                unreadable_reason: Some(format!("{}: {error}", path.display())),
                ..Run::new(run_id.to_owned())
            };
        }
    };
    match serde_json::from_str::<Run>(&text) {
        Ok(run) if run.run_id == run_id => run,
        // Present and unreadable, which is not the same as absent. A pointer
        // under this run's name existed, so whether it swore is unknown — and an
        // unknown is not clearance.
        _ => Run {
            unreadable: true,
            ..Run::new(run_id.to_owned())
        },
    }
}

/// Writes this run's pointer.
pub fn store(root: &Path, run: &Run) -> Result<bool, Refusal> {
    let path = pointer_path(root, &run.run_id);
    // Not written in place: `fs::write` truncates first, so a reader arriving
    // mid-write would see a partial file, which `load` answers with an
    // *unreadable* run.
    //
    // The stale-writer check: a pointer on disk that has been written more
    // times than the one in hand is news this caller has not seen. Skipped
    // rather than refused — storing is best effort by design.
    //
    // A pointer that was **removed** is news as well, and the revision cannot
    // carry it: a missing file reads as revision zero, which every copy in
    // hand is newer than.
    if run.revision > 0 && !path.exists() {
        return Ok(false);
    }
    // A run loaded from a pointer that would not parse knows nothing, and
    // `unreadable` is `#[serde(skip)]` — so storing it writes a **readable**
    // pointer saying this run holds no issue. Dropped rather than refused,
    // like the two checks around it: storing is best effort.
    if run.unreadable {
        return Ok(false);
    }
    // `false` rather than `Ok(())`: this used to report a write it had not
    // made, and every caller wrote `let _ =`. See [`update`].
    let on_disk = load(root, &run.run_id);
    // A pointer that cannot be read is news too: the revision that would say
    // whether this caller is behind is exactly what cannot be read out of it.
    // Dropped rather than refused, like the two checks around it.
    if on_disk.unreadable || on_disk.revision > run.revision {
        return Ok(false);
    }
    let mut next = run.clone();
    next.revision = on_disk.revision.max(run.revision).saturating_add(1);

    let write = fs::create_dir_all(root)
        .and_then(|()| serde_json::to_string_pretty(&next).map_err(std::io::Error::other))
        .and_then(|text| crate::paths::replace_atomically(&path, &text))
        .map(|()| true);
    write.map_err(|error| {
        Refusal::not_started(
            "run-pointer-not-writable",
            format!("{}: {error}", path.display()),
            Resolution::no_command(
                NoCommandReason::WorldAction,
                "write permission on the Estigia state directory",
            ),
        )
    })
}

/// How many times a change is re-applied against a pointer that moved.
///
/// Two, because the writers are a hook and a tool rather than a crowd: losing
/// twice in a row means something is storing continuously, and a third read
/// would be as likely to lose as the second.
const UPDATE_ATTEMPTS: usize = 2;

/// Applies a change to this run's pointer, against the newest one on disk.
///
/// The change is re-applied to what is on disk rather than dropped, because a
/// **tool** records what only it knows — a lost `issue` or `worktree` write is
/// not carried by the fresher pointer. `change` runs again on each attempt and
/// must be idempotent, which setting fields is.
pub fn update(root: &Path, run_id: &str, change: impl FnMut(&mut Run)) -> Run {
    updated(root, run_id, change).0
}

/// The same, and whether what the caller asked for is **on disk**: a pointer
/// write that failed must be distinguishable from one that landed, because the
/// tracker write did happen and an unknown result is not clearance.
pub fn updated(root: &Path, run_id: &str, mut change: impl FnMut(&mut Run)) -> (Run, bool) {
    let mut run = load(root, run_id);
    for _ in 0..UPDATE_ATTEMPTS {
        // An unreadable pointer is not a fresh one, and writing over it would
        // throw away whatever a person still has to read out of it.
        if run.unreadable {
            return (run, false);
        }
        change(&mut run);
        match store(root, &run) {
            Ok(true) => return (run, true),
            Err(_) => return (run, false),
            Ok(false) => run = load(root, run_id),
        }
    }
    // Every attempt lost the race to another writer. Nothing of this caller's
    // is on disk, and the count is not the point — what is on disk is.
    (run, false)
}

/// Forgets this run's pointer. The claim on the tracker is untouched.
pub fn forget(root: &Path, run_id: &str) {
    let _ = fs::remove_file(pointer_path(root, run_id));
}

/// What a stop-after request records when the operator gave no reason.
const STOP_AFTER_DEFAULT: &str = "the operator asked to stop after the current task";

/// Where one run's stop-after request lives: beside the run pointers, not
/// among them, the way [`ledger_path`] is.
///
/// A file of its own rather than a field on the pointer, because `release`
/// calls [`forget`] and the loop reads this only after the claim is released:
/// a flag on the pointer would be gone before the loop could act on it.
pub fn stop_after_dir(state_root: &Path) -> PathBuf {
    state_root.parent().unwrap_or(state_root).join("stop-after")
}

/// Where one run's stop-after request lives.
pub fn stop_after_path(state_root: &Path, run_id: &str) -> PathBuf {
    stop_after_dir(state_root).join(format!("{}.json", run_file_name(run_id)))
}

/// The reason recorded for this run, or `None` when no request was made or the
/// file cannot be read.
///
/// An unreadable file is not a set request: it is not clearance to stop, and it
/// is not a reason to stop either.
pub fn stop_after(state_root: &Path, run_id: &str) -> Option<String> {
    let reason = fs::read_to_string(stop_after_path(state_root, run_id)).ok()?;
    (!reason.is_empty()).then_some(reason)
}

/// Records that this run's loop should stop once its claim is released.
pub fn set_stop_after(
    state_root: &Path,
    run_id: &str,
    reason: Option<&str>,
) -> std::io::Result<()> {
    let path = stop_after_path(state_root, run_id);
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let reason = reason
        .filter(|reason| !reason.is_empty())
        .unwrap_or(STOP_AFTER_DEFAULT);
    crate::paths::replace_atomically(&path, reason)
}

/// Takes back this run's stop-after request, ignoring its absence.
pub fn clear_stop_after(state_root: &Path, run_id: &str) {
    let _ = fs::remove_file(stop_after_path(state_root, run_id));
}

/// A run's stored loop: the objective `/flow-loop` gave it, the completion cap,
/// and how it last answered.
///
/// A file of its own beside the stop-after request and for the same reason: the
/// Claude `Stop` hook reads it after the run pointer may have been forgotten by
/// a `release`, and a loop outlives each task it delivers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopGoal {
    /// The prompt the loop is working, already rendered by the objective rule.
    pub objective: String,
    /// Stop after this many issues reached `done` under the run, when set.
    #[serde(default)]
    pub max: Option<u64>,
    /// The stop that ended the loop, when it has ended.
    #[serde(default)]
    pub stopped: Option<LoopEnd>,
    /// The last decision the hook reached, whatever it was.
    #[serde(default)]
    pub last: Option<LoopEnd>,
}

/// One recorded moment of a run's loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopEnd {
    /// What happened.
    pub reason: String,
    /// When, in Unix seconds.
    pub at: u64,
}

/// Where one run's loop record lives: beside the stop-after requests, under
/// `~/.estigia/`, so `release` forgetting the run pointer does not forget it.
pub fn loop_dir(state_root: &Path) -> PathBuf {
    state_root.parent().unwrap_or(state_root).join("loop")
}

/// Where one run's loop record lives.
pub fn loop_path(state_root: &Path, run_id: &str) -> PathBuf {
    loop_dir(state_root).join(format!("{}.json", run_file_name(run_id)))
}

/// Reads this run's loop record, or `None` when none was stored or it cannot be
/// read. An unreadable record is not a running loop: it neither continues nor
/// restarts anything.
pub fn loop_goal(state_root: &Path, run_id: &str) -> Option<LoopGoal> {
    let text = fs::read_to_string(loop_path(state_root, run_id)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes this run's loop record.
pub fn set_loop_goal(state_root: &Path, run_id: &str, goal: &LoopGoal) -> std::io::Result<()> {
    let path = loop_path(state_root, run_id);
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let text = serde_json::to_string_pretty(goal).map_err(std::io::Error::other)?;
    crate::paths::replace_atomically(&path, &text)
}

/// Forgets this run's loop record.
pub fn clear_loop_goal(state_root: &Path, run_id: &str) {
    let _ = fs::remove_file(loop_path(state_root, run_id));
}

/// Records what the loop just answered, and, for a terminal answer, that it
/// stopped, answering whether the record reached disk.
///
/// The caller holds the goal it read, so a write that fails is visible to it:
/// a terminal answer that could not be recorded must not be treated as durable,
/// and the caller keeps whatever request it would have consumed.
pub fn record_loop_answer(
    state_root: &Path,
    run_id: &str,
    goal: &mut LoopGoal,
    reason: &str,
    terminal: bool,
) -> bool {
    let at = now_seconds().unwrap_or(0);
    goal.last = Some(LoopEnd {
        reason: reason.to_owned(),
        at,
    });
    if terminal {
        goal.stopped = Some(LoopEnd {
            reason: reason.to_owned(),
            at,
        });
    }
    set_loop_goal(state_root, run_id, goal).is_ok()
}

/// A 32-character lowercase hex idempotency key.
///
/// Derived, not random: the crate carries no RNG, and this key does not need
/// unpredictability — it needs to be fresh per claim and stable across a retry.
/// Freshness comes from the clock, stability from storing it the moment it is
/// minted. Two runs on one machine differ by run-id; two claims by one run on
/// one issue differ by the nanosecond they were minted at.
pub fn mint_operation_id(run_id: &str, issue: u64) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let mut seed = [0u64; 2];
    seed[0] = nanos as u64;
    seed[1] = (nanos >> 64) as u64 ^ issue ^ std::process::id() as u64;
    for byte in run_id.bytes() {
        // FNV-1a, which is small, has no dependency, and spreads a short input
        // well enough for a key whose only job is to be distinct.
        seed[1] ^= u64::from(byte);
        seed[1] = seed[1].wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}{:016x}", seed[0], seed[1])
}

/// Unix seconds now, or nothing when the clock will not say.
pub fn now_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

/// The name the decision history used before it moved into the local store.
///
/// Nothing writes it any more: it survives only as the source
/// [`maintain`] imports once and then deletes.
pub const LEDGER: &str = "decisions.jsonl";

/// How long a decision or a role record is kept before the age prune removes it.
pub const RECORD_RETENTION_DAYS: u64 = 30;

/// How long a decision append waits on the store's lock before giving up.
const APPEND_WAIT: Duration = Duration::from_secs(5);

/// Appends one decision to the local store.
///
/// Only [`super::Decision::Allow`] and [`super::Decision::Deny`] are recorded:
/// `Outside` is Estigia standing aside. Best effort, always: it never fails a
/// decision, because a refusal that could not be written down is still a
/// refusal. A failed append is counted in the store, so `doctor` can report it
/// rather than discard it.
pub fn record(root: &Path, entry: &Value) {
    append(root, Record::Decision, entry);
}

fn append(root: &Path, kind: Record, entry: &Value) {
    append_waiting(root, kind, entry, APPEND_WAIT);
}

fn append_waiting(root: &Path, kind: Record, entry: &Value, wait: Duration) {
    let path = crate::transport::store::beside(root);
    let Ok(store) = Store::open_waiting(&path, wait) else {
        return;
    };
    if store.add_record(kind, entry).is_err() {
        let _ = store.note_record_failure(kind);
    }
}

/// A count of seconds, written the way every other stamp in this crate is. The
/// conversion is written out rather than taken from a crate: it is one
/// civil-from-days function, and `strftime("%Y-%m-%dT%H:%M:%SZ")` is what the
/// binding writes.
pub fn stamp_of(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's civil-from-days, shifted so the era starts on 0000-03-01.
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        (rest % 3_600) / 60,
        rest % 60
    )
}

/// Where the legacy decision file sits: beside the run pointers, not among them.
pub fn ledger_path(state_root: &Path) -> PathBuf {
    state_root.parent().unwrap_or(state_root).join(LEDGER)
}

/// The historical pair: the legacy files' one-time import, and the age prune.
const LEGACY_IMPORTED: &str = "legacy-imported";
const LAST_PRUNE: &str = "last-prune";
const PRUNE_INTERVAL: u64 = 24 * 60 * 60;

/// Imports any legacy JSONL history once, then prunes by age at most once a day.
///
/// Called from `SessionStart` so neither ever lands on a write's critical path.
/// Best effort: a store that will not open is reported by `doctor`, never a
/// reason to fail a session that has not decided anything yet.
pub fn maintain(root: &Path) {
    let path = crate::transport::store::beside(root);
    let Ok(store) = Store::open_waiting(&path, APPEND_WAIT) else {
        return;
    };
    import_legacy(&store, root);
    let now = now_seconds().unwrap_or_default();
    let last = store
        .meta(LAST_PRUNE)
        .ok()
        .flatten()
        .and_then(|at| at.parse::<u64>().ok())
        .unwrap_or_default();
    if now.saturating_sub(last) < PRUNE_INTERVAL {
        return;
    }
    let cutoff = now.saturating_sub(RECORD_RETENTION_DAYS * 24 * 60 * 60);
    let _ = store.prune_records(cutoff);
    let _ = store.set_meta(LAST_PRUNE, &now.to_string());
}

/// The legacy files, oldest first, each with the store history it fed.
fn legacy_files(root: &Path) -> [(PathBuf, Record); 4] {
    let ledger = ledger_path(root);
    let roles = roles_path(root);
    [
        (rotated(&ledger), Record::Decision),
        (ledger, Record::Decision),
        (rotated(&roles), Record::Role),
        (roles, Record::Role),
    ]
}

fn rotated(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

fn import_legacy(store: &Store, root: &Path) {
    let mut batch: Vec<(Record, Vec<Value>)> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for (path, kind) in legacy_files(root) {
        match fs::read_to_string(&path) {
            Ok(text) => {
                let entries = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .collect();
                batch.push((kind, entries));
                files.push(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                let _ = store.note_record_failure(kind);
                return;
            }
        }
    }
    match store.import_records(LEGACY_IMPORTED, &batch) {
        Ok(true) => {
            for path in files {
                let _ = fs::remove_file(&path);
            }
        }
        Ok(false) => {}
        Err(_) => {
            for (kind, _) in &batch {
                let _ = store.note_record_failure(*kind);
            }
        }
    }
}

/// The name the role history used before it moved into the local store.
///
/// A separate history from [`LEDGER`] because a role observation is not an
/// adjudication. Like [`LEDGER`], the file is now a legacy import source only.
pub const ROLES: &str = "roles.jsonl";

/// Appends one observed role to the local store.
///
/// `entry` is built by the caller and carries the run, the `agent_type` the gate
/// saw, the checkout and, when the run holds one, the issue. Best effort,
/// always: a role that could not be written down is still a role the gate saw,
/// and a failed append is counted for `doctor`.
pub fn record_role(root: &Path, entry: &Value) {
    append(root, Record::Role, entry);
}

/// Where the legacy role file sits: beside the ledger, not among the run pointers.
pub fn roles_path(state_root: &Path) -> PathBuf {
    state_root.parent().unwrap_or(state_root).join(ROLES)
}

/// Every role this machine observed, oldest first.
///
/// A store that will not open answers nothing rather than a fabricated empty
/// history; a caller that needs the difference reads the store directly.
pub fn role_records(root: &Path) -> Vec<Value> {
    records(root, Record::Role)
}

/// Every decision this machine recorded, oldest first.
pub fn decision_records(root: &Path) -> Vec<Value> {
    records(root, Record::Decision)
}

fn records(root: &Path, kind: Record) -> Vec<Value> {
    let path = crate::transport::store::beside(root);
    let Ok(Some(store)) = crate::transport::store::open_for_read(&path) else {
        return Vec::new();
    };
    store.records(kind).unwrap_or_default()
}

/// Every run this machine has a pointer for: *which issues to go and ask
/// about*, since the tracker is the authority on whether those claims are live.
/// Quiet about failure: an unreadable state directory is a machine with nothing
/// to report.
pub fn holdings(root: &Path) -> Vec<Run> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut runs: Vec<Run> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|kind| kind == "json"))
        .filter_map(|entry| {
            let path = entry.path();
            let run = pointer_at(&path)?;
            Some(run)
        })
        .filter(|run| run.issue.is_some())
        .collect();
    // Oldest answer first: the ones most likely to be dead read first.
    runs.sort_by_key(|run| (run.verified_at, run.run_id.clone()));
    runs
}

/// Run pointers that are there and cannot be read: not the same answer as
/// [`holdings`], because a corrupt pointer must not let a push through as
/// [`crate::harness::Decision::Outside`]. A missing directory is not
/// unreadable: a machine where nothing has ever claimed has no pointers, and
/// that is an answer.
pub fn unreadable_holdings(root: &Path) -> Vec<String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => return vec![format!("{}: {error}", root.display())],
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "json"))
        // A stand-down a build before the move left here; it is not a pointer
        // and never was.
        .filter(|path| path.file_name() != Some(std::ffi::OsStr::new("stand-down.json")))
        // The same question `holdings` asks, asked the other way round, and
        // through the same reader so the two lists cannot part company.
        .filter(|path| pointer_at(path).is_none())
        .map(|path| path.display().to_string())
        .collect()
}

/// The pointer in this file, when it is one this build can act on: it has to
/// parse, and it has to be **filed under the name it gives itself**.
fn pointer_at(path: &Path) -> Option<Run> {
    let run: Run = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    (path.file_stem()?.to_str()? == run.run_id).then_some(run)
}

/// A span of seconds, in the largest unit that still says something. Under
/// three quarters of a minute is `just now` rather than `0 min ago`.
pub fn age(seconds: u64) -> String {
    let plural = |count: u64, unit: &str| {
        if count == 1 {
            format!("{count} {unit} ago")
        } else {
            format!("{count} {unit}s ago")
        }
    };
    // Truncated, never rounded. Rounding gave `3599` seconds sixty minutes,
    // which is an hour written in the unit below it — a boundary that reads as
    // a bug in the report.
    match seconds {
        0..60 => "just now".to_owned(),
        60..3_600 => format!("{} min ago", seconds / 60),
        3_600..86_400 => format!("{} h ago", seconds / 3_600),
        _ => plural(seconds / 86_400, "day"),
    }
}

/// How long since the tracker last answered for this run.
///
/// Three answers, not two: a stamp ahead of this machine's clock is neither
/// "just now" nor "never verified" — a run whose clock moved **was** verified,
/// and a fact and the absence of one are not the same sentence.
pub enum Silence {
    /// The tracker has never answered for this run.
    Never,
    /// It answered this many seconds ago.
    For(u64),
    /// It answered at a time this machine's clock has not reached.
    AheadOfTheClock,
}

/// How long since the tracker last answered for this run.
pub fn silence(run: &Run) -> Silence {
    let (Some(verified), Some(now)) = (run.verified_at, now_seconds()) else {
        return Silence::Never;
    };
    match now.checked_sub(verified) {
        Some(seconds) => Silence::For(seconds),
        None => Silence::AheadOfTheClock,
    }
}

impl Silence {
    /// The seconds, when there are seconds — for a caller that wants a number.
    pub fn seconds(&self) -> Option<u64> {
        match self {
            Self::For(seconds) => Some(*seconds),
            _ => None,
        }
    }

    /// What a person reads about it.
    pub fn said(&self) -> String {
        match self {
            Self::Never => "never verified".to_owned(),
            Self::For(seconds) => age(*seconds),
            Self::AheadOfTheClock => "stamped ahead of this clock".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_written_under_the_old_receipt_key_still_loads_and_is_rewritten_under_the_new() {
        let old = serde_json::json!({
            "run_id": "claude-old-key",
            "issue": 20,
            "state": "review",
            "review_receipt": {
                "epoch": "a".repeat(32),
                "pr": 54,
                "head": "b".repeat(40),
                "base": "c".repeat(40),
                "digest": "d".repeat(64),
            },
        })
        .to_string();

        let run: Run = serde_json::from_str(&old).expect("a pointer with the old key loads");
        let transaction = run.review_transaction.clone().expect("the old key is read");
        assert_eq!((transaction.pr, transaction.epoch.len()), (54, 32));

        let rewritten = serde_json::to_value(&run).expect("the pointer serialises");
        assert!(rewritten.get("review_receipt").is_none(), "{rewritten}");
        assert_eq!(rewritten["review_transaction"]["pr"], 54, "{rewritten}");
    }

    #[test]
    fn a_legacy_reviewed_head_remains_readable_but_is_not_a_complete_receipt() {
        let legacy = serde_json::json!({
            "run_id": "claude-legacy",
            "issue": 20,
            "state": "review",
            "repo_dir": serde_json::Value::Null,
            "verified_at": serde_json::Value::Null,
            "reviewed_head": "a".repeat(40),
        })
        .to_string();
        let run: Run = serde_json::from_str(&legacy).expect("the legacy pointer remains readable");
        assert_eq!(run.reviewed_head.as_deref(), Some("a".repeat(40).as_str()));
        assert_eq!(run.review_transaction, None);
    }

    #[test]
    fn a_run_whose_own_pointer_would_not_parse_does_not_write_one_back() {
        let dir = tempfile::tempdir().expect("a temporary root");
        let root = dir.path();
        let path = pointer_path(root, "claude-a");
        std::fs::create_dir_all(root).expect("the state directory");
        let wreckage = r#"{"run_id": "claude-a", "issue": 7, "revi"#;
        fs::write(&path, wreckage).expect("a pointer that will not parse");

        let run = load(root, "claude-a");
        assert!(run.unreadable, "the corpus is wrong: these bytes parse");
        assert_eq!(run.issue, None, "an unreadable pointer answers no issue");

        // What is on disk when the store happens, and it has to be the one
        // combination only this guard stops. A pointer that is still unreadable
        // is caught by the on-disk check, and a readable one at a higher
        // revision by the stale-writer check — so the case that isolates this
        // one is a **readable pointer at revision zero**, which is exactly what
        // a build from before the `revision` field left behind: the field is
        // `#[serde(default)]`, so those load as zero.
        //
        // It is also the sequence the `unreadable` field's own doc names: a
        // release changes this struct, every pointer the last one wrote stops
        // parsing, and something puts an older-format one back.
        let older = r#"{"run_id": "claude-a", "issue": null, "state": null, "repo_dir": null,
                        "verified_at": null}"#;
        fs::write(&path, older).expect("an older, readable pointer");
        assert_eq!(load(root, "claude-a").revision, 0, "the fixture is wrong");

        assert_eq!(
            store(root, &run),
            Ok(false),
            "a run that knows nothing wrote a pointer saying it holds nothing"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("still there"),
            older,
            "a run loaded from bytes nobody could parse wrote a readable pointer swearing to none"
        );
    }

    #[test]
    fn a_pointer_that_went_unreadable_is_not_written_over() {
        let dir = tempfile::tempdir().expect("a temporary root");
        let root = dir.path();

        let mut held = Run::new("claude-a".to_owned());
        held.issue = Some(crate::identity::Identity::Number(7));
        store(root, &held).expect("the first store");
        // What the hook is still carrying: readable, and behind.
        let stale = load(root, "claude-a");
        assert!(!stale.unreadable && stale.issue == Some(crate::identity::Identity::Number(7)));

        // The release is interrupted part-way through putting it down.
        let path = pointer_path(root, "claude-a");
        let wreckage = r#"{"run_id": "claude-a", "issue": null, "revi"#;
        fs::write(&path, wreckage).expect("a half-written pointer");
        assert!(
            load(root, "claude-a").unreadable,
            "the corpus is wrong: these bytes parse"
        );

        assert_eq!(
            store(root, &stale),
            Ok(false),
            "a store reported a write over a pointer it could not read"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("still there"),
            wreckage,
            "the corruption was replaced, and with it the only sign of it"
        );

        // And `update` stops rather than spinning: it reloads on a dropped
        // write, sees the unreadable pointer, and leaves it alone.
        let after = update(root, "claude-a", |run| {
            run.issue = Some(crate::identity::Identity::Number(9))
        });
        assert!(after.unreadable, "update walked past an unreadable pointer");
        assert_eq!(
            fs::read_to_string(&path).expect("still there"),
            wreckage,
            "update wrote over it after store had refused to"
        );
    }

    #[test]
    fn an_io_unreadable_pointer_is_an_unknown_and_is_never_written_over() {
        // At the loader rather than at the gate: a pointer the
        // filesystem refuses to read — not one that is absent — must load as
        // *unreadable*, never as a fresh run. The fresh-run reading is the one
        // `gate` answers `Outside(NothingSworn)` for, and that is the silent
        // disarming the issue is about: the tracker still names the run as
        // holder while its writes pass through ungated.
        let dir = tempfile::tempdir().expect("a temporary root");
        let root = dir.path();
        let path = pointer_path(root, "claude-dirblock");
        std::fs::create_dir_all(&path).expect("a directory where the pointer should be");

        let run = load(root, "claude-dirblock");
        assert!(
            run.unreadable,
            "an unreadable pointer was loaded as a run that swore nothing"
        );
        assert!(
            run.unreadable_reason
                .as_deref()
                .is_some_and(|why| why.contains("claude-dirblock.json")),
            "the reason does not name the pointer it could not read: {:?}",
            run.unreadable_reason
        );
        assert_eq!(run.issue, None, "an unreadable pointer answers no issue");

        // And `store` leaves the unreadable thing alone: the guard that stops a
        // run whose own copy came from unreadable bytes must also stop a run
        // whose pointer cannot be opened, because writing a fresh pointer over
        // it would discard the only sign that the run's record was lost.
        let mut stale = Run::new("claude-dirblock".to_owned());
        stale.issue = Some(crate::identity::Identity::Number(7));
        assert_eq!(
            store(root, &stale),
            Ok(false),
            "a store wrote a pointer over a record that could not be read"
        );
        assert!(
            path.is_dir(),
            "the unreadable thing was replaced, and with it the only sign of it"
        );
    }

    #[test]
    fn a_state_root_that_is_not_a_directory_is_an_unknown_and_not_an_empty_one() {
        // The spec for this issue asked for a pointer whose parent is a file,
        // expecting the read to fail with something that is not `NotFound`.
        // Measured on Windows it is exactly `NotFound` — `ERROR_PATH_NOT_FOUND`,
        // os error 3 — so the arm that answers *this run swore nothing* is the
        // one it lands in, for every run at once, and the gate stands every one
        // of them aside. On Linux the same shape answers `NotADirectory`, which
        // is why reading the error kind alone cannot decide it. Absence is
        // confirmed against the root instead.
        let dir = tempfile::tempdir().expect("a temporary root");
        let root = dir.path().join("runs");
        std::fs::write(&root, "a file where the pointers should live")
            .expect("something that is not a directory");

        let run = load(&root, "claude-notadir");
        assert!(
            run.unreadable,
            "a state root that cannot hold a pointer was read as a run that swore nothing"
        );
        assert!(
            run.unreadable_reason
                .as_deref()
                .is_some_and(|why| why.contains("runs")),
            "the reason names no root, so nobody can find what to look at: {:?}",
            run.unreadable_reason
        );

        // The ordinary case is the other side of the same arm and must stay
        // exactly as it was: nothing claimed yet on this machine, so the root
        // is simply not there.
        let fresh = load(&dir.path().join("never-created"), "claude-firstrun");
        assert!(
            !fresh.unreadable && fresh.issue.is_none(),
            "a machine with no state yet was refused instead of being read as unsworn"
        );
    }

    #[test]
    fn a_run_id_is_stable_for_one_session() {
        let first = run_id(DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345");
        let second = run_id(DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345");
        assert_eq!(first, second);
        // The readable prefix is still the readable prefix: a person reading a
        // run id in an issue comment is why it is there.
        assert!(first.starts_with("claude-0198fe1c"), "{first}");
        // Pinned, because this value is written to disk and read back by a
        // later build. A hasher free to change between releases would deny
        // every write for a run in flight after a toolchain upgrade.
        assert_eq!(first, "claude-0198fe1c69bc2b12");
    }

    #[test]
    fn two_sessions_do_not_collide() {
        assert_ne!(
            run_id(DEFAULT_RUNTIME, "aaaaaaaa-1111"),
            run_id(DEFAULT_RUNTIME, "bbbbbbbb-2222")
        );
        for (first, second) in [
            (
                "session_01KvQFLvJbDRzAWzq6GCy8B7",
                "session_01ZZZZZZZZZZZZZZZZZZZZZZ",
            ),
            // Same twenty characters, one apart at the end.
            ("thread-0001112223334445", "thread-0001112223334446"),
            // A UUID whose first block repeats, which is not exotic: only the
            // first eight hex digits were ever read.
            (
                "4d1f0f7a-57a6-4911-a1bc-25cbfaa16cd0",
                "4d1f0f7a-99ff-4000-bbbb-000000000000",
            ),
        ] {
            assert_ne!(
                run_id(DEFAULT_RUNTIME, first),
                run_id(DEFAULT_RUNTIME, second),
                "two sessions of one host mint one identity, so one inherits the other's oath"
            );
        }
    }

    #[test]
    fn a_second_session_does_not_inherit_the_first_ones_oath() {
        let home = tempfile::tempdir().expect("a temporary home");
        let root = state_root(Some(home.path())).expect("a state root");

        let mine = run_id(DEFAULT_RUNTIME, "session_01AAAAAAAAAAAAAAAAAAAAAA");
        let mut run = Run::new(mine.clone());
        run.issue = Some(crate::identity::Identity::Number(42));
        store(&root, &run).expect("the pointer is written");

        let theirs = run_id(DEFAULT_RUNTIME, "session_01BBBBBBBBBBBBBBBBBBBBBB");
        assert_ne!(mine, theirs, "two sessions minted one identity");
        assert_eq!(
            load(&root, &theirs).issue,
            None,
            "another session's run read this one's claim as its own"
        );
        // And this run still finds its own.
        assert_eq!(
            load(&root, &mine).issue,
            Some(crate::identity::Identity::Number(42))
        );
    }

    #[test]
    fn a_session_with_no_id_still_mints_something_usable() {
        // An agent that sends no session_id must not produce a run-id of
        // `claude-`, which would collide with every other such run.
        assert_eq!(run_id(DEFAULT_RUNTIME, ""), "claude-unknown");
        assert_eq!(run_id(DEFAULT_RUNTIME, "///"), "claude-unknown");
    }

    #[test]
    fn runtime_of_reads_back_what_run_id_minted() {
        assert_eq!(runtime_of(&run_id("codex", "session-abc")), "codex");
        assert_eq!(runtime_of(&run_id(DEFAULT_RUNTIME, "")), DEFAULT_RUNTIME);
    }

    #[test]
    fn runtime_of_falls_back_to_the_whole_id_when_there_is_no_dash() {
        assert_eq!(runtime_of("myrun"), "myrun");
    }

    #[test]
    fn runtime_of_keeps_a_dashed_runtime_whole() {
        assert_eq!(runtime_of("claude-code-3edc7a52"), "claude-code");
        assert_eq!(
            runtime_of(&run_id("claude-code", "a-session")),
            "claude-code"
        );
    }

    /// The predicate is crossed against the minter itself, so it cannot drift
    /// into refusing an identity this crate produces — the failure that would
    /// be worse than the one it closes.
    #[test]
    fn run_id_shape_reads_back_what_run_id_minted() {
        for (runtime, session) in [
            (DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345"),
            ("codex", "session-abc"),
            ("claude-code", "a-session"),
            ("opencode", "ses_01KvQFLvJbDRzAWzq6GCy8B7"),
        ] {
            let id = run_id(runtime, session);
            assert_eq!(
                run_id_shape(&id),
                RunIdShape::Named { runtime },
                "{id} is an id the minter produced and the predicate refuses"
            );
        }
        assert_eq!(
            run_id_shape(&run_id("codex", "")),
            RunIdShape::Nameless { runtime: "codex" }
        );
        for session in [
            "",
            "///",
            "a",
            "abcdefghijklmnop",
            "SESSION-UPPER",
            "session_01AAAAAAAAAAAAAAAAAAAAAA",
        ] {
            let id = run_id("codex", session);
            assert!(
                matches!(
                    run_id_shape(&id),
                    RunIdShape::Named { .. } | RunIdShape::Nameless { .. }
                ),
                "the minter produced an id the predicate refuses: {id}"
            );
        }
    }

    /// `<runtime>-<session-prefix>` from the contract's
    /// own instruction, on a host whose session id carries a dash. The
    /// last-dash split reads `codex-01k4-9f2a` as runtime `codex-01k4`, a
    /// well-formed component, so the alphabet check admits it and `dev:codex-01k4`
    /// is what gets minted — invisible to the `gh issue list --label dev:codex`
    /// query the label exists for. Refused rather than derived: nothing can
    /// tell a dashed runtime from a suffix that carries a dash, so a derivation
    /// would guess, and the suffix is not a shape the minter produces.
    #[test]
    fn a_suffix_that_carries_a_dash_is_not_a_shape_run_id_mints() {
        assert_eq!(
            run_id_shape("codex-01k4-9f2a"),
            RunIdShape::Unminted(Unminted::Suffix)
        );
        // What the split would have projected, so the recorded decision is
        // pinned to the value that made it necessary.
        assert_eq!(runtime_of("codex-01k4-9f2a"), "codex-01k4");
        assert_eq!(
            run_id_shape("claude-3edc7a52-5e11"),
            RunIdShape::Unminted(Unminted::Suffix)
        );
        assert_eq!(
            run_id_shape("claude-3edc7a52"),
            RunIdShape::Named { runtime: "claude" }
        );
    }

    /// These were closed one at a time, each round complete for its own
    /// spelling. One answer refuses all of them, and names which part is not
    /// the minted shape.
    #[test]
    fn the_spellings_48_closed_one_at_a_time_are_one_class_here() {
        assert_eq!(
            run_id_shape("myrun"),
            RunIdShape::Unminted(Unminted::NoRuntime)
        );
        assert_eq!(
            run_id_shape("abcd1234"),
            RunIdShape::Unminted(Unminted::NoRuntime)
        );
        for runtime in ["-abcd1234", "My Runtime-abcd1234", "a/b-cd"] {
            assert_eq!(
                run_id_shape(runtime),
                RunIdShape::Unminted(Unminted::Runtime),
                "{runtime}"
            );
        }
        for suffix in [
            "claude-a",
            "claude-holder",
            "claude-elsewhere",
            "claude-torn0",
        ] {
            assert_eq!(
                run_id_shape(suffix),
                RunIdShape::Unminted(Unminted::Suffix),
                "{suffix}"
            );
        }
        assert_eq!(
            run_id_shape("claude-unknown"),
            RunIdShape::Nameless { runtime: "claude" }
        );
        for minted in [
            "claude-0198fe1c",
            "claude-0198fe1c3f2a9b41",
            "claude-code-60fabae1",
            "codex-abcd1234",
        ] {
            assert!(
                matches!(run_id_shape(minted), RunIdShape::Named { .. }),
                "{minted} is a shape the minter produces and the predicate refuses"
            );
        }
    }

    /// `create`'s declared runtime and the one derived from a run id are held
    /// by the same function, or the label it mints and the label a claim mints
    /// could disagree about what a runtime is.
    #[test]
    fn runtime_component_reads_a_declared_runtime_the_way_a_derived_one_is_read() {
        for runtime in ["codex", "claude-code", "opencode", "claude"] {
            assert!(runtime_component(runtime).is_ok(), "{runtime}");
        }
        for runtime in ["", "My Runtime", "a/b", "codex 01", "Codex"] {
            assert_eq!(
                runtime_component(runtime),
                Err(Unminted::Runtime),
                "{runtime}"
            );
        }
    }

    #[test]
    fn a_pointer_survives_a_round_trip() {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut run = Run::new("claude-abcd1234".to_owned());
        run.issue = Some(crate::identity::Identity::Number(12));
        run.state = Some("in-progress".to_owned());
        run.mark_verified();
        store(root.path(), &run).expect("the pointer writes");

        // Everything survives except the revision, which store moves on purpose:
        // it counts writes rather than describing the run, and a pointer that
        // came back with the number it went in with would be one no stale writer
        // could ever be told apart from.
        let read = load(root.path(), "claude-abcd1234");
        assert_eq!(read.revision, run.revision + 1);
        assert_eq!(
            Run {
                revision: run.revision,
                ..read
            },
            run
        );
    }

    #[test]
    fn a_missing_pointer_is_a_run_that_has_sworn_nothing() {
        let root = tempfile::tempdir().expect("a temporary root");
        let run = load(root.path(), "claude-nothinghere");
        assert_eq!(run.issue, None);
        assert!(!run.within_window(Duration::from_secs(600)));
    }

    #[test]
    fn a_pointer_belonging_to_another_run_is_not_read_as_ours() {
        // The file name is derived from the run-id, so this should be
        // unreachable — which is exactly why it is worth pinning. A pointer
        // that answers for the wrong run would hand one session another's
        // claim.
        let root = tempfile::tempdir().expect("a temporary root");
        let mut theirs = Run::new("claude-theirs00".to_owned());
        theirs.issue = Some(crate::identity::Identity::Number(99));
        store(root.path(), &theirs).expect("the pointer writes");
        fs::rename(
            root.path().join("claude-theirs00.json"),
            root.path().join("claude-ours0000.json"),
        )
        .expect("rename");

        let ours = load(root.path(), "claude-ours0000");
        assert_eq!(ours.issue, None, "another run's claim was adopted");
    }

    #[test]
    fn every_decision_survives_the_ledger_passing_its_cap_while_runs_are_writing() {
        // The ledger is the only evidence of what the gate decided, and every
        // run on the machine appends to it. It used to be one file rotated by
        // rename, and two runs crossing the cap together lost the history one of
        // them had just set aside — 55 of 160 decisions survived on macOS. The
        // store is one transaction per append, so under the same writers every
        // decision must come back. The wait is far past `APPEND_WAIT`: that bound
        // drops a decision on a starved machine by design, and this test measures
        // the cap, not the bound.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");

        let runs = 8;
        let each = 20;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(runs));
        std::thread::scope(|scope| {
            for run in 0..runs {
                let state = state.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    for n in 0..each {
                        append_waiting(
                            &state,
                            Record::Decision,
                            &serde_json::json!({
                                "at": 1,
                                "run_id": format!("claude-{run}"),
                                "verdict": "new",
                                "n": n,
                            }),
                            Duration::from_secs(600),
                        );
                    }
                });
            }
        });

        let records = decision_records(&state);
        let survived = records
            .iter()
            .filter(|record| record.get("verdict").and_then(Value::as_str) == Some("new"))
            .count();
        assert_eq!(
            survived,
            runs * each,
            "only {survived} of {} decisions came back, and the store is meant to keep every one",
            runs * each
        );
    }

    #[test]
    fn the_age_prune_keeps_recent_records_and_runs_at_most_once_a_day() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let now = now_seconds().expect("a clock");
        let day = 24 * 60 * 60;
        let old = now - 31 * day;
        let recent = now - 29 * day;
        record(
            &state,
            &serde_json::json!({ "at": old, "verdict": "stale" }),
        );
        record(
            &state,
            &serde_json::json!({ "at": recent, "verdict": "recent" }),
        );
        record_role(
            &state,
            &serde_json::json!({ "at": old, "agent_type": "gone" }),
        );

        maintain(&state);

        let records = decision_records(&state);
        assert_eq!(records.len(), 1, "the prune kept the wrong decisions");
        assert_eq!(
            records[0].get("verdict").and_then(Value::as_str),
            Some("recent")
        );
        assert!(
            role_records(&state).is_empty(),
            "the prune kept a stale role"
        );

        // Inside the window the marker holds it back: a record old enough to
        // prune that arrives now stays, because the interval has not elapsed.
        record(&state, &serde_json::json!({ "at": old, "verdict": "late" }));
        maintain(&state);
        assert_eq!(
            decision_records(&state).len(),
            2,
            "a second prune ran inside the interval"
        );

        // Past the interval it runs again and takes the stale record.
        let store = Store::open(&crate::transport::store::beside(&state)).expect("the store");
        let elapsed = now - 25 * 60 * 60;
        store
            .set_meta(LAST_PRUNE, &elapsed.to_string())
            .expect("the marker writes");
        maintain(&state);
        assert_eq!(
            decision_records(&state).len(),
            1,
            "the prune did not run once the interval elapsed"
        );
    }

    #[test]
    fn the_legacy_files_are_imported_once_and_then_removed() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let ledger = ledger_path(&state);
        let now = now_seconds().expect("a clock");
        std::fs::write(
            &ledger,
            format!(
                "{{\"at\":{},\"verdict\":\"allow\"}}\n{{\"at\":{},\"verdict\":\"deny\"}}\n",
                now - 60,
                now - 50
            ),
        )
        .expect("a ledger");
        std::fs::write(
            rotated(&ledger),
            format!("{{\"at\":{},\"verdict\":\"oldest\"}}\n", now - 70),
        )
        .expect("a rotated ledger");
        std::fs::write(
            roles_path(&state),
            format!("{{\"at\":{},\"agent_type\":\"explore\"}}\n", now - 40),
        )
        .expect("a role log");

        maintain(&state);

        assert!(!ledger.exists(), "the ledger file outlived the import");
        assert!(
            !rotated(&ledger).exists(),
            "the rotated ledger outlived the import"
        );
        assert_eq!(
            decision_records(&state).len(),
            3,
            "the import lost a record"
        );
        assert_eq!(role_records(&state).len(), 1, "the import lost a role");

        maintain(&state);
        assert_eq!(
            decision_records(&state).len(),
            3,
            "a second maintain imported the history twice"
        );
    }

    #[test]
    fn two_session_starts_import_the_legacy_history_once_between_them() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let ledger = ledger_path(&state);
        let now = now_seconds().expect("a clock");
        let mut lines = String::new();
        for n in 0..20 {
            lines.push_str(&format!("{{\"at\":{},\"verdict\":\"v{n}\"}}\n", now - 60));
        }
        std::fs::write(&ledger, lines).expect("a ledger");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let state = state.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    maintain(&state);
                });
            }
        });

        assert_eq!(
            decision_records(&state).len(),
            20,
            "two SessionStarts imported the legacy history twice"
        );
    }

    #[test]
    fn a_failed_import_keeps_the_legacy_file_and_counts_the_failure() {
        // The only copy of the pre-migration history is the file itself, so a
        // failed append must not take it: the import keeps the file, leaves the
        // one-time marker unset, and counts the failure `doctor` reads.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let ledger = ledger_path(&state);
        let now = now_seconds().expect("a clock");
        std::fs::write(
            &ledger,
            format!("{{\"at\":{},\"verdict\":\"allow\"}}\n", now - 60),
        )
        .expect("a ledger");

        // The table the import would write to is gone, so every append fails
        // the way a write failure would.
        Store::open(&crate::transport::store::beside(&state)).expect("the store");
        rusqlite::Connection::open(crate::transport::store::beside(&state))
            .expect("a raw connection")
            .execute_batch("DROP TABLE decisions")
            .expect("drop the decision table");

        maintain(&state);

        assert!(ledger.exists(), "a failed import deleted its only copy");
        let store = Store::open(&crate::transport::store::beside(&state)).expect("the store");
        assert!(
            store
                .meta(LEGACY_IMPORTED)
                .expect("the marker reads")
                .is_none(),
            "a failed import marked itself done"
        );
        assert!(
            store
                .record_failures()
                .expect("the failures read")
                .iter()
                .any(|(kind, count)| kind == "decisions" && *count == 1),
            "a failed import was not counted"
        );
    }

    #[test]
    fn a_run_id_never_escapes_the_state_directory() {
        // *Never* is a claim about a population, and this checked one string.
        // `pointer_path` is public and says why it sanitises again — "the next
        // caller may not mint its argument the same way" — and `--run-id` is a
        // flag, so the argument is whatever somebody typed.
        let root = Path::new("/state");
        for id in [
            "../../etc/passwd",
            "..\\..\\Windows\\System32",
            "/etc/passwd",
            "C:\\Windows\\System32\\drivers\\etc\\hosts",
            "\\\\server\\share\\x",
            "..",
            ".",
            "",
            "a/b",
            "a\0b",
            "a b",
            // Reserved Windows device names, which `transport::worktree`
            // refuses outright for a *directory* component. **Measured here
            // rather than assumed**: a pointer named `CON.json` inside a
            // subdirectory is an ordinary file — it stores and loads back with
            // its issue intact — so this needs no rule of its own, and adding
            // one on reasoning alone is what that module's own guard forbids.
            "CON",
            "NUL",
            "COM1",
            "aux",
        ] {
            let path = pointer_path(root, id);
            assert_eq!(path.parent(), Some(root), "{id:?} left the directory");
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            assert!(
                !name.contains(['/', '\\', ':', '\0']),
                "{id:?} became {name:?}, which is more than one path component"
            );
            assert!(
                name.ends_with(".json"),
                "{id:?} became {name:?}, which the reader will not find"
            );
        }
    }

    #[test]
    fn the_window_closes() {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut run = Run::new("claude-window00".to_owned());
        run.mark_verified();
        assert!(run.within_window(Duration::from_secs(600)));
        // An answer from an hour ago does not clear a write now.
        run.verified_at = run.verified_at.map(|seconds| seconds - 3600);
        assert!(!run.within_window(Duration::from_secs(600)));
        store(root.path(), &run).expect("the pointer writes");
    }

    #[test]
    fn an_answer_stamped_in_the_future_is_not_an_answer_that_is_still_fresh() {
        // The window was `now.saturating_sub(verified) < window`, and
        // `saturating_sub` floors at zero. So a verification stamped **ahead of
        // the clock** measured as nought seconds old and stayed inside the
        // window — not for a while, but for as long as the run lived. Every
        // routine write from then on rode on an answer the tracker was never
        // asked for again.
        //
        // It does not take an attacker. A clock corrected backwards is ordinary:
        // an NTP step, a VM resuming from a snapshot, a laptop whose RTC was
        // running fast, a dual-boot machine that keeps local time in hardware.
        // Any of those turns every stamp already written into a future one.
        //
        // And the arm below this one says what the answer should have been in
        // its own words — a clock that will not say what time it is means
        // *ask*, "which is the safe direction". A clock that says something
        // impossible was taking the other one.
        let mut run = Run::new("claude-ahead000".to_owned());
        run.mark_verified();
        let window = Duration::from_secs(600);

        // A minute ahead: a small skew, and already outside anything that can be
        // called a fresh answer.
        run.verified_at = run.verified_at.map(|seconds| seconds + 60);
        assert!(
            !run.within_window(window),
            "an answer from the future was counted as one from just now"
        );

        // An hour ahead — what a real clock correction leaves behind — and the
        // window would never close again for the life of the run.
        run.verified_at = run.verified_at.map(|seconds| seconds + 3600);
        assert!(
            !run.within_window(window),
            "the window never closes once a stamp is ahead of the clock"
        );

        // The floor: the window still opens for an answer that really is fresh,
        // so a fix that simply answered `false` would not pass here.
        run.mark_verified();
        assert!(
            run.within_window(window),
            "an answer from just now stopped clearing a routine write"
        );

        // And the report says the same thing the gate now does. `silence` held
        // its own `saturating_sub`, so `status` printed `last answer just now`
        // for a stamp the window had already stopped believing — the report and
        // the gate disagreeing about one field.
        assert_eq!(silence(&run).said(), "just now");
        run.verified_at = run.verified_at.map(|seconds| seconds + 3600);
        assert_eq!(
            silence(&run).said(),
            "stamped ahead of this clock",
            "the report still claimed a fresh answer the gate had stopped trusting"
        );
        assert_eq!(
            silence(&run).seconds(),
            None,
            "a number was given for a time that has not elapsed"
        );
        // Not `never verified`, which is what `None` renders as: this run **was**
        // verified, and only its clock moved. Flattening the two would be the
        // same fault one field over.
        assert_ne!(
            silence(&run).said(),
            silence(&Run::new("x".to_owned())).said()
        );
        assert_eq!(silence(&Run::new("x".to_owned())).said(), "never verified");
    }

    #[test]
    fn forgetting_a_run_leaves_no_pointer() {
        let root = tempfile::tempdir().expect("a temporary root");
        let run = Run::new("claude-forgetme".to_owned());
        store(root.path(), &run).expect("the pointer writes");
        forget(root.path(), &run.run_id);
        assert_eq!(load(root.path(), &run.run_id).verified_at, None);
    }

    #[test]
    fn a_load_racing_a_store_never_sees_a_run_that_swore_nothing() {
        // The gate reads the pointer on every tool call and writes it back on
        // every allowed one, so an agent making parallel calls has a store and a
        // load on the same file at the same time as its ordinary state. With a
        // truncating write, the reader's parse fails and `load` answers with a
        // fresh run — no issue, so `gate` says the run is outside its authority
        // and the write goes through. The gate opens for the length of a
        // truncation, and nothing anywhere records that it did.
        let root = tempfile::tempdir().expect("a temporary root");
        let root = root.path().to_path_buf();
        let mut run = Run::new("claude-racing00".to_owned());
        run.issue = Some(crate::identity::Identity::Number(9));
        store(&root, &run).expect("the first store lands");

        let writing = {
            let root = root.clone();
            std::thread::spawn(move || {
                for _ in 0..400 {
                    store(&root, &run).expect("a store lands");
                }
            })
        };

        for _ in 0..400 {
            let seen = load(&root, "claude-racing00");
            assert_eq!(
                seen.issue,
                Some(crate::identity::Identity::Number(9)),
                "a load caught the pointer mid-write and read it as unsworn"
            );
        }
        writing.join().expect("the writer finishes");

        // And nothing is left beside it: a directory filling with temporaries
        // is a second failure nobody would think to look for.
        let stray: Vec<_> = std::fs::read_dir(&root)
            .expect("the directory is readable")
            .flatten()
            .map(|entry| entry.file_name())
            .filter(|name| name.to_string_lossy() != "claude-racing00.json")
            .collect();
        assert!(stray.is_empty(), "left behind: {stray:?}");
    }

    #[test]
    fn the_state_root_is_under_a_home_rather_than_wherever_the_process_stands() {
        // `state_root -> Ok(Default::default())` survived: an empty path makes
        // every run pointer relative to the working directory, so a run's oath
        // would follow whoever changed directory and be invisible from anywhere
        // else. The gate would read no claim and stand aside.
        let root = state_root(None).expect("a state root");
        assert!(root.is_absolute(), "{} is not absolute", root.display());
        assert!(root.ends_with("runs"), "{}", root.display());
    }

    #[test]
    fn an_operation_id_is_distinct_across_the_things_it_is_meant_to_separate() {
        // The doc says the key's only job is to be distinct. Nothing held it to
        // that: replacing the hash's `^=` with `&=` or `|=` left the suite green,
        // and both collapse the run-id's contribution — so two runs claiming the
        // same issue in the same nanosecond would share an idempotency key, which
        // is a retry writing over somebody else's operation.
        let ids: Vec<String> = (0..64)
            .map(|n| mint_operation_id(&format!("claude-{n:08}"), 7))
            .collect();
        assert!(ids.iter().all(|id| id.len() == 32));

        // The **second** half, on its own. The first sixteen digits are the
        // nanosecond, which separates these calls whatever the hash does — so a
        // set over the whole key stayed full even with the run-id's contribution
        // thrown away, and the test passed while proving nothing. Two runs that
        // claim in the same nanosecond are separated by this half and by nothing
        // else.
        let tails: std::collections::BTreeSet<&str> = ids.iter().map(|id| &id[16..]).collect();
        assert_eq!(
            tails.len(),
            64,
            "sixty-four run-ids produced {} distinct keys: the run-id is not reaching the key",
            tails.len()
        );

        // And the issue reaches it too. One run claiming two issues in the same
        // nanosecond is separated by this and by nothing else — the seed folds
        // the issue in before the run-id does its rounds, and an `&` there drops
        // it silently for every issue at once.
        let per_issue: std::collections::BTreeSet<String> = (1..64u64)
            .map(|issue| mint_operation_id("claude-oneruns0", issue)[16..].to_owned())
            .collect();
        assert_eq!(
            per_issue.len(),
            63,
            "sixty-three issues produced {} distinct keys: the issue is not reaching the key",
            per_issue.len()
        );
    }

    #[test]
    fn forgetting_a_run_removes_its_pointer() {
        // `forget` replaced with `()` survived. `SessionEnd` calls it, so the
        // pointer would outlive every session that ever ran — and `status` would
        // list runs that ended, from a directory that only grows.
        let root = tempfile::tempdir().expect("a temporary root");
        let mut run = Run::new("claude-ending00".to_owned());
        run.issue = Some(crate::identity::Identity::Number(3));
        store(root.path(), &run).expect("the pointer lands");
        assert_eq!(
            load(root.path(), "claude-ending00").issue,
            Some(crate::identity::Identity::Number(3))
        );

        forget(root.path(), "claude-ending00");
        assert_eq!(load(root.path(), "claude-ending00").issue, None);
        assert!(holdings(root.path()).is_empty());
    }

    #[test]
    fn a_writer_with_old_news_does_not_overwrite_a_fresher_pointer() {
        // The threat a review-authority model calls "concurrent or
        // stale writer", and the field that makes it dangerous here is
        // `worktree`. A hook loads the pointer, allows a write and stores; if
        // `start-branch` recorded the isolated checkout in between, the hook's
        // store used to erase it — and a write inside that worktree then passed
        // as `Outside`, which is the hole that was closed once already.
        let root = tempfile::tempdir().expect("a temporary root");
        let root = root.path();

        let mut sworn = Run::new("claude-stale000".to_owned());
        sworn.issue = Some(crate::identity::Identity::Number(7));
        store(root, &sworn).expect("the claim lands");

        // What a second process is holding: loaded before the worktree existed.
        let stale = load(root, "claude-stale000");

        // Meanwhile the isolated checkout is recorded.
        let mut fresh = load(root, "claude-stale000");
        fresh.worktree = Some(root.join("trees").join("issue-7"));
        store(root, &fresh).expect("the worktree lands");

        // The stale writer stores what it had. It must not win.
        store(root, &stale).expect("a stale store is not an error");
        let after = load(root, "claude-stale000");
        assert!(
            after.worktree.is_some(),
            "a writer with old news erased the isolated checkout"
        );
        assert_eq!(after.issue, Some(crate::identity::Identity::Number(7)));

        // And the ordinary path still works: a writer holding the current
        // revision writes, and the revision moves.
        let mut current = load(root, "claude-stale000");
        current.state = Some("review".to_owned());
        store(root, &current).expect("a current store lands");
        let after = load(root, "claude-stale000");
        assert_eq!(after.state.as_deref(), Some("review"));
        assert!(after.revision > current.revision);
    }
}

#[cfg(test)]
mod stamp_tests {
    #[test]
    fn a_count_of_seconds_is_written_the_way_every_other_stamp_is() {
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00Z"),
            (951_782_400, "2000-02-29T00:00:00Z"),
            (1_078_012_800, "2004-02-29T00:00:00Z"),
            (1_709_164_800, "2024-02-29T00:00:00Z"),
            (1_785_904_685, "2026-08-05T04:38:05Z"),
            (4_102_444_800, "2100-01-01T00:00:00Z"),
        ] {
            assert_eq!(
                super::stamp_of(seconds),
                expected,
                "{seconds} is not the day the binding says it is"
            );
        }
    }
}

#[cfg(test)]
mod ages {
    #[test]
    fn an_age_is_written_the_way_somebody_reads_it() {
        for (seconds, expected) in [
            (0, "just now"),
            (59, "just now"),
            (60, "1 min ago"),
            (90, "1 min ago"),
            (3_599, "59 min ago"),
            (3_600, "1 h ago"),
            (7_200, "2 h ago"),
            (86_399, "23 h ago"),
            (86_400, "1 day ago"),
            (259_200, "3 days ago"),
            (34_560_000, "400 days ago"),
        ] {
            assert_eq!(
                super::age(seconds),
                expected,
                "{seconds} seconds is not what a person would call it"
            );
        }
    }
}

#[cfg(test)]
mod store_outcome_tests {
    use super::*;

    #[test]
    fn a_pointer_that_could_not_be_written_says_so() {
        let root = tempfile::tempdir().expect("a temporary state root");

        // The floor: an ordinary update lands and says it landed.
        let (run, on_disk) = updated(root.path(), "claude-aaaa1111", |run| {
            run.issue = Some(crate::identity::Identity::Number(7))
        });
        assert!(on_disk, "an ordinary pointer write reported failure");
        assert_eq!(run.issue, Some(crate::identity::Identity::Number(7)));
        assert_eq!(
            load(root.path(), "claude-aaaa1111").issue,
            Some(crate::identity::Identity::Number(7)),
            "the pointer this said it wrote is not on disk"
        );

        // A directory where the pointer file has to go: the read cannot
        // succeed, on every platform, so the update refuses before anything is
        // written, and the change is never applied to the run in hand.
        let blocked = pointer_path(root.path(), "claude-bbbb2222");
        std::fs::create_dir_all(&blocked).expect("something unreadable in its place");
        let (run, on_disk) = updated(root.path(), "claude-bbbb2222", |run| {
            run.issue = Some(crate::identity::Identity::Number(9))
        });
        assert!(
            !on_disk,
            "an unreadable pointer was reported as having taken an update"
        );
        assert!(
            run.unreadable,
            "the update answered a fresh run for a pointer it could not read"
        );
        assert!(
            run.unreadable_reason
                .as_deref()
                .is_some_and(|why| why.contains("claude-bbbb2222.json")),
            "the update named no pointer path: {:?}",
            run.unreadable_reason
        );
        assert!(blocked.is_dir(), "the unreadable thing was written over");

        // And a pointer of bytes nobody can parse is the same answer: nothing
        // was applied.
        let path = pointer_path(root.path(), "claude-cccc3333");
        fs::write(&path, "esto no parsea").expect("a pointer that will not parse");
        let (_, on_disk) = updated(root.path(), "claude-cccc3333", |run| {
            run.issue = Some(crate::identity::Identity::Number(11))
        });
        assert!(
            !on_disk,
            "an unreadable pointer was reported as having taken an update"
        );
    }

    #[test]
    fn a_stop_after_request_round_trips_and_is_cleared() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        let run_id = run_id(DEFAULT_RUNTIME, "session-stop-after");

        assert_eq!(
            stop_after(&state, &run_id),
            None,
            "an unrequested run answered a request"
        );

        set_stop_after(&state, &run_id, Some("the operator asked")).expect("the request writes");
        assert_eq!(
            stop_after(&state, &run_id).as_deref(),
            Some("the operator asked")
        );

        clear_stop_after(&state, &run_id);
        assert_eq!(stop_after(&state, &run_id), None);
        clear_stop_after(&state, &run_id);
    }

    #[test]
    fn a_stop_after_request_with_no_reason_records_the_default() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        let run_id = run_id(DEFAULT_RUNTIME, "session-stop-after-default");

        set_stop_after(&state, &run_id, None).expect("the request writes");
        assert_eq!(
            stop_after(&state, &run_id).as_deref(),
            Some(STOP_AFTER_DEFAULT)
        );
    }

    #[test]
    fn a_stop_after_request_nobody_can_read_is_not_a_request() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("state");
        let run_id = run_id(DEFAULT_RUNTIME, "session-stop-after-unreadable");
        let path = stop_after_path(&state, &run_id);
        std::fs::create_dir_all(&path).expect("a directory in the file's place");

        assert_eq!(
            stop_after(&state, &run_id),
            None,
            "an unreadable request was read as one"
        );
    }

    #[test]
    fn a_stop_after_request_sits_beside_the_run_pointers() {
        let root = tempfile::tempdir().expect("a temporary root");
        let state = state_root(Some(root.path())).expect("a state root");
        assert_eq!(
            stop_after_path(&state, "claude-aaaa1111"),
            root.path()
                .join(".estigia")
                .join("stop-after")
                .join("claude-aaaa1111.json")
        );
    }
}
