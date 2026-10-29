use std::collections::BTreeMap;

use rusqlite::OptionalExtension as _;

use super::{Store, int, unreadable};
use crate::transport::Failure;

pub(crate) struct MessageSent<'a> {
    pub(crate) sender_run_id: Option<&'a str>,
    pub(crate) recipient_run_id: &'a str,
    pub(crate) body: &'a str,
}

#[derive(Default)]
pub(crate) struct MessageShape<'a> {
    pub(crate) kind: Option<&'a str>,
    pub(crate) thread: Option<i64>,
    pub(crate) in_reply_to: Option<i64>,
    pub(crate) expires_at: Option<u64>,
}

#[derive(Default)]
pub(crate) struct MessageListing<'a> {
    pub(crate) recipient: Option<&'a str>,
    pub(crate) sender: Option<&'a str>,
    pub(crate) kind: Option<&'a str>,
    pub(crate) thread: Option<i64>,
    pub(crate) state: Option<&'a str>,
    pub(crate) limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) id: i64,
    pub(crate) sender_run_id: Option<String>,
    pub(crate) body: String,
    pub(crate) created_at: u64,
    pub(crate) recipient_run_id: String,
    pub(crate) kind: String,
    pub(crate) thread: i64,
    pub(crate) in_reply_to: Option<i64>,
    pub(crate) delivered_at: Option<u64>,
    pub(crate) read_at: Option<u64>,
    pub(crate) acked_at: Option<u64>,
    pub(crate) expires_at: Option<u64>,
}

impl Message {
    pub(crate) fn state(&self) -> &'static str {
        if self.acked_at.is_some() {
            "acked"
        } else if self.read_at.is_some() {
            "read"
        } else if self.delivered_at.is_some() {
            "delivered"
        } else {
            "queued"
        }
    }
}

pub(super) const COLUMNS: &str = "id, sender_run_id, body, created_at, recipient_run_id, kind,
    COALESCE(thread, id), in_reply_to, delivered_at, read_at, acked_at, expires_at";

const STATE: &str =
    "CASE WHEN acked_at IS NOT NULL THEN 'acked' WHEN read_at IS NOT NULL THEN 'read'
    WHEN delivered_at IS NOT NULL THEN 'delivered' ELSE 'queued' END";

fn optional_seconds(value: Option<i64>) -> Option<u64> {
    value.map(seconds)
}

fn seconds(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

pub(super) fn message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        sender_run_id: row.get(1)?,
        body: row.get(2)?,
        created_at: seconds(row.get(3)?),
        recipient_run_id: row.get(4)?,
        kind: row.get(5)?,
        thread: row.get(6)?,
        in_reply_to: row.get(7)?,
        delivered_at: optional_seconds(row.get(8)?),
        read_at: optional_seconds(row.get(9)?),
        acked_at: optional_seconds(row.get(10)?),
        expires_at: optional_seconds(row.get(11)?),
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
    #[cfg(test)]
    pub(crate) fn message_sent(&self, sent: &MessageSent<'_>, now: u64) -> Result<i64, Failure> {
        self.message_sent_shaped(sent, &MessageShape::default(), now)
            .map(|(id, _)| id)
    }

    pub(crate) fn message_sent_shaped(
        &self,
        sent: &MessageSent<'_>,
        shape: &MessageShape<'_>,
        now: u64,
    ) -> Result<(i64, i64), Failure> {
        let inherited = match (shape.thread, shape.in_reply_to) {
            (Some(thread), _) => Some(thread),
            (None, Some(parent)) => self.message(parent)?.map(|parent| parent.thread),
            (None, None) => None,
        };
        self.connection()
            .execute(
                "INSERT INTO messages
                     (sender_run_id, recipient_run_id, body, created_at, kind, thread,
                      in_reply_to, expires_at)
                 VALUES (?1, ?2, ?3, ?4, COALESCE(?5, 'note'), ?6, ?7, ?8)",
                rusqlite::params![
                    sent.sender_run_id,
                    sent.recipient_run_id,
                    sent.body,
                    int(now),
                    shape.kind,
                    inherited,
                    shape.in_reply_to,
                    shape.expires_at.map(int),
                ],
            )
            .map_err(unreadable("record a message"))?;
        let id = self.connection().last_insert_rowid();
        Ok((id, inherited.unwrap_or(id)))
    }

    pub(crate) fn messages_listed(
        &self,
        listing: &MessageListing<'_>,
    ) -> Result<Vec<Message>, Failure> {
        let sql = format!(
            "SELECT {COLUMNS} FROM messages
             WHERE (?1 IS NULL OR recipient_run_id = ?1)
               AND (?2 IS NULL OR sender_run_id = ?2)
               AND (?3 IS NULL OR kind = ?3)
               AND (?4 IS NULL OR COALESCE(thread, id) = ?4)
               AND (?5 IS NULL OR {STATE} = ?5)
             ORDER BY id DESC LIMIT ?6"
        );
        let mut statement = self
            .connection()
            .prepare(&sql)
            .map_err(unreadable("list messages"))?;
        let mut listed = statement
            .query_map(
                rusqlite::params![
                    listing.recipient,
                    listing.sender,
                    listing.kind,
                    listing.thread,
                    listing.state,
                    int(listing.limit),
                ],
                message_row,
            )
            .map_err(unreadable("list messages"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unreadable("list messages"))?;
        listed.reverse();
        Ok(listed)
    }

    pub(crate) fn messages_marked(
        &self,
        recipient_run_id: &str,
        ids: &[i64],
        acked: bool,
        now: u64,
    ) -> Result<(), Failure> {
        let sql = if acked {
            "UPDATE messages SET acked_at = COALESCE(acked_at, ?3),
                 read_at = COALESCE(read_at, ?3), delivered_at = COALESCE(delivered_at, ?3)
             WHERE id = ?1 AND recipient_run_id = ?2"
        } else {
            "UPDATE messages SET read_at = COALESCE(read_at, ?3)
             WHERE id = ?1 AND recipient_run_id = ?2"
        };
        for id in ids {
            self.connection()
                .execute(sql, rusqlite::params![id, recipient_run_id, int(now)])
                .map_err(unreadable("mark a message"))?;
        }
        Ok(())
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
            .prepare(&format!(
                "UPDATE messages SET delivered_at = ?2
                 WHERE recipient_run_id = ?1 AND delivered_at IS NULL AND id <= ?3
                 RETURNING {COLUMNS}"
            ))
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
                &format!("SELECT {COLUMNS} FROM messages WHERE id = ?1"),
                rusqlite::params![id],
                message_row,
            )
            .optional()
            .map_err(unreadable("read one message"))
    }

    pub(crate) fn messages_pending(&self, recipient_run_id: &str) -> Result<Vec<Message>, Failure> {
        let mut statement = self
            .connection()
            .prepare(&format!(
                "SELECT {COLUMNS} FROM messages
                 WHERE recipient_run_id = ?1 AND delivered_at IS NULL ORDER BY id"
            ))
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
