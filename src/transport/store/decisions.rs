use rusqlite::OptionalExtension;
use serde_json::Value;

use super::{Failure, Store, int, unreadable};

/// The two machine-wide histories the gate appends to. `Decision` records what
/// the gate decided; `Role` records the delegated contexts it saw. Both are
/// append-only and pruned by age, never by size, so no record is lost to a
/// rotation two runs race.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Record {
    Decision,
    Role,
}

impl Record {
    fn table(self) -> &'static str {
        match self {
            Self::Decision => "decisions",
            Self::Role => "roles",
        }
    }
}

impl Store {
    /// Appends one record. The row carries the entry's own `at` for retention
    /// and the entry's JSON for everything a reader needs, so no field is lost.
    pub(crate) fn add_record(&self, kind: Record, entry: &Value) -> Result<(), Failure> {
        let body = serde_json::to_string(entry)
            .map_err(|error| Failure::Read(format!("a record would not serialise: {error}")))?;
        let at = entry.get("at").and_then(Value::as_u64).map(int);
        self.connection()
            .execute(
                &format!("INSERT INTO {} (at, body) VALUES (?1, ?2)", kind.table()),
                rusqlite::params![at, body],
            )
            .map_err(unreadable("record"))?;
        Ok(())
    }

    /// Imports the legacy histories in one transaction, so two concurrent
    /// callers import once between them. Answers `false` when `done_key` is
    /// already set, which is the second caller of a race rather than a failure.
    pub(crate) fn import_records(
        &self,
        done_key: &str,
        batch: &[(Record, Vec<Value>)],
    ) -> Result<bool, Failure> {
        let transaction = rusqlite::Transaction::new_unchecked(
            self.connection(),
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(unreadable("begin the legacy import"))?;
        let done: Option<String> = transaction
            .query_row(
                "SELECT value FROM store_meta WHERE key = ?1",
                [done_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read the legacy import marker"))?;
        if done.as_deref() == Some("1") {
            transaction
                .commit()
                .map_err(unreadable("commit the legacy import"))?;
            return Ok(false);
        }
        for (kind, entries) in batch {
            let mut statement = transaction
                .prepare(&format!(
                    "INSERT INTO {} (at, body) VALUES (?1, ?2)",
                    kind.table()
                ))
                .map_err(unreadable("record"))?;
            for entry in entries {
                let body = serde_json::to_string(entry).map_err(|error| {
                    Failure::Read(format!("a record would not serialise: {error}"))
                })?;
                let at = entry.get("at").and_then(Value::as_u64).map(int);
                statement
                    .execute(rusqlite::params![at, body])
                    .map_err(unreadable("record"))?;
            }
        }
        transaction
            .execute(
                "INSERT INTO store_meta (key, value) VALUES (?1, '1')
                 ON CONFLICT (key) DO UPDATE SET value = '1'",
                [done_key],
            )
            .map_err(unreadable("write the legacy import marker"))?;
        transaction
            .commit()
            .map_err(unreadable("commit the legacy import"))?;
        Ok(true)
    }

    /// Every record of one kind, oldest first. A body that is not JSON is a
    /// failed read, never a silently skipped record.
    pub(crate) fn records(&self, kind: Record) -> Result<Vec<Value>, Failure> {
        let mut statement = self
            .connection()
            .prepare(&format!("SELECT body FROM {} ORDER BY rowid", kind.table()))
            .map_err(unreadable("read the records"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(unreadable("read the records"))?;
        let mut records = Vec::new();
        for row in rows {
            let body = row.map_err(unreadable("read the records"))?;
            let record = serde_json::from_str(&body)
                .map_err(|error| Failure::Read(format!("a stored record is not JSON: {error}")))?;
            records.push(record);
        }
        Ok(records)
    }

    /// Counts one failed append, so a record that could not be written down is
    /// visible to `doctor` rather than silently lost.
    pub(crate) fn note_record_failure(&self, kind: Record) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO record_failures (kind, count) VALUES (?1, 1)
                 ON CONFLICT (kind) DO UPDATE SET count = count + 1",
                [kind.table()],
            )
            .map_err(unreadable("count a failed record"))?;
        Ok(())
    }

    /// Counts one snapshot that could not be written, so `doctor` reports it.
    pub(crate) fn note_snapshot_failure(&self) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO record_failures (kind, count) VALUES ('snapshot', 1)
                 ON CONFLICT (kind) DO UPDATE SET count = count + 1",
                [],
            )
            .map_err(unreadable("count a failed snapshot"))?;
        Ok(())
    }

    /// Every kind that failed to be recorded and how many times.
    pub(crate) fn record_failures(&self) -> Result<Vec<(String, u64)>, Failure> {
        let mut statement = self
            .connection()
            .prepare("SELECT kind, count FROM record_failures ORDER BY kind")
            .map_err(unreadable("read the failed records"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(unreadable("read the failed records"))?;
        rows.map(|row| {
            let (kind, count) = row.map_err(unreadable("read the failed records"))?;
            Ok((kind, u64::try_from(count).unwrap_or_default()))
        })
        .collect()
    }

    /// Forgets records whose `at` is older than `cutoff`, across both kinds, and
    /// answers how many went. A record that recorded no `at` is never old enough.
    pub(crate) fn prune_records(&self, cutoff: u64) -> Result<usize, Failure> {
        let mut removed = 0;
        for kind in [Record::Decision, Record::Role] {
            removed += self
                .connection()
                .execute(
                    &format!(
                        "DELETE FROM {} WHERE at IS NOT NULL AND at < ?1",
                        kind.table()
                    ),
                    [int(cutoff)],
                )
                .map_err(unreadable("prune the records"))?;
        }
        Ok(removed)
    }

    /// One machine-wide marker: the import has run, the prune last ran, and so on.
    pub(crate) fn meta(&self, key: &str) -> Result<Option<String>, Failure> {
        self.connection()
            .query_row(
                "SELECT value FROM store_meta WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read a store marker"))
    }

    pub(crate) fn set_meta(&self, key: &str, value: &str) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO store_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )
            .map_err(unreadable("write a store marker"))?;
        Ok(())
    }
}
