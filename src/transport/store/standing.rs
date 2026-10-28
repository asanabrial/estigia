use rusqlite::OptionalExtension;

use super::{Repo, Store, int, unreadable};
use crate::transport::Failure;

impl Store {
    /// Every repository the store holds an issue numbered `number` in, oldest repository first.
    pub(crate) fn repos_holding(&self, number: u64) -> Result<Vec<Repo>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT r.host, r.owner, r.name FROM issues i
                 JOIN repos r ON r.id = i.repo_id WHERE i.number = ?1 ORDER BY r.id",
            )
            .map_err(unreadable("find the repositories of an issue"))?;
        statement
            .query_map([int(number)], |row| {
                Ok(Repo {
                    host: row.get(0)?,
                    owner: row.get(1)?,
                    name: row.get(2)?,
                })
            })
            .and_then(Iterator::collect)
            .map_err(unreadable("find the repositories of an issue"))
    }

    /// The workflow state the store recorded for one issue: `None` when it holds no such issue,
    /// `Some(None)` when it holds the issue with no state yet.
    pub(crate) fn recorded_state(
        &self,
        repo: &Repo,
        number: u64,
    ) -> Result<Option<Option<String>>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(None);
        };
        self.connection()
            .query_row(
                "SELECT state FROM issues WHERE repo_id = ?1 AND number = ?2",
                rusqlite::params![repo_id, int(number)],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read an issue's state"))
    }

    /// Whether the store has ever seen `repo`.
    pub(crate) fn holds_repo(&self, repo: &Repo) -> Result<bool, Failure> {
        Ok(self.repo_id(repo)?.is_some())
    }
}
