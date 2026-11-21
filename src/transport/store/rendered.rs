use rusqlite::OptionalExtension;

use super::{Store, int, unreadable};
use crate::transport::Failure;

const KEPT_SECONDS: u64 = 90 * 24 * 60 * 60;

impl Store {
    pub(crate) fn rendered_text(&self, key: &str) -> Result<Option<String>, Failure> {
        self.connection()
            .query_row(
                "SELECT text FROM rendered_texts WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read a rendered text"))
    }

    /// Keeps the first text a key was rendered to, so a repeat of the same
    /// request is answered with the words already used.
    pub(crate) fn keep_rendered_text(
        &self,
        key: &str,
        text: &str,
        now: u64,
    ) -> Result<(), Failure> {
        let what = "keep a rendered text";
        let connection = self.connection();
        connection
            .execute(
                "INSERT INTO rendered_texts (key, text, at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO NOTHING",
                rusqlite::params![key, text, int(now)],
            )
            .map_err(unreadable(what))?;
        connection
            .execute(
                "DELETE FROM rendered_texts WHERE at < ?1",
                [int(now.saturating_sub(KEPT_SECONDS))],
            )
            .map(|_| ())
            .map_err(unreadable(what))
    }
}
