use super::{Store, int, unreadable};
use crate::transport::Failure;

pub(crate) struct Need<'a> {
    pub(crate) kind: &'a str,
    pub(crate) repo: &'a str,
    pub(crate) issue: Option<u64>,
    pub(crate) run_id: Option<&'a str>,
    pub(crate) dedup_key: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct NeedRow {
    pub(crate) kind: String,
    pub(crate) repo: String,
    pub(crate) issue: Option<u64>,
    pub(crate) run_id: Option<String>,
    pub(crate) at: u64,
    pub(crate) delivery: String,
}

impl Store {
    /// Records that a person is needed, once per `(repo, dedup_key)`.
    ///
    /// Answers whether this call wrote the record: a repeat of the same moment
    /// writes nothing and answers `false`.
    pub(crate) fn need_recorded(&self, need: &Need<'_>, now: u64) -> Result<bool, Failure> {
        self.connection()
            .execute(
                "INSERT OR IGNORE INTO needs_person (kind, repo, issue, run_id, dedup_key, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    need.kind,
                    need.repo,
                    need.issue.map(int),
                    need.run_id,
                    need.dedup_key,
                    int(now)
                ],
            )
            .map(|changed| changed > 0)
            .map_err(unreadable("record that a person is needed"))
    }

    /// The records, newest first, at most `limit` of them.
    pub(crate) fn needs_person(&self, limit: u64) -> Result<Vec<NeedRow>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT kind, repo, issue, run_id, at, delivery FROM needs_person
                 ORDER BY id DESC LIMIT ?1",
            )
            .map_err(unreadable("read who is needed"))?;
        statement
            .query_map([int(limit)], |row| {
                Ok(NeedRow {
                    kind: row.get(0)?,
                    repo: row.get(1)?,
                    issue: row
                        .get::<_, Option<i64>>(2)?
                        .map(|issue| u64::try_from(issue).unwrap_or_default()),
                    run_id: row.get(3)?,
                    at: u64::try_from(row.get::<_, i64>(4)?).unwrap_or_default(),
                    delivery: row.get(5)?,
                })
            })
            .map_err(unreadable("read who is needed"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read who is needed"))
    }
}
