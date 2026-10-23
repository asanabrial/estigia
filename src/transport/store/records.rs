use rusqlite::{OptionalExtension, TransactionBehavior};

use super::{Failure, Repo, Store, int, unreadable};

/// What a freshly filed issue is recorded with besides its body: the dedup
/// identity, the `blocked-until` clauses and the `review` marker of a
/// finding-filed issue, as the markers the store keeps.
#[derive(Default)]
pub(crate) struct Recorded<'a> {
    pub(crate) identity: Option<&'a str>,
    pub(crate) clauses: &'a [String],
    pub(crate) review: Option<&'a str>,
}

/// One task checklist row as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskRow {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) route: String,
    pub(crate) done: bool,
    pub(crate) commit: Option<String>,
    pub(crate) evidence: Option<String>,
}

/// What one `task` call asks to change; an absent field leaves the row as it was.
pub(crate) struct TaskChange<'a> {
    pub(crate) id: &'a str,
    pub(crate) text: Option<&'a str>,
    pub(crate) route: Option<&'a str>,
    pub(crate) done: Option<bool>,
    pub(crate) commit: Option<&'a str>,
    pub(crate) evidence: Option<&'a str>,
}

impl Store {
    /// The workflow state the store recorded for one issue, `None` when it has
    /// never admitted the issue or recorded no state for it.
    pub(crate) fn issue_state(&self, repo: &Repo, issue: u64) -> Result<Option<String>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(None);
        };
        self.connection()
            .query_row(
                "SELECT state FROM issues WHERE repo_id = ?1 AND number = ?2",
                rusqlite::params![repo_id, int(issue)],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(Option::flatten)
            .map_err(unreadable("read an issue's state"))
    }

    /// Records one `blocked-until` clause, or answers `false` when this exact
    /// clause is already recorded for the issue.
    pub(crate) fn record_blocked_clause(
        &mut self,
        repo: &Repo,
        issue: u64,
        clause: &str,
        now: &str,
    ) -> Result<bool, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a clause write"))?;
        let write = unreadable("record a blocked clause");
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .map_err(&write)?;
        let held: bool = transaction
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM blocked_clauses
                                WHERE repo_id = ?1 AND issue = ?2 AND body = ?3)",
                rusqlite::params![repo_id, int(issue), clause],
                |row| row.get(0),
            )
            .map_err(&write)?;
        if !held {
            transaction
                .execute(
                    "INSERT INTO blocked_clauses (repo_id, issue, body, at)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![repo_id, int(issue), clause, now],
                )
                .map_err(&write)?;
        }
        transaction
            .commit()
            .map_err(unreadable("commit a clause write"))?;
        Ok(!held)
    }

    /// Removes every clause recorded for one issue and says how many there were.
    pub(crate) fn clear_blocked_clauses(
        &mut self,
        repo: &Repo,
        issue: u64,
    ) -> Result<usize, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(0);
        };
        self.connection()
            .execute(
                "DELETE FROM blocked_clauses WHERE repo_id = ?1 AND issue = ?2",
                rusqlite::params![repo_id, int(issue)],
            )
            .map_err(unreadable("clear blocked clauses"))
    }

    /// Every checklist row of one issue, in the order they were first recorded.
    pub(crate) fn tasks(&self, repo: &Repo, issue: u64) -> Result<Vec<TaskRow>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(Vec::new());
        };
        read_tasks(self.connection(), repo_id, issue).map_err(unreadable("read the tasks"))
    }

    /// Applies one change to a checklist row, creating it when it is new.
    ///
    /// A new row needs its text and its route; a row cannot be done without a
    /// commit identity or an `evidence` note, whether recorded now or before.
    /// Refused with nothing written otherwise.
    pub(crate) fn change_task(
        &mut self,
        repo: &Repo,
        issue: u64,
        change: &TaskChange<'_>,
        now: &str,
    ) -> Result<TaskRow, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a task write"))?;
        let write = unreadable("record a task");
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .map_err(&write)?;
        let existing = read_tasks(&transaction, repo_id, issue)
            .map_err(&write)?
            .into_iter()
            .find(|row| row.id == change.id);
        let row = match existing {
            Some(row) => TaskRow {
                id: row.id,
                text: change.text.map_or(row.text, ToOwned::to_owned),
                route: change.route.map_or(row.route, ToOwned::to_owned),
                done: change.done.unwrap_or(row.done),
                commit: change.commit.map(ToOwned::to_owned).or(row.commit),
                evidence: change.evidence.map(ToOwned::to_owned).or(row.evidence),
            },
            None => {
                let (Some(text), Some(route)) = (change.text, change.route) else {
                    return Err(Failure::Stop(serde_json::json!({
                        "ok": false,
                        "reason": "task-incomplete",
                        "detail": "a task not yet recorded needs its `text` and its `route`",
                        "action": "call `task` again with `text` and `route` as well as the id",
                    })));
                };
                TaskRow {
                    id: change.id.to_owned(),
                    text: text.to_owned(),
                    route: route.to_owned(),
                    done: change.done.unwrap_or(false),
                    commit: change.commit.map(ToOwned::to_owned),
                    evidence: change.evidence.map(ToOwned::to_owned),
                }
            }
        };
        if row.done && row.commit.is_none() && row.evidence.is_none() {
            return Err(Failure::Stop(serde_json::json!({
                "ok": false,
                "reason": "task-done-without-commit",
                "detail": "a task is done once its work-unit commit exists, or its `evidence` \
                           names where the work is recorded when no commit was required",
                "action": "call `task` again with `status` done and either `commit` naming the \
                           work-unit commit or `evidence` naming where the work is recorded",
            })));
        }
        transaction
            .execute(
                "INSERT INTO tasks (repo_id, issue, id, text, route, done, commit_id, evidence, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT (repo_id, issue, id) DO UPDATE SET
                    text = excluded.text, route = excluded.route, done = excluded.done,
                    commit_id = excluded.commit_id, evidence = excluded.evidence, at = excluded.at",
                rusqlite::params![
                    repo_id,
                    int(issue),
                    row.id,
                    row.text,
                    row.route,
                    row.done,
                    row.commit,
                    row.evidence,
                    now
                ],
            )
            .map_err(&write)?;
        transaction
            .commit()
            .map_err(unreadable("commit a task write"))?;
        Ok(row)
    }
}

fn read_tasks(
    connection: &rusqlite::Connection,
    repo_id: i64,
    issue: u64,
) -> rusqlite::Result<Vec<TaskRow>> {
    let mut statement = connection.prepare(
        "SELECT id, text, route, done, commit_id, evidence FROM tasks
         WHERE repo_id = ?1 AND issue = ?2 ORDER BY rowid",
    )?;
    statement
        .query_map(rusqlite::params![repo_id, int(issue)], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                text: row.get(1)?,
                route: row.get(2)?,
                done: row.get(3)?,
                commit: row.get(4)?,
                evidence: row.get(5)?,
            })
        })?
        .collect()
}
