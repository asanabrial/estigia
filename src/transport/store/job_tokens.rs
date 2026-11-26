use rusqlite::OptionalExtension;

use super::{Failure, Store, int, unreadable};

impl Store {
    /// Mints the token one job's runner writes into its result and records it
    /// under the job's id, answering the token to hand to that runner.
    ///
    /// The value comes from SQLite's own `randomblob`, so every platform reads
    /// the same source and no dependency is added for it. A relaunch of the
    /// same id — a check job whose runner died, whose directory is written
    /// again — replaces the token rather than leaving the dead run's in place.
    pub(crate) fn mint_job_token(&self, id: &str, at: u64) -> Result<String, Failure> {
        self.connection()
            .query_row(
                "INSERT INTO job_tokens (id, token, at) VALUES (?1, lower(hex(randomblob(32))), ?2)
                 ON CONFLICT (id) DO UPDATE SET token = excluded.token, at = excluded.at
                 RETURNING token",
                rusqlite::params![id, int(at)],
                |row| row.get(0),
            )
            .map_err(unreadable("mint a job token"))
    }

    /// The token minted for `id`, or `None` when no launch of this binary
    /// recorded one — a job another binary launched, or one it did not launch.
    pub(crate) fn job_token(&self, id: &str) -> Result<Option<String>, Failure> {
        self.connection()
            .query_row("SELECT token FROM job_tokens WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(unreadable("read a job token"))
    }

    /// Forgets the tokens of jobs older than `before`, which the directory
    /// bound has already removed or will remove on the same schedule.
    pub(crate) fn bound_job_tokens(&self, before: u64) -> Result<(), Failure> {
        self.connection()
            .execute("DELETE FROM job_tokens WHERE at < ?1", [int(before)])
            .map(|_| ())
            .map_err(unreadable("bound the job tokens"))
    }
}
