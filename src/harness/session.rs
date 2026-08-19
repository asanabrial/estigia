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

/// The runtime prefix in a run-id, when the harness cannot name a better one.
pub const DEFAULT_RUNTIME: &str = "claude";

/// One run's pointer into the tracker.
///
/// Every field is either derived from the agent's own session or a note about
/// when Estigia last asked the tracker something. None of it is authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// `<runtime>-<session-prefix>`, minted once and reused for every write.
    pub run_id: String,
    /// The issue this run holds, once it has claimed one.
    pub issue: Option<u64>,
    /// How many times this pointer has been written.
    ///
    /// The "expected revision" a stale writer fails against. Two processes share
    /// one pointer — a hook storing after an allowed write, a tool recording a
    /// claim — and the loser of that race used to overwrite the winner with what
    /// it had loaded *before*. The dangerous field is `worktree`: a hook that
    /// loaded before `start-branch` ran, and stored after, erased it. `covered`
    /// then no longer contained the isolated checkout, and a write inside it
    /// passed as `Outside` — the exact hole that was closed once already,
    /// reopened by a writer with old news.
    ///
    /// `#[serde(default)]` so a pointer written by an earlier build loads as
    /// revision zero rather than failing to parse, which would deny every write
    /// for that run on the first upgrade.
    #[serde(default)]
    pub revision: u64,
    /// A pointer that is on disk and could not be read.
    ///
    /// Not serialised: it describes the read, not the run. `load` used to answer
    /// an unparseable pointer with a fresh run — no issue, so `gate` read it as
    /// a run that swore nothing and stood aside. "An unknown result is not
    /// clearance" is one of the three rules the directive states, and this was
    /// the place the code said otherwise.
    ///
    /// The case that makes it more than corruption: a release that changes this
    /// struct makes every pointer written by the last one unreadable, and the
    /// gate would open for every run in flight at exactly the moment nobody is
    /// looking for it.
    #[serde(skip)]
    pub unreadable: bool,
    /// Why the read failed, when it did: the pointer path and the error.
    ///
    /// Not serialised, like [`Self::unreadable`]: it describes the read, not
    /// the run. Carried so the refusal a gate or tool builds can name the
    /// **file** the operator has to look at — an unreadable pointer that names
    /// no path is one a person cannot find to read or take away, and the
    /// resolutions already in place tell them to do exactly that. A read that
    /// failed because the pointer was simply not there needs no reason: that is
    /// the ordinary state, and it does not reach this field.
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
    /// The isolated checkout this run works in, once `start_branch` made one.
    ///
    /// Recorded because it is *where the work happens*. A claim is made in the
    /// base checkout and the delivery is written in a worktree somewhere else
    /// entirely, so a gate scoped only to `repo_dir` covers the one directory
    /// the run does not edit — and lets the whole delivery through ungated.
    #[serde(default)]
    pub worktree: Option<PathBuf>,
    /// Unix seconds of the last answer from the tracker.
    pub verified_at: Option<u64>,
    /// The idempotency key this run minted for its claim.
    ///
    /// Kept because the transport documents `claim` as *"fresh 32 lowercase hex
    /// chars; reuse on retry"* — a retry after a timeout with a fresh id is a
    /// second claim event rather than the same one, which is exactly the
    /// duplicate the key exists to prevent.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// The operation id this run's **release** is reserved under.
    ///
    /// A slot of its own, and not the one above: an operation id reserves one
    /// kind, so handing a release the id a claim already reserved is a conflict
    /// the transport refuses by name.
    ///
    /// `unassign` used to mint a fresh id on every call and store none, beside
    /// the two lines that reuse one for `claim` and `reclaim` and say why. It is
    /// the same why: `cmd_unassign` answers a retry from the marker already on
    /// the issue — that whole path, including the only refusal that tells an
    /// operator their target is not one anybody can end — and a second call
    /// carrying a different id never reaches it. The retry the taxonomy asks
    /// for after an ambiguous write posted a second release comment instead.
    #[serde(default)]
    pub release_id: Option<String>,
    /// The complete immutable publication receipt this run may spend.
    ///
    /// A head identifies bytes only after the pull request lineage has been
    /// selected, so all five receipt fields are persisted together.
    #[serde(default)]
    pub review_receipt: Option<crate::transport::claim::ReviewReceipt>,
    /// The head `publish_review` bound the review target to, when it did.
    ///
    /// Recorded because the rule the whole product is named for — *a verdict is
    /// bound to exact bytes; every push invalidates it* — was enforced where it
    /// is **written** and not where it is **used**. The transport binds the
    /// publication and reads it back; the boundary that delivers on it asked
    /// only whether the claim was still live, which it can be while the bytes
    /// under it have moved.
    ///
    /// Only what this run published through Estigia's own tools. A run that
    /// shells out to the transport is not seen here, which is why the honesty
    /// contract states the reach rather than the rule.
    #[serde(default)]
    pub reviewed_head: Option<String>,
    /// The roles this run's delegated contexts actually ran as.
    ///
    /// **Observed, not declared.** Claude Code sends `agent_type` on every tool
    /// event fired inside a sub-agent, so this is what the gate *saw* rather than
    /// what an orchestrator said it launched.
    ///
    /// **Within this run only.** The pointer is keyed on a run id derived from the
    /// session id, and `SessionEnd` removes it, so nothing outside this session
    /// ever reads this set and nothing reads it afterwards. Issue 83 asked for a
    /// record a *later* run could read and this is not that; #91 owns it, and
    /// `docs/honesty.md` states the gap with the measurement that found it rather
    /// than leaving this field to imply more than it does.
    ///
    /// What it does not carry, stated here because the field invites the wider
    /// reading: a context refused at the role gate before any call is **not**
    /// here, because the gate returns at the role check before this point — which
    /// is the honest shape, since a launch that is refused contributes no judge.
    /// The reason is the early return and **not** the store’s condition; that
    /// condition is `saw_new_role || Allow`, and keying it on `Allow` alone is the
    /// version that failed its own test. Nor is a host that does not send
    /// `agent_type` visible at all.
    ///
    /// A set, so re-entering one role does not grow the file, and ordered, so two
    /// runs that saw the same roles write the same bytes.
    #[serde(default)]
    pub roles: BTreeSet<String>,
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
            review_receipt: None,
            reviewed_head: None,
        }
    }

    /// Whether the last verification is still inside the renewal window.
    ///
    /// Never consulted for an irreversible boundary — see
    /// [`crate::harness::Sensitivity`]. This only decides whether a routine
    /// write may ride on the answer the previous one already paid for.
    pub fn within_window(&self, window: Duration) -> bool {
        match (self.verified_at, now_seconds()) {
            // `checked_sub`, not `saturating_sub`. Saturating floors at zero, so
            // a stamp **ahead of the clock** measured as nought seconds old and
            // stayed inside the window for as long as the run lived — every
            // routine write from then on riding on an answer the tracker was
            // never asked for again. It takes no attacker: an NTP step, a VM
            // resumed from a snapshot, an RTC that was running fast, a dual-boot
            // machine keeping local time in hardware. Any of those turns every
            // stamp already written into a future one.
            //
            // `None` here is a time that cannot have elapsed, and it joins the
            // arm below rather than getting an answer of its own: a clock saying
            // something impossible is a clock that will not say what time it is.
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
}

/// Mints the run identity from the agent's session.
///
/// `<runtime>-<session-prefix><digest>`, which is the shape `SKILL.md` requires:
/// bounded runtime attribution in the labels, unbounded run-id attribution in
/// the text. It is derived rather than random so that two invocations of the
/// hook inside one session agree without having to write anything down first.
///
/// # Why the digest is not decoration
///
/// This was the first eight alphanumerics of the session id and nothing else,
/// which is safe only if session ids differ inside their first eight — a
/// property of **somebody else's** id format, claimed here and crossed against
/// nothing. `session_01KvQFLvJbDRzAWzq6GCy8B7` and
/// `session_01ZZZZZZZZZZZZZZZZZZZZZZ` both minted `claude-session0`, and that
/// is a real shipping shape rather than a contrivance.
///
/// Two runs sharing an identity share a **pointer**, and the pointer is what
/// says which issue a run holds. Measured: a second session loaded the first
/// one's pointer and read `issue = Some(42)` — it believed it held a claim
/// nobody had given it, and the gate answers from that belief. Of everything
/// that can go wrong here, an oath quietly transferred is the worst.
///
/// The prefix stays, because a run id is read by people in issue comments and
/// `claude-session0…` tells them more than a bare hash would. What the digest
/// adds is that the *identity* depends on the whole session id rather than on
/// its first eight characters.
///
/// # Why not the standard hasher
///
/// `DefaultHasher` is deterministic within a Rust release and not guaranteed
/// across them. This value is written to disk and read back by a later build,
/// so a hasher that may move is a pointer that stops matching its own run after
/// a toolchain upgrade — every write denied until somebody re-claims. FNV-1a is
/// eight lines and never moves.
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

/// Whether this run id names a run at all.
///
/// `<runtime>-unknown` is **a name two runs share**, not a run: every session
/// this build cannot identify is handed the same one. It is fine as a label —
/// the ledger records under it, and a decision has to be filed somewhere — and
/// it is not fine as an identity, because an oath is bound to one.
///
/// Two sessions swearing under it means the second overwrites the first's
/// pointer, and the gate then measures one run's writes against the other's
/// issue. The `--run-id` flag's own help gives the rule this breaks: *a claim
/// recorded under the wrong run-id is a claim the gate will never match, and
/// being asked beats being silently wrong.*
pub fn is_nameless(run_id: &str) -> bool {
    run_id
        .rsplit_once('-')
        .is_some_and(|(runtime, tail)| !runtime.is_empty() && tail == NAMELESS)
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
    // Run-ids are minted from an alphanumeric filter above, so this cannot walk
    // out of the directory. Sanitised again anyway: the function is public and
    // the next caller may not mint its argument the same way.
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
    // every pointer already on disk keeps the file it is in.
    if !run_id.is_empty()
        && run_id.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return root.join(format!("{safe}.json"));
    }
    // Anything else carries a digest of what was actually asked for, because the
    // line above **folds**: every character outside the set becomes `_`, and a
    // `_` is one of the things it becomes. So `claude/aaaa` and `claude_aaaa`
    // were one file, and on a case-insensitive filesystem so were
    // `claude-aaaa1111` and `Claude-AAAA1111`. Measured through the running
    // server, two ids and one record:
    //
    // ```text
    // claude-aaaa1111 -> wrote claude-aaaa1111.json
    // Claude-AAAA1111 -> "this run's record exists and cannot be read"
    // ```
    //
    // It read perfectly; it belonged to another run. The transport states this
    // rule for its own use of a run id and refuses rather than folds — *any
    // transformation that could fold two distinct IDs into one is refused rather
    // than applied* — and this side applied one. A pointer is not authority, but
    // it is what says which issue a run believes it holds, and two runs sharing
    // one is the shape the `<runtime>-unknown` note above already calls out: the
    // gate measures one run's writes against the other's issue.
    //
    // Separated by `_`, which a name that maps to itself can never contain, so a
    // digested name cannot collide with an undigested one either.
    root.join(format!("{safe}_{:08x}.json", fnv1a(run_id)))
}

/// Reads this run's pointer, or a fresh one.
///
/// Only a pointer that is **not there** is a run that has sworn nothing. A
/// pointer that is there and cannot be read — a directory at its path, a
/// permission failure, a transient I/O error — says a run under this name
/// existed and not what it swore, so it loads as *unreadable*, never as fresh:
/// the gate that consults it refuses that state rather than treating "nothing
/// held" as outside its authority. Issue #38 measured what the collapse cost:
/// the installer took a live pointer, and the run kept its claim while every
/// write passed through ungated, because the missing record read as an unsworn
/// run and nothing anywhere said so.
pub fn load(root: &Path, run_id: &str) -> Run {
    let path = pointer_path(root, run_id);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        // No pointer at all: this run has sworn nothing, which is the ordinary
        // state and the one the oath model is built on. `NotFound` is the only
        // read failure that may answer this — every other way the read fails
        // proves nothing about whether the pointer exists.
        //
        // And `NotFound` only answers it while the directory the pointer would
        // be in is one. Windows answers a path whose parent is **not** a
        // directory with `ERROR_PATH_NOT_FOUND`, which std maps to `NotFound`
        // — measured, os error 3 — so a state root that is a file reads as
        // *every run swore nothing* and stands every gate aside at once. That
        // is this issue's own failure arriving through the arm meant to be
        // safe, so absence is confirmed against the root rather than taken from
        // the error kind. A root that is not there at all is still absence: it
        // is what a machine looks like before anything was ever claimed.
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
    // Not written in place. `fs::write` truncates first, so a reader arriving
    // mid-write would see a partial file — and `load` answers an unparseable
    // pointer with an *unreadable* run, which the gate refuses: every write
    // from that run stops until a person reads the file out. An agent making
    // parallel tool calls stores after every allowed write, so a store racing a
    // load on one pointer is the ordinary case rather than an unlucky one, and
    // an atomic replace is what keeps that race from manufacturing a pointer
    // nobody can read. (Older builds answered the same torn read with a fresh
    // run and let the write through, which is the hole the unreadable reading
    // closed; this comment named that older consequence long after it stopped
    // being true, and this issue's fix is exactly about such drift.)
    // The stale-writer check. A pointer on disk that has been written more times
    // than the one in hand is news this caller has not seen, and overwriting it
    // would drop whatever arrived in between.
    //
    // Skipped rather than refused: storing is best effort by design — failing to
    // record when we last asked costs one extra read and must never become a
    // denial. Dropping a stale write costs the same and keeps the fresher
    // answer, which is the only direction that fails closed.
    // A pointer that was **removed** is news as well, and the revision cannot
    // carry it: a missing file reads as revision zero, which every copy in hand
    // is newer than. So a hook holding a pointer from before a release stored it
    // back afterwards and brought the issue with it — measured: `issue=None`
    // after the release, `issue=Some(7)` after the hook's store. The run then
    // held an issue it had put down, and `claim` refused it the next one by
    // name: "already holds issue #7 and cannot also swear to #8".
    //
    // Anything loaded from a pointer that is now gone is stale by definition.
    // A caller that means to *create* one has loaded a fresh run, at revision
    // zero, and is untouched by this.
    if run.revision > 0 && !path.exists() {
        return Ok(false);
    }
    // A run loaded from a pointer that would not parse knows nothing, and
    // `unreadable` is `#[serde(skip)]` — so storing it writes a **readable**
    // pointer saying this run holds no issue. The gate reads that as a run that
    // swore nothing and lets every write through, and `doctor` sees a healthy
    // file. `update` states this rule two functions down, in its own words: *an
    // unreadable pointer is not a fresh one, and writing over it would throw
    // away whatever a person still has to read out of it.* It was stated there
    // and not here, and here is where the hot path writes.
    //
    // How it is reached, measured: the gate refuses an unreadable pointer, and a
    // stand-down turns that refusal into an allowance — which is the one branch
    // that then stores. So a bounded, recorded window left a permanent, silent
    // hole: the corruption gone, the run reading as having sworn nothing, and
    // nothing anywhere saying so once the window expired. A stand-down whose
    // whole design is *bounded* must not outlive itself.
    //
    // Dropped rather than refused, like the two checks around it: storing is
    // best effort, and losing this write costs one extra read.
    if run.unreadable {
        return Ok(false);
    }
    // `false` rather than `Ok(())`: this used to report a write it had not
    // made, and every caller wrote `let _ =`. See [`update`].
    let on_disk = load(root, &run.run_id);
    // A pointer that cannot be read is news too, and the loudest kind: the
    // revision that would say whether this caller is behind is exactly what
    // cannot be read out of it. The stale-writer check used to be *skipped*
    // there — `!on_disk.unreadable && …` — so the one case where being behind
    // is unknowable was the one case that wrote anyway. An unknown is not
    // clearance, and this rule already appears twice in this function: once for
    // a pointer that is **gone** and once for a run whose own copy is
    // unreadable. The copy on disk is the third door and had no lock.
    //
    // What it costs, which is the incident three paragraphs up arriving by
    // another route: a release stores `issue=None` and is interrupted, leaving
    // bytes nobody can parse. A hook still holding `issue=Some(7)` from before
    // it then stores, drops a check it cannot evaluate, and the run holds an
    // issue it had put down — after which `claim` refuses it the next one by
    // name. Measured: the corrupt pointer replaced, `store` reporting `true`,
    // and `doctor` seeing a healthy file.
    //
    // Dropped rather than refused, like the two checks around it. The run is
    // already stopped: the gate refuses an unreadable pointer, so nothing here
    // is racing to unblock anybody — `doctor` names it and a person reads it.
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
/// [`store`] drops a write made from a pointer that has since moved, and keeps
/// the fresher one. Its own note calls that "the only direction that fails
/// closed", and for the hook it is: what the hook records is when it last
/// asked, and the fresher answer has a better one.
///
/// It is the wrong way round for a **tool**, which records what only it knows.
/// A `claim` stores the issue; `start-branch` stores the isolated checkout. Lose
/// that write and the fresher pointer does not carry them at all: `covered` no
/// longer contains the worktree, and a write inside it passes as `Outside` —
/// which is the very hole the revision check was added to close, reopened from
/// the other side by the fix for it. Measured, not deduced: a hook storing
/// between a tool's load and its store left `issue=None worktree=None`.
///
/// So the change is re-applied to what is on disk rather than dropped. `change`
/// runs again on each attempt and must be idempotent, which setting fields is.
pub fn update(root: &Path, run_id: &str, change: impl FnMut(&mut Run)) -> Run {
    updated(root, run_id, change).0
}

/// The same, and whether what the caller asked for is **on disk**.
///
/// `update` answered with the run it had in hand whether or not the store
/// worked, so a pointer write that failed — a read-only home, a full disk, a
/// lock held on this crate's own platform — was indistinguishable from one that
/// landed. The caller then reported the operation as done.
///
/// That direction is the one nothing here may fail in. The tracker write **did**
/// happen; the harness's record of it did not, so the gate reads a run that
/// swore nothing and every write after it goes through ungated, while the
/// tracker says the issue is held. An unknown result is not clearance, and this
/// was not even unknown — it was known and dropped.
///
/// `false` also for a pointer that could not be read, which is the arm above:
/// nothing was applied, and saying otherwise would be the same lie in the other
/// spelling.
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

/// Where the decisions this machine's gate made are appended.
pub const LEDGER: &str = "decisions.jsonl";

/// The size past which the ledger is set aside and a fresh one begun.
///
/// Two megabytes is roughly forty thousand decisions — months of ordinary work.
/// An unbounded file on the critical path of every edit is a disk that fills
/// while the tool reports nothing wrong.
///
/// So the bound on disk is **two** of these, not one: [`rotate`] renames rather
/// than rewrites, and [`previous_ledger_path`] is still read. That is the cost
/// of never halving a file other runs are appending to, and it is the cheaper
/// half of the trade.
const LEDGER_CAP: u64 = 2 * 1024 * 1024;

/// Appends one decision to the ledger.
///
/// # Why there is a ledger at all
///
/// Estigia refused a write, the terminal scrolled, and nothing remained. For a
/// tool whose whole argument is durable evidence, its own decisions were the one
/// thing it kept no evidence of — and "why did it stop me?" had no answer an
/// hour later. This is the answer.
///
/// # What is written, and what is not
///
/// Only [`super::Decision::Allow`] and [`super::Decision::Deny`]: those are the calls inside
/// Estigia's authority. `Outside` is Estigia standing aside, and recording every
/// tool call of every session that never swore would bury the ones that matter
/// under the ones that never involved it.
///
/// # Best effort, always
///
/// On the critical path of every gated edit. It never fails a decision: a
/// refusal that could not be written down is still a refusal, and turning a full
/// disk into an allowed write would be the exact inversion this file exists to
/// prevent.
pub fn record(root: &Path, entry: &Value) {
    let path = ledger_path(root);
    let Some(directory) = path.parent() else {
        return;
    };
    let Ok(line) = serde_json::to_string(entry) else {
        return;
    };
    if fs::create_dir_all(directory).is_err() {
        return;
    }
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > LEDGER_CAP) {
        rotate(&path, root);
    }
    // Appended rather than replaced: this is the one file Estigia writes that is
    // a history rather than a state, and rewriting it whole on every decision
    // would make a log of forty thousand lines cost forty thousand rewrites.
    // One `write_all` of the line **and** its newline, not `writeln!`. Appending
    // is atomic per write, and `writeln!` is not one write: it formats, which
    // can issue the line and the newline separately — and two runs recording at
    // once interleave between them. Eight threads across one oversized ledger
    // left **51 lines that are not JSON**, which `doctor` reads as calls that
    // may have gone undecided, so the machine stops being able to swear.
    //
    // Building the string first makes it one call, and `O_APPEND` makes that
    // call atomic against every other writer.
    let mut line = line;
    line.push('\n');
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| {
            use std::io::Write;
            file.write_all(line.as_bytes())
        });
}

/// A count of seconds, written the way every other stamp in this crate is.
///
/// `doctor` printed the raw number — *"most recently at 1785904685"* — in a
/// report whose whole job is to be read by a person deciding what to do. Five
/// ungated calls **yesterday** and five from last spring need opposite
/// responses, and nothing on that line told the two apart. It was measured on
/// this crate's own machine, where the number turned out to mean *yesterday*
/// after a reader had assumed it was history.
///
/// The conversion is written out rather than taken from a crate: it is one
/// civil-from-days function, the dependency would be the only one here for it,
/// and `strftime("%Y-%m-%dT%H:%M:%SZ")` is what the binding writes.
pub fn stamp_of(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Howard Hinnant's civil-from-days, shifted so the era starts on 0000-03-01
    // — which is what makes the leap day the *last* day of a year and removes
    // every special case for February.
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

/// Where the ledger sits: beside the run pointers, not among them.
///
/// `runs/` holds one file per run, and `holdings` walks it. A history file in
/// there survives only because that walk happens to filter on an extension —
/// which is luck, not design, and luck that a later reader would have to
/// rediscover before changing the filter.
pub fn ledger_path(state_root: &Path) -> PathBuf {
    state_root.parent().unwrap_or(state_root).join(LEDGER)
}

/// Where the previous ledger sits once the current one is rotated away.
///
/// Read by everything that reads the ledger, because rotation is what keeps
/// "what happened just now" from being the only thing left.
pub fn previous_ledger_path(state_root: &Path) -> PathBuf {
    let path = ledger_path(state_root);
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

/// Sets an oversized ledger aside, rather than rewriting it in place.
///
/// It read the file, kept the newer half and renamed that over the top — and
/// every line appended between the read and the rename went with the old half.
/// Measured with eight threads recording across one trim: **22 of 160 decisions
/// lost and 43 lines left that are not JSON**. Both matter, and the second more:
/// `doctor` counts an unparsable line as a call that may have gone undecided,
/// so a torn ledger reports the silence BROKEN and no run on the machine can
/// swear.
///
/// A rename moves no bytes and reads none. Whatever is mid-append lands in the
/// file that is being set aside, which is still on disk and still read; the
/// next append creates a fresh one. Nothing is halved, so nothing is torn.
///
/// What it does not fix: two processes both over the cap both rename, and the
/// second one's `.1` is the file the first just started. That loses **history**
/// and never a current record, which is the trade a rotation makes and the
/// rewrite did not.
fn rotate(path: &Path, state_root: &Path) {
    let _ = fs::rename(path, previous_ledger_path(state_root));
}

/// Every run this machine has a pointer for.
///
/// Incident I06: five runs died after claiming and before transitioning, and the
/// issues sat assigned and labelled `ready` until somebody noticed. The tracker
/// is the authority on whether those claims are live — this only says *which
/// issues to go and ask about*, which is the question nobody could answer
/// before, because a dead run leaves no trace anywhere a person looks.
///
/// Quiet about failure: an unreadable state directory is a machine with nothing
/// to report, not an error worth interrupting `status` for.
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

/// Run pointers that are there and cannot be read.
///
/// [`holdings`] answers *which runs hold something*; this answers *which ones
/// could not be asked*, and they are not the same answer. A pointer that will
/// not open or will not parse was dropped from the list, and an empty list is
/// how the push guard is told that no claim covers this checkout — so a
/// corrupt pointer let a push through as [`crate::harness::Decision::Outside`].
/// The single-run path already refuses this way: "this run's record exists and
/// cannot be read, so whether it holds an issue is unknown". The all-runs path
/// never got the rule.
///
/// A missing directory is not unreadable: a machine where nothing has ever
/// claimed has no pointers, and that is an answer.
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
        // A stand-down a build before the move left here. It is not a pointer
        // and never was; counting it as one made the operator's own way to
        // loosen the gate refuse every push instead. `standdown::path` writes
        // beside this directory now, so this only ever matches a leftover.
        .filter(|path| path.file_name() != Some(std::ffi::OsStr::new("stand-down.json")))
        // The same question `holdings` asks, asked the other way round, and
        // through the same reader so the two lists cannot part company: a
        // pointer this build cannot act on belongs in exactly one of them, and
        // for a while it belonged in neither.
        .filter(|path| pointer_at(path).is_none())
        .map(|path| path.display().to_string())
        .collect()
}

/// The pointer in this file, when it is one this build can act on.
///
/// Two conditions, and the second is the one that was missing. It has to parse,
/// and it has to be **filed under the name it gives itself**: `load` already
/// refuses a pointer whose `run_id` disagrees with its file name, because a
/// pointer that does not name the run it is filed under is not one that run may
/// act on.
///
/// `holdings` read the same directory by content and did not ask, so `status`
/// listed the identity written inside — and its own hint says
/// `estigia release --run-id <id>` puts one down, beside an id that command
/// answers *holds no issue* for. A message naming a command that does not
/// discharge is what the ratchet forbids, and that one named it with the
/// argument filled in.
fn pointer_at(path: &Path) -> Option<Run> {
    let run: Run = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    (path.file_stem()?.to_str()? == run.run_id).then_some(run)
}

/// A span of seconds, in the largest unit that still says something.
///
/// `status` rendered every age in minutes, so a run last seen four hundred days
/// ago read `576000 min ago`. That command answers *which issues to go and ask
/// about* — it exists for the incident where five runs died after claiming and
/// sat unnoticed — and the number somebody acts on was the one they had to
/// divide first.
///
/// Under three quarters of a minute is `just now` rather than `0 min ago`,
/// which reads as a measurement that failed.
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
/// Three answers, not two. This returned `Option<u64>` with `saturating_sub`
/// inside it, so a stamp ahead of this machine's clock came back as nought
/// seconds and `status` printed **`last answer just now`** — for the life of the
/// run, and precisely when [`Run::within_window`] had stopped believing that
/// stamp and gone back to asking the tracker on every write. The report and the
/// gate disagreeing about the same field is the shape this crate keeps paying
/// for.
///
/// Answering `None` instead would have been the other flattening: `None` is
/// rendered *never verified*, and a run whose clock moved **was** verified. A
/// fact and the absence of one are not the same sentence — the argument
/// `Change::Kept` and `Change::Unrecorded` are separate variants for.
pub enum Silence {
    /// The tracker has never answered for this run.
    Never,
    /// It answered this many seconds ago.
    For(u64),
    /// It answered at a time this machine's clock has not reached.
    ///
    /// No number can be given, and the honest thing to say is why: an NTP step,
    /// a snapshot resumed, an RTC that ran fast. It is worth its own words
    /// because it also explains a behaviour the operator would otherwise find
    /// inexplicable — the renewal window no longer opening.
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
    fn a_complete_review_receipt_round_trips_as_one_pointer_field() {
        let receipt = crate::transport::claim::ReviewReceipt {
            epoch: "a".repeat(32),
            pr: 54,
            head: "b".repeat(40),
            base: "c".repeat(40),
            digest: "d".repeat(64),
        };
        let mut run = Run::new("claude-receipt".to_owned());
        run.review_receipt = Some(receipt.clone());

        let encoded = serde_json::to_string(&run).expect("the pointer serialises");
        let decoded: Run = serde_json::from_str(&encoded).expect("the pointer deserialises");
        assert_eq!(decoded.review_receipt, Some(receipt));
        assert_eq!(decoded.reviewed_head, None);
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
        assert_eq!(run.review_receipt, None);
    }

    /// A pointer nobody can read is not one this run may write over.
    ///
    /// The stale-writer check answers "has anything arrived that I have not
    /// seen", and it used to be skipped when the pointer on disk was
    /// unreadable — which is precisely when the answer is unknowable. The one
    /// case that could not be evaluated was the one case that wrote anyway.
    ///
    /// The scenario is the incident this function already documents, reached
    /// through the other door: a release puts the issue down and is interrupted
    /// mid-write, and a hook still holding the issue from before stores over the
    /// wreckage. Before this, the run came back holding an issue it had
    /// released — and `claim` then refused it the next one by name.
    /// A run loaded from a pointer nobody could read does not write one back.
    ///
    /// The sibling of the test below, and the one that was missing: that one
    /// covers a pointer that went unreadable **on disk** between the load and
    /// the store, and this covers the run whose own copy came from bytes
    /// nothing could parse.
    ///
    /// `unreadable` is `#[serde(skip)]`, so storing such a run writes a
    /// perfectly **readable** pointer saying it holds no issue — the gate then
    /// reads a run that swore nothing and lets every write through, and
    /// `doctor` sees a healthy file. The guard has been there since the round
    /// that found it; what was not there was anything that would notice it
    /// going away. Found by turning it off and watching the suite stay green.
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
        held.issue = Some(7);
        store(root, &held).expect("the first store");
        // What the hook is still carrying: readable, and behind.
        let stale = load(root, "claude-a");
        assert!(!stale.unreadable && stale.issue == Some(7));

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
        let after = update(root, "claude-a", |run| run.issue = Some(9));
        assert!(after.unreadable, "update walked past an unreadable pointer");
        assert_eq!(
            fs::read_to_string(&path).expect("still there"),
            wreckage,
            "update wrote over it after store had refused to"
        );
    }

    #[test]
    fn an_io_unreadable_pointer_is_an_unknown_and_is_never_written_over() {
        // Issue #38, at the loader rather than at the gate: a pointer the
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
        stale.issue = Some(7);
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

    /// Two sessions of one host do not share an identity.
    ///
    /// This existed and used `aaaaaaaa-1111` against `bbbbbbbb-2222`, which
    /// differ in their **first character** — so it passed against a minting
    /// rule that read only the first eight and could not have failed.
    ///
    /// The shape that breaks it is the ordinary one: a host whose ids carry a
    /// constant prefix. `session_01KvQ…` and `session_01ZZZ…` both minted
    /// `claude-session0`, and two runs with one identity share the pointer that
    /// says which issue they hold — measured below, in the test that follows.
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

    /// The pointer is what says a run holds an issue, so an identity two runs
    /// share is a claim two runs share.
    ///
    /// The consequence, measured rather than argued: before the digest, a
    /// second session loaded the first one's pointer and read `Some(42)`. It
    /// held nothing and believed it held an issue, and every gate answer after
    /// that is drawn from the belief.
    #[test]
    fn a_second_session_does_not_inherit_the_first_ones_oath() {
        let home = tempfile::tempdir().expect("a temporary home");
        let root = state_root(Some(home.path())).expect("a state root");

        let mine = run_id(DEFAULT_RUNTIME, "session_01AAAAAAAAAAAAAAAAAAAAAA");
        let mut run = Run::new(mine.clone());
        run.issue = Some(42);
        store(&root, &run).expect("the pointer is written");

        let theirs = run_id(DEFAULT_RUNTIME, "session_01BBBBBBBBBBBBBBBBBBBBBB");
        assert_ne!(mine, theirs, "two sessions minted one identity");
        assert_eq!(
            load(&root, &theirs).issue,
            None,
            "another session's run read this one's claim as its own"
        );
        // And this run still finds its own.
        assert_eq!(load(&root, &mine).issue, Some(42));
    }

    #[test]
    fn a_session_with_no_id_still_mints_something_usable() {
        // An agent that sends no session_id must not produce a run-id of
        // `claude-`, which would collide with every other such run.
        assert_eq!(run_id(DEFAULT_RUNTIME, ""), "claude-unknown");
        assert_eq!(run_id(DEFAULT_RUNTIME, "///"), "claude-unknown");
    }

    #[test]
    fn a_pointer_survives_a_round_trip() {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut run = Run::new("claude-abcd1234".to_owned());
        run.issue = Some(12);
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
        theirs.issue = Some(99);
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
        // run on the machine appends to one file. Two things were wrong at
        // once, and the second only showed after the first was fixed:
        //
        // 1. The cap was enforced by reading the file, keeping the newer half
        //    and renaming that over the top. Every line appended between the
        //    read and the rename went with the old half.
        // 2. The append was `writeln!`, which is not one write — it can issue
        //    the line and the newline separately, and two runs interleave
        //    between them.
        //
        // Measured before the fix, with the loop below: **138 of 160 decisions
        // kept and 43 lines left that are not JSON**. The second number is the
        // worse one: `doctor` counts an unparsable line as a call that may have
        // gone undecided, so a torn ledger reports the silence BROKEN and no run
        // on this machine can swear.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let path = ledger_path(&state);

        // Over the cap, so the next `record` rotates while the others append.
        let filler: String = (0..30_000)
            .map(|n| {
                format!(
                    "{{\"at\":{n},\"verdict\":\"old\",\"pad\":\"{}\"}}\n",
                    "x".repeat(60)
                )
            })
            .collect();
        assert!(
            filler.len() as u64 > LEDGER_CAP,
            "the fixture is under the cap"
        );
        std::fs::write(&path, &filler).expect("an oversized ledger");

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
                        record(
                            &state,
                            &serde_json::json!({
                                "at": 1,
                                "run_id": format!("claude-{run}"),
                                "verdict": "new",
                                "n": n,
                            }),
                        );
                    }
                });
            }
        });

        // Both halves, because rotation is what a run crossing the cap does and
        // the records nearest it are the ones most likely to be in the older
        // one. `doctor` reads them the same way.
        let now = std::fs::read_to_string(&path).unwrap_or_default();
        let older = std::fs::read_to_string(previous_ledger_path(&state)).unwrap_or_default();
        let lines: Vec<&str> = older
            .lines()
            .chain(now.lines())
            .filter(|line| !line.trim().is_empty())
            .collect();

        let torn: Vec<&&str> = lines
            .iter()
            .filter(|line| serde_json::from_str::<serde_json::Value>(line).is_err())
            .collect();
        assert!(
            torn.is_empty(),
            "{} line(s) are not JSON, which reads as a call nobody decided on: {:?}",
            torn.len(),
            &torn[..torn.len().min(2)]
        );
        // The same correction the `"old"` count below already carries, applied
        // to this one. Equality here claimed the guarantee `rotate` disclaims in
        // its own words: two writers both over the cap both rename, and the
        // second one's `.1` is the file the first just started, so a run that
        // rotates twice legitimately keeps less than everything. Measured on CI:
        // 55 of 160, on macOS, for the product behaving as documented.
        //
        // Windows never showed it because a file open for writing cannot be
        // renamed there — the rotation fails instead of clobbering, and the
        // records stay where they are. One platform's file semantics were
        // holding an assertion the other two could not.
        //
        // What is still measured is the defect this was written for: no record
        // is torn, above, and a decision written since the last rotation is
        // still here. Whether losing any at all is acceptable for the record of
        // what the gate decided is a product question, not this test's, and it
        // is open on the tracker.
        let survived = lines.iter().filter(|line| line.contains("\"new\"")).count();
        assert!(
            survived > 0 && survived <= runs * each,
            "{survived} of {} decisions are in neither half, and a record that keeps none of \
             them is not rotation",
            runs * each
        );
        // Whole or gone, and never halved. The count itself cannot be asserted
        // here: `rotate` says in its own words that two writers both over the
        // cap both rename, and the second one's `.1` is the file the first just
        // started — so a run of this test that rotates twice legitimately keeps
        // no history at all. Asserting 30_000 claimed a guarantee the product
        // disclaims, and it went red roughly one full-suite run in five, under
        // load, for the product behaving as documented. A test that fails for
        // the right behaviour teaches people to re-run the suite.
        //
        // What is still measured is the bug this was written for: trimming by
        // reading the file and keeping the newer half left *some* of the old
        // lines, never all of them and never none.
        let history = lines.iter().filter(|line| line.contains("\"old\"")).count();
        assert!(
            history == 30_000 || history == 0,
            "the ledger was halved rather than set aside: {history} of 30000 old lines"
        );
    }

    #[test]
    fn rotating_the_ledger_sets_the_whole_history_aside_rather_than_halving_it() {
        // The other half of the test above, with one writer so it can assert
        // what that one cannot. Concurrency is not what the halving bug needed
        // — a single `record` crossing the cap is enough to show it — and
        // measuring it here means the count is exact rather than one of two
        // allowed answers.
        let root = tempfile::tempdir().expect("a temporary root");
        let state = root.path().join("runs");
        std::fs::create_dir_all(&state).expect("the state directory");
        let path = ledger_path(&state);

        let filler: String = (0..30_000)
            .map(|n| {
                format!(
                    "{{\"at\":{n},\"verdict\":\"old\",\"pad\":\"{}\"}}\n",
                    "x".repeat(60)
                )
            })
            .collect();
        assert!(
            filler.len() as u64 > LEDGER_CAP,
            "the fixture is under the cap"
        );
        std::fs::write(&path, &filler).expect("an oversized ledger");

        record(&state, &serde_json::json!({ "verdict": "new" }));

        let now = std::fs::read_to_string(&path).unwrap_or_default();
        let older = std::fs::read_to_string(previous_ledger_path(&state)).unwrap_or_default();
        let lines: Vec<&str> = older.lines().chain(now.lines()).collect();
        assert_eq!(
            lines.iter().filter(|line| line.contains("\"old\"")).count(),
            30_000,
            "rotating the ledger dropped what it was rotating"
        );
        assert_eq!(
            lines.iter().filter(|line| line.contains("\"new\"")).count(),
            1,
            "the record that crossed the cap is in neither half"
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
        run.issue = Some(9);
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
                Some(9),
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
        run.issue = Some(3);
        store(root.path(), &run).expect("the pointer lands");
        assert_eq!(load(root.path(), "claude-ending00").issue, Some(3));

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
        sworn.issue = Some(7);
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
        assert_eq!(after.issue, Some(7));

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
    /// Seconds are written as a stamp, and the awkward days are the test.
    ///
    /// Every value here was produced by the binding's own spelling —
    /// `datetime.fromtimestamp(s, utc).strftime("%Y-%m-%dT%H:%M:%SZ")` — rather
    /// than by reading this function back to itself, which is the only way an
    /// arithmetic test says anything.
    ///
    /// Three of the six are leap days, because a civil-from-days that is wrong
    /// is almost always wrong on 29 February, and one is a century that is
    /// **not** a leap year by the 100-year rule but is caught again by the
    /// 400-year one.
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
    /// An age is written the way somebody reads it, not the way it divides.
    ///
    /// `status` answers *which issues to go and ask about* — it exists for
    /// incident I06, five runs that died after claiming and sat unnoticed — and
    /// it rendered every age in minutes. A run last seen four hundred days ago
    /// read `576000 min ago`, and one seen three days ago read `4320 min ago`.
    /// The number an operator acts on was the one they had to divide first.
    ///
    /// The same defect `stamp_of` above was written for, in the other unit: a
    /// report a person reads has to be legible at a glance, or the row that
    /// matters is the row nobody notices.
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

    /// A pointer that could not be written is not a pointer that was.
    ///
    /// `update` answered with the run it had in hand whether or not the store
    /// worked, so a failed write — a read-only home, a full disk, a lock held
    /// on this crate's own platform — was indistinguishable from one that
    /// landed, and the caller reported the operation as done.
    ///
    /// The direction is what makes it matter. The tracker write **did** happen;
    /// the record of it did not, so the gate reads a run that swore nothing and
    /// every write after it goes through ungated while the tracker says the
    /// issue is held.
    ///
    /// The directory fixture this used to reach that arm with was posing two
    /// things at once, and issue #38 separated them: a directory at the pointer
    /// path is a **read** failure before it is a write failure, and `load` now
    /// answers it as unreadable — so `updated` refuses before any write is
    /// attempted, which is the arm below. The remaining write failures are
    /// environmental — a read-only home, a full disk, a lock — and no std-only
    /// fixture poses them deterministically on every platform; the arm stays in
    /// the code and this states rather than claims its coverage.
    #[test]
    fn a_pointer_that_could_not_be_written_says_so() {
        let root = tempfile::tempdir().expect("a temporary state root");

        // The floor: an ordinary update lands and says it landed.
        let (run, on_disk) = updated(root.path(), "claude-aaaa1111", |run| run.issue = Some(7));
        assert!(on_disk, "an ordinary pointer write reported failure");
        assert_eq!(run.issue, Some(7));
        assert_eq!(
            load(root.path(), "claude-aaaa1111").issue,
            Some(7),
            "the pointer this said it wrote is not on disk"
        );

        // A directory where the pointer file has to go: the read cannot
        // succeed, on every platform, so the update refuses before anything is
        // written, and the change is never applied to the run in hand.
        let blocked = pointer_path(root.path(), "claude-bbbb2222");
        std::fs::create_dir_all(&blocked).expect("something unreadable in its place");
        let (run, on_disk) = updated(root.path(), "claude-bbbb2222", |run| run.issue = Some(9));
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
        let (_, on_disk) = updated(root.path(), "claude-cccc3333", |run| run.issue = Some(11));
        assert!(
            !on_disk,
            "an unreadable pointer was reported as having taken an update"
        );
    }
}
