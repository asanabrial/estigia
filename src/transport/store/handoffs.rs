use super::{Failure, Store, int, unreadable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HandoffRow {
    pub(crate) repo_dir: String,
    pub(crate) issue: u64,
    pub(crate) runtime: String,
    pub(crate) from_run_id: Option<String>,
    pub(crate) created_at: String,
    pub(crate) projection: String,
}

impl Store {
    /// Records that `issue` was handed to `runtime` in `repo_dir`.
    ///
    /// One row per repository, issue and runtime: a retry of the same handoff
    /// refreshes who last made it and when, rather than growing a second row
    /// that would read as two handoffs.
    pub(crate) fn record_handoff(
        &self,
        repo_dir: &str,
        issue: u64,
        runtime: &str,
        from_run_id: Option<&str>,
        projection: &str,
        now: &str,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO handoffs (repo_dir, issue, runtime, from_run_id, created_at, projection)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (repo_dir, issue, runtime) DO UPDATE SET
                    from_run_id = excluded.from_run_id,
                    created_at = excluded.created_at,
                    projection = excluded.projection",
                rusqlite::params![repo_dir, int(issue), runtime, from_run_id, now, projection],
            )
            .map_err(unreadable("record a handoff"))?;
        Ok(())
    }

    /// Marks the deferred rows of one issue and runtime as landed.
    ///
    /// The label is a fact about the issue, not about a checkout, so a flush in any checkout of the
    /// repository answers for every row that was waiting on it.
    pub(crate) fn mark_handoff_landed(&self, issue: u64, runtime: &str) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE handoffs SET projection = 'landed'
                 WHERE issue = ?1 AND runtime = ?2 AND projection = 'deferred'",
                rusqlite::params![int(issue), runtime],
            )
            .map_err(unreadable("mark a handoff landed"))?;
        Ok(())
    }

    /// Every handoff the store holds, oldest first.
    pub(crate) fn handoffs(&self) -> Result<Vec<HandoffRow>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT repo_dir, issue, runtime, from_run_id, created_at, projection
                 FROM handoffs ORDER BY created_at, repo_dir, issue, runtime",
            )
            .map_err(unreadable("read the handoffs"))?;
        statement
            .query_map([], |row| {
                Ok(HandoffRow {
                    repo_dir: row.get(0)?,
                    issue: u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
                    runtime: row.get(2)?,
                    from_run_id: row.get(3)?,
                    created_at: row.get(4)?,
                    projection: row.get(5)?,
                })
            })
            .map_err(unreadable("read the handoffs"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the handoffs"))
    }

    /// Whether this store handed `issue` to `runtime` in `repo_dir`, which is
    /// what makes that runtime's `dev:` label explainable to a later claim.
    pub(crate) fn handoff_explains(
        &self,
        repo_dir: &str,
        issue: u64,
        runtime: &str,
    ) -> Result<bool, Failure> {
        let found: Option<i64> = self
            .connection()
            .query_row(
                "SELECT 1 FROM handoffs WHERE repo_dir = ?1 AND issue = ?2 AND runtime = ?3",
                rusqlite::params![repo_dir, int(issue), runtime],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
            .map_err(unreadable("read a handoff"))?;
        Ok(found.is_some())
    }
}
