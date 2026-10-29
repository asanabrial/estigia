use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use super::jobs::Job;
use super::{Imported, Repo, Store, int, unreadable};
use crate::transport::{Context, Failure, markers, ownership};

/// One issue's slice of the store: what the claim family decides from.
///
/// Opening it is the lazy import: the first touch of an issue reads its whole
/// history from GitHub once, and every later one reads nothing from GitHub but
/// whether the issue is still open. Any failure to open it is a failed read.
pub(crate) struct Ledger {
    store: Store,
    repo: Repo,
    issue: u64,
    open: bool,
    notifications: crate::config::Notifications,
}

/// What GitHub still answers for an issue: whether it is open, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Facts {
    /// Whether the issue is open.
    pub open: bool,
    /// The repository the issue belongs to.
    pub repo: Repo,
}

fn notifications_of(context: &Context) -> crate::config::Notifications {
    let written = context.get("notifications");
    written
        .and_then(crate::config::Notifications::parse)
        .unwrap_or_default()
}

pub(crate) fn facts_of(data: &serde_json::Value, issue: u64) -> Result<Facts, Failure> {
    let unreadable = |what: &str| {
        Failure::Read(format!(
            "the REST answer for issue {issue} did not carry {what}"
        ))
    };
    let state = data
        .get("state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| unreadable("a state"))?;
    let url = data
        .get("repository_url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| unreadable("a repository_url"))?;
    let rest = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .trim_end_matches('/');
    let mut parts: Vec<&str> = rest.split('/').collect();
    let name = parts.pop().filter(|part| !part.is_empty());
    let owner = parts.pop().filter(|part| !part.is_empty());
    let host = parts.first().copied().filter(|part| !part.is_empty());
    let (Some(host), Some(owner), Some(name)) = (host, owner, name) else {
        return Err(unreadable("a repository_url naming host, owner and name"));
    };
    Ok(Facts {
        open: state.eq_ignore_ascii_case("open"),
        repo: Repo {
            host: host.strip_prefix("api.").unwrap_or(host).to_owned(),
            owner: owner.to_owned(),
            name: name.to_owned(),
        },
    })
}

fn read_facts(context: &Context, issue: u64) -> Result<Facts, Failure> {
    let endpoint = format!("repos/{{owner}}/{{repo}}/issues/{issue}");
    let data = crate::transport::gh_json(&["api", &endpoint], Some(&context.repo_dir))?
        .ok_or_else(|| Failure::Read(format!("gh api {endpoint} returned nothing")))?;
    facts_of(&data, issue)
}

#[cfg(test)]
thread_local! {
    static OPENS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// How many times this test thread has opened a ledger.
#[cfg(test)]
pub(crate) fn ledger_opens() -> u64 {
    OPENS.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static SCRATCH: tempfile::TempDir = tempfile::tempdir().expect("a scratch store directory");
}

/// Forgets everything this test thread's store held, for a test that poses two
/// different worlds for one issue: GitHub changing under the store is a
/// situation the store cannot see, and a second world is a second store.
#[cfg(test)]
pub(crate) fn forget_scratch_store() {
    SCRATCH.with(|directory| {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(directory.path().join(format!("state.db{suffix}")));
        }
    });
}

/// The bodies recorded locally for `issue` in the scratch store, oldest first.
#[cfg(test)]
pub(crate) fn local_bodies(issue: u64) -> String {
    let path = SCRATCH.with(|directory| directory.path().join("state.db"));
    let Ok(connection) = rusqlite::Connection::open(path) else {
        return String::new();
    };
    let Ok(mut statement) = connection
        .prepare("SELECT body FROM entries WHERE source = 'local' AND issue = ?1 ORDER BY seq")
    else {
        return String::new();
    };
    statement
        .query_map([i64::try_from(issue).unwrap_or(i64::MAX)], |row| {
            row.get::<_, String>(0)
        })
        .map(|rows| rows.filter_map(Result::ok).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

pub(crate) fn store_path() -> Result<PathBuf, Failure> {
    #[cfg(test)]
    {
        if let Some(named) = super::calls::named_path() {
            return Ok(named);
        }
        Ok(SCRATCH.with(|directory| directory.path().join("state.db")))
    }
    #[cfg(not(test))]
    {
        super::default_path(None)
    }
}

fn status_store_unreadable(issue: u64, detail: &str) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "review-status-store-unreadable",
        "issue": issue,
        "detail": detail,
        "action": "the local store could not be read, so nothing is known about this issue: \
                   run `estigia doctor` to see the store, and do not act on a guess",
    }))
}

fn status_issue_not_in_store(issue: u64) -> Failure {
    Failure::Stop(serde_json::json!({
        "ok": false,
        "reason": "review-status-issue-not-in-store",
        "issue": issue,
        "action": "the local store holds no record of this issue, so there is no timeline to \
                   read: claim it first, which imports its history",
    }))
}

fn lane_rerun_key(repo: &Repo, issue: u64, epoch: &str) -> String {
    format!(
        "lane-rerun {}/{}/{} {issue} {epoch}",
        repo.host, repo.owner, repo.name
    )
}

impl Ledger {
    /// The issue's timeline from the local store alone: no GitHub read, no
    /// import, no sync, and a store that does not exist is not created.
    ///
    /// An issue the store does not hold is `review-status-issue-not-in-store`,
    /// never an empty timeline; a store that cannot be read, or carries another
    /// schema, is `review-status-store-unreadable`. Both stop in the
    /// `unavailable` class.
    pub(crate) fn read_timeline_at(
        path: &Path,
        repo: &Repo,
        issue: u64,
    ) -> Result<Vec<ownership::Comment>, Failure> {
        let store = Self::read_only_store(path, repo, issue)?;
        store
            .timeline(repo, issue)
            .map_err(|failure| status_store_unreadable(issue, &failure.detail()))
    }

    /// The issue's timeline as the store holds it, the repository named by the
    /// tracker's own answer for the issue, without opening the store for writing.
    pub(crate) fn read_timeline_of(
        context: &Context,
        issue: u64,
    ) -> Result<Vec<ownership::Comment>, Failure> {
        let facts = read_facts(context, issue)?;
        Self::read_timeline_at(&store_path()?, &facts.repo, issue)
    }

    /// The jobs of the issue's newest launch of each input on `epoch`, read from the store
    /// without opening it for writing.
    pub(crate) fn read_jobs_at(
        path: &Path,
        repo: &Repo,
        issue: u64,
        epoch: &str,
    ) -> Result<Vec<Job>, Failure> {
        let store = Self::read_only_store(path, repo, issue)?;
        store
            .jobs_on(repo, issue, epoch)
            .map_err(|failure| status_store_unreadable(issue, &failure.detail()))
    }

    /// The run a re-run of failed jobs was offered for on `epoch`, if one was.
    pub(crate) fn lane_rerun_at(
        path: &Path,
        repo: &Repo,
        issue: u64,
        epoch: &str,
    ) -> Result<Option<String>, Failure> {
        let store = Self::read_only_store(path, repo, issue)?;
        store
            .connection
            .query_row(
                "SELECT value FROM store_meta WHERE key = ?1",
                [lane_rerun_key(repo, issue, epoch)],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| status_store_unreadable(issue, &error.to_string()))
    }

    /// Records that a re-run of `run_id`'s failed jobs was offered on `epoch`; the first record stands.
    pub(crate) fn record_lane_rerun_at(
        path: &Path,
        repo: &Repo,
        issue: u64,
        epoch: &str,
        run_id: &str,
    ) -> Result<(), Failure> {
        let store = Store::open(path)?;
        store
            .connection
            .execute(
                "INSERT INTO store_meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO NOTHING",
                rusqlite::params![lane_rerun_key(repo, issue, epoch), run_id],
            )
            .map(|_| ())
            .map_err(unreadable("record a lane re-run"))
    }

    fn read_only_store(path: &Path, repo: &Repo, issue: u64) -> Result<Store, Failure> {
        let unreadable = |detail: String| status_store_unreadable(issue, &detail);
        let not_held = || status_issue_not_in_store(issue);
        match path.try_exists() {
            Ok(true) => {}
            Ok(false) => return Err(not_held()),
            Err(error) => return Err(unreadable(error.to_string())),
        }
        let connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| unreadable(error.to_string()))?;
        let store = Store { connection };
        let known = u32::try_from(super::schema::MIGRATIONS.len()).unwrap_or(u32::MAX);
        let found = store
            .schema_version()
            .map_err(|failure| unreadable(failure.detail()))?;
        if found != known {
            return Err(unreadable(format!(
                "the store carries schema {found}, this binary reads {known}"
            )));
        }
        if !store
            .holds(repo, issue)
            .map_err(|failure| unreadable(failure.detail()))?
        {
            return Err(not_held());
        }
        Ok(store)
    }

    /// Opens the issue's ledger at the store's default path, importing the
    /// issue on first touch.
    pub(crate) fn open(context: &Context, issue: u64, now: &str) -> Result<Self, Failure> {
        Self::open_at(&store_path()?, context, issue, now)
    }

    pub(crate) fn open_at(
        path: &Path,
        context: &Context,
        issue: u64,
        now: &str,
    ) -> Result<Self, Failure> {
        #[cfg(test)]
        OPENS.with(|opens| opens.set(opens.get() + 1));
        if crate::transport::outbox::is_provisional(issue) {
            return Self::open_provisional(path, context, issue, now);
        }
        let facts = read_facts(context, issue)?;
        let mut store = Store::open(path)?;
        store.snapshot(
            path,
            crate::harness::session::now_seconds().unwrap_or_default(),
        );
        if !store.holds(&facts.repo, issue)? {
            let github = crate::transport::gh_json(
                &[
                    "issue",
                    "view",
                    &issue.to_string(),
                    "--json",
                    "body,labels,comments",
                ],
                Some(&context.repo_dir),
            )?
            .ok_or_else(|| Failure::Read(format!("gh issue view {issue} returned nothing")))?;
            let _: Imported = store.import(&facts.repo, issue, &github, now)?;
        }
        let ledger = Self {
            store,
            repo: facts.repo,
            issue,
            open: facts.open,
            notifications: notifications_of(context),
        };
        ledger.sync(context)?;
        Ok(ledger)
    }

    /// The ledger of an issue only this machine knows yet: GitHub is never
    /// asked, because it has no such issue, and nothing it could say would
    /// decide anything about one. Once the issue has its real number the
    /// provisional one opens that issue instead.
    fn open_provisional(
        path: &Path,
        context: &Context,
        issue: u64,
        now: &str,
    ) -> Result<Self, Failure> {
        let store = Store::open(path)?;
        store.snapshot(
            path,
            crate::harness::session::now_seconds().unwrap_or_default(),
        );
        if let Some(Some(real)) = store.provisional_real_anywhere(issue)? {
            drop(store);
            return Self::open_at(path, context, real, now);
        }
        let repo = store.provisional_repo(issue)?.ok_or_else(|| {
            Failure::Read(format!(
                "issue {issue} is a provisional number this store never allocated, and GitHub \
                 cannot know it"
            ))
        })?;
        if !store.holds(&repo, issue)? {
            return Err(Failure::Read(format!(
                "provisional issue {issue} has no local record to open"
            )));
        }
        Ok(Self {
            store,
            repo,
            issue,
            open: true,
            notifications: notifications_of(context),
        })
    }

    /// Appends the comments GitHub carries that the store does not **and that
    /// only stop a run**, so a person can still stop a run the store alone would let go on:
    /// an adjudication or a stand-down the authenticated user wrote and nobody edited.
    /// Every other comment, marker or prose, is left on GitHub and decides nothing: after the
    /// first-touch import the store alone decides who holds the issue and in what state.
    ///
    /// One REST read, `since` the newest comment already synced, a page at a time.
    /// A comment is known by its node id, which is the id GraphQL gave the import.
    ///
    /// REST has no `viewerDidAuthor`, so it is derived and never more generous
    /// than GraphQL: a comment is admitted only when its author's login is the
    /// authenticated one **and** `updated_at` equals `created_at`. GraphQL's
    /// `includesCreatedEdit` is true for fewer comments than that, so this
    /// refuses a superset of what GraphQL refuses. A missing login is not admitted.
    pub(crate) fn sync(&self, context: &Context) -> Result<(), Failure> {
        let since: Option<String> = self
            .store
            .connection()
            .query_row(
                "SELECT synced_through FROM issues WHERE number = ?4 AND repo_id =
                    (SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3)",
                rusqlite::params![
                    self.repo.host,
                    self.repo.owner,
                    self.repo.name,
                    int(self.issue)
                ],
                |row| row.get(0),
            )
            .map_err(unreadable("read the sync watermark"))?;
        let mut fetched: Vec<RestComment> = Vec::new();
        for page in 1..=COMMENT_PAGES {
            let mut endpoint = format!(
                "repos/{{owner}}/{{repo}}/issues/{}/comments?per_page={COMMENT_PAGE}&page={page}",
                self.issue
            );
            if let Some(since) = &since {
                endpoint.push_str(&format!("&since={since}"));
            }
            let answer =
                crate::transport::gh_json(&["api", &endpoint], Some(&context.repo_dir))?
                    .ok_or_else(|| Failure::Read(format!("gh api {endpoint} returned nothing")))?;
            let comments = answer.as_array().ok_or_else(|| {
                Failure::Read(format!(
                    "gh api {endpoint} did not answer a list of comments"
                ))
            })?;
            for comment in comments {
                fetched.push(rest_comment(comment, self.issue)?);
            }
            if comments.len() < COMMENT_PAGE {
                break;
            }
            if page == COMMENT_PAGES {
                return Err(Failure::Read(format!(
                    "issue {} has {} or more comments since the last sync",
                    self.issue,
                    COMMENT_PAGES * COMMENT_PAGE
                )));
            }
        }
        if fetched.is_empty() {
            return Ok(());
        }
        let known = |node: &str| -> Result<bool, Failure> {
            self.store
                .connection()
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM entries WHERE source_comment_id = ?1)
                         OR EXISTS (SELECT 1 FROM seen_comments WHERE node_id = ?1)",
                    [node],
                    |row| row.get(0),
                )
                .map_err(unreadable("look for a synced comment"))
        };
        let mut unknown = Vec::new();
        for comment in &fetched {
            if is_control_instruction(&comment.body) && !known(&comment.node_id)? {
                unknown.push(comment);
            }
        }
        let viewer = if unknown.is_empty() {
            String::new()
        } else {
            viewer_login(context)?
        };
        let transaction =
            Transaction::new_unchecked(self.store.connection(), TransactionBehavior::Immediate)
                .map_err(unreadable("begin a sync"))?;
        let repo_id = repo_id(&transaction, &self.repo)?;
        for comment in unknown {
            let seen: bool = transaction
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM entries WHERE source_comment_id = ?1)
                         OR EXISTS (SELECT 1 FROM seen_comments WHERE node_id = ?1)",
                    [&comment.node_id],
                    |row| row.get(0),
                )
                .map_err(unreadable("look for a synced comment"))?;
            if seen {
                continue;
            }
            let author = !viewer.is_empty() && comment.login == viewer;
            if !author || comment.updated_at != comment.created_at {
                continue;
            }
            insert_entry(
                &transaction,
                &Entry {
                    repo_id,
                    issue: self.issue,
                    body: &comment.body,
                    at: &comment.created_at,
                    trust: (author, comment.updated_at != comment.created_at),
                    imported_as: Some(&comment.node_id),
                    github_id: comment.github_id,
                },
            )?;
        }
        if let Some(newest) = fetched
            .iter()
            .map(|comment| comment.updated_at.as_str())
            .max_by_key(|stamp| ownership::stamp_rank(stamp))
        {
            transaction
                .execute(
                    "UPDATE issues SET synced_through = ?3 WHERE repo_id = ?1 AND number = ?2",
                    rusqlite::params![repo_id, int(self.issue), newest],
                )
                .map_err(unreadable("advance the sync watermark"))?;
        }
        transaction.commit().map_err(unreadable("commit a sync"))
    }

    /// Whether GitHub said the issue is open when this ledger was opened.
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// The issue's whole timeline, in the order it was recorded.
    pub(crate) fn timeline(&self) -> Result<Vec<ownership::Comment>, Failure> {
        self.store.timeline(&self.repo, self.issue)
    }

    /// Whether the store holds a clause that discharges this issue's `blocked`
    /// state. An unreadable answer counts as one, so that it records nothing.
    pub(crate) fn has_blocked_clause(&self) -> bool {
        self.store
            .blocked_clauses(&self.repo, self.issue)
            .map_or(true, |clauses| !clauses.is_empty())
    }

    /// Records that a person is needed for this issue, once per `key`, and
    /// answers whether this call wrote the record. The write it describes never
    /// fails on account of it, so a record that cannot be written is the error
    /// the caller reports beside its own answer.
    pub(crate) fn needs_person(
        &self,
        kind: &str,
        run_id: Option<&str>,
        key: &str,
    ) -> Result<bool, Failure> {
        let now = crate::harness::session::now_seconds()
            .ok_or_else(|| Failure::Read("this machine could not say what time it is".into()))?;
        let repo = format!("{}/{}/{}", self.repo.host, self.repo.owner, self.repo.name);
        crate::notify::record(
            &self.store,
            &super::Need {
                kind,
                repo: &repo,
                issue: Some(self.issue),
                run_id,
                dedup_key: &format!("{kind}:{}:{key}", self.issue),
            },
            &self.notifications,
            now,
        )
    }

    /// Whether this store handed the issue to `runtime` in `repo_dir`.
    pub(crate) fn handoff_explains(&self, repo_dir: &Path, runtime: &str) -> Result<bool, Failure> {
        self.store
            .handoff_explains(&repo_dir.display().to_string(), self.issue, runtime)
    }

    /// The workflow state the store holds for the issue, if it holds one.
    pub(crate) fn state(&self) -> Result<Option<String>, Failure> {
        let found: Option<Option<String>> = self
            .store
            .connection()
            .query_row(
                "SELECT state FROM issues WHERE repo_id = (SELECT id FROM repos
                    WHERE host = ?1 AND owner = ?2 AND name = ?3) AND number = ?4",
                rusqlite::params![
                    self.repo.host,
                    self.repo.owner,
                    self.repo.name,
                    int(self.issue)
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read an issue's state"))?;
        Ok(found.flatten())
    }

    /// Records the workflow state the issue moved to, and the move itself.
    ///
    /// One transaction holds both, so a history that cannot be written leaves
    /// the state where it was. Setting the state the issue already has records
    /// no move. The run recorded is whoever holds the issue at `now`, since a
    /// transition names no caller.
    pub(crate) fn set_state(&self, to: &str, now: &str) -> Result<(), Failure> {
        let transaction =
            Transaction::new_unchecked(self.store.connection(), TransactionBehavior::Immediate)
                .map_err(unreadable("begin a state change"))?;
        let repo_id = repo_id(&transaction, &self.repo)?;
        let issue = int(self.issue);
        let from: Option<String> = transaction
            .query_row(
                "SELECT state FROM issues WHERE repo_id = ?1 AND number = ?2",
                rusqlite::params![repo_id, issue],
                |row| row.get(0),
            )
            .optional()
            .map_err(unreadable("read an issue's state"))?
            .flatten();
        transaction
            .execute(
                "UPDATE issues SET state = ?3 WHERE repo_id = ?1 AND number = ?2",
                rusqlite::params![repo_id, issue, to],
            )
            .map_err(unreadable("record an issue's state"))?;
        if from.as_deref() != Some(to) {
            let holder = crate::transport::claim::holding(
                &super::entries(&transaction, repo_id, self.issue)
                    .map_err(unreadable("read a timeline"))?,
                now,
            )
            .holder;
            transaction
                .execute(
                    "INSERT INTO transitions (repo_id, issue, at, run_id, from_state, to_state)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![repo_id, issue, now, holder, from, to],
                )
                .map_err(unreadable("record a state transition"))?;
        }
        transaction
            .commit()
            .map_err(unreadable("commit a state change"))
    }

    /// Appends one entry written here and posted nowhere: the claim family's
    /// markers live in the store alone.
    pub(crate) fn append(&self, body: &str, now: &str) -> Result<(), Failure> {
        let transaction =
            Transaction::new_unchecked(self.store.connection(), TransactionBehavior::Immediate)
                .map_err(unreadable("begin an append"))?;
        let repo_id = repo_id(&transaction, &self.repo)?;
        insert(&transaction, repo_id, self.issue, body, now)?;
        transaction.commit().map_err(unreadable("commit an append"))
    }

    /// Runs `body` inside one `BEGIN IMMEDIATE` transaction over this issue.
    ///
    /// `Ok` commits and `Err` rolls back, so a refusal leaves nothing behind and
    /// a decision that must stay on the record is returned inside the `Ok`.
    pub(crate) fn transact<T>(
        &self,
        body: impl FnOnce(&Tx<'_>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let transaction =
            Transaction::new_unchecked(self.store.connection(), TransactionBehavior::Immediate)
                .map_err(unreadable("begin a transaction"))?;
        let repo_id = repo_id(&transaction, &self.repo)?;
        let tx = Tx {
            transaction: &transaction,
            repo_id,
            issue: self.issue,
        };
        let answer = body(&tx)?;
        transaction
            .commit()
            .map_err(unreadable("commit a transaction"))?;
        Ok(answer)
    }
}

/// An open transaction over one issue's entries.
pub(crate) struct Tx<'a> {
    pub(super) transaction: &'a Transaction<'a>,
    pub(super) repo_id: i64,
    pub(super) issue: u64,
}

impl Tx<'_> {
    /// The timeline as this transaction sees it, its own appends included.
    pub(crate) fn timeline(&self) -> Result<Vec<ownership::Comment>, Failure> {
        super::entries(self.transaction, self.repo_id, self.issue)
            .map_err(unreadable("read a timeline"))
    }

    /// Appends one entry, stamped no earlier than the newest one on the issue.
    pub(crate) fn append(&self, body: &str, now: &str) -> Result<(), Failure> {
        insert(self.transaction, self.repo_id, self.issue, body, now)
    }
}

fn repo_id(transaction: &Connection, repo: &Repo) -> Result<i64, Failure> {
    transaction
        .query_row(
            "SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3",
            rusqlite::params![repo.host, repo.owner, repo.name],
            |row| row.get(0),
        )
        .map_err(unreadable("find the repository"))
}

pub(super) fn insert(
    transaction: &Transaction<'_>,
    repo_id: i64,
    issue: u64,
    body: &str,
    now: &str,
) -> Result<(), Failure> {
    insert_entry(
        transaction,
        &Entry {
            repo_id,
            issue,
            body,
            at: now,
            trust: (true, false),
            imported_as: None,
            github_id: None,
        },
    )
}

struct Entry<'a> {
    repo_id: i64,
    issue: u64,
    body: &'a str,
    at: &'a str,
    trust: (bool, bool),
    imported_as: Option<&'a str>,
    github_id: Option<i64>,
}

fn insert_entry(transaction: &Transaction<'_>, entry: &Entry<'_>) -> Result<(), Failure> {
    let Entry {
        repo_id,
        issue,
        body,
        at,
        trust: (viewer_did_author, includes_created_edit),
        imported_as,
        github_id,
    } = *entry;
    let at = if imported_as.is_some() {
        at.to_owned()
    } else {
        let newest = super::entries(transaction, repo_id, issue)
            .map_err(unreadable("read the last entry"))?
            .into_iter()
            .map(|entry| entry.created_at)
            .max_by_key(|stamp| ownership::stamp_rank(stamp));
        match newest {
            Some(newest) if ownership::stamp_rank(&newest) > ownership::stamp_rank(at) => newest,
            _ => at.to_owned(),
        }
    };
    let marker = markers::parse(body).into_iter().next();
    let field = |name: &str| marker.as_ref().and_then(|found| found.get(name).cloned());
    transaction
        .execute(
            "INSERT OR IGNORE INTO entries (repo_id, issue, created_at, body, kind, run_id,
                op_id, viewer_did_author, includes_created_edit, source, source_comment_id,
                github_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                repo_id,
                int(issue),
                at,
                body,
                field("kind"),
                field("run-id"),
                field("op-id").filter(|_| imported_as.is_none()),
                viewer_did_author,
                includes_created_edit,
                if imported_as.is_some() {
                    "import"
                } else {
                    "local"
                },
                imported_as,
                github_id,
            ],
        )
        .map(|_| ())
        .map_err(unreadable("append an entry"))
}

/// One comment as the REST comments endpoint reports it.
struct RestComment {
    node_id: String,
    github_id: Option<i64>,
    login: String,
    created_at: String,
    updated_at: String,
    body: String,
}

fn rest_comment(value: &serde_json::Value, issue: u64) -> Result<RestComment, Failure> {
    let text = |found: Option<&serde_json::Value>| {
        found
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    };
    let missing = || {
        Failure::Read(format!(
            "a REST comment on issue {issue} was missing a field"
        ))
    };
    let created_at = text(value.get("created_at")).ok_or_else(missing)?;
    Ok(RestComment {
        node_id: text(value.get("node_id")).ok_or_else(missing)?,
        github_id: value.get("id").and_then(serde_json::Value::as_i64),
        login: text(value.get("user").and_then(|user| user.get("login"))).unwrap_or_default(),
        updated_at: text(value.get("updated_at")).unwrap_or_else(|| created_at.clone()),
        created_at,
        body: text(value.get("body")).unwrap_or_default(),
    })
}

thread_local! {
    static VIEWER: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The authenticated login, asked of `gh api user` once per thread.
pub(crate) fn viewer_login(context: &Context) -> Result<String, Failure> {
    if let Some(known) = VIEWER.with(|cached| cached.borrow().clone()) {
        return Ok(known);
    }
    let data =
        crate::transport::gh_json(&["api", "user"], Some(&context.repo_dir))?.unwrap_or_default();
    match data.get("login").and_then(serde_json::Value::as_str) {
        Some(login) if !login.is_empty() => {
            VIEWER.with(|cached| *cached.borrow_mut() = Some(login.to_owned()));
            Ok(login.to_owned())
        }
        _ => Err(Failure::Read("gh api user returned no login".to_owned())),
    }
}

const COMMENT_PAGE: usize = 100;
const COMMENT_PAGES: usize = 100;

impl Store {
    pub(super) fn holds(&self, repo: &Repo, issue: u64) -> Result<bool, Failure> {
        self.connection()
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE number = ?4 AND repo_id =
                    (SELECT id FROM repos WHERE host = ?1 AND owner = ?2 AND name = ?3))",
                rusqlite::params![repo.host, repo.owner, repo.name, int(issue)],
                |row| row.get(0),
            )
            .map_err(unreadable("look for an issue"))
    }
}

const ADMITTED_FROM_GITHUB: [&str; 2] = ["adjudication", "standdown"];

/// Whether a comment is nothing but a stop: every marker in it is an adjudication or a
/// stand-down, and there is at least one. Both only end a run's hold; a claim, an unassign or a
/// reclaim can hand the issue to somebody else, so a comment carrying one is refused whole.
fn is_control_instruction(body: &str) -> bool {
    let marks = crate::transport::markers::parse(body);
    !marks.is_empty()
        && marks.iter().all(|mark| {
            let kind = mark.get("kind").map(String::as_str).unwrap_or_default();
            ADMITTED_FROM_GITHUB.contains(&kind)
        })
}

/// Adds the failure of a need-for-a-person write to an answer, so a store that
/// could not record one reads as that and not as there being none.
pub(crate) fn report_unrecorded(answer: &mut serde_json::Value, recorded: Result<bool, Failure>) {
    if let (Err(failure), Some(fields)) = (recorded, answer.as_object_mut()) {
        fields.insert(
            "needs_person_unrecorded".to_owned(),
            serde_json::Value::String(failure.detail()),
        );
    }
}
