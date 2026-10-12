use rusqlite::{OptionalExtension, TransactionBehavior};

use super::{Failure, Repo, Store, int, unreadable};
use crate::transport::outbox::{PROVISIONAL_BASE, is_provisional};

const PROVISIONAL_COUNTER: &str = "provisional-counter";
const OUTBOX_LEASE: &str = "outbox-lease";

fn lease_key(repo: &Repo) -> String {
    format!("{OUTBOX_LEASE} {}/{}/{}", repo.host, repo.owner, repo.name)
}

/// What a queued write does when it is finally sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutboxKind {
    Create,
    Comment,
    Reconverge,
    Link,
}

impl OutboxKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Comment => "comment",
            Self::Reconverge => "reconverge",
            Self::Link => "link",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "create" => Self::Create,
            "comment" => Self::Comment,
            "reconverge" => Self::Reconverge,
            "link" => Self::Link,
            _ => return None,
        })
    }
}

/// Where a queued write stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutboxState {
    Queued,
    Sending,
    Sent,
}

impl OutboxState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "queued" => Self::Queued,
            "sending" => Self::Sending,
            "sent" => Self::Sent,
            _ => return None,
        })
    }
}

/// One queued GitHub write, as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutboxRow {
    pub(crate) seq: u64,
    pub(crate) issue: u64,
    pub(crate) kind: OutboxKind,
    pub(crate) payload: serde_json::Value,
    pub(crate) op_id: String,
    pub(crate) state: OutboxState,
    pub(crate) enqueued_at: String,
    pub(crate) attempted_at: Option<String>,
    pub(crate) detail: Option<String>,
}

const COLUMNS: &str = "seq, issue, kind, payload, op_id, state, enqueued_at, attempted_at, detail";

fn row_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxRow> {
    let corrupt = |what: &str, value: String| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("an outbox row carries {what} {value:?}").into(),
        )
    };
    let kind: String = row.get(2)?;
    let state: String = row.get(5)?;
    let payload: String = row.get(3)?;
    Ok(OutboxRow {
        seq: u64::try_from(row.get::<_, i64>(0)?).unwrap_or_default(),
        issue: u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
        kind: OutboxKind::parse(&kind).ok_or_else(|| corrupt("the kind", kind))?,
        payload: serde_json::from_str(&payload).map_err(|_| corrupt("the payload", payload))?,
        op_id: row.get(4)?,
        state: OutboxState::parse(&state).ok_or_else(|| corrupt("the state", state))?,
        enqueued_at: row.get(6)?,
        attempted_at: row.get(7)?,
        detail: row.get(8)?,
    })
}

impl Store {
    /// Appends one write to the queue and answers its sequence number.
    ///
    /// A `reconverge` is a statement about where an issue should end up, so a
    /// newer one replaces a queued older one for the same issue and takes the
    /// tail; one already `sending` is left to finish.
    pub(crate) fn enqueue(
        &mut self,
        repo: &Repo,
        issue: u64,
        kind: OutboxKind,
        payload: &serde_json::Value,
        op_id: &str,
        now: &str,
    ) -> Result<u64, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a queue write"))?;
        let write = unreadable("queue a write");
        transaction
            .execute(
                "INSERT OR IGNORE INTO repos (host, owner, name) VALUES (?1, ?2, ?3)",
                rusqlite::params![repo.host, repo.owner, repo.name],
            )
            .map_err(&write)?;
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .map_err(&write)?;
        let mut payload = payload.clone();
        if kind == OutboxKind::Reconverge {
            let replaced: Option<String> = transaction
                .query_row(
                    "SELECT payload FROM outbox
                     WHERE repo_id = ?1 AND issue = ?2 AND kind = 'reconverge'
                       AND state = 'queued'",
                    rusqlite::params![repo_id, int(issue)],
                    |row| row.get(0),
                )
                .optional()
                .map_err(&write)?;
            if let Some(older) = replaced.and_then(|text| serde_json::from_str(&text).ok())
                && let (Some(newer), serde_json::Value::Object(older)) =
                    (payload.as_object_mut(), older)
            {
                for (name, value) in older {
                    newer.entry(name).or_insert(value);
                }
            }
            transaction
                .execute(
                    "DELETE FROM outbox
                     WHERE repo_id = ?1 AND issue = ?2 AND kind = 'reconverge'
                       AND state = 'queued'",
                    rusqlite::params![repo_id, int(issue)],
                )
                .map_err(&write)?;
        }
        transaction
            .execute(
                "INSERT INTO outbox (repo_id, issue, kind, payload, op_id, state, enqueued_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
                rusqlite::params![
                    repo_id,
                    int(issue),
                    kind.as_str(),
                    payload.to_string(),
                    op_id,
                    now
                ],
            )
            .map_err(&write)?;
        let seq = transaction.last_insert_rowid();
        transaction
            .commit()
            .map_err(unreadable("commit a queue write"))?;
        Ok(u64::try_from(seq).unwrap_or_default())
    }

    /// Every write the queue holds for `repo`, oldest first, sent ones included.
    pub(crate) fn outbox(&self, repo: &Repo) -> Result<Vec<OutboxRow>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(Vec::new());
        };
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT {COLUMNS} FROM outbox WHERE repo_id = ?1 ORDER BY seq"
            ))
            .map_err(unreadable("read the queue"))?;
        statement
            .query_map([repo_id], row_of)
            .and_then(Iterator::collect)
            .map_err(unreadable("read the queue"))
    }

    /// The oldest write for `repo` that has not been sent.
    pub(crate) fn next_outbox(&self, repo: &Repo) -> Result<Option<OutboxRow>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(None);
        };
        self.connection
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM outbox
                     WHERE repo_id = ?1 AND state <> 'sent' ORDER BY seq LIMIT 1"
                ),
                [repo_id],
                row_of,
            )
            .optional()
            .map_err(unreadable("read the next queued write"))
    }

    /// Moves one write to `state`, keeping `detail` as the last thing said about
    /// it. Entering `sending` stamps the attempt.
    pub(crate) fn mark_outbox(
        &mut self,
        seq: u64,
        state: OutboxState,
        detail: Option<&str>,
        now: &str,
    ) -> Result<(), Failure> {
        let changed = self
            .connection
            .execute(
                "UPDATE outbox SET state = ?2, detail = ?3,
                    attempted_at = CASE WHEN ?2 = 'sending' THEN ?4 ELSE attempted_at END
                 WHERE seq = ?1",
                rusqlite::params![int(seq), state.as_str(), detail, now],
            )
            .map_err(unreadable("mark a queued write"))?;
        if changed == 0 {
            return Err(Failure::Write(format!(
                "queued write {seq} is not in the store, so it was not marked {}",
                state.as_str()
            )));
        }
        Ok(())
    }

    /// How many writes in the whole store have not been sent, for any
    /// repository. A store written before the queue existed holds none.
    pub(crate) fn unsent_outbox(&self) -> Result<usize, Failure> {
        let present: bool = self
            .connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'outbox')",
                [],
                |row| row.get(0),
            )
            .map_err(unreadable("look for the queue"))?;
        if !present {
            return Ok(0);
        }
        self.connection
            .query_row(
                "SELECT COUNT(*) FROM outbox WHERE state <> 'sent'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| usize::try_from(count).unwrap_or_default())
            .map_err(unreadable("count the queue"))
    }

    /// Takes the flush lease of `repo` for `holder` until `expires` (epoch
    /// seconds), and answers `None`; or answers the pid of the live holder that
    /// keeps it. Another repository's lease is a different lease.
    ///
    /// A lease whose time has passed, or whose holder `alive` says is gone, is
    /// taken over. The holder renews its own by taking it again.
    pub(crate) fn take_outbox_lease(
        &mut self,
        repo: &Repo,
        holder: u32,
        now: u64,
        expires: u64,
        alive: impl Fn(u32) -> bool,
    ) -> Result<Option<u32>, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a lease"))?;
        let write = unreadable("take the flush lease");
        let key = lease_key(repo);
        let held: Option<String> = transaction
            .query_row(
                "SELECT value FROM store_meta WHERE key = ?1",
                [&key],
                |row| row.get(0),
            )
            .optional()
            .map_err(&write)?;
        if let Some((pid, until)) = held.as_deref().and_then(|text| {
            let (pid, until) = text.split_once(' ')?;
            Some((pid.parse::<u32>().ok()?, until.parse::<u64>().ok()?))
        }) && pid != holder
            && until > now
            && alive(pid)
        {
            return Ok(Some(pid));
        }
        transaction
            .execute(
                "INSERT INTO store_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, format!("{holder} {expires}")],
            )
            .map_err(&write)?;
        transaction.commit().map_err(unreadable("commit a lease"))?;
        Ok(None)
    }

    /// Gives the flush lease of `repo` up when `holder` still has it.
    pub(crate) fn drop_outbox_lease(&mut self, repo: &Repo, holder: u32) -> Result<(), Failure> {
        self.connection
            .execute(
                "DELETE FROM store_meta WHERE key = ?1 AND value LIKE ?2",
                rusqlite::params![lease_key(repo), format!("{holder} %")],
            )
            .map(|_| ())
            .map_err(unreadable("give up the flush lease"))
    }

    /// Reserves the next provisional issue number for `repo`.
    pub(crate) fn allocate_provisional(&mut self, repo: &Repo) -> Result<u64, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a provisional number"))?;
        let write = unreadable("allocate a provisional number");
        transaction
            .execute(
                "INSERT OR IGNORE INTO repos (host, owner, name) VALUES (?1, ?2, ?3)",
                rusqlite::params![repo.host, repo.owner, repo.name],
            )
            .map_err(&write)?;
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .map_err(&write)?;
        let counter: u64 = transaction
            .query_row(
                "SELECT value FROM store_meta WHERE key = ?1",
                [PROVISIONAL_COUNTER],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(&write)?
            .and_then(|text| text.parse().ok())
            .unwrap_or(0)
            + 1;
        transaction
            .execute(
                "INSERT INTO store_meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                rusqlite::params![PROVISIONAL_COUNTER, counter.to_string()],
            )
            .map_err(&write)?;
        let provisional = PROVISIONAL_BASE + counter;
        transaction
            .execute(
                "INSERT INTO provisional_issues (repo_id, provisional, real) VALUES (?1, ?2, NULL)",
                rusqlite::params![repo_id, int(provisional)],
            )
            .map_err(&write)?;
        transaction
            .commit()
            .map_err(unreadable("commit a provisional number"))?;
        Ok(provisional)
    }

    /// What the store knows of a provisional number: `None` when it never
    /// allocated it, `Some(None)` while GitHub has given it no number, and
    /// `Some(Some(real))` once it has.
    #[cfg(test)]
    pub(crate) fn provisional_real(
        &self,
        repo: &Repo,
        provisional: u64,
    ) -> Result<Option<Option<u64>>, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(None);
        };
        self.connection
            .query_row(
                "SELECT real FROM provisional_issues WHERE repo_id = ?1 AND provisional = ?2",
                rusqlite::params![repo_id, int(provisional)],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map(|found| found.map(|real| real.and_then(|real| u64::try_from(real).ok())))
            .map_err(unreadable("read a provisional number"))
    }

    /// [`Store::provisional_real`] for a number whose repository the caller
    /// does not know: provisional numbers come from one counter, so a number
    /// names at most one allocation.
    pub(crate) fn provisional_real_anywhere(
        &self,
        provisional: u64,
    ) -> Result<Option<Option<u64>>, Failure> {
        self.connection
            .query_row(
                "SELECT real FROM provisional_issues WHERE provisional = ?1",
                [int(provisional)],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map(|found| found.map(|real| real.and_then(|real| u64::try_from(real).ok())))
            .map_err(unreadable("read a provisional number"))
    }

    /// The repository a provisional number was allocated in, `None` when this
    /// store never allocated it.
    pub(crate) fn provisional_repo(&self, provisional: u64) -> Result<Option<Repo>, Failure> {
        self.connection
            .query_row(
                "SELECT repos.host, repos.owner, repos.name FROM provisional_issues
                 JOIN repos ON repos.id = provisional_issues.repo_id
                 WHERE provisional_issues.provisional = ?1",
                [int(provisional)],
                |row| {
                    Ok(Repo {
                        host: row.get(0)?,
                        owner: row.get(1)?,
                        name: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(unreadable("read a provisional number"))
    }

    /// Whether the store already holds `issue` as a real issue of `repo`.
    pub(crate) fn holds_issue(&self, repo: &Repo, issue: u64) -> Result<bool, Failure> {
        let Some(repo_id) = self.repo_id(repo)? else {
            return Ok(false);
        };
        self.connection()
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE repo_id = ?1 AND number = ?2)",
                rusqlite::params![repo_id, int(issue)],
                |row| row.get(0),
            )
            .map_err(unreadable("read whether an issue is held"))
    }

    /// Gives a provisional issue the number GitHub issued for it: every row
    /// that named the provisional number names `real` afterwards, in one
    /// transaction, with `sent` marking a queued write sent in the same one.
    ///
    /// Repeating a rekey that already happened is a no-op; rekeying to a
    /// different number, or onto an issue the store already holds, is refused
    /// with nothing written.
    pub(crate) fn rekey(
        &mut self,
        repo: &Repo,
        provisional: u64,
        real: u64,
        sent: Option<u64>,
    ) -> Result<(), Failure> {
        let refuse = |detail: String| Failure::Read(format!("rekey refused: {detail}"));
        if !is_provisional(provisional) || is_provisional(real) {
            return Err(refuse(format!(
                "{provisional} is not provisional or {real} still is"
            )));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin a rekey"))?;
        let write = unreadable("rekey an issue");
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .optional()
            .map_err(&write)?
            .ok_or_else(|| refuse("the store holds no such repository".to_owned()))?;
        let recorded: Option<Option<i64>> = transaction
            .query_row(
                "SELECT real FROM provisional_issues WHERE repo_id = ?1 AND provisional = ?2",
                rusqlite::params![repo_id, int(provisional)],
                |row| row.get(0),
            )
            .optional()
            .map_err(&write)?;
        let (provisional_sql, real_sql) = (int(provisional), int(real));
        let exists = |number: i64| -> Result<bool, Failure> {
            transaction
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM issues WHERE repo_id = ?1 AND number = ?2)",
                    rusqlite::params![repo_id, number],
                    |row| row.get(0),
                )
                .map_err(&write)
        };
        match recorded {
            None => {
                return Err(refuse(format!("{provisional} was never allocated here")));
            }
            Some(Some(known)) if known != real_sql => {
                return Err(refuse(format!(
                    "{provisional} already became {known}, not {real}"
                )));
            }
            Some(Some(_)) => {}
            Some(None) => {
                if exists(real_sql)? {
                    return Err(refuse(format!("{real} is already an issue in the store")));
                }
                transaction
                    .execute(
                        "UPDATE provisional_issues SET real = ?3
                         WHERE repo_id = ?1 AND provisional = ?2",
                        rusqlite::params![repo_id, provisional_sql, real_sql],
                    )
                    .map_err(&write)?;
            }
        }
        if exists(provisional_sql)? {
            transaction
                .execute(
                    "INSERT INTO issues (repo_id, number, state, identity, history, imported_at,
                        synced_through)
                     SELECT repo_id, ?3, state, identity, history, imported_at, synced_through
                     FROM issues WHERE repo_id = ?1 AND number = ?2",
                    rusqlite::params![repo_id, provisional_sql, real_sql],
                )
                .map_err(&write)?;
            for table in [
                "entries",
                "tasks",
                "blocked_clauses",
                "review_jobs",
                "outbox",
            ] {
                transaction
                    .execute(
                        &format!("UPDATE {table} SET issue = ?3 WHERE repo_id = ?1 AND issue = ?2"),
                        rusqlite::params![repo_id, provisional_sql, real_sql],
                    )
                    .map_err(&write)?;
            }
            transaction
                .execute(
                    "DELETE FROM issues WHERE repo_id = ?1 AND number = ?2",
                    rusqlite::params![repo_id, provisional_sql],
                )
                .map_err(&write)?;
        }
        let needle = format!("issue={provisional}");
        let clauses: Vec<(i64, String)> = {
            let mut statement = transaction
                .prepare(
                    "SELECT rowid, body FROM blocked_clauses
                     WHERE repo_id = ?1 AND instr(body, ?2) > 0",
                )
                .map_err(&write)?;
            statement
                .query_map(rusqlite::params![repo_id, needle], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .and_then(Iterator::collect)
                .map_err(&write)?
        };
        for (rowid, body) in clauses {
            let rewritten = rename_issue_token(&body, provisional, real);
            if rewritten != body {
                transaction
                    .execute(
                        "UPDATE blocked_clauses SET body = ?2 WHERE rowid = ?1",
                        rusqlite::params![rowid, rewritten],
                    )
                    .map_err(&write)?;
            }
        }
        if let Some(seq) = sent {
            let changed = transaction
                .execute(
                    "UPDATE outbox SET state = 'sent', detail = NULL WHERE seq = ?1",
                    [int(seq)],
                )
                .map_err(&write)?;
            if changed == 0 {
                return Err(refuse(format!("queued write {seq} is not in the store")));
            }
        }
        transaction.commit().map_err(unreadable("commit a rekey"))
    }
}

fn rename_issue_token(body: &str, provisional: u64, real: u64) -> String {
    let needle = format!("issue={provisional}");
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest.find(&needle) {
        let after = &rest[at + needle.len()..];
        out.push_str(&rest[..at]);
        if after.starts_with(|c: char| c.is_ascii_digit()) {
            out.push_str(&needle);
        } else {
            out.push_str(&format!("issue={real}"));
        }
        rest = after;
    }
    out.push_str(rest);
    out
}
