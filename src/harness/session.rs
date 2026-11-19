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
pub const AGENT_RUNTIMES: &[(&str, &str)] = &[("codex", "codex"), ("cursor", "cursor")];

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
    /// the work happens*. Written by `claim` with `branch`, or by a `claim` or renewal
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
    /// matched fragment, the verb, or the tool. A write the claim does not
    /// cover, to an absolute path outside every covered checkout, is not
    /// recorded. A write before this run's
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
    /// The base an assisted run's branch was started from: the one ref its
    /// worktree may merge in.
    #[serde(default)]
    pub base: Option<String>,
    /// The session run this assisted claim is one of several for. A child is an
    /// ordinary single-claim pointer; the hook reaches it through
    /// [`children`] by where a call lands.
    #[serde(default)]
    pub parent: Option<String>,
    /// What kind of claim this pointer records. An unknown spelling fails the
    /// parse, so a pointer no build can read denies rather than reading as a
    /// run that holds nothing.
    #[serde(default)]
    pub kind: RunKind,
    /// The pull request a [`RunKind::ReviewOnly`] run reviews. Never in
    /// [`Run::issue`]: every reader of that field asks the tracker about an issue.
    #[serde(default)]
    pub reviewed_pr: Option<u64>,
    /// What this assisted run last pushed through `draft_pr`, and how many
    /// corrections that makes. On the pointer so a resumed run keeps the count.
    #[serde(default)]
    pub assisted_publication: Option<AssistedPublication>,
    /// The repositories whose hold on an unclaimed write this run's `SessionStart` announced.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub announced_holds: BTreeSet<PathBuf>,
    /// The horizon the held claim was sworn with, which taking it back from the parked list must
    /// repeat: the claim's operation id is reused, and its marker records this horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<String>,
    /// Claims this run still holds in the store and has set aside while it works another:
    /// built work that waits only for the git operation window. Never covered by the gate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parked: Vec<ParkedClaim>,
}

/// One claim a run has set aside, with what its pointer said about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParkedClaim {
    /// The parked issue.
    pub issue: crate::identity::Identity,
    /// The workflow state the run believed it was in.
    pub state: Option<String>,
    /// The checkout holding its uncommitted work.
    pub worktree: Option<PathBuf>,
    /// The claim's idempotency key, so taking it back is a renewal.
    pub operation_id: Option<String>,
    /// The release key reserved for it.
    pub release_id: Option<String>,
    /// The review transaction it may spend.
    pub review_transaction: Option<crate::transport::claim::ReviewTransaction>,
    /// The head its review target was bound to.
    pub reviewed_head: Option<String>,
    /// The horizon it was sworn with.
    #[serde(default)]
    pub horizon: Option<String>,
}

/// One branch's publication under an assisted run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistedPublication {
    /// The branch that was pushed.
    pub branch: String,
    /// The head that landed last.
    pub head: String,
    /// New heads pushed after the first.
    pub corrections: u32,
}

/// What a run's pointer claims.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunKind {
    /// An issue, or nothing yet: the claim that delivers.
    #[default]
    Delivery,
    /// One pull request this run did not build, reviewed and never changed.
    ReviewOnly,
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
            base: None,
            parent: None,
            kind: RunKind::Delivery,
            reviewed_pr: None,
            assisted_publication: None,
            announced_holds: BTreeSet::new(),
            horizon: None,
            parked: Vec::new(),
        }
    }

    /// Whether this pointer is a review-only claim on a pull request.
    pub fn is_review_only(&self) -> bool {
        self.kind == RunKind::ReviewOnly
    }

    /// What this run last pushed to `branch`, when it pushed to that branch at all.
    pub fn published_on(&self, branch: &str) -> Option<&AssistedPublication> {
        self.assisted_publication
            .as_ref()
            .filter(|published| published.branch == branch)
    }

    /// Records that `head` landed on `branch`: the first head is the original,
    /// each different one is a correction, and the same head again is nothing.
    pub fn record_assisted_push(&mut self, branch: &str, head: &str) {
        let corrections = match self.published_on(branch) {
            Some(published) if published.head == head => return,
            Some(published) => published.corrections.saturating_add(1),
            None => 0,
        };
        self.assisted_publication = Some(AssistedPublication {
            branch: branch.to_owned(),
            head: head.to_owned(),
            corrections,
        });
    }

    /// The issue number, when this run holds an issue and its tracker numbers
    /// issues. `None` for a Linear key: those operations are GitHub-only.
    pub fn issue_number(&self) -> Option<u64> {
        self.issue
            .as_ref()
            .and_then(crate::identity::Identity::number)
    }

    /// Whether `issue` is one of this run's parked claims.
    pub fn holds_parked(&self, issue: u64) -> bool {
        self.parked
            .iter()
            .any(|parked| parked.issue.number() == Some(issue))
    }

    /// Sets the held claim aside and makes `issue` the active one: brought back
    /// from the parked list when it is there, and otherwise left for the claim
    /// that follows to fill in.
    pub fn park_for(&mut self, issue: u64) {
        let held = self.issue.take().map(|held| ParkedClaim {
            issue: held,
            state: self.state.take(),
            worktree: self.worktree.take(),
            operation_id: self.operation_id.take(),
            release_id: self.release_id.take(),
            review_transaction: self.review_transaction.take(),
            reviewed_head: self.reviewed_head.take(),
            horizon: self.horizon.take(),
        });
        if let Some(index) = self
            .parked
            .iter()
            .position(|parked| parked.issue.number() == Some(issue))
        {
            let back = self.parked.remove(index);
            self.issue = Some(back.issue);
            self.state = back.state;
            self.worktree = back.worktree;
            self.operation_id = back.operation_id;
            self.release_id = back.release_id;
            self.review_transaction = back.review_transaction;
            self.reviewed_head = back.reviewed_head;
            self.horizon = back.horizon;
        }
        self.parked.extend(held);
        self.verified_at = None;
    }

    /// Empties the active claim's slot, leaving the parked list as it is.
    pub fn clear_active(&mut self) {
        self.issue = None;
        self.state = None;
        self.worktree = None;
        self.operation_id = None;
        self.release_id = None;
        self.review_transaction = None;
        self.reviewed_head = None;
        self.horizon = None;
        self.verified_at = None;
    }

    /// Puts back the active and parked claims `before` held, undoing a
    /// [`Run::park_for`] whose claim did not land.
    pub fn restore_claims(&mut self, before: &Run) {
        self.issue.clone_from(&before.issue);
        self.state.clone_from(&before.state);
        self.worktree.clone_from(&before.worktree);
        self.operation_id.clone_from(&before.operation_id);
        self.release_id.clone_from(&before.release_id);
        self.review_transaction
            .clone_from(&before.review_transaction);
        self.reviewed_head.clone_from(&before.reviewed_head);
        self.horizon.clone_from(&before.horizon);
        self.verified_at = before.verified_at;
        self.parked.clone_from(&before.parked);
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
    let _held = pointers_locked(root);
    store_locked(root, run)
}

fn store_locked(root: &Path, run: &Run) -> Result<bool, Refusal> {
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
/// Holding the lock, the first attempt wins. The second is for a lock that could
/// not be had in [`POINTERS_LOCK_WAIT`], where the revision check is all there is.
const UPDATE_ATTEMPTS: usize = 2;

const POINTERS_LOCK: &str = "pointers.lock";

/// Reading a pointer and writing it back takes a moment, so a longer wait means
/// a stopped holder, and a gate call must not hang on one.
const POINTERS_LOCK_WAIT: Duration = Duration::from_secs(2);

/// The one file every pointer write locks.
pub fn pointers_lock_path(root: &Path) -> PathBuf {
    root.join(POINTERS_LOCK)
}

// One file for every pointer: it is never removed, so no two writers can hold
// different locks under one name. Dropping the handle unlocks it.
fn pointers_locked(root: &Path) -> Option<fs::File> {
    fs::create_dir_all(root).ok()?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(pointers_lock_path(root))
        .ok()?;
    let started = std::time::Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Some(file),
            Err(fs::TryLockError::WouldBlock) if started.elapsed() < POINTERS_LOCK_WAIT => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    }
}

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
    let _held = pointers_locked(root);
    let mut run = load(root, run_id);
    for _ in 0..UPDATE_ATTEMPTS {
        // An unreadable pointer is not a fresh one, and writing over it would
        // throw away whatever a person still has to read out of it.
        if run.unreadable {
            return (run, false);
        }
        change(&mut run);
        match store_locked(root, &run) {
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
    let _held = pointers_locked(root);
    let _ = fs::remove_file(pointer_path(root, run_id));
}

/// Forgets the claim a release put down and keeps the rest: a parked claim
/// released alone leaves the active one, and the active one released leaves the
/// parked list. The pointer is removed only when nothing is left on it.
pub fn forget_released(root: &Path, run_id: &str, released: Option<u64>) {
    let _held = pointers_locked(root);
    let mut run = load(root, run_id);
    if released.is_some_and(|issue| !run.holds_parked(issue) && run.issue_number() != Some(issue)) {
        return;
    }
    let parked_alone =
        released.is_some_and(|issue| run.holds_parked(issue) && run.issue_number() != Some(issue));
    if parked_alone {
        run.parked
            .retain(|parked| parked.issue.number() != released);
    } else {
        run.clear_active();
    }
    if run.unreadable || (run.issue.is_none() && run.parked.is_empty()) {
        let _ = fs::remove_file(pointer_path(root, run_id));
        return;
    }
    if !matches!(store_locked(root, &run), Ok(true)) && run.parked.is_empty() {
        let _ = fs::remove_file(pointer_path(root, run_id));
    }
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

/// How many decisions one repository keeps, newest first, beside the age limit.
pub const DECISIONS_PER_REPOSITORY: u64 = 10_000;

/// How long a job directory is kept, by its own modification time.
pub const JOB_RETENTION_DAYS: u64 = 30;

/// How many finished job directories one repository keeps, newest first.
pub const JOBS_PER_REPOSITORY: usize = 500;

/// How long past its deadline a `running` job with no held lock may stand.
pub(crate) const JOB_SETTLE_SECONDS: u64 = 24 * 60 * 60;

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
        let _ = store.note_record_failure(kind, now_seconds());
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

/// Imports any legacy JSONL history once, then runs the daily bounds and the
/// running-job sweep at most once a day.
///
/// Called from `SessionStart` so none of it lands on a write's critical path.
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
    let _ = store.prune_failures(cutoff);
    let _ = store.cap_decisions(DECISIONS_PER_REPOSITORY, crate::harness::doctor::UNDECIDED);
    let jobs = crate::transport::store::jobs_beside(&path);
    let _ = store.bound_jobs(&jobs, now);
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
    let now = now_seconds();
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
                let _ = store.note_record_failure(kind, now);
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
                let _ = store.note_record_failure(*kind, now);
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
    pointers_where(root, |run| run.issue.is_some())
}

/// Every pointer holding parked claims, active issue or not.
pub fn parked_holdings(root: &Path) -> Vec<Run> {
    pointers_where(root, |run| !run.parked.is_empty())
}

/// Every pointer holding a review-only claim. Apart from [`holdings`] because
/// such a run holds no issue and is no evidence for anything that delivers.
pub fn review_holdings(root: &Path) -> Vec<Run> {
    pointers_where(root, Run::is_review_only)
}

fn pointers_where(root: &Path, wanted: impl Fn(&Run) -> bool) -> Vec<Run> {
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
        .filter(|run| wanted(run))
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

/// The id of one of `parent`'s several assisted claims, stable across a retry.
pub fn child_run_id(parent: &str, issue: &crate::identity::Identity, branch: &str) -> String {
    run_id(runtime_of(parent), &format!("{parent}:{issue}:{branch}"))
}

/// The assisted claims `parent` holds through child pointers. A pointer that
/// cannot be read is nobody's child, so a call that would land in it stays on
/// the parent's policy.
pub fn children(root: &Path, parent: &str) -> Vec<Run> {
    holdings(root)
        .into_iter()
        .filter(|run| run.parent.as_deref() == Some(parent))
        .collect()
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
mod tests;

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
