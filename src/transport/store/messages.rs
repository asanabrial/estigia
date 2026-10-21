use std::collections::BTreeMap;

use rusqlite::OptionalExtension as _;

use super::{Store, int, unreadable};
use crate::transport::Failure;

pub(crate) struct MessageSent<'a> {
    pub(crate) sender_run_id: Option<&'a str>,
    pub(crate) recipient_run_id: &'a str,
    pub(crate) body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) id: i64,
    pub(crate) sender_run_id: Option<String>,
    pub(crate) body: String,
    pub(crate) created_at: u64,
}

fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        sender_run_id: row.get(1)?,
        body: row.get(2)?,
        created_at: seconds(row.get(3)?),
    })
}

fn counted(store: &Store, sql: &str, what: &'static str) -> Result<BTreeMap<String, u64>, Failure> {
    let mut statement = store.connection().prepare(sql).map_err(unreadable(what))?;
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, seconds(row.get(1)?)))
        })
        .map_err(unreadable(what))?
        .collect::<Result<_, _>>()
        .map_err(unreadable(what))
}

impl Store {
    pub(crate) fn message_sent(&self, sent: &MessageSent<'_>, now: u64) -> Result<i64, Failure> {
        self.connection()
            .execute(
                "INSERT INTO messages (sender_run_id, recipient_run_id, body, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    sent.sender_run_id,
                    sent.recipient_run_id,
                    sent.body,
                    int(now)
                ],
            )
            .map_err(unreadable("record a message"))?;
        Ok(self.connection().last_insert_rowid())
    }

    #[cfg(test)]
    pub(crate) fn messages_taken(
        &self,
        recipient_run_id: &str,
        now: u64,
    ) -> Result<Vec<Message>, Failure> {
        self.messages_taken_through(recipient_run_id, now, i64::MAX)
    }

    pub(crate) fn messages_taken_through(
        &self,
        recipient_run_id: &str,
        now: u64,
        through: i64,
    ) -> Result<Vec<Message>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "UPDATE messages SET delivered_at = ?2
                 WHERE recipient_run_id = ?1 AND delivered_at IS NULL AND id <= ?3
                 RETURNING id, sender_run_id, body, created_at",
            )
            .map_err(unreadable("take the pending messages"))?;
        let mut taken = statement
            .query_map(
                rusqlite::params![recipient_run_id, int(now), through],
                message_row,
            )
            .map_err(unreadable("take the pending messages"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("take the pending messages"))?;
        taken.sort_by_key(|message| message.id);
        Ok(taken)
    }

    pub(crate) fn message(&self, id: i64) -> Result<Option<Message>, Failure> {
        self.connection()
            .query_row(
                "SELECT id, sender_run_id, body, created_at FROM messages WHERE id = ?1",
                rusqlite::params![id],
                message_row,
            )
            .optional()
            .map_err(unreadable("read one message"))
    }

    pub(crate) fn messages_pending(&self, recipient_run_id: &str) -> Result<Vec<Message>, Failure> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT id, sender_run_id, body, created_at FROM messages
                 WHERE recipient_run_id = ?1 AND delivered_at IS NULL ORDER BY id",
            )
            .map_err(unreadable("read the pending messages"))?;
        statement
            .query_map(rusqlite::params![recipient_run_id], message_row)
            .map_err(unreadable("read the pending messages"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("read the pending messages"))
    }

    pub(crate) fn messages_waiting_for(&self) -> Result<BTreeMap<String, u64>, Failure> {
        counted(
            self,
            "SELECT recipient_run_id, COUNT(*) FROM messages WHERE delivered_at IS NULL
             GROUP BY recipient_run_id",
            "count the waiting messages",
        )
    }

    pub(crate) fn messages_undelivered_from(&self) -> Result<BTreeMap<String, u64>, Failure> {
        counted(
            self,
            "SELECT sender_run_id, COUNT(*) FROM messages
             WHERE delivered_at IS NULL AND sender_run_id IS NOT NULL GROUP BY sender_run_id",
            "count the undelivered messages",
        )
    }
}
