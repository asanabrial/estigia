use super::*;

fn reason(failure: Failure) -> String {
    failure
        .envelope()
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_child_ref_is_never_mistaken_for_the_parent() {
    // Verified against git 2.55: with only `foo/bar` present, the pattern
    // `refs/heads/foo` prints `refs/heads/foo/bar`. Reporting that object id
    // as `foo`'s head is not an unhelpful answer, it is the wrong one — `foo`
    // would be present at a commit belonging to another branch.
    let listing = "refs/heads/foo/bar\t1111111111111111111111111111111111111111\n";
    assert!(
        exact_refs(listing, "refs/heads/foo")
            .expect("parsed")
            .is_empty(),
        "the child was taken for the parent"
    );

    // The parent itself, alongside its children.
    let both = "refs/heads/foo\t2222222222222222222222222222222222222222\n\
                refs/heads/foo/bar\t1111111111111111111111111111111111111111\n";
    assert_eq!(
        exact_refs(both, "refs/heads/foo").expect("parsed"),
        vec!["2222222222222222222222222222222222222222".to_owned()]
    );

    // A line git could not have written is a read that did not answer, not an
    // absence: absence is what makes a resumed branch restart from zero.
    assert!(exact_refs("refs/heads/foo 2222", "refs/heads/foo").is_err());
}

#[test]
fn an_object_id_is_a_full_lowercase_hash_and_nothing_else() {
    assert!(is_object_id(&"a".repeat(40)));
    assert!(is_object_id(&"0".repeat(64)));
    // Abbreviated, over-long, uppercase, and not hex at all.
    for bad in ["abc1234", &"a".repeat(41), &"A".repeat(40), &"g".repeat(40)] {
        assert!(!is_object_id(bad), "{bad:?} passed as an object id");
    }
}

#[test]
fn a_worktree_override_must_be_absolute() {
    assert_eq!(
        reason(super::validate_worktree_override(Some("relative/worktree")).expect_err("relative")),
        "worktree-location-not-absolute"
    );
    super::validate_worktree_override(None).expect("no override");
    super::validate_worktree_override(Some(std::env::temp_dir().to_string_lossy().as_ref()))
        .expect("absolute override");
}

#[test]
fn start_branch_reads_head_from_the_isolated_checkout() {
    let clone = tempfile::tempdir().expect("a clone");
    crate::transport::run(
        &["git", "init", "--quiet", "-b", "main", "."],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("a repository is made");
    crate::transport::run(
        &["git", "config", "user.email", "estigia@example.invalid"],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("an identity is configured");
    crate::transport::run(
        &["git", "config", "user.name", "Estigia test"],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("an identity is configured");
    std::fs::write(clone.path().join("main.txt"), "main\n").expect("main content");
    crate::transport::run(
        &["git", "add", "main.txt"],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("main content is staged");
    crate::transport::run(
        &["git", "commit", "--quiet", "-m", "main"],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("main is committed");
    let main_head = super::object_at(clone.path(), "HEAD").expect("main head");

    crate::transport::run(
        &["git", "branch", "work", "HEAD"],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("a work branch exists");
    let checkout = clone.path().join("checkout");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("worktree"),
            std::ffi::OsString::from("add"),
            std::ffi::OsString::from("--"),
            checkout.as_os_str().to_owned(),
            std::ffi::OsString::from("work"),
        ],
        Some(clone.path()),
        crate::transport::How::write(),
    )
    .expect("the isolated checkout exists");
    std::fs::write(checkout.join("work.txt"), "work\n").expect("work content");
    crate::transport::run(
        &["git", "add", "work.txt"],
        Some(&checkout),
        crate::transport::How::write(),
    )
    .expect("work content is staged");
    crate::transport::run(
        &["git", "commit", "--quiet", "-m", "work"],
        Some(&checkout),
        crate::transport::How::write(),
    )
    .expect("work is committed");

    let (work_head, coherent, verdict) =
        super::checkout_identity(&checkout, true, None, None).expect("identity is read");
    assert_ne!(work_head, main_head, "the fixture must diverge");
    assert_eq!(
        work_head,
        super::object_at(&checkout, "HEAD").expect("the checkout's actual head"),
        "start_branch read HEAD from the repository checkout instead of the isolated worktree"
    );
    let (_, coherent_at_base, verdict_at_base) =
        super::checkout_identity(&checkout, true, Some(&work_head), Some(&work_head))
            .expect("fresh identity is judged");
    assert!(coherent, "the existing local-only state was rejected");
    assert_eq!(verdict, "local-only");
    assert!(
        coherent_at_base,
        "the unrelated main checkout made the worktree look divergent"
    );
    assert_eq!(verdict_at_base, "fresh-at-base");

    let missing = "a".repeat(40);
    let failed = super::checkout_identity(&checkout, false, Some(&missing), None)
        .expect_err("an ancestry command failure was read as not-an-ancestor");
    assert!(
        matches!(failed, Failure::Read(_)),
        "an unreadable ancestry was not a failed read: {failed:?}"
    );
}

#[test]
fn preflight_discovers_and_resumes_a_remote_only_branch() {
    let root = tempfile::tempdir().expect("a fixture root");
    let remote = root.path().join("remote.git");
    let publisher = root.path().join("publisher");
    let clone = root.path().join("clone");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("init"),
            std::ffi::OsString::from("--quiet"),
            std::ffi::OsString::from("--bare"),
            remote.as_os_str().to_owned(),
        ],
        Some(root.path()),
        crate::transport::How::write(),
    )
    .expect("a bare remote exists");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("init"),
            std::ffi::OsString::from("--quiet"),
            std::ffi::OsString::from("-b"),
            std::ffi::OsString::from("main"),
            publisher.as_os_str().to_owned(),
        ],
        Some(root.path()),
        crate::transport::How::write(),
    )
    .expect("a publisher exists");
    for (key, value) in [
        ("user.email", "estigia@example.invalid"),
        ("user.name", "Estigia test"),
    ] {
        crate::transport::run(
            &["git", "config", key, value],
            Some(&publisher),
            crate::transport::How::write(),
        )
        .expect("an identity is configured");
    }
    std::fs::write(publisher.join("main.txt"), "main\n").expect("main content");
    crate::transport::run(
        &["git", "add", "main.txt"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("main is staged");
    crate::transport::run(
        &["git", "commit", "--quiet", "-m", "main"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("main is committed");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("remote"),
            std::ffi::OsString::from("add"),
            std::ffi::OsString::from("origin"),
            remote.as_os_str().to_owned(),
        ],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("the remote is configured");
    crate::transport::run(
        &["git", "push", "--quiet", "-u", "origin", "main"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("main is published");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("clone"),
            std::ffi::OsString::from("--quiet"),
            std::ffi::OsString::from("--single-branch"),
            std::ffi::OsString::from("--branch"),
            std::ffi::OsString::from("main"),
            remote.as_os_str().to_owned(),
            clone.as_os_str().to_owned(),
        ],
        Some(root.path()),
        crate::transport::How::write(),
    )
    .expect("the consumer clone predates the issue branch");

    crate::transport::run(
        &["git", "switch", "--quiet", "-c", "fix/82"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("the issue branch exists");
    std::fs::write(publisher.join("issue.txt"), "issue\n").expect("issue content");
    crate::transport::run(
        &["git", "add", "issue.txt"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("issue content is staged");
    crate::transport::run(
        &["git", "commit", "--quiet", "-m", "issue"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("issue work is committed");
    crate::transport::run(
        &["git", "push", "--quiet", "-u", "origin", "fix/82"],
        Some(&publisher),
        crate::transport::How::write(),
    )
    .expect("the issue branch is published after the clone");
    let published = super::object_at(&publisher, "HEAD").expect("published head");

    let preflight = super::branch_preflight(&clone, "fix/82", "main").expect("preflight");
    assert!(!preflight.fresh, "a remote-only branch was called fresh");
    assert_eq!(
        super::object_at(&clone, "fix/82").expect("local issue branch"),
        published,
        "the remote-only branch was restarted from the base"
    );
    assert!(
        super::refresh_remote_branch(&clone, "missing/branch", true).is_err(),
        "a required published branch was allowed to disappear"
    );
}

#[test]
fn a_detached_checkout_still_owns_its_directory() {
    // `-z`: records separated by an empty field, attributes one field each.
    // Real object ids: a malformed HEAD is refused now, for the reason the
    // transport gives — a record nothing can read is not a directory nobody owns.
    let listing = "worktree /w/a\0HEAD aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\0\
                   branch refs/heads/fix/6\0\0\
                   worktree /w/b\0HEAD bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\0\
                   detached\0\0";
    let found = parse_worktree_listing(listing).expect("a well-formed registry");
    assert_eq!(found.len(), 2);
    assert_eq!(
        found.get(&normalise_path(std::path::Path::new("/w/a"))),
        Some(&Some("fix/6".to_owned())),
        "the branch was not read, or the ref prefix was not stripped"
    );
    // Present with no branch, rather than absent: a detached checkout owns the
    // directory just as firmly, and an absent path reads as one this run may
    // take.
    assert_eq!(
        found.get(&normalise_path(std::path::Path::new("/w/b"))),
        Some(&None),
        "the detached checkout was read as an unowned directory"
    );
}

#[test]
fn each_way_a_worktree_can_fail_to_be_this_runs_is_refused_by_its_own_name() {
    // One refusal used to answer all of these, spelled
    // `worktree-not-owned-by-this-run`, which the transport never emits and
    // `SKILL.md` never names. The three below are the transport's, and the
    // agent is told to do something different for each — so which one it is
    // *is* the answer.
    let registered = |path: &std::path::Path, branch: Option<&str>| {
        let mut found = std::collections::BTreeMap::new();
        found.insert(normalise_path(path), branch.map(ToOwned::to_owned));
        found
    };
    let empty = std::collections::BTreeMap::new();
    let root = std::env::temp_dir().join("estigia-occupy-probe");
    let _ = std::fs::remove_dir_all(&root);
    let here = root.join("checkout");
    std::fs::create_dir_all(&here).expect("a directory to argue about");
    let mine = serde_json::json!({"run_id": "claude-a", "issue": 7});

    // Absent is fresh, whatever the registry still says. A checkout removed by
    // hand stays registered until `git worktree prune`, and refusing there is a
    // start-branch that never works again.
    let gone = root.join("pruned");
    for known in [empty.clone(), registered(&gone, Some("fix/6"))] {
        assert!(
            !may_occupy(&gone, &known, None, "claude-a", "fix/6").expect("fresh"),
            "an absent path was read as a resume"
        );
    }

    // The marker names this run and the registry names this branch: a resume.
    assert!(
        may_occupy(
            &here,
            &registered(&here, Some("fix/6")),
            Some(&mine),
            "claude-a",
            "fix/6"
        )
        .expect("a resume"),
        "this run could not prove it owned its own directory"
    );

    // Registered to another branch, or to no branch at all. Nothing consulted
    // the registry before this, so a checkout of somebody else's branch was
    // resumed into as long as the marker matched.
    for known in [
        registered(&here, Some("fix/9")),
        registered(&here, None),
        empty,
    ] {
        assert_eq!(
            reason(
                may_occupy(&here, &known, Some(&mine), "claude-a", "fix/6")
                    .expect_err("another branch")
            ),
            "worktree-path-occupied"
        );
    }

    // This branch, but nobody has ever recorded owning it. The only one of the
    // three carrying a written recovery, which the single refusal withheld.
    let unproven = may_occupy(
        &here,
        &registered(&here, Some("fix/6")),
        None,
        "claude-a",
        "fix/6",
    )
    .expect_err("unproven");
    let action = unproven
        .envelope()
        .get("action")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    assert_eq!(reason(unproven), "worktree-ownership-unproven");
    assert!(
        action.contains("git worktree remove"),
        "the recovery this refusal exists to give was not given: {action:?}"
    );

    // This branch, claimed by somebody else.
    assert_eq!(
        reason(
            may_occupy(
                &here,
                &registered(&here, Some("fix/6")),
                Some(&serde_json::json!({"run_id": "claude-b", "issue": 7})),
                "claude-a",
                "fix/6"
            )
            .expect_err("another run")
        ),
        "worktree-owned-by-another-run"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_link_in_an_ancestor_is_resolved_and_a_linked_leaf_is_refused() {
    let root = std::env::temp_dir().join("estigia-alias-probe");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("real")).expect("a real directory");

    // An ordinary path: canonicalised, and accepted.
    let plain = canonical_worktree_path(&root.join("real")).expect("a real directory");
    assert_eq!(normalise_path(&plain), normalise_path(&root.join("real")));
    #[cfg(unix)]
    assert_eq!(
        plain,
        std::fs::canonicalize(&root)
            .expect("the temporary root resolves")
            .join("real"),
        "Linux and macOS keep the native canonical path unchanged"
    );
    #[cfg(windows)]
    assert!(
        !plain.to_string_lossy().starts_with(r"\\?\"),
        "a path passed to git must not retain Windows' verbatim prefix: {plain:?}"
    );

    // A link in an ancestor redirects every run identically. Resolve it and
    // retain the leaf name rather than refusing a valid worktree root.
    std::fs::create_dir_all(root.join("real-parent")).expect("a real parent");
    #[cfg(windows)]
    let ancestor_linked =
        std::os::windows::fs::symlink_dir(root.join("real-parent"), root.join("linked-parent"))
            .is_ok();
    #[cfg(unix)]
    let ancestor_linked =
        std::os::unix::fs::symlink(root.join("real-parent"), root.join("linked-parent")).is_ok();
    if ancestor_linked {
        let through_ancestor =
            canonical_worktree_path(&root.join("linked-parent").join("checkout"))
                .expect("an ancestor link is ordinary");
        // Both sides through the same resolution, which is the property being
        // claimed: two spellings of one place answer identically.
        //
        // The right-hand side used to be `normalise_path` of the unresolved
        // path, and that compares two different questions. `checkout` is never
        // created, so `canonicalize` fails on it and `normalise_path` falls back
        // to the spelling it was handed — while the left-hand side had its
        // ancestors resolved. On a machine where the temporary directory is
        // already canonical the two agree by accident and the test passes; on
        // one where it is not, they never can. Measured both ways: green here
        // with `TEMP=C:\Users\<name>\…`, red with the 8.3 form
        // `C:\Users\<NAME>~1\…`, and red on macOS where the same directory is
        // reached as `/var/folders/…` and canonicalises to `/private/var/…`.
        //
        // Production never asks this. `normalise_path` is only reached for a
        // path that exists — `worktree_is_ours` returns early when it does not,
        // and the registry it compares against holds paths git has reported —
        // so the fallback is not a defect the code can hit. It was a defect in
        // what this test asserted.
        assert_eq!(
            normalise_path(&through_ancestor),
            normalise_path(
                &canonical_worktree_path(&root.join("real-parent").join("checkout"))
                    .expect("the real parent is ordinary too")
            )
        );
    }

    // A leaf that is a link to another run's directory. Two distinct valid run
    // IDs resolving to one real directory is the collapse no resolution fixes.
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_dir(root.join("real"), root.join("alias")).is_ok();
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(root.join("real"), root.join("alias")).is_ok();
    if linked {
        assert_eq!(
            reason(canonical_worktree_path(&root.join("alias")).expect_err("an alias")),
            "aliased-worktree-path"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// An unreadable ownership marker is a failed read, not an unowned checkout.
///
/// The port read it with `.ok().and_then(|text| from_str(&text).ok())`, so a
/// marker that exists and cannot be parsed arrived at `may_occupy` spelled
/// exactly like one that was never written — and that spelling has a message:
/// *no run has ever recorded owning it*, followed by instructions to remove the
/// checkout with `git worktree remove`.
///
/// So a corrupted marker told an agent a false fact and then handed it the
/// destructive recovery for it. The binding raises `ReadFailure` on all three
/// of unreadable, unparsable and no-run-id, and says why in its own words:
/// *unknown may not be spelled like a permissive fact*.
#[test]
fn an_unreadable_ownership_marker_is_not_an_unowned_checkout() {
    // A real repository, because the marker now lives where the transport puts
    // it: inside the checkout's **private git admin directory**, which only git
    // can name. That is the whole of the change this fixture is following — a
    // marker in the working tree is one people delete, and one neither side
    // could see in the other's place.
    let checkout = tempfile::tempdir().expect("a checkout");
    crate::transport::run(
        &["git", "init", "--quiet", "."],
        Some(checkout.path()),
        crate::transport::How::write(),
    )
    .expect("a repository is made");
    let marker = super::ownership_path(checkout.path()).expect("git names its admin directory");

    // The floor, both ways round: a checkout nothing marked answers `None`, and
    // a marked one answers with its record. A reader that failed on everything
    // would satisfy the assertions below and refuse every resume there is.
    assert_eq!(
        super::read_ownership(checkout.path()).expect("an unmarked checkout was refused"),
        None,
        "a checkout nothing ever marked is not an absent answer"
    );
    std::fs::write(&marker, r#"{"run_id":"claude-abcd1234"}"#).expect("a marker is written");
    assert_eq!(
        super::read_ownership(checkout.path())
            .expect("a readable marker was refused")
            .and_then(|record| record
                .get("run_id")
                .and_then(|id| id.as_str().map(str::to_owned))),
        Some("claude-abcd1234".to_owned()),
        "a marker that names a run did not come back"
    );

    // And the three that are not answers.
    for (what, text) in [
        ("unparsable", "{not json"),
        ("not a record", "[\"claude-abcd1234\"]"),
        ("naming no run", r#"{"issue":12}"#),
        ("naming a run that is not a name", r#"{"run_id":12}"#),
    ] {
        std::fs::write(&marker, text).expect("a marker is written");
        let refusal = super::read_ownership(checkout.path())
            .expect_err(&format!("a marker {what} passed for an unowned checkout"));
        assert!(
            matches!(refusal, crate::transport::Failure::Read(_)),
            "a marker {what} refused as something other than a failed read: {refusal:?}"
        );
        assert!(
            format!("{refusal:?}").contains("issue-flow-owner.json"),
            "the refusal does not name the marker it could not read: {refusal:?}"
        );
    }
}

#[test]
fn an_absent_worktree_has_no_ownership_marker_to_read() {
    let root = tempfile::tempdir().expect("a worktree root");
    let absent = root.path().join("not-created-yet");

    assert_eq!(
        super::read_ownership(&absent).expect("an absent worktree is fresh"),
        None,
        "start-branch tried to run git inside the checkout before creating it"
    );
}

/// One branch, one reservation — and a run adopts its own leftover.
///
/// The binding serializes `start-branch` for one branch within one clone with
/// `O_CREAT | O_EXCL`, and says why it is not a timeout: *a timeout is a guess
/// about a process nobody looked at, and the case it guesses wrong is the
/// expensive one — a slow-but-live run gets its checkout taken while it is
/// writing into it.* Git's own "already used by worktree at" check is a read
/// followed by a write with nothing between them, and the binding's note
/// records it losing that race on Git 2.55.0.windows.3: two successful
/// concurrent worktrees for one branch.
///
/// None of it was ported. The absence is invisible to every crossing there is,
/// because a crossing runs one process at a time and an uncontended lock leaves
/// no trace in the envelope.
#[test]
fn one_branch_is_reserved_once_and_a_run_adopts_its_own_lock() {
    let clone = tempfile::tempdir().expect("a clone");
    let lock = clone.path().join("branch-locks").join("one.json");
    std::fs::create_dir_all(lock.parent().expect("that file has a directory"))
        .expect("the lock directory is made");

    let first = super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-aaaa1111",
        12,
        "2026-08-01T12:00Z",
    )
    .expect("the first reservation was refused");

    // A second run meets it and stops. Never broken on elapsed time: what it
    // reports is who holds it and where.
    let refusal = super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-bbbb2222",
        12,
        "2026-08-01T12:00Z",
    )
    .expect_err("a second run took a branch another run had reserved");
    let crate::transport::Failure::Stop(envelope) = &refusal else {
        panic!("a held lock refused as something other than a stop: {refusal:?}");
    };
    assert_eq!(
        envelope.get("reason").and_then(|r| r.as_str()),
        Some("branch-locked-by-another-run")
    );
    assert_eq!(
        envelope
            .get("held_by")
            .and_then(|held| held.get("run_id"))
            .and_then(|id| id.as_str()),
        Some("claude-aaaa1111"),
        "the refusal does not say who holds it: {envelope}"
    );

    // And this run's own leftover is adopted rather than merely stepped past.
    // Proceeding without adopting is how a crashed run's lock outlives every
    // retry and blocks a different run forever: nobody is left entitled to
    // remove it.
    let again = super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-aaaa1111",
        12,
        "2026-08-01T12:00Z",
    )
    .expect("a run was refused its own leftover lock");
    drop(again);
    assert!(
        !lock.exists(),
        "an adopted lock was not removed, so nobody is entitled to remove it"
    );

    // The floor: the first reservation still removes its own on the way out,
    // and the branch is free again afterwards.
    drop(first);
    super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-cccc3333",
        12,
        "2026-08-01T12:00Z",
    )
    .expect("the lock outlived the run that took it");
}

/// A lock nobody can parse still proves somebody holds it.
///
/// The line is drawn in a different place from the ownership marker's, and the
/// binding draws it there on purpose. A marker that does not parse leaves the
/// *owner* unknown, and unknown may not be spelled like a fact. A lock that
/// does not parse leaves the **holder's name** unknown while the fact that
/// somebody created the file is not in doubt — so it comes back as a holder
/// nobody can name, and the run that meets it still stops.
///
/// What must never happen is this answering *nobody holds it*.
#[test]
fn a_lock_that_does_not_parse_is_a_holder_nobody_can_name() {
    let clone = tempfile::tempdir().expect("a clone");
    let lock = clone.path().join("held.json");

    // The floor: a lock that does parse comes back as itself.
    std::fs::write(&lock, r#"{"run_id":"claude-aaaa1111"}"#).expect("a lock is written");
    assert_eq!(
        super::read_lock_record(&lock)
            .expect("a readable lock was refused")
            .get("run_id")
            .and_then(|id| id.as_str()),
        Some("claude-aaaa1111"),
        "a lock that names its holder did not come back"
    );

    for text in ["{not json", "[\"claude-aaaa1111\"]"] {
        std::fs::write(&lock, text).expect("a lock is written");
        let held = super::read_lock_record(&lock).expect("an unparsable lock refused the read");
        assert_eq!(
            held.get("run_id"),
            Some(&serde_json::Value::Null),
            "an unparsable lock named a holder it cannot know: {held}"
        );
        assert_eq!(
            held.get("unreadable").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        // And a run meeting it stops, which is the whole point of the shape.
        let refusal =
            super::reserve_branch(&lock, "fix/12", "claude-bbbb2222", 12, "2026-08-01T12:00Z")
                .expect_err("a lock nobody can parse was treated as a free branch");
        assert!(
            matches!(refusal, crate::transport::Failure::Stop(_)),
            "{refusal:?}"
        );
    }

    // Unreadable is the other answer: the file is there and the read failed.
    std::fs::remove_file(&lock).expect("the lock goes");
    std::fs::create_dir_all(&lock).expect("something unreadable in its place");
    let refusal = super::read_lock_record(&lock).expect_err("an unreadable lock read as a record");
    assert!(
        matches!(refusal, crate::transport::Failure::Read(_)),
        "an unreadable lock refused as something other than a failed read: {refusal:?}"
    );
}

/// The lock's path is spelled the way the binding spells it.
///
/// `git rev-parse --path-format=absolute --git-common-dir` answers with forward
/// slashes on Windows. The binding builds on that with `pathlib`, which
/// normalises every separator, and the port built on it with `Path::join`,
/// which does not — so the same file came back as `C:\repo\.git\issue-flow\…`
/// there and `C:/repo/.git\issue-flow\…` here.
///
/// It names the same file, and it is still a difference: it is the path an
/// operator is handed in `branch-locked-by-another-run` and then types, and it
/// is what a crossing of that refusal would compare.
#[test]
fn the_lock_path_is_spelled_the_way_the_binding_spells_it() {
    let mixed = super::lock_under("C:/repo/.git", "fix/12-thing");
    let shown = mixed.display().to_string();
    if cfg!(windows) {
        assert!(
            !shown.contains('/'),
            "the lock path mixes separators, which is not how the binding spells it: {shown}"
        );
    }
    // The floor, on every platform: it is still the file the binding names —
    // under the common admin directory, by digest, and not the branch itself.
    assert!(
        shown.contains("issue-flow") && shown.contains("branch-locks"),
        "the lock is not where the binding keeps it: {shown}"
    );
    assert!(
        !shown.contains("fix"),
        "the branch is spelled in the filename, which a `/` in a branch name turns into a path: {shown}"
    );
    assert_eq!(
        mixed
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .len(),
        "0123456789abcdef0123456789abcdef.json".len(),
        "the digest is not the length the binding takes: {shown}"
    );
}

/// A legacy checkout git still lists but nobody deleted properly is not a stop.
///
/// The transport asks `legacy.exists()` before it looks the path up in the
/// registry; this side asked the registry alone. `git worktree list` goes on
/// listing a worktree whose directory somebody removed with `rm -rf` — measured
/// on a real repository, it comes back marked `prunable gitdir file points to
/// non-existent location` — and neither side's registry reader filters that, so
/// this one stopped a `start-branch` the transport performs.
///
/// What the stop protects is unpushed work in a directory, and its own way out
/// says so. A registration whose directory is gone holds no work, and the
/// refusal sends the operator to rescue a tree that is not there.
///
/// Both answers are asserted. A fix that simply stopped refusing would pass the
/// stale half and lose the case the stop exists for.
#[test]
fn a_legacy_worktree_git_lists_but_nobody_left_behind_is_not_a_stop() {
    let root = tempfile::tempdir().expect("a temporary root");
    let legacy = root.path().join("legacy");
    let scoped = root.path().join("legacy~claude-abcd1234");
    std::fs::create_dir_all(&legacy).expect("the legacy checkout");

    let mut registered = std::collections::BTreeMap::new();
    registered.insert(
        super::normalise_path(&legacy),
        Some("old-branch".to_owned()),
    );

    // On disk and registered: the case the refusal was written for.
    let refusal = super::legacy_worktree_block(&legacy, &scoped, &registered)
        .expect("a checkout that is still there is still a stop");
    assert!(
        format!("{refusal:?}").contains("legacy-worktree-registered"),
        "the stop lost its reason: {refusal:?}"
    );

    // Registered **detached**: git lists the path with no branch against it. The
    // binding said for a long time that this does not stop the command; it does,
    // and reports `occupied_by_branch: null`. `registered.get(…)?` unwraps only
    // the outer `Option`, so the inner `None` never reaches the `?`. Posed here
    // because a sentence in the operator's own recovery said otherwise, and the
    // way a claim like that stops drifting is a test rather than a proofread.
    let mut detached = std::collections::BTreeMap::new();
    detached.insert(super::normalise_path(&legacy), None);
    let refusal = super::legacy_worktree_block(&legacy, &scoped, &detached)
        .expect("a detached legacy checkout is still a registration");
    let envelope = format!("{refusal:?}");
    assert!(
        envelope.contains("legacy-worktree-registered"),
        "the stop lost its reason: {envelope}"
    );

    // Registered and gone: what `rm -rf` leaves behind, and what git goes on
    // listing. The registry entry is untouched, so only existence decides.
    std::fs::remove_dir_all(&legacy).expect("the directory goes");
    assert!(
        super::legacy_worktree_block(&legacy, &scoped, &registered).is_none(),
        "a registration whose directory is gone stopped a command the transport performs"
    );

    // And a path git never listed at all is not a stop either, present or not.
    std::fs::create_dir_all(&legacy).expect("the directory comes back");
    assert!(
        super::legacy_worktree_block(&legacy, &scoped, &std::collections::BTreeMap::new())
            .is_none(),
        "a directory git does not list is not a worktree"
    );
}

#[test]
fn a_runs_second_issue_gets_its_own_checkout_from_the_template_an_operator_writes() {
    // Issue #27: the checkout's directory carried the run and not the branch,
    // so a run working a queue met the checkout it made for its previous issue
    // and was refused `worktree-path-occupied` — correct for what the check
    // could see, wrong that two branches of one run were asked to share a path.
    //
    // The first attempt at this test picked `/w/<repo>/<branch>` and proved
    // nothing: a template that already names the branch is the one shape that
    // cannot exhibit the defect. So the templates here are the ones an operator
    // actually writes. `docs/configuration.md` documents the accepted value as
    // "an absolute directory" and names no placeholder at all, and the skill
    // ships the row `unset` — a bare directory is the ordinary answer, and it is
    // the shape that collides in both dimensions at once.
    let run = "claude-81d69d3e372497b6";
    let path_of = |template: &str, branch: &str, issue: u64| {
        let (scoped, migrated) = crate::transport::worktree::scoped_template(template);
        assert!(migrated, "{template:?} was left as it was");
        crate::transport::worktree::worktree_path(&scoped, "estigia", branch, run, issue)
            .expect("a path is composed")
    };

    for template in [
        // The reproduction on issue #27, verbatim.
        r"H:\REPO\worktree\estigia",
        "/w/<repo>",
        // Run-scoped and branch-less: the half that was never migrated, and the
        // one whose legacy directory is a checkout this run owns.
        "/w/<repo>~<run-id>",
    ] {
        let first = path_of(template, "fix/1-publish-review", 1);
        let second = path_of(template, "fix/2-closed-issue-gates", 2);
        assert_ne!(
            first, second,
            "{template:?}: a run's second issue was handed the checkout it made for its first"
        );
        // Named, not merely different: the branch is what keeps two tasks of one
        // run apart, and a path differing for any other reason would satisfy the
        // assertion above while leaving the defect standing.
        for (path, slug) in [
            (&first, "fix-1-publish-review"),
            (&second, "fix-2-closed-issue-gates"),
        ] {
            assert!(
                path.to_string_lossy().contains(slug),
                "{template:?}: {slug} is not in the path composed for it: {}",
                path.display()
            );
        }
        // Both still name the run, so two runs of one branch stay apart too —
        // the dimension that already worked must not be traded for this one.
        for path in [&first, &second] {
            assert!(
                path.to_string_lossy().contains(run),
                "{template:?}: the run scope was lost: {}",
                path.display()
            );
        }
    }

    // And a template that names both is left exactly as the operator wrote it.
    let (scoped, migrated) =
        crate::transport::worktree::scoped_template("/w/<repo>/<branch>~<run-id>");
    assert!(!migrated, "a fully scoped template was rewritten");
    assert_eq!(scoped, "/w/<repo>/<branch>~<run-id>");
}
