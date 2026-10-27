use super::{Store, int, unreadable};
use crate::transport::Failure;

pub(crate) const TRANSITION: u8 = 0;
const ENTRY: u8 = 1;
pub(crate) const MESSAGE: u8 = 2;

pub(crate) struct FeedWindow<'a> {
    pub(crate) since: u64,
    pub(crate) until: u64,
    pub(crate) repo: Option<&'a str>,
    pub(crate) issue: Option<u64>,
    pub(crate) run: Option<&'a str>,
    pub(crate) bodies: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedRow {
    pub(crate) source: u8,
    pub(crate) id: i64,
    pub(crate) at: u64,
    pub(crate) repo: Option<String>,
    pub(crate) issue: Option<u64>,
    pub(crate) run_id: Option<String>,
    pub(crate) kind: String,
    pub(crate) from: Option<String>,
    pub(crate) to: Option<String>,
    pub(crate) body: String,
}

fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

const TRANSITIONS: &str = "
    SELECT t.seq, ts, r.owner || '/' || r.name, t.issue, t.run_id, 'transition',
           t.from_state, t.to_state, ''
    FROM (SELECT *, CAST(strftime('%s', at) AS INTEGER) AS ts FROM transitions) t
    JOIN repos r ON r.id = t.repo_id
    WHERE ts BETWEEN ?1 AND ?2
      AND (?3 IS NULL OR r.owner || '/' || r.name = ?3)
      AND (?4 IS NULL OR t.issue = ?4)
      AND (?5 IS NULL OR t.run_id = ?5)
      AND ?6 IN (0, 1)";

const ENTRIES: &str = "
    SELECT e.seq, ts, r.owner || '/' || r.name, e.issue, e.run_id, e.kind, NULL, NULL,
           CASE WHEN ?6 THEN e.body ELSE '' END
    FROM (SELECT *, CAST(strftime('%s', created_at) AS INTEGER) AS ts FROM entries) e
    JOIN repos r ON r.id = e.repo_id
    WHERE e.kind IS NOT NULL AND ts BETWEEN ?1 AND ?2
      AND (?3 IS NULL OR r.owner || '/' || r.name = ?3)
      AND (?4 IS NULL OR e.issue = ?4)
      AND (?5 IS NULL OR e.run_id = ?5)";

const MESSAGES: &str = "
    SELECT m.id, m.created_at, NULL, NULL, m.sender_run_id, 'message', NULL, m.recipient_run_id,
           CASE WHEN ?6 THEN m.body ELSE '' END
    FROM messages m
    WHERE m.created_at BETWEEN ?1 AND ?2
      AND ?3 IS NULL AND ?4 IS NULL
      AND (?5 IS NULL OR m.sender_run_id = ?5 OR m.recipient_run_id = ?5)";

impl Store {
    pub(crate) fn repository_known(&self, repo: &str) -> Result<bool, Failure> {
        self.connection()
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM repos WHERE owner || '/' || name = ?1)",
                [repo],
                |row| row.get(0),
            )
            .map_err(unreadable("look a repository up"))
    }

    pub(crate) fn feed_rows(&self, window: &FeedWindow<'_>) -> Result<Vec<FeedRow>, Failure> {
        let mut rows = Vec::new();
        for (source, sql) in [
            (TRANSITION, TRANSITIONS),
            (ENTRY, ENTRIES),
            (MESSAGE, MESSAGES),
        ] {
            let mut statement = self
                .connection()
                .prepare(sql)
                .map_err(unreadable("read the feed"))?;
            let read = |row: &rusqlite::Row<'_>| {
                Ok(FeedRow {
                    source,
                    id: row.get(0)?,
                    at: seconds(row.get(1)?),
                    repo: row.get(2)?,
                    issue: row.get::<_, Option<i64>>(3)?.map(seconds),
                    run_id: row.get(4)?,
                    kind: row.get(5)?,
                    from: row.get(6)?,
                    to: row.get(7)?,
                    body: row.get(8)?,
                })
            };
            let found = statement
                .query_map(
                    rusqlite::params![
                        int(window.since),
                        int(window.until),
                        window.repo,
                        window.issue.map(int),
                        window.run,
                        window.bodies
                    ],
                    read,
                )
                .map_err(unreadable("read the feed"))?;
            for row in found {
                rows.push(row.map_err(unreadable("read the feed"))?);
            }
        }
        Ok(rows)
    }
}
