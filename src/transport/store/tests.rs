use super::calls::{self, Kind};
use super::{Imported, Repo, Store};
use crate::transport::{Failure, claim, ownership};

const RUN: &str = "claude-aaaa";
const OP: &str = "0123456789abcdef0123456789abcdef";
const NOW: &str = "2026-01-01T00:10Z";

fn repo() -> Repo {
    Repo {
        host: "github.com".to_owned(),
        owner: "asanabrial".to_owned(),
        name: "estigia".to_owned(),
    }
}

fn open() -> (tempfile::TempDir, Store) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let store = Store::open(&directory.path().join("state.db")).expect("the store opens");
    (directory, store)
}

fn comment(id: &str, at: &str, body: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "createdAt": at,
        "viewerDidAuthor": true,
        "includesCreatedEdit": false,
        "body": body,
    })
}

fn claim_comment() -> serde_json::Value {
    comment(
        "IC_1",
        "2026-01-01T00:00Z",
        &format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={RUN} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={OP} -->\n"
        ),
    )
}

fn unassign_comment() -> serde_json::Value {
    comment(
        "IC_2",
        "2026-01-01T00:05Z",
        &format!(
            "Released.\n\n<!-- issue-flow: unassign run-id={RUN} runtime=claude \
             op-id=fedcba9876543210fedcba9876543210 target-op={OP} -->\n"
        ),
    )
}

fn issue(comments: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "body": "Body.\n\n<!-- issue-flow: analysis identity=thing-1 -->\n",
        "labels": [{ "name": "status:in-progress" }, { "name": "priority:high" }],
        "comments": comments,
    })
}

fn holder_of(comments: &[ownership::Comment]) -> Option<String> {
    claim::holding(comments, NOW).holder
}

#[test]
fn an_imported_live_claim_is_held_by_the_same_run_over_the_store() {
    let (_directory, mut store) = open();
    let github = issue(vec![claim_comment()]);

    let imported = store.import(&repo(), 7, &github, NOW).expect("imports");

    assert_eq!(imported, Imported::Comments(1));
    let stored = store.timeline(&repo(), 7).expect("the timeline reads");
    assert_eq!(
        holder_of(&stored),
        Some(RUN.to_owned()),
        "the stored timeline must hold the run GitHub's does"
    );
    assert_eq!(
        holder_of(&claim::comments_of(&github)),
        Some(RUN.to_owned())
    );
}

#[test]
fn an_imported_claim_and_its_unassign_leave_nobody_holding_over_both_sources() {
    let (_directory, mut store) = open();
    let github = issue(vec![claim_comment(), unassign_comment()]);
    assert_eq!(
        holder_of(&claim::comments_of(&github)),
        None,
        "the fixture must release the claim, or this proves nothing"
    );

    store.import(&repo(), 7, &github, NOW).expect("imports");

    assert_eq!(holder_of(&store.timeline(&repo(), 7).expect("reads")), None);
}

#[test]
fn the_timeline_is_in_insertion_order_with_the_trust_flags_the_tracker_reported() {
    let (_directory, mut store) = open();
    let mut edited = comment("IC_2", "2026-01-01T00:01Z", "edited later");
    edited["includesCreatedEdit"] = serde_json::Value::Bool(true);
    let mut foreign = comment("IC_3", "2026-01-01T00:02Z", "somebody else");
    foreign["viewerDidAuthor"] = serde_json::Value::Bool(false);
    store
        .import(
            &repo(),
            7,
            &issue(vec![claim_comment(), edited, foreign]),
            NOW,
        )
        .expect("imports");

    let stored = store.timeline(&repo(), 7).expect("reads");

    let ids: Vec<_> = stored
        .iter()
        .map(|c| c.id.clone().unwrap_or_default())
        .collect();
    assert_eq!(ids, ["IC_1", "IC_2", "IC_3"]);
    assert!(stored[0].viewer_did_author && !stored[0].includes_created_edit);
    assert!(
        stored[1].includes_created_edit,
        "an edited comment stays untrusted"
    );
    assert!(
        !stored[2].viewer_did_author,
        "a foreign comment stays untrusted"
    );
}

#[test]
fn an_import_records_the_state_and_identity_and_marks_the_history_imported() {
    let (_directory, mut store) = open();
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");

    let row: (Option<String>, Option<String>, String) = store
        .connection()
        .query_row(
            "SELECT state, identity, history FROM issues WHERE number = 7",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("the issue row exists");

    assert_eq!(
        row,
        (
            Some("in-progress".to_owned()),
            Some("thing-1".to_owned()),
            "imported".to_owned()
        )
    );
}

#[test]
fn two_status_labels_import_as_no_state_rather_than_a_guess() {
    let (_directory, mut store) = open();
    let mut github = issue(vec![]);
    github["labels"] = serde_json::json!([{ "name": "status:ready" }, { "name": "status:done" }]);
    store.import(&repo(), 7, &github, NOW).expect("imports");

    let state: Option<String> = store
        .connection()
        .query_row("SELECT state FROM issues WHERE number = 7", [], |row| {
            row.get(0)
        })
        .expect("the row exists");

    assert_eq!(state, None);
}

#[test]
fn a_divergent_import_refuses_as_import_diverged_and_writes_nothing() {
    let (_directory, mut store) = open();
    let github = issue(vec![claim_comment()]);

    let forged = |transaction: &rusqlite::Transaction| {
        transaction
            .execute(
                "INSERT INTO entries (repo_id, issue, created_at, body, viewer_did_author,
                    includes_created_edit, source)
                 VALUES (1, 7, '2026-01-01T00:00Z', ?1, 1, 0, 'local')",
                [format!(
                    "<!-- issue-flow: claim run-id=other runtime=claude \
                     horizon=2099-01-01T00:00Z op-id={} -->",
                    "b".repeat(32)
                )],
            )
            .map(|_| ())
    };
    let refused = store
        .import_checked(
            &repo(),
            7,
            &github,
            NOW,
            &super::Recorded::default(),
            forged,
        )
        .expect_err("a store that holds more than GitHub does must not import");

    let Failure::Stop(payload) = &refused else {
        panic!("the refusal is an answer, not a read failure: {refused:?}");
    };
    assert_eq!(payload["reason"], "import-diverged");
    for table in ["repos", "issues", "entries"] {
        let rows: i64 = store
            .connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("counts");
        assert_eq!(rows, 0, "{table} kept a row from a refused import");
    }
}

#[test]
fn an_issue_already_held_is_not_imported_twice() {
    let (_directory, mut store) = open();
    let github = issue(vec![claim_comment()]);
    store.import(&repo(), 7, &github, NOW).expect("imports");

    let again = store.import(&repo(), 7, &github, NOW).expect("answers");

    assert_eq!(again, Imported::AlreadyHeld);
    assert_eq!(store.timeline(&repo(), 7).expect("reads").len(), 1);
}

#[test]
fn entries_refuse_update_and_delete() {
    let (_directory, mut store) = open();
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");

    for statement in ["UPDATE entries SET body = 'x'", "DELETE FROM entries"] {
        let error = store
            .connection()
            .execute(statement, [])
            .expect_err("an append-only table accepted a rewrite");
        assert!(
            error.to_string().contains("append-only"),
            "{statement}: {error}"
        );
    }
}

#[test]
fn the_store_opens_in_wal_with_foreign_keys_and_a_recorded_schema() {
    let (_directory, store) = crate::paths::with_real_fsync(open);
    let connection = store.connection();

    let mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("reads");
    let keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("reads");
    let synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("reads");

    assert_eq!(mode, "wal");
    assert_eq!(keys, 1);
    assert_eq!(synchronous, 2, "FULL");
    assert_eq!(store.schema_version().expect("reads"), 17);
}

#[cfg(debug_assertions)]
#[test]
fn a_test_build_that_skips_fsync_opens_the_store_without_syncing() {
    let (_directory, store) = open();
    let synchronous: i64 = store
        .connection()
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("reads");
    assert_eq!(synchronous, 0, "OFF");
}

#[test]
fn reopening_keeps_the_schema_and_a_newer_one_is_refused() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    drop(Store::open(&path).expect("opens"));
    assert_eq!(
        Store::open(&path)
            .expect("reopens")
            .schema_version()
            .expect("reads"),
        17
    );

    rusqlite::Connection::open(&path)
        .expect("opens raw")
        .execute("UPDATE meta SET schema_version = 99", [])
        .expect("writes");

    assert!(matches!(Store::open(&path), Err(Failure::Read(_))));
}

#[test]
fn a_store_that_cannot_open_is_a_failed_read_never_clearance() {
    let directory = tempfile::tempdir().expect("a temporary directory");

    let failure = Store::open(directory.path())
        .err()
        .expect("a directory is not a database");

    assert!(matches!(failure, Failure::Read(_)), "{failure:?}");
}

#[test]
fn a_foreign_key_refuses_an_entry_for_an_issue_nobody_imported() {
    let (_directory, store) = open();
    store
        .connection()
        .execute(
            "INSERT INTO repos (host, owner, name) VALUES ('h', 'o', 'n')",
            [],
        )
        .expect("a repo");

    let refused = store.connection().execute(
        "INSERT INTO entries (repo_id, issue, created_at, body, viewer_did_author,
            includes_created_edit, source) VALUES (1, 9, 't', 'b', 1, 0, 'local')",
        [],
    );

    assert!(refused.is_err());
}

#[test]
fn gh_spawns_are_classified_graphql_or_rest() {
    let kind = |args: &[&str]| calls::classify(args);

    assert_eq!(kind(&["gh", "issue", "view", "1"]), Some(Kind::Graphql));
    assert_eq!(kind(&["gh", "pr", "view", "1"]), Some(Kind::Graphql));
    assert_eq!(
        kind(&["gh", "api", "graphql", "-f", "query=x"]),
        Some(Kind::Graphql)
    );
    assert_eq!(
        kind(&["gh", "api", "-X", "POST", "graphql", "-f", "query=x"]),
        Some(Kind::Graphql)
    );
    assert_eq!(kind(&["gh", "api", "repos/o/r/issues/1"]), Some(Kind::Rest));
    assert_eq!(
        kind(&[
            "gh",
            "api",
            "-f",
            "body=graphql",
            "repos/o/r/issues/1/comments"
        ]),
        Some(Kind::Rest),
        "a field value is not the endpoint"
    );
    assert_eq!(kind(&["gh", "label", "create", "x"]), Some(Kind::Other));
    assert_eq!(kind(&["git", "status"]), None);
}

#[test]
fn a_counted_operation_records_its_graphql_and_rest_spawns_in_the_store() {
    let (directory, store) = open();
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();

    calls::count_into(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&[]), || {
            calls::counting("verify-claim", || {
                for args in [
                    &["gh", "issue", "view", "1"][..],
                    &["gh", "pr", "view", "2"],
                    &["gh", "api", "repos/o/r/issues/1"],
                    &["git", "--version"],
                ] {
                    let _ = crate::transport::run(args, None, crate::transport::How::tolerated());
                }
            });
        });
    });

    let summary = store.calls_since(0).expect("reads");
    assert_eq!((summary.graphql, summary.rest, summary.other), (2, 1, 0));
    let operation: String = store
        .connection()
        .query_row("SELECT DISTINCT operation FROM calls", [], |row| row.get(0))
        .expect("one operation");
    assert_eq!(operation, "verify-claim");
}

#[test]
fn a_gated_write_on_an_open_held_issue_makes_no_graphql_call_and_two_rest_calls() {
    let (directory, store) = open();
    let path = directory.path().join("state.db");
    let bin = crate::test_env::scripted_gh();
    let script =
        crate::test_env::answers(&[crate::test_env::live_holder_issue(RUN, 7, "in-progress")]);
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let flags: Vec<String> = [
        "--issue",
        "7",
        "--run-id",
        RUN,
        "--expect-state",
        "in-progress",
    ]
    .iter()
    .map(|flag| (*flag).to_owned())
    .collect();
    let verify = || {
        calls::count_into(&path, || {
            crate::test_env::with_scripted_gh(bin.path(), &script, || {
                crate::transport::dispatch::dispatch(&context, "verify-claim", &flags, NOW)
            })
            .expect("the claim verifies");
        });
    };

    verify();
    let first = store.calls_since(0).expect("reads");
    assert_eq!(
        (first.graphql, first.rest, first.other),
        (1, 2, 0),
        "the first touch imports the issue: one `gh issue view`, the REST open check and the \
         comments sync"
    );

    verify();
    let both = store.calls_since(0).expect("reads");
    assert_eq!(
        (
            both.graphql - first.graphql,
            both.rest - first.rest,
            both.other - first.other
        ),
        (0, 2, 0),
        "a gated write on an open, held issue reads GitHub twice over REST (whether the issue is \
         open, and the comments since the last sync, of which only a control instruction is \
         admitted) and never over GraphQL"
    );
}

#[test]
fn an_uncountable_store_never_fails_the_operation() {
    let directory = tempfile::tempdir().expect("a temporary directory");

    let answer = calls::count_into(directory.path(), || {
        calls::counting("anything", || {
            calls::note(&["gh", "issue", "view", "1"]);
            "answered"
        })
    });

    assert_eq!(answer, "answered");
}

#[test]
fn doctor_reports_an_absent_store_without_creating_it() {
    let home = tempfile::tempdir().expect("a temporary home");

    let check = crate::harness::doctor::store(Some(home.path()));

    assert_eq!(check.name, "store");
    assert!(matches!(
        check.health,
        crate::harness::doctor::Health::Skipped { .. }
    ));
    assert!(
        !home.path().join(".estigia").exists(),
        "doctor created the store"
    );
}

#[test]
fn doctor_reports_the_path_the_schema_and_the_last_hours_calls() {
    let home = tempfile::tempdir().expect("a temporary home");
    let path = super::default_path(Some(home.path())).expect("a path");
    let store = Store::open(&path).expect("opens");
    store
        .connection()
        .execute(
            "INSERT INTO calls (operation, kind, count, at) VALUES ('claim', 'graphql', 3, ?1)",
            [super::int(
                crate::harness::session::now_seconds().unwrap_or_default(),
            )],
        )
        .expect("a call row");
    drop(store);

    let check = crate::harness::doctor::store(Some(home.path()));

    let crate::harness::doctor::Health::Fine { detail } = check.health else {
        panic!("a readable store is fine: {:?}", check.health);
    };
    assert!(
        detail.contains("state.db") && detail.contains("schema 17"),
        "{detail}"
    );
    assert!(detail.contains("3 GraphQL, 0 REST"), "{detail}");
}

#[test]
fn doctor_reports_a_record_that_could_not_be_written_down() {
    let home = tempfile::tempdir().expect("a temporary home");
    let state = crate::harness::session::state_root(Some(home.path())).expect("a state root");
    std::fs::create_dir_all(&state).expect("the state directory");
    let path = crate::transport::store::beside(&state);
    Store::open(&path).expect("the store opens");
    // The store opens, but its decision table is gone, so the next append
    // fails the way a write failure would: the row is not on the record.
    rusqlite::Connection::open(&path)
        .expect("a raw connection")
        .execute_batch("DROP TABLE decisions")
        .expect("drop the decision table");

    crate::harness::session::record(&state, &serde_json::json!({ "verdict": "allow" }));

    let check = crate::harness::doctor::store(Some(home.path()));
    let crate::harness::doctor::Health::Broken { detail, .. } = check.health else {
        panic!("a record that could not be written down was reported fine");
    };
    assert!(
        detail.contains("could not be written down") && detail.contains('1'),
        "{detail}"
    );
}

#[test]
fn a_snapshot_is_due_only_once_the_newest_is_a_day_old() {
    assert!(
        super::snapshot::snapshot_due(None),
        "a store with no snapshot is always due one"
    );
    for age in [0, 1, super::snapshot::SNAPSHOT_INTERVAL - 1] {
        assert!(
            !super::snapshot::snapshot_due(Some(age)),
            "a snapshot {age}s old was read as due"
        );
    }
    for age in [
        super::snapshot::SNAPSHOT_INTERVAL,
        super::snapshot::SNAPSHOT_INTERVAL + 1,
    ] {
        assert!(
            super::snapshot::snapshot_due(Some(age)),
            "a snapshot {age}s old was not read as due"
        );
    }
}

#[test]
fn a_due_snapshot_is_written_beside_the_store_and_the_oldest_are_pruned() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let store = Store::open(&path).expect("opens");

    for now in [1_000_000u64, 2_000_000, 3_000_000, 4_000_000] {
        store.snapshot(&path, now);
    }

    let mut names: Vec<String> = std::fs::read_dir(super::snapshot::snapshots_dir(&path))
        .expect("the snapshots directory exists")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["2000000", "3000000", "4000000"],
        "retention did not keep the newest three snapshots"
    );
}

#[test]
fn a_snapshot_that_cannot_be_written_is_counted_and_does_not_fail_the_operation() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let store = Store::open(&path).expect("opens");
    std::fs::write(directory.path().join("snapshots"), "not a directory")
        .expect("a file where the snapshots directory goes");

    store.snapshot(&path, 1_000_000);

    assert_eq!(
        store.record_failures().expect("reads"),
        [("snapshot".to_owned(), 1)],
        "a snapshot that could not be written was not counted"
    );
}

#[test]
fn opening_a_ledger_writes_a_snapshot_when_none_is_recent() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        7,
        serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
    )]);

    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::Ledger::open_at(&path, &context, 7, NOW).expect("the ledger opens");
    });

    let snapshots: Vec<_> = std::fs::read_dir(super::snapshot::snapshots_dir(&path))
        .expect("the write path made a snapshots directory")
        .flatten()
        .collect();
    assert_eq!(
        snapshots.len(),
        1,
        "the write path did not snapshot the store it opened"
    );
}

#[test]
fn the_filesystem_classifier_tells_a_network_mount_from_a_local_one() {
    for kind in ["nfs", "nfs4", "smbfs", "cifs", "afpfs", "webdav"] {
        assert!(
            super::filesystem::is_network_filesystem(kind),
            "{kind} was read as a local filesystem"
        );
    }
    for kind in ["apfs", "hfs", "ext4", "xfs", "tmpfs", "overlay", ""] {
        assert!(
            !super::filesystem::is_network_filesystem(kind),
            "{kind} was read as a network mount"
        );
    }
    assert!(
        super::filesystem::is_network_filesystem("NFS"),
        "the classifier is case-sensitive"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_linux_magic_numbers_name_the_network_kinds() {
    assert_eq!(super::filesystem::kind_of(0x6969), Some("nfs"));
    assert_eq!(super::filesystem::kind_of(0xFF534D42), Some("cifs"));
    assert_eq!(
        super::filesystem::kind_of(0xEF53),
        None,
        "ext4 is not network"
    );
}

#[test]
fn a_store_on_a_local_filesystem_is_not_refused() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    if let Some(kind) = super::filesystem::filesystem_kind(directory.path()) {
        assert!(
            !super::filesystem::is_network_filesystem(&kind),
            "a local temporary directory reported the {kind} filesystem"
        );
    }
    let path = directory.path().join("state.db");
    Store::open(&path).expect("a store on a local filesystem opens");
}

#[test]
fn doctor_reports_the_age_of_the_newest_snapshot() {
    let home = tempfile::tempdir().expect("a temporary home");
    let path = super::default_path(Some(home.path())).expect("a path");
    Store::open(&path).expect("opens");
    let now = crate::harness::session::now_seconds().unwrap_or_default();
    let snapshots = super::snapshot::snapshots_dir(&path);
    std::fs::create_dir_all(&snapshots).expect("the snapshots directory");
    std::fs::write(
        snapshots.join(now.saturating_sub(3600).to_string()),
        "snapshot",
    )
    .expect("a snapshot");

    let check = crate::harness::doctor::store(Some(home.path()));

    let crate::harness::doctor::Health::Fine { detail } = check.health else {
        panic!("a store with a snapshot is fine: {:?}", check.health);
    };
    assert!(
        detail.contains("last snapshot 1h ago"),
        "the store row does not name the snapshot's age: {detail}"
    );
}

#[test]
fn a_migration_another_opener_finished_while_this_one_waited_is_not_applied_twice() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let slow = Store::connect(&path, std::time::Duration::from_secs(5)).expect("connects");

    slow.migrate_after_read(|| {
        drop(Store::open(&path).expect("the faster opener migrates"));
    })
    .expect("the slower opener finds the schema already laid down");

    assert_eq!(slow.schema_version().expect("reads"), 17);
}

#[test]
fn many_openers_of_a_fresh_file_all_open() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));

    let openers: Vec<_> = (0..8)
        .map(|_| {
            let (path, barrier) = (path.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                Store::open(&path).map(|_| ())
            })
        })
        .collect();

    for opener in openers {
        opener.join().expect("joins").expect("every opener opens");
    }
}

#[test]
fn a_wal_answer_that_lost_a_race_is_asked_again_and_then_refused() {
    let mut answers = ["delete", "wal"].into_iter();
    super::settle_wal(|| Ok(answers.next().expect("asked until WAL").to_owned()))
        .expect("the second answer is WAL");

    let mut busy = vec![Ok("wal".to_owned()), Err(rusqlite::Error::InvalidQuery)];
    super::settle_wal(|| busy.pop().expect("asked until WAL"))
        .expect("a locked answer is asked again too");

    let refused = super::settle_wal(|| Ok("delete".to_owned()))
        .expect_err("a store that never answers WAL is a refusal");
    assert!(matches!(refused, Failure::Read(_)), "{refused:?}");
}

#[test]
fn the_counter_never_makes_the_gate_wait_on_a_busy_store() {
    let (directory, store) = open();
    let path = directory.path().join("state.db");
    store
        .connection()
        .execute_batch("BEGIN IMMEDIATE;")
        .expect("another process holds the write lock");

    let started = std::time::Instant::now();
    calls::count_into(&path, || {
        calls::counting("verify-claim", || {
            calls::note(&["gh", "issue", "view", "1"])
        });
    });

    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "the counter waited {:?} on a locked store",
        started.elapsed()
    );
    store
        .connection()
        .execute_batch("ROLLBACK;")
        .expect("releases");
}

#[test]
fn one_recorded_tally_is_one_transaction() {
    let (directory, store) = open();
    let path = directory.path().join("state.db");
    store
        .connection()
        .execute_batch(
            "CREATE TRIGGER refuse_other BEFORE INSERT ON calls WHEN NEW.kind = 'other'
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .expect("a trigger that fails the last row");

    calls::count_into(&path, || {
        calls::counting("claim", || {
            calls::note(&["gh", "issue", "view", "1"]);
            calls::note(&["gh", "api", "repos/o/r/issues/1"]);
            calls::note(&["gh", "label", "create", "x"]);
        });
    });

    let summary = store.calls_since(0).expect("reads");
    assert_eq!(
        (summary.graphql, summary.rest, summary.other),
        (0, 0, 0),
        "a tally that failed halfway left part of itself behind"
    );
}

#[test]
fn an_imported_legacy_claim_keeps_the_epoch_github_named_it() {
    let (_directory, mut store) = open();
    let legacy = comment(
        "IC_77",
        "2026-01-01T00:00Z",
        &format!(
            "Claimed.\n\n<!-- issue-flow: claim run-id={RUN} runtime=claude \
             horizon=2099-01-01T00:00Z -->\n"
        ),
    );
    let github = issue(vec![legacy]);
    store.import(&repo(), 7, &github, NOW).expect("imports");

    let epoch_of = |comments: &[ownership::Comment]| {
        let held = claim::holding(comments, NOW)
            .event
            .expect("the claim holds");
        ownership::ownership_epoch(
            held.operation_id.as_deref(),
            held.comment.id.as_deref(),
            &held.comment.created_at,
            &held.comment.body,
        )
    };

    let stored = epoch_of(&store.timeline(&repo(), 7).expect("reads"));

    assert_eq!(stored, "legacy-IC_77");
    assert_eq!(stored, epoch_of(&claim::comments_of(&github)));
}

#[test]
fn transactions_over_one_issue_are_serialised_so_the_second_sees_the_first() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::issue_view_answer(
        7,
        serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
    )]);

    let mut seen = crate::test_env::with_scripted_gh_across_threads(bin.path(), &script, || {
        super::Ledger::open_at(&path, &context, 7, NOW).expect("the first touch imports");
        let racer = || {
            let ledger = super::Ledger::open_at(&path, &context, 7, NOW).expect("the ledger opens");
            ledger
                .transact(|tx| {
                    let seen = tx.timeline()?.len();
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    tx.append("racing", NOW)?;
                    Ok(seen)
                })
                .expect("the transaction commits")
        };
        std::thread::scope(|scope| {
            let first = scope.spawn(racer);
            let second = scope.spawn(racer);
            vec![
                first.join().expect("the first racer finishes"),
                second.join().expect("the second racer finishes"),
            ]
        })
    });

    seen.sort_unstable();
    assert_eq!(
        seen,
        [0, 1],
        "both transactions read the timeline before either appended to it"
    );
}

#[test]
fn a_synced_stamp_is_kept_as_github_gave_it_and_a_local_insert_stays_monotonic() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let foreign = serde_json::json!([{
        "id": 1, "node_id": "IC_f", "user": { "login": "fixture" },
        "created_at": "2026-07-28T05:00:00Z", "updated_at": "2026-07-28T05:00:00Z",
        "body": crate::transport::markers::render("adjudication", &[("target", "claude-a")])
            .expect("renders"),
    }])
    .to_string();
    let script = |comments: &str| {
        crate::test_env::answers(&[
            crate::test_env::issue_view_answer(
                7,
                serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
            ),
            serde_json::json!({ "matches": "issues/7/comments", "stdout": comments, "status": 0 }),
            serde_json::json!({
                "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0,
            }),
        ])
    };

    crate::test_env::with_scripted_gh(bin.path(), &script("[]"), || {
        let ledger = super::Ledger::open_at(&path, &context, 7, NOW).expect("opens");
        ledger
            .append("ours", "2026-07-28T06:00:00Z")
            .expect("appends");
    });
    let stamps = crate::test_env::with_scripted_gh(bin.path(), &script(&foreign), || {
        let ledger = super::Ledger::open_at(&path, &context, 7, NOW).expect("syncs");
        ledger
            .append("later", "2026-07-28T05:30:00Z")
            .expect("appends");
        ledger
            .timeline()
            .expect("reads")
            .into_iter()
            .map(|entry| entry.created_at)
            .collect::<Vec<_>>()
    });

    assert_eq!(
        stamps,
        [
            "2026-07-28T06:00:00Z",
            "2026-07-28T05:00:00Z",
            "2026-07-28T06:00:00Z"
        ],
        "the synced stamp was clamped, or a local one was stamped before an earlier row"
    );
}

#[test]
fn after_the_first_touch_only_a_stop_is_admitted_from_github_comments() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let rest = |id: &str, body: &str| {
        serde_json::json!({
            "id": id.len(), "node_id": id, "user": { "login": "fixture" },
            "created_at": "2026-07-28T05:00:00Z", "updated_at": "2026-07-28T05:00:00Z",
            "body": body,
        })
    };
    let render = |kind: &str, attributes: &[(&str, &str)]| {
        crate::transport::markers::render(kind, attributes).expect("renders")
    };
    let comments = serde_json::Value::Array(vec![
        rest("IC_prose", "please stop, claude-a (no marker, only prose)"),
        rest(
            "IC_claim",
            &render(
                "claim",
                &[("run-id", "claude-b"), ("horizon", "2099-01-01T00:00Z")],
            ),
        ),
        rest(
            "IC_unassign",
            &render("unassign", &[("run-id", "claude-a")]),
        ),
        rest(
            "IC_reclaim",
            &render("reclaim", &[("run-id", "claude-b"), ("from", "claude-a")]),
        ),
        rest("IC_stop", &render("standdown", &[("target", "claude-a")])),
        rest(
            "IC_mixed",
            &format!(
                "{}\n{}",
                render("claim", &[("run-id", "claude-b")]),
                render(
                    "adjudication",
                    &[("run-id", "claude-b"), ("target", "claude-a")]
                )
            ),
        ),
        rest(
            "IC_order",
            &render(
                "adjudication",
                &[("run-id", "claude-b"), ("target", "claude-a")],
            ),
        ),
    ])
    .to_string();
    let script = |comments: &str| {
        crate::test_env::answers(&[
            crate::test_env::issue_view_answer(
                7,
                serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
            ),
            serde_json::json!({ "matches": "issues/7/comments", "stdout": comments, "status": 0 }),
            serde_json::json!({
                "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0,
            }),
        ])
    };

    crate::test_env::with_scripted_gh(bin.path(), &script("[]"), || {
        super::Ledger::open_at(&path, &context, 7, NOW).expect("the first touch imports");
    });
    let bodies = crate::test_env::with_scripted_gh(bin.path(), &script(&comments), || {
        super::Ledger::open_at(&path, &context, 7, NOW)
            .expect("opens")
            .timeline()
            .expect("reads")
            .into_iter()
            .map(|entry| entry.body)
            .collect::<Vec<_>>()
    });

    assert_eq!(
        bodies,
        [
            render("standdown", &[("target", "claude-a")]),
            render(
                "adjudication",
                &[("run-id", "claude-b"), ("target", "claude-a")]
            ),
        ],
        "only a stand-down or an adjudication may come in; a claim, an unassign, a reclaim, a \
         mixed comment and prose decide nothing"
    );
}

#[test]
fn a_comment_list_longer_than_the_page_cap_is_a_failed_read_not_a_silent_stop() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let page: Vec<serde_json::Value> = (0..100)
        .map(|n| {
            serde_json::json!({
                "id": n, "node_id": format!("IC_{n}"), "user": { "login": "fixture" },
                "created_at": "2026-07-28T05:00:00Z", "updated_at": "2026-07-28T05:00:00Z",
                "body": format!("comment {n}"),
            })
        })
        .collect();
    let script = crate::test_env::answers(&[
        crate::test_env::issue_view_answer(
            7,
            serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
        ),
        serde_json::json!({
            "matches": "issues/7/comments",
            "stdout": serde_json::Value::Array(page).to_string(),
            "status": 0,
        }),
        serde_json::json!({
            "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0,
        }),
    ]);

    let opened = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::Ledger::open_at(&path, &context, 7, NOW).map(|_| ())
    });

    assert!(matches!(opened, Err(Failure::Read(_))), "{opened:?}");
}

#[test]
fn rows_sharing_a_stamp_are_ordered_imported_then_by_github_id_then_unlisted() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let context = crate::transport::Context {
        skill_dir: directory.path().to_path_buf(),
        repo_dir: directory.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let bin = crate::test_env::scripted_gh();
    let at = "2026-07-28T05:00:00Z";
    let order = |id: i64| {
        crate::transport::markers::render("adjudication", &[("target", &format!("claude-{id}"))])
            .expect("renders")
    };
    let rest = |id: i64| {
        serde_json::json!({
            "id": id, "node_id": format!("IC_{id}"), "user": { "login": "fixture" },
            "created_at": at, "updated_at": at, "body": order(id),
        })
    };
    let script = |listed: Vec<serde_json::Value>| {
        crate::test_env::answers(&[
            crate::test_env::issue_view_answer(
                7,
                serde_json::json!({
                    "state": "OPEN", "labels": [],
                    "comments": [comment("IC_0", at, "imported")],
                }),
            ),
            serde_json::json!({
                "matches": "issues/7/comments",
                "stdout": serde_json::Value::Array(listed).to_string(),
                "status": 0,
            }),
            serde_json::json!({
                "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0,
            }),
        ])
    };

    crate::test_env::with_scripted_gh(bin.path(), &script(vec![]), || {
        let ledger = super::Ledger::open_at(&path, &context, 7, NOW).expect("imports");
        ledger.append("unlisted", at).expect("appends");
    });
    let seen =
        crate::test_env::with_scripted_gh(bin.path(), &script(vec![rest(5), rest(3)]), || {
            super::Ledger::open_at(&path, &context, 7, NOW)
                .expect("syncs")
                .timeline()
                .expect("reads")
                .into_iter()
                .map(|entry| entry.body.trim().to_owned())
                .collect::<Vec<_>>()
        });

    assert_eq!(
        seen,
        [
            "imported".to_owned(),
            order(3),
            order(5),
            "unlisted".to_owned()
        ],
        "a tie was not broken by GitHub's order"
    );
}

fn reason_of(failure: Failure) -> String {
    failure.envelope()["reason"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_store_only_timeline_read_makes_no_gh_call() {
    let (directory, mut store) = open();
    let path = directory.path().join("state.db");
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");
    let bin = crate::test_env::scripted_gh();

    let stored = calls::count_into(&path, || {
        crate::test_env::with_scripted_gh(bin.path(), &crate::test_env::answers(&[]), || {
            calls::counting("status", || {
                super::ledger::Ledger::read_timeline_at(&path, &repo(), 7)
            })
        })
    })
    .expect("the stored timeline reads");

    assert_eq!(holder_of(&stored), Some(RUN.to_owned()));
    let summary = store.calls_since(0).expect("reads");
    assert_eq!((summary.graphql, summary.rest, summary.other), (0, 0, 0));
}

#[test]
fn an_issue_the_store_does_not_hold_is_refused_not_read_as_an_empty_timeline() {
    let (directory, mut store) = open();
    let path = directory.path().join("state.db");
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");

    let failure = super::ledger::Ledger::read_timeline_at(&path, &repo(), 8)
        .expect_err("an issue never imported is not an empty one");

    assert_eq!(reason_of(failure), "review-status-issue-not-in-store");
}

#[test]
fn a_store_file_that_does_not_exist_is_not_created_by_the_read() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");

    let failure =
        super::ledger::Ledger::read_timeline_at(&path, &repo(), 7).expect_err("nothing to read");

    assert_eq!(reason_of(failure), "review-status-issue-not-in-store");
    assert!(!path.exists(), "a read must not create the store");
}

#[test]
fn an_unreadable_store_is_refused_as_unreadable() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    std::fs::write(&path, "this is not a database").expect("writes");

    let failure =
        super::ledger::Ledger::read_timeline_at(&path, &repo(), 7).expect_err("not a database");

    assert_eq!(reason_of(failure), "review-status-store-unreadable");
}

#[test]
fn a_store_of_another_schema_is_unreadable_and_is_left_unmigrated() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    drop(Store::open(&path).expect("opens"));
    rusqlite::Connection::open(&path)
        .expect("opens raw")
        .execute("UPDATE meta SET schema_version = 99", [])
        .expect("writes");

    let failure = super::ledger::Ledger::read_timeline_at(&path, &repo(), 7)
        .expect_err("a newer schema is not read");

    assert_eq!(reason_of(failure), "review-status-store-unreadable");
    let raw = rusqlite::Connection::open(&path).expect("opens raw");
    let version: u32 = raw
        .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
        .expect("reads");
    assert_eq!(version, 99, "the read migrated the store");
}

#[test]
fn a_successful_read_leaves_the_store_bytes_unchanged() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    let mut store = Store::open(&path).expect("opens");
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");
    drop(store);
    let before = std::fs::read(&path).expect("reads the bytes");

    super::ledger::Ledger::read_timeline_at(&path, &repo(), 7).expect("reads");

    assert_eq!(std::fs::read(&path).expect("reads the bytes"), before);
}

#[cfg(unix)]
#[test]
fn a_store_whose_directory_cannot_be_inspected_is_unreadable_not_absent() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("a temporary directory");
    let parent = directory.path().join("closed");
    std::fs::create_dir(&parent).expect("creates");
    let path = parent.join("state.db");
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o000)).expect("closes");

    let failure = super::ledger::Ledger::read_timeline_at(&path, &repo(), 7);

    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).expect("reopens");
    assert_eq!(
        reason_of(failure.expect_err("an unreadable path")),
        "review-status-store-unreadable"
    );
}

fn receipt(epoch: &str) -> crate::reviewer::Receipt {
    crate::reviewer::Receipt {
        epoch: epoch.to_owned(),
        pr: 7,
        head: "h".repeat(40),
        base: "b".repeat(40),
        digest: "d".repeat(64),
        payload_digest: "p".repeat(64),
    }
}

fn with_tx<T>(body: impl FnOnce(&super::ledger::Tx<'_>) -> T) -> T {
    let (_directory, mut store) = open();
    store
        .import(&repo(), 7, &issue(vec![claim_comment()]), NOW)
        .expect("imports");
    let transaction = rusqlite::Transaction::new_unchecked(
        store.connection(),
        rusqlite::TransactionBehavior::Immediate,
    )
    .expect("begins");
    let repo_id = transaction
        .query_row("SELECT id FROM repos", [], |row| row.get(0))
        .expect("a repo");
    let tx = super::ledger::Tx {
        transaction: &transaction,
        repo_id,
        issue: 7,
    };
    body(&tx)
}

fn new_job<'a>(
    id: &'a str,
    name: &'a str,
    receipt: &'a crate::reviewer::Receipt,
    offered: &'a serde_json::Value,
) -> super::jobs::NewJob<'a> {
    super::jobs::NewJob {
        id,
        run_id: RUN,
        name,
        receipt,
        offered,
        attempt: None,
        started_at: 100,
        deadline_at: 1000,
        dir: "/jobs/x",
    }
}

fn recorded(inserted: super::jobs::Inserted) -> bool {
    matches!(inserted, super::jobs::Inserted::Recorded)
}

#[test]
fn migration_five_creates_the_review_jobs_table() {
    let (_directory, store) = open();
    assert_eq!(store.schema_version().expect("a version"), 17);
    let columns: Vec<String> = store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('review_jobs')")
        .expect("prepares")
        .query_map([], |row| row.get(0))
        .expect("reads")
        .collect::<Result<_, _>>()
        .expect("rows");
    for wanted in [
        "id",
        "repo_id",
        "issue",
        "run_id",
        "name",
        "epoch",
        "pr",
        "head",
        "base",
        "digest",
        "payload_digest",
        "offered",
        "attempt",
        "pid",
        "started_at",
        "deadline_at",
        "dir",
        "state",
    ] {
        assert!(columns.iter().any(|found| found == wanted), "no {wanted}");
    }
}

#[test]
fn migration_six_creates_the_blocked_clauses_table() {
    let (_directory, store) = open();
    assert_eq!(store.schema_version().expect("a version"), 17);
    let columns: Vec<String> = store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('blocked_clauses')")
        .expect("prepares")
        .query_map([], |row| row.get(0))
        .expect("reads")
        .collect::<Result<_, _>>()
        .expect("rows");
    for wanted in ["repo_id", "issue", "body", "at"] {
        assert!(columns.iter().any(|found| found == wanted), "no {wanted}");
    }
}

#[test]
fn migration_seven_creates_the_record_tables() {
    let (_directory, store) = open();
    assert_eq!(store.schema_version().expect("a version"), 17);
    for (table, wanted) in [
        ("decisions", ["at", "body"].as_slice()),
        ("roles", ["at", "body"].as_slice()),
        ("record_failures", ["kind", "count"].as_slice()),
        ("store_meta", ["key", "value"].as_slice()),
    ] {
        let columns: Vec<String> = store
            .connection()
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .expect("prepares")
            .query_map([], |row| row.get(0))
            .expect("reads")
            .collect::<Result<_, _>>()
            .expect("rows");
        for column in wanted {
            assert!(
                columns.iter().any(|found| found == column),
                "no {table}.{column}"
            );
        }
    }
}

#[test]
fn records_round_trip_and_the_prune_takes_only_the_stale_ones() {
    use super::Record;
    let (_directory, store) = open();
    store
        .add_record(
            Record::Decision,
            &serde_json::json!({"at": 10, "verdict": "stale"}),
        )
        .expect("a decision");
    store
        .add_record(
            Record::Decision,
            &serde_json::json!({"at": 90, "verdict": "keep"}),
        )
        .expect("a decision");
    store
        .add_record(
            Record::Role,
            &serde_json::json!({"at": 5, "agent_type": "stale"}),
        )
        .expect("a role");
    store
        .add_record(
            Record::Role,
            &serde_json::json!({"at": 95, "agent_type": "keep"}),
        )
        .expect("a role");
    // A record with no `at` is never old enough to drop.
    store
        .add_record(Record::Decision, &serde_json::json!({"verdict": "undated"}))
        .expect("a decision");

    assert_eq!(store.records(Record::Decision).expect("reads").len(), 3);
    assert_eq!(store.prune_records(50).expect("prunes"), 2);

    let kept: Vec<String> = store
        .records(Record::Decision)
        .expect("reads")
        .iter()
        .filter_map(|record| {
            record
                .get("verdict")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(kept, ["keep", "undated"]);
    let roles: Vec<String> = store
        .records(Record::Role)
        .expect("reads")
        .iter()
        .filter_map(|record| {
            record
                .get("agent_type")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(roles, ["keep"]);

    store.set_meta("last-prune", "42").expect("a marker");
    assert_eq!(
        store.meta("last-prune").expect("reads").as_deref(),
        Some("42")
    );
    assert_eq!(store.meta("never-written").expect("reads"), None);
    store.note_record_failure(Record::Decision).expect("counts");
    store.note_record_failure(Record::Decision).expect("counts");
    assert_eq!(
        store.record_failures().expect("reads"),
        [("decisions".to_owned(), 2)]
    );
}

#[test]
fn an_import_records_the_blocked_until_clauses_the_body_carried() {
    let (_directory, mut store) = open();
    store
        .import(
            &repo(),
            21,
            &serde_json::json!({
                "body": "<!-- issue-flow: blocked-until issue=20 state=done -->\n\
                         <!-- issue-flow: blocked-until issue=19 -->",
                "labels": [{ "name": "status:blocked" }],
                "comments": [],
            }),
            NOW,
        )
        .expect("the issue imports");
    let clauses = store
        .blocked_clauses(&repo(), 21)
        .expect("the clauses read");
    assert_eq!(clauses.len(), 2, "{clauses:?}");
    assert!(
        clauses[0].contains("issue=20") && clauses[0].contains("state=done"),
        "{clauses:?}"
    );
    // The malformed clause is kept, so the reader can report it rather than
    // count it as holding.
    assert!(clauses[1].contains("issue=19"), "{clauses:?}");
}

#[test]
fn the_issue_rows_carry_the_state_and_identity_the_store_recorded() {
    let (_directory, mut store) = open();
    store
        .import(
            &repo(),
            21,
            &serde_json::json!({
                "body": "<!-- issue-flow: analysis encoding=pct identity=a-probe -->",
                "labels": [{ "name": "status:ready" }],
                "comments": [],
            }),
            NOW,
        )
        .expect("the issue imports");
    let rows = store.issue_rows(&repo()).expect("the rows read");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].number, 21);
    assert_eq!(rows[0].state.as_deref(), Some("ready"));
    assert_eq!(rows[0].identity.as_deref(), Some("a-probe"));
    // A repository the store has never seen answers nothing, not an error.
    let other = Repo {
        host: "github.com".to_owned(),
        owner: "someone".to_owned(),
        name: "else".to_owned(),
    };
    assert!(
        store.issue_rows(&other).expect("the rows read").is_empty(),
        "an unknown repository answered rows"
    );
}

#[test]
fn a_second_running_job_for_one_slot_is_refused_and_a_finished_one_frees_it() {
    with_tx(|tx| {
        let receipt = receipt("e1");
        let offered = serde_json::json!({"role": "risk"});
        let insert = |id, name, receipt| {
            recorded(
                tx.insert_job(&new_job(id, name, receipt, &offered))
                    .expect("reads"),
            )
        };
        assert!(insert("a", "review-risk", &receipt));
        assert!(
            !insert("b", "review-risk", &receipt),
            "a second launch for the slot"
        );
        assert!(
            insert("c", "review-resilience", &receipt),
            "another slot is free"
        );
        let other_epoch = self::receipt("e2");
        assert!(
            insert("d", "review-risk", &other_epoch),
            "another epoch is free"
        );

        assert!(
            tx.compare_and_set_state("a", "running", "abandoned")
                .unwrap()
        );
        assert!(
            insert("e", "review-risk", &receipt),
            "an abandoned job no longer holds the slot"
        );
        let found = tx
            .job_for_slot("e1", "review-risk")
            .expect("reads")
            .expect("a job");
        assert_eq!((found.id.as_str(), found.state.as_str()), ("e", "running"));
        assert_eq!(found.receipt, receipt);
        assert_eq!(found.offered, offered);
    });
}

#[test]
fn the_unique_index_alone_refuses_a_second_running_job() {
    with_tx(|tx| {
        let receipt = receipt("e1");
        let offered = serde_json::json!({});
        tx.insert_job(&new_job("a", "review-risk", &receipt, &offered))
            .expect("recorded");
        let direct = tx.transaction.execute(
            "INSERT INTO review_jobs (id, repo_id, issue, run_id, name, epoch, pr, head, base,
                digest, payload_digest, offered, started_at, deadline_at, dir, state)
             SELECT 'z', repo_id, issue, run_id, name, epoch, pr, head, base, digest,
                payload_digest, offered, started_at, deadline_at, dir, 'admitting'
             FROM review_jobs WHERE id = 'a'",
            [],
        );
        assert!(direct.is_err(), "the index let a second live job in");
    });
}

#[test]
fn running_moves_to_admitting_exactly_once() {
    with_tx(|tx| {
        let receipt = receipt("e1");
        let offered = serde_json::json!({});
        tx.insert_job(&new_job("a", "review-risk", &receipt, &offered))
            .expect("recorded");
        assert!(
            tx.compare_and_set_state("a", "running", "admitting")
                .unwrap()
        );
        assert!(
            !tx.compare_and_set_state("a", "running", "admitting")
                .unwrap()
        );
        let state = tx.job_for_slot("e1", "review-risk").unwrap().unwrap().state;
        assert_eq!(state, "admitting");
        assert!(
            tx.compare_and_set_state("a", "admitting", "admitted")
                .unwrap()
        );
        assert!(
            !tx.compare_and_set_state("nothing", "running", "admitted")
                .unwrap()
        );
    });
}

#[test]
fn a_recorded_process_is_read_back() {
    with_tx(|tx| {
        let receipt = receipt("e1");
        let offered = serde_json::json!({});
        tx.insert_job(&new_job("a", "review-risk", &receipt, &offered))
            .expect("recorded");
        assert_eq!(
            tx.job_for_slot("e1", "review-risk").unwrap().unwrap().pid,
            None
        );
        tx.mark_spawned("a", 4242).expect("marks");
        assert_eq!(
            tx.job_for_slot("e1", "review-risk").unwrap().unwrap().pid,
            Some(4242)
        );
    });
}

#[test]
fn the_jobs_root_sits_beside_the_store() {
    let home = std::path::Path::new("/home/someone");
    assert_eq!(
        super::jobs_root(Some(home)).unwrap(),
        home.join(".estigia").join("jobs")
    );
}

#[test]
fn a_fresh_store_carries_the_task_table_and_an_older_one_gains_it() {
    let (directory, store) = open();
    assert_eq!(
        store.schema_version().expect("the version reads"),
        u32::try_from(super::schema::MIGRATIONS.len()).expect("a few migrations")
    );
    assert_eq!(store.tasks(&repo(), 7).expect("no tasks read"), []);
    drop(store);

    let path = directory.path().join("older.db");
    let older = rusqlite::Connection::open(&path).expect("a file to age");
    let before = super::schema::MIGRATIONS.len() - 1;
    older
        .execute_batch("CREATE TABLE meta (schema_version INTEGER NOT NULL);")
        .expect("the version table");
    for sql in &super::schema::MIGRATIONS[..before] {
        older.execute_batch(sql).expect("an old migration applies");
    }
    older
        .execute(
            "INSERT INTO meta (schema_version) VALUES (?1)",
            [i64::try_from(before).expect("small")],
        )
        .expect("the old version is recorded");
    drop(older);
    let migrated = Store::open(&path).expect("the older store migrates");
    assert_eq!(migrated.tasks(&repo(), 7).expect("no tasks read"), []);
}

#[test]
fn a_fresh_store_and_an_older_one_both_carry_the_append_only_transitions_table() {
    let (directory, store) = open();
    let append_only = |store: &Store| {
        store
            .connection()
            .execute(
                "INSERT INTO issues (repo_id, number, history) VALUES (1, 9, 'native')",
                [],
            )
            .ok();
        store
            .connection()
            .execute(
                "INSERT INTO transitions (repo_id, issue, at, run_id, from_state, to_state)
                 VALUES (1, 9, ?1, NULL, NULL, 'ready')",
                [NOW],
            )
            .expect("a transition appends");
        for statement in [
            "UPDATE transitions SET to_state = 'done'",
            "DELETE FROM transitions",
        ] {
            let error = store
                .connection()
                .execute(statement, [])
                .expect_err("the append-only table accepted it");
            assert!(
                error.to_string().contains("append-only"),
                "{statement}: {error}"
            );
        }
    };
    store
        .connection()
        .execute(
            "INSERT INTO repos (host, owner, name) VALUES ('github.com', 'asanabrial', 'estigia')",
            [],
        )
        .expect("a repo");
    append_only(&store);
    drop(store);

    let path = directory.path().join("older.db");
    let older = rusqlite::Connection::open(&path).expect("a file to age");
    let before = super::schema::MIGRATIONS.len() - 1;
    older
        .execute_batch("CREATE TABLE meta (schema_version INTEGER NOT NULL);")
        .expect("the version table");
    for sql in &super::schema::MIGRATIONS[..before] {
        older.execute_batch(sql).expect("an old migration applies");
    }
    older
        .execute(
            "INSERT INTO meta (schema_version) VALUES (?1)",
            [i64::try_from(before).expect("small")],
        )
        .expect("the old version is recorded");
    older
        .execute(
            "INSERT INTO repos (host, owner, name) VALUES ('github.com', 'asanabrial', 'estigia')",
            [],
        )
        .expect("a repo");
    drop(older);
    let migrated = Store::open(&path).expect("the older store migrates");
    append_only(&migrated);
}

#[test]
fn an_older_store_gains_the_task_evidence_column_and_keeps_its_rows() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("older.db");
    let older = rusqlite::Connection::open(&path).expect("a file to age");
    let before = super::schema::MIGRATIONS.len() - 1;
    older
        .execute_batch("CREATE TABLE meta (schema_version INTEGER NOT NULL);")
        .expect("the version table");
    for sql in &super::schema::MIGRATIONS[..before] {
        older.execute_batch(sql).expect("an old migration applies");
    }
    older
        .execute(
            "INSERT INTO repos (host, owner, name) VALUES ('github.com', 'asanabrial', 'estigia')",
            [],
        )
        .expect("a repo");
    older
        .execute(
            "INSERT INTO tasks (repo_id, issue, id, text, route, done, commit_id, at)
             VALUES (1, 7, 'T1', 'one', 'inline', 1, 'abcdef0', '2026-01-01T00:00Z')",
            [],
        )
        .expect("a task from the older schema");
    older
        .execute(
            "INSERT INTO meta (schema_version) VALUES (?1)",
            [i64::try_from(before).expect("small")],
        )
        .expect("the old version is recorded");
    drop(older);

    let store = Store::open(&path).expect("the older store migrates");
    let columns: Vec<String> = store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('tasks')")
        .expect("prepares")
        .query_map([], |row| row.get(0))
        .expect("reads")
        .collect::<Result<_, _>>()
        .expect("rows");
    assert!(
        columns.iter().any(|name| name == "evidence"),
        "the gained column is missing: {columns:?}"
    );
    let tasks = store.tasks(&repo(), 7).expect("the tasks read");
    assert_eq!(tasks.len(), 1, "{tasks:?}");
    assert_eq!(tasks[0].commit.as_deref(), Some("abcdef0"));
    assert_eq!(
        tasks[0].evidence, None,
        "the gained column was filled with something nobody wrote"
    );
}

#[test]
fn recorded_clauses_are_kept_in_order_once_each_and_cleared_together() {
    let (_directory, mut store) = open();
    store
        .import(&repo(), 7, &issue(Vec::new()), NOW)
        .expect("imports");
    let first = "<!-- issue-flow: blocked-until encoding=pct issue=3 state=done -->";
    let second = "<!-- issue-flow: blocked-until encoding=pct issue=4 state=ready -->";

    assert!(
        store
            .record_blocked_clause(&repo(), 7, first, NOW)
            .expect("records")
    );
    assert!(
        store
            .record_blocked_clause(&repo(), 7, second, NOW)
            .expect("records")
    );
    assert!(
        !store
            .record_blocked_clause(&repo(), 7, first, NOW)
            .expect("a repeat")
    );
    assert_eq!(
        store.blocked_clauses(&repo(), 7).expect("reads"),
        [first, second]
    );
    assert_eq!(store.clear_blocked_clauses(&repo(), 7).expect("clears"), 2);
    assert_eq!(
        store.blocked_clauses(&repo(), 7).expect("reads"),
        Vec::<String>::new()
    );
}

#[test]
fn an_issue_imported_as_recorded_keeps_the_identity_and_clauses_it_was_given() {
    let (_directory, mut store) = open();
    let clause = "<!-- issue-flow: blocked-until encoding=pct issue=3 state=done -->".to_owned();
    let recorded = super::Recorded {
        identity: Some("filed-key"),
        clauses: std::slice::from_ref(&clause),
        review: None,
    };
    store
        .import_recorded(&repo(), 7, &issue(Vec::new()), NOW, &recorded)
        .expect("imports");

    let rows = store.issue_rows(&repo()).expect("the issues read");
    assert_eq!(rows[0].identity.as_deref(), Some("filed-key"));
    assert_eq!(store.blocked_clauses(&repo(), 7).expect("reads"), [clause]);
}

#[test]
fn a_task_is_revised_in_place_and_keeps_its_place() {
    let (_directory, mut store) = open();
    store
        .import(&repo(), 7, &issue(Vec::new()), NOW)
        .expect("imports");
    let change = |id, text, route, done, commit| super::TaskChange {
        id,
        text,
        route,
        done,
        commit,
        evidence: None,
    };
    for (id, text) in [("T1", "one"), ("T2", "two")] {
        store
            .change_task(
                &repo(),
                7,
                &change(id, Some(text), Some("inline"), None, None),
                NOW,
            )
            .expect("a new task records");
    }
    let revised = store
        .change_task(
            &repo(),
            7,
            &change("T1", None, Some("delegated"), Some(true), Some("abc1234")),
            NOW,
        )
        .expect("a task revises");
    assert_eq!(
        (revised.text.as_str(), revised.route.as_str(), revised.done),
        ("one", "delegated", true)
    );
    let reopened = store
        .change_task(
            &repo(),
            7,
            &change("T1", None, None, Some(false), None),
            NOW,
        )
        .expect("a task reopens");
    assert!(!reopened.done);
    assert_eq!(reopened.commit.as_deref(), Some("abc1234"));
    let ids: Vec<String> = store
        .tasks(&repo(), 7)
        .expect("reads")
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, ["T1", "T2"]);
}

#[test]
fn migration_eight_creates_the_tasks_table() {
    let (_directory, store) = open();
    let columns: Vec<String> = store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('tasks')")
        .expect("the table is described")
        .query_map([], |row| row.get(0))
        .expect("the columns read")
        .collect::<Result<_, _>>()
        .expect("every column reads");
    assert_eq!(
        columns,
        [
            "repo_id",
            "issue",
            "id",
            "text",
            "route",
            "done",
            "commit_id",
            "at",
            "evidence"
        ]
    );
}

mod outbox {
    use super::*;
    use crate::transport::outbox::{PROVISIONAL_BASE, is_provisional};
    use crate::transport::store::{OutboxKind, OutboxState};

    fn queue(store: &mut Store, issue: u64, kind: OutboxKind, op: &str) -> Result<u64, Failure> {
        store.enqueue(
            &repo(),
            issue,
            kind,
            &serde_json::json!({ "n": op }),
            op,
            NOW,
        )
    }

    fn order(store: &Store) -> Vec<(u64, OutboxKind, OutboxState)> {
        store
            .outbox(&repo())
            .expect("the queue reads")
            .into_iter()
            .map(|row| (row.issue, row.kind, row.state))
            .collect()
    }

    #[test]
    fn a_fresh_store_carries_the_outbox_and_an_older_one_gains_it() {
        let (directory, store) = open();
        for table in ["outbox", "provisional_issues"] {
            let found: i64 = store
                .connection()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("reads");
            assert_eq!(found, 1, "{table} is missing from a fresh store");
        }
        drop(store);

        let path = directory.path().join("v8.db");
        let older = rusqlite::Connection::open(&path).expect("a file to age");
        let before = super::super::schema::MIGRATIONS.len() - 1;
        older
            .execute_batch("CREATE TABLE meta (schema_version INTEGER NOT NULL);")
            .expect("the version table");
        for sql in &super::super::schema::MIGRATIONS[..before] {
            older.execute_batch(sql).expect("an old migration applies");
        }
        older
            .execute(
                "INSERT INTO meta (schema_version) VALUES (?1)",
                [i64::try_from(before).expect("small")],
            )
            .expect("recorded");
        drop(older);
        let migrated = Store::open(&path).expect("the older store migrates");
        assert_eq!(migrated.outbox(&repo()).expect("reads"), []);
    }

    #[test]
    fn the_provisional_range_sits_above_every_issue_number_and_inside_a_safe_integer() {
        const {
            assert!(PROVISIONAL_BASE > 1_000_000_000_000);
            assert!(PROVISIONAL_BASE < (1u64 << 53));
        }
        assert!(is_provisional(PROVISIONAL_BASE + 1));
        assert!(is_provisional(PROVISIONAL_BASE));
        assert!(!is_provisional(PROVISIONAL_BASE - 1));
        assert!(!is_provisional(1078));
    }

    #[test]
    fn provisional_numbers_are_distinct_increasing_and_survive_a_reopen() {
        let (directory, mut store) = open();
        let first = store.allocate_provisional(&repo()).expect("allocates");
        let second = store.allocate_provisional(&repo()).expect("allocates");
        assert_eq!(first, PROVISIONAL_BASE + 1);
        assert_eq!(second, PROVISIONAL_BASE + 2);
        drop(store);
        let mut store = Store::open(&directory.path().join("state.db")).expect("reopens");
        assert_eq!(
            store.allocate_provisional(&repo()).expect("allocates"),
            PROVISIONAL_BASE + 3
        );
        assert_eq!(
            store.provisional_real(&repo(), first).expect("reads"),
            Some(None),
            "an allocated number is known and has no real number yet"
        );
        assert_eq!(store.provisional_real(&repo(), 1078).expect("reads"), None);
    }

    #[test]
    fn writes_queue_in_order_and_the_next_one_is_the_oldest_unsent() {
        let (_directory, mut store) = open();
        let first = queue(&mut store, 7, OutboxKind::Comment, "aa").expect("queues");
        let second = queue(&mut store, 8, OutboxKind::Create, "bb").expect("queues");
        assert!(second > first);
        assert_eq!(
            store
                .next_outbox(&repo())
                .expect("reads")
                .expect("a row")
                .seq,
            first
        );

        store
            .mark_outbox(first, OutboxState::Sent, None, NOW)
            .expect("marks");
        let next = store.next_outbox(&repo()).expect("reads").expect("a row");
        assert_eq!(
            (next.seq, next.kind, next.issue),
            (second, OutboxKind::Create, 8)
        );
        assert_eq!(next.payload["n"], "bb");
        assert_eq!(next.op_id, "bb");

        store
            .mark_outbox(second, OutboxState::Sent, None, NOW)
            .expect("marks");
        assert!(store.next_outbox(&repo()).expect("reads").is_none());
    }

    #[test]
    fn a_row_keeps_the_state_and_detail_it_was_marked_with() {
        let (_directory, mut store) = open();
        let seq = queue(&mut store, 7, OutboxKind::Link, "aa").expect("queues");
        store
            .mark_outbox(seq, OutboxState::Sending, None, "2026-01-01T00:11Z")
            .expect("marks");
        let sending = store.next_outbox(&repo()).expect("reads").expect("a row");
        assert_eq!(sending.state, OutboxState::Sending);
        assert_eq!(sending.attempted_at.as_deref(), Some("2026-01-01T00:11Z"));

        store
            .mark_outbox(seq, OutboxState::Queued, Some("gh refused"), NOW)
            .expect("marks");
        let back = store.next_outbox(&repo()).expect("reads").expect("a row");
        assert_eq!(back.state, OutboxState::Queued);
        assert_eq!(back.detail.as_deref(), Some("gh refused"));
        assert!(
            store
                .mark_outbox(seq + 100, OutboxState::Sent, None, NOW)
                .is_err(),
            "marking a row that is not there said it worked"
        );
    }

    #[test]
    fn a_second_reconverge_for_an_issue_replaces_the_queued_one_at_the_tail() {
        let (_directory, mut store) = open();
        queue(&mut store, 7, OutboxKind::Reconverge, "r1").expect("queues");
        queue(&mut store, 8, OutboxKind::Reconverge, "r2").expect("queues");
        queue(&mut store, 7, OutboxKind::Comment, "c1").expect("queues");
        queue(&mut store, 7, OutboxKind::Reconverge, "r3").expect("queues");

        assert_eq!(
            order(&store),
            [
                (8, OutboxKind::Reconverge, OutboxState::Queued),
                (7, OutboxKind::Comment, OutboxState::Queued),
                (7, OutboxKind::Reconverge, OutboxState::Queued),
            ]
        );
        let ops: Vec<_> = store
            .outbox(&repo())
            .expect("reads")
            .into_iter()
            .map(|row| row.op_id)
            .collect();
        assert_eq!(ops, ["r2", "c1", "r3"]);
    }

    #[test]
    fn a_reconverge_already_sending_is_not_replaced() {
        let (_directory, mut store) = open();
        let seq = queue(&mut store, 7, OutboxKind::Reconverge, "r1").expect("queues");
        store
            .mark_outbox(seq, OutboxState::Sending, None, NOW)
            .expect("marks");
        queue(&mut store, 7, OutboxKind::Reconverge, "r2").expect("queues");
        assert_eq!(
            order(&store),
            [
                (7, OutboxKind::Reconverge, OutboxState::Sending),
                (7, OutboxKind::Reconverge, OutboxState::Queued),
            ]
        );
    }

    #[test]
    fn two_comments_for_one_issue_both_stay_and_an_operation_id_is_used_once() {
        let (_directory, mut store) = open();
        queue(&mut store, 7, OutboxKind::Comment, "c1").expect("queues");
        queue(&mut store, 7, OutboxKind::Comment, "c2").expect("queues");
        assert_eq!(order(&store).len(), 2);
        assert!(queue(&mut store, 7, OutboxKind::Comment, "c1").is_err());
        assert_eq!(order(&store).len(), 2);
    }

    #[test]
    fn the_schema_itself_refuses_two_queued_reconverges_for_one_issue() {
        let (_directory, mut store) = open();
        queue(&mut store, 7, OutboxKind::Reconverge, "r1").expect("queues");
        let error = store
            .connection()
            .execute(
                "INSERT INTO outbox (repo_id, issue, kind, payload, op_id, state, enqueued_at)
                 SELECT repo_id, issue, kind, payload, 'other', state, enqueued_at FROM outbox",
                [],
            )
            .expect_err("a second queued reconverge was accepted");
        assert!(error.to_string().contains("UNIQUE"), "{error}");
    }

    fn entry_on(store: &Store, issue: i64) {
        store
            .connection()
            .execute(
                "INSERT OR IGNORE INTO repos (id, host, owner, name)
                 VALUES (1, 'github.com', 'asanabrial', 'estigia')",
                [],
            )
            .expect("a repo");
        store
            .connection()
            .execute(
                "INSERT INTO issues (repo_id, number, history) VALUES (1, ?1, 'native')",
                [issue],
            )
            .expect("an issue");
        store
            .connection()
            .execute(
                "INSERT INTO entries (repo_id, issue, created_at, body, viewer_did_author,
                    includes_created_edit, source)
                 VALUES (1, ?1, ?2, 'body', 1, 0, 'local')",
                rusqlite::params![issue, NOW],
            )
            .expect("an entry");
    }

    #[test]
    fn an_entry_may_move_from_a_provisional_issue_to_its_recorded_real_number_and_nowhere_else() {
        let (_directory, store) = open();
        let provisional = i64::try_from(PROVISIONAL_BASE + 1).expect("fits");
        entry_on(&store, provisional);
        store
            .connection()
            .execute(
                "INSERT INTO provisional_issues (repo_id, provisional, real) VALUES (1, ?1, NULL)",
                [provisional],
            )
            .expect("recorded");
        store
            .connection()
            .execute(
                "INSERT INTO issues (repo_id, number, history) VALUES (1, 55, 'native'),
                    (1, 56, 'native')",
                [],
            )
            .expect("real issues");

        let refused = |statement: &str| {
            let error = store
                .connection()
                .execute(statement, [])
                .expect_err("the append-only table accepted it");
            assert!(
                error.to_string().contains("append-only"),
                "{statement}: {error}"
            );
        };
        refused("UPDATE entries SET issue = 55");
        store
            .connection()
            .execute(
                "UPDATE provisional_issues SET real = 55 WHERE repo_id = 1",
                [],
            )
            .expect("the real number is recorded");
        refused("UPDATE entries SET issue = 56");
        refused("UPDATE entries SET issue = 55, body = 'rewritten'");
        refused("UPDATE entries SET body = 'rewritten'");
        refused("DELETE FROM entries");

        store
            .connection()
            .execute("UPDATE entries SET issue = 55", [])
            .expect("the recorded move is the one permitted rewrite");
        let moved: i64 = store
            .connection()
            .query_row("SELECT issue FROM entries", [], |row| row.get(0))
            .expect("reads");
        assert_eq!(moved, 55);
    }
}

mod rekey {
    use super::*;
    use crate::transport::outbox::PROVISIONAL_BASE;
    use crate::transport::store::{OutboxKind, OutboxState, Recorded, TaskChange};

    const REAL: u64 = 55;

    fn provisional_issue() -> (tempfile::TempDir, Store, u64) {
        let (directory, mut store) = open();
        let provisional = store.allocate_provisional(&repo()).expect("allocates");
        store
            .import_recorded(
                &repo(),
                provisional,
                &issue(vec![claim_comment()]),
                NOW,
                &Recorded::default(),
            )
            .expect("the provisional issue is held");
        store
            .change_task(
                &repo(),
                provisional,
                &TaskChange {
                    id: "T1",
                    text: Some("a task"),
                    route: Some("inline"),
                    done: None,
                    commit: None,
                    evidence: None,
                },
                NOW,
            )
            .expect("a task");
        store
            .enqueue(
                &repo(),
                provisional,
                OutboxKind::Create,
                &serde_json::json!({}),
                "aa",
                NOW,
            )
            .expect("queued");
        store
            .import(&repo(), 7, &issue(Vec::new()), NOW)
            .expect("a blocked issue");
        store
            .record_blocked_clause(
                &repo(),
                7,
                &format!(
                    "<!-- issue-flow: blocked-until encoding=pct issue={provisional} state=done -->"
                ),
                NOW,
            )
            .expect("a clause pointing at the provisional issue");
        store
            .connection()
            .execute(
                "INSERT INTO review_jobs (id, repo_id, issue, run_id, name, epoch, pr, head, base,
                    digest, payload_digest, offered, started_at, deadline_at, dir, state)
                 SELECT 'job', id, ?1, 'r', 'n', 'e', 1, 'h', 'b', 'd', 'p', '{}', 1, 2, 'x',
                        'abandoned' FROM repos",
                [i64::try_from(provisional).expect("fits")],
            )
            .expect("a review job");
        (directory, store, provisional)
    }

    #[test]
    fn rekeying_moves_every_row_of_the_issue_to_the_real_number() {
        let (_directory, mut store, provisional) = provisional_issue();
        let seqs_before: Vec<_> = store
            .timeline(&repo(), provisional)
            .expect("reads")
            .into_iter()
            .map(|comment| comment.id)
            .collect();

        store
            .rekey(&repo(), provisional, REAL, None)
            .expect("rekeys");

        let seqs_after: Vec<_> = store
            .timeline(&repo(), REAL)
            .expect("reads")
            .into_iter()
            .map(|comment| comment.id)
            .collect();
        assert_eq!(seqs_after, seqs_before, "the entries kept their identity");
        assert!(
            store
                .timeline(&repo(), provisional)
                .expect("reads")
                .is_empty()
        );
        assert_eq!(store.tasks(&repo(), REAL).expect("reads").len(), 1);
        assert!(store.tasks(&repo(), provisional).expect("reads").is_empty());
        assert_eq!(
            store.issue_state(&repo(), REAL).expect("reads"),
            Some("in-progress".to_owned())
        );
        assert_eq!(
            store.issue_state(&repo(), provisional).expect("reads"),
            None
        );
        assert_eq!(
            store.outbox(&repo()).expect("reads")[0].issue,
            REAL,
            "the queued write follows the issue"
        );
        assert_eq!(
            store.blocked_clauses(&repo(), 7).expect("reads"),
            [format!(
                "<!-- issue-flow: blocked-until encoding=pct issue={REAL} state=done -->"
            )],
            "a clause that named the provisional issue names the real one"
        );
        let jobs: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM review_jobs WHERE issue = ?1",
                [i64::try_from(REAL).expect("fits")],
                |row| row.get(0),
            )
            .expect("reads");
        assert_eq!(jobs, 1);
        assert_eq!(
            store.provisional_real(&repo(), provisional).expect("reads"),
            Some(Some(REAL))
        );
        assert_eq!(
            store.provisional_real_anywhere(provisional).expect("reads"),
            Some(Some(REAL))
        );
    }

    #[test]
    fn rekeying_moves_the_transitions_of_the_issue_with_it() {
        let (_directory, mut store, provisional) = provisional_issue();
        store
            .connection()
            .execute(
                "INSERT INTO transitions (repo_id, issue, at, run_id, from_state, to_state)
                 SELECT id, ?1, ?2, NULL, 'ready', 'in-progress' FROM repos",
                rusqlite::params![i64::try_from(provisional).expect("fits"), NOW],
            )
            .expect("a transition");

        store
            .rekey(&repo(), provisional, REAL, None)
            .expect("rekeys");

        let moved: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM transitions WHERE issue = ?1",
                [i64::try_from(REAL).expect("fits")],
                |row| row.get(0),
            )
            .expect("reads");
        assert_eq!(moved, 1);
    }

    #[test]
    fn a_clause_naming_a_longer_number_that_starts_the_same_is_left_alone() {
        let (_directory, mut store, provisional) = provisional_issue();
        let other = format!("<!-- issue-flow: blocked-until encoding=pct issue={provisional}0 -->");
        store
            .record_blocked_clause(&repo(), 7, &other, NOW)
            .expect("records");
        store
            .rekey(&repo(), provisional, REAL, None)
            .expect("rekeys");
        assert!(
            store
                .blocked_clauses(&repo(), 7)
                .expect("reads")
                .contains(&other)
        );
    }

    #[test]
    fn rekeying_can_mark_the_create_sent_in_the_same_transaction() {
        let (_directory, mut store, provisional) = provisional_issue();
        let seq = store.outbox(&repo()).expect("reads")[0].seq;
        store
            .rekey(&repo(), provisional, REAL, Some(seq))
            .expect("rekeys");
        let row = &store.outbox(&repo()).expect("reads")[0];
        assert_eq!((row.state, row.issue), (OutboxState::Sent, REAL));
    }

    #[test]
    fn rekeying_twice_to_the_same_number_is_a_no_op_and_to_another_is_refused() {
        let (_directory, mut store, provisional) = provisional_issue();
        store
            .rekey(&repo(), provisional, REAL, None)
            .expect("rekeys");
        store
            .rekey(&repo(), provisional, REAL, None)
            .expect("the repeat is idempotent");
        assert_eq!(store.timeline(&repo(), REAL).expect("reads").len(), 1);
        assert!(store.rekey(&repo(), provisional, REAL + 1, None).is_err());
    }

    #[test]
    fn rekeying_refuses_what_is_not_a_provisional_number_or_a_real_one_and_changes_nothing() {
        let (_directory, mut store, provisional) = provisional_issue();
        assert!(store.rekey(&repo(), 7, REAL, None).is_err());
        assert!(
            store.rekey(&repo(), provisional + 100, REAL, None).is_err(),
            "a number the store never allocated"
        );
        assert!(
            store
                .rekey(&repo(), provisional, PROVISIONAL_BASE + 9, None)
                .is_err(),
            "a real number must not itself be provisional"
        );
        assert!(
            store.rekey(&repo(), provisional, 7, None).is_err(),
            "the real number is already another issue"
        );
        assert_eq!(
            store.timeline(&repo(), provisional).expect("reads").len(),
            1
        );
        assert_eq!(
            store.provisional_real(&repo(), provisional).expect("reads"),
            Some(None),
            "a refused rekey recorded the real number anyway"
        );
        assert!(store.timeline(&repo(), 7).expect("reads").is_empty());
    }
}

mod provisional_ledger {
    use super::*;
    use crate::transport::store::{Ledger, Recorded};

    fn context(directory: &std::path::Path) -> crate::transport::Context {
        crate::transport::Context {
            skill_dir: directory.to_path_buf(),
            repo_dir: directory.to_path_buf(),
            config: Vec::new(),
            repo: Some("asanabrial/estigia".to_owned()),
        }
    }

    fn calls(log: &std::path::Path) -> String {
        std::fs::read_to_string(log).unwrap_or_default()
    }

    fn held_provisional(path: &std::path::Path) -> u64 {
        let mut store = Store::open(path).expect("opens");
        let provisional = store.allocate_provisional(&repo()).expect("allocates");
        store
            .import_recorded(
                &repo(),
                provisional,
                &issue(vec![claim_comment()]),
                NOW,
                &Recorded::default(),
            )
            .expect("held");
        provisional
    }

    #[test]
    fn a_provisional_issue_opens_without_asking_github_anything() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = held_provisional(&path);
        let log = directory.path().join("gh.log");
        let bin = crate::test_env::scripted_gh();

        let ledger = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
            Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
        })
        .expect("the provisional ledger opens");

        assert_eq!(calls(&log), "", "a provisional issue reached gh");
        assert!(ledger.is_open());
        assert_eq!(ledger.timeline().expect("reads").len(), 1);
        assert_eq!(
            ledger.state().expect("reads"),
            Some("in-progress".to_owned())
        );
        ledger.set_state("review", NOW).expect("moves");
        ledger.append("a local note", NOW).expect("appends");
        assert_eq!(ledger.state().expect("reads"), Some("review".to_owned()));
        assert_eq!(ledger.timeline().expect("reads").len(), 2);
    }

    type Row = (String, Option<String>, Option<String>, String, String);

    fn transitions(path: &std::path::Path) -> Vec<Row> {
        let connection = rusqlite::Connection::open(path).expect("opens");
        let mut statement = connection
            .prepare("SELECT at, run_id, from_state, to_state, repo_id || '#' || issue FROM transitions ORDER BY seq")
            .expect("prepares");
        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("reads")
            .collect::<Result<_, _>>()
            .expect("rows")
    }

    #[test]
    fn a_state_change_is_recorded_with_when_who_held_the_issue_and_where_it_came_from() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = held_provisional(&path);
        let ledger = Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
            .expect("the provisional ledger opens");

        ledger.set_state("review", NOW).expect("moves");
        ledger.set_state("done", NOW).expect("moves again");

        assert_eq!(
            transitions(&path),
            [
                (
                    NOW.to_owned(),
                    Some(RUN.to_owned()),
                    Some("in-progress".to_owned()),
                    "review".to_owned(),
                    format!("1#{provisional}")
                ),
                (
                    NOW.to_owned(),
                    Some(RUN.to_owned()),
                    Some("review".to_owned()),
                    "done".to_owned(),
                    format!("1#{provisional}")
                ),
            ]
        );
    }

    #[test]
    fn setting_the_state_it_already_has_records_nothing() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = held_provisional(&path);
        let ledger = Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
            .expect("the provisional ledger opens");

        ledger.set_state("in-progress", NOW).expect("settles");

        assert_eq!(transitions(&path), []);
    }

    #[test]
    fn a_history_write_that_fails_leaves_the_state_where_it_was() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = held_provisional(&path);
        let ledger = Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
            .expect("the provisional ledger opens");
        rusqlite::Connection::open(&path)
            .expect("opens")
            .execute_batch("DROP TABLE transitions")
            .expect("the history is made unwritable");

        let refused = ledger.set_state("review", NOW);

        assert!(refused.is_err(), "a state moved with no record of it");
        assert_eq!(
            ledger.state().expect("reads"),
            Some("in-progress".to_owned())
        );
    }

    #[test]
    fn a_provisional_number_nothing_allocated_is_refused_without_asking_github() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let log = directory.path().join("gh.log");
        let bin = crate::test_env::scripted_gh();
        let unknown = crate::transport::outbox::PROVISIONAL_BASE + 40;

        let refused = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
            Ledger::open_at(&path, &context(directory.path()), unknown, NOW)
        });

        assert!(
            matches!(refused, Err(Failure::Read(_))),
            "{:?}",
            refused.err()
        );
        assert_eq!(calls(&log), "");
    }

    #[test]
    fn a_provisional_number_that_was_allocated_but_never_recorded_is_refused() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = Store::open(&path)
            .expect("opens")
            .allocate_provisional(&repo())
            .expect("allocates");
        let log = directory.path().join("gh.log");
        let bin = crate::test_env::scripted_gh();

        let refused = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
            Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
        });

        assert!(
            matches!(refused, Err(Failure::Read(_))),
            "{:?}",
            refused.err()
        );
        assert_eq!(
            calls(&log),
            "",
            "a number only this machine invented reached gh"
        );
    }

    #[test]
    fn a_rekeyed_provisional_number_opens_the_real_issue() {
        let directory = tempfile::tempdir().expect("a directory");
        let path = directory.path().join("state.db");
        let provisional = held_provisional(&path);
        Store::open(&path)
            .expect("opens")
            .rekey(&repo(), provisional, 55, None)
            .expect("rekeys");
        let log = directory.path().join("gh.log");
        let bin = crate::test_env::scripted_gh();

        let _ = crate::test_env::with_logging_gh(bin.path(), "", &log, || {
            Ledger::open_at(&path, &context(directory.path()), provisional, NOW)
        });

        let asked = calls(&log);
        assert!(asked.contains("issues/55"), "{asked}");
        assert!(!asked.contains(&provisional.to_string()), "{asked}");
    }

    #[test]
    fn a_lease_is_kept_by_a_live_holder_and_taken_from_a_dead_or_expired_one() {
        let (_directory, mut store) = open();

        assert_eq!(
            store
                .take_outbox_lease(&repo(), 10, 100, 700, |_| true)
                .expect("taken"),
            None
        );
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 11, 200, 800, |_| true)
                .expect("asked"),
            Some(10),
            "a live holder was robbed"
        );
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 10, 300, 900, |_| true)
                .expect("renewed"),
            None,
            "the holder could not renew"
        );
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 11, 400, 1000, |_| false)
                .expect("taken"),
            None,
            "a dead holder kept the lease"
        );
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 12, 1000, 1600, |_| true)
                .expect("taken"),
            None,
            "an expired lease kept the queue"
        );
    }

    #[test]
    fn a_lease_of_one_repository_does_not_keep_another_repository() {
        let (_directory, mut store) = open();
        let other = Repo {
            name: "other".to_owned(),
            ..repo()
        };
        store
            .take_outbox_lease(&repo(), 10, 100, 700, |_| true)
            .expect("taken");

        assert_eq!(
            store
                .take_outbox_lease(&other, 11, 200, 800, |_| true)
                .expect("asked"),
            None
        );
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 11, 200, 800, |_| true)
                .expect("asked"),
            Some(10)
        );
    }

    #[test]
    fn a_lease_is_given_up_only_by_the_process_that_holds_it() {
        let (_directory, mut store) = open();
        store
            .take_outbox_lease(&repo(), 10, 100, 700, |_| true)
            .expect("taken");

        store.drop_outbox_lease(&repo(), 11).expect("dropped");
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 12, 200, 800, |_| true)
                .expect("asked"),
            Some(10)
        );
        store.drop_outbox_lease(&repo(), 10).expect("dropped");
        assert_eq!(
            store
                .take_outbox_lease(&repo(), 12, 200, 800, |_| true)
                .expect("taken"),
            None
        );
    }

    #[test]
    fn the_unsent_count_ignores_sent_writes() {
        let (_directory, mut store) = open();
        assert_eq!(store.unsent_outbox().expect("counts"), 0);
        let seq = store
            .enqueue(
                &repo(),
                5,
                crate::transport::store::OutboxKind::Comment,
                &serde_json::json!({}),
                "a",
                NOW,
            )
            .expect("queued");
        assert_eq!(store.unsent_outbox().expect("counts"), 1);
        store
            .mark_outbox(seq, crate::transport::store::OutboxState::Sent, None, NOW)
            .expect("marked");
        assert_eq!(store.unsent_outbox().expect("counts"), 0);
    }
}

fn seen(store: &Store, run_id: &str, now: u64) {
    store
        .session_seen(
            &super::SessionSeen {
                run_id,
                runtime: "pi",
                session_id: "host-id",
                repo_dir: Some("/work/repo"),
                wake: None,
                pane: None,
            },
            now,
        )
        .expect("the session is recorded");
}

#[test]
fn a_session_seen_again_keeps_its_start_and_moves_its_last_seen() {
    let (_directory, store) = open();
    seen(&store, "pi-aaaa", 100);
    seen(&store, "pi-aaaa", 250);

    let rows = store.sessions().expect("the sessions read");
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].started_at, rows[0].last_seen), (100, 250));
    assert_eq!(rows[0].runtime, "pi");
    assert_eq!(rows[0].repo_dir.as_deref(), Some("/work/repo"));
    assert_eq!(rows[0].ended_at, None);
}

#[test]
fn a_session_is_given_a_short_name_from_its_repository_and_keeps_it() {
    let (_directory, store) = open();
    seen(&store, "pi-aaaa", 100);
    seen(&store, "pi-bbbb", 110);
    seen(&store, "pi-aaaa", 120);
    seen(&store, "claude-cccc", 130);

    let mut names: Vec<String> = store
        .sessions()
        .expect("the sessions read")
        .into_iter()
        .filter_map(|row| row.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["repo-1", "repo-2", "repo-3"]);
    let again = store.sessions().expect("the sessions read");
    let aaaa = again
        .iter()
        .find(|row| row.run_id == "pi-aaaa")
        .expect("the restarted row");
    assert_eq!(aaaa.name.as_deref(), Some("repo-1"), "a restart renamed it");
}

#[test]
fn a_registration_refreshes_the_build_and_the_wake_it_names() {
    let (_directory, store) = open();
    store
        .session_seen(
            &super::SessionSeen {
                run_id: "pi-aaaa",
                runtime: "pi",
                session_id: "host-id",
                repo_dir: Some("/work/repo"),
                wake: Some("poll"),
                pane: None,
            },
            100,
        )
        .expect("the session is recorded");
    let first = &store.sessions().expect("the sessions read")[0];
    assert_eq!(first.wake.as_deref(), Some("poll"));
    assert_eq!(first.build.as_deref(), Some(crate::build_info::VERSION));

    store
        .session_seen(
            &super::SessionSeen {
                run_id: "pi-aaaa",
                runtime: "pi",
                session_id: "host-id",
                repo_dir: Some("/work/repo"),
                wake: None,
                pane: None,
            },
            200,
        )
        .expect("the session is recorded again");
    let again = &store.sessions().expect("the sessions read")[0];
    assert_eq!(
        again.wake, None,
        "an old reachability survived a registration"
    );
}

#[test]
fn an_ended_session_stays_listed_and_a_restart_reopens_it() {
    let (_directory, store) = open();
    seen(&store, "pi-aaaa", 100);
    store
        .session_ended("pi-aaaa", Some("quit"), 300)
        .expect("the end is recorded");

    let rows = store.sessions().expect("the sessions read");
    assert_eq!(rows[0].ended_at, Some(300));
    assert_eq!(rows[0].end_reason.as_deref(), Some("quit"));

    seen(&store, "pi-aaaa", 400);
    let rows = store.sessions().expect("the sessions read");
    assert_eq!((rows[0].ended_at, rows[0].end_reason.clone()), (None, None));
}

#[test]
fn ending_a_session_nobody_registered_invents_no_row() {
    let (_directory, store) = open();
    store
        .session_ended("pi-ghost", None, 300)
        .expect("the end is a no-op");
    assert!(store.sessions().expect("the sessions read").is_empty());
}

#[test]
fn the_sessions_read_does_not_create_the_store() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let path = directory.path().join("state.db");
    assert!(
        super::open_for_read(&path)
            .expect("absent is an answer")
            .is_none()
    );
    assert!(!path.exists());
}

fn mail(store: &Store, from: Option<&str>, to: &str, body: &str, now: u64) -> i64 {
    store
        .message_sent(
            &super::MessageSent {
                sender_run_id: from,
                recipient_run_id: to,
                body,
            },
            now,
        )
        .expect("the message is recorded")
}

#[test]
fn migration_eleven_creates_the_messages_table() {
    let (_directory, store) = open();
    let columns: Vec<String> = store
        .connection()
        .prepare("SELECT name FROM pragma_table_info('messages')")
        .expect("prepares")
        .query_map([], |row| row.get(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    for wanted in [
        "id",
        "sender_run_id",
        "recipient_run_id",
        "body",
        "created_at",
        "delivered_at",
    ] {
        assert!(columns.iter().any(|name| name == wanted), "{columns:?}");
    }
}

#[test]
fn a_message_is_taken_once_in_the_order_it_was_sent_and_stamped_delivered() {
    let (_directory, store) = open();
    mail(&store, Some("claude-aaaa"), "pi-bbbb", "first", 100);
    mail(&store, None, "pi-bbbb", "second", 110);
    mail(&store, None, "pi-cccc", "someone else's", 120);

    let taken = store.messages_taken("pi-bbbb", 200).expect("taken");
    let bodies: Vec<&str> = taken.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, ["first", "second"]);
    assert_eq!(taken[0].sender_run_id.as_deref(), Some("claude-aaaa"));
    assert_eq!(taken[1].sender_run_id, None);

    assert!(
        store
            .messages_taken("pi-bbbb", 300)
            .expect("again")
            .is_empty()
    );
    let stamped: Vec<Option<i64>> = store
        .connection()
        .prepare("SELECT delivered_at FROM messages ORDER BY id")
        .expect("prepares")
        .query_map([], |row| row.get(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    assert_eq!(stamped, [Some(200), Some(200), None]);
}

#[test]
fn pending_messages_are_read_without_being_stamped_and_taken_only_up_to_the_id_read() {
    let (_directory, store) = open();
    mail(&store, None, "pi-bbbb", "first", 100);
    mail(&store, None, "pi-bbbb", "second", 110);

    let pending = store.messages_pending("pi-bbbb").expect("pending");
    assert_eq!(pending.len(), 2);
    assert_eq!(
        store.messages_pending("pi-bbbb").expect("again").len(),
        2,
        "reading stamped a message"
    );

    mail(&store, None, "pi-bbbb", "third", 120);
    let taken = store
        .messages_taken_through("pi-bbbb", 200, pending[1].id)
        .expect("taken");
    let bodies: Vec<&str> = taken.iter().map(|m| m.body.as_str()).collect();
    assert_eq!(bodies, ["first", "second"]);
    let left = store.messages_pending("pi-bbbb").expect("left");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].body, "third");
}

#[test]
fn the_counts_name_what_waits_for_a_recipient_and_what_a_sender_still_has_undelivered() {
    let (_directory, store) = open();
    mail(&store, Some("claude-aaaa"), "pi-bbbb", "one", 100);
    mail(&store, Some("claude-aaaa"), "pi-bbbb", "two", 101);
    mail(&store, Some("claude-aaaa"), "pi-cccc", "three", 102);
    store.messages_taken("pi-cccc", 150).expect("taken");

    let waiting = store.messages_waiting_for().expect("counts");
    assert_eq!(waiting.get("pi-bbbb"), Some(&2));
    assert_eq!(waiting.get("pi-cccc"), None);
    let undelivered = store.messages_undelivered_from().expect("counts");
    assert_eq!(undelivered.get("claude-aaaa"), Some(&2));
}

#[test]
fn a_busy_open_names_sqlites_code_and_how_long_it_waited() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("state.db");
    let holder = rusqlite::Connection::open(&path).expect("the holder opens");
    holder
        .execute_batch("BEGIN EXCLUSIVE;")
        .expect("the holder takes the lock");

    let wait = std::time::Duration::from_millis(300);
    let Failure::Read(message) = Store::open_waiting(&path, wait)
        .err()
        .expect("a locked file is refused")
    else {
        panic!("a locked file is a failed read");
    };

    assert!(
        message.starts_with("the local store could not "),
        "{message}"
    );
    assert!(message.contains("database is locked"), "{message}");
    let tail = message
        .rsplit_once(" (sqlite 5, after ")
        .unwrap_or_else(|| panic!("no code and elapsed in {message:?}"))
        .1;
    let elapsed: u128 = tail
        .strip_suffix(" ms)")
        .unwrap_or_else(|| panic!("elapsed is not closed in {message:?}"))
        .parse()
        .expect("elapsed is whole milliseconds");
    assert!(elapsed >= wait.as_millis(), "{elapsed} ms in {message:?}");
    holder.execute_batch("ROLLBACK;").expect("releases");
}
