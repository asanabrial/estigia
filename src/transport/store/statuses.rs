use super::{Store, int, unreadable};
use crate::transport::Failure;

const REPLACE_WITHIN_SECONDS: u64 = 60;
const KEPT: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusEntry {
    pub(crate) status: String,
    pub(crate) phase: Option<String>,
    pub(crate) set_at: u64,
}

impl Store {
    pub(crate) fn status_set(
        &self,
        run_id: &str,
        status: &str,
        phase: Option<&str>,
        now: u64,
    ) -> Result<bool, Failure> {
        let what = "record a self-reported status";
        let connection = self.connection();
        let replaced = connection
            .execute(
                "UPDATE run_statuses SET status = ?2, phase = ?3, set_at = ?4
                 WHERE id = (SELECT id FROM run_statuses WHERE run_id = ?1
                             ORDER BY id DESC LIMIT 1)
                   AND ?4 - set_at < ?5",
                rusqlite::params![run_id, status, phase, int(now), int(REPLACE_WITHIN_SECONDS)],
            )
            .map_err(unreadable(what))?
            > 0;
        if !replaced {
            connection
                .execute(
                    "INSERT INTO run_statuses (run_id, status, phase, set_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![run_id, status, phase, int(now)],
                )
                .map_err(unreadable(what))?;
            connection
                .execute(
                    "DELETE FROM run_statuses WHERE run_id = ?1 AND id NOT IN
                     (SELECT id FROM run_statuses WHERE run_id = ?1 ORDER BY id DESC LIMIT ?2)",
                    rusqlite::params![run_id, KEPT],
                )
                .map_err(unreadable(what))?;
        }
        Ok(replaced)
    }

    #[cfg(test)]
    pub(crate) fn statuses(&self, run_id: &str) -> Result<Vec<StatusEntry>, Failure> {
        let what = "read the self-reported statuses";
        let mut statement = self
            .connection()
            .prepare(
                "SELECT status, phase, set_at FROM run_statuses WHERE run_id = ?1
                 ORDER BY id DESC",
            )
            .map_err(unreadable(what))?;
        statement
            .query_map([run_id], |row| {
                Ok(StatusEntry {
                    status: row.get(0)?,
                    phase: row.get(1)?,
                    set_at: u64::try_from(row.get::<_, i64>(2)?).unwrap_or_default(),
                })
            })
            .map_err(unreadable(what))?
            .collect::<Result<_, _>>()
            .map_err(unreadable(what))
    }

    pub(crate) fn latest_statuses(
        &self,
    ) -> Result<std::collections::BTreeMap<String, StatusEntry>, Failure> {
        let what = "read the newest self-reported statuses";
        let mut statement = self
            .connection()
            .prepare(
                "SELECT run_id, status, phase, set_at FROM run_statuses
                 WHERE id IN (SELECT MAX(id) FROM run_statuses GROUP BY run_id)",
            )
            .map_err(unreadable(what))?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    StatusEntry {
                        status: row.get(1)?,
                        phase: row.get(2)?,
                        set_at: u64::try_from(row.get::<_, i64>(3)?).unwrap_or_default(),
                    },
                ))
            })
            .map_err(unreadable(what))?
            .collect::<Result<_, _>>()
            .map_err(unreadable(what))
    }

    pub(crate) fn first_open_tasks(
        &self,
    ) -> Result<std::collections::BTreeMap<u64, (usize, usize)>, Failure> {
        let what = "read the task checklists";
        let mut statement = self
            .connection()
            .prepare("SELECT repo_id, issue, done FROM tasks ORDER BY repo_id, issue, rowid")
            .map_err(unreadable(what))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(unreadable(what))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable(what))?;
        let mut checklists: std::collections::BTreeMap<u64, Vec<(usize, Option<usize>)>> =
            std::collections::BTreeMap::new();
        let mut last: Option<(i64, u64)> = None;
        let mut position = 0;
        for (repo, issue, done) in rows {
            if last != Some((repo, issue)) {
                checklists.entry(issue).or_default().push((0, None));
                last = Some((repo, issue));
                position = 0;
            }
            position += 1;
            if let Some(entry) = checklists.entry(issue).or_default().last_mut() {
                entry.0 = position;
                if !done && entry.1.is_none() {
                    entry.1 = Some(position);
                }
            }
        }
        Ok(checklists
            .into_iter()
            .filter_map(|(issue, lists)| match lists.as_slice() {
                [(total, Some(first))] => Some((issue, (*first, *total))),
                _ => None,
            })
            .collect())
    }
}
