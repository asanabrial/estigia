use super::{Store, int, unreadable};
use crate::transport::Failure;

pub(crate) struct SessionSeen<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) runtime: &'a str,
    pub(crate) session_id: &'a str,
    pub(crate) repo_dir: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionRow {
    pub(crate) run_id: String,
    pub(crate) runtime: String,
    pub(crate) session_id: String,
    pub(crate) repo_dir: Option<String>,
    pub(crate) started_at: u64,
    pub(crate) last_seen: u64,
    pub(crate) ended_at: Option<u64>,
    pub(crate) end_reason: Option<String>,
}

fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

impl Store {
    pub(crate) fn session_seen(&self, seen: &SessionSeen<'_>, now: u64) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO sessions (run_id, runtime, session_id, repo_dir, started_at, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT (run_id) DO UPDATE SET
                    runtime = excluded.runtime,
                    session_id = excluded.session_id,
                    repo_dir = excluded.repo_dir,
                    last_seen = excluded.last_seen,
                    ended_at = NULL,
                    end_reason = NULL",
                rusqlite::params![
                    seen.run_id,
                    seen.runtime,
                    seen.session_id,
                    seen.repo_dir,
                    int(now)
                ],
            )
            .map_err(unreadable("record a session"))?;
        Ok(())
    }

    pub(crate) fn session_ended(
        &self,
        run_id: &str,
        reason: Option<&str>,
        now: u64,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE sessions SET ended_at = ?2, end_reason = ?3 WHERE run_id = ?1",
                rusqlite::params![run_id, int(now), reason],
            )
            .map_err(unreadable("record the end of a session"))?;
        Ok(())
    }

    pub(crate) fn sessions(&self) -> Result<Vec<SessionRow>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT run_id, runtime, session_id, repo_dir, started_at, last_seen, ended_at,
                        end_reason FROM sessions ORDER BY last_seen DESC, run_id",
            )
            .map_err(unreadable("read the sessions"))?;
        statement
            .query_map([], |row| {
                Ok(SessionRow {
                    run_id: row.get(0)?,
                    runtime: row.get(1)?,
                    session_id: row.get(2)?,
                    repo_dir: row.get(3)?,
                    started_at: seconds(row.get(4)?),
                    last_seen: seconds(row.get(5)?),
                    ended_at: row.get::<_, Option<i64>>(6)?.map(seconds),
                    end_reason: row.get(7)?,
                })
            })
            .map_err(unreadable("read the sessions"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the sessions"))
    }
}
