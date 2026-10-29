use std::path::Path;

use super::{Repo, Store, int, unreadable};
use crate::transport::Failure;

pub(crate) struct SessionSeen<'a> {
    pub(crate) run_id: &'a str,
    pub(crate) runtime: &'a str,
    pub(crate) session_id: &'a str,
    pub(crate) repo_dir: Option<&'a str>,
    pub(crate) wake: Option<&'a str>,
    pub(crate) pane: Option<&'a str>,
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
    pub(crate) name: Option<String>,
    pub(crate) build: Option<String>,
    pub(crate) wake: Option<String>,
    pub(crate) pane: Option<String>,
    pub(crate) host_pid: Option<u64>,
    pub(crate) host_name: Option<String>,
    pub(crate) binary: Option<String>,
    pub(crate) session_file: Option<String>,
}

fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

const NAME_ATTEMPTS: u32 = 5;

fn basename(dir: &str) -> &str {
    Path::new(dir)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("session")
}

impl Store {
    pub(crate) fn session_seen(&self, seen: &SessionSeen<'_>, now: u64) -> Result<(), Failure> {
        for _ in 0..NAME_ATTEMPTS {
            let ord = match seen.repo_dir {
                Some(dir) => self
                    .connection()
                    .query_row(
                        "SELECT COALESCE(MAX(ord), 0) + 1 FROM sessions WHERE repo_dir = ?1",
                        rusqlite::params![dir],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(unreadable("read the next session name"))?,
                None => 0,
            };
            let name = seen.repo_dir.map(|dir| format!("{}-{ord}", basename(dir)));
            let written = self.connection().execute(
                "INSERT INTO sessions (run_id, runtime, session_id, repo_dir, started_at, last_seen,
                                       name, ord, build, wake, pane)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (run_id) DO UPDATE SET
                    runtime = excluded.runtime,
                    session_id = excluded.session_id,
                    repo_dir = excluded.repo_dir,
                    last_seen = excluded.last_seen,
                    build = excluded.build,
                    wake = excluded.wake,
                    pane = excluded.pane,
                    ended_at = NULL,
                    end_reason = NULL",
                rusqlite::params![
                    seen.run_id,
                    seen.runtime,
                    seen.session_id,
                    seen.repo_dir,
                    int(now),
                    name,
                    ord,
                    crate::build_info::VERSION,
                    seen.wake,
                    seen.pane
                ],
            );
            match written {
                Ok(_) => return Ok(()),
                Err(error) if contention(&error) => continue,
                Err(error) => return Err(unreadable("record a session")(error)),
            }
        }
        Err(Failure::Read(format!(
            "record a session: `{}` could not be given a short name after five attempts, \
             because another registration kept taking the ordinal this one computed",
            seen.run_id
        )))
    }

    pub(crate) fn session_process(
        &self,
        run_id: &str,
        pid: Option<u64>,
        host: Option<&str>,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE sessions SET host_pid = ?2, host_name = ?3 WHERE run_id = ?1",
                rusqlite::params![run_id, pid.map(int), host],
            )
            .map_err(unreadable("record the process of a session"))?;
        Ok(())
    }

    pub(crate) fn session_binary(&self, run_id: &str, binary: Option<&str>) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE sessions SET binary = ?2 WHERE run_id = ?1",
                rusqlite::params![run_id, binary],
            )
            .map_err(unreadable("record the program of a session"))?;
        Ok(())
    }

    pub(crate) fn session_file(&self, run_id: &str, file: &str) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE sessions SET session_file = ?2 WHERE run_id = ?1",
                rusqlite::params![run_id, file],
            )
            .map_err(unreadable("record the file of a session"))?;
        Ok(())
    }

    pub(crate) fn session_wake(&self, run_id: &str, wake: &str) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE sessions SET wake = ?2 WHERE run_id = ?1",
                rusqlite::params![run_id, wake],
            )
            .map_err(unreadable("record how a session wakes"))?;
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

    pub(crate) fn claims_marked_by(&self, run_id: &str) -> Result<Vec<(Repo, u64)>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT DISTINCT repos.host, repos.owner, repos.name, entries.issue
                 FROM entries JOIN repos ON repos.id = entries.repo_id
                 WHERE instr(entries.body, 'run-id=' || ?1 || ' ') > 0
                 ORDER BY repos.host, repos.owner, repos.name, entries.issue",
            )
            .map_err(unreadable("read the claims a run marked"))?;
        statement
            .query_map([run_id], |row| {
                Ok((
                    Repo {
                        host: row.get(0)?,
                        owner: row.get(1)?,
                        name: row.get(2)?,
                    },
                    seconds(row.get(3)?),
                ))
            })
            .map_err(unreadable("read the claims a run marked"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the claims a run marked"))
    }

    pub(crate) fn sessions(&self) -> Result<Vec<SessionRow>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT run_id, runtime, session_id, repo_dir, started_at, last_seen, ended_at,
                        end_reason, name, build, wake, pane, host_pid, host_name, binary,
                        session_file FROM sessions
                 ORDER BY last_seen DESC, run_id",
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
                    name: row.get(8)?,
                    build: row.get(9)?,
                    wake: row.get(10)?,
                    pane: row.get(11)?,
                    host_pid: row.get::<_, Option<i64>>(12)?.map(seconds),
                    host_name: row.get(13)?,
                    binary: row.get(14)?,
                    session_file: row.get(15)?,
                })
            })
            .map_err(unreadable("read the sessions"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the sessions"))
    }
}

fn contention(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
    )
}
