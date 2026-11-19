use super::*;

#[test]
fn concurrent_writers_do_not_lose_a_pointer_update() {
    const WRITERS: usize = 16;
    let root = tempfile::tempdir().expect("a state root");
    store(root.path(), &Run::new("claude-crowd".to_owned())).expect("the pointer writes");
    let start = std::sync::Barrier::new(WRITERS);
    let landed = std::thread::scope(|scope| {
        let writers: Vec<_> = (0..WRITERS)
            .map(|writer| {
                let (start, root) = (&start, root.path());
                scope.spawn(move || {
                    start.wait();
                    updated(root, "claude-crowd", |run| {
                        run.roles.insert(format!("role-{writer:02}"));
                    })
                    .1
                })
            })
            .collect();
        writers
            .into_iter()
            .map(|writer| writer.join().expect("the writer finishes"))
            .collect::<Vec<bool>>()
    });
    assert!(landed.iter().all(|landed| *landed), "{landed:?}");
    let roles = load(root.path(), "claude-crowd").roles;
    assert_eq!(roles.len(), WRITERS, "{roles:?}");
}

#[test]
fn a_child_run_id_is_stable_minted_and_distinct_per_issue_and_branch() {
    use crate::identity::Identity::Number;
    let first = child_run_id("claude-abcd1234", &Number(1), "a");
    assert_eq!(first, child_run_id("claude-abcd1234", &Number(1), "a"));
    assert_ne!(first, child_run_id("claude-abcd1234", &Number(2), "a"));
    assert_ne!(first, child_run_id("claude-abcd1234", &Number(1), "b"));
    assert_ne!(first, child_run_id("claude-ffff0000", &Number(1), "a"));
    assert!(matches!(
        run_id_shape(&first),
        RunIdShape::Named { runtime: "claude" }
    ));
}

#[test]
fn a_pointer_written_under_the_old_receipt_key_still_loads_and_is_rewritten_under_the_new() {
    let old = serde_json::json!({
        "run_id": "claude-old-key",
        "issue": 20,
        "state": "review",
        "review_receipt": {
            "epoch": "a".repeat(32),
            "pr": 54,
            "head": "b".repeat(40),
            "base": "c".repeat(40),
            "digest": "d".repeat(64),
        },
    })
    .to_string();

    let run: Run = serde_json::from_str(&old).expect("a pointer with the old key loads");
    let transaction = run.review_transaction.clone().expect("the old key is read");
    assert_eq!((transaction.pr, transaction.epoch.len()), (54, 32));

    let rewritten = serde_json::to_value(&run).expect("the pointer serialises");
    assert!(rewritten.get("review_receipt").is_none(), "{rewritten}");
    assert_eq!(rewritten["review_transaction"]["pr"], 54, "{rewritten}");
}

#[test]
fn a_legacy_reviewed_head_remains_readable_but_is_not_a_complete_receipt() {
    let legacy = serde_json::json!({
        "run_id": "claude-legacy",
        "issue": 20,
        "state": "review",
        "repo_dir": serde_json::Value::Null,
        "verified_at": serde_json::Value::Null,
        "reviewed_head": "a".repeat(40),
    })
    .to_string();
    let run: Run = serde_json::from_str(&legacy).expect("the legacy pointer remains readable");
    assert_eq!(run.reviewed_head.as_deref(), Some("a".repeat(40).as_str()));
    assert_eq!(run.review_transaction, None);
}

#[test]
fn a_run_whose_own_pointer_would_not_parse_does_not_write_one_back() {
    let dir = tempfile::tempdir().expect("a temporary root");
    let root = dir.path();
    let path = pointer_path(root, "claude-a");
    std::fs::create_dir_all(root).expect("the state directory");
    let wreckage = r#"{"run_id": "claude-a", "issue": 7, "revi"#;
    fs::write(&path, wreckage).expect("a pointer that will not parse");

    let run = load(root, "claude-a");
    assert!(run.unreadable, "the corpus is wrong: these bytes parse");
    assert_eq!(run.issue, None, "an unreadable pointer answers no issue");

    // What is on disk when the store happens, and it has to be the one
    // combination only this guard stops. A pointer that is still unreadable
    // is caught by the on-disk check, and a readable one at a higher
    // revision by the stale-writer check — so the case that isolates this
    // one is a **readable pointer at revision zero**, which is exactly what
    // a build from before the `revision` field left behind: the field is
    // `#[serde(default)]`, so those load as zero.
    //
    // It is also the sequence the `unreadable` field's own doc names: a
    // release changes this struct, every pointer the last one wrote stops
    // parsing, and something puts an older-format one back.
    let older = r#"{"run_id": "claude-a", "issue": null, "state": null, "repo_dir": null,
                        "verified_at": null}"#;
    fs::write(&path, older).expect("an older, readable pointer");
    assert_eq!(load(root, "claude-a").revision, 0, "the fixture is wrong");

    assert_eq!(
        store(root, &run),
        Ok(false),
        "a run that knows nothing wrote a pointer saying it holds nothing"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("still there"),
        older,
        "a run loaded from bytes nobody could parse wrote a readable pointer swearing to none"
    );
}

#[test]
fn a_pointer_that_went_unreadable_is_not_written_over() {
    let dir = tempfile::tempdir().expect("a temporary root");
    let root = dir.path();

    let mut held = Run::new("claude-a".to_owned());
    held.issue = Some(crate::identity::Identity::Number(7));
    store(root, &held).expect("the first store");
    // What the hook is still carrying: readable, and behind.
    let stale = load(root, "claude-a");
    assert!(!stale.unreadable && stale.issue == Some(crate::identity::Identity::Number(7)));

    // The release is interrupted part-way through putting it down.
    let path = pointer_path(root, "claude-a");
    let wreckage = r#"{"run_id": "claude-a", "issue": null, "revi"#;
    fs::write(&path, wreckage).expect("a half-written pointer");
    assert!(
        load(root, "claude-a").unreadable,
        "the corpus is wrong: these bytes parse"
    );

    assert_eq!(
        store(root, &stale),
        Ok(false),
        "a store reported a write over a pointer it could not read"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("still there"),
        wreckage,
        "the corruption was replaced, and with it the only sign of it"
    );

    // And `update` stops rather than spinning: it reloads on a dropped
    // write, sees the unreadable pointer, and leaves it alone.
    let after = update(root, "claude-a", |run| {
        run.issue = Some(crate::identity::Identity::Number(9))
    });
    assert!(after.unreadable, "update walked past an unreadable pointer");
    assert_eq!(
        fs::read_to_string(&path).expect("still there"),
        wreckage,
        "update wrote over it after store had refused to"
    );
}

#[test]
fn an_io_unreadable_pointer_is_an_unknown_and_is_never_written_over() {
    // At the loader rather than at the gate: a pointer the
    // filesystem refuses to read — not one that is absent — must load as
    // *unreadable*, never as a fresh run. The fresh-run reading is the one
    // `gate` answers `Outside(NothingSworn)` for, and that is the silent
    // disarming the issue is about: the tracker still names the run as
    // holder while its writes pass through ungated.
    let dir = tempfile::tempdir().expect("a temporary root");
    let root = dir.path();
    let path = pointer_path(root, "claude-dirblock");
    std::fs::create_dir_all(&path).expect("a directory where the pointer should be");

    let run = load(root, "claude-dirblock");
    assert!(
        run.unreadable,
        "an unreadable pointer was loaded as a run that swore nothing"
    );
    assert!(
        run.unreadable_reason
            .as_deref()
            .is_some_and(|why| why.contains("claude-dirblock.json")),
        "the reason does not name the pointer it could not read: {:?}",
        run.unreadable_reason
    );
    assert_eq!(run.issue, None, "an unreadable pointer answers no issue");

    // And `store` leaves the unreadable thing alone: the guard that stops a
    // run whose own copy came from unreadable bytes must also stop a run
    // whose pointer cannot be opened, because writing a fresh pointer over
    // it would discard the only sign that the run's record was lost.
    let mut stale = Run::new("claude-dirblock".to_owned());
    stale.issue = Some(crate::identity::Identity::Number(7));
    assert_eq!(
        store(root, &stale),
        Ok(false),
        "a store wrote a pointer over a record that could not be read"
    );
    assert!(
        path.is_dir(),
        "the unreadable thing was replaced, and with it the only sign of it"
    );
}

#[test]
fn a_state_root_that_is_not_a_directory_is_an_unknown_and_not_an_empty_one() {
    // The spec for this issue asked for a pointer whose parent is a file,
    // expecting the read to fail with something that is not `NotFound`.
    // Measured on Windows it is exactly `NotFound` — `ERROR_PATH_NOT_FOUND`,
    // os error 3 — so the arm that answers *this run swore nothing* is the
    // one it lands in, for every run at once, and the gate stands every one
    // of them aside. On Linux the same shape answers `NotADirectory`, which
    // is why reading the error kind alone cannot decide it. Absence is
    // confirmed against the root instead.
    let dir = tempfile::tempdir().expect("a temporary root");
    let root = dir.path().join("runs");
    std::fs::write(&root, "a file where the pointers should live")
        .expect("something that is not a directory");

    let run = load(&root, "claude-notadir");
    assert!(
        run.unreadable,
        "a state root that cannot hold a pointer was read as a run that swore nothing"
    );
    assert!(
        run.unreadable_reason
            .as_deref()
            .is_some_and(|why| why.contains("runs")),
        "the reason names no root, so nobody can find what to look at: {:?}",
        run.unreadable_reason
    );

    // The ordinary case is the other side of the same arm and must stay
    // exactly as it was: nothing claimed yet on this machine, so the root
    // is simply not there.
    let fresh = load(&dir.path().join("never-created"), "claude-firstrun");
    assert!(
        !fresh.unreadable && fresh.issue.is_none(),
        "a machine with no state yet was refused instead of being read as unsworn"
    );
}

#[test]
fn a_run_id_is_stable_for_one_session() {
    let first = run_id(DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345");
    let second = run_id(DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345");
    assert_eq!(first, second);
    // The readable prefix is still the readable prefix: a person reading a
    // run id in an issue comment is why it is there.
    assert!(first.starts_with("claude-0198fe1c"), "{first}");
    // Pinned, because this value is written to disk and read back by a
    // later build. A hasher free to change between releases would deny
    // every write for a run in flight after a toolchain upgrade.
    assert_eq!(first, "claude-0198fe1c69bc2b12");
}

#[test]
fn two_sessions_do_not_collide() {
    assert_ne!(
        run_id(DEFAULT_RUNTIME, "aaaaaaaa-1111"),
        run_id(DEFAULT_RUNTIME, "bbbbbbbb-2222")
    );
    for (first, second) in [
        (
            "session_01KvQFLvJbDRzAWzq6GCy8B7",
            "session_01ZZZZZZZZZZZZZZZZZZZZZZ",
        ),
        // Same twenty characters, one apart at the end.
        ("thread-0001112223334445", "thread-0001112223334446"),
        // A UUID whose first block repeats, which is not exotic: only the
        // first eight hex digits were ever read.
        (
            "4d1f0f7a-57a6-4911-a1bc-25cbfaa16cd0",
            "4d1f0f7a-99ff-4000-bbbb-000000000000",
        ),
    ] {
        assert_ne!(
            run_id(DEFAULT_RUNTIME, first),
            run_id(DEFAULT_RUNTIME, second),
            "two sessions of one host mint one identity, so one inherits the other's oath"
        );
    }
}

#[test]
fn a_second_session_does_not_inherit_the_first_ones_oath() {
    let home = tempfile::tempdir().expect("a temporary home");
    let root = state_root(Some(home.path())).expect("a state root");

    let mine = run_id(DEFAULT_RUNTIME, "session_01AAAAAAAAAAAAAAAAAAAAAA");
    let mut run = Run::new(mine.clone());
    run.issue = Some(crate::identity::Identity::Number(42));
    store(&root, &run).expect("the pointer is written");

    let theirs = run_id(DEFAULT_RUNTIME, "session_01BBBBBBBBBBBBBBBBBBBBBB");
    assert_ne!(mine, theirs, "two sessions minted one identity");
    assert_eq!(
        load(&root, &theirs).issue,
        None,
        "another session's run read this one's claim as its own"
    );
    // And this run still finds its own.
    assert_eq!(
        load(&root, &mine).issue,
        Some(crate::identity::Identity::Number(42))
    );
}

#[test]
fn a_session_with_no_id_still_mints_something_usable() {
    // An agent that sends no session_id must not produce a run-id of
    // `claude-`, which would collide with every other such run.
    assert_eq!(run_id(DEFAULT_RUNTIME, ""), "claude-unknown");
    assert_eq!(run_id(DEFAULT_RUNTIME, "///"), "claude-unknown");
}

#[test]
fn runtime_of_reads_back_what_run_id_minted() {
    assert_eq!(runtime_of(&run_id("codex", "session-abc")), "codex");
    assert_eq!(runtime_of(&run_id(DEFAULT_RUNTIME, "")), DEFAULT_RUNTIME);
}

#[test]
fn runtime_of_falls_back_to_the_whole_id_when_there_is_no_dash() {
    assert_eq!(runtime_of("myrun"), "myrun");
}

#[test]
fn runtime_of_keeps_a_dashed_runtime_whole() {
    assert_eq!(runtime_of("claude-code-3edc7a52"), "claude-code");
    assert_eq!(
        runtime_of(&run_id("claude-code", "a-session")),
        "claude-code"
    );
}

/// The predicate is crossed against the minter itself, so it cannot drift
/// into refusing an identity this crate produces — the failure that would
/// be worse than the one it closes.
#[test]
fn run_id_shape_reads_back_what_run_id_minted() {
    for (runtime, session) in [
        (DEFAULT_RUNTIME, "0198fe1c-2b7a-7d4e-9f01-abcdef012345"),
        ("codex", "session-abc"),
        ("claude-code", "a-session"),
        ("opencode", "ses_01KvQFLvJbDRzAWzq6GCy8B7"),
    ] {
        let id = run_id(runtime, session);
        assert_eq!(
            run_id_shape(&id),
            RunIdShape::Named { runtime },
            "{id} is an id the minter produced and the predicate refuses"
        );
    }
    assert_eq!(
        run_id_shape(&run_id("codex", "")),
        RunIdShape::Nameless { runtime: "codex" }
    );
    for session in [
        "",
        "///",
        "a",
        "abcdefghijklmnop",
        "SESSION-UPPER",
        "session_01AAAAAAAAAAAAAAAAAAAAAA",
    ] {
        let id = run_id("codex", session);
        assert!(
            matches!(
                run_id_shape(&id),
                RunIdShape::Named { .. } | RunIdShape::Nameless { .. }
            ),
            "the minter produced an id the predicate refuses: {id}"
        );
    }
}

/// `<runtime>-<session-prefix>` from the contract's
/// own instruction, on a host whose session id carries a dash. The
/// last-dash split reads `codex-01k4-9f2a` as runtime `codex-01k4`, a
/// well-formed component, so the alphabet check admits it and `dev:codex-01k4`
/// is what gets minted — invisible to the `gh issue list --label dev:codex`
/// query the label exists for. Refused rather than derived: nothing can
/// tell a dashed runtime from a suffix that carries a dash, so a derivation
/// would guess, and the suffix is not a shape the minter produces.
#[test]
fn a_suffix_that_carries_a_dash_is_not_a_shape_run_id_mints() {
    assert_eq!(
        run_id_shape("codex-01k4-9f2a"),
        RunIdShape::Unminted(Unminted::Suffix)
    );
    // What the split would have projected, so the recorded decision is
    // pinned to the value that made it necessary.
    assert_eq!(runtime_of("codex-01k4-9f2a"), "codex-01k4");
    assert_eq!(
        run_id_shape("claude-3edc7a52-5e11"),
        RunIdShape::Unminted(Unminted::Suffix)
    );
    assert_eq!(
        run_id_shape("claude-3edc7a52"),
        RunIdShape::Named { runtime: "claude" }
    );
}

/// These were closed one at a time, each round complete for its own
/// spelling. One answer refuses all of them, and names which part is not
/// the minted shape.
#[test]
fn the_spellings_48_closed_one_at_a_time_are_one_class_here() {
    assert_eq!(
        run_id_shape("myrun"),
        RunIdShape::Unminted(Unminted::NoRuntime)
    );
    assert_eq!(
        run_id_shape("abcd1234"),
        RunIdShape::Unminted(Unminted::NoRuntime)
    );
    for runtime in ["-abcd1234", "My Runtime-abcd1234", "a/b-cd"] {
        assert_eq!(
            run_id_shape(runtime),
            RunIdShape::Unminted(Unminted::Runtime),
            "{runtime}"
        );
    }
    for suffix in [
        "claude-a",
        "claude-holder",
        "claude-elsewhere",
        "claude-torn0",
    ] {
        assert_eq!(
            run_id_shape(suffix),
            RunIdShape::Unminted(Unminted::Suffix),
            "{suffix}"
        );
    }
    assert_eq!(
        run_id_shape("claude-unknown"),
        RunIdShape::Nameless { runtime: "claude" }
    );
    for minted in [
        "claude-0198fe1c",
        "claude-0198fe1c3f2a9b41",
        "claude-code-60fabae1",
        "codex-abcd1234",
    ] {
        assert!(
            matches!(run_id_shape(minted), RunIdShape::Named { .. }),
            "{minted} is a shape the minter produces and the predicate refuses"
        );
    }
}

/// `create`'s declared runtime and the one derived from a run id are held
/// by the same function, or the label it mints and the label a claim mints
/// could disagree about what a runtime is.
#[test]
fn runtime_component_reads_a_declared_runtime_the_way_a_derived_one_is_read() {
    for runtime in ["codex", "claude-code", "opencode", "claude"] {
        assert!(runtime_component(runtime).is_ok(), "{runtime}");
    }
    for runtime in ["", "My Runtime", "a/b", "codex 01", "Codex"] {
        assert_eq!(
            runtime_component(runtime),
            Err(Unminted::Runtime),
            "{runtime}"
        );
    }
}

#[test]
fn a_pointer_survives_a_round_trip() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.state = Some("in-progress".to_owned());
    run.mark_verified();
    store(root.path(), &run).expect("the pointer writes");

    // Everything survives except the revision, which store moves on purpose:
    // it counts writes rather than describing the run, and a pointer that
    // came back with the number it went in with would be one no stale writer
    // could ever be told apart from.
    let read = load(root.path(), "claude-abcd1234");
    assert_eq!(read.revision, run.revision + 1);
    assert_eq!(
        Run {
            revision: run.revision,
            ..read
        },
        run
    );
}

#[test]
fn a_missing_pointer_is_a_run_that_has_sworn_nothing() {
    let root = tempfile::tempdir().expect("a temporary root");
    let run = load(root.path(), "claude-nothinghere");
    assert_eq!(run.issue, None);
    assert!(!run.within_window(Duration::from_secs(600)));
}

#[test]
fn a_pointer_belonging_to_another_run_is_not_read_as_ours() {
    // The file name is derived from the run-id, so this should be
    // unreachable — which is exactly why it is worth pinning. A pointer
    // that answers for the wrong run would hand one session another's
    // claim.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut theirs = Run::new("claude-theirs00".to_owned());
    theirs.issue = Some(crate::identity::Identity::Number(99));
    store(root.path(), &theirs).expect("the pointer writes");
    fs::rename(
        root.path().join("claude-theirs00.json"),
        root.path().join("claude-ours0000.json"),
    )
    .expect("rename");

    let ours = load(root.path(), "claude-ours0000");
    assert_eq!(ours.issue, None, "another run's claim was adopted");
}

#[test]
fn every_decision_survives_the_ledger_passing_its_cap_while_runs_are_writing() {
    // The ledger is the only evidence of what the gate decided, and every
    // run on the machine appends to it. It used to be one file rotated by
    // rename, and two runs crossing the cap together lost the history one of
    // them had just set aside — 55 of 160 decisions survived on macOS. The
    // store is one transaction per append, so under the same writers every
    // decision must come back. The wait is far past `APPEND_WAIT`: that bound
    // drops a decision on a starved machine by design, and this test measures
    // the cap, not the bound.
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");

    let runs = 8;
    let each = 20;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(runs));
    std::thread::scope(|scope| {
        for run in 0..runs {
            let state = state.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                for n in 0..each {
                    append_waiting(
                        &state,
                        Record::Decision,
                        &serde_json::json!({
                            "at": 1,
                            "run_id": format!("claude-{run}"),
                            "verdict": "new",
                            "n": n,
                        }),
                        Duration::from_secs(600),
                    );
                }
            });
        }
    });

    let records = decision_records(&state);
    let survived = records
        .iter()
        .filter(|record| record.get("verdict").and_then(Value::as_str) == Some("new"))
        .count();
    assert_eq!(
        survived,
        runs * each,
        "only {survived} of {} decisions came back, and the store is meant to keep every one",
        runs * each
    );
}

#[test]
fn record_stores_a_decision_under_the_decision_kind_and_a_role_under_the_role_kind() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");

    record(&state, &serde_json::json!({ "at": 1, "verdict": "kept" }));
    assert_eq!(
        decision_records(&state),
        vec![serde_json::json!({ "at": 1, "verdict": "kept" })]
    );
    assert!(
        role_records(&state).is_empty(),
        "a decision was filed as a role"
    );

    record_role(
        &state,
        &serde_json::json!({ "at": 2, "agent_type": "seen" }),
    );
    assert_eq!(decision_records(&state).len(), 1);
    assert_eq!(
        role_records(&state),
        vec![serde_json::json!({ "at": 2, "agent_type": "seen" })]
    );
}

#[test]
fn the_age_prune_keeps_recent_records_and_runs_at_most_once_a_day() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let now = now_seconds().expect("a clock");
    let day = 24 * 60 * 60;
    let old = now - 31 * day;
    let recent = now - 29 * day;
    record(
        &state,
        &serde_json::json!({ "at": old, "verdict": "stale" }),
    );
    record(
        &state,
        &serde_json::json!({ "at": recent, "verdict": "recent" }),
    );
    record_role(
        &state,
        &serde_json::json!({ "at": old, "agent_type": "gone" }),
    );

    maintain(&state);

    let records = decision_records(&state);
    assert_eq!(records.len(), 1, "the prune kept the wrong decisions");
    assert_eq!(
        records[0].get("verdict").and_then(Value::as_str),
        Some("recent")
    );
    assert!(
        role_records(&state).is_empty(),
        "the prune kept a stale role"
    );

    // Inside the window the marker holds it back: a record old enough to
    // prune that arrives now stays, because the interval has not elapsed.
    record(&state, &serde_json::json!({ "at": old, "verdict": "late" }));
    maintain(&state);
    assert_eq!(
        decision_records(&state).len(),
        2,
        "a second prune ran inside the interval"
    );

    // Past the interval it runs again and takes the stale record.
    let store = Store::open(&crate::transport::store::beside(&state)).expect("the store");
    let elapsed = now - 25 * 60 * 60;
    store
        .set_meta(LAST_PRUNE, &elapsed.to_string())
        .expect("the marker writes");
    maintain(&state);
    assert_eq!(
        decision_records(&state).len(),
        1,
        "the prune did not run once the interval elapsed"
    );
}

#[test]
fn maintain_caps_one_repositorys_decisions_while_every_row_is_young() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let now = now_seconds().expect("a clock");
    let entries: Vec<Value> = (0..DECISIONS_PER_REPOSITORY + 3)
        .map(|n| {
            serde_json::json!({
                "at": now,
                "verdict": "allow",
                "repo": "github.com/a/b",
                "n": n,
            })
        })
        .collect();
    Store::open(&crate::transport::store::beside(&state))
        .expect("the store")
        .import_records(
            "cap-wiring",
            &[(crate::transport::store::Record::Decision, entries)],
        )
        .expect("the fixture records");

    maintain(&state);

    let kept = decision_records(&state);
    assert_eq!(
        kept.len() as u64,
        DECISIONS_PER_REPOSITORY,
        "the daily pass did not cap one repository's young decisions"
    );
    assert_eq!(
        kept.iter()
            .filter_map(|record| record.get("n").and_then(Value::as_u64))
            .min(),
        Some(3),
        "the daily pass kept a different end of the repository"
    );
}

#[test]
fn maintain_bounds_the_job_directories() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let jobs = root.path().join("jobs");
    let old = jobs.join("checks-old");
    std::fs::create_dir_all(&old).expect("a job directory");
    let now = now_seconds().expect("a clock");
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(now - 31 * 24 * 60 * 60);
    crate::transport::store::open_directory(&old)
        .expect("the job directory")
        .set_times(std::fs::FileTimes::new().set_modified(stamp))
        .expect("the directory mtime");

    maintain(&state);

    assert!(
        !old.exists(),
        "the daily pass left a job directory past the age limit"
    );
}

#[test]
fn the_legacy_files_are_imported_once_and_then_removed() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let ledger = ledger_path(&state);
    let now = now_seconds().expect("a clock");
    std::fs::write(
        &ledger,
        format!(
            "{{\"at\":{},\"verdict\":\"allow\"}}\n{{\"at\":{},\"verdict\":\"deny\"}}\n",
            now - 60,
            now - 50
        ),
    )
    .expect("a ledger");
    std::fs::write(
        rotated(&ledger),
        format!("{{\"at\":{},\"verdict\":\"oldest\"}}\n", now - 70),
    )
    .expect("a rotated ledger");
    std::fs::write(
        roles_path(&state),
        format!("{{\"at\":{},\"agent_type\":\"explore\"}}\n", now - 40),
    )
    .expect("a role log");

    maintain(&state);

    assert!(!ledger.exists(), "the ledger file outlived the import");
    assert!(
        !rotated(&ledger).exists(),
        "the rotated ledger outlived the import"
    );
    assert_eq!(
        decision_records(&state).len(),
        3,
        "the import lost a record"
    );
    assert_eq!(role_records(&state).len(), 1, "the import lost a role");

    maintain(&state);
    assert_eq!(
        decision_records(&state).len(),
        3,
        "a second maintain imported the history twice"
    );
}

#[test]
fn two_session_starts_import_the_legacy_history_once_between_them() {
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let ledger = ledger_path(&state);
    let now = now_seconds().expect("a clock");
    let mut lines = String::new();
    for n in 0..20 {
        lines.push_str(&format!("{{\"at\":{},\"verdict\":\"v{n}\"}}\n", now - 60));
    }
    std::fs::write(&ledger, lines).expect("a ledger");

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let state = state.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                maintain(&state);
            });
        }
    });

    assert_eq!(
        decision_records(&state).len(),
        20,
        "two SessionStarts imported the legacy history twice"
    );
}

#[test]
fn a_failed_import_keeps_the_legacy_file_and_counts_the_failure() {
    // The only copy of the pre-migration history is the file itself, so a
    // failed append must not take it: the import keeps the file, leaves the
    // one-time marker unset, and counts the failure `doctor` reads.
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("runs");
    std::fs::create_dir_all(&state).expect("the state directory");
    let ledger = ledger_path(&state);
    let now = now_seconds().expect("a clock");
    std::fs::write(
        &ledger,
        format!("{{\"at\":{},\"verdict\":\"allow\"}}\n", now - 60),
    )
    .expect("a ledger");

    // The table the import would write to is gone, so every append fails
    // the way a write failure would.
    Store::open(&crate::transport::store::beside(&state)).expect("the store");
    rusqlite::Connection::open(crate::transport::store::beside(&state))
        .expect("a raw connection")
        .execute_batch("DROP TABLE decisions")
        .expect("drop the decision table");

    maintain(&state);

    assert!(ledger.exists(), "a failed import deleted its only copy");
    let store = Store::open(&crate::transport::store::beside(&state)).expect("the store");
    assert!(
        store
            .meta(LEGACY_IMPORTED)
            .expect("the marker reads")
            .is_none(),
        "a failed import marked itself done"
    );
    assert!(
        store
            .record_failures()
            .expect("the failures read")
            .iter()
            .any(|failure| failure.kind == "decisions" && failure.count == 1),
        "a failed import was not counted"
    );
}

#[test]
fn a_run_id_never_escapes_the_state_directory() {
    // *Never* is a claim about a population, and this checked one string.
    // `pointer_path` is public and says why it sanitises again — "the next
    // caller may not mint its argument the same way" — and `--run-id` is a
    // flag, so the argument is whatever somebody typed.
    let root = Path::new("/state");
    for id in [
        "../../etc/passwd",
        "..\\..\\Windows\\System32",
        "/etc/passwd",
        "C:\\Windows\\System32\\drivers\\etc\\hosts",
        "\\\\server\\share\\x",
        "..",
        ".",
        "",
        "a/b",
        "a\0b",
        "a b",
        // Reserved Windows device names, which `transport::worktree`
        // refuses outright for a *directory* component. **Measured here
        // rather than assumed**: a pointer named `CON.json` inside a
        // subdirectory is an ordinary file — it stores and loads back with
        // its issue intact — so this needs no rule of its own, and adding
        // one on reasoning alone is what that module's own guard forbids.
        "CON",
        "NUL",
        "COM1",
        "aux",
    ] {
        let path = pointer_path(root, id);
        assert_eq!(path.parent(), Some(root), "{id:?} left the directory");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        assert!(
            !name.contains(['/', '\\', ':', '\0']),
            "{id:?} became {name:?}, which is more than one path component"
        );
        assert!(
            name.ends_with(".json"),
            "{id:?} became {name:?}, which the reader will not find"
        );
    }
}

#[test]
fn the_window_closes() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut run = Run::new("claude-window00".to_owned());
    run.mark_verified();
    assert!(run.within_window(Duration::from_secs(600)));
    // An answer from an hour ago does not clear a write now.
    run.verified_at = run.verified_at.map(|seconds| seconds - 3600);
    assert!(!run.within_window(Duration::from_secs(600)));
    store(root.path(), &run).expect("the pointer writes");
}

#[test]
fn an_answer_stamped_in_the_future_is_not_an_answer_that_is_still_fresh() {
    // The window was `now.saturating_sub(verified) < window`, and
    // `saturating_sub` floors at zero. So a verification stamped **ahead of
    // the clock** measured as nought seconds old and stayed inside the
    // window — not for a while, but for as long as the run lived. Every
    // routine write from then on rode on an answer the tracker was never
    // asked for again.
    //
    // It does not take an attacker. A clock corrected backwards is ordinary:
    // an NTP step, a VM resuming from a snapshot, a laptop whose RTC was
    // running fast, a dual-boot machine that keeps local time in hardware.
    // Any of those turns every stamp already written into a future one.
    //
    // And the arm below this one says what the answer should have been in
    // its own words — a clock that will not say what time it is means
    // *ask*, "which is the safe direction". A clock that says something
    // impossible was taking the other one.
    let mut run = Run::new("claude-ahead000".to_owned());
    run.mark_verified();
    let window = Duration::from_secs(600);

    // A minute ahead: a small skew, and already outside anything that can be
    // called a fresh answer.
    run.verified_at = run.verified_at.map(|seconds| seconds + 60);
    assert!(
        !run.within_window(window),
        "an answer from the future was counted as one from just now"
    );

    // An hour ahead — what a real clock correction leaves behind — and the
    // window would never close again for the life of the run.
    run.verified_at = run.verified_at.map(|seconds| seconds + 3600);
    assert!(
        !run.within_window(window),
        "the window never closes once a stamp is ahead of the clock"
    );

    // The floor: the window still opens for an answer that really is fresh,
    // so a fix that simply answered `false` would not pass here.
    run.mark_verified();
    assert!(
        run.within_window(window),
        "an answer from just now stopped clearing a routine write"
    );

    // And the report says the same thing the gate now does. `silence` held
    // its own `saturating_sub`, so `status` printed `last answer just now`
    // for a stamp the window had already stopped believing — the report and
    // the gate disagreeing about one field.
    assert_eq!(silence(&run).said(), "just now");
    run.verified_at = run.verified_at.map(|seconds| seconds + 3600);
    assert_eq!(
        silence(&run).said(),
        "stamped ahead of this clock",
        "the report still claimed a fresh answer the gate had stopped trusting"
    );
    assert_eq!(
        silence(&run).seconds(),
        None,
        "a number was given for a time that has not elapsed"
    );
    // Not `never verified`, which is what `None` renders as: this run **was**
    // verified, and only its clock moved. Flattening the two would be the
    // same fault one field over.
    assert_ne!(
        silence(&run).said(),
        silence(&Run::new("x".to_owned())).said()
    );
    assert_eq!(silence(&Run::new("x".to_owned())).said(), "never verified");
}

#[test]
fn forgetting_a_run_leaves_no_pointer() {
    let root = tempfile::tempdir().expect("a temporary root");
    let run = Run::new("claude-forgetme".to_owned());
    store(root.path(), &run).expect("the pointer writes");
    forget(root.path(), &run.run_id);
    assert_eq!(load(root.path(), &run.run_id).verified_at, None);
}

#[test]
fn a_load_racing_a_store_never_sees_a_run_that_swore_nothing() {
    // The gate reads the pointer on every tool call and writes it back on
    // every allowed one, so an agent making parallel calls has a store and a
    // load on the same file at the same time as its ordinary state. With a
    // truncating write, the reader's parse fails and `load` answers with a
    // fresh run — no issue, so `gate` says the run is outside its authority
    // and the write goes through. The gate opens for the length of a
    // truncation, and nothing anywhere records that it did.
    let root = tempfile::tempdir().expect("a temporary root");
    let root = root.path().to_path_buf();
    let mut run = Run::new("claude-racing00".to_owned());
    run.issue = Some(crate::identity::Identity::Number(9));
    store(&root, &run).expect("the first store lands");

    let writing = {
        let root = root.clone();
        std::thread::spawn(move || {
            for _ in 0..400 {
                store(&root, &run).expect("a store lands");
            }
        })
    };

    for _ in 0..400 {
        let seen = load(&root, "claude-racing00");
        assert_eq!(
            seen.issue,
            Some(crate::identity::Identity::Number(9)),
            "a load caught the pointer mid-write and read it as unsworn"
        );
    }
    writing.join().expect("the writer finishes");

    // And nothing is left beside it: a directory filling with temporaries
    // is a second failure nobody would think to look for.
    let stray: Vec<_> = std::fs::read_dir(&root)
        .expect("the directory is readable")
        .flatten()
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy() != "claude-racing00.json")
        .filter(|name| name.to_string_lossy() != POINTERS_LOCK)
        .collect();
    assert!(stray.is_empty(), "left behind: {stray:?}");
}

#[test]
fn the_state_root_is_under_a_home_rather_than_wherever_the_process_stands() {
    // `state_root -> Ok(Default::default())` survived: an empty path makes
    // every run pointer relative to the working directory, so a run's oath
    // would follow whoever changed directory and be invisible from anywhere
    // else. The gate would read no claim and stand aside.
    let root = state_root(None).expect("a state root");
    assert!(root.is_absolute(), "{} is not absolute", root.display());
    assert!(root.ends_with("runs"), "{}", root.display());
}

#[test]
fn an_operation_id_is_distinct_across_the_things_it_is_meant_to_separate() {
    // The doc says the key's only job is to be distinct. Nothing held it to
    // that: replacing the hash's `^=` with `&=` or `|=` left the suite green,
    // and both collapse the run-id's contribution — so two runs claiming the
    // same issue in the same nanosecond would share an idempotency key, which
    // is a retry writing over somebody else's operation.
    let ids: Vec<String> = (0..64)
        .map(|n| mint_operation_id(&format!("claude-{n:08}"), 7))
        .collect();
    assert!(ids.iter().all(|id| id.len() == 32));

    // The **second** half, on its own. The first sixteen digits are the
    // nanosecond, which separates these calls whatever the hash does — so a
    // set over the whole key stayed full even with the run-id's contribution
    // thrown away, and the test passed while proving nothing. Two runs that
    // claim in the same nanosecond are separated by this half and by nothing
    // else.
    let tails: std::collections::BTreeSet<&str> = ids.iter().map(|id| &id[16..]).collect();
    assert_eq!(
        tails.len(),
        64,
        "sixty-four run-ids produced {} distinct keys: the run-id is not reaching the key",
        tails.len()
    );

    // And the issue reaches it too. One run claiming two issues in the same
    // nanosecond is separated by this and by nothing else — the seed folds
    // the issue in before the run-id does its rounds, and an `&` there drops
    // it silently for every issue at once.
    let per_issue: std::collections::BTreeSet<String> = (1..64u64)
        .map(|issue| mint_operation_id("claude-oneruns0", issue)[16..].to_owned())
        .collect();
    assert_eq!(
        per_issue.len(),
        63,
        "sixty-three issues produced {} distinct keys: the issue is not reaching the key",
        per_issue.len()
    );
}

#[test]
fn forgetting_a_run_removes_its_pointer() {
    // `forget` replaced with `()` survived. `SessionEnd` calls it, so the
    // pointer would outlive every session that ever ran — and `status` would
    // list runs that ended, from a directory that only grows.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut run = Run::new("claude-ending00".to_owned());
    run.issue = Some(crate::identity::Identity::Number(3));
    store(root.path(), &run).expect("the pointer lands");
    assert_eq!(
        load(root.path(), "claude-ending00").issue,
        Some(crate::identity::Identity::Number(3))
    );

    forget(root.path(), "claude-ending00");
    assert_eq!(load(root.path(), "claude-ending00").issue, None);
    assert!(holdings(root.path()).is_empty());
}

#[test]
fn a_writer_with_old_news_does_not_overwrite_a_fresher_pointer() {
    // The threat a review-authority model calls "concurrent or
    // stale writer", and the field that makes it dangerous here is
    // `worktree`. A hook loads the pointer, allows a write and stores; if
    // `start-branch` recorded the isolated checkout in between, the hook's
    // store used to erase it — and a write inside that worktree then passed
    // as `Outside`, which is the hole that was closed once already.
    let root = tempfile::tempdir().expect("a temporary root");
    let root = root.path();

    let mut sworn = Run::new("claude-stale000".to_owned());
    sworn.issue = Some(crate::identity::Identity::Number(7));
    store(root, &sworn).expect("the claim lands");

    // What a second process is holding: loaded before the worktree existed.
    let stale = load(root, "claude-stale000");

    // Meanwhile the isolated checkout is recorded.
    let mut fresh = load(root, "claude-stale000");
    fresh.worktree = Some(root.join("trees").join("issue-7"));
    store(root, &fresh).expect("the worktree lands");

    // The stale writer stores what it had. It must not win.
    store(root, &stale).expect("a stale store is not an error");
    let after = load(root, "claude-stale000");
    assert!(
        after.worktree.is_some(),
        "a writer with old news erased the isolated checkout"
    );
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(7)));

    // And the ordinary path still works: a writer holding the current
    // revision writes, and the revision moves.
    let mut current = load(root, "claude-stale000");
    current.state = Some("review".to_owned());
    store(root, &current).expect("a current store lands");
    let after = load(root, "claude-stale000");
    assert_eq!(after.state.as_deref(), Some("review"));
    assert!(after.revision > current.revision);
}
