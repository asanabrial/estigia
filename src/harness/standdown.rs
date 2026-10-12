//! Standing the gate down — declared, bounded, and on the record.
//!
//! # Why this is not a switch
//!
//! Estigia's declared asymmetry is that *a setting which can loosen a guard rail
//! turns it into a preference*. An on/off toggle breaks it head-on: an operator
//! who flips it once is loosened forever, silently, and nothing in the run says
//! so. But the opposite — a harness that cannot be stood down at all — is a
//! harness that gets uninstalled the first time it is wrong at a bad moment,
//! and an uninstalled harness gates nothing.
//!
//! The resolution is the shape [`crate::transport::claim::reclaim`] already uses
//! for a forced takeover: **declarable and answerable for**. A stand-down
//!
//! - must carry a reason, so there is something to answer for;
//! - **expires**, so nobody can be loosened without choosing to be, again;
//! - is capped, because a window long enough to forget about is a switch with
//!   extra steps;
//! - and every write it lets through is recorded as having gone through it, so
//!   the trace says "allowed under a stand-down" rather than "allowed".
//!
//! What it does **not** do is make anything quiet. That is the whole difference.

use serde::{Deserialize, Serialize};

/// The longest a stand-down may run.
///
/// Four hours: long enough to finish the thing that could not wait, short enough
/// that nobody plans around it. A window an operator can set and forget is a
/// switch, and the cap is what keeps this from becoming one.
pub const LONGEST: u64 = 4 * 60 * 60;

/// One declared stand-down.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandDown {
    /// Why the gate was stood down. Free text, and required.
    pub reason: String,
    /// When it was declared, in seconds since the epoch.
    pub declared_at: u64,
    /// The second it stops applying.
    pub until: u64,
    /// Who declared it.
    pub declared_by: String,
}

/// What is at the stand-down's path, as three answers rather than two.
///
/// A file that is there and will not open, or that holds something no parser
/// recognises, is not the same thing as no file at all — and `doctor` must be
/// able to say so. The gate is right to keep treating it as absent: *treating
/// an unreadable one as in force would let a corrupt file open the gate*. This
/// makes that decision a stated one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// No file. The ordinary state of almost every machine.
    Away,
    /// A file that is there and could not be read or understood.
    Unreadable(String),
    /// A record that was read. Whether it still *applies* is a separate
    /// question, and the clock answers it.
    Declared(StandDown),
}

/// Reads what is at the stand-down's path.
pub fn standing(state_root: &std::path::Path) -> Standing {
    let file = path(state_root);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Standing::Away,
        Err(error) => return Standing::Unreadable(format!("{}: {error}", file.display())),
    };
    match serde_json::from_str(&text) {
        Ok(record) => Standing::Declared(record),
        Err(error) => Standing::Unreadable(format!("{}: {error}", file.display())),
    }
}

impl StandDown {
    /// Whether this still applies at `now`.
    ///
    /// Exclusive at the far end: a stand-down `until` some second has stopped
    /// applying *at* that second. An inclusive bound would leave one second of
    /// loosened gate after the declared window.
    ///
    /// **And the cap, checked here rather than only on the way out.** The file
    /// on disk must not be the only authority on how long the gate stays down:
    /// a hand-edited or cross-machine record is the switch this module says it
    /// refuses to be. Put here rather than in [`in_force`] because two of the
    /// three readers reach past `in_force` to this function directly — a rule
    /// held in three places is one that will disagree with itself.
    ///
    /// Inclusive against the cap on purpose: `declare` grants exactly [`LONGEST`]
    /// and a reader stricter than the writer would refuse the longest stand-down
    /// this build actually issues.
    ///
    /// **And it has to have started.** The cap above bounds the *width* of the
    /// window and says nothing about where it sits, so moving both stamps
    /// forward together walks straight past it — a record declared a year from
    /// now, thirty minutes wide, is a legal window and the gate stays down for
    /// the whole year. A clock that is ahead (a dead RTC, a restored snapshot,
    /// an NTP jump) writes that stamp by itself.
    pub fn covers(&self, now: u64) -> bool {
        self.declared_at <= now
            && now < self.until
            && self.until.saturating_sub(self.declared_at) <= LONGEST
    }

    /// How much of it is left, in seconds.
    pub fn remaining(&self, now: u64) -> u64 {
        self.until.saturating_sub(now)
    }
}

/// Why a proposed stand-down cannot be declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// No reason given, so there would be nothing to answer for.
    NoReason,
    /// Zero or negative time: a stand-down that covers nothing.
    NoTime,
    /// Longer than [`LONGEST`].
    TooLong,
}

impl Rejected {
    /// The code this refusal carries.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoReason => "stand-down-needs-a-reason",
            Self::NoTime => "stand-down-covers-no-time",
            Self::TooLong => "stand-down-too-long",
        }
    }

    /// What the operator has to do differently.
    pub fn action(self) -> String {
        match self {
            Self::NoReason => {
                "a reason — a stand-down with nothing to answer for is a switch".to_owned()
            }
            Self::NoTime => "a window of at least one minute".to_owned(),
            Self::TooLong => format!(
                "a window of {} minutes or less — a longer one is a switch with extra steps",
                LONGEST / 60
            ),
        }
    }
}

/// Builds a stand-down, or says why it is not one.
///
/// Pure and fed the clock, because every interesting case here is about *time*
/// and a function that reads the clock itself cannot be shown to handle any of
/// them.
pub fn declare(
    reason: &str,
    minutes: u64,
    now: u64,
    declared_by: &str,
) -> Result<StandDown, Rejected> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(Rejected::NoReason);
    }
    if minutes == 0 {
        return Err(Rejected::NoTime);
    }
    let seconds = minutes.saturating_mul(60);
    if seconds > LONGEST {
        return Err(Rejected::TooLong);
    }
    Ok(StandDown {
        reason: reason.to_owned(),
        declared_at: now,
        until: now.saturating_add(seconds),
        declared_by: declared_by.to_owned(),
    })
}

/// The stand-down in force at `now`, if any.
///
/// An expired record is not an error and is not deleted here: it is the evidence
/// that the gate *was* stood down, and a stand-down that erases itself when it
/// expires is one nobody can answer for afterwards.
pub fn in_force(stored: Option<&StandDown>, now: u64) -> Option<&StandDown> {
    stored.filter(|declared| declared.covers(now))
}

/// Where the record sits: beside the run pointers, not among them.
///
/// Not among them because a stand-down is not a run — a directory listing that
/// mixed the two would let a stale stand-down read as a held issue, and the
/// pointer reader would have to learn to skip it. For the same reason
/// [`crate::harness::session::ledger_path`] is out, in the words it already
/// used: *`runs/` holds one file per run, and `holdings` walks it.*
pub fn path(state_root: &std::path::Path) -> std::path::PathBuf {
    state_root
        .parent()
        .unwrap_or(state_root)
        .join("stand-down.json")
}

/// Where a build before the move wrote it.
///
/// Read by nothing and skipped by the pointer walk, so a machine that upgrades
/// with one in force is not left refusing every push until somebody finds the
/// file. The stand-down itself is simply over — bounded windows expire, and
/// losing one to an upgrade is the direction that needs no guarding.
pub fn legacy_path(state_root: &std::path::Path) -> std::path::PathBuf {
    state_root.join("stand-down.json")
}

/// Lets a refusal through, under a stand-down, saying so.
///
/// The allowance **names what it overrode and how long is left**. That is the
/// whole difference between this and a switch: a trace reading "allowed" and one
/// reading "allowed under a stand-down declared for X, 12 minutes left, over
/// out-of-phase" are not the same record, and only the second can be answered
/// for afterwards.
///
/// `Outside` and `Allow` are returned untouched; an `Ask` is stood down like a `Deny`. A stand-down that turned
/// `Outside` into an allowance would be claiming authority the run never swore,
/// and re-labelling an ordinary `Allow` would put a stand-down in the trace of a
/// write that never needed one — which is how a record stops being evidence.
/// `now` is an [`Option`] because a clock that will not answer is not a moment:
/// read as the epoch, it falls inside *every* window ever declared, so a
/// machine whose clock cannot be read would have the gate permanently stood
/// down. The clock is the third way in, and it is the one that fails open.
pub fn over(
    decision: crate::harness::Decision,
    declared: Option<&StandDown>,
    now: Option<u64>,
) -> crate::harness::Decision {
    use crate::harness::Decision;
    let (Decision::Deny(refusal) | Decision::Ask(refusal)) = &decision else {
        return decision;
    };
    let Some(now) = now else {
        return decision;
    };
    let Some(declared) = in_force(declared, now) else {
        return decision;
    };
    let left = declared.remaining(now).div_ceil(60);
    Decision::Allow(format!(
        "stood down by {} for {:?} — {left} minute(s) left, over `{}`",
        declared.declared_by, declared.reason, refusal.code
    ))
}

#[cfg(test)]
mod tests;
