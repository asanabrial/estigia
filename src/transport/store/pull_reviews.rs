use rusqlite::OptionalExtension;

use super::{Repo, Store, int, unreadable};
use crate::transport::Failure;

/// One pull request a run holds a review-only claim on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullReview {
    pub(crate) pr: u64,
    pub(crate) run_id: String,
    pub(crate) epoch: String,
    pub(crate) head: String,
    pub(crate) base_ref: String,
    pub(crate) base_tip: String,
    pub(crate) merge_base: String,
    pub(crate) worktree: String,
    pub(crate) claimed_at: u64,
    pub(crate) horizon: u64,
}

/// One reviewer a review-only claim started, and what it answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewSlot {
    pub(crate) name: String,
    pub(crate) dir: String,
    pub(crate) pid: Option<u32>,
    pub(crate) deadline_at: u64,
    pub(crate) state: String,
    pub(crate) answer: Option<String>,
}

const COLUMNS: &str =
    "pr, run_id, epoch, head, base_ref, base_tip, merge_base, worktree, claimed_at, horizon";

fn review_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<PullReview> {
    Ok(PullReview {
        pr: u64::try_from(row.get::<_, i64>(0)?).unwrap_or_default(),
        run_id: row.get(1)?,
        epoch: row.get(2)?,
        head: row.get(3)?,
        base_ref: row.get(4)?,
        base_tip: row.get(5)?,
        merge_base: row.get(6)?,
        worktree: row.get(7)?,
        claimed_at: u64::try_from(row.get::<_, i64>(8)?).unwrap_or_default(),
        horizon: u64::try_from(row.get::<_, i64>(9)?).unwrap_or_default(),
    })
}

impl Store {
    /// Adjudicates one pull request: the earliest claim whose horizon has not
    /// passed wins. Answers the row that holds it after the attempt, whoever's.
    pub(crate) fn claim_pull_review(
        &self,
        repo: &Repo,
        wanted: &PullReview,
        now: u64,
    ) -> Result<PullReview, Failure> {
        let what = "adjudicate a pull request review";
        let connection = self.connection();
        connection
            .execute(
                "INSERT OR IGNORE INTO repos (host, owner, name) VALUES (?1, ?2, ?3)",
                rusqlite::params![repo.host, repo.owner, repo.name],
            )
            .map_err(unreadable(what))?;
        connection
            .execute(
                "INSERT INTO pull_reviews (repo_id, pr, run_id, epoch, head, base_ref, base_tip,
                                           merge_base, worktree, claimed_at, horizon)
                 VALUES ((SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3),
                         ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT (repo_id, pr) DO UPDATE SET
                    run_id = excluded.run_id, epoch = excluded.epoch, head = excluded.head,
                    base_ref = excluded.base_ref, base_tip = excluded.base_tip,
                    merge_base = excluded.merge_base, worktree = excluded.worktree,
                    claimed_at = excluded.claimed_at, horizon = excluded.horizon
                 WHERE pull_reviews.horizon <= ?14",
                rusqlite::params![
                    repo.host,
                    repo.owner,
                    repo.name,
                    int(wanted.pr),
                    wanted.run_id,
                    wanted.epoch,
                    wanted.head,
                    wanted.base_ref,
                    wanted.base_tip,
                    wanted.merge_base,
                    wanted.worktree,
                    int(wanted.claimed_at),
                    int(wanted.horizon),
                    int(now),
                ],
            )
            .map_err(unreadable(what))?;
        connection
            .execute(
                "DELETE FROM pull_review_slots
                 WHERE epoch NOT IN (SELECT epoch FROM pull_reviews)",
                [],
            )
            .map_err(unreadable(what))?;
        connection
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM pull_reviews
                     WHERE repo_id = (SELECT id FROM repos
                                      WHERE host = ?1 AND owner = ?2 AND name = ?3)
                       AND pr = ?4"
                ),
                rusqlite::params![repo.host, repo.owner, repo.name, int(wanted.pr)],
                review_of,
            )
            .map_err(unreadable(what))
    }

    /// The pull request `run_id` holds a review-only claim on, if any.
    pub(crate) fn pull_review_of(&self, run_id: &str) -> Result<Option<PullReview>, Failure> {
        self.connection()
            .query_row(
                &format!("SELECT {COLUMNS} FROM pull_reviews WHERE run_id = ?1"),
                [run_id],
                review_of,
            )
            .optional()
            .map_err(unreadable("read a pull request review"))
    }

    /// Puts down the review `run_id` holds, reporting whether there was one.
    pub(crate) fn drop_pull_review(&self, run_id: &str) -> Result<bool, Failure> {
        let what = "put down a pull request review";
        let connection = self.connection();
        let removed = connection
            .execute("DELETE FROM pull_reviews WHERE run_id = ?1", [run_id])
            .map_err(unreadable(what))?;
        connection
            .execute(
                "DELETE FROM pull_review_slots
                 WHERE epoch NOT IN (SELECT epoch FROM pull_reviews)",
                [],
            )
            .map_err(unreadable(what))?;
        Ok(removed > 0)
    }

    pub(crate) fn pull_review_slots(&self, epoch: &str) -> Result<Vec<ReviewSlot>, Failure> {
        let what = "read a pull request review's reviewers";
        let mut statement = self
            .connection()
            .prepare(
                "SELECT name, dir, pid, deadline_at, state, answer FROM pull_review_slots
                 WHERE epoch = ?1 ORDER BY name",
            )
            .map_err(unreadable(what))?;
        statement
            .query_map([epoch], |row| {
                Ok(ReviewSlot {
                    name: row.get(0)?,
                    dir: row.get(1)?,
                    pid: row
                        .get::<_, Option<i64>>(2)?
                        .and_then(|pid| u32::try_from(pid).ok()),
                    deadline_at: u64::try_from(row.get::<_, i64>(3)?).unwrap_or_default(),
                    state: row.get(4)?,
                    answer: row.get(5)?,
                })
            })
            .and_then(Iterator::collect)
            .map_err(unreadable(what))
    }

    pub(crate) fn insert_pull_review_slot(
        &self,
        epoch: &str,
        name: &str,
        dir: &str,
        deadline_at: u64,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "INSERT INTO pull_review_slots (epoch, name, dir, deadline_at, state)
                 VALUES (?1, ?2, ?3, ?4, 'running')
                 ON CONFLICT (epoch, name) DO UPDATE SET
                    dir = excluded.dir, pid = NULL, deadline_at = excluded.deadline_at,
                    state = 'running', answer = NULL
                 WHERE pull_review_slots.state = 'abandoned'",
                rusqlite::params![epoch, name, dir, int(deadline_at)],
            )
            .map(|_| ())
            .map_err(unreadable("record a pull request reviewer"))
    }

    pub(crate) fn mark_pull_review_slot_spawned(
        &self,
        epoch: &str,
        name: &str,
        pid: u32,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE pull_review_slots SET pid = ?3 WHERE epoch = ?1 AND name = ?2",
                rusqlite::params![epoch, name, i64::from(pid)],
            )
            .map(|_| ())
            .map_err(unreadable("record a pull request reviewer's process"))
    }

    pub(crate) fn settle_pull_review_slot(
        &self,
        epoch: &str,
        name: &str,
        state: &str,
        answer: Option<&str>,
    ) -> Result<(), Failure> {
        self.connection()
            .execute(
                "UPDATE pull_review_slots SET state = ?3, answer = ?4
                 WHERE epoch = ?1 AND name = ?2",
                rusqlite::params![epoch, name, state, answer],
            )
            .map(|_| ())
            .map_err(unreadable("record a pull request reviewer's answer"))
    }
}
