//! What an install put on disk that was not there before.
//!
//! [`super::uninstall`] has to know this and cannot work it out from the disk.
//! A file Estigia created and a file it overwrote are indistinguishable
//! afterwards: in both cases the contents are Estigia's. Without the record,
//! uninstalling on top of an existing `issue-flow` checkout deleted the
//! checkout — every file Estigia had merely rewritten went out with the ones it
//! made, and `bindings/`, `references/` and `scripts/` with them.
//!
//! That is not an edge case. Estigia installs *upstream's* skill under
//! upstream's name (see the module docs above) precisely so the two can be the
//! same directory, which makes "these files were already here" the expected
//! arrangement rather than an unlucky one.
//!
//! # Where it lives, and why not in `~/.estigia`
//!
//! Beside the files it describes, so it is found by whoever finds them and dies
//! with them. Kept under the home directory instead it would outlive a skill
//! somebody deleted by hand, and a stale record is worse than none: it names
//! files to remove that a later install did not create. It also made the test
//! suite write into the real profile, which is its own kind of wrong.
//!
//! # What is written, and what is not
//!
//! Only paths, and only created ones. Not the contents: a record carrying the
//! previous text would be a backup, and a backup nobody asked for is a second
//! copy of the operator's files somewhere they did not choose.
//!
//! Two sets of them. The relative names under `root` are the skill's own files.
//! The absolute ones are what an install created *outside* it — today that is
//! each agent's instruction file, which is the one place where "was this ours"
//! cannot be read back off the disk: a `CLAUDE.md` holding nothing but the
//! directive block is either one Estigia made or one the operator kept empty,
//! and until this was written down the uninstall deleted both.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn record_path(root: &Path) -> PathBuf {
    root.join(".estigia").join("installed.json")
}

/// The record's contents, in both shapes it has ever had.
///
/// The first shape was a bare array of the paths created **under** `root`. It
/// gained a second set because one question could not be answered from the
/// disk and had to be remembered instead: whether an agent's instruction file
/// was Estigia's to delete. That file lives outside `root`, so it cannot be one
/// of the relative names, and it is not one either — the two sets are removed
/// by different code and only the first is joined to `root`.
///
/// A record written by an older build parses as `Legacy`, whose outside set is
/// empty. That is the safe reading rather than a lossy one: an instruction file
/// missing from the record is treated as the operator's and kept, so the worst
/// an old record can do is leave an empty file behind. Read the other way it
/// would delete files nobody recorded creating.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Stored {
    Current(Ledger),
    Legacy(BTreeSet<String>),
}

/// What an install made, in the two places it can make something.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Ledger {
    /// Relative, forward-slashed paths under `root`.
    #[serde(default)]
    created: BTreeSet<String>,
    /// Absolute, forward-slashed paths Estigia created outside `root`.
    #[serde(default)]
    outside: BTreeSet<String>,
    /// What Estigia last **wrote** at each path under `root`, as a digest.
    ///
    /// The record kept names and nothing about their contents, and the honesty
    /// contract named the cost in its own words: *the question it cannot answer
    /// is «did you change this, or did an older build write it»* — so a file an
    /// operator had edited was overwritten and removed exactly like one that was
    /// merely stale, on its own `update` or `remove` line, unmarked.
    ///
    /// A digest answers that question and only that question. It is not a
    /// backup: what it buys is the difference between *this is our file, older*
    /// and *this is our file with somebody's work in it*, which is the
    /// difference between bookkeeping and taking something away.
    ///
    /// Absent on every record written before this existed, and absent is not
    /// "edited": a machine whose record predates digests must go on syncing
    /// quietly, or the fix announces a false alarm on every installation there
    /// is.
    #[serde(default)]
    digests: std::collections::BTreeMap<String, String>,
}

/// What the record on disk turned out to be.
///
/// Absent and unreadable are separated because the writers must treat them
/// differently, and treating them alike destroyed the only copy: a record that
/// would not parse read as empty, and the next install wrote a fresh one over
/// it. The names it held are the files `uninstall` removes, so the corruption
/// went from "removes nothing this once" to "cannot ever remove them" — and the
/// bytes that would have said which were gone.
enum Held {
    /// No record here. An install may write the first one.
    Absent,
    /// A record that parsed, in either of its shapes.
    Read(Ledger),
    /// A record that is there and could not be understood.
    Unreadable(String),
}

fn held(root: &Path) -> Held {
    let path = record_path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Held::Absent,
        // Unreadable for any other reason is still unreadable: a permission
        // error is not an empty record either.
        Err(error) => return Held::Unreadable(error.to_string()),
    };
    match serde_json::from_str::<Stored>(&text) {
        Ok(Stored::Current(ledger)) => Held::Read(ledger),
        Ok(Stored::Legacy(created)) => Held::Read(Ledger {
            created,
            outside: BTreeSet::new(),
            digests: std::collections::BTreeMap::new(),
        }),
        Err(error) => Held::Unreadable(error.to_string()),
    }
}

/// The record as the *readers* see it: unreadable is empty, and safely so.
///
/// Every question asked of a record is one whose empty answer removes nothing
/// and keeps the operator's files, so a record nobody can read costs a cautious
/// answer rather than a wrong one. The writers ask [`held`] instead, because
/// for them the same conflation is destructive.
fn ledger(root: &Path) -> Ledger {
    match held(root) {
        Held::Read(ledger) => ledger,
        Held::Absent | Held::Unreadable(_) => Ledger::default(),
    }
}

/// The record to write into, or a refusal naming what stands in the way.
///
/// A write on top of a record nobody can read is the one that cannot be undone.
fn writable(root: &Path) -> Result<Ledger> {
    match held(root) {
        Held::Absent => Ok(Ledger::default()),
        Held::Read(ledger) => Ok(ledger),
        // Classified here rather than left as a bare error, because `setup`
        // wraps anything unclassified as `setup-write-failed` — which reports
        // the outcome as *unknown* for a run that wrote nothing, and sends the
        // operator to `estigia status` over a file `status` cannot fix. The
        // note on `setup_failed` describes that flattening arriving from three
        // directions; a plain `bail!` here was a fourth.
        Held::Unreadable(why) => Err(crate::outcome::Refusal {
            code: "install-record-unreadable",
            message: format!(
                "the install record at {} cannot be read ({why}), and writing over it would lose \
                 the list of files this install must remove later",
                record_path(root).display()
            ),
            outcome: crate::outcome::MutationOutcome::NotStarted,
            replay: crate::outcome::Replayability::ManualActionRequired,
            resolution: crate::outcome::Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                format!(
                    "delete {} to start a new record — Estigia then forgets it created the files \
                     already there, and a later uninstall leaves them for you to remove",
                    record_path(root).display()
                ),
            ),
        }
        .into()),
    }
}

/// A path as the record spells it: forward slashes, whatever the platform.
///
/// One spelling, because the install writes it and the uninstall looks it up,
/// and a path recorded one way and asked for another is a file that reads as
/// the operator's on the machine that created it.
pub fn spell(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The paths an install created under `root`, relative and forward-slashed.
///
/// Empty when Estigia has no record of installing there — which is the answer
/// for a directory somebody else's tool filled, and for one installed by a
/// build that predates the record. An unreadable or malformed record reads as
/// empty too: `uninstall` removes what the record names, so the failure mode of
/// not understanding it is removing nothing rather than removing everything.
pub fn created(root: &Path) -> BTreeSet<String> {
    ledger(root).created
}

/// Whether Estigia's install created `path`, which sits outside `root`.
///
/// The question `uninstall` cannot answer from the disk: an instruction file
/// holding nothing but the directive block is either one Estigia made or one
/// the operator kept empty, and afterwards the two are the same bytes. Absent
/// from the record means theirs — see [`Stored`] for why that is the safe way
/// round.
pub fn created_outside(root: &Path, path: &Path) -> bool {
    ledger(root).outside.contains(&spell(path))
}

/// Whether Estigia has any record of installing under `root`.
///
/// Distinct from [`created`] coming back empty, and the difference is the whole
/// message: a file missing from a record that exists was there before Estigia,
/// while a file missing because there is *no* record could be anything —
/// including a file Estigia wrote, from a build that predates the record or
/// after somebody deleted it. Only the first is a fact to tell an operator.
pub fn exists(root: &Path) -> bool {
    record_path(root).is_file()
}

/// What a write to the record would do to the file.
///
/// Asked by the planner before a run and applied by the writer during it, so
/// the rule that decides whether the file changes at all lives here once. Said
/// anywhere else it is a second copy of the rule, and the report drifts from the
/// run the first time this one moves — which is how three files came to be
/// written, and later removed, by a command that named neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrote {
    /// The record already says this; nothing is touched.
    Nothing,
    /// There was no record here.
    Created,
    /// There was one, and it gains or loses names.
    Updated,
    /// The last name goes, and the record goes with it.
    Removed,
}

/// Where the record for `root` lives, for a report that has to name it.
pub fn path(root: &Path) -> PathBuf {
    record_path(root)
}

/// What [`note_created`] would do, without doing it.
pub fn note_would<'a>(root: &Path, created: impl Iterator<Item = &'a str>) -> Wrote {
    let known = self::created(root);
    if created.into_iter().all(|path| known.contains(path)) {
        return Wrote::Nothing;
    }
    if exists(root) {
        Wrote::Updated
    } else {
        Wrote::Created
    }
}

/// What Estigia last wrote at `path`, when the record knows.
///
/// `None` for a path the record has never held a digest for — which includes
/// every record written before digests existed, and is deliberately *not* the
/// same answer as "the digest does not match".
pub fn written(root: &Path, path: &str) -> Option<String> {
    ledger(root).digests.get(path).cloned()
}

/// Records what an install just wrote at each path.
///
/// Added to, never replaced, like [`note_created`] and for the same reason:
/// eight adapters share the neutral skill root and each runs its own install.
pub fn note_written<'a>(
    root: &Path,
    written: impl Iterator<Item = (&'a str, String)>,
) -> Result<()> {
    let mut held = writable(root)?;
    let before = held.digests.clone();
    held.digests
        .extend(written.map(|(path, digest)| (path.to_owned(), digest)));
    if held.digests == before {
        return Ok(());
    }
    write(root, &held)
}

/// The digest this record stores contents under.
///
/// Over the text with its line endings **normalised**, because that is how the
/// rest of this crate decides whether a file is the one it wrote: the install's
/// own sameness check is `normalize(found) == normalize(desired)`, and a digest
/// that disagreed with it is a second rule answering one question.
///
/// It did disagree, and the direction it failed in is the expensive one.
/// Measured on Windows, where `core.autocrlf` and half the editors rewrite line
/// endings without anybody editing anything: a `runtime-notes.md` converted to
/// CRLF and nothing else came back `kept` from the uninstall, as though somebody
/// had worked on it. So the fix against *leaving somebody's work behind* began
/// leaving the **application** behind, which is the failure it was written to
/// prevent, inverted.
pub fn digest_of(text: &str) -> String {
    let normalised = text.replace("\r\n", "\n");
    crate::transport::ownership::sha256_hex(normalised.as_bytes())
}

/// Notes that `created` were written into `root` by an install.
///
/// Added to, never replaced. Eight adapters share the neutral skill root and
/// each runs its own install; the second must not forget what the first made.
pub fn note_created<'a>(root: &Path, created: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut held = writable(root)?;
    let before = held.created.len();
    held.created.extend(created.map(str::to_owned));
    if held.created.len() == before {
        return Ok(());
    }
    write(root, &held)
}

/// Notes that `path`, which is outside `root`, was created by an install.
///
/// Added to the same record, because it is the same fact: what was not there
/// before. Kept as a separate set because the two are removed by different
/// code, and a path outside `root` joined to `root` is a delete somewhere
/// nobody meant.
pub fn note_created_outside(root: &Path, path: &Path) -> Result<()> {
    let mut held = writable(root)?;
    if !held.outside.insert(spell(path)) {
        return Ok(());
    }
    write(root, &held)
}

/// Forgets an outside path, and the record itself once nothing is left in it.
pub fn forget_outside(root: &Path, path: &Path) -> Result<()> {
    let mut held = writable(root)?;
    if !held.outside.remove(&spell(path)) {
        return Ok(());
    }
    if held.created.is_empty() && held.outside.is_empty() {
        return erase(root);
    }
    write(root, &held)
}

/// What [`forget`] would do, without doing it.
pub fn forget_would<'a>(root: &Path, gone: impl Iterator<Item = &'a str>) -> Wrote {
    let mut known = ledger(root).created;
    let before = known.len();
    for path in gone {
        known.remove(path);
    }
    if known.len() == before {
        return Wrote::Nothing;
    }
    if known.is_empty() {
        Wrote::Removed
    } else {
        Wrote::Updated
    }
}

/// Forgets the paths named, and the record itself once nothing is left in it.
pub fn forget<'a>(root: &Path, gone: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut held = writable(root)?;
    let before = held.created.len();
    for path in gone {
        held.created.remove(path);
    }
    if held.created.len() == before {
        return Ok(());
    }
    // On `created` alone, and deliberately. An outside entry outliving the last
    // file under `root` cannot be one anybody still needs: `created` is only
    // emptied when no other agent shares this root, so there is nobody left to
    // ask. Weighing both sets here also made the planner and the run disagree —
    // the plan is drawn without the writes, so it still saw an entry the run had
    // already dropped, and reported `update` where the run did `remove`.
    if !held.created.is_empty() {
        return write(root, &held);
    }
    erase(root)
}

/// Nothing of Estigia's is left here, so neither is its bookkeeping. A
/// directory that came out empty is removed with it.
fn erase(root: &Path) -> Result<()> {
    let path = record_path(root);
    if let Err(error) = std::fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error).with_context(|| format!("remove {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(())
}

fn write(root: &Path, known: &Ledger) -> Result<()> {
    let path = record_path(root);
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create directory {}", parent.display()))?;
    let text = serde_json::to_string_pretty(known).context("render the install record")?;
    crate::paths::replace_atomically(&path, &text)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record an older build wrote is still read, and gains the new set.
    ///
    /// The shape changed from a bare array to an object, so every record on
    /// every machine that already has Estigia is in the old one. Read wrongly
    /// it comes back empty, and an empty record means `uninstall` removes
    /// nothing under the skill root — Estigia's own files left standing on the
    /// next upgrade, by a change meant to stop it deleting somebody else's.
    #[test]
    fn a_record_written_before_the_second_set_still_reads() {
        let root = tempfile::tempdir().expect("a temporary root");
        let path = record_path(root.path());
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
        std::fs::write(&path, r#"["SKILL.md","scripts/github.py"]"#).expect("an old record");

        let known = created(root.path());
        assert!(
            known.contains("SKILL.md") && known.contains("scripts/github.py"),
            "an old record read as {known:?}"
        );
        // And nothing outside it, which is the reading that keeps an operator's
        // file: absent from the record means theirs.
        assert!(!created_outside(
            root.path(),
            Path::new("/home/a/.claude/CLAUDE.md")
        ));

        // Writing brings it forward without losing what it said.
        note_created_outside(root.path(), Path::new("/home/a/.claude/CLAUDE.md"))
            .expect("the record is written");
        let known = created(root.path());
        assert!(known.contains("SKILL.md"), "the old names were dropped");
        assert!(created_outside(
            root.path(),
            Path::new("/home/a/.claude/CLAUDE.md")
        ));
    }

    /// A record nobody can read is not one nobody wrote.
    ///
    /// Reading it as empty is right and stays: every question asked of the
    /// record has an empty answer that removes nothing. *Writing* over it on the
    /// same reading is not — the names it held are the files `uninstall` takes
    /// out, so replacing it turned "removes nothing this once" into "cannot ever
    /// remove them", and destroyed the only bytes that said which. Measured on a
    /// record truncated mid-write: the next install left `{"created":["new.md"]}`
    /// where fourteen names had been.
    #[test]
    fn an_unreadable_record_is_not_written_over() {
        let root = tempfile::tempdir().expect("a temporary root");
        let path = record_path(root.path());
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");

        for broken in [
            // Truncated, as a half-finished write leaves it.
            r#"["SKILL.md","scripts/gith"#,
            // Valid JSON of the wrong shape, which parses and means nothing.
            r#"{"created": "SKILL.md"}"#,
            "",
        ] {
            std::fs::write(&path, broken).expect("a broken record");

            // The readers keep the cautious answer.
            assert!(
                created(root.path()).is_empty(),
                "an unreadable record answered a reader with names"
            );
            assert!(!created_outside(root.path(), Path::new("/a/CLAUDE.md")));

            // The writers refuse, and say what standing in the way costs.
            for refused in [
                note_created(root.path(), ["new.md"].into_iter()),
                note_created_outside(root.path(), Path::new("/a/CLAUDE.md")),
                forget(root.path(), ["SKILL.md"].into_iter()),
                forget_outside(root.path(), Path::new("/a/CLAUDE.md")),
            ] {
                let refusal = refused.expect_err("a write over an unreadable record");
                let refusal = refusal
                    .downcast_ref::<crate::outcome::Refusal>()
                    .expect("a refusal that classified itself, not a bare error");
                // Both halves. The code is what `setup` matches on to let this
                // through instead of flattening it into `setup-write-failed`,
                // and the resolution is what the operator does about it — a
                // refusal that says only what went wrong is the dead end the
                // ratchet forbids.
                assert_eq!(refusal.code, "install-record-unreadable");
                assert!(
                    refusal.outcome.is_clean(),
                    "a write that never started reported that it might have landed"
                );
                assert!(
                    format!("{:?}", refusal.resolution).contains("delete"),
                    "the refusal does not name the way out: {:?}",
                    refusal.resolution
                );
            }

            // And the bytes are still there to be looked at.
            assert_eq!(
                std::fs::read_to_string(&path).expect("still there"),
                broken,
                "a refused write changed the record anyway"
            );
        }

        // Absent is not unreadable: the first record is still written.
        std::fs::remove_file(&path).expect("no record");
        note_created(root.path(), ["new.md"].into_iter()).expect("the first record is written");
        assert!(created(root.path()).contains("new.md"));
    }

    /// One spelling for a path, whichever way the platform slashes it.
    #[test]
    fn a_path_is_recorded_and_looked_up_the_same_way() {
        let root = tempfile::tempdir().expect("a temporary root");
        let theirs = root.path().join(".claude").join("CLAUDE.md");
        note_created_outside(root.path(), &theirs).expect("written");
        assert!(
            created_outside(root.path(), &theirs),
            "a path was recorded one way and asked for another"
        );
    }
}
