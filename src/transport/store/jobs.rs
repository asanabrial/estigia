use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ledger::Tx;
use super::{Repo, Store, int, unreadable};
use crate::harness::session::{JOB_RETENTION_DAYS, JOB_SETTLE_SECONDS, JOBS_PER_REPOSITORY};
use crate::reviewer::Receipt;
use crate::transport::Failure;

const TERMINAL_STATES: [&str; 3] = ["admitted", "stale", "abandoned"];

const COLUMNS: &str = "id, run_id, name, epoch, pr, head, base, digest, payload_digest, offered, \
                       attempt, pid, started_at, deadline_at, dir, state";

#[derive(Debug)]
pub(crate) enum Inserted {
    Recorded,
    SlotTaken,
}

pub(crate) struct NewJob<'a> {
    pub(crate) id: &'a str,
    pub(crate) run_id: &'a str,
    pub(crate) name: &'a str,
    pub(crate) receipt: &'a Receipt,
    pub(crate) offered: &'a serde_json::Value,
    pub(crate) attempt: Option<&'a str>,
    pub(crate) started_at: u64,
    pub(crate) deadline_at: u64,
    pub(crate) dir: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) run_id: String,
    pub(crate) name: String,
    pub(crate) receipt: Receipt,
    pub(crate) offered: serde_json::Value,
    pub(crate) attempt: Option<String>,
    pub(crate) pid: Option<u32>,
    pub(crate) started_at: u64,
    pub(crate) deadline_at: u64,
    pub(crate) dir: String,
    pub(crate) state: String,
}

fn job_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let offered: String = row.get(9)?;
    Ok(Job {
        id: row.get(0)?,
        run_id: row.get(1)?,
        name: row.get(2)?,
        receipt: Receipt {
            epoch: row.get(3)?,
            pr: u64::try_from(row.get::<_, i64>(4)?).unwrap_or_default(),
            head: row.get(5)?,
            base: row.get(6)?,
            digest: row.get(7)?,
            payload_digest: row.get(8)?,
        },
        offered: serde_json::from_str(&offered).unwrap_or(serde_json::Value::Null),
        attempt: row.get(10)?,
        pid: row
            .get::<_, Option<i64>>(11)?
            .and_then(|pid| u32::try_from(pid).ok()),
        started_at: u64::try_from(row.get::<_, i64>(12)?).unwrap_or_default(),
        deadline_at: u64::try_from(row.get::<_, i64>(13)?).unwrap_or_default(),
        dir: row.get(14)?,
        state: row.get(15)?,
    })
}

fn live_slot(
    connection: &Connection,
    repo_id: i64,
    issue: u64,
    epoch: &str,
    name: &str,
) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM review_jobs WHERE repo_id = ?1 AND issue = ?2
            AND epoch = ?3 AND name = ?4 AND state IN ('running', 'admitting'))",
        rusqlite::params![repo_id, int(issue), epoch, name],
        |row| row.get(0),
    )
}

impl Store {
    pub(super) fn jobs_on(
        &self,
        repo: &Repo,
        issue: u64,
        epoch: &str,
    ) -> Result<Vec<Job>, Failure> {
        let mut statement = self
            .connection()
            .prepare(&format!(
                "SELECT {COLUMNS} FROM review_jobs WHERE issue = ?4 AND epoch = ?5 AND repo_id =
                    (SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3)
                    ORDER BY rowid"
            ))
            .map_err(unreadable("read the review jobs"))?;
        let rows = statement
            .query_map(
                rusqlite::params![repo.host, repo.owner, repo.name, int(issue), epoch],
                job_of,
            )
            .map_err(unreadable("read the review jobs"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the review jobs"))?;
        let mut newest: Vec<Job> = Vec::new();
        for job in rows {
            newest.retain(|held| held.name != job.name);
            newest.push(job);
        }
        Ok(newest)
    }

    /// The newest relay job for one run and reviewer name, in any state, with the
    /// issue it belongs to.
    ///
    /// The relay's job is recorded by `launch_review_input` and admitted by
    /// `estigia review admit`, neither of which is told the issue the job was
    /// launched for: the host names only the run and the reviewer role. A job
    /// already admitted is returned too, so a second admission answers instead of
    /// reporting the slot missing.
    pub(crate) fn relay_job(
        &self,
        run_id: &str,
        name: &str,
    ) -> Result<Option<(u64, Job)>, Failure> {
        self.connection()
            .query_row(
                &format!(
                    "SELECT {COLUMNS}, issue FROM review_jobs WHERE run_id = ?1 AND name = ?2
                        ORDER BY rowid DESC LIMIT 1"
                ),
                rusqlite::params![run_id, name],
                |row| {
                    let job = job_of(row)?;
                    let issue = u64::try_from(row.get::<_, i64>(16)?).unwrap_or_default();
                    Ok((issue, job))
                },
            )
            .optional()
            .map_err(unreadable("read a relay review job"))
    }

    /// Bounds the job directories under `jobs_root` and settles `running` rows
    /// no lock can still be holding.
    pub(crate) fn bound_jobs(&self, jobs_root: &Path, now: u64) -> Result<(), Failure> {
        for job in self.bound_job_rows()? {
            if job.state != "running" || job.deadline_at.saturating_add(JOB_SETTLE_SECONDS) >= now {
                continue;
            }
            match crate::reviewer::job::job_status(Path::new(&job.dir)) {
                Ok(crate::reviewer::job::JobStatus::Running) | Err(_) => continue,
                Ok(
                    crate::reviewer::job::JobStatus::Finished(_)
                    | crate::reviewer::job::JobStatus::Lost,
                ) => {}
            }
            self.abandon_running_job(&job.id)?;
        }
        let rows = self.bound_job_rows()?;
        let by_dir: HashMap<String, &BoundJob> =
            rows.iter().map(|job| (job.dir.clone(), job)).collect();
        let Ok(entries) = std::fs::read_dir(jobs_root) else {
            return Ok(());
        };
        let mut by_repo: HashMap<i64, Vec<(PathBuf, u64)>> = HashMap::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.parent() != Some(jobs_root)
                || !entry.file_type().is_ok_and(|kind| kind.is_dir())
            {
                continue;
            }
            let Some(modified) = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs())
            else {
                continue;
            };
            let row = by_dir.get(path.to_string_lossy().as_ref()).copied();
            let terminal = row.is_some_and(|row| TERMINAL_STATES.contains(&row.state.as_str()));
            if (row.is_none() || terminal)
                && now.saturating_sub(modified) >= JOB_RETENTION_DAYS * 24 * 60 * 60
            {
                let _ = std::fs::remove_dir_all(&path);
                continue;
            }
            if let Some(row) = row.filter(|_| terminal) {
                by_repo
                    .entry(row.repo_id)
                    .or_default()
                    .push((path, modified));
            }
        }
        for mut dirs in by_repo.into_values() {
            dirs.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
            for (path, _) in dirs.into_iter().skip(JOBS_PER_REPOSITORY) {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
        Ok(())
    }

    fn bound_job_rows(&self) -> Result<Vec<BoundJob>, Failure> {
        let mut statement = self
            .connection()
            .prepare("SELECT id, repo_id, dir, state, deadline_at FROM review_jobs")
            .map_err(unreadable("read the review jobs"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(BoundJob {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    dir: row.get(2)?,
                    state: row.get(3)?,
                    deadline_at: u64::try_from(row.get::<_, i64>(4)?).unwrap_or_default(),
                })
            })
            .map_err(unreadable("read the review jobs"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the review jobs"))?;
        Ok(rows)
    }

    fn abandon_running_job(&self, id: &str) -> Result<bool, Failure> {
        self.connection()
            .execute(
                "UPDATE review_jobs SET state = 'abandoned' WHERE id = ?1 AND state = 'running'",
                [id],
            )
            .map(|changed| changed == 1)
            .map_err(unreadable("settle a running review job"))
    }
}

/// One row the daily bound reads: the directory it names and what may be
/// removed from it.
struct BoundJob {
    id: String,
    repo_id: i64,
    dir: String,
    state: String,
    deadline_at: u64,
}

impl Tx<'_> {
    pub(crate) fn insert_job(&self, job: &NewJob<'_>) -> Result<Inserted, Failure> {
        let taken = live_slot(
            self.transaction,
            self.repo_id,
            self.issue,
            &job.receipt.epoch,
            job.name,
        )
        .map_err(unreadable("look for a running review job"))?;
        if taken {
            return Ok(Inserted::SlotTaken);
        }
        let receipt = job.receipt;
        self.transaction
            .execute(
                "INSERT INTO review_jobs (id, repo_id, issue, run_id, name, epoch, pr, head, base,
                    digest, payload_digest, offered, attempt, started_at, deadline_at, dir, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    'running')",
                rusqlite::params![
                    job.id,
                    self.repo_id,
                    int(self.issue),
                    job.run_id,
                    job.name,
                    receipt.epoch,
                    int(receipt.pr),
                    receipt.head,
                    receipt.base,
                    receipt.digest,
                    receipt.payload_digest,
                    job.offered.to_string(),
                    job.attempt,
                    int(job.started_at),
                    int(job.deadline_at),
                    job.dir,
                ],
            )
            .map(|_| Inserted::Recorded)
            .or_else(|error| match error {
                rusqlite::Error::SqliteFailure(code, _)
                    if code.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    Ok(Inserted::SlotTaken)
                }
                other => Err(unreadable("record a review job")(other)),
            })
    }

    pub(crate) fn job_for_slot(&self, epoch: &str, name: &str) -> Result<Option<Job>, Failure> {
        self.transaction
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM review_jobs WHERE repo_id = ?1 AND issue = ?2
                        AND epoch = ?3 AND name = ?4 ORDER BY rowid DESC LIMIT 1"
                ),
                rusqlite::params![self.repo_id, int(self.issue), epoch, name],
                job_of,
            )
            .optional()
            .map_err(unreadable("read a review job"))
    }

    #[cfg(test)]
    pub(crate) fn job_by_id(&self, id: &str) -> Result<Option<Job>, Failure> {
        self.transaction
            .query_row(
                &format!("SELECT {COLUMNS} FROM review_jobs WHERE id = ?1 AND repo_id = ?2"),
                rusqlite::params![id, self.repo_id],
                job_of,
            )
            .optional()
            .map_err(unreadable("read a review job"))
    }

    pub(crate) fn live_job_for_name(&self, name: &str) -> Result<Option<Job>, Failure> {
        self.transaction
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM review_jobs WHERE repo_id = ?1 AND issue = ?2
                        AND name = ?3 AND state IN ('running', 'admitting')
                        ORDER BY rowid DESC LIMIT 1"
                ),
                rusqlite::params![self.repo_id, int(self.issue), name],
                job_of,
            )
            .optional()
            .map_err(unreadable("read a review job"))
    }

    pub(crate) fn compare_and_set_state(
        &self,
        id: &str,
        from: &str,
        to: &str,
    ) -> Result<bool, Failure> {
        self.transaction
            .execute(
                "UPDATE review_jobs SET state = ?1 WHERE id = ?2 AND state = ?3
                    AND repo_id = ?4 AND issue = ?5",
                rusqlite::params![to, id, from, self.repo_id, int(self.issue)],
            )
            .map(|changed| changed == 1)
            .map_err(unreadable("move a review job"))
    }

    pub(crate) fn mark_spawned(&self, id: &str, pid: u32) -> Result<(), Failure> {
        self.transaction
            .execute(
                "UPDATE review_jobs SET pid = ?1 WHERE id = ?2 AND repo_id = ?3 AND issue = ?4",
                rusqlite::params![i64::from(pid), id, self.repo_id, int(self.issue)],
            )
            .map(|_| ())
            .map_err(unreadable("record a review job's process"))
    }
}
