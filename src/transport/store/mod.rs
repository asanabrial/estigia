//! The local workflow store: one SQLite file per user, every row keyed by the
//! repository it belongs to. The claim family
//! decides from it; `docs/honesty.md` lists what it does not yet hold.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{Failure, ownership};

mod calls;
mod decisions;
mod feed;
mod filesystem;
mod handoffs;
mod import;
mod jobs;
mod ledger;
mod messages;
mod needs_person;
mod outbox;
mod records;
mod schema;
mod sessions;
mod snapshot;
mod standing;
mod statuses;
mod taken_decisions;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use calls::count_into as with_store_at;
pub(crate) use calls::counting;
pub(super) use calls::note;
pub(crate) use decisions::Record;
pub(crate) use feed::{FeedRow, FeedWindow, MESSAGE, TRANSITION};
pub(crate) use handoffs::HandoffRow;
pub use import::Imported;
pub(crate) use jobs::{Inserted, Job, NewJob};
pub(crate) use ledger::{Ledger, report_unrecorded, store_path, viewer_login};
#[cfg(test)]
pub(crate) use ledger::{forget_scratch_store, ledger_opens, local_bodies};
pub(crate) use messages::{Message, MessageSent};
pub(crate) use needs_person::Need;
pub(crate) use outbox::{OutboxKind, OutboxRow, OutboxState};
pub(crate) use records::{Recorded, TaskChange, TaskRow};
pub(crate) use sessions::{SessionRow, SessionSeen};
pub(crate) use snapshot::{age_in_words, snapshot_age, snapshots_dir};
#[cfg(test)]
pub(crate) use statuses::StatusEntry;
pub(crate) use taken_decisions::TakenDecision;

const BUSY_WAIT: Duration = Duration::from_millis(5000);

/// The repository a row belongs to, as the host reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The GitHub host, `github.com` unless the operator names another.
    pub host: String,
    /// The owning user or organisation.
    pub owner: String,
    /// The repository name.
    pub name: String,
}

/// An open store. Every open failure is a failed read: nothing was learned.
pub struct Store {
    connection: Connection,
}

/// What the call counter recorded over a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CallSummary {
    /// Spawns of `gh issue`, `gh pr` and `gh api graphql`.
    pub graphql: u64,
    /// Spawns of any other `gh api`.
    pub rest: u64,
    /// Spawns of any other `gh` subcommand.
    pub other: u64,
}

/// One issue's stored row: the workflow state and the dedup identity the store
/// recorded for it, both absent when it recorded none.
pub(crate) struct IssueRow {
    pub(crate) number: u64,
    pub(crate) state: Option<String>,
    pub(crate) identity: Option<String>,
}

/// Where the store lives: `<home>/.estigia/state.db`, beside the run pointers.
pub fn default_path(home: Option<&Path>) -> Result<PathBuf, Failure> {
    let root = crate::harness::session::state_root(home)
        .map_err(|refusal| Failure::Read(format!("no home for the local store: {refusal:?}")))?;
    Ok(beside(&root))
}

/// The store beside a state root, whether or not it exists yet.
pub(crate) fn beside(state_root: &Path) -> PathBuf {
    state_root.with_file_name("state.db")
}

/// A read-only identity of the store file: the same while the file is the same
/// file, and different once it is replaced.
pub(crate) fn store_identity(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let Ok(metadata) = std::fs::metadata(path) else {
            return "absent".to_owned();
        };
        format!("{:x}:{:x}", metadata.dev(), metadata.ino())
    }
    #[cfg(windows)]
    {
        if std::fs::metadata(path).is_err() {
            return "absent".to_owned();
        }
        handle_identity(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        if std::fs::metadata(path).is_err() {
            "absent".to_owned()
        } else {
            "unknown".to_owned()
        }
    }
}

#[cfg(windows)]
fn handle_identity(path: &Path) -> String {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let Ok(file) = std::fs::File::open(path) else {
        return "unknown".to_owned();
    };
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return "unknown".to_owned();
    }
    let information = unsafe { information.assume_init() };
    format!(
        "{:x}:{:x}",
        information.dwVolumeSerialNumber,
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow)
    )
}

/// Where review job directories live: `<home>/.estigia/jobs`, beside `state.db`.
pub fn jobs_root(home: Option<&Path>) -> Result<PathBuf, Failure> {
    Ok(default_path(home)?.with_file_name("jobs"))
}

/// Opens the store for reading alone when its file exists, and answers `None`
/// when there is none yet: a read must not create the store it reads. A file
/// another schema wrote is refused rather than read as this one.
pub(crate) fn open_for_read(path: &Path) -> Result<Option<Store>, Failure> {
    match path.try_exists() {
        Ok(true) => {}
        Ok(false) => return Ok(None),
        Err(error) => {
            return Err(Failure::Read(format!(
                "the local store path could not be read: {error}"
            )));
        }
    }
    let connection = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(unreadable("open"))?;
    let store = Store { connection };
    let known = u32::try_from(schema::MIGRATIONS.len()).unwrap_or(u32::MAX);
    if store.schema_version()? > known {
        return Err(Failure::Read(
            "the local store carries a schema newer than this build reads".to_owned(),
        ));
    }
    Ok(Some(store))
}

const WAL_ATTEMPTS: u32 = 5;

fn settle_wal(mut ask: impl FnMut() -> rusqlite::Result<String>) -> Result<(), Failure> {
    let mut last = Err(Failure::Read(
        "the local store was never asked for WAL".to_owned(),
    ));
    for attempt in 0..WAL_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(20 * u64::from(attempt)));
        }
        match ask() {
            Ok(mode) if mode.eq_ignore_ascii_case("wal") => return Ok(()),
            Ok(mode) => {
                last = Err(Failure::Read(format!(
                    "the local store refused WAL mode and answered {mode:?}"
                )));
            }
            Err(error) => last = Err(unreadable("enter WAL mode")(error)),
        }
    }
    last
}

pub(super) fn int(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

thread_local! {
    static OPENING_SINCE: std::cell::Cell<Option<std::time::Instant>> =
        const { std::cell::Cell::new(None) };
}

struct Opening;

impl Opening {
    fn begin() -> Self {
        OPENING_SINCE.set(Some(std::time::Instant::now()));
        Self
    }
}

impl Drop for Opening {
    fn drop(&mut self) {
        OPENING_SINCE.set(None);
    }
}

fn unreadable(what: &str) -> impl Fn(rusqlite::Error) -> Failure + '_ {
    move |error| {
        let code = error.sqlite_error().map(|failure| failure.extended_code);
        let elapsed = OPENING_SINCE.get().map(|since| since.elapsed().as_millis());
        let detail = match (code, elapsed) {
            (Some(code), Some(ms)) => format!(" (sqlite {code}, after {ms} ms)"),
            (Some(code), None) => format!(" (sqlite {code})"),
            _ => String::new(),
        };
        Failure::Read(format!("the local store could not {what}: {error}{detail}"))
    }
}

/// `FULL`, unless the debug-only fsync skip `paths::sync_durably` obeys is armed.
fn durability() -> &'static str {
    #[cfg(debug_assertions)]
    if crate::paths::fsync_skipped() {
        return "PRAGMA synchronous = OFF; PRAGMA foreign_keys = ON;";
    }
    "PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;"
}

impl Store {
    /// Opens the store at `path`, creating and migrating it.
    pub fn open(path: &Path) -> Result<Self, Failure> {
        Self::open_waiting(path, BUSY_WAIT)
    }

    /// Opens the store waiting at most `wait` on a lock another opener holds.
    pub(crate) fn open_waiting(path: &Path, wait: Duration) -> Result<Self, Failure> {
        let _opening = Opening::begin();
        let store = Self::connect(path, wait)?;
        store.migrate()?;
        Ok(store)
    }

    fn connect(path: &Path, wait: Duration) -> Result<Self, Failure> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                Failure::Read(format!("could not create {}: {error}", parent.display()))
            })?;
        }
        let probe = path
            .parent()
            .filter(|parent| parent.exists())
            .unwrap_or(path);
        if let Some(kind) = filesystem::filesystem_kind(probe)
            && filesystem::is_network_filesystem(&kind)
        {
            return Err(filesystem::store_unwritable(&kind, probe));
        }
        let connection = Connection::open(path).map_err(unreadable("open"))?;
        connection
            .busy_timeout(wait)
            .map_err(unreadable("set its busy timeout"))?;
        settle_wal(|| connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)))?;
        connection
            .execute_batch(durability())
            .map_err(unreadable("set its durability"))?;
        Ok(Self { connection })
    }

    pub(super) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// The schema version this file carries.
    pub fn schema_version(&self) -> Result<u32, Failure> {
        let found: Option<u32> = self
            .connection
            .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
            .optional()
            .map_err(unreadable("read its schema version"))?;
        found.ok_or_else(|| Failure::Read("the local store records no schema version".to_owned()))
    }

    fn migrate(&self) -> Result<(), Failure> {
        self.migrate_after_read(|| {})
    }

    fn migrate_after_read(&self, after_read: impl FnOnce()) -> Result<(), Failure> {
        let known = u32::try_from(schema::MIGRATIONS.len()).unwrap_or(u32::MAX);
        self.connection
            .execute_batch("CREATE TABLE IF NOT EXISTS meta (schema_version INTEGER NOT NULL);")
            .map_err(unreadable("create its version table"))?;
        let current: u32 = self
            .connection
            .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
            .optional()
            .map_err(unreadable("read its schema version"))?
            .unwrap_or(0);
        if current > known {
            return Err(Failure::Read(format!(
                "the local store is schema {current} and this build understands up to {known}"
            )));
        }
        after_read();
        for (index, sql) in schema::MIGRATIONS.iter().enumerate().skip(current as usize) {
            let version = index + 1;
            let transaction = rusqlite::Transaction::new_unchecked(
                &self.connection,
                TransactionBehavior::Immediate,
            )
            .map_err(unreadable("begin a migration"))?;
            let inside: u32 = transaction
                .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
                .optional()
                .map_err(unreadable("re-read its schema version"))?
                .unwrap_or(0);
            if inside as usize >= version {
                continue;
            }
            transaction
                .execute_batch(&format!(
                    "{sql}
                     DELETE FROM meta;
                     INSERT INTO meta (schema_version) VALUES ({version});"
                ))
                .map_err(unreadable("apply a migration"))?;
            transaction
                .commit()
                .map_err(unreadable("commit a migration"))?;
        }
        Ok(())
    }

    /// Every entry of one issue as the comments the reducers consume, in
    /// insertion order, ids spelled `ES_<seq>`.
    pub fn timeline(&self, repo: &Repo, issue: u64) -> Result<Vec<ownership::Comment>, Failure> {
        let repo_id: Option<i64> = self
            .connection
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read a timeline"))?;
        match repo_id {
            Some(id) => entries(&self.connection, id, issue).map_err(unreadable("read a timeline")),
            None => Ok(Vec::new()),
        }
    }

    /// Every issue the store holds for `repo`, by number, with the state and the
    /// dedup identity it recorded. A repository the store has never seen answers
    /// nothing, never an error.
    pub(crate) fn issue_rows(&self, repo: &Repo) -> Result<Vec<IssueRow>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(Vec::new());
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT number, state, identity FROM issues WHERE repo_id = ?1 ORDER BY number",
            )
            .map_err(unreadable("read the issues"))?;
        statement
            .query_map([repo_id], |row| {
                Ok(IssueRow {
                    number: u64::try_from(row.get::<_, i64>(0)?).unwrap_or_default(),
                    state: row.get(1)?,
                    identity: row.get(2)?,
                })
            })
            .and_then(Iterator::collect)
            .map_err(unreadable("read the issues"))
    }

    /// The `blocked-until` marker bodies one issue recorded, in the order
    /// recorded. A marker that is not well formed is kept as it was recorded, so
    /// the reader can report it rather than silently count it as holding.
    pub(crate) fn blocked_clauses(&self, repo: &Repo, issue: u64) -> Result<Vec<String>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(Vec::new());
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT body FROM blocked_clauses
                 WHERE repo_id = ?1 AND issue = ?2 ORDER BY rowid",
            )
            .map_err(unreadable("read blocked clauses"))?;
        statement
            .query_map(rusqlite::params![repo_id, int(issue)], |row| {
                row.get::<_, String>(0)
            })
            .and_then(Iterator::collect)
            .map_err(unreadable("read blocked clauses"))
    }

    fn repo_id(&self, repo: &Repo) -> Result<Option<i64>, Failure> {
        self.connection
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("find the repository"))
    }

    /// What the counter recorded at or after `since` (epoch seconds).
    pub fn calls_since(&self, since: u64) -> Result<CallSummary, Failure> {
        let mut statement = self
            .connection
            .prepare("SELECT kind, COALESCE(SUM(count), 0) FROM calls WHERE at >= ?1 GROUP BY kind")
            .map_err(unreadable("prepare the call summary"))?;
        let rows = statement
            .query_map([int(since)], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(unreadable("read the call summary"))?;
        let mut summary = CallSummary::default();
        for row in rows {
            let (kind, count) = row.map_err(unreadable("read the call summary"))?;
            let count = u64::try_from(count).unwrap_or_default();
            match kind.as_str() {
                "graphql" => summary.graphql = count,
                "rest" => summary.rest = count,
                _ => summary.other = count,
            }
        }
        Ok(summary)
    }
}

pub(super) fn entries(
    connection: &Connection,
    repo_id: i64,
    issue: u64,
) -> rusqlite::Result<Vec<ownership::Comment>> {
    let mut statement = connection.prepare(
        "SELECT seq,
                COALESCE((SELECT github_created_at FROM seen_comments
                          WHERE matched_seq = entries.seq), created_at),
                body, viewer_did_author, includes_created_edit, source_comment_id,
                COALESCE(github_id,
                         (SELECT github_id FROM seen_comments WHERE matched_seq = entries.seq),
                         CASE source WHEN 'import' THEN 0 ELSE 9223372036854775807 END)
         FROM entries WHERE repo_id = ?1 AND issue = ?2 ORDER BY seq",
    )?;
    let mut rows: Vec<(i64, ownership::Comment)> = statement
        .query_map(rusqlite::params![repo_id, int(issue)], |row| {
            Ok((
                row.get::<_, i64>(6)?,
                ownership::Comment {
                    id: Some(match row.get::<_, Option<String>>(5)? {
                        Some(imported) => imported,
                        None => format!("ES_{}", row.get::<_, i64>(0)?),
                    }),
                    created_at: row.get(1)?,
                    body: row.get(2)?,
                    viewer_did_author: row.get(3)?,
                    includes_created_edit: row.get(4)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    order_ties_by_github(&mut rows);
    Ok(rows.into_iter().map(|(_, comment)| comment).collect())
}

/// Rows that share a stamp are put in GitHub's order, by its comment id, in the
/// slots those rows already occupy; a row GitHub has not yet listed sorts last
/// and an imported one first. Every other position is left as it was recorded.
fn order_ties_by_github(rows: &mut [(i64, ownership::Comment)]) {
    let mut slots: std::collections::BTreeMap<_, Vec<usize>> = Default::default();
    for (index, (_, comment)) in rows.iter().enumerate() {
        slots
            .entry(ownership::stamp_rank(&comment.created_at))
            .or_default()
            .push(index);
    }
    for indices in slots.values().filter(|indices| indices.len() > 1) {
        let mut tied: Vec<_> = indices.iter().map(|&index| rows[index].clone()).collect();
        tied.sort_by_key(|(github_id, _)| *github_id);
        for (&index, row) in indices.iter().zip(tied) {
            rows[index] = row;
        }
    }
}

#[cfg(test)]
mod identity {
    use super::store_identity;

    #[test]
    fn the_store_identity_is_stable_for_one_file_distinct_for_two_and_absent_for_no_file() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let one = directory.path().join("one.db");
        let two = directory.path().join("two.db");
        std::fs::write(&one, "one").expect("a first file");
        std::fs::write(&two, "two").expect("a second file");

        let identity = store_identity(&one);
        assert_eq!(
            identity,
            store_identity(&one),
            "the same file answered two identities, so a cursor could be refused for a store that never moved"
        );
        assert_ne!(
            identity,
            store_identity(&two),
            "two files answered one identity, so a replaced store would go undetected"
        );
        assert_eq!(
            store_identity(&directory.path().join("none.db")),
            "absent",
            "a path with no file did not answer the absent sentinel"
        );
    }
}
