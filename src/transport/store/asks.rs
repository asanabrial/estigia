use rusqlite::OptionalExtension as _;

use super::messages::{COLUMNS, Message, message_row};
use super::{Failure, Store, int, unreadable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ask {
    pub(crate) question_id: i64,
    pub(crate) asker: String,
    pub(crate) recipient: String,
    pub(crate) choices: Vec<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) settled_at: Option<u64>,
    pub(crate) answer_id: Option<i64>,
}

#[derive(Default)]
pub(crate) struct MessageTake<'a> {
    pub(crate) recipient: &'a str,
    pub(crate) sender: Option<&'a str>,
    pub(crate) kind: Option<&'a str>,
    pub(crate) thread: Option<i64>,
    pub(crate) in_reply_to: Option<i64>,
}

impl Store {
    pub(crate) fn ask_recorded(
        &self,
        question_id: i64,
        asker: &str,
        recipient: &str,
        choices: &[String],
    ) -> Result<(), Failure> {
        let choices = serde_json::to_string(choices).unwrap_or_else(|_| "[]".to_owned());
        self.connection()
            .execute(
                "INSERT INTO asks (question_id, asker, recipient, choices) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![question_id, asker, recipient, choices],
            )
            .map_err(unreadable("record a question"))?;
        Ok(())
    }

    pub(crate) fn ask(&self, question_id: i64) -> Result<Option<Ask>, Failure> {
        self.connection()
            .query_row(
                "SELECT question_id, asker, recipient, choices, outcome, settled_at, answer_id
                 FROM asks WHERE question_id = ?1",
                rusqlite::params![question_id],
                |row| {
                    let choices: String = row.get(3)?;
                    Ok(Ask {
                        question_id: row.get(0)?,
                        asker: row.get(1)?,
                        recipient: row.get(2)?,
                        choices: serde_json::from_str(&choices).unwrap_or_default(),
                        outcome: row.get(4)?,
                        settled_at: row
                            .get::<_, Option<i64>>(5)?
                            .map(|at| u64::try_from(at).unwrap_or_default()),
                        answer_id: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(unreadable("read a question"))
    }

    pub(crate) fn ask_settled(
        &self,
        question_id: i64,
        outcome: &str,
        answer_id: Option<i64>,
        now: u64,
    ) -> Result<bool, Failure> {
        let changed = self
            .connection()
            .execute(
                "UPDATE asks SET outcome = ?2, settled_at = ?3, answer_id = ?4
                 WHERE question_id = ?1 AND outcome IS NULL",
                rusqlite::params![question_id, outcome, int(now), answer_id],
            )
            .map_err(unreadable("settle a question"))?;
        Ok(changed == 1)
    }

    pub(crate) fn message_taken_first(
        &self,
        take: &MessageTake<'_>,
        now: u64,
    ) -> Result<Option<Message>, Failure> {
        self.connection()
            .query_row(
                &format!(
                    "UPDATE messages SET delivered_at = ?1
                     WHERE id = (SELECT id FROM messages
                         WHERE recipient_run_id = ?2 AND delivered_at IS NULL
                           AND (?3 IS NULL OR sender_run_id = ?3)
                           AND (?4 IS NULL OR kind = ?4)
                           AND (?5 IS NULL OR COALESCE(thread, id) = ?5)
                           AND (?6 IS NULL OR in_reply_to = ?6)
                         ORDER BY id LIMIT 1)
                     RETURNING {COLUMNS}"
                ),
                rusqlite::params![
                    int(now),
                    take.recipient,
                    take.sender,
                    take.kind,
                    take.thread,
                    take.in_reply_to
                ],
                message_row,
            )
            .optional()
            .map_err(unreadable("take one message"))
    }
}
