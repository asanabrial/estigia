use rusqlite::OptionalExtension as _;

use super::messages::{MessageSent, MessageShape};
use super::{Failure, Store, int, unreadable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Duty {
    pub(crate) repo_dir: String,
    pub(crate) issue: u64,
    pub(crate) runtime: String,
    pub(crate) sender: String,
    pub(crate) due_at: Option<u64>,
    pub(crate) accepted_by: Option<String>,
    pub(crate) accepted_at: Option<u64>,
    pub(crate) outcome: Option<String>,
    pub(crate) result: Option<String>,
    pub(crate) settled_at: Option<u64>,
}

impl Duty {
    pub(crate) fn status(&self, now: u64) -> &str {
        if let Some(outcome) = &self.outcome {
            outcome
        } else if self.due_at.is_some_and(|due| due <= now) {
            "overdue"
        } else if self.accepted_by.is_some() {
            "accepted"
        } else {
            "offered"
        }
    }
}

const COLUMNS: &str = "repo_dir, issue, runtime, sender, due_at, accepted_by, accepted_at,
    outcome, result, settled_at";

fn seconds(value: Option<i64>) -> Option<u64> {
    value.map(|value| u64::try_from(value).unwrap_or_default())
}

fn duty_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Duty> {
    Ok(Duty {
        repo_dir: row.get(0)?,
        issue: u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
        runtime: row.get(2)?,
        sender: row.get(3)?,
        due_at: seconds(row.get(4)?),
        accepted_by: row.get(5)?,
        accepted_at: seconds(row.get(6)?),
        outcome: row.get(7)?,
        result: row.get(8)?,
        settled_at: seconds(row.get(9)?),
    })
}

impl Store {
    /// Opens the duty a handoff creates, replacing the one a repeated handoff of the same issue
    /// to the same runtime left behind: a repeat is a new request, not a second report.
    pub(crate) fn duty_opened(
        &self,
        duty: (&str, u64, &str),
        sender: &str,
        due_at: Option<u64>,
    ) -> Result<(), Failure> {
        let (repo_dir, issue, runtime) = duty;
        self.connection()
            .execute(
                "INSERT INTO handoff_duties (repo_dir, issue, runtime, sender, due_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (repo_dir, issue, runtime) DO UPDATE SET
                    sender = excluded.sender, due_at = excluded.due_at,
                    accepted_by = NULL, accepted_at = NULL,
                    outcome = NULL, result = NULL, settled_at = NULL",
                rusqlite::params![repo_dir, int(issue), runtime, sender, due_at.map(int)],
            )
            .map_err(unreadable("open a handoff duty"))?;
        Ok(())
    }

    /// Binds an unaccepted, unsettled duty to the run that accepted it. Only the first
    /// acceptance takes; `false` means nothing changed.
    pub(crate) fn duty_accepted(
        &self,
        duty: (&str, u64, &str),
        run_id: &str,
        now: u64,
    ) -> Result<bool, Failure> {
        let (repo_dir, issue, runtime) = duty;
        let changed = self
            .connection()
            .execute(
                "UPDATE handoff_duties SET accepted_by = ?4, accepted_at = ?5
                 WHERE repo_dir = ?1 AND issue = ?2 AND runtime = ?3
                   AND accepted_by IS NULL AND outcome IS NULL",
                rusqlite::params![repo_dir, int(issue), runtime, run_id, int(now)],
            )
            .map_err(unreadable("accept a handoff"))?;
        Ok(changed == 1)
    }

    pub(crate) fn duty_held_by(&self, run_id: &str, issue: u64) -> Result<Option<Duty>, Failure> {
        self.connection()
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM handoff_duties
                     WHERE accepted_by = ?1 AND issue = ?2
                     ORDER BY outcome IS NOT NULL, accepted_at DESC LIMIT 1"
                ),
                rusqlite::params![run_id, int(issue)],
                duty_row,
            )
            .optional()
            .map_err(unreadable("read a handoff duty"))
    }

    pub(crate) fn duties_sent_by(&self, sender: &str) -> Result<Vec<Duty>, Failure> {
        let mut statement = self
            .connection()
            .prepare(&format!(
                "SELECT {COLUMNS} FROM handoff_duties WHERE sender = ?1
                 ORDER BY issue, runtime, repo_dir"
            ))
            .map_err(unreadable("read the handoff duties"))?;
        statement
            .query_map(rusqlite::params![sender], duty_row)
            .map_err(unreadable("read the handoff duties"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the handoff duties"))
    }

    /// Records the one word that ends a duty. The predicate is the rule: a duty that already has
    /// an outcome, or that `run_id` did not accept, changes nothing and answers `false`.
    pub(crate) fn duty_settled(
        &self,
        duty: &Duty,
        run_id: &str,
        outcome: &str,
        result: &str,
        now: u64,
    ) -> Result<bool, Failure> {
        let changed = self
            .connection()
            .execute(
                "UPDATE handoff_duties SET outcome = ?5, result = ?6, settled_at = ?7
                 WHERE repo_dir = ?1 AND issue = ?2 AND runtime = ?3
                   AND accepted_by = ?4 AND outcome IS NULL",
                rusqlite::params![
                    duty.repo_dir,
                    int(duty.issue),
                    duty.runtime,
                    run_id,
                    outcome,
                    result,
                    int(now)
                ],
            )
            .map_err(unreadable("settle a handoff"))?;
        Ok(changed == 1)
    }

    /// Ends every duty `run_id` accepted and never settled as `abandoned`, and tells each sender
    /// by an `escalation` from the run that ended.
    pub(super) fn duties_abandoned(&self, run_id: &str, now: u64) -> Result<(), Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "UPDATE handoff_duties SET outcome = 'abandoned', settled_at = ?2
                 WHERE accepted_by = ?1 AND outcome IS NULL
                 RETURNING sender, issue, runtime",
            )
            .map_err(unreadable("abandon the handoffs of an ended run"))?;
        let abandoned = statement
            .query_map(rusqlite::params![run_id, int(now)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(unreadable("abandon the handoffs of an ended run"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("abandon the handoffs of an ended run"))?;
        for (sender, issue, runtime) in abandoned {
            let body = format!(
                "{run_id} ended without sending `done` or `blocked` for #{issue}, which was \
                 handed to {runtime}: the handoff ends as abandoned"
            );
            self.message_sent_shaped(
                &MessageSent {
                    sender_run_id: Some(run_id),
                    recipient_run_id: &sender,
                    body: &body,
                },
                &MessageShape {
                    kind: Some("escalation"),
                    ..MessageShape::default()
                },
                now,
            )?;
        }
        Ok(())
    }
}
