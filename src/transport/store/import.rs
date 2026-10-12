use rusqlite::TransactionBehavior;

use super::{Failure, Recorded, Repo, Store, int, unreadable};
use crate::transport::{claim, commands, ownership};

/// What an import found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Imported {
    /// The issue was already held; nothing was written.
    AlreadyHeld,
    /// The issue was imported, with this many comments.
    Comments(usize),
}

fn ownership_view(holding: &mut ownership::Holding) {
    let scrub = |events: &mut Vec<ownership::Event>| {
        for event in events {
            event.comment.id = None;
        }
    };
    if let Some(event) = holding.event.as_mut() {
        event.comment.id = None;
    }
    scrub(&mut holding.live);
    scrub(&mut holding.stale);
}

fn diverged(issue: u64, detail: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "import-diverged",
        "issue": issue,
        "detail": detail,
        "action": "the store was left untouched: re-read the issue and retry; if it repeats, the \
                   timeline holds something the store cannot reproduce, so keep deciding from GitHub",
    }))
}

impl Store {
    /// Imports one issue's history from a `gh issue view --json
    /// body,labels,comments` payload, or refuses with `import-diverged` and
    /// writes nothing when the store would answer differently from GitHub.
    pub fn import(
        &mut self,
        repo: &Repo,
        issue: u64,
        github: &serde_json::Value,
        now: &str,
    ) -> Result<Imported, Failure> {
        self.import_checked(repo, issue, github, now, &Recorded::default(), |_| Ok(()))
    }

    /// [`Store::import`] for an issue this run just filed: the identity and the
    /// `blocked-until` clauses it names are recorded with the issue, in the same
    /// transaction, and read from no body.
    pub(crate) fn import_recorded(
        &mut self,
        repo: &Repo,
        issue: u64,
        github: &serde_json::Value,
        now: &str,
        recorded: &Recorded<'_>,
    ) -> Result<Imported, Failure> {
        self.import_checked(repo, issue, github, now, recorded, |_| Ok(()))
    }

    pub(super) fn import_checked(
        &mut self,
        repo: &Repo,
        issue: u64,
        github: &serde_json::Value,
        now: &str,
        recorded: &Recorded<'_>,
        before_check: impl FnOnce(&rusqlite::Transaction) -> rusqlite::Result<()>,
    ) -> Result<Imported, Failure> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unreadable("begin an import"))?;
        let read = unreadable("import an issue");
        transaction
            .execute(
                "INSERT OR IGNORE INTO repos (host, owner, name) VALUES (?1, ?2, ?3)",
                rusqlite::params![repo.host, repo.owner, repo.name],
            )
            .map_err(&read)?;
        let repo_id: i64 = transaction
            .query_row(
                "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
                rusqlite::params![repo.host, repo.owner, repo.name],
                |row| row.get(0),
            )
            .map_err(&read)?;
        let held: bool = transaction
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE repo_id = ?1 AND number = ?2)",
                rusqlite::params![repo_id, int(issue)],
                |row| row.get(0),
            )
            .map_err(&read)?;
        if held {
            return Ok(Imported::AlreadyHeld);
        }

        let states = commands::status_labels(github);
        let state = match states.as_slice() {
            [only] => only
                .split_once(':')
                .map(|(_, state)| state)
                .filter(|state| crate::config::STATES.contains(state))
                .map(ToOwned::to_owned),
            _ => None,
        };
        let identity = recorded.identity.map(ToOwned::to_owned).or_else(|| {
            github
                .get("body")
                .and_then(serde_json::Value::as_str)
                .and_then(commands::recorded_identity)
        });
        transaction
            .execute(
                "INSERT INTO issues (repo_id, number, state, identity, history, imported_at,
                    synced_through)
                 VALUES (?1, ?2, ?3, ?4, 'imported', ?5, ?6)",
                rusqlite::params![
                    repo_id,
                    int(issue),
                    state,
                    identity,
                    now,
                    claim::comments_of(github)
                        .iter()
                        .map(|comment| comment.created_at.as_str())
                        .max_by_key(|stamp| ownership::stamp_rank(stamp))
                ],
            )
            .map_err(&read)?;

        let comments = claim::comments_of(github);
        let urls: Vec<Option<i64>> = github
            .get("comments")
            .and_then(serde_json::Value::as_array)
            .map(|raw| {
                raw.iter()
                    .map(|comment| {
                        comment
                            .get("url")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|url| url.rsplit_once("issuecomment-"))
                            .and_then(|(_, id)| id.parse().ok())
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (index, comment) in comments.iter().enumerate() {
            transaction
                .execute(
                    "INSERT INTO entries (repo_id, issue, created_at, body, viewer_did_author,
                        includes_created_edit, source, source_comment_id, github_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'import', ?7, ?8)",
                    rusqlite::params![
                        repo_id,
                        int(issue),
                        comment.created_at,
                        comment.body,
                        comment.viewer_did_author,
                        comment.includes_created_edit,
                        comment.id,
                        urls.get(index).copied().flatten(),
                    ],
                )
                .map_err(&read)?;
        }
        let from_body = github
            .get("body")
            .and_then(serde_json::Value::as_str)
            .map(commands::recorded_blocked_markers)
            .unwrap_or_default();
        for body in from_body.iter().chain(recorded.clauses) {
            transaction
                .execute(
                    "INSERT INTO blocked_clauses (repo_id, issue, body, at)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![repo_id, int(issue), body, now],
                )
                .map_err(&read)?;
        }
        if let Some(review) = recorded.review {
            super::ledger::insert(&transaction, repo_id, issue, review, now)?;
        }
        before_check(&transaction).map_err(&read)?;

        let stored = super::entries(&transaction, repo_id, issue).map_err(&read)?;
        let (mut from_github, mut from_store) =
            (claim::holding(&comments, now), claim::holding(&stored, now));
        if comments.iter().any(|comment| comment.id.is_none()) {
            ownership_view(&mut from_github);
            ownership_view(&mut from_store);
        }
        if from_github != from_store {
            return Err(diverged(
                issue,
                "the holder over the stored timeline differs from the holder over GitHub's",
            ));
        }
        if !claim::same_latest_publication(&comments, &stored) {
            return Err(diverged(
                issue,
                "the latest publication over the stored timeline differs from GitHub's",
            ));
        }
        transaction
            .commit()
            .map_err(unreadable("commit an import"))?;
        Ok(Imported::Comments(comments.len()))
    }
}
