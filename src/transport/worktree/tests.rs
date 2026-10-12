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
fn a_run_id_is_checked_and_never_repaired() {
    assert_eq!(
        run_component("claude-a1").expect("well formed"),
        "claude-a1"
    );
    assert_eq!(run_component("a").expect("one group"), "a");

    // Every one of these could be *made* well formed. None of them is, because
    // the repair is what folds two IDs into one directory.
    for bad in [
        "",          // nothing to name a directory with
        "Claude-A",  // case folds on the filesystem that has to keep them apart
        "claude_a",  // underscore is not in the alphabet
        "claude--a", // an empty group: `claude-a` after any tidy-up
        "-claude",   // leading separator
        "claude-",   // trailing separator
        "claude a",  // a space
        "claude/a",  // a separator, which would restructure the path
        "claude.a",  // a dot, which reaches the device branch
    ] {
        assert_eq!(
            reason(run_component(bad).expect_err(&format!("{bad:?} is not a run ID"))),
            "unsafe-run-id",
            "{bad:?} was accepted"
        );
    }
}

#[test]
fn a_device_name_is_refused_however_it_is_dressed() {
    // Bare, in any case, and with any extension: Windows resolves all of them
    // to the device rather than to a directory.
    for name in [
        "con",
        "NUL",
        "Com1",
        "lpt9",
        "aux.txt",
        "nul.log",
        "COM3.anything",
    ] {
        assert_eq!(
            reason(assert_safe_component(name, "branch").expect_err("a device")),
            "reserved-device-component",
            "{name:?} was accepted"
        );
    }
    // The check is on the part before the *first* dot, and nothing else is a
    // device: a name that merely starts with those letters is a directory.
    for name in [
        "console",
        "connect",
        "com10",
        "nullable",
        "auxiliary",
        "a.con",
    ] {
        assert!(
            assert_safe_component(name, "branch").is_ok(),
            "{name:?} was refused"
        );
    }
}

#[test]
fn a_branch_only_template_is_migrated_to_a_sibling_and_not_a_child() {
    let (scoped, migrated) = scoped_template("/w/<repo>/<branch>");
    assert!(migrated);
    assert_eq!(scoped, "/w/<repo>/<branch>~<run-id>");
    // A sibling: the legacy path is neither a parent of nor a child of it.
    assert!(!scoped.starts_with("/w/<repo>/<branch>/"));

    // A trailing separator does not produce an empty segment.
    assert_eq!(scoped_template("/w/<branch>/").0, "/w/<branch>~<run-id>");
    assert_eq!(
        scoped_template("C:\\w\\<branch>\\").0,
        "C:\\w\\<branch>~<run-id>"
    );

    // Already scoped in **both** dimensions: untouched, wherever the
    // placeholders sit. This list used to hold `/w/<run-id>` and `<run-id>`
    // too, on the grounds that a run-scoped template was already scoped — which
    // is that defect written into a test. A template naming the run
    // and not the branch gives every branch of one run the same directory, so
    // it is exactly as unmigrated as a branch-only one.
    for already in ["/w/<run-id>/<branch>", "<branch>~<run-id>"] {
        assert_eq!(
            scoped_template(already),
            (already.to_owned(), false),
            "{already:?} was migrated twice"
        );
    }

    // And the other half of the same rule: a run-scoped template with no branch
    // gains one, as a sibling, for the reason the branch-only case gains a run.
    // The sibling matters more here — the legacy path is a checkout this run
    // owns, so a nested child would be a worktree inside a worktree.
    for (template, expected) in [
        ("/w/<run-id>", "/w/<run-id>~<branch>"),
        ("<run-id>", "<run-id>~<branch>"),
    ] {
        let (scoped, migrated) = scoped_template(template);
        assert!(migrated, "{template:?} was left colliding");
        assert_eq!(scoped, expected);
        assert!(
            !scoped.starts_with(&format!("{template}/")),
            "the migration nested the new checkout inside the legacy one"
        );
    }

    // A template naming neither gains both, and the run stays outermost.
    assert_eq!(
        scoped_template("/w/<repo>"),
        ("/w/<repo>~<branch>~<run-id>".to_owned(), true)
    );
    assert_eq!(
        scoped_template(r"H:\REPO\worktree\estigia").0,
        r"H:\REPO\worktree\estigia~<branch>~<run-id>"
    );
}

#[test]
fn the_join_makes_the_composed_name_unambiguous() {
    // The pair that motivates `~`. With `-` as the join both spell the same
    // directory, and they are two different branches taking two different
    // branch locks — so nothing downstream would catch the collision.
    let one = worktree_path(
        "<branch>~<run-id>",
        "r",
        "fix-6",
        "a-b",
        &crate::identity::Identity::Number(1),
    )
    .expect("one");
    let two = worktree_path(
        "<branch>~<run-id>",
        "r",
        "fix-6-a",
        "b",
        &crate::identity::Identity::Number(1),
    )
    .expect("two");
    assert_ne!(one, two);
    assert_eq!(one, std::path::PathBuf::from("fix-6~a-b"));
    assert_eq!(two, std::path::PathBuf::from("fix-6-a~b"));
}

#[test]
fn branches_that_differ_only_in_separators_get_distinct_components() {
    let (scoped, _) = scoped_template("/w/<repo>");
    let path_of = |branch: &str| {
        worktree_path(
            &scoped,
            "estigia",
            branch,
            "run-x",
            &crate::identity::Identity::Number(1),
        )
        .unwrap_or_else(|failure| panic!("{branch:?} was refused: {}", failure.detail()))
    };
    let branches = [
        "fix/1-a", "fix-1-a", "fix//1-a", "fix\\1-a", "fix+1-a", "fix+s1-a",
    ];
    let paths: Vec<_> = branches.iter().map(|branch| path_of(branch)).collect();
    for (index, left) in paths.iter().enumerate() {
        for (other, right) in paths.iter().enumerate().skip(index + 1) {
            assert_ne!(
                left, right,
                "{:?} and {:?} share a directory",
                branches[index], branches[other]
            );
        }
    }
    assert_eq!(
        path_of("fix/1-a"),
        std::path::PathBuf::from("/w/estigia~fix+s1-a~run-x")
    );
}

#[test]
fn placeholder_text_inside_a_branch_is_never_substituted_again() {
    let five = crate::identity::Identity::Number(5);
    let run =
        worktree_path("/w/<branch>~<run-id>", "r", "a<run-id>", "rr", &five).expect("composes");
    let plain = worktree_path("/w/<branch>~<run-id>", "r", "arr", "rr", &five).expect("composes");
    assert_ne!(run, plain);
    assert_eq!(run, std::path::PathBuf::from("/w/a<run-id>~rr"));

    let issue = worktree_path("/w/<branch>~<run-id>~<issue>", "r", "a<issue>", "rr", &five)
        .expect("composes");
    let number =
        worktree_path("/w/<branch>~<run-id>~<issue>", "r", "a5", "rr", &five).expect("composes");
    assert_ne!(issue, number);
    assert_eq!(issue, std::path::PathBuf::from("/w/a<issue>~rr~5"));

    let repo =
        worktree_path("/w/<repo>~<branch>", "x<branch>", "b", "rr", &five).expect("composes");
    assert_eq!(repo, std::path::PathBuf::from("/w/x<branch>~b"));
}

#[test]
fn an_unknown_or_unclosed_placeholder_is_left_as_written() {
    let five = crate::identity::Identity::Number(5);
    assert_eq!(
        worktree_path("/w/<other>/<branch/<run-id>", "r", "b", "rr", &five).expect("composes"),
        std::path::PathBuf::from("/w/<other>/<branch/rr")
    );
}

#[test]
fn a_component_that_trim_would_change_is_refused_and_not_given_its_twin() {
    let five = crate::identity::Identity::Number(5);
    for branch in ["\u{a0}fix", "fix\u{2003}", " fix", "fix\t ", "\u{3000}"] {
        assert_eq!(
            reason(worktree_path("<branch>", "r", branch, "rr", &five).expect_err("refused")),
            "unsafe-worktree-component",
            "{branch:?}"
        );
    }
    assert_eq!(
        worktree_path("<repo>", "r", "fix", "rr", &five).expect("composes"),
        std::path::PathBuf::from("r")
    );
    assert_eq!(
        reason(worktree_path("<repo>", "\u{a0}r", "fix", "rr", &five).expect_err("refused")),
        "unsafe-worktree-component"
    );
}

#[test]
fn the_branch_component_escapes_the_escape_and_both_separators() {
    assert_eq!(branch_component("docs/931-hold"), "docs+s931-hold");
    assert_eq!(branch_component("a\\b"), "a+bb");
    assert_eq!(branch_component("a+b"), "a+pb");
    assert_eq!(branch_component("a+/b"), "a+p+sb");
    assert_eq!(branch_component("plain-branch.1"), "plain-branch.1");
}

#[test]
fn a_branch_without_the_escape_or_a_separator_keeps_the_path_it_always_had() {
    for branch in ["fix-1-a", "Fix_2.b", "feature~x", "naïve", "a b"] {
        assert_eq!(branch_component(branch), branch);
        assert_eq!(
            worktree_path(
                "/w/<branch>~<run-id>",
                "r",
                branch,
                "run-x",
                &crate::identity::Identity::Number(1),
            )
            .expect("composes"),
            std::path::PathBuf::from(format!("/w/{}~run-x", branch.trim()))
        );
    }
}

#[test]
fn an_encoded_branch_is_still_checked_for_a_climb_and_a_device() {
    for (branch, expected) in [
        ("a/../b", "unsafe-worktree-component"),
        ("nul", "reserved-device-component"),
        ("nul.txt", "reserved-device-component"),
    ] {
        assert_eq!(
            reason(
                worktree_path(
                    "<branch>",
                    "r",
                    branch,
                    "r-1",
                    &crate::identity::Identity::Number(1)
                )
                .expect_err("refused")
            ),
            expected,
            "{branch:?}"
        );
    }
}

#[test]
fn a_flattened_component_refuses_a_climb_and_a_device() {
    let path = worktree_path(
        "<repo>/<branch>/<issue>/<run-id>",
        "a/b",
        "fix-x",
        "r-1",
        &crate::identity::Identity::Number(42),
    )
    .expect("resolved");
    assert_eq!(path, std::path::PathBuf::from("a-b/fix-x/42/r-1"));

    // Backslashes too, and a run of separators collapses to one `-` rather
    // than to a run of them.
    assert_eq!(
        worktree_path(
            "<repo>",
            "a\\\\b//c",
            "fix-x",
            "r-1",
            &crate::identity::Identity::Number(1)
        )
        .expect("resolved"),
        std::path::PathBuf::from("a-b-c")
    );

    // `..` is refused rather than flattened: flattening it would keep a path
    // that climbs out of the configured root.
    assert_eq!(
        reason(
            worktree_path(
                "<repo>",
                "../escape",
                "fix-x",
                "r-1",
                &crate::identity::Identity::Number(1)
            )
            .expect_err("climbs")
        ),
        "unsafe-worktree-component"
    );
    // Separators alone are not empty: they flatten to `-`, which is a name a
    // directory can have. Only something that leaves *nothing* is refused.
    assert_eq!(
        worktree_path(
            "<repo>",
            "///",
            "fix-x",
            "r-1",
            &crate::identity::Identity::Number(1)
        )
        .expect("a dash"),
        std::path::PathBuf::from("-")
    );
    for nothing in ["", "   "] {
        assert_eq!(
            reason(
                worktree_path(
                    "<repo>",
                    nothing,
                    "fix-x",
                    "r-1",
                    &crate::identity::Identity::Number(1)
                )
                .expect_err("empty")
            ),
            "unsafe-worktree-component",
            "{nothing:?} named a repository directory"
        );
        assert_eq!(
            reason(
                worktree_path(
                    "<branch>",
                    "r",
                    nothing,
                    "r-1",
                    &crate::identity::Identity::Number(1)
                )
                .expect_err("empty")
            ),
            "unsafe-worktree-component",
            "{nothing:?} named a branch directory"
        );
    }
    // The device rule reaches the flattened components, not just the run ID.
    assert_eq!(
        reason(
            worktree_path(
                "<branch>",
                "r",
                "nul",
                "r-1",
                &crate::identity::Identity::Number(1)
            )
            .expect_err("a device")
        ),
        "reserved-device-component"
    );
    assert_eq!(
        reason(
            worktree_path(
                "<repo>",
                "nul",
                "fix-x",
                "r-1",
                &crate::identity::Identity::Number(1)
            )
            .expect_err("a device")
        ),
        "reserved-device-component"
    );
}

#[test]
fn advancing_the_local_base_refuses_every_state_it_cannot_prove_safe() {
    // Nothing to move: the caller has no local base branch at all, or it is
    // already at the remote tip. Reported as Current rather than as a refusal,
    // because a refusal names a problem and there is none.
    assert_eq!(
        base_advance(None, "aaa", None, false, false, false),
        BaseAdvance::Current
    );
    assert_eq!(
        base_advance(Some("aaa"), "aaa", Some(true), true, true, false),
        BaseAdvance::Current
    );

    // The whole point: a base with commits the remote has never seen is NOT
    // fast-forwardable, and moving the ref would drop them on the floor. This
    // is `branch_start_point`'s `--force` lesson at a different ref.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", Some(false), false, false, false),
        BaseAdvance::Hold("base-diverged")
    );

    // Ancestry could not be established. "An unknown result is not clearance"
    // — an unreadable answer must not read as a safe one.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", None, false, false, false),
        BaseAdvance::Hold("ancestry-unknown")
    );

    // Checked out with a dirty tree: moving the ref underneath a working tree
    // that has changes leaves the index describing a commit the files do not
    // match, and every later `git status` lies about what this checkout holds.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", Some(true), true, true, false),
        BaseAdvance::Hold("base-checked-out-dirty")
    );

    // Checked out in ANOTHER worktree, which is this crate's ordinary state:
    // `start-branch` exists to make linked checkouts. `git update-ref` does not
    // refuse this — measured, exit 0 — and the worktree nobody touched then
    // reports `D <file>`, a staged deletion of a file its operator never
    // removed. Clean or dirty is not even asked: the tree that would be
    // corrupted is not the one this run can see.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", Some(true), false, false, true),
        BaseAdvance::Hold("base-checked-out-elsewhere")
    );

    // The two cases that may proceed, and they are not the same operation.
    // Not checked out: the ref moves and no file on disk changes.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", Some(true), false, false, false),
        BaseAdvance::UpdateRef
    );
    // Checked out and clean: git has to move index and files with the ref, so
    // it is a different command and gets its own verdict rather than sharing one.
    assert_eq!(
        base_advance(Some("aaa"), "bbb", Some(true), true, false, false),
        BaseAdvance::FastForwardCheckout
    );
}

#[test]
fn a_resume_never_moves_a_ref_that_may_hold_unpushed_work() {
    // A local branch is authoritative and is left alone. This is the case the
    // original got wrong with `--force`, which rewound the branch to the base
    // and discarded every commit on it — in exactly the situation that has
    // commits to lose.
    assert_eq!(branch_start_point(true, false, "fix/6", "main"), None);
    assert_eq!(branch_start_point(true, true, "fix/6", "main"), None);

    // No local branch but a published one: start from the published work, not
    // from the base. Starting at the base silently restarts it from zero.
    assert_eq!(
        branch_start_point(false, true, "fix/6", "main").as_deref(),
        Some("origin/fix/6")
    );
    // Neither exists: genuinely new, and the base is right.
    assert_eq!(
        branch_start_point(false, false, "fix/6", "main").as_deref(),
        Some("origin/main")
    );
}

#[test]
fn three_heads_have_to_tell_one_story() {
    let base = Some("aaa");

    // Nothing published: nothing to disagree with.
    assert_eq!(
        branch_identity_verdict(true, Some("aaa"), None, base, None),
        (true, "local-only")
    );

    // Fresh is the strict case: local == remote == base, exactly.
    assert_eq!(
        branch_identity_verdict(true, Some("aaa"), Some("aaa"), base, None),
        (true, "fresh-at-base")
    );
    // The server branched from a base this run never saw. Reporting the
    // recorded base would be a lie every later "reviewed-base" claim inherits.
    assert_eq!(
        branch_identity_verdict(true, Some("aaa"), Some("bbb"), base, None),
        (false, "base-moved-during-creation")
    );
    // Local is not at the base either: a different story again, and told apart
    // so the two send the caller somewhere different.
    assert_eq!(
        branch_identity_verdict(true, Some("ccc"), Some("bbb"), base, None),
        (false, "fresh-branch-diverged")
    );
    // No base recorded at all cannot be "fresh at base", whatever else matches.
    assert!(
        !branch_identity_verdict(true, None, Some("aaa"), None, None).0,
        "a fresh branch with no recorded base was called coherent"
    );

    // A resume is looser in one direction only.
    assert_eq!(
        branch_identity_verdict(false, Some("aaa"), Some("aaa"), base, None),
        (true, "resumed-in-sync")
    );
    // Local ahead of remote is ordinary unpushed work.
    assert_eq!(
        branch_identity_verdict(false, Some("bbb"), Some("aaa"), base, Some(true)),
        (true, "resumed-local-ahead")
    );
    // Remote ahead is not: somebody else pushed, and continuing would build on
    // a head this checkout has never seen.
    assert_eq!(
        branch_identity_verdict(false, Some("aaa"), Some("bbb"), base, Some(false)),
        (false, "remote-not-reachable-from-local")
    );
    // And an ancestry that could not be established is not a pass.
    assert_eq!(
        branch_identity_verdict(false, Some("aaa"), Some("bbb"), base, None),
        (false, "ancestry-unknown")
    );
}
