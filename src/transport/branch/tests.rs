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
fn an_unset_worktree_location_refuses_with_a_copyable_template() {
    // Through the failure `start_branch` returns, not the helper alone: what
    // this guards is the wiring between them, and reading the helper would stay
    // green if the refusal stopped calling it.
    let root = tempfile::tempdir().expect("a temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    shell(&repo, &["init", "--quiet", "-b", "main"]);
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: repo,
        config: Vec::new(),
        repo: None,
    };
    let what = start_what("fix-12", None);
    let bin = crate::test_env::scripted_gh();
    let failure = crate::test_env::with_scripted_gh(bin.path(), &claim_script(), || {
        super::start_branch(&context, &what, None)
    })
    .expect_err("a start without a worktree location passed");
    let envelope = failure.envelope();
    assert_eq!(envelope["reason"], "no-worktree-location");
    let action = envelope["action"].as_str().unwrap_or_default();
    assert!(
        action.contains("Worktree location")
            && action.contains("C:\\estigia\\worktree\\<repo>\\<branch>")
            && action.contains("/var/estigia/worktree/<repo>/<branch>"),
        "the refusal names no copyable value: {action:?}"
    );
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
        found
            .get(&normalise_path(std::path::Path::new("/w/a")))
            .map(|entry| entry.branch.clone()),
        Some(Some("fix/6".to_owned())),
        "the branch was not read, or the ref prefix was not stripped"
    );
    // Present with no branch, rather than absent: a detached checkout owns the
    // directory just as firmly, and an absent path reads as one this run may
    // take.
    assert_eq!(
        found
            .get(&normalise_path(std::path::Path::new("/w/b")))
            .map(|entry| entry.branch.clone()),
        Some(None),
        "the detached checkout was read as an unowned directory"
    );
}

/// The map's key is `normalise_path`'s comparison key, not a path — under
/// `#[cfg(unix)]` it is the hex encoding of the path bytes, which does not
/// round-trip through `PathBuf::from`. A caller that reconstructed a
/// filesystem path from the key instead of reading `RegisteredWorktree::path`
/// was inert on Linux and macOS (every candidate a nonexistent relative path)
/// and silently lowercased on Windows, which is what this test exists to
/// catch on every platform rather than only the two it was never run on.
#[test]
fn the_registered_path_is_carried_verbatim_not_reconstructed_from_the_key() {
    let listing = "worktree /w/Mixed-Case/Checkout\0\
                   HEAD aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\0\
                   branch refs/heads/fix/6\0\0";
    let found = parse_worktree_listing(listing).expect("a well-formed registry");
    let entry = found
        .values()
        .next()
        .expect("the single registered checkout");
    assert_eq!(
        entry.path,
        std::path::PathBuf::from("/w/Mixed-Case/Checkout"),
        "the carried path was reconstructed from the comparison key instead \
         of the path git actually reported"
    );
}

use crate::test_env::repository_with_commit;

/// `discover_adopted_worktree` returns the path git itself listed for the
/// matching checkout, not one rebuilt from the registry's comparison key.
///
/// `the_registered_path_is_carried_verbatim_not_reconstructed_from_the_key`
/// already pins the parser this reads, but it builds a synthetic listing and
/// never calls this function — the defect that was fixed lived in *this*
/// caller, which used to walk `registry.keys()` and rebuild a path from each
/// one, and nothing exercised that call directly. Reverting to
/// `for path in registry.keys() { PathBuf::from(path) }` still leaves the
/// parser test green, and on Windows it leaves every renewal test in
/// `harness::mcp::tests` green too — they compare through
/// `std::fs::canonicalize` on both sides, and Windows folds the lowercased,
/// forward-slashed key back onto the same directory the fixture created. Only
/// the Unix legs of those tests redden, on the platform where the original
/// defect was invisible because `#[cfg(unix)]`'s key is a hex string that
/// resolves nowhere at all.
///
/// So this compares against `registered_worktrees`'s own answer for the same
/// checkout — never `canonicalize` on either side — which is what makes it
/// red under that mutation on every platform: the hex key on Unix and the
/// lowercased spelling on Windows both differ from the path git actually
/// reported, canonicalized or not.
#[test]
fn discovery_returns_the_path_git_listed_not_the_registrys_comparison_key() {
    let repo = repository_with_commit();
    let elsewhere = tempfile::tempdir().expect("somewhere for the adopted checkout");
    let adopted = elsewhere.path().join("issue-91");
    let added = run(
        &[
            "git",
            "worktree",
            "add",
            "-b",
            "issue-91",
            adopted.to_str().expect("the fixture path is valid UTF-8"),
        ],
        Some(repo.path()),
        How::write(),
    );
    assert!(
        added.is_ok(),
        "git could not add the worktree this fixture needs: {added:?}"
    );

    let marker =
        ownership_path(&adopted).expect("git names the admin directory of a worktree it just made");
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent).expect("the marker's directory exists");
    }
    std::fs::write(
        &marker,
        serde_json::json!({
            "run_id": "claude-earlier00",
            "issue": 91,
            "branch": "issue-91",
            "worktree": adopted.display().to_string(),
            "claimed_at": "2026-01-01T00:00:00Z",
        })
        .to_string(),
    )
    .expect("the ownership marker is written");

    let context = Context {
        skill_dir: repo.path().join("skill-does-not-exist"),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };

    // "The path git listed" — read independently of `discover_adopted_worktree`,
    // so the comparison below never canonicalizes either side.
    let registered = registered_worktrees(&context).expect("git worktree list answers");
    let expected = registered
        .values()
        .find(|entry| entry.branch.as_deref() == Some("issue-91"))
        .map(|entry| entry.path.clone())
        .expect("the added worktree is registered");

    let discovered = discover_adopted_worktree(&context, &crate::identity::Identity::Number(91))
        .expect("discovery reads")
        .expect("the marked checkout is found");

    assert_eq!(
        discovered, expected,
        "discovery did not carry the path git listed verbatim: {discovered:?} != {expected:?}"
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
        found.insert(
            normalise_path(path),
            RegisteredWorktree {
                path: path.to_path_buf(),
                branch: branch.map(ToOwned::to_owned),
            },
        );
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

/// Two branches differing only in case compose two spellings that are one
/// directory on a case-insensitive filesystem. The second run's path exists
/// there — it is the first run's checkout — but the registry is keyed by the
/// first run's spelling, so the lookup misses and the refusal used to read as
/// an orphan, the one diagnosis that invites deleting a live checkout.
///
/// The collision is decided on the spellings, not on the filesystem, so this is
/// red and green on every platform rather than only where the volume folds case.
#[test]
fn two_branches_differing_only_by_case_are_refused_rather_than_called_an_orphan() {
    let root = tempfile::tempdir().expect("a worktree root");
    let compose = |branch: &str| {
        let relative = crate::transport::worktree::worktree_path(
            "<branch>~<run-id>",
            "estigia",
            branch,
            "run-x",
            &crate::identity::Identity::Number(1),
        )
        .expect("the branch composes");
        root.path().join(relative)
    };
    let registered_path = compose("Fix-1-a");
    let path = compose("fix-1-a");
    assert_ne!(
        registered_path, path,
        "the two branches must compose two spellings"
    );
    std::fs::create_dir_all(&registered_path).expect("the first checkout's directory");
    std::fs::create_dir_all(&path).expect("the second spelling's directory");

    let mut registered = std::collections::BTreeMap::new();
    registered.insert(
        normalise_path(&registered_path),
        RegisteredWorktree {
            path: registered_path.clone(),
            branch: Some("Fix-1-a".to_owned()),
        },
    );

    let failure = may_occupy(&path, &registered, None, "run-x", "fix-1-a")
        .expect_err("a case-folded collision was accepted");
    assert_eq!(
        reason(failure),
        "case-insensitive-branch-collision",
        "the second branch was not told it collides with a live checkout by case"
    );
}

/// The same collision by Unicode normalization rather than case. A composed `é`
/// branch and its decomposed sibling compose two byte sequences, and default
/// APFS is normalization-insensitive, so the two are ONE directory there. The
/// registry key is the byte spelling, so the lookup misses for the same reason
/// it misses for case, and the refusal used to read as an orphan.
#[test]
fn two_branches_differing_only_by_normalization_are_refused_rather_than_called_an_orphan() {
    let root = tempfile::tempdir().expect("a worktree root");
    let compose = |branch: &str| {
        let relative = crate::transport::worktree::worktree_path(
            "<branch>~<run-id>",
            "estigia",
            branch,
            "run-x",
            &crate::identity::Identity::Number(1),
        )
        .expect("the branch composes");
        root.path().join(relative)
    };
    let registered_path = compose("caf\u{e9}-1-a");
    let path = compose("cafe\u{301}-1-a");
    assert_ne!(
        registered_path, path,
        "the two branches must compose two byte sequences"
    );
    std::fs::create_dir_all(&registered_path).expect("the first checkout's directory");
    std::fs::create_dir_all(&path).expect("the second spelling's directory");

    let mut registered = std::collections::BTreeMap::new();
    registered.insert(
        normalise_path(&registered_path),
        RegisteredWorktree {
            path: registered_path.clone(),
            branch: Some("caf\u{e9}-1-a".to_owned()),
        },
    );

    let failure = may_occupy(&path, &registered, None, "run-x", "cafe\u{301}-1-a")
        .expect_err("a normalization-folded collision was accepted");
    assert_eq!(
        reason(failure),
        "case-insensitive-branch-collision",
        "the second branch was not told it collides with a live checkout by normalization"
    );
}

/// The fold that decides the collision, in both directions.
///
/// A separator difference is NOT a collision, or every Windows resume would be
/// refused: git reports forward slashes and this run composes native ones for
/// the same directory. A case difference is, even across those separators.
#[test]
fn a_case_only_difference_is_the_collision_and_a_separator_one_is_not() {
    assert!(folds_only_by_case_or_normalization(
        std::path::Path::new("C:/w/estigia~Fix-1-a~run-x"),
        std::path::Path::new(r"C:\w\estigia~fix-1-a~run-x")
    ));
    assert!(!folds_only_by_case_or_normalization(
        std::path::Path::new("C:/w/estigia~fix-1-a~run-x"),
        std::path::Path::new(r"C:\w\estigia~fix-1-a~run-x")
    ));
    assert!(!folds_only_by_case_or_normalization(
        std::path::Path::new("/w/estigia~fix-1-a~run-x"),
        std::path::Path::new("/w/estigia~fix-2-a~run-x")
    ));
    assert!(!folds_only_by_case_or_normalization(
        std::path::Path::new("/w/estigia~Fix-1-a~run-x"),
        std::path::Path::new("/w/estigia~Fix-1-a~run-x")
    ));
    // A normalization-only difference is the collision too, and it survives
    // neither fold: `é` precomposed and `é` decomposed are one directory on a
    // filesystem that folds normalization, which is default APFS.
    assert!(folds_only_by_case_or_normalization(
        std::path::Path::new("/w/estigia~caf\u{e9}-1-a~run-x"),
        std::path::Path::new("/w/estigia~cafe\u{301}-1-a~run-x")
    ));
    // And case and normalization together: the composed upper-case spelling and
    // the decomposed lower-case one agree once both are folded.
    assert!(folds_only_by_case_or_normalization(
        std::path::Path::new("/w/estigia~CAF\u{c9}-1-a~run-x"),
        std::path::Path::new("/w/estigia~cafe\u{301}-1-a~run-x")
    ));
    // A capital base with no precomposed accented form needs the recomposing
    // second pass: `J`+U+030C lowercases to `j`+U+030C, while `ǰ` is one code
    // point, and the two are one directory on a normalization-insensitive
    // filesystem.
    assert!(folds_only_by_case_or_normalization(
        std::path::Path::new("/w/estigia~\u{1f0}-1-a~run-x"),
        std::path::Path::new("/w/estigia~J\u{30c}-1-a~run-x")
    ));
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
        &crate::identity::Identity::Number(12),
        "2026-08-01T12:00Z",
    )
    .expect("the first reservation was refused");

    // A second run meets it and stops. Never broken on elapsed time: what it
    // reports is who holds it and where.
    let refusal = super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-bbbb2222",
        &crate::identity::Identity::Number(12),
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
        &crate::identity::Identity::Number(12),
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
        &crate::identity::Identity::Number(12),
        "2026-08-01T12:00Z",
    )
    .expect("the lock outlived the run that took it");
}

/// A write that fails must not leave a lock behind.
///
/// An empty or partial file reads as a holder nobody can name, so the same run's
/// retry was refused by a holder that did not exist.
#[test]
fn a_lock_whose_write_fails_leaves_nothing_and_the_same_run_can_retry() {
    let clone = tempfile::tempdir().expect("a clone");
    let lock = clone.path().join("branch-locks").join("one.json");
    let issue = crate::identity::Identity::Number(12);

    let failed = super::reserve_branch_writing(
        &lock,
        "fix/12-thing",
        "claude-aaaa1111",
        &issue,
        "2026-08-01T12:00Z",
        |_, _| Err(std::io::Error::other("the disk is full")),
    );
    assert!(
        matches!(failed, Err(crate::transport::Failure::Write(_))),
        "a failed write was not reported as a write failure: {failed:?}"
    );
    assert!(!lock.exists(), "a failed write left a lock behind");

    let retry = super::reserve_branch(
        &lock,
        "fix/12-thing",
        "claude-aaaa1111",
        &issue,
        "2026-08-01T12:01Z",
    )
    .expect("the same run was refused after its own failed write");
    let held = super::read_lock_record(&lock).expect("the lock is readable");
    assert_eq!(
        held.get("run_id").and_then(|id| id.as_str()),
        Some("claude-aaaa1111")
    );
    drop(retry);
    assert!(!lock.exists());
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
        let refusal = super::reserve_branch(
            &lock,
            "fix/12",
            "claude-bbbb2222",
            &crate::identity::Identity::Number(12),
            "2026-08-01T12:00Z",
        )
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
        RegisteredWorktree {
            path: legacy.clone(),
            branch: Some("old-branch".to_owned()),
        },
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
    detached.insert(
        super::normalise_path(&legacy),
        RegisteredWorktree {
            path: legacy.clone(),
            branch: None,
        },
    );
    let refusal = super::legacy_worktree_block(&legacy, &scoped, &detached)
        .expect("a detached legacy checkout is still a registration");
    let envelope = refusal.envelope();
    assert_eq!(
        envelope.get("reason").and_then(serde_json::Value::as_str),
        Some("legacy-worktree-registered"),
        "the stop lost its reason: {envelope}"
    );
    // The value, not only the reason. The binding tells an operator this reads
    // `occupied_by_branch: null` for a detached registration, and until this
    // assertion existed that half was held by nothing — replacing the field with
    // a literal left the whole suite green.
    assert_eq!(
        envelope.get("occupied_by_branch"),
        Some(&serde_json::Value::Null),
        "a detached registration did not report a null branch: {envelope}"
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
    // The checkout's directory carried the run and not the branch,
    // so a run working a queue met the checkout it made for its previous issue
    // and was refused `worktree-path-occupied` — correct for what the check
    // could see, wrong that two branches of one run were asked to share a path.
    //
    // The first attempt at this test picked `/w/<repo>/<branch>` and proved
    // nothing: a template that already names the branch is the one shape that
    // cannot exhibit the defect. So the templates here are the ones an operator
    // actually writes. The skill ships `Worktree location` as `unset`, so every
    // operator invents one, and an absolute directory with no placeholder in it
    // is accepted — the ordinary answer, and the shape that collides in both
    // dimensions at once.
    let run = "claude-81d69d3e372497b6";
    let path_of = |template: &str, branch: &str, issue: u64| {
        let (scoped, migrated) = crate::transport::worktree::scoped_template(template);
        assert!(migrated, "{template:?} was left as it was");
        crate::transport::worktree::worktree_path(
            &scoped,
            "estigia",
            branch,
            run,
            &crate::identity::Identity::Number(issue),
        )
        .expect("a path is composed")
    };

    for template in [
        // The reproduction, verbatim.
        r"H:\REPO\worktree\estigia",
        "/w/<repo>",
        // Run-scoped and branch-less: the half that was never migrated, and the
        // one whose legacy directory is a checkout this run owns.
        "/w/<repo>~<run-id>",
    ] {
        let first = path_of(template, "fix-1-publish-review", 1);
        let second = path_of(template, "fix-2-closed-issue-gates", 2);
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

#[test]
fn a_branch_another_checkout_holds_is_refused_by_name_before_git_is_asked() {
    let repo = tempfile::tempdir().expect("a clone");
    for argv in [
        vec!["git", "init", "--quiet", "-b", "main", "."],
        vec!["git", "config", "user.email", "estigia@example.invalid"],
        vec!["git", "config", "user.name", "Estigia test"],
    ] {
        crate::transport::run(&argv, Some(repo.path()), crate::transport::How::write())
            .expect("the repository is prepared");
    }
    std::fs::write(repo.path().join("main.txt"), "main\n").expect("main content");
    for argv in [
        vec!["git", "add", "main.txt"],
        vec!["git", "commit", "--quiet", "-m", "main"],
        vec!["git", "branch", "work", "HEAD"],
        vec!["git", "branch", "spare", "HEAD"],
    ] {
        crate::transport::run(&argv, Some(repo.path()), crate::transport::How::write())
            .expect("main is committed and the branches exist");
    }

    let predecessor = repo.path().join("checkout~run-one");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("worktree"),
            std::ffi::OsString::from("add"),
            std::ffi::OsString::from("--"),
            predecessor.as_os_str().to_owned(),
            std::ffi::OsString::from("work"),
        ],
        Some(repo.path()),
        crate::transport::How::write(),
    )
    .expect("the predecessor's checkout exists");

    let context = Context {
        skill_dir: repo.path().join("skill-does-not-exist"),
        repo_dir: repo.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let registered = registered_worktrees(&context).expect("git worktree list answers");
    let held = registered
        .values()
        .find(|entry| entry.branch.as_deref() == Some("work"))
        .map(|entry| entry.path.clone())
        .expect("the predecessor's checkout is registered");

    let successor = repo.path().join("checkout~run-two");
    let envelope = branch_pinned_elsewhere(&registered, &successor, "work")
        .expect("a successor asking for a branch another checkout holds is refused")
        .envelope();
    assert_eq!(envelope["reason"], "branch-checked-out-elsewhere");
    assert_eq!(
        envelope["path"],
        held.display().to_string(),
        "the refusal did not carry the path git listed"
    );

    assert!(
        branch_pinned_elsewhere(&registered, &held, "work").is_none(),
        "the checkout that already holds the branch was refused its own resume"
    );
    assert!(
        branch_pinned_elsewhere(&registered, &successor, "spare").is_none(),
        "a branch no checkout holds at all was reported as pinned"
    );
}

fn schedule_context(skill: &std::path::Path, repo: &std::path::Path, value: &str) -> Context {
    Context {
        skill_dir: skill.to_path_buf(),
        repo_dir: repo.to_path_buf(),
        config: vec![(
            "Git operation window".to_owned(),
            value.to_owned(),
            crate::transport::Source::Written,
        )],
        repo: None,
    }
}

fn contract_with(skill: &std::path::Path, value: &str) {
    std::fs::create_dir_all(skill).expect("a skill root");
    std::fs::write(
        skill.join(crate::skill::CONTRACT),
        format!(
            "| Setting | Value here | Skill default |\n|---|---|---|\n| Git operation window | {value} | disabled |\n"
        ),
    )
    .expect("a contract");
}

fn shell(at: &std::path::Path, arguments: &[&str]) {
    crate::transport::run(
        &["git"]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>(),
        Some(at),
        crate::transport::How::write(),
    )
    .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()));
}

fn start_what<'a>(branch: &'a str, worktree_root: Option<&'a str>) -> Start<'a> {
    Start {
        issue: crate::identity::Identity::Number(12),
        branch,
        base: "main",
        run_id: "claude-abcd1234",
        worktree_root,
        repo_name: "acme/app",
        expect_state: "in-progress",
        link: true,
        now: "2026-06-01T00:00Z",
    }
}

#[test]
fn the_started_branch_is_recorded_in_the_store_and_posts_no_comment() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: root.path().to_path_buf(),
        config: Vec::new(),
        repo: None,
    };
    let what = start_what("fix-12", None);
    let bin = crate::test_env::scripted_gh();
    let log = root.path().join("gh.log");

    let timeline = crate::test_env::with_logging_gh(bin.path(), &claim_script(), &log, || {
        let ledger = Ledger::open(&context, 12, what.now).expect("the ledger opens");
        super::record_started_branch(&ledger, &what).expect("the branch is recorded");
        ledger.timeline().expect("the timeline reads")
    });

    let recorded = timeline.last().expect("an entry");
    assert!(
        recorded.body.contains("issue-flow: branch") && recorded.body.contains("fix-12"),
        "the store does not hold the branch marker: {}",
        recorded.body
    );
    let calls = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    assert!(
        !calls.contains("issue comment"),
        "a started branch posts no comment to GitHub: {calls}"
    );
}

fn claim_script() -> String {
    crate::test_env::answers(&[crate::test_env::live_holder_issue(
        "claude-abcd1234",
        12,
        "in-progress",
    )])
}

std::thread_local! {
    static SCHEDULE_AT: std::cell::Cell<u16> = const { std::cell::Cell::new(0) };
}

fn schedule_clock() -> Option<u16> {
    SCHEDULE_AT.with(|cell| Some(cell.get()))
}

fn denying_at(minutes: u16, run: impl FnOnce()) {
    SCHEDULE_AT.with(|cell| cell.set(minutes));
    crate::harness::operation_window::with_now_override(Some(schedule_clock), run);
}

#[test]
fn start_branch_inside_the_period_reaches_the_checks_after_the_schedule() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join("skill");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    shell(&repo, &["init", "--quiet", "-b", "main"]);
    contract_with(&skill, "enabled 09:00-17:00");
    let context = schedule_context(&skill, &repo, "enabled 09:00-17:00");
    let what = start_what("fix/12", None);
    let bin = crate::test_env::scripted_gh();
    denying_at(12 * 60, || {
        let failure = crate::test_env::with_scripted_gh(bin.path(), &claim_script(), || {
            super::start_branch(&context, &what, None)
        })
        .expect_err("a start without a worktree location passed");
        assert_eq!(
            failure.envelope()["reason"],
            "no-worktree-location",
            "noon did not reach the checks after the schedule: {}",
            failure.envelope()
        );
    });
}

#[test]
fn start_branch_outside_the_period_reaches_the_checks_after_the_schedule() {
    let root = tempfile::tempdir().expect("a temporary root");
    let skill = root.path().join("skill");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    shell(&repo, &["init", "--quiet", "-b", "main"]);
    contract_with(&skill, "enabled 09:00-17:00");
    let context = schedule_context(&skill, &repo, "enabled 09:00-17:00");
    let what = start_what("fix/12", None);
    let bin = crate::test_env::scripted_gh();
    denying_at(20 * 60, || {
        let failure = crate::test_env::with_scripted_gh(bin.path(), &claim_script(), || {
            super::start_branch(&context, &what, None)
        })
        .expect_err("a start without a worktree location passed");
        assert_eq!(
            failure.envelope()["reason"],
            "no-worktree-location",
            "a closed window stopped a start before the checks that follow the schedule: {}",
            failure.envelope()
        );
    });
}

/// A repository with one commit and a registered checkout of `work` beside it.
fn delivered_checkout() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
) {
    let root = tempfile::tempdir().expect("a fixture root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    shell(&repo, &["init", "--quiet", "-b", "main", "."]);
    shell(&repo, &["config", "user.email", "estigia@example.invalid"]);
    shell(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join("main.txt"), "main\n").expect("main content");
    shell(&repo, &["add", "main.txt"]);
    shell(&repo, &["commit", "--quiet", "-m", "main"]);
    let head = super::object_at(&repo, "HEAD").expect("the repository head");
    shell(&repo, &["branch", "work", "HEAD"]);
    let checkout = root.path().join("checkout");
    crate::transport::run_os(
        &[
            std::ffi::OsString::from("git"),
            std::ffi::OsString::from("worktree"),
            std::ffi::OsString::from("add"),
            std::ffi::OsString::from("--"),
            checkout.as_os_str().to_owned(),
            std::ffi::OsString::from("work"),
        ],
        Some(&repo),
        crate::transport::How::write(),
    )
    .expect("the checkout is registered");
    (root, repo, checkout, head)
}

#[test]
fn a_branch_with_a_separator_lives_at_its_encoded_path_from_start_to_release() {
    let root = tempfile::tempdir().expect("a fixture root");
    let remote = root.path().join("remote.git");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("the repository directory");
    shell(root.path(), &["init", "--quiet", "--bare", "remote.git"]);
    shell(&repo, &["init", "--quiet", "-b", "main", "."]);
    shell(&repo, &["config", "user.email", "estigia@example.invalid"]);
    shell(&repo, &["config", "user.name", "Estigia test"]);
    std::fs::write(repo.join("main.txt"), "main\n").expect("main content");
    shell(&repo, &["add", "main.txt"]);
    shell(&repo, &["commit", "--quiet", "-m", "main"]);
    shell(
        &repo,
        &["remote", "add", "origin", &remote.display().to_string()],
    );
    shell(&repo, &["push", "--quiet", "-u", "origin", "main"]);

    let branch = "docs/931-hold-max-days-ledger-status";
    let template = root
        .path()
        .join("wt")
        .join("<branch>")
        .display()
        .to_string();
    let context = retire_context(&repo);
    let what = start_what(branch, Some(&template));

    let first = super::reserve_checkout(&context, &what, None).expect("the first start");
    let expected = root
        .path()
        .join("wt")
        .join("docs+s931-hold-max-days-ledger-status~claude-abcd1234");
    assert!(!first.resuming);
    assert_eq!(
        super::canonical_worktree_path(&first.path).expect("the checkout resolves"),
        super::canonical_worktree_path(&expected).expect("the expected path resolves"),
    );
    assert!(first.path.join("main.txt").exists(), "no checkout was made");
    drop(first);

    let second = super::reserve_checkout(&context, &what, None).expect("the resume");
    assert!(second.resuming, "the encoded checkout was not re-adopted");
    let path = second.path.clone();
    drop(second);

    let head = super::object_at(&path, "HEAD").expect("the checkout head");
    let retired = super::retire_checkout(&context, &path, &head);
    assert!(retired.worktree_removed, "the checkout stayed: {retired:?}");
    assert_eq!(retired.branch.as_deref(), Some(branch));
    assert!(!path.exists(), "the directory survived the release");
}

fn retire_context(repo: &std::path::Path) -> Context {
    Context {
        skill_dir: repo.join("skill-does-not-exist"),
        repo_dir: repo.to_path_buf(),
        config: Vec::new(),
        repo: None,
    }
}

#[test]
fn a_delivered_checkout_is_put_down_with_its_branch() {
    let (_root, repo, checkout, head) = delivered_checkout();
    let retired = super::retire_checkout(&retire_context(&repo), &checkout, &head);
    assert!(retired.worktree_removed, "the checkout stayed: {retired:?}");
    assert!(retired.branch_deleted, "the branch stayed: {retired:?}");
    assert_eq!(retired.branch.as_deref(), Some("work"));
    assert!(retired.reason.is_none(), "something was kept: {retired:?}");
    assert!(!checkout.exists(), "the directory survived the release");

    let listing = crate::transport::run(
        &["git", "worktree", "list", "--porcelain"],
        Some(&repo),
        crate::transport::How::read(),
    )
    .expect("the registry is read");
    assert!(
        !listing.stdout.contains(&checkout.display().to_string()),
        "the checkout is still registered: {}",
        listing.stdout
    );
    let branches = crate::transport::run(
        &["git", "branch", "--list", "work"],
        Some(&repo),
        crate::transport::How::read(),
    )
    .expect("the branches are read");
    assert!(
        branches.stdout.trim().is_empty(),
        "the branch survived: {}",
        branches.stdout
    );
}

/// The workflow tells a run to work inside its own
/// isolated checkout, so `release` can be launched there and the context's
/// `repo_dir` is the directory being removed. Both destructive commands must run
/// from outside it, or `git worktree remove` refuses its own cwd and the local
/// branch survives.
#[test]
fn a_release_launched_from_inside_the_checkout_still_puts_it_down() {
    let (_root, repo, checkout, head) = delivered_checkout();
    let retired = super::retire_checkout(&retire_context(&checkout), &checkout, &head);
    assert!(
        retired.worktree_removed,
        "a release launched from its own checkout kept the directory: {retired:?}"
    );
    assert!(
        retired.branch_deleted,
        "a release launched from its own checkout kept the branch: {retired:?}"
    );
    assert!(retired.reason.is_none(), "something was kept: {retired:?}");
    assert!(!checkout.exists(), "the directory survived the release");
    let branches = crate::transport::run(
        &["git", "branch", "--list", "work"],
        Some(&repo),
        crate::transport::How::read(),
    )
    .expect("the branches are read");
    assert!(
        branches.stdout.trim().is_empty(),
        "the branch survived: {}",
        branches.stdout
    );
}

/// The partial case the `removed` conjunction cannot name: the directory went
/// and the branch stayed. A consumer reading `removed: false` would take that
/// for "nothing moved".
#[test]
fn a_partial_retirement_names_the_half_that_stayed() {
    let retired = super::Retired {
        worktree: std::path::PathBuf::from("/tmp/checkout"),
        worktree_removed: true,
        branch: Some("work".to_owned()),
        branch_deleted: false,
        reason: Some("the branch work could not be deleted: boom".to_owned()),
    };
    let answer = retired.to_json();
    assert_eq!(answer["removed"], serde_json::json!(false));
    assert_eq!(
        answer["outcome"],
        serde_json::json!("worktree-removed-branch-kept")
    );
    assert_eq!(answer["worktree_removed"], serde_json::json!(true));
    assert_eq!(answer["branch_deleted"], serde_json::json!(false));
    assert_eq!(answer["branch"], serde_json::json!("work"));
}

#[test]
fn a_checkout_carrying_unpublished_work_is_kept() {
    let (_root, repo, checkout, head) = delivered_checkout();
    std::fs::write(checkout.join("scratch.txt"), "not committed\n").expect("scratch content");

    let retired = super::retire_checkout(&retire_context(&repo), &checkout, &head);
    assert!(
        !retired.worktree_removed,
        "a checkout with untracked work was removed: {retired:?}"
    );
    assert!(
        retired
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("uncommitted")),
        "the kept checkout did not name why: {retired:?}"
    );
    assert!(checkout.exists(), "the directory was removed anyway");
    let branches = crate::transport::run(
        &["git", "branch", "--list", "work"],
        Some(&repo),
        crate::transport::How::read(),
    )
    .expect("the branches are read");
    assert!(
        branches.stdout.contains("work"),
        "the branch was deleted under work that stayed: {}",
        branches.stdout
    );
}

#[test]
fn a_checkout_at_other_bytes_than_the_delivery_is_kept() {
    let (_root, repo, checkout, _head) = delivered_checkout();
    let other = "a".repeat(40);
    let retired = super::retire_checkout(&retire_context(&repo), &checkout, &other);
    assert!(
        !retired.worktree_removed,
        "a checkout at a head nobody delivered was removed: {retired:?}"
    );
    assert!(
        retired
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("not at the delivered head")),
        "the kept checkout did not name why: {retired:?}"
    );
    assert!(checkout.exists(), "the directory was removed anyway");
}

struct AssistRig {
    root: tempfile::TempDir,
    repo: std::path::PathBuf,
    context: Context,
}

fn assist_rig() -> AssistRig {
    let root = tempfile::tempdir().expect("a temporary root");
    let origin = root.path().join("origin.git");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&origin).expect("a remote directory");
    std::fs::create_dir_all(&repo).expect("a repository directory");
    shell(&origin, &["init", "--quiet", "--bare", "-b", "main"]);
    shell(&repo, &["init", "--quiet", "-b", "main"]);
    shell(&repo, &["config", "user.email", "estigia@example.invalid"]);
    shell(&repo, &["config", "user.name", "Estigia test"]);
    shell(
        &repo,
        &["remote", "add", "origin", &origin.to_string_lossy()],
    );
    shell(&repo, &["commit", "--allow-empty", "--quiet", "-m", "base"]);
    shell(&repo, &["push", "--quiet", "origin", "main"]);
    let context = Context {
        skill_dir: root.path().join("skill"),
        repo_dir: repo.clone(),
        config: Vec::new(),
        repo: None,
    };
    AssistRig {
        root,
        repo,
        context,
    }
}

fn repository_state(repo: &std::path::Path) -> (String, String) {
    let read = |arguments: &[&str]| {
        crate::transport::run(
            &["git"]
                .into_iter()
                .chain(arguments.iter().copied())
                .collect::<Vec<_>>(),
            Some(repo),
            crate::transport::How::read(),
        )
        .expect("git answers")
        .stdout
    };
    (read(&["branch", "--list"]), read(&["worktree", "list"]))
}

fn assist_answers(view: serde_json::Value) -> String {
    crate::test_env::answers(&[crate::test_env::issue_view_answer(12, view)])
}

#[test]
fn assist_reserves_the_checkout_and_asks_the_tracker_only_to_read() {
    let rig = assist_rig();
    let trees = rig.root.path().join("trees");
    let trees = trees.to_string_lossy().into_owned();
    let what = start_what("issue-12", Some(&trees));
    let bin = crate::test_env::scripted_gh();
    let log = rig.root.path().join("gh.log");
    let script = assist_answers(serde_json::json!({ "state": "OPEN", "title": "A title" }));
    let answer = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
        super::assist(&rig.context, &what, None)
    })
    .expect("the checkout is reserved");
    assert_eq!(answer["tracker_written"], false);
    assert_eq!(answer["title"], "A title");
    let worktree = std::path::PathBuf::from(answer["worktree"].as_str().expect("a worktree"));
    assert!(worktree.is_dir(), "no worktree was made");
    let marker = read_ownership(&worktree)
        .expect("the marker reads")
        .expect("the marker exists");
    assert_eq!(marker["run_id"], what.run_id);
    assert_eq!(marker["branch"], "issue-12");

    let asked = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
    let verbs: Vec<&str> = asked
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default())
        .collect();
    assert!(
        !verbs.is_empty() && verbs.iter().all(|line| line.starts_with("issue view 12 ")),
        "assist asked gh for something other than reading the issue: {verbs:?}"
    );
}

#[test]
fn assist_never_links_the_branch_whatever_the_start_says() {
    for link in [true, false] {
        let rig = assist_rig();
        let trees = rig.root.path().join("trees");
        let trees = trees.to_string_lossy().into_owned();
        let what = Start {
            link,
            ..start_what("issue-12", Some(&trees))
        };
        let bin = crate::test_env::scripted_gh();
        let log = rig.root.path().join("gh.log");
        let script = assist_answers(serde_json::json!({ "state": "OPEN", "title": "A title" }));
        let answer = crate::test_env::with_logging_gh(bin.path(), &script, &log, || {
            super::assist(&rig.context, &what, None)
        })
        .expect("the checkout is reserved");
        assert_eq!(answer["tracker_written"], false, "link: {link}");
        assert!(
            answer.get("link_outcome").is_none(),
            "link: {link}: {answer}"
        );
        let asked = std::fs::read_to_string(&log).expect("the fake gh logged its calls");
        assert!(
            !asked.contains("issue develop"),
            "assist linked the branch with link: {link}: {asked}"
        );
    }
}

#[test]
fn assist_stops_before_any_repository_write_when_the_issue_cannot_be_read() {
    let rig = assist_rig();
    let trees = rig.root.path().join("trees");
    let trees_arg = trees.to_string_lossy().into_owned();
    let what = start_what("issue-12", Some(&trees_arg));
    let before = repository_state(&rig.repo);
    let bin = crate::test_env::scripted_gh();
    let unreadable = serde_json::json!([{
        "matches": "view 12 --json",
        "stdout": "",
        "status": 1,
    }])
    .to_string();
    let failure = crate::test_env::with_scripted_gh(bin.path(), &unreadable, || {
        super::assist(&rig.context, &what, None)
    })
    .expect_err("an unreadable issue was worked");
    assert_eq!(reason(failure), "assisted-issue-unreadable");
    assert_eq!(
        before,
        repository_state(&rig.repo),
        "a branch or worktree was left behind"
    );
    assert!(!trees.exists(), "a worktree directory was made");
    assert!(
        !std::fs::read_dir(rig.repo.join(".git"))
            .expect("the git directory")
            .any(|entry| entry.is_ok_and(|entry| entry.file_name() == "worktrees")),
        "the worktree registry was written"
    );
}

#[test]
fn assist_refuses_a_closed_issue_before_any_repository_write() {
    let rig = assist_rig();
    let trees = rig.root.path().join("trees");
    let trees_arg = trees.to_string_lossy().into_owned();
    let what = start_what("issue-12", Some(&trees_arg));
    let before = repository_state(&rig.repo);
    let bin = crate::test_env::scripted_gh();
    let script = assist_answers(serde_json::json!({ "state": "CLOSED", "title": "A title" }));
    let failure = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        super::assist(&rig.context, &what, None)
    })
    .expect_err("a closed issue was worked");
    assert_eq!(reason(failure), "issue-not-open");
    assert_eq!(before, repository_state(&rig.repo));
    assert!(!trees.exists(), "a worktree directory was made");
}

#[test]
fn assist_refuses_a_key_from_another_team_before_any_repository_write() {
    let mut rig = assist_rig();
    rig.context.config = vec![(
        "Tracker".to_owned(),
        "linear ENG".to_owned(),
        crate::transport::Source::Written,
    )];
    let trees = rig.root.path().join("trees");
    let trees_arg = trees.to_string_lossy().into_owned();
    let mut what = start_what("issue-plat-9", Some(&trees_arg));
    what.issue = crate::identity::Identity::Key("PLAT-9".to_owned());
    let before = repository_state(&rig.repo);
    let failure = super::assist(&rig.context, &what, None).expect_err("another team was worked");
    assert_eq!(reason(failure), "linear-team-mismatch");
    assert_eq!(before, repository_state(&rig.repo));
    assert!(!trees.exists(), "a worktree directory was made");
}

#[test]
fn assist_refuses_issue_number_zero() {
    let rig = assist_rig();
    let trees = rig.root.path().join("trees");
    let trees_arg = trees.to_string_lossy().into_owned();
    let mut what = start_what("issue-0", Some(&trees_arg));
    what.issue = crate::identity::Identity::Number(0);
    let failure = super::assist(&rig.context, &what, None).expect_err("issue 0 was worked");
    assert_eq!(reason(failure), "malformed-argument");
    assert!(!trees.exists(), "a worktree directory was made");
}
