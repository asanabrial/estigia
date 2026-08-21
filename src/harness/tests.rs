use serde_json::json;

use super::*;

/// A gate context pointing at a skill root that carries no transport.
///
/// Enough to exercise every path that decides *before* the tracker is asked,
/// which is most of them — and the ones that do reach the tracker are the ones
/// that must refuse when it is not there.
fn context(root: &Path) -> GateContext {
    GateContext {
        stand_down: None,
        integration: crate::config::Integration::Branch,
        evidence: crate::config::Evidence::Reading,
        flag: None,
        skill_root: root.join("skill"),
        repo_dir: root.join("repo"),
        state_root: root.join("state"),
        window: RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        boundaries: Vec::new(),
    }
}

fn sworn(issue: u64, repo_dir: &Path) -> Run {
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(issue);
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo_dir.to_path_buf());
    run
}

#[test]
fn an_edit_is_a_routine_write() {
    let (action, how) = classify("Edit", &json!({"file_path": "src/main.rs"}));
    assert_eq!(
        action,
        Action::Write {
            target: "src/main.rs".to_owned()
        }
    );
    assert_eq!(how, Sensitivity::Routine);
}

#[test]
fn a_pr_merge_keeps_the_pull_request_identity_it_will_deliver() {
    let (first, _) = classify("Bash", &json!({"command": "gh pr merge 54 --merge"}));
    let (second, _) = classify("Bash", &json!({"command": "gh pr merge 55 --merge"}));

    assert_eq!(first.subject(), Some("gh pr merge 54".to_owned()));
    assert_eq!(second.subject(), Some("gh pr merge 55".to_owned()));
    assert_ne!(
        first.subject(),
        second.subject(),
        "different PR lineages collapsed to the same delivery identity"
    );

    for command in [
        "gh pr merge",
        "gh pr merge 0",
        "gh pr merge feature/ready",
        "gh pr merge https://github.com/o/r/pull/54",
        "gh pr merge 54 --repo foreign/repo",
        "gh pr merge 54 --repo=foreign/repo",
        "gh pr merge 54 -R foreign/repo",
        "gh pr merge 54 -Rforeign/repo",
        "gh pr merge 54 -dRforeign-owner/foreign-repo",
        "gh pr merge 54 -mRforeign-owner/foreign-repo",
        "gh pr merge 54 -rRforeign-owner/foreign-repo",
        "gh pr merge 54 -sRforeign-owner/foreign-repo",
        "gh pr merge 54 \"--repo\" foreign/repo",
        "gh pr merge 54 '-Rforeign/repo'",
        "gh pr merge 54 --re\\po foreign/repo",
        "gh pr merge 54 --re\"\"po foreign/repo",
        "gh pr merge 54 --re^po foreign/repo",
        "gh pr merge 54 --re{po,view} foreign/repo",
        "gh pr merge 54 --re* foreign/repo",
        "gh pr merge 54 --re?? foreign/repo",
        "gh pr merge 54 --re[p]o foreign/repo",
        "gh pr merge 54 %REPO_OPTION% foreign/repo",
        "gh pr merge 54 !REPO_OPTION! foreign/repo",
        "gh pr merge 54 @repo_options",
        "gh pr merge 54 --repo,foreign/repo",
        "gh pr merge 54 55",
        "gh pr merge 54 && gh pr merge 55",
        "gh pr merge 54 $(gh pr merge 55)",
        "gh pr merge 54 `gh pr merge 55`",
        "gh pr merge 54 <(gh pr merge 55)",
        "gh pr merge 54 >(gh pr merge 55)",
        "gh pr merge 54 $TARGET",
    ] {
        let (action, sensitivity) = classify("Bash", &json!({"command": command}));
        assert_eq!(sensitivity, Sensitivity::Boundary, "{command}");
        assert!(
            matches!(
                action,
                Action::Boundary {
                    ref command,
                    pr: None,
                    ..
                } if command == "gh pr merge"
            ),
            "an ambiguous merge retained a PR identity: {command} -> {action:?}"
        );
    }

    let (array, sensitivity) = classify(
        "Bash",
        &json!({"command": ["gh", "pr", "merge", "54", "--merge"]}),
    );
    assert_eq!(sensitivity, Sensitivity::Boundary);
    assert!(matches!(array, Action::Boundary { pr: None, .. }));
}

/// Every boundary and every repository write has a command line somebody wrote.
///
/// The three tests below this one walk `IRREVERSIBLE`, `REPOSITORY_SHELL` and
/// `DELIVERS` and build their command line **out of the entry they are
/// checking** — `format!("cd /x && {fragment} origin main")`. That cannot fail
/// for a renamed entry: it renames the question along with the answer. Measured
/// one round earlier on the shell verbs, where `add-content` could become
/// `add-contents` and `nice` could become `niced` with the suite green, and
/// deleting an entry was invisible too, because the deleted entry leaves the
/// population the loop walks.
///
/// These are the lists with the most authority behind them: `IRREVERSIBLE` is
/// what always re-reads the tracker however fresh the last answer was,
/// `DELIVERS` is what may not land from a state where no verdict exists, and
/// `REPOSITORY_SHELL` is what makes a shell line a repository write at all. An
/// entry misspelled in any of them is a boundary that stops being one, silently.
///
/// So the lines here are written out, and the counts are crossed: rename an
/// entry and a line stops matching, add one and the count is short.
#[test]
fn every_gated_spelling_has_a_command_line_written_out() {
    const BOUNDARIES: &[&str] = &[
        "git push origin main",
        "git merge --no-ff feature",
        "git tag v1.2.3",
        "gh pr merge 12 --squash",
        "gh pr create --fill",
        "gh release create v1.2.3",
        "gh release edit v1.2.3 --draft=false",
    ];
    const WRITES: &[&str] = &[
        "git worktree add ../wt-1 -b work",
        "git checkout -b work",
        "git switch -c work",
        "git commit -m 'a message'",
        "git rebase main",
        "git reset --hard HEAD~1",
        "git cherry-pick abc1234",
        "gh issue develop 12",
        "git branch -d work",
        "git branch --delete work",
        "git checkout -- src/x.rs",
        "git checkout . ",
        "git restore src/x.rs",
        "git stash",
        "git clean -fd",
        "git apply a.patch",
        "git am a.patch",
        "git rm src/x.rs",
        "git mv src/x.rs src/y.rs",
        "git revert abc1234",
        "git reset --keep HEAD~1",
        "git reset --merge HEAD~1",
        "git submodule update --init",
        "git worktree remove ../wt-1",
        "git filter-branch --all",
    ];
    // `out_of_phase` matches the fragment exactly rather than a whole line, so
    // these are the fragments — spelled here by hand, which is the point: the
    // day one is renamed in `DELIVERS`, the spelling written here stops
    // matching and this fails.
    const LANDS: &[&str] = &[
        "git merge",
        "git tag",
        "gh pr merge",
        "gh release create",
        "gh release edit",
    ];

    assert_eq!(
        BOUNDARIES.len(),
        IRREVERSIBLE.len(),
        "a boundary was added to the list and no command line was written for it"
    );
    assert_eq!(
        WRITES.len(),
        REPOSITORY_SHELL.len(),
        "a repository write was added to the list and no command line was written for it"
    );
    assert_eq!(
        LANDS.len(),
        DELIVERS.len(),
        "a landing step was added to the list and no command line was written for it"
    );

    for line in BOUNDARIES {
        let (action, how) = classify("Bash", &json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` is not read as a boundary"
        );
        assert!(
            matches!(action, Action::Boundary { .. }),
            "`{line}` is not read as a boundary"
        );
    }
    for line in WRITES {
        let (action, _) = classify("Bash", &json!({ "command": line }));
        assert!(
            matches!(action, Action::Write { .. } | Action::Boundary { .. }),
            "`{line}` writes to the repository and is not gated"
        );
    }
    for line in LANDS {
        assert!(
            out_of_phase(
                line,
                "in-progress",
                12,
                crate::config::Integration::Branch,
                None
            )
            .is_some(),
            "`{line}` lands the work and was allowed from a state where no verdict exists"
        );
    }

    // The floor: a read is still a read, or every assertion above would hold
    // against a classifier that gates everything.
    for read in ["git status", "git log --oneline", "gh pr view 12"] {
        let (action, how) = classify("Bash", &json!({ "command": read }));
        assert_eq!(how, Sensitivity::Routine, "`{read}` is not a boundary");
        assert!(matches!(action, Action::Untouched), "`{read}` was gated");
    }
}

/// Every reason the gate stands aside has a name a program can match.
///
/// `estigia gate --json` printed this field with `format!("{aside:?}")`, so the
/// one machine-facing value in the whole `--json` surface that spoke Rust was
/// also the one built out of a `Debug` rendering — which is not an interface:
/// rename the variant and a caller reads something else with nothing said.
///
/// Crossed over the five, because a code is only useful if it is stable, whole
/// and distinct: kebab-case like every refusal code, one per reason, and none
/// of them the Rust spelling.
#[test]
fn every_reason_for_standing_aside_has_a_stable_name() {
    // Walked from the enum's own list rather than from a copy here, which is
    // what went stale: a fifth reason was added and this test went on checking
    // four, so two asides could have shared a code and one could have carried no
    // sentence, with the suite green.
    let asides = Aside::ALL;
    let mut seen = std::collections::BTreeSet::new();
    let mut sentences = std::collections::BTreeSet::new();
    for aside in asides.iter().copied() {
        let code = aside.code();
        assert!(
            !code.is_empty() && code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "`{code}` is not spelled the way a refusal code is"
        );
        assert_ne!(
            code,
            format!("{aside:?}"),
            "the code is the Rust identifier, which changes when the variant is renamed"
        );
        assert!(seen.insert(code), "`{code}` names two reasons");
        // And the sentence, which is the half a program does not read and a
        // person does. An empty one tells an agent nothing about why the
        // harness stood aside.
        let sentence = aside.why("Write");
        assert!(
            sentence.contains("Write") && sentence.len() > 40,
            "`{code}` stands aside without saying why: {sentence:?}"
        );
        assert!(
            sentences.insert(sentence.clone()),
            "`{code}` reuses another reason's sentence: {sentence:?}"
        );
    }
    assert_eq!(seen.len(), asides.len(), "a reason lost its name");

    // And the population itself, read out of the source rather than trusted.
    // `ALL` is hand-written, so the way it goes stale is a variant that gains a
    // code and never joins the list — measured: that leaves two asides able to
    // share a code and one able to carry no sentence, with the suite green.
    let source = include_str!("mod.rs");
    let body = source
        .split_once("pub fn code(self) -> &'static str {")
        .expect("the code map exists")
        .1
        .split_once(
            "
    }",
        )
        .expect("the code map ends")
        .0;
    let arms = body.matches("Self::").count();
    assert_eq!(
        arms,
        asides.len(),
        "`code` answers for {arms} reasons and `ALL` lists {}; one is checked by nothing",
        asides.len()
    );
}

#[test]
fn every_irreversible_spelling_is_a_boundary() {
    // guard:population irreversible-shell — each listed spelling reaches the
    // boundary arm. This is the "these are gated" half; the declaration on the
    // constant owns the "these are not the only spellings" half.
    for fragment in IRREVERSIBLE {
        let (action, how) = classify(
            "Bash",
            &json!({ "command": format!("cd /x && {fragment} origin main") }),
        );
        assert_eq!(how, Sensitivity::Boundary, "{fragment}");
        assert!(matches!(action, Action::Boundary { .. }), "{fragment}");
    }
}

#[test]
fn a_read_that_merely_starts_like_a_boundary_is_not_one() {
    // Reported from a real run on 2026-08-14: `git merge-base --is-ancestor`
    // was refused as `git merge` while an issue was closed. It writes nothing —
    // it answers whether one commit reaches another, which is exactly what a
    // run cleaning up needs in order to ask *is this branch already integrated*.
    // This crate calls it itself, twice, in `transport::branch`.
    //
    // The cause is that the boundary list was matched with `contains`, so every
    // fragment also matched any longer subcommand beginning with it. A boundary
    // must be the whole word: the fragment ends where the command does, or at a
    // space. Anything else is a different subcommand wearing the same prefix.
    //
    // **`git tag --list` is NOT this defect and is not fixed here.** There the
    // fragment really is the whole command and a space really does follow it;
    // telling a listing from a creation means reading the flags, and `git tag
    // -d` deletes. That is a gate widening rather than a matcher correction, it
    // wants its own change, and doing it here to make one more line of this test
    // pass is exactly what `AGENTS.md` refuses.
    for read_only in [
        "git merge-base --is-ancestor ff7a004 origin/main",
        "git merge-tree main topic",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": read_only }));
        assert_ne!(
            how,
            Sensitivity::Boundary,
            "{read_only} writes nothing and was gated as a delivery boundary"
        );
        assert!(
            !matches!(action, Action::Boundary { .. }),
            "{read_only} writes nothing and was gated as a delivery boundary"
        );
    }

    // And the boundaries themselves still are ones, including the two whose
    // prefixes the reads above share.
    for boundary in ["git merge origin/main", "git tag v1.2.3", "git push"] {
        let (_, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}");
    }
}

#[test]
fn a_boundary_is_recognised_through_spacing_and_case() {
    let (_, how) = classify(
        "Bash",
        &json!({"command": "GIT   PUSH  --force origin main"}),
    );
    assert_eq!(how, Sensitivity::Boundary);
}

#[test]
fn only_one_literal_fast_forward_command_preserves_a_target_for_proof() {
    for command in [
        "git merge --ff-only origin/main",
        "git\tmerge  --ff-only\t-- origin/main",
    ] {
        let (action, how) = classify("Bash", &json!({"command": command}));
        assert_eq!(how, Sensitivity::Boundary, "{command}");
        let Action::Boundary {
            command: boundary,
            local_fast_forward_target,
            ..
        } = action
        else {
            panic!("{command} stopped being a boundary");
        };
        assert_eq!(boundary, "git merge");
        assert_eq!(local_fast_forward_target.as_deref(), Some("origin/main"));
    }

    for command in [
        "git merge --ff-only origin/main && echo unsafe",
        "git merge --ff-only origin/main; echo unsafe",
        "git merge --ff-only origin/main\ngit status",
        "sudo git merge --ff-only origin/main",
        "git -C elsewhere merge --ff-only origin/main",
        "git --git-dir=.git merge --ff-only origin/main",
        "git -c advice.detachedHead=false merge --ff-only origin/main",
        "git merge --ff-only --no-edit origin/main",
        "git merge --ff-only origin/main other",
        "git merge --ff-only 'origin/main'",
        "git merge --ff-only $UPSTREAM",
        "git merge --ff-only origin/{main}",
        "git merge --ff-only -topic",
    ] {
        let (action, how) = classify("Bash", &json!({"command": command}));
        assert_eq!(how, Sensitivity::Boundary, "{command}");
        let Action::Boundary {
            local_fast_forward_target,
            ..
        } = action
        else {
            panic!("{command} stopped being a boundary");
        };
        assert_eq!(
            local_fast_forward_target, None,
            "{command} retained metadata that could bypass out-of-phase"
        );
    }

    let (array, how) = classify(
        "Bash",
        &json!({"command": ["git", "merge", "--ff-only", "origin/main"]}),
    );
    assert_eq!(how, Sensitivity::Boundary);
    assert!(matches!(
        array,
        Action::Boundary {
            local_fast_forward_target: None,
            ..
        }
    ));

    // Each of these carries more than one key that normalises to a command
    // argument. Classification must still see every boundary named anywhere in
    // the ambiguous payload, while no one value may donate proof metadata for
    // another value's classification.
    for input in [
        json!({
            "command": "git merge --ff-only origin/main",
            "commandLine": "git status",
        }),
        json!({
            "command": "git status",
            "command_line": "git merge --ff-only origin/main",
        }),
        json!({
            "commandLine": "git merge --ff-only origin/main",
            "command_line": "git merge --ff-only origin/main",
        }),
        json!({
            "command": ["git", "merge", "--ff-only", "origin/main"],
            "commandLine": "git status",
        }),
        json!({
            "command": ["git", "status"],
            "commandLine": ["git", "merge", "--ff-only", "origin/main"],
        }),
    ] {
        let (action, how) = classify("Bash", &input);
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "ambiguous payload escaped: {input}"
        );
        assert!(
            matches!(
                action,
                Action::Boundary {
                    ref command,
                    local_fast_forward_target: None,
                    ..
                } if command == "git merge"
            ),
            "ambiguous payload retained proof metadata: {input} -> {action:?}"
        );
    }
}

#[test]
fn local_fast_forward_proof_fails_closed_on_repository_state() {
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |arguments: &[&str]| -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(arguments)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);
    let branch = git(&["branch", "--show-current"]);
    let base = git(&["rev-parse", "HEAD"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "upstream"]);
    let upstream = git(&["rev-parse", "HEAD"]);
    git(&["tag", "-a", "upstream-tag", "-m", "tag object"]);
    let tag_object = git(&["rev-parse", "upstream-tag"]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["reset", "--hard", "--quiet", &base]);
    git(&["remote", "add", "origin", "https://example.invalid/o/r.git"]);
    git(&["config", &format!("branch.{branch}.remote"), "origin"]);
    git(&[
        "config",
        &format!("branch.{branch}.merge"),
        "refs/heads/main",
    ]);

    assert!(is_safe_local_fast_forward(repo.path(), "origin/main"));
    assert!(is_safe_local_fast_forward(
        repo.path(),
        "refs/remotes/origin/main"
    ));
    assert!(is_safe_local_fast_forward(repo.path(), &upstream));
    assert!(
        !is_safe_local_fast_forward(repo.path(), &tag_object),
        "an object ID that peeled to a different commit ID was accepted"
    );
    assert!(!is_safe_local_fast_forward(repo.path(), "origin/other"));

    std::fs::write(repo.path().join("untracked"), "dirty").expect("a dirty worktree");
    assert!(!is_safe_local_fast_forward(repo.path(), "origin/main"));
    std::fs::remove_file(repo.path().join("untracked")).expect("the worktree is clean again");

    git(&["checkout", "--detach", "--quiet"]);
    assert!(!is_safe_local_fast_forward(repo.path(), "origin/main"));
    git(&["checkout", "--quiet", &branch]);

    git(&["commit", "--allow-empty", "--quiet", "-m", "side"]);
    let side = git(&["rev-parse", "HEAD"]);
    git(&["reset", "--hard", "--quiet", &base]);
    assert!(
        !is_safe_local_fast_forward(repo.path(), &side),
        "a commit outside the upstream ancestry was accepted"
    );
}

#[test]
fn every_proof_git_process_removes_repository_steering_environment() {
    let command = proof_git_command_with_environment(
        Path::new("repo"),
        &["status"],
        [
            "GIT_DIR",
            "git_work_tree",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG_COUNT",
            "GIT_SHALLOW_FILE",
            "PATH",
        ]
        .map(std::ffi::OsString::from),
    );
    let changes: std::collections::BTreeMap<String, Option<std::ffi::OsString>> = command
        .get_envs()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.map(std::ffi::OsStr::to_os_string),
            )
        })
        .collect();
    for name in [
        "GIT_DIR",
        "git_work_tree",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG_COUNT",
        "GIT_SHALLOW_FILE",
    ] {
        assert_eq!(
            changes.get(name),
            Some(&None),
            "{name} would steer a proof subprocess: {changes:?}"
        );
    }
    assert_eq!(
        changes.get("PATH"),
        None,
        "unrelated environment was changed"
    );
    assert!(is_git_environment(std::ffi::OsStr::new("GIT_DIR")));
    assert!(is_git_environment(std::ffi::OsStr::new("git_work_tree")));
    assert!(!is_git_environment(std::ffi::OsStr::new("PATH")));
}

#[test]
fn shell_steering_environment_cannot_earn_the_fast_forward_exception() {
    for name in [
        "BASH_ENV",
        "bash_env",
        "ENV",
        "env",
        "BASH_FUNC_git%%",
        "bash_func_GIT%%",
        "GIT_DIR",
    ] {
        assert!(
            !fast_forward_environment_is_unsteered([std::ffi::OsString::from(name)]),
            "{name} could make the shell execute a different git"
        );
    }
    for name in ["PATH", "PATHEXT", "SHELL"] {
        assert!(
            fast_forward_environment_is_unsteered([std::ffi::OsString::from(name)]),
            "{name} does not introduce shell-only git resolution"
        );
    }
}

#[test]
fn reading_the_repository_is_not_the_harness_s_business() {
    for command in ["ls -la", "cat README.md", "git status", "git log --oneline"] {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        assert_eq!(action, Action::Untouched, "{command}");
    }
}

#[test]
fn a_tool_the_gate_does_not_know_is_left_alone() {
    for tool in ["Read", "Grep", "Glob", "WebFetch", "mcp__leteo__mem_save"] {
        let (action, _) = classify(tool, &json!({}));
        assert_eq!(action, Action::Untouched, "{tool}");
    }
}

#[test]
fn a_run_that_has_sworn_nothing_is_outside_the_harness() {
    // The whole scoping decision, pinned. A person asking an unrelated question
    // must be able to edit a file.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = Run::new("claude-abcd1234".to_owned());
    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert_eq!(
        gate(&context, &mut run, &action, Sensitivity::Routine),
        Decision::Outside(Aside::NothingSworn)
    );
}

#[test]
fn a_claim_in_another_checkout_does_not_gate_this_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &root.path().join("somewhere-else"));
    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert_eq!(
        gate(&context, &mut run, &action, Sensitivity::Routine),
        Decision::Outside(Aside::AnotherCheckout)
    );
}

#[test]
fn a_routine_write_rides_on_an_answer_from_inside_the_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    // The contract, and only the contract. Issue #29 moved that refusal above
    // the window, so a fixture that installs nothing measures the refusal
    // rather than the window — and the fast path this gate exists to keep cheap
    // would have gone unmeasured while every in-window write silently started
    // paying for a tracker round trip.
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    // The transport is not installed, so reaching it would deny. Riding on the
    // window is the only way this can allow.
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Allow(reason) => assert!(reason.contains("renewal window")),
        other => panic!("expected an allow inside the window, got {other:?}"),
    }
}

#[test]
fn a_boundary_never_rides_on_the_window() {
    // The one rule the window must never cover. A merge is not cleared by a
    // read that happened two minutes ago.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let action = Action::Boundary {
        command: "git push".to_owned(),
        pr: None,
        local_fast_forward_target: None,
    };
    let decision = gate(&context, &mut run, &action, Sensitivity::Boundary);
    assert!(
        decision.denies(),
        "a boundary rode on a cached answer: {decision:?}"
    );
}

#[test]
fn an_unreadable_control_surface_permits_no_write() {
    // `SKILL.md`: "an unreadable control surface permits no write". Here the
    // transport is missing entirely, which is as unreadable as it gets.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => {
            // The contract, not a script. This used to be
            // `transport-not-installed`, and what it actually detected was that
            // `scripts/github.py` was missing — true here by accident, because
            // nothing was installed at all. The gate answers in this process
            // now, so it asks about the surface its authority rests on.
            assert_eq!(refusal.code, "control-surface-not-installed");
            // And it names what to run, because there is something to run.
            assert!(refusal.to_string().contains("estigia setup --all"));
        }
        other => panic!("an unreadable control surface allowed a write: {other:?}"),
    }
}

/// The same rule, one `mark_verified()` line later.
///
/// The renewal window caches a *claim verification*; it says nothing about
/// whether the contract is still on disk. While the window's `Allow` answered
/// above the contract refusal, the fixture above passed only because it was
/// outside the window — add the stamp and a routine write went through with no
/// `SKILL.md` installed at all, for the whole width of the window. That is what
/// stops this ordering drifting back.
#[test]
fn an_unreadable_control_surface_permits_no_write_inside_the_renewal_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);
    // The only difference from the fixture above.
    run.mark_verified();

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "control-surface-not-installed");
            assert!(refusal.to_string().contains("estigia setup --all"));
        }
        other => panic!(
            "an unreadable control surface allowed a write inside the renewal window: {other:?}"
        ),
    }
}

#[test]
fn a_denial_names_what_was_being_attempted() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);

    let action = Action::Write {
        target: "src/secrets.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => assert!(refusal.message.starts_with("src/secrets.rs:")),
        other => panic!("expected a denial, got {other:?}"),
    }
}

#[test]
fn the_gate_never_writes_to_the_tracker() {
    // The property that lets this sit on the critical path of every edit: being
    // wrong costs a refused edit, never a damaged issue. Verified structurally —
    // the only transport call the gate makes is `verify-claim`, which reads.
    let source = include_str!("mod.rs");
    // Counted where the call now is. The gate stopped spawning the transport —
    // `tracker::invoke` is gone from this module entirely — and asks the port in
    // this process, so the structural property is counted against the dispatch.
    assert_eq!(
        source.matches("tracker::invoke(").count(),
        0,
        "the gate spawns the transport again"
    );
    // Two now, and both reads. The second is
    // `tracker_answer_for_pointer` — the one question
    // `guard::adjudicate_action`'s reconciliation and `doctor`'s
    // `stale-run-pointer` row both ask, held to `gate`'s own shape rather than
    // written a third time: same `Context::live`, same `GH_REPO`, same
    // `verify-claim`. A third call site would be the copy this file's own
    // history keeps finding; this test still holds the count to exactly the
    // ones this module owns.
    let calls = source.matches("dispatch::dispatch(").count();
    assert_eq!(
        calls, 2,
        "the gate grew a transport call this test does not know about"
    );
    assert!(
        source.contains("\"verify-claim\""),
        "the one call the gate makes is no longer the read it is allowed to make"
    );
    let mutating = ["\"claim\"", "\"transition\"", "\"comment\"", "\"reclaim\""];
    for operation in mutating {
        assert!(
            !source.contains(operation),
            "the gate names the mutating operation {operation}"
        );
    }
}

#[test]
fn a_window_of_zero_asks_every_time() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.window = Duration::from_secs(0);
    // With nothing installed this test passes without the window existing at
    // all: `control-surface-not-installed` answers above it since issue #29, and
    // a fixture that measures a refusal it would get either way is a fixture
    // that has stopped asking its own question.
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    let decision = gate(&context, &mut run, &action, Sensitivity::Routine);
    assert!(
        decision.denies(),
        "a zero window still rode on a previous answer"
    );
    // And it is the tracker road that refused, not the contract. The pair with
    // `a_routine_write_rides_on_an_answer_from_inside_the_window` — same context,
    // same verified run, same installed contract, only the window differs — is
    // what measures that an in-window routine write is still answered without a
    // tracker round trip.
    let Decision::Deny(refusal) = &decision else {
        unreachable!("just asserted a denial")
    };
    assert_ne!(
        refusal.code, "control-surface-not-installed",
        "a zero window was refused before the window was ever consulted: {decision:?}"
    );
}

#[test]
fn an_issue_that_is_not_a_number_is_refused_with_what_is_wanted() {
    let refusal = issue_not_a_number("twelve");
    assert_eq!(refusal.code, "issue-not-a-number");
    assert!(refusal.to_string().contains("operator-knowledge"));
}

#[test]
fn a_hand_rolled_repository_write_is_gated() {
    // guard:population repository-shell. `branch + worktree` is a transport
    // operation that verifies the claim before it makes the checkout. An agent
    // typing `git worktree add` instead has made the first write of a delivery
    // with nothing checked.
    for fragment in REPOSITORY_SHELL {
        let (action, how) = classify(
            "Bash",
            &json!({ "command": format!("{fragment} something") }),
        );
        assert!(
            matches!(action, Action::Write { .. }),
            "{fragment} was not gated"
        );
        assert_eq!(how, Sensitivity::Routine, "{fragment}");
    }
}

#[test]
fn a_denied_write_cannot_be_respelled_as_a_shell_command() {
    // The bypass this closes, end to end rather than at the classifier: a run
    // under oath whose control surface cannot be reached has its `Edit` denied.
    // Every one of these reaches the same file by another spelling, and each
    // used to classify as `Untouched` — no decision, and the write went through.
    // Denying the tool while allowing `echo … > src/x.rs` is not authority; it
    // is a speed bump.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let run = sworn(12, &context.repo_dir);

    let denied = |action: &Action| {
        let mut run = run.clone();
        gate(&context, &mut run, action, Sensitivity::Routine).denies()
    };

    let edit = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert!(denied(&edit), "the tool spelling was not denied");

    for command in [
        "echo fn main() {} > src/x.rs",
        "cat >> src/x.rs",
        "sed -i s/a/b/ src/x.rs",
        "python -c \"open('src/x.rs','w').write('')\"",
        "cp /tmp/x.rs src/x.rs",
        "rm src/x.rs",
    ] {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        assert!(
            matches!(action, Action::Write { .. }),
            "`{command}` was not read as a write"
        );
        assert!(
            denied(&action),
            "`{command}` reached the tree after a denial"
        );
    }

    // And the reads beside them still cost nothing: these must stay outside, or
    // the gate starts verifying a claim on `ls`.
    for command in ["ls src", "cargo test 2>&1", "grep -rn confirm src/"] {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        assert_eq!(action, Action::Untouched, "{command}");
    }
}

#[test]
fn a_delivery_is_refused_in_a_state_that_holds_no_verdict() {
    // statewright's narrowing: holding an issue and being allowed to land it are
    // two questions, and only the first was being asked. A run in `in-progress`
    // that ran `gh pr merge` was told yes.
    for command in DELIVERS {
        for state in ["analysis", "ready", "in-progress", "blocked"] {
            let refusal =
                out_of_phase(command, state, 12, crate::config::Integration::Branch, None)
                    .unwrap_or_else(|| panic!("{command} was allowed to land from {state}"));
            assert_eq!(refusal.code, "out-of-phase");
            // No command is named, because none discharges this: the missing
            // thing is somebody's answer, not an invocation.
            assert!(
                refusal.to_string().contains("[human-authority]"),
                "{command}"
            );
        }
        for state in DELIVERY_STATES {
            assert!(
                out_of_phase(command, state, 12, crate::config::Integration::Branch, None)
                    .is_none(),
                "{command} was refused from {state}, where a verdict can exist"
            );
        }
    }
}

#[test]
fn publishing_a_review_target_is_never_out_of_phase() {
    // The deadlock this avoids. Publishing is how a run *reaches* review, so
    // gating it on review would mean no run could ever get there.
    for command in ["git push", "gh pr create"] {
        for state in [
            "analysis",
            "ready",
            "in-progress",
            "review",
            "blocked",
            "done",
        ] {
            assert!(
                out_of_phase(command, state, 12, crate::config::Integration::Branch, None)
                    .is_none(),
                "{command} was refused from {state}"
            );
        }
    }
}

#[test]
fn a_boundary_the_operator_declared_is_never_read_as_a_delivery() {
    // Estigia cannot know whether somebody's `make deploy` delivers or
    // rehearses, and a guess would refuse a step it never understood. Matched by
    // exact equality for exactly that reason.
    for command in ["npm publish", "terraform apply", "make deploy"] {
        assert!(
            out_of_phase(
                command,
                "in-progress",
                12,
                crate::config::Integration::Branch,
                None
            )
            .is_none(),
            "{command}"
        );
    }
}

#[test]
fn an_unreadable_contract_costs_a_declared_boundary_its_boundary_status() {
    // The measurement behind the honesty contract entry, kept as a test so the
    // entry cannot quietly stop being true. `gate_context` reads the installed
    // contract with `unwrap_or_default()`: a file that will not parse yields an
    // empty boundary list, and this is what an empty list buys the operator.
    //
    // Half the loss is taken back there — the renewal window goes to zero, so
    // nothing rides a cached answer. This half is not: a command the operator
    // declared irreversible classifies as a routine write, and a routine write
    // never reaches the phase question. Not a gap that can be closed by being
    // careful here; the list is what went missing.
    for command in ["npm publish", "terraform apply", "make deploy"] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[command.to_owned()]).1,
            Sensitivity::Boundary,
            "{command} was declared and is not a boundary"
        );
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Routine,
            "{command} kept its boundary status without the declaration, so this \
             test measures nothing"
        );
    }
}

#[test]
fn every_delivery_is_one_of_the_irreversible_spellings() {
    // Two hand-written lists that have to agree. A delivery that is not an
    // irreversible boundary never reaches the phase question at all, because
    // nothing classifies it as one.
    for command in DELIVERS {
        assert!(
            IRREVERSIBLE.contains(command),
            "{command} is a delivery the classifier never produces"
        );
    }
}

#[test]
fn the_delivery_states_are_states_the_binding_declares() {
    // The failure this exists for is the one that looks like working: rename a
    // state in the binding and `DELIVERY_STATES` matches nothing, so every
    // delivery is refused forever and the gate looks strict rather than broken.
    let binding = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skill/bindings/github.md"),
    )
    .expect("the GitHub binding ships with the crate");
    let sentence = binding
        .lines()
        .find(|line| line.starts_with("The workflow's states are"))
        .expect("the binding names its states in a sentence this test can read");
    let declared: Vec<&str> = sentence.split('`').skip(1).step_by(2).collect();
    assert!(
        declared.len() >= 5,
        "the states sentence changed shape and this test stopped reading it: {sentence}"
    );
    for state in DELIVERY_STATES {
        assert!(
            declared.contains(state),
            "`{state}` is not one of the binding's states {declared:?}"
        );
    }
}

#[test]
fn the_gate_asks_the_phase_question_only_after_the_claim_holds() {
    // The proof boundary, stated: the tests above prove the *rule*, and this
    // proves the gate reaches it — structurally, the way
    // `the_gate_never_writes_to_the_tracker` does, because no fixture here can
    // hand `gate` a transport that answers `0`.
    let source = include_str!("mod.rs");
    let arm = source
        .split_once("run.mark_verified();")
        .expect("the gate marks a verified claim")
        .1;
    let allow = arm.find("Decision::Allow").expect("the allow arm follows");
    assert!(
        arm[..allow].contains("out_of_phase("),
        "the phase question is not asked between verifying the claim and allowing"
    );
}

#[test]
fn an_irreversible_step_outranks_a_plain_repository_write() {
    // `git push` and `git commit` in one command line is a push. Classifying it
    // as the routine half would let it ride on the renewal window, and a push
    // is the thing the window must never cover.
    let (_, how) = classify(
        "Bash",
        &json!({"command": "git commit -m x && git push origin HEAD"}),
    );
    assert_eq!(how, Sensitivity::Boundary);
}

#[test]
fn reading_the_repository_is_still_not_gated_after_widening_the_population() {
    for command in [
        "git status",
        "git log --oneline",
        "git diff",
        "git worktree list",
        "gh issue view 12",
        "cargo test",
    ] {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        assert_eq!(action, Action::Untouched, "{command}");
    }
}

#[test]
fn a_write_in_the_isolated_checkout_is_covered_by_the_claim() {
    // The defect this test exists for: a claim is made in the base checkout and
    // the delivery is written in a worktree somewhere else entirely.
    // `repository-delivery.md` requires exactly that — "Place each
    // implementation checkout outside the base working tree" — so a gate scoped
    // only to where `claim` ran watches the one directory the run never edits,
    // and lets the whole delivery through as `Outside`.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    let worktree = root.path().join("trees").join("issue-12");
    context.repo_dir = worktree.clone();
    // This fixture is about coverage, and it rides the window to get an answer
    // without a transport. The contract refusal now answers above the window,
    // so without this the test would measure an uninstalled Estigia instead of
    // the checkout it is named for.
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let mut run = sworn(12, &root.path().join("repo"));
    run.worktree = Some(worktree);
    run.mark_verified();

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Allow(_) => {}
        other => panic!("the isolated checkout was not covered by the claim: {other:?}"),
    }
}

#[test]
fn a_write_in_neither_the_repository_nor_its_worktree_is_still_outside() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.repo_dir = root.path().join("somebody-elses-project");

    let mut run = sworn(12, &root.path().join("repo"));
    run.worktree = Some(root.path().join("trees").join("issue-12"));

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert_eq!(
        gate(&context, &mut run, &action, Sensitivity::Routine),
        Decision::Outside(Aside::AnotherCheckout)
    );
}

#[test]
fn a_run_with_no_recorded_directory_is_gated_wherever_it_writes() {
    // Failing open here would mean a pointer that lost its paths becomes a run
    // nothing checks. It has still sworn, so it is still gated.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(12);

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert!(gate(&context, &mut run, &action, Sensitivity::Routine).denies());
}

#[test]
fn holdings_lists_only_runs_that_actually_hold_something() {
    // Incident I06. A pointer with no issue is a session that swore nothing, and
    // listing it as a holding would bury the ones that matter.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut held = Run::new("claude-holding0".to_owned());
    held.issue = Some(12);
    held.state = Some("in-progress".to_owned());
    session::store(root.path(), &held).expect("the pointer writes");
    session::store(root.path(), &Run::new("claude-nothing0".to_owned()))
        .expect("the pointer writes");

    let holdings = session::holdings(root.path());
    assert_eq!(holdings.len(), 1);
    assert_eq!(holdings[0].run_id, "claude-holding0");
    assert_eq!(holdings[0].issue, Some(12));
}

#[test]
fn holdings_reads_the_quietest_run_first() {
    // The one most likely to be dead is the one worth asking the tracker about.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut recent = Run::new("claude-recent00".to_owned());
    recent.issue = Some(1);
    recent.mark_verified();
    let mut old = Run::new("claude-old00000".to_owned());
    old.issue = Some(2);
    old.verified_at = recent.verified_at.map(|seconds| seconds - 9000);
    session::store(root.path(), &recent).expect("writes");
    session::store(root.path(), &old).expect("writes");

    let holdings = session::holdings(root.path());
    assert_eq!(holdings.first().map(|run| run.issue), Some(Some(2)));
    assert!(session::silence(&holdings[0]).seconds().unwrap_or(0) > 8000);
}

#[test]
fn a_machine_with_no_state_directory_reports_no_holdings() {
    let root = tempfile::tempdir().expect("a temporary root");
    assert!(session::holdings(&root.path().join("never-created")).is_empty());
}

#[test]
fn a_tracker_with_no_transport_leaves_the_gate_standing_aside() {
    // Not a denial: there is nothing to ask, so denying every write would be a
    // lock rather than authority. `doctor` is where this is said out loud.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.tracker = crate::config::Tracker::Linear;
    let mut run = sworn(12, &context.repo_dir);

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    assert_eq!(
        gate(&context, &mut run, &action, Sensitivity::Routine),
        Decision::Outside(Aside::NoTracker)
    );
}

#[test]
fn a_control_surface_that_is_nowhere_is_as_unreadable_as_one_that_is_empty() {
    // The two spellings of the same rule, which used to answer opposite ways.
    // `an_unreadable_control_surface_permits_no_write` covers the transport
    // missing from the root that was found. This covers it missing from every
    // root — which reached the hook as a refusal, and a refusal the hook turned
    // into no decision at all, so the write went through.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.skill_root = root.path().join("nowhere-at-all");
    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };

    // A run that swore is measured against an oath it can no longer check.
    let mut sworn_run = sworn(12, &context.repo_dir);
    match gate(&context, &mut sworn_run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => assert_eq!(refusal.code, "control-surface-not-installed"),
        other => panic!("a sworn run wrote against a surface nobody can read: {other:?}"),
    }

    // And a run that swore nothing is still outside, because failing closed on
    // everybody would be a lock rather than authority — and would teach people
    // to take the hook back out.
    let mut unsworn = Run::new("claude-unsworn0".to_owned());
    assert!(matches!(
        gate(&context, &mut unsworn, &action, Sensitivity::Routine),
        Decision::Outside(Aside::NothingSworn)
    ));
}

#[test]
fn a_run_record_that_cannot_be_read_is_an_unknown_rather_than_an_absence() {
    // "An unknown result is not clearance" — one of the three rules the
    // directive states to every agent on every turn. A pointer that is on disk
    // and will not parse says a run under this name existed; it says nothing
    // about what it swore, and answering "swore nothing" is an answer nobody
    // checked.
    //
    // The case that is not corruption: a release that changes `Run` makes every
    // pointer the previous one wrote unreadable at once.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::write(
        context.state_root.join("claude-corrupt0.json"),
        "{\"run_id\": \"claude-corrupt0\", \"issue\":",
    )
    .expect("plant a half-written pointer");

    let mut run = session::load(&context.state_root, "claude-corrupt0");
    assert!(run.unreadable);
    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "run-pointer-unreadable");
            // And it names **no** command, because none of Estigia's discharges
            // this. It used to name `estigia release`, under a comment saying
            // that releasing the run makes the pointer readable again — which
            // was never true and was never checked: with an unreadable pointer
            // `release` cannot say what to put down either, and answers with
            // this same code. The assertion measured that the string was there,
            // not that running it would help.
            assert!(
                matches!(
                    refusal.resolution,
                    crate::outcome::Resolution::NoCommand { .. }
                ),
                "a command was named for something no command of Estigia's clears: {refusal}"
            );
            assert!(
                !refusal.to_string().contains("estigia release"),
                "the loop is back: {refusal}"
            );
        }
        other => panic!("an unknown was read as clearance: {other:?}"),
    }

    // A pointer that is simply not there is still the ordinary state, and still
    // outside: this must not turn a first tool call into a refusal.
    let mut fresh = session::load(&context.state_root, "claude-nothing0");
    assert!(!fresh.unreadable);
    assert!(matches!(
        gate(&context, &mut fresh, &action, Sensitivity::Routine),
        Decision::Outside(Aside::NothingSworn)
    ));
}

#[test]
fn a_pointer_that_will_not_open_is_an_unknown_rather_than_an_absence() {
    // The sibling test above poses bytes that will not parse. This poses a
    // pointer the filesystem refuses to read at all — issue #38's measured
    // incident, reduced: a run's pointer is gone or unreadable while the
    // tracker still names that run as holder, and the gate must not read the
    // missing record as "this run swore nothing". A directory at the pointer
    // path is the deterministic cross-platform spelling of a failed read that
    // is not `NotFound`: the read fails on both platforms, with an error kind
    // that is not absence.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::create_dir_all(context.state_root.join("claude-blocked0.json"))
        .expect("a directory where the pointer should be");

    let mut run = session::load(&context.state_root, "claude-blocked0");
    assert!(
        run.unreadable,
        "a pointer the filesystem refuses to read was reported as a run that swore nothing"
    );
    assert!(
        run.unreadable_reason
            .as_deref()
            .is_some_and(|why| why.contains("claude-blocked0.json")),
        "the refusal carries no path, so nobody can find the file to look at: {:?}",
        run.unreadable_reason
    );

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    match gate(&context, &mut run, &action, Sensitivity::Routine) {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "run-pointer-unreadable");
            assert!(
                refusal.to_string().contains("claude-blocked0.json"),
                "the gate named no path for a record it could not read: {refusal}"
            );
        }
        other => panic!("a failed read was read as clearance: {other:?}"),
    }
}

#[test]
fn the_predicate_the_other_tests_lean_on_answers_for_itself() {
    // Mutation testing found this one, and it is the sharpest of the twelve:
    // `denies()` is used by five assertions and by no shipping code, so
    // replacing its body with `true` made all five pass. A predicate held up
    // only by the tests it holds up is a single point that can fail silently in
    // both directions at once.
    assert!(
        Decision::Deny(Box::new(Refusal::not_started(
            "test-only",
            "a refusal",
            Resolution::run("estigia status"),
        )))
        .denies()
    );
    assert!(!Decision::Outside(Aside::NothingSworn).denies());
    assert!(!Decision::Allow("a reason".to_owned()).denies());
}

#[test]
fn the_control_surface_is_a_real_location_rather_than_an_empty_path() {
    // `control_surface -> Default::default()` survived too. An empty path has no
    // transport either, so the gate still denies and the difference hides — but
    // the refusal would name nothing, and the message that tells an operator
    // where to install is the whole point of naming a path at all.
    let surface = control_surface();
    assert!(
        !surface.as_os_str().is_empty(),
        "the control surface has no path to name"
    );
    assert!(
        surface.ends_with(crate::skill::DIRECTORY),
        "the control surface is not where the skill goes: {}",
        surface.display()
    );
}

/// Two installed roots, identical contracts, and one operator file.
///
/// The shape the defect was measured in: `setup --all` writes the same
/// `SKILL.md` everywhere, so the only thing telling the candidates apart is the
/// file Estigia never writes.
fn two_roots_one_configured(
    home: &Path,
) -> (
    crate::setup::SetupOptions,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.to_path_buf()),
        config_home: Some(home.join(".config")),
        app_data: Some(home.join("AppData").join("Roaming")),
        platform: Some(crate::setup::Platform::Unix),
        skip_harness: true,
        ..crate::setup::SetupOptions::default()
    };
    let config = crate::config::Config::default();
    let mut roots = Vec::new();
    for slug in ["agents", "claude-code"] {
        let adapter = crate::setup::find_agent(slug).expect("a declared agent");
        crate::setup::setup(adapter, &config, &options).expect("the install writes");
        roots.push(
            crate::setup::resolve_paths(adapter, &options)
                .expect("paths")
                .skill_root,
        );
    }
    let claude = roots.pop().expect("the claude-code root");
    let neutral = roots.pop().expect("the neutral root");
    (options, neutral, claude)
}

#[test]
fn the_canonical_root_is_the_one_holding_the_operators_own_file() {
    // The gate decides in the root this returns, and it was deciding in the one
    // that holds none of the operator's values: every candidate carries a
    // configuration block after `setup --all`, so the preference could not
    // discriminate and the `AGENTS` order took the shared neutral root. Measured
    // on the machine that filed #41 — two byte-identical `SKILL.md`, one
    // `estigia.local.md` beside the Claude Code contract, and a gate answering
    // `Blind judges: single` to an operator who had written `two blind`.
    let home = tempfile::tempdir().expect("a temporary home");
    let (options, neutral, claude) = two_roots_one_configured(home.path());

    // Before their file exists, nothing distinguishes the two and the order
    // stands: the neutral root, first in `AGENTS`, is still the answer.
    assert_eq!(
        discover_skill_root_in(&options).expect("a root"),
        neutral,
        "with nothing to tell the roots apart the declared order stopped deciding"
    );

    std::fs::write(
        claude.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |\n|---|---|\n| Blind judges | two blind |\n",
    )
    .expect("the operator's own file");

    assert_eq!(
        discover_skill_root_in(&options).expect("a root"),
        claude,
        "the gate decides in a root that holds none of the operator's overrides"
    );
}

#[test]
fn the_gate_reads_the_row_the_operator_wrote_rather_than_its_default() {
    // The selection is only worth changing if what the gate reads changes with
    // it. This is the same read `gate_context` performs — the canonical root,
    // layered — so a root chosen without the operator's file hands the gate
    // `Config::default()`, which is the loosening direction.
    let home = tempfile::tempdir().expect("a temporary home");
    let (options, _neutral, claude) = two_roots_one_configured(home.path());
    std::fs::write(
        claude.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |\n|---|---|\n| Blind judges | two blind |\n| Change size | 120 |\n",
    )
    .expect("the operator's own file");

    let canonical = discover_skill_root_in(&options).expect("a root");
    let (config, _) =
        crate::skill::installed_config_in_keeping_what_parses(&canonical, home.path());

    assert_eq!(
        config.judges,
        crate::config::Judges::TwoBlind,
        "the gate is adjudicating against a default the operator overrode"
    );
    assert_eq!(config.change_size, 120);
}

#[test]
fn a_root_with_no_configuration_block_is_still_not_preferred() {
    // The rule this replaced was written for a real case and keeps its job: an
    // operator already running upstream `issue-flow` has a root that holds a
    // contract Estigia never wrote. Their own file landing there must not make
    // it the place the gate decides, because the block — the thing carrying
    // every other row — is not in it.
    let home = tempfile::tempdir().expect("a temporary home");
    let (options, neutral, claude) = two_roots_one_configured(home.path());
    std::fs::write(
        neutral.join(crate::skill::CONTRACT),
        "# somebody else's contract\n",
    )
    .expect("a contract with no block");
    std::fs::write(
        neutral.join(crate::config::LOCAL_FILE),
        "| Setting | Value here |\n|---|---|\n| Blind judges | two blind |\n",
    )
    .expect("their file, in the root with no block");

    assert_eq!(
        discover_skill_root_in(&options).expect("a root"),
        claude,
        "a contract with no configuration block was preferred over one that has it"
    );
}

#[test]
fn a_session_holding_an_isolated_checkout_is_told_which_one_it_is() {
    // `references/repository-delivery.md`: "Keep the base checkout read-only.
    // One issue uses one traceable branch and one isolated worktree." Estigia
    // holds both paths and refuses neither — that reference is a floor the
    // repository's own rules override, and a write to the base checkout is
    // sometimes right. What it can do is say which directory is this run's, once,
    // in the message the agent reads before touching anything.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);
    run.worktree = Some(root.path().join("trees").join("issue-12"));

    let response = hook::session_start_response(&context, &run);
    let injected = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected");
    assert!(injected.contains("issue-12"), "{injected}");
    assert!(
        injected.contains("base checkout stays read-only"),
        "{injected}"
    );

    // A run with no worktree of its own is told nothing about one: a sentence
    // about a directory that does not exist is worse than silence.
    let plain = sworn(12, &context.repo_dir);
    let response = hook::session_start_response(&context, &plain);
    let injected = response["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context is injected");
    assert!(!injected.contains("read-only"), "{injected}");
}

#[test]
fn a_decision_leaves_evidence_and_standing_aside_does_not() {
    // Estigia refused a write, the terminal scrolled, and nothing was left. For
    // a tool whose whole argument is durable evidence, its own decisions were
    // the one thing it kept none of — and "why did it stop me?" had no answer an
    // hour later.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let ledger = session::ledger_path(&context.state_root);

    // Standing aside writes nothing: a line per tool call of every session that
    // never swore would bury the ones that matter.
    session::record(
        &context.state_root,
        &serde_json::json!({"verdict": "allow"}),
    );
    let after_one = std::fs::read_to_string(&ledger).expect("the ledger exists");
    assert_eq!(after_one.lines().count(), 1);

    // Appended, not replaced: this is a history rather than a state.
    session::record(&context.state_root, &serde_json::json!({"verdict": "deny"}));
    let text = std::fs::read_to_string(&ledger).expect("the ledger exists");
    assert_eq!(text.lines().count(), 2);
    for line in text.lines() {
        serde_json::from_str::<serde_json::Value>(line).expect("every line is a record");
    }
    assert!(text.contains("deny"));
}

#[test]
fn an_oversized_ledger_keeps_its_newer_half() {
    // Unbounded growth on the critical path of every edit is a disk that fills
    // while the tool reports nothing wrong. Halved rather than emptied, so "what
    // happened just now" survives the trim that "what happened in March" does
    // not.
    let root = tempfile::tempdir().expect("a temporary root");
    let state = root.path().join("state");
    std::fs::create_dir_all(&state).expect("the state directory");
    let ledger = session::ledger_path(&state);

    // Sized against the real cap rather than a guess. The first version wrote
    // forty thousand short lines, which came to under half a megabyte and never
    // tripped the trim — a test that passed by never reaching the thing it was
    // testing.
    let padding = "x".repeat(1200);
    let bulk: String = (0..2_000)
        .map(|n| format!("{{\"n\":{n},\"pad\":\"{padding}\"}}\n"))
        .collect();
    assert!(
        bulk.len() as u64 > 2 * 1024 * 1024,
        "the fixture is under the cap and would prove nothing"
    );
    std::fs::write(&ledger, &bulk).expect("a large ledger");
    let before = std::fs::read_to_string(&ledger)
        .expect("readable")
        .lines()
        .count();

    session::record(&state, &serde_json::json!({"n": "newest"}));

    let after = std::fs::read_to_string(&ledger).expect("readable");
    assert!(
        after.lines().count() < before,
        "the ledger grew past its cap without being trimmed"
    );
    // The newest line survives, which is the whole point of keeping the tail.
    assert!(
        after.contains("newest"),
        "the trim dropped the newest record"
    );
    // And what is left is still a record per line.
    for line in after.lines() {
        serde_json::from_str::<serde_json::Value>(line).expect("every surviving line parses");
    }
}

#[test]
fn a_cline_payload_reaches_the_classifier_as_the_tool_it_names() {
    // Cline sends `{hookName, tool_call: {id, name, input}}` — no `tool_name`,
    // no `session_id`, no `cwd`. The tool is one level down, and the two
    // absences are handled elsewhere: a missing session means "ask which oath
    // covers this checkout", which is the answer the git hook already uses.
    let body = serde_json::json!({
        "hookName": "PreToolUse",
        "tool_call": {"id": "1", "name": "write_file", "input": {"file_path": "src/x.rs"}}
    })
    .to_string();
    let input = hook::read_input(std::io::Cursor::new(body));
    assert_eq!(input.tool_name, "write_file");

    let (action, how) = classify(&input.tool_name, &input.tool_input);
    assert_eq!(
        action,
        Action::Write {
            target: "src/x.rs".to_owned()
        }
    );
    assert_eq!(how, Sensitivity::Routine);

    // And a payload that already names its tool is left alone: the unwrapping
    // must not fire for the six agents that send the ordinary shape.
    let ordinary = serde_json::json!({
        "tool_name": "Bash", "tool_input": {"command": "git push origin main"}
    })
    .to_string();
    let input = hook::read_input(std::io::Cursor::new(ordinary));
    assert_eq!(input.tool_name, "Bash");
    let (_, how) = classify(&input.tool_name, &input.tool_input);
    assert_eq!(how, Sensitivity::Boundary);
}

#[test]
fn cline_is_told_to_pause_rather_than_to_cancel() {
    // Its two stopping shapes are `cancel`, which kills the whole task, and
    // `review`, which pauses for a person. A claim that could be renewed in one
    // command is not worth throwing away a task over.
    let refusal = crate::outcome::Refusal::not_started(
        "test-only",
        "a refusal",
        crate::outcome::Resolution::run("estigia status"),
    );
    let answer = hook::response_in(hook::Dialect::Cline, &Decision::Deny(Box::new(refusal)));
    assert_eq!(answer["review"], true);
    assert!(
        answer.get("cancel").is_none(),
        "a refusal cancelled the task"
    );
    assert!(
        answer["context"]
            .as_str()
            .is_some_and(|text| text.contains("estigia")),
        "the pause says nothing about why"
    );
}

#[test]
fn every_dialect_answers_to_the_slug_setup_writes_for_it() {
    // The seam that let a whole agent be registered and ignored. `setup` writes
    // `--dialect <slug>` into the hook command and `from_slug` reads it back; a
    // dialect missing from that list falls through to the default, so the agent
    // is handed another agent's JSON and discards it. Registered, running,
    // deciding nothing — for the fifth time in this crate's history, and the
    // first time a compiler could have caught it.
    for dialect in hook::Dialect::all() {
        assert_eq!(
            hook::Dialect::from_slug(dialect.slug()),
            dialect,
            "`{}` does not resolve back to itself",
            dialect.slug()
        );
    }
}

#[test]
fn the_gate_honours_a_stand_down_and_stops_when_it_expires() {
    // Through `gate()` itself, not through `standdown::over` — the unit test
    // proves the decision, this proves the wiring. A stand-down honoured only
    // in a module nobody calls is a feature that exists in its own tests.
    use crate::harness::standdown;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());

    let decide = |declared: Option<standdown::StandDown>| -> Decision {
        let root = tempfile::tempdir().expect("a temporary root");
        let mut context = context(root.path());
        context.stand_down = declared;
        let mut run = sworn(12, &context.repo_dir);
        run.mark_verified();
        let action = Action::Boundary {
            command: "git push".to_owned(),
            pr: None,
            local_fast_forward_target: None,
        };
        gate(&context, &mut run, &action, Sensitivity::Boundary)
    };

    // The baseline has to refuse, or nothing below proves anything.
    let plain = decide(None);
    let Decision::Deny(refusal) = &plain else {
        panic!("the fixture no longer refuses, so this test proves nothing: {plain:?}");
    };
    let code = refusal.code;

    // In force: allowed, and the allowance names what it overrode and why.
    let live = standdown::declare("the tracker is down", 30, now, "operator").expect("declared");
    let Decision::Allow(why) = decide(Some(live)) else {
        panic!("the gate ignored a stand-down in force");
    };
    assert!(
        why.contains(code),
        "the trace does not say what it overrode"
    );
    assert!(why.contains("the tracker is down"), "the reason is missing");

    // Expired: the gate decides on its own again, with the same refusal. This
    // half is what makes it a stand-down rather than a switch.
    let over = standdown::declare("the tracker is down", 1, now - 600, "operator")
        .expect("declared ten minutes ago");
    assert!(
        decide(Some(over)).denies(),
        "an expired stand-down still opened the gate"
    );
}

#[test]
fn trunk_based_swaps_the_review_for_a_flag_rather_than_removing_it() {
    use crate::config::Integration;
    let reason = |refusal: Option<Refusal>| refusal.map(|r| r.code);
    let landing = |integration, flag| {
        reason(out_of_phase(
            "gh pr merge",
            "in-progress",
            12,
            integration,
            flag,
        ))
    };

    // On a branch, nothing changes: a delivery from a state with no verdict is
    // refused, and naming a flag does not buy anything — on a branch the review
    // *is* the protection, and a flag is not a substitute for it.
    assert_eq!(landing(Integration::Branch, None), Some("out-of-phase"));
    assert_eq!(
        landing(Integration::Branch, Some("checkout-v2")),
        Some("out-of-phase"),
        "a flag opened a branch delivery, where it protects nothing"
    );

    // On trunk with a flag named: allowed. This is the trade being made.
    assert_eq!(landing(Integration::Trunk, Some("checkout-v2")), None);

    // On trunk with nothing named: refused, and with its *own* code — the way
    // out is a flag, not a review, and sending somebody to `out-of-phase`
    // would send them to the wrong one.
    assert_eq!(
        landing(Integration::Trunk, None),
        Some("unflagged-on-trunk")
    );

    // Whitespace is not a flag. A variable set to nothing is how a shell says
    // "unset", and reading it as a declaration would open the gate for anybody
    // who exported it once.
    for empty in ["", "   ", "\t"] {
        assert_eq!(
            landing(Integration::Trunk, Some(empty)),
            Some("unflagged-on-trunk"),
            "{empty:?} passed as a flag"
        );
    }

    // And neither mode touches a state where a verdict already exists, nor a
    // command that is not a delivery — the flag rule narrows one branch of the
    // decision, it does not replace it.
    for integration in Integration::all() {
        assert_eq!(
            reason(out_of_phase("gh pr merge", "review", 12, integration, None)),
            None,
            "{integration:?} refused a reviewed delivery"
        );
        assert_eq!(
            reason(out_of_phase(
                "git status",
                "in-progress",
                12,
                integration,
                None
            )),
            None,
            "{integration:?} gated something that delivers nothing"
        );
    }
}

#[test]
fn every_event_name_resolves_to_exactly_one_event() {
    use super::hook::Event;

    // A settings file is a thing people copy and hand-edit, and the name they
    // reach for is the one their agent's documentation prints. Refusing it
    // exits non-zero — which for a `PreToolUse` hook in Claude Code is a
    // *non-blocking* error: the call goes through ungated, with one line in the
    // transcript. A gate that is installed, looks installed and decides nothing
    // is the failure this whole project is written against.
    for event in Event::all() {
        for spelling in [event.slug(), event.agent_name()] {
            assert_eq!(
                Event::from_slug(spelling),
                Some(event),
                "{spelling:?} is a name this build writes and does not read"
            );
        }
        // Case and separators are what differ between a name typed by a person,
        // one written by `setup`, and one copied out of somebody else's docs.
        let slug = event.slug();
        for spelling in [
            slug.to_ascii_uppercase(),
            slug.replace('-', "_"),
            slug.replace('-', ""),
            format!(" {slug} ").trim().to_owned(),
        ] {
            assert_eq!(Event::from_slug(&spelling), Some(event), "{spelling:?}");
        }
    }

    // And no two events collide under that reading, or one settings file would
    // silently register the wrong gate.
    for event in Event::all() {
        for other in Event::all() {
            if event == other {
                continue;
            }
            for mine in [event.slug(), event.agent_name()] {
                assert_ne!(
                    Event::from_slug(mine),
                    Some(other),
                    "{mine:?} reaches two different events"
                );
            }
        }
    }

    // A genuine misspelling is still one, and still says so. Reading a name two
    // ways is not the same as accepting anything.
    for nonsense in ["", "   ", "pre-tool", "tool-use", "presession", "push"] {
        assert_eq!(
            Event::from_slug(nonsense),
            None,
            "{nonsense:?} was read as an event"
        );
    }
}

#[test]
fn a_mistyped_argument_is_not_sent_to_read_the_configuration() {
    use super::tracker::{Answer, translate};

    // Exit `2` carries two different faults — the operator's configuration and
    // the caller's own typo — and both used to be answered with `estigia config
    // list`. Running it discharges the first and does nothing whatever for the
    // second, which is the ratchet's one rule about naming a command.
    //
    // Measured on the real binary: `estigia claim 1 --horizon "not a date"`
    // answered `invalid-horizon` and then sent the operator to read a table
    // that has no horizon in it.
    let refusal = translate(
        &Answer {
            code: 2,
            body: Some(serde_json::json!({ "ok": false, "reason": "invalid-horizon" })),
        },
        "claim",
    )
    .expect("exit 2 is a refusal");
    let message = format!("{refusal}");
    assert!(
        !message.contains("estigia config list"),
        "a mistyped horizon is answered with a command that cannot fix it: {message}"
    );
    // What it says instead is the shape this build actually enforces.
    assert!(message.contains("YYYY-MM-DDTHH:MMZ"), "{message}");
    assert!(message.contains("operator-knowledge"), "{message}");

    // The same for the other argument the transport exits 2 for.
    let refusal = translate(
        &Answer {
            code: 2,
            body: Some(serde_json::json!({ "ok": false, "reason": "invalid-operation-id" })),
        },
        "reclaim",
    )
    .expect("exit 2 is a refusal");
    let message = format!("{refusal}");
    assert!(!message.contains("estigia config list"), "{message}");
    assert!(message.contains("32"), "{message}");

    // And a configuration fault on the same code still names the command that
    // does fix it. Narrowing the resolution must not take it from the one case
    // it was right for.
    let refusal = translate(
        &Answer {
            code: 2,
            body: Some(serde_json::json!({ "ok": false, "reason": "tracker-not-configured" })),
        },
        "claim",
    )
    .expect("exit 2 is a refusal");
    assert!(
        format!("{refusal}").contains("estigia config list"),
        "the configuration case lost the command that fixes it"
    );
}

#[test]
fn nothing_claims_the_gate_sees_every_write() {
    // The gate sees a write made through a tool its matcher covers, in an agent
    // whose calls Estigia can gate — and `status` reports eight of the eleven
    // adapters as contract only. "Every repository write from this run is now
    // measured against that claim" was therefore false for most installations,
    // and it was the sentence somebody reads at the moment they swear. The same
    // sentence was in the MCP tool description, where the reader is the model.
    //
    // The same fault as the push guard's line, in the other message that
    // describes the oath. Pinned in both places so neither drifts back.
    for (what, source) in [
        ("the claim command", include_str!("../cli/mod.rs")),
        ("the MCP tool", include_str!("mcp/tools.rs")),
    ] {
        for overstated in [
            "every repository write from this run",
            "every repository write this run makes",
        ] {
            assert!(
                !source.contains(overstated),
                "{what} claims the gate sees {overstated}"
            );
        }
    }
    // And what replaced it names the condition rather than dropping the claim.
    assert!(
        include_str!("../cli/mod.rs").contains("a repository write the gate sees"),
        "the claim command stopped saying what the oath covers"
    );
    assert!(
        include_str!("mcp/tools.rs").contains("a repository write the gate sees"),
        "the MCP tool stopped saying what the oath covers"
    );
}

#[test]
fn the_gate_reads_the_clock_through_the_one_reader_that_can_say_it_does_not_know() {
    // `standdown::over` takes an `Option<u64>`, so a clock that will not answer
    // is not a moment inside anybody's window — held by
    // `standdown::tests::a_clock_that_will_not_answer_does_not_stand_the_gate_down`.
    // What that cannot reach is this file's own line. The gate hand-rolled the
    // read and defaulted it to the epoch, and the epoch is inside every window
    // ever declared: a stand-down from last March came back in force, and a
    // live one never ran out. Nothing about `over` fails if that default
    // returns here, because it would arrive as a perfectly good `Some(0)`.
    //
    // Structural for the same reason the tracker check above is: a test cannot
    // stop this machine's clock, and a guard that needs a broken clock to fire
    // is a guard that never fires.
    let source = include_str!("mod.rs");
    assert!(
        source.contains("let now = session::now_seconds();"),
        "the gate no longer takes its clock from the one reader that returns None"
    );
    assert!(
        !source.contains("UNIX_EPOCH"),
        "the gate reads the wall clock itself again, and every such read has to \
         choose what a failure means"
    );
}

/// A write under a prefix that runs it is still a write.
///
/// `sudo rm -rf src` hides nothing: the command is on the line, which is
/// `writing-shell`'s own boundary. It was missed anyway, because the head of
/// the segment was `sudo` and `sudo` writes no files. Seven spellings of a
/// write the list already knew went past the gate wearing a hat.
/// The gate measures its own disarmament.
///
/// The whole authority model rests on the agent not being able to switch the
/// gate off. It could: `estigia stand-down`, `estigia guard --uninstall` and
/// `estigia uninstall` all classified as `Untouched` — the answer that means
/// *this is not a write, carry on* — through Bash, the tool an agent uses most.
///
/// What gating them is worth is narrow and worth stating: a run that claimed
/// nothing is `Outside` and refused nothing either way. What changes is the run
/// **holding a claim**, which now has to renew it here, the same as at a merge.
#[test]
fn the_gate_measures_the_commands_that_switch_it_off() {
    // guard:population disarming-shell — every listed subcommand is one the CLI
    // has, and each reaches the classifier as a boundary rather than as nothing.
    let help = include_str!("../cli/mod.rs");
    for line in super::DISARMS_THE_HARNESS {
        // Estigia's own are crossed against the command table, so a made-up
        // member is a guard about nothing. The rest are not this binary's
        // verbs at all — `core.hooksPath` points git away from every hook in
        // the repository and never says the word `estigia` — and what makes
        // one of those honest is the classification below, which every fragment
        // here is held to.
        if let Some(subcommand) = line
            .strip_prefix("estigia ")
            .and_then(|rest| rest.split_whitespace().next())
        {
            assert!(
                help.contains(&format!("{subcommand:?}")) || help.contains(subcommand),
                "`{subcommand}` is gated and is not a command this binary has"
            );
        }

        let (action, how) = crate::harness::classify("Bash", &serde_json::json!({"command": line}));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Boundary,
            "`{line}` switches the gate off and the gate reads it as {how:?}"
        );
        assert!(
            matches!(action, crate::harness::Action::Boundary { .. }),
            "`{line}` reached the classifier as {action:?}"
        );
    }

    // And the other direction, which the crossing above never asked: every name
    // the CLI answers to for a listed verb has to be listed too. `setup` is
    // declared with `visible_aliases = ["install", "tui"]`, so `estigia install
    // --uninstall` took Estigia out and classified as `Untouched` — the same
    // operation as `estigia setup --uninstall`, under a name the product prints
    // in its own `--help`.
    //
    // Read out of the argument definitions rather than restated here, because a
    // list of aliases written beside the list of commands is the second spelling
    // of one question and would drift the moment somebody adds a third name.
    let args = include_str!("../cli/args.rs");
    let mut checked = 0;
    for (at, _) in args.match_indices("visible_aliases = [") {
        let rest = &args[at + "visible_aliases = [".len()..];
        let inside = &rest[..rest.find(']').expect("a closed list")];
        // The verb the attribute sits on is the next `Setup`-style variant.
        let after = &rest[rest.find(']').expect("a closed list")..];
        let verb = after
            .lines()
            .find_map(|line| {
                let word = line.trim().trim_end_matches(&[' ', '{', '(', ','][..]);
                (!word.is_empty()
                    && !word.starts_with('#')
                    && !word.starts_with("//")
                    && !word.starts_with(']'))
                .then(|| word.to_ascii_lowercase())
            })
            .expect("an attribute sits on something");
        if !super::DISARMS_THE_HARNESS
            .iter()
            .any(|line| super::disarming_verb(line) == Some(verb.as_str()))
        {
            continue;
        }
        for alias in inside.split(',') {
            let alias = alias.trim().trim_matches('"');
            if alias.is_empty() {
                continue;
            }
            checked += 1;
            let spelling = format!("estigia {alias}");
            assert!(
                super::DISARMS_THE_HARNESS.contains(&spelling.as_str()),
                "`{spelling}` runs `estigia {verb}`, which is gated, and is not itself gated"
            );
            let (action, how) =
                crate::harness::classify("Bash", &serde_json::json!({"command": spelling}));
            assert_eq!(
                how,
                crate::harness::Sensitivity::Boundary,
                "`{spelling}` switches the gate off and the gate reads it as {how:?}"
            );
            assert!(matches!(action, crate::harness::Action::Boundary { .. }));
        }
    }
    // The alphabet is really there: if the attribute stopped being written this
    // way, "every alias is covered" would be true and empty.
    assert!(
        checked >= 2,
        "only {checked} alias(es) of a gated verb were found, which is fewer than \
         `cli/args.rs` declares \u{2014} this crossing has stopped reading them"
    );

    // Under a wrapper too, and with the absolute path a hook writes.
    for line in [
        "sudo estigia stand-down --reason x",
        r"C:\Users\me\.cargo\bin\estigia.exe uninstall --all",
        "cd /tmp && estigia guard --uninstall",
    ] {
        let (_, how) = crate::harness::classify("Bash", &serde_json::json!({"command": line}));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Boundary,
            "`{line}` switches the gate off and went through as {how:?}"
        );
    }

    // And Estigia's own reads are still reads. A harness that gated every
    // mention of itself would be one nobody keeps switched on.
    for line in [
        "estigia status",
        "estigia doctor",
        "estigia config list",
        "estigia gate --tool Read",
    ] {
        let (_, how) = crate::harness::classify("Bash", &serde_json::json!({"command": line}));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Routine,
            "`{line}` only reads and was gated as a boundary"
        );
    }
}

#[test]
fn a_write_under_a_wrapper_is_still_a_write() {
    // guard:population running-shell — each listed wrapper reaches the command
    // it runs, and a read under the same wrapper stays untouched. A plain `//`
    // inside the body on purpose: a `///` here is parsed as a second
    // *declaration* of the family rather than as this test claiming it.
    use crate::harness::shell::writes_a_file;

    for line in [
        "sudo rm -rf src",
        // Two words between the wrapper and the command, which is why this
        // reads every later word rather than only the next one.
        "sudo -u bob rm src/x.rs",
        "env FOO=1 rm src/x.rs",
        "nohup rm src/x.rs",
        "time rm src/x.rs",
        "command rm src/x.rs",
        "doas rm src/x.rs",
        "busybox rm src/x.rs",
        "timeout 5 rm src/x.rs",
        "find . -name '*.rs' | xargs rm",
        // `find` says what it does on its own line.
        "find . -name '*.rs' -delete",
        "find . -exec rm {} ;",
        // The fifteen that wore the same hat and were not on the list. Each one
        // leaves the write where anybody can read it, which is this family's
        // whole boundary — and each went through untouched while `timeout 5 rm`
        // beside them did not.
        "flock /var/lock/x rm -rf src",
        "taskset 0x1 rm src/x.rs",
        "chrt -f 10 rm src/x.rs",
        "setarch x86_64 rm src/x.rs",
        "eatmydata rm src/x.rs",
        "unbuffer rm src/x.rs",
        "runuser -u bob rm src/x.rs",
        "chroot /jail rm src/x.rs",
        "watch rm src/x.rs",
        "systemd-run rm src/x.rs",
        "parallel rm ::: src/x.rs",
        "strace rm src/x.rs",
        "ltrace rm src/x.rs",
        "xvfb-run rm src/x.rs",
        "proxychains rm src/x.rs",
        "torify rm src/x.rs",
    ] {
        assert!(
            writes_a_file(line).is_some(),
            "{line:?} writes a file in plain sight and went through untouched"
        );
    }

    // And a read under the same wrapper is left alone, which is the property
    // that keeps the harness switched on.
    for line in [
        "sudo ls -la",
        "sudo cat src/x.rs",
        "env FOO=1 cargo test",
        "time cargo build",
        "nohup npm run dev",
        // No `-exec`, so nothing runs: `rm` here is a filename to look for.
        "find . -name rm",
        // The floor for the fifteen above: widening the list must not turn a
        // read under one of them into a write, or the assertions above would be
        // satisfied by a function that answers yes to everything.
        "flock /var/lock/x cargo test",
        "strace cargo build",
        "watch git status",
        "parallel echo ::: a b",
        "runuser -u bob ls -la",
        "systemd-run cargo build",
    ] {
        assert_eq!(
            writes_a_file(line),
            None,
            "{line:?} reads and was gated as a write"
        );
    }
}

/// A write to what the harness reads its answers from is not an ordinary write.
///
/// `disarming-shell` closed the road through the shell. This is the shorter
/// one: writing `~/.estigia/standdown.json` stands the gate down with no
/// subcommand at all, writing a run pointer grants a claim nobody made, and
/// removing the entry from an agent's settings ends the gate. Every one of
/// those was `Routine` — the same answer as `src/main.rs` — through the `Write`
/// tool every agent has.
#[test]
fn the_gate_measures_writes_to_its_own_control_surface() {
    // Each of these is a file whose contents decide what the harness enforces,
    // and each reaches the classifier as a boundary rather than an ordinary edit.
    let home = "C:/Users/me";
    for target in [
        format!("{home}/.estigia/standdown.json"),
        format!("{home}/.estigia/runs/claude-abc123.json"),
        format!("{home}/.estigia/decisions.jsonl"),
        format!("{home}/.agents/skills/issue-flow/SKILL.md"),
        format!("{home}/.agents/skills/issue-flow/operator.local.md"),
        format!("{home}/.agents/skills/issue-flow/estigia.qwen.md"),
        "H:/REPO/mine/.git/hooks/pre-push".to_owned(),
        // The separator the platform hands the tool, not the one this crate
        // happens to write.
        home.replace('/', "\\") + r"\.estigia\standdown.json",
    ] {
        let (_, how) =
            crate::harness::classify("Write", &serde_json::json!({ "file_path": target.clone() }));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Boundary,
            "a write to {target} changes what the gate enforces and the gate calls it routine"
        );
    }

    // And ordinary work stays ordinary. A harness that treated every edit as a
    // boundary would ask the tracker on every keystroke, which is a harness
    // nobody keeps switched on.
    for target in [
        "src/main.rs",
        "README.md",
        "C:/Users/me/notes/estigia-ideas.md",
        // Their own settings, in a project that is not an agent's home.
        "H:/REPO/mine/.vscode/settings.json",
    ] {
        let (_, how) =
            crate::harness::classify("Write", &serde_json::json!({ "file_path": target }));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Routine,
            "{target} is ordinary work and was gated as a boundary"
        );
    }
}

#[test]
fn the_directory_the_installer_writes_to_is_the_one_the_gate_measures() {
    // `CONTROL_SURFACE` names the skill tree by directory, and it matches on
    // fragments: `skills/issue-flow/` does not match `skills/flow/`. So renaming
    // `skill::DIRECTORY` and leaving the list alone would make the contract —
    // the file the gate reads its own settings out of — writable on a `Routine`
    // answer, which is the disarmament the whole list exists to refuse. Nothing
    // crossed the two, and the rename to `flow` is exactly the change that would
    // have gone through green.
    //
    // Derived from `DIRECTORY` rather than spelled, because a test that repeats
    // the constant is a test the next rename edits into agreement without ever
    // going red. Turn the `skills/flow/` entry off and this fails.
    let home = "C:/Users/me";
    for agent in [".claude", ".agents", ".codex"] {
        let target = format!("{home}/{agent}/skills/{}/SKILL.md", crate::skill::DIRECTORY);
        let (_, how) =
            crate::harness::classify("Write", &serde_json::json!({ "file_path": &target }));
        assert_eq!(
            how,
            crate::harness::Sensitivity::Boundary,
            "the installed contract is not measured at the gate: {target}"
        );
    }

    // And the shell road to the same directory, which was the unmeasured one
    // when this list was wired into `Write` alone.
    let command = format!("rm -rf $HOME/.claude/skills/{}", crate::skill::DIRECTORY);
    assert_eq!(
        classify_with("Bash", &serde_json::json!({ "command": &command }), &[]).1,
        Sensitivity::Boundary,
        "{command} went past as an ordinary write"
    );
}

#[test]
fn the_directory_the_previous_name_installed_into_is_still_measured() {
    // The rename does not retire the old fragment, and this says why in a place
    // that fails if somebody tidies it away. An operator who ran `issue-flow`
    // still has that tree on disk until an upgrade moves it, and until then it
    // is a contract an agent can read and therefore one a run must not rewrite
    // unmeasured. Removing the entry costs nothing visible and reopens the gate
    // on every machine that has not upgraded yet.
    let target = "C:/Users/me/.agents/skills/issue-flow/SKILL.md";
    let (_, how) = crate::harness::classify("Write", &serde_json::json!({ "file_path": target }));
    assert_eq!(
        how,
        crate::harness::Sensitivity::Boundary,
        "the tree the previous name installed into stopped being measured"
    );
}

#[test]
fn a_shell_line_reaches_the_control_surface_the_same_way_the_write_tool_does() {
    // This was measured through the `Write` tool and not through the shell, so
    // `rm ~/.claude/settings.json` — the cheapest disarmament there is — came
    // back `Routine` while `Write(~/.claude/settings.json)` came back
    // `Boundary`. The list exists because an agent could switch the gate off
    // with the tool it uses most, and the tool it uses most is the shell.
    for command in [
        "rm ~/.claude/settings.json",
        // No trailing separator, where the fragment has one.
        "rm -rf $HOME/.claude/skills/issue-flow",
        "echo '{}' > ~/.claude/settings.json",
        "truncate -s 0 ~/.codex/hooks.json",
        // Not a deletion: moving it away ends the gate just as completely.
        "mv ~/.claude/settings.json /tmp/x",
        "rm -rf ~/.estigia",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Boundary,
            "{command} went past as an ordinary write"
        );
    }

    // And an ordinary write stays ordinary, or the harness gates `echo`.
    for command in ["echo hi > src/main.rs", "rm target/debug/x", "ls ~/.claude"] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Routine,
            "{command} was raised to a boundary"
        );
    }

    // The two roads answer alike, which is the whole point of one function.
    let shell = serde_json::json!({ "command": "rm ~/.codex/hooks.json" });
    let tool = serde_json::json!({ "file_path": "~/.codex/hooks.json" });
    assert_eq!(
        classify_with("Bash", &shell, &[]).1,
        classify_with("Write", &tool, &[]).1
    );
}

#[test]
fn discarding_uncommitted_work_is_seen_whichever_git_spelling_does_it() {
    // `git reset --hard` was on `REPOSITORY_SHELL` and the ordinary ways of
    // doing the same thing to the same tree were not, so they read as
    // `Untouched` — invisible to the gate, where their neighbour renewed the
    // claim. Each of these names git and destroys work that was never
    // committed, which is the population that list already declared.
    for command in [
        "git checkout -- src/main.rs",
        "git checkout .",
        "git restore src/main.rs",
        "git restore --staged --worktree .",
        "git stash",
        "git clean -fd",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Write { .. } | Action::Boundary { .. }
            ),
            "{command} was invisible to the gate"
        );
    }

    // Switching branches discards nothing — git refuses when it would — so the
    // two checkout spellings are listed rather than the bare word.
    let switching = serde_json::json!({ "command": "git checkout main" });
    assert_eq!(
        classify_with("Bash", &switching, &[]).0,
        Action::Untouched,
        "changing branch was read as discarding work"
    );
}

#[test]
fn a_merge_performed_through_the_api_is_a_boundary_like_any_other() {
    // Two declarations handed this gap to each other. `irreversible-shell` gave
    // `gh api` as its example of a spelling it does *not* match, and
    // `delivery-phase` told its reader that `irreversible-shell` "already owns"
    // exactly that escape. Nobody owned it, and
    // `gh api -X PUT repos/o/r/pulls/7/merge` merges a pull request.
    for command in [
        "gh api -X PUT repos/o/r/pulls/7/merge",
        // The method after the path, which no single substring could catch.
        "gh api repos/o/r/pulls/7/merge -X PUT",
        "gh api --method DELETE repos/o/r/git/refs/heads/main",
        "gh api --method=POST repos/o/r/releases",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Boundary,
            "{command} reached the tracker unmeasured"
        );
    }

    // Reading through the same tool is not a boundary, or every lookup renews a
    // claim and the harness becomes one nobody keeps on.
    for command in ["gh api repos/o/r/pulls/7", "gh api user"] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).0,
            Action::Untouched,
            "{command} only reads"
        );
    }
}

#[test]
fn deleting_a_ref_is_a_repository_write_like_changing_one() {
    // `repository-shell` names "the working tree, the index or a ref", and the
    // two ordinary ways of deleting a ref were not on it.
    for command in ["git branch -D feat/x", "git branch --delete feat/x"] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Write { .. } | Action::Boundary { .. }
            ),
            "{command} was invisible to the gate"
        );
    }

    // Creating one is not deleting one.
    let creating = serde_json::json!({ "command": "git branch feat/x" });
    assert_eq!(classify_with("Bash", &creating, &[]).0, Action::Untouched);
}

#[test]
fn taking_a_control_surface_out_of_service_counts_even_though_it_writes_nothing() {
    // A `pre-push` hook without its execute bit is not a hook: git skips it,
    // silently. Nothing about the file changed, so the write reader was right
    // not to call it a write — and the gate saw nothing at all, which is how
    // `chmod -x .git/hooks/pre-push` ends the push boundary while every report
    // goes on saying it is installed.
    for command in [
        "chmod -x .git/hooks/pre-push",
        "chmod 000 ~/.claude/settings.json",
        "chmod 000 ~/.estigia/runs",
        "chown nobody ~/.codex/hooks.json",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Boundary,
            "{command} took the harness out of service unmeasured"
        );
    }

    // And only together with a surface. A harness that gates `chmod +x
    // build.sh` is one nobody keeps on.
    for command in [
        "chmod +x build.sh",
        "chmod 644 src/main.rs",
        "chown me /tmp/x",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).0,
            Action::Untouched,
            "{command} is nobody's business"
        );
    }
}

#[test]
fn a_release_keeps_its_own_operation_id_and_does_not_borrow_the_claims() {
    // `unassign` used to mint a fresh id on every call and store none, beside
    // the two lines that reuse one for `claim` and `reclaim` and say why. It is
    // the same why: the transport answers a repeated release from the marker
    // already on the issue, and a second call carrying a different id never
    // reaches that path — so the retry the taxonomy asks for after an ambiguous
    // write posted a second release comment instead.
    let root = tempfile::tempdir().expect("a state root");
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(7);
    run.operation_id = Some("a".repeat(32));
    run.release_id = Some("b".repeat(32));
    session::store(root.path(), &run).expect("the pointer writes");

    let read = session::load(root.path(), "claude-abcd1234");
    assert_eq!(
        read.release_id.as_deref(),
        Some("b".repeat(32).as_str()),
        "a release id that does not survive the pointer cannot be reused"
    );
    assert_ne!(
        read.release_id, read.operation_id,
        "a release under the claim's own id is a conflict the transport refuses by name"
    );

    // A pointer written before this field existed loads as a run that has not
    // released yet, rather than failing to parse — which would deny every write
    // for that run on the first upgrade.
    let older = serde_json::json!({
        "run_id": "claude-99999999",
        "issue": 7,
        "operation_id": "c".repeat(32)
    });
    let older = serde_json::from_value::<Run>(older).expect("an older pointer still loads");
    assert_eq!(older.release_id, None);
}

#[test]
fn a_tool_recording_what_only_it_knows_does_not_lose_it_to_a_racing_hook() {
    // `store` drops a write made from a pointer that has since moved. Its own
    // note calls that "the only direction that fails closed", and for the hook
    // it is: what the hook records is when it last asked, and the fresher answer
    // has a better one.
    //
    // It is the wrong way round for a tool. `claim` records the issue and
    // `start-branch` records the isolated checkout, and the fresher pointer does
    // not carry either — so `covered` stops containing the worktree and a write
    // inside it passes as `Outside`. That is the hole the revision check was
    // added to close, reopened from the other side by the fix for it.
    let root = tempfile::tempdir().expect("a state root");
    session::store(root.path(), &Run::new("claude-abcd1234".to_owned())).expect("writes");

    // A hook stores between the tool's read and its write, which the pointer's
    // own documentation calls the ordinary case rather than an unlucky one.
    let racing = || {
        let mut hook = session::load(root.path(), "claude-abcd1234");
        hook.mark_verified();
        session::store(root.path(), &hook).expect("writes");
    };
    // Inside the change, because that is the only place it bites: the change
    // runs between `update`'s own read and its write, which is exactly the
    // window a hook storing after an allowed write lands in.
    let mut first = true;
    let after = session::update(root.path(), "claude-abcd1234", |run| {
        if first {
            first = false;
            racing();
        }
        run.issue = Some(7);
        run.worktree = Some(std::path::PathBuf::from("/w/7"));
    });
    assert_eq!(after.issue, Some(7));
    let read = session::load(root.path(), "claude-abcd1234");
    assert_eq!(
        read.issue,
        Some(7),
        "the issue the tool recorded was dropped"
    );
    assert!(
        read.worktree.is_some(),
        "the isolated checkout was dropped, so the gate stops covering it"
    );

    // And `store` now says which it did, so a caller can tell the two apart.
    let stale = session::load(root.path(), "claude-abcd1234");
    racing();
    assert_eq!(
        session::store(root.path(), &stale),
        Ok(false),
        "a write that was dropped reported itself as made"
    );
}

#[test]
fn a_released_issue_does_not_come_back_because_a_hook_was_slow() {
    // The hook stores after every allowed write, and the pointer's own
    // documentation calls a store racing a load "the ordinary case rather than
    // an unlucky one". So a hook holding a copy from before a release stored it
    // back afterwards, and the issue came with it: the run held one it had put
    // down, and `claim` refused it the next one by name.
    //
    // The revision cannot carry this on its own — a removed pointer reads as
    // revision zero, which every copy in hand is newer than.
    let root = tempfile::tempdir().expect("a state root");
    let mut held = Run::new("claude-abcd1234".to_owned());
    held.issue = Some(7);
    session::store(root.path(), &held).expect("writes");

    let mut hook = session::load(root.path(), "claude-abcd1234");
    session::forget(root.path(), "claude-abcd1234");
    hook.mark_verified();
    assert_eq!(
        session::store(root.path(), &hook),
        Ok(false),
        "a pointer that was released was written back"
    );
    assert_eq!(
        session::load(root.path(), "claude-abcd1234").issue,
        None,
        "the issue this run put down came back"
    );

    // And the run can swear to the next one, which is what the whole release
    // was for.
    let next = session::update(root.path(), "claude-abcd1234", |run| run.issue = Some(8));
    assert_eq!(next.issue, Some(8));
    assert_eq!(
        session::load(root.path(), "claude-abcd1234").issue,
        Some(8),
        "a fresh claim after a release could not be recorded"
    );
}

#[test]
fn declaring_a_boundary_only_ever_tightens_what_the_gate_says() {
    // The asymmetry this whole crate runs on, as a property: an operator's
    // `Irreversible commands` may make the gate ask about more, never about
    // less. Nothing checked it — the declarations are threaded into the middle
    // of a chain of matches, and a fragment that matched earlier than a
    // built-in would change *which* boundary was recorded, which the delivery
    // list then compares by exact equality.
    //
    // Generated on both axes, because the interesting case is a declaration
    // that overlaps a built-in rather than one that sits beside it.
    let mut state: u64 = 0x2026_0805;
    let mut next = |bound: usize| -> usize {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % bound.max(1)
    };

    const COMMANDS: &[&str] = &[
        "git push origin main",
        "git merge --ff-only feat/x",
        "git tag -a v1.0.0 -m x",
        "gh pr merge 7 --squash",
        "gh release create v1.0.0",
        "gh api -X PUT repos/o/r/pulls/7/merge",
        "git commit -am x",
        "git checkout -- src/main.rs",
        "git branch -D feat/x",
        "rm ~/.claude/settings.json",
        "chmod -x .git/hooks/pre-push",
        "estigia stand-down --reason x",
        "echo hi > src/main.rs",
        "cargo test",
        "ls -la",
        "make deploy",
        "npm run release",
        "git worktree add -- ../w fix/6",
        "cargo build --release",
        "pytest -q",
        "docker build -t x .",
        "terraform apply -auto-approve",
        "kubectl delete pod web-7",
        "./deploy.sh production",
        "curl -X POST https://api.example.com/ship",
        "python -m twine upload dist/*",
    ];
    // Fragments an operator might really declare, including ones that overlap
    // a built-in and ones that would match almost everything.
    const DECLARED: &[&str] = &[
        "make deploy",
        "npm run release",
        "git",
        "git push",
        "git merge",
        "gh",
        "rm",
        "e",
        " ",
        "cargo test",
        "estigia",
        "terraform apply",
        "kubectl delete",
        "./deploy.sh",
        "twine upload",
        "docker build",
        "curl -X POST",
    ];
    const TOOLS: &[&str] = &["Bash", "Write", "Edit", "Read"];

    let mut looser: Vec<String> = Vec::new();
    let mut raised = 0;
    for _ in 0..1500 {
        let tool = TOOLS[next(TOOLS.len())];
        let command = COMMANDS[next(COMMANDS.len())];
        let input = if tool == "Bash" {
            serde_json::json!({ "command": command })
        } else {
            serde_json::json!({ "file_path": command })
        };
        let mut extra: Vec<String> = Vec::new();
        for _ in 0..next(3) {
            extra.push(DECLARED[next(DECLARED.len())].to_owned());
        }
        // Half the time, a declaration taken from the command in front of us —
        // an operator declares what they actually run. This is also the case
        // that presses hardest on the built-ins, because the first word of
        // `git push origin main` is a fragment that would match before them if
        // the chain were ordered the other way.
        if next(2) == 0 {
            let word = command
                .split_whitespace()
                .take(next(2) + 1)
                .collect::<Vec<_>>()
                .join(" ");
            extra.push(word);
        }

        let (bare_action, bare) = classify_with(tool, &input, &[]);
        let (declared_action, declared) = classify_with(tool, &input, &extra);

        // Sensitivity may only go up.
        if bare == Sensitivity::Boundary && declared != Sensitivity::Boundary {
            looser.push(format!(
                "{command:?} with {extra:?}: {bare:?} became {declared:?}"
            ));
        }
        if bare != Sensitivity::Boundary && declared == Sensitivity::Boundary {
            raised += 1;
        }
        // And a command the gate already watched may not become one it ignores.
        if !matches!(bare_action, Action::Untouched) && matches!(declared_action, Action::Untouched)
        {
            looser.push(format!(
                "{command:?} with {extra:?}: {bare_action:?} became untouched"
            ));
        }
        // A built-in boundary keeps its own name, or the delivery list — which
        // compares the recorded fragment by exact equality — stops recognising
        // it.
        if let (
            Action::Boundary { command: was, .. },
            Action::Boundary {
                command: now_named, ..
            },
        ) = (&bare_action, &declared_action)
            && was != now_named
        {
            looser.push(format!(
                "{command:?} with {extra:?}: {was:?} renamed {now_named:?}"
            ));
        }
    }
    assert!(
        raised > 100,
        "no declaration ever tightened anything, so this refutes nothing: {raised}"
    );
    assert!(
        looser.is_empty(),
        "{} declarations loosened the gate: {:?}",
        looser.len(),
        &looser[..looser.len().min(6)]
    );
}

#[test]
fn each_way_the_gate_stands_aside_says_a_different_thing() {
    // The four are not interchangeable: one says the tool is not watched, one
    // says nothing was sworn, one says the tracker cannot be asked, one says
    // the claim is over a different checkout. They were one sentence, and it
    // was the first one — so `estigia gate Bash --input '{"command":"git push
    // origin main"}'` answered "Bash is not something this run's oath covers"
    // about a boundary command through a watched tool.
    //
    // That answer is what OpenCode's plugin receives, and what the one command
    // whose stated purpose is *working out why a write was refused* prints.
    let every = [
        Aside::NotWatched,
        Aside::NothingSworn,
        Aside::NoTracker,
        Aside::AnotherCheckout,
    ];
    let said: std::collections::BTreeSet<String> =
        every.iter().map(|aside| aside.why("Bash")).collect();
    assert_eq!(
        said.len(),
        every.len(),
        "two ways of standing aside give the same sentence: {said:#?}"
    );
    // And only the one that means it says the tool is not covered.
    for aside in every {
        let why = aside.why("Bash");
        assert_eq!(
            aside == Aside::NotWatched,
            why.contains("is not something"),
            "{aside:?} says {why:?}"
        );
        assert!(why.starts_with("Bash"), "{aside:?} does not name the tool");
    }
}

/// A delivery on a verdict bound to bytes that have moved is refused; the push
/// that moved them is not.
///
/// The rule the product is named for held where the verdict is **written** and
/// not where it is **used**: `publish-review` binds the target and reads it
/// back, and the boundary that spends it asked only whether the claim was still
/// live — which it is, across any number of pushes.
///
/// The half that must not regress is the second one. Pushing after a review is
/// how a run fixes what the review found, and the contract's answer to a moved
/// head is *re-publish*, not *stop*. A check that refused the push would refuse
/// the repair, so `git push` is absent from `DELIVERS` and this asserts it.
#[test]
fn a_delivery_on_a_moved_head_is_refused_and_the_push_that_moved_it_is_not() {
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .output()
            .expect("git runs")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.invalid"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(repo.path().join("a.txt"), "one").expect("a file");
    git(&["add", "-A"]);
    git(&["commit", "-qm", "one"]);
    let reviewed = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert!(!reviewed.is_empty(), "the fixture has no head");

    let mut run = Run::new("claude-abcd1234".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    run.review_receipt = Some(crate::transport::claim::ReviewReceipt {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });

    // Nothing has moved: the delivery is not this check's business.
    assert!(
        stale_verdict(
            &Action::Boundary {
                command: "gh pr merge".to_owned(),
                pr: Some(54),
                local_fast_forward_target: None,
            },
            &run,
            repo.path(),
        )
        .is_none(),
        "a delivery on the head that was reviewed was refused"
    );

    // A run with nothing published is left exactly as it was.
    let mut unpublished = run.clone();
    unpublished.review_receipt = None;
    let missing = stale_verdict(
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            local_fast_forward_target: None,
        },
        &unpublished,
        repo.path(),
    )
    .expect("a PR merge spent no complete receipt");
    assert_eq!(missing.code, "complete-review-receipt-missing");

    // The push that moves it.
    std::fs::write(repo.path().join("a.txt"), "two").expect("a change");
    git(&["add", "-A"]);
    git(&["commit", "-qm", "two"]);
    let now = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert_ne!(now, reviewed, "the fixture did not move the head");

    // The repair loop stays open: none of these is a delivery.
    for allowed in ["git push", "gh pr create"] {
        assert!(
            stale_verdict(
                &Action::Boundary {
                    command: allowed.to_owned(),
                    pr: None,
                    local_fast_forward_target: None,
                },
                &run,
                repo.path(),
            )
            .is_none(),
            "`{allowed}` was refused, which is how a run fixes what a review found"
        );
    }

    // And the delivery that would spend the stale verdict.
    for delivery in ["gh pr merge", "git merge"] {
        let refusal = stale_verdict(
            &Action::Boundary {
                command: delivery.to_owned(),
                pr: (delivery == "gh pr merge").then_some(54),
                local_fast_forward_target: None,
            },
            &run,
            repo.path(),
        )
        .unwrap_or_else(|| panic!("`{delivery}` delivered on a verdict bound to other bytes"));
        assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
        assert!(
            refusal.message.contains(&reviewed[..7]) && refusal.message.contains(&now[..7]),
            "the refusal does not name both heads: {}",
            refusal.message
        );
    }

    // A matching SHA in another clone is not this run's reviewed checkout.
    let elsewhere = tempfile::tempdir().expect("a parent for an unrelated clone");
    let unrelated = elsewhere.path().join("unrelated");
    let cloned = std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(repo.path())
        .arg(&unrelated)
        .output()
        .expect("git clones the fixture");
    assert!(
        cloned.status.success(),
        "the unrelated clone was not created"
    );
    let checked_out = std::process::Command::new("git")
        .arg("-C")
        .arg(&unrelated)
        .args(["checkout", "-q", &reviewed])
        .output()
        .expect("git checks out the reviewed SHA elsewhere");
    assert!(
        checked_out.status.success(),
        "the unrelated clone did not reach the reviewed SHA"
    );
    let refusal = stale_verdict(
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            local_fast_forward_target: None,
        },
        &run,
        &unrelated,
    )
    .expect("an unrelated clone spent this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
    assert!(refusal.message.contains(&unrelated.display().to_string()));

    // Nor does an unreadable HEAD become evidence that the reviewed bytes are
    // present merely because the path itself is covered.
    let unreadable = repo.path().join("not-a-checkout");
    std::fs::create_dir(&unreadable).expect("an ordinary directory");
    std::fs::write(unreadable.join(".git"), "not a gitdir pointer")
        .expect("an unreadable nested checkout marker");
    let refusal = stale_verdict(
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            local_fast_forward_target: None,
        },
        &run,
        &unreadable,
    )
    .expect("an unreadable head spent this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
    assert!(
        refusal.message.contains("an unreadable head")
            && refusal.message.contains(&unreadable.display().to_string())
    );
}

#[test]
fn a_steered_environment_cannot_spend_another_checkout_s_verdict() {
    if std::env::var_os("ESTIGIA_STEERED_DELIVERY_CHILD").is_some() {
        assert_steered_delivery_child();
        return;
    }
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .output()
            .expect("git runs")
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@example.invalid"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(repo.path().join("a.txt"), "one").expect("a file");
    git(&["add", "-A"]);
    git(&["commit", "-qm", "one"]);
    let reviewed = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert!(!reviewed.is_empty(), "the fixture has no head");

    let elsewhere = tempfile::tempdir().expect("a parent for an unrelated clone");
    let unrelated = elsewhere.path().join("unrelated");
    let cloned = std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(repo.path())
        .arg(&unrelated)
        .output()
        .expect("git clones the fixture");
    assert!(
        cloned.status.success(),
        "the unrelated clone was not created"
    );
    let checked_out = std::process::Command::new("git")
        .arg("-C")
        .arg(&unrelated)
        .args(["checkout", "-q", &reviewed])
        .output()
        .expect("git checks out the reviewed SHA elsewhere");
    assert!(
        checked_out.status.success(),
        "the unrelated clone did not reach the reviewed SHA"
    );

    // A child process, not this one: exporting `GIT_DIR` here would steer every
    // other unit test that shells out to git. The defect is inheritance, so the
    // child is born with the variable and this process never holds it.
    let executable = std::env::current_exe().expect("the test executable");
    let git_dir = repo.path().join(".git");
    for name in ["GIT_DIR", "GIT_COMMON_DIR"] {
        let output = std::process::Command::new(&executable)
            .args([
                "--exact",
                "harness::tests::a_steered_environment_cannot_spend_another_checkout_s_verdict",
                "--nocapture",
            ])
            .env("ESTIGIA_STEERED_DELIVERY_CHILD", "1")
            .env(name, &git_dir)
            .env("ESTIGIA_STEERED_REPO", repo.path())
            .env("ESTIGIA_STEERED_UNRELATED", &unrelated)
            .env("ESTIGIA_STEERED_REVIEWED", &reviewed)
            .output()
            .unwrap_or_else(|error| panic!("{name} child did not start: {error}"));
        assert!(
            output.status.success(),
            "{name} child failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_steered_delivery_child() {
    let repo = std::path::PathBuf::from(
        std::env::var_os("ESTIGIA_STEERED_REPO").expect("the parent names the reviewed checkout"),
    );
    let unrelated = std::path::PathBuf::from(
        std::env::var_os("ESTIGIA_STEERED_UNRELATED")
            .expect("the parent names the unrelated clone"),
    );
    let reviewed = std::env::var("ESTIGIA_STEERED_REVIEWED").expect("the parent names the head");
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.repo_dir = Some(repo.clone());
    run.review_receipt = Some(crate::transport::claim::ReviewReceipt {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed,
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    let delivery = Action::Boundary {
        command: "gh pr merge".to_owned(),
        pr: Some(54),
        local_fast_forward_target: None,
    };
    assert!(
        stale_verdict(&delivery, &run, &repo).is_none(),
        "a steered environment refused a delivery on the checkout that was reviewed"
    );
    let refusal = stale_verdict(&delivery, &run, &unrelated)
        .expect("a steered environment let an unrelated clone spend this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
    assert!(refusal.message.contains(&unrelated.display().to_string()));
}

#[test]
fn what_a_push_is_aimed_at_is_recorded_and_not_adjudicated() {
    // The gap, measured rather than asserted, because it sits at the boundary
    // the product is named for.
    //
    // `git push` is deliberately absent from `DELIVERS`, and the reason is
    // written where the list is used: publishing a review target is how a run
    // *reaches* review and pushing after one is how it fixes what the review
    // found, so refusing those would refuse the repair. What that reasoning
    // assumes is that a push is a step **towards** delivery. A push aimed at the
    // base branch is the delivery itself, and it never runs `git merge`.
    //
    // The one thing that tells those apart is the destination ref. git hands it
    // to a `pre-push` hook on standard input, for free — and the `PrePush` arm
    // calls `guard::decide` without reading a byte of it, so both doors see the
    // same three words.
    //
    // For the eight adapters that get the contract and no tool gate, this hook
    // is the only enforcement there is.
    assert!(
        !DELIVERS.contains(&"git push"),
        "`git push` is a delivery now, so the note below and the honesty contract \
         entry it belongs to are both out of date"
    );
    // The floor: the list is the one this is about, and it is not empty.
    assert!(
        DELIVERS.contains(&"git merge") && DELIVERS.len() >= 5,
        "the delivery population moved: {DELIVERS:?}"
    );

    // And what the push boundary is adjudicated as: three words, carrying no
    // ref. A test that read this from the decision would need a tracker; the
    // shape is the part that matters and it is right here.
    let source = include_str!("guard.rs");
    assert!(
        source.contains("command: \"git push\".to_owned()"),
        "the push guard stopped naming its action, so what it adjudicates is no \
         longer what this measures"
    );
    // Pinned on both halves, because they have moved apart. The refs are now
    // **read and written down** — the ledger names which push was decided on
    // rather than only that one was — and they are still **not adjudicated**:
    // what `decide` is handed is the checkout, and what it answers about is the
    // three words above.
    //
    // The first version of this assertion said nobody read them, and it went on
    // passing after they were read, because the reading moved into a helper and
    // out of the window it looked at. A pin on the absence of a word is a pin a
    // rename defeats.
    let handler = include_str!("../cli/mod.rs");
    let at = handler
        .find("harness::hook::Event::PrePush =>")
        .expect("the pre-push arm is in this file");
    // To where the arm actually ends, not a count of characters. It was
    // `at + 1600`, and adding eight lines of comment inside the arm pushed
    // `refs_being_pushed()` out of the window and failed this test for a change
    // that moved nothing. The doc above already names this shape one way round
    // — *a pin on the absence of a word is a pin a rename defeats* — and a pin
    // on a fixed slice is the same mistake facing the other way.
    let arm = &handler[at..];
    let arm = &arm[..arm.find("\n        _ =>").unwrap_or(arm.len())];
    assert!(
        arm.contains("harness::guard::adjudicate_action(")
            && arm.contains("command: \"git push\".to_owned()")
            && arm.contains("pr: None"),
        "the pre-push arm no longer decides one un-targeted git-push action"
    );
    assert!(
        arm.contains("refs_being_pushed()"),
        "the refs stopped reaching the ledger, so a push can be adjudicated and not named"
    );
    // What closing the gap would look like: a decision that is given them.
    // `decide` takes a checkout and nothing else, and its argument list is the
    // shape to watch.
    assert!(
        source.contains("pub fn decide(context: &GateContext, repo_dir: &Path) -> Decision"),
        "the push decision takes something new; if it is the refs, the honesty contract \
         entry saying nothing adjudicates a destination is out of date"
    );
}

/// Two run ids never share one pointer.
///
/// `pointer_path` replaces every character outside `[A-Za-z0-9-]` with `_` — a
/// guard against walking out of the directory, and correct for that. What it
/// also did was **fold**: `_` is one of the things it produces, and a
/// case-insensitive filesystem folds the rest. Measured through the running
/// server, two ids and one record:
///
/// ```text
/// claude-aaaa1111 -> wrote claude-aaaa1111.json
/// Claude-AAAA1111 -> "this run's record exists and cannot be read"
/// ```
///
/// It read perfectly. It belonged to another run — and the message named a
/// broken file, which is a cause an operator would go looking for.
///
/// The transport states the rule for its own use of a run id and refuses rather
/// than folds: *any transformation that could fold two distinct IDs into one is
/// refused rather than applied*.
#[test]
fn two_run_ids_never_name_one_pointer() {
    let root = std::path::Path::new("/state");
    let path = |run_id: &str| super::session::pointer_path(root, run_id);

    // The pairs that shared a file. Compared case-insensitively, because the
    // filesystem this crate is written on does not tell them apart.
    for (one, other) in [
        ("claude-aaaa1111", "Claude-AAAA1111"),
        ("claude/aaaa", "claude_aaaa"),
        ("a b", "a-b"),
        ("x", "X"),
        ("", "_"),
    ] {
        let (here, there) = (path(one), path(other));
        assert_ne!(
            here.display().to_string().to_lowercase(),
            there.display().to_string().to_lowercase(),
            "{one:?} and {other:?} share a pointer"
        );
    }

    // A name that maps to itself keeps the file it is already in: this must not
    // orphan the pointers on disk, which would read as a run holding nothing.
    for canonical in [
        "claude-aaaa1111",
        "codex-4d1f0f7aed7c056b",
        "opencode-unknown",
    ] {
        assert_eq!(
            path(canonical),
            root.join(format!("{canonical}.json")),
            "a well-formed run id was moved to another file"
        );
    }

    // And the property the replacement was written for is still true.
    for hostile in [
        "../../elsewhere",
        r"..\..\elsewhere",
        "/etc/passwd",
        "a/b/c",
    ] {
        let path = path(hostile);
        assert_eq!(
            path.parent(),
            Some(root),
            "{hostile:?} named a file outside the run directory: {}",
            path.display()
        );
    }
}

/// Every gated verb is one the alias crossing can see.
///
/// That crossing requires each alias of a gated verb to be gated too, and it
/// found the verb by comparing the **whole** tail after `estigia `. Two of the
/// listed spellings carry a flag, so the comparison answered "no verb" for them
/// and they were skipped in silence — including `estigia guard --uninstall`,
/// which takes the push guard out. An alias on `guard` would then do exactly
/// what `estigia install --uninstall` did: disarm Estigia under a name the
/// product prints in its own help, classifying as `Untouched`.
///
/// A skip that is silent is the failure here, not the missing alias: nothing
/// would have said the verb had stopped being checked.
#[test]
fn a_gated_spelling_that_carries_a_flag_still_names_its_verb() {
    assert_eq!(
        super::disarming_verb("estigia uninstall"),
        Some("uninstall")
    );
    assert_eq!(
        super::disarming_verb("estigia guard --uninstall"),
        Some("guard"),
        "the entry that takes the push guard out names no verb, so its aliases go unchecked"
    );
    assert_eq!(super::disarming_verb("estigia config set"), Some("config"));
    // Not everything on the list is a verb of Estigia's, and inventing one for
    // `core.hookspath` would have the crossing demand aliases for a git key.
    assert_eq!(super::disarming_verb("core.hookspath"), None);

    // And every listed spelling that does name a verb names one the CLI really
    // has. Asked of clap rather than of the source text: a verb is spelled from
    // its variant name, so `StandDown` is `stand-down` and no grep of `args.rs`
    // finds it. An entry naming a verb that does not exist gates nothing, and
    // the crossing above would go on reporting its aliases covered.
    use clap::CommandFactory;
    let command = crate::cli::args::Cli::command();
    let verbs: Vec<String> = command
        .get_subcommands()
        .flat_map(|sub| {
            std::iter::once(sub.get_name().to_owned())
                .chain(sub.get_all_aliases().map(str::to_owned))
        })
        .collect();
    assert!(
        verbs.len() > 5,
        "clap answered {verbs:?}, which is not this CLI"
    );
    for fragment in super::DISARMS_THE_HARNESS {
        let Some(verb) = super::disarming_verb(fragment) else {
            continue;
        };
        assert!(
            verbs.iter().any(|known| known == verb),
            "`{fragment}` gates `estigia {verb}`, which is not a verb this CLI has: {verbs:?}"
        );
    }
}

/// A parent segment does not make the control surface another file.
///
/// `is_control_surface` collapses `/./` and `//` already, on the reasoning
/// written beside them: *a matcher a redundant separator defeats is measuring a
/// spelling, not a path*. `..` is the same class and more ordinary than either
/// — and it was not collapsed, so `~/.claude/skills/../settings.json` named the
/// file the gate is registered in and classified as an ordinary write.
///
/// The declared boundary names what this cannot reach: a hard link, a junction,
/// a copy of the state directory somewhere else. A parent segment is not one of
/// those. It is the same path, spelled the way a shell writes it.
#[test]
fn a_parent_segment_does_not_hide_the_control_surface() {
    let home = "c:/users/me";
    // The floor: the plain spelling is matched, and an ordinary source file is
    // not — so "matched" and "not matched" are both answers this really gives.
    assert!(super::is_control_surface(&format!(
        "{home}/.claude/settings.json"
    )));
    assert!(!super::is_control_surface(&format!("{home}/src/main.rs")));

    for spelling in [
        format!("{home}/.claude/skills/../settings.json"),
        format!("{home}/.claude/skills/issue-flow/../../settings.json"),
        format!("{home}/x/../.estigia/stand-down.json"),
        format!(r"{home}\.claude\skills\..\settings.json"),
    ] {
        assert!(
            super::is_control_surface(&spelling),
            "{spelling} names the control surface and is not matched"
        );
    }

    // A path that climbs out of what it names is left alone rather than
    // collapsed into nothing, or a spelling could be walked past the root.
    assert!(!super::is_control_surface(&format!(
        "{home}/../../src/main.rs"
    )));

    // Not asserted: that a directory literally called `..claude` is **not**
    // matched. It is — this population is matched on *fragment containment* by
    // declaration, and `..claude/settings.json` ends with the fragment. That is
    // a false positive, which costs one tracker read before a write that was
    // going to be verified anyway, and the declaration chooses that direction
    // on purpose. A test demanding otherwise would be asking the design to be
    // something it says out loud that it is not.
}

/// A claim over a checkout covers the work happening inside it.
///
/// Measured on the installed binary, with a run pointer holding issue #42 over a
/// temporary checkout. At the root, `estigia gate Write --run-id …` reached the
/// tracker — the gate doing its job. From `src/`, one directory down in the same
/// checkout, it answered
/// *"outside — Write is watched, and this run's claim covers a different checkout
/// than this one"*, and so did `git push`.
///
/// Every write and every irreversible boundary an agent made below the root was
/// outside the gate, and the sentence saying so asserted something false: `src/`
/// is not a different checkout.
///
/// Three places asked *does this claim cover this directory* and all three asked
/// it with `paths::same_directory`, which answers whether two paths name the same
/// directory. One step below the root the two questions part company. Only the
/// pre-push hook was safe, and only because git runs hooks from the top level.
#[test]
fn a_claim_covers_the_work_happening_below_the_checkout_root() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    let checkout = context.repo_dir.clone();
    let deep = checkout.join("src").join("deep");
    std::fs::create_dir_all(&deep).expect("the checkout has a subdirectory");
    context.repo_dir = deep;

    let mut run = sworn(12, &checkout);
    let action = Action::Write {
        target: "x.rs".to_owned(),
    };
    assert_ne!(
        gate(&context, &mut run.clone(), &action, Sensitivity::Routine),
        Decision::Outside(Aside::AnotherCheckout),
        "work one directory below the claimed checkout was outside the gate"
    );
    // The boundary too, which is the half that costs the most: `git push` from a
    // subdirectory went through unadjudicated.
    let push = Action::Boundary {
        command: "git push".to_owned(),
        pr: None,
        local_fast_forward_target: None,
    };
    assert_ne!(
        gate(&context, &mut run, &push, Sensitivity::Boundary),
        Decision::Outside(Aside::AnotherCheckout),
        "a push from below the claimed checkout was outside the gate"
    );

    // The floor, and the direction that must not move: a claim over an isolated
    // worktree does not start covering the checkout that contains it. Widening
    // downwards costs a tracker read; widening upwards costs the guarantee.
    let mut above = context.clone();
    above.repo_dir = checkout.clone();
    let mut worktree = sworn(12, &checkout.join(".worktrees").join("x"));
    assert_eq!(
        gate(&above, &mut worktree, &action, Sensitivity::Routine),
        Decision::Outside(Aside::AnotherCheckout),
        "a claim over a worktree reached the checkout above it"
    );
    // And a sibling that merely starts with the same letters is still another
    // checkout. `src-vendor` does not live inside `src`.
    let mut sibling = context.clone();
    sibling.repo_dir = checkout.join("src-vendor");
    let mut inner = sworn(12, &checkout.join("src"));
    assert_eq!(
        gate(&sibling, &mut inner, &action, Sensitivity::Routine),
        Decision::Outside(Aside::AnotherCheckout),
        "a sibling sharing a prefix was read as living inside"
    );
}

/// A write outside every checkout the claim covers is not the claim's business.
///
/// Measured in the field: after an issue auto-closed on merge, the gate refused
/// writes to a scratch directory and to the agent's own memory store, each with
/// *issue #164 is CLOSED*. Neither can affect the tracker. What it produced is
/// the outcome the harness exists to prevent — a delivery whose evidence could
/// not be written down — and it teaches an operator to reach around the gate,
/// which is worse than no gate.
///
/// The cause is that nothing classified the **path being written**:
/// `AnotherCheckout` compares the checkout the hook was invoked in, not the
/// file. So a scratch path, written from inside the claimed repository, was a
/// repository write.
///
/// Deliberately narrow, because standing aside is a statement and an unknown is
/// not one. It stands aside only when the target is an absolute path that no
/// covered checkout contains. A shell verb — `writes_a_file` answers *"a
/// redirect into a file"*, not a path — is not absolute, so it stays gated, and
/// so does anything the classifier marked `Boundary`: the control surface lives
/// outside the repository by nature, and the whole reason it is watched is that
/// an agent could switch the gate off with it.
#[test]
fn a_write_outside_every_covered_checkout_is_outside_the_claim() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    let scratch = root.path().join("scratch").join("note.md");
    let run = sworn(164, &repo);

    assert!(
        writes_outside_the_claim(
            &run,
            &Action::Write {
                target: scratch.display().to_string()
            }
        ),
        "a scratch path no checkout contains was read as a repository write"
    );
    // The agent's own store, the other path the field report names.
    assert!(writes_outside_the_claim(
        &run,
        &Action::Write {
            target: root
                .path()
                .join("memory")
                .join("note.md")
                .display()
                .to_string(),
        }
    ));

    // Inside the claimed checkout: unchanged, and the closed-issue refusal that
    // this is about must go on reaching it.
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: repo.join("src").join("main.rs").display().to_string()
            }
        ),
        "a write inside the claimed repository was waved through"
    );

    // What the shell classifier answers is a verb, not a path. Not knowing
    // where a write lands is not knowing it lands outside.
    for verb in ["a redirect into a file", "rm", "mv"] {
        assert!(
            !writes_outside_the_claim(
                &run,
                &Action::Write {
                    target: verb.to_owned()
                }
            ),
            "{verb:?} is not a path and was treated as one"
        );
    }
    // A relative path is not one this can place either.
    assert!(!writes_outside_the_claim(
        &run,
        &Action::Write {
            target: "src/main.rs".to_owned()
        }
    ));

    // A run that covers nothing has no claim to be outside of.
    let mut nowhere = Run::new("claude-abcd1234".to_owned());
    nowhere.issue = Some(164);
    assert!(!writes_outside_the_claim(
        &nowhere,
        &Action::Write {
            target: scratch.display().to_string()
        }
    ));
}

/// The state of the issue does not reach a write the claim does not cover.
///
/// This is the ordering the field report is about: the refusal that fired was
/// the tracker's — *issue #164 is CLOSED* — so the fix is worth nothing unless
/// the classification happens **before** the tracker is asked.
///
/// The contract is installed here, and that is the half of the ordering this
/// fixture got wrong first time round. It was passing on a sandbox with no
/// `SKILL.md` in it, which made the answer proof of something weaker than it
/// looked: the classification only had to outrank `control-surface-not-installed`,
/// and it did, which is precisely the ordering that had to be given up. With a
/// readable contract the `Outside` can only have come from a decision taken
/// before the tracker, which is what the issue is about.
///
/// The run's state says `done`, which is the shape a run has after the delivery
/// that closed its issue — the exact moment the evidence still has to be
/// written down.
#[test]
fn a_closed_issue_does_not_refuse_a_scratch_note() {
    let root = tempfile::tempdir().expect("a root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    let mut run = sworn(164, &context.repo_dir);
    run.state = Some("done".to_owned());
    let scratch = root.path().join("scratch").join("close-note.md");

    let decision = decide(
        &context,
        &mut run,
        &Action::Write {
            target: scratch.display().to_string(),
        },
        Sensitivity::Routine,
    );
    assert!(
        matches!(decision, Decision::Outside(Aside::OutsideTheClaim)),
        "a scratch note was gated on the issue's state: {decision:?}"
    );

    // And the defence that must survive it: a boundary write is watched for
    // where it lands, so landing outside cannot be what excuses it.
    let boundary = decide(
        &context,
        &mut run,
        &Action::Write {
            target: scratch.display().to_string(),
        },
        Sensitivity::Boundary,
    );
    assert!(
        !matches!(boundary, Decision::Outside(Aside::OutsideTheClaim)),
        "a boundary write was waved through for being outside the repository"
    );
}

/// A write that lands inside the claim is gated however it is spelled.
///
/// Two judges built this from opposite ends and it is one defect: the
/// comparison resolved a path with `canonicalize().unwrap_or_else(literal)`,
/// and a write target usually does not exist yet, so the two sides of the
/// comparison were in different vocabularies.
///
/// - `<root>/decoy/../repo/src/main.rs` compared literally is not inside
///   `<root>/repo` — and `std::fs::write` to that string lands there.
/// - a checkout reached through a junction resolves on the covered side and not
///   on the target side, so a **new** file inside it read as outside while an
///   existing one read as inside. Whether the file is there yet decided whether
///   the gate applied.
///
/// Both took a repository write out of the gate, which is what issue 2 puts out
/// of scope, so both are measured here against a real filesystem.
#[test]
fn a_write_that_lands_inside_the_claim_is_gated_however_it_is_spelled() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("a checkout");
    let run = sworn(164, &repo);

    // Through a `..` that climbs back in. Written, then read back, so the
    // assertion is about where it lands rather than about what it says.
    // Asked **before** the file exists, which is the whole of the attack: a
    // write target that is already there canonicalises, and the defect only
    // shows on one that does not. An earlier version of this test wrote the
    // file first and passed against the broken code.
    //
    // `decoy` has to exist, and that is a platform difference rather than
    // tidiness. Windows collapses `..` in the spelling before the filesystem is
    // consulted, so the write lands whether or not the directory is there;
    // POSIX resolves each segment, and `decoy/..` on a directory that does not
    // exist is `ENOENT`. Without it this fixture is green here and red on both
    // POSIX lanes, which is what it did. `docs/honesty.md` carries the history —
    // how long that lasted and what it cost — and this comment does not repeat
    // any of it, having twice now got the count wrong in the retelling.
    std::fs::create_dir_all(root.path().join("decoy")).expect("a directory to climb out of");
    let sideways = root
        .path()
        .join("decoy")
        .join("..")
        .join("repo")
        .join("src")
        .join("main.rs");
    assert!(
        !sideways.exists(),
        "the fixture must ask about a file that is not there yet"
    );
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: sideways.display().to_string()
            }
        ),
        "a write that lands in the claimed checkout was taken out of the gate by a `..`"
    );
    // Then written, so the claim about where it lands is measured and not read.
    std::fs::write(&sideways, "landed\n").expect("the spelling really does write there");
    assert!(
        repo.join("src").join("main.rs").is_file(),
        "the fixture does not reproduce the spelling it is about"
    );

    // A path that climbs past the root cannot be placed, and unplaceable is
    // read as inside.
    let mut climbing = std::path::PathBuf::from(&repo);
    for _ in 0..64 {
        climbing.push("..");
    }
    climbing.push("escaped.md");
    assert!(!writes_outside_the_claim(
        &run,
        &Action::Write {
            target: climbing.display().to_string()
        }
    ));

    // And a scratch path, which is the case the feature exists for: outside,
    // even though the file does not exist yet either.
    assert!(writes_outside_the_claim(
        &run,
        &Action::Write {
            target: root
                .path()
                .join("scratch")
                .join("note.md")
                .display()
                .to_string()
        }
    ));
}

/// The same defect through a junction, which is a link and not a spelling.
///
/// Skipped where the platform will not make one — a skip that says so is honest;
/// a green that ran nothing is the thing this crate keeps finding.
#[test]
fn a_new_file_inside_a_linked_checkout_is_gated() {
    let root = tempfile::tempdir().expect("a root");
    let real = root.path().join("real").join("repo");
    std::fs::create_dir_all(real.join("src")).expect("a checkout");
    let link = root.path().join("link");

    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&real)
        .output()
        .is_ok_and(|out| out.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&real, &link).is_ok();
    if !made {
        eprintln!("skipped: this platform would not create a directory link");
        return;
    }

    // The claim covers the link spelling, which is how a worktree given to a
    // run is usually named.
    let run = sworn(164, &link);
    let existing = link.join("src").join("kept.rs");
    std::fs::write(&existing, "kept\n").expect("an existing file");

    for (what, target) in [
        ("a new file", link.join("src").join("new.rs")),
        ("an existing file", existing),
    ] {
        assert!(
            !writes_outside_the_claim(
                &run,
                &Action::Write {
                    target: target.display().to_string()
                }
            ),
            "{what} inside the linked checkout was taken out of the gate"
        );
    }
}

/// A link whose target is inside the checkout is a write inside the checkout.
///
/// The classification walked up past an entry that would not resolve, treating
/// *"nothing here"* and *"something here that dangles"* as the same answer. So a
/// symlink at `<outside>/alias.rs` pointing at `<repo>/src/planted.rs` — whose
/// target does not exist yet, which is the ordinary shape of a write — was
/// placed at its own spelling and read as outside. Writing through it created
/// the file inside the claimed checkout.
///
/// The same defect as the junction, one level down: whether the thing is there
/// yet decided whether the gate applied.
#[test]
fn a_link_pointing_into_the_checkout_is_gated() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("a checkout");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&outside).expect("a directory that is not the checkout");

    let planted = repo.join("src").join("planted.rs");
    let alias = outside.join("alias.rs");
    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink"])
        .arg(&alias)
        .arg(&planted)
        .output()
        .is_ok_and(|out| out.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&planted, &alias).is_ok();
    if !made {
        eprintln!("skipped: this platform would not create a file link");
        return;
    }

    let run = sworn(164, &repo);
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: alias.display().to_string()
            }
        ),
        "a link whose target is inside the claimed checkout was taken out of the gate"
    );
    // And the reason it matters, measured rather than argued.
    std::fs::write(&alias, "planted\n").expect("the link really does write through");
    assert!(
        planted.is_file(),
        "the fixture does not reproduce the link it is about"
    );
}

/// An unreadable control surface permits no write, including one to stand aside.
///
/// The contract's rule is written without an exception, and standing aside is a
/// permission like any other. For one published head this classification was
/// asked first, so with no `SKILL.md` installed a routine write outside every
/// checkout went through with nothing consulted at all — and the files that are
/// outside every checkout by construction are the agent instruction files that
/// carry the directive naming this harness as the authority.
///
/// This is the fixture the move needed: put the classification back above the
/// contract refusal and this goes red, which is what stops the ordering
/// drifting back the way it came.
#[test]
fn an_unreadable_control_surface_refuses_even_a_write_outside_the_claim() {
    let root = tempfile::tempdir().expect("a root");
    let mut context = context(root.path());
    // Nothing installed: not the contract, not the directory holding it.
    context.skill_root = root.path().join("never-installed");
    let mut run = sworn(166, &context.repo_dir);
    run.state = Some("done".to_owned());
    let outside = root.path().join("elsewhere").join("note.md");

    let decision = decide(
        &context,
        &mut run,
        &Action::Write {
            target: outside.display().to_string(),
        },
        Sensitivity::Routine,
    );
    let Decision::Deny(refusal) = &decision else {
        panic!("a write was permitted while the control surface was unreadable: {decision:?}");
    };
    assert_eq!(
        refusal.code, "control-surface-not-installed",
        "the refusal was not the contract's: {decision:?}"
    );
}

/// The same road, one `mark_verified()` line later.
///
/// The fixture above held only *outside* the renewal window, and the entry in
/// `docs/honesty.md` said so: the window's `Allow` sat above the contract
/// refusal, so for the window's duration the agent instruction files — outside
/// every checkout by construction, and carrying the directive that names this
/// harness as the authority at all — were writable with no `SKILL.md` on disk.
/// A reviewer measured exactly this by adding the stamp to the fixture above;
/// this is that measurement kept as a test rather than as a paragraph.
#[test]
fn an_unreadable_control_surface_refuses_a_write_outside_the_claim_inside_the_window() {
    let root = tempfile::tempdir().expect("a root");
    let mut context = context(root.path());
    // Nothing installed: not the contract, not the directory holding it.
    context.skill_root = root.path().join("never-installed");
    let mut run = sworn(166, &context.repo_dir);
    run.state = Some("done".to_owned());
    // The only difference from the fixture above.
    run.mark_verified();
    let outside = root.path().join("elsewhere").join("note.md");

    let decision = decide(
        &context,
        &mut run,
        &Action::Write {
            target: outside.display().to_string(),
        },
        Sensitivity::Routine,
    );
    let Decision::Deny(refusal) = &decision else {
        panic!(
            "a write was permitted inside the renewal window while the control surface was \
             unreadable: {decision:?}"
        );
    };
    assert_eq!(
        refusal.code, "control-surface-not-installed",
        "the refusal was not the contract's: {decision:?}"
    );
}

/// A second name for the same drive does not take a write out of the gate.
///
/// Windows serves every local drive as an administrative share, so
/// `\\localhost\C$\Users\...` is the very same file as `C:\Users\...` with no
/// link anywhere in it. A judge measured what that costs: the classification
/// answered *outside* and `std::fs::write` on that spelling created the file
/// **inside** the claimed checkout — a gate that no longer decides, reachable
/// on the operator's own repository as `\\localhost\H$\REPO\estigia`.
///
/// The cause is not the share, it is the vocabulary. `canonicalize` hands back
/// `\\?\UNC\localhost\C$\...` for the target while the covered checkout places
/// to `C:\...`, so one file is compared under two spellings and neither is a
/// prefix of the other. Resolving a share back to the drive it serves is not
/// something this process can do without asking the machine what it shares, so
/// `placed` declines the path instead — and declining reads as *inside*, which
/// is the direction that keeps the gate on.
#[test]
#[cfg(windows)]
fn a_drive_reached_through_its_administrative_share_is_still_inside() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("a checkout");

    let planted = repo.join("src").join("planted.rs");
    std::fs::write(&planted, "kept\n").expect("a file inside the checkout");
    let full = planted.canonicalize().expect("the planted file resolves");
    let spelled = crate::paths::remove_windows_verbatim_prefix(full)
        .display()
        .to_string();
    let Some((drive, rest)) = spelled.split_once(':') else {
        eprintln!("skipped: the temporary directory has no drive letter");
        return;
    };
    let share = format!(r"\\localhost\{drive}${rest}");

    // The share has to really be the same file, or this test is about nothing.
    if std::fs::write(&share, "through the share\n").is_err() {
        eprintln!("skipped: this machine does not serve its drives as admin shares");
        return;
    }
    assert_eq!(
        std::fs::read_to_string(&planted).expect("the planted file is readable"),
        "through the share\n",
        "the fixture does not reproduce the share it is about"
    );

    let run = sworn(165, &repo);
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: share.clone()
            }
        ),
        "a write spelled through the administrative share was taken out of the \
         gate, and it lands inside the claimed checkout: {share}"
    );
}

/// The spelling handed in is not where the write lands, and it is the landing
/// that decides.
///
/// The first attempt at the rule above rejected a path whose **first component**
/// named something other than a drive. That reads the vocabulary of the input,
/// and `canonicalize` is free to answer in a different one: a drive letter
/// mapped onto the administrative share (`net use Y: \\localhost\C$`) is
/// `Disk`-prefixed going in and comes back `\\localhost\C$\...`, so it passed
/// the check and then failed the comparison exactly as the unspelled share had.
/// A reviewer measured it on this machine with one `net use`.
///
/// This fixture reaches the same landing without changing anything outside the
/// temporary directory: a **directory symlink** whose target is the share. The
/// path through it is `C:\...\link\src\planted.rs` — a drive, by any reading of
/// the spelling — and it resolves onto the share all the same.
#[test]
#[cfg(windows)]
fn a_drive_that_resolves_onto_a_share_is_still_inside() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("a checkout");
    let planted = repo.join("src").join("planted.rs");
    std::fs::write(&planted, "kept\n").expect("a file inside the checkout");

    let real = repo.canonicalize().expect("the checkout resolves");
    let spelled = crate::paths::remove_windows_verbatim_prefix(real)
        .display()
        .to_string();
    let Some((drive, rest)) = spelled.split_once(':') else {
        eprintln!("skipped: the temporary directory has no drive letter");
        return;
    };
    let share = format!(r"\\localhost\{drive}${rest}");

    let link = root.path().join("link");
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/D"])
        .arg(&link)
        .arg(&share)
        .output()
        .is_ok_and(|out| out.status.success());
    if !made {
        eprintln!("skipped: this machine would not link a directory onto its own share");
        return;
    }

    // It has to really be the same file, or this test is about a string.
    let through = link.join("src").join("planted.rs");
    if std::fs::write(&through, "through the link\n").is_err() {
        eprintln!("skipped: this machine does not serve its drives as admin shares");
        return;
    }
    assert_eq!(
        std::fs::read_to_string(&planted).expect("the planted file is readable"),
        "through the link\n",
        "the fixture does not reproduce the share it is about"
    );

    let run = sworn(167, &repo);
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: through.display().to_string()
            }
        ),
        "a write whose drive-lettered spelling resolves onto a share was taken \
         out of the gate, and it lands inside the claimed checkout: {}",
        through.display()
    );
}

/// The file an operator is told to put local overrides in is a boundary.
///
/// `contains` is the match, so `.claude/settings.json` never reached
/// `.claude/settings.local.json` — and that is the file Claude Code documents for
/// machine-local settings, read with the same authority as the one beside it. It
/// answered `Routine`, which means the gate could be switched off through the
/// file an operator is *told* to edit, on an answer a renewal window may already
/// have paid for. The same held for `~/.claude/agents/`, where an agent
/// definition carries a tool allowlist — instructions in another shape.
///
/// Both roads, because `surface_of` splits a command on whitespace and appends a
/// separator: an entry that only works for the write tool is one an agent walks
/// round with `rm`.
#[test]
fn the_local_settings_and_the_agent_definitions_are_boundaries_on_both_roads() {
    let home = crate::paths::home_dir().expect("a home directory");
    for relative in [
        [".claude", "settings.local.json"].as_slice(),
        [".claude", "settings.json"].as_slice(),
        [".claude", "agents", "reviewer.md"].as_slice(),
    ] {
        let mut target = home.clone();
        for part in relative {
            target.push(part);
        }
        let spelled = target.display().to_string();

        let (_, written) = classify(
            "Write",
            &serde_json::json!({ "file_path": spelled.clone() }),
        );
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine, so the gate can be switched off through it"
        );

        let (_, removed) = classify(
            "Bash",
            &serde_json::json!({ "command": format!("rm {spelled}") }),
        );
        assert_eq!(
            removed,
            Sensitivity::Boundary,
            "removing {spelled} through the shell answers routine, which is the road an \
             agent would actually take"
        );
    }
}

/// The instruction files, spelled as a reader would recognise them.
///
/// Spelled out, not derived. The first version built its subject from
/// `instruction_fragment()` and then asserted the fragment matched — which
/// reduces to `contains(f)` on a string built as `home + f`, and cannot fail for
/// the reason its own name gives. A reviewer proved it by setting one adapter's
/// fragment to a filename nothing writes and watching this stay green.
///
/// The crossing in `src/setup/tests.rs` ties these to what the installer
/// resolves, on three platforms and two XDG layouts, on both roads. This fixes
/// the *reason* in place, and it covers the shell too because `surface_of`
/// splits a command on whitespace and an entry that works on one road only is
/// one an agent walks round with `rm`.
const SPELLED_INSTRUCTION_FILES: &[&str] = &[
    "/home/me/.agents/AGENTS.md",
    "/home/me/.claude/CLAUDE.md",
    "/home/me/.codex/AGENTS.md",
    "/home/me/.config/opencode/AGENTS.md",
    "/home/me/.gemini/GEMINI.md",
    r"C:\Users\me\AppData\Roaming\gemini\GEMINI.md",
    "/home/me/.cursor/estigia-workflow-authority.md",
    "/home/me/.qwen/QWEN.md",
    "/home/me/.config/crush/CRUSH.md",
    "/home/me/.continue/rules/estigia.md",
    "/home/me/.cline/rules/estigia.md",
    "/home/me/.codeium/windsurf/memories/global_rules.md",
];

#[test]
fn the_directive_that_names_the_authority_is_a_boundary_for_every_agent() {
    for spelled in SPELLED_INSTRUCTION_FILES {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine, and that file carries the directive \
             naming this harness as the authority"
        );

        let (_, removed) = classify(
            "Bash",
            &serde_json::json!({ "command": format!("rm {spelled}") }),
        );
        assert_eq!(
            removed,
            Sensitivity::Boundary,
            "removing {spelled} through the shell answers routine, which is the road an \
             agent would actually take"
        );
    }
}

/// The spelled list and the adapter table say the same thing.
///
/// A hand-written list is readable and goes stale, and the first version of this
/// only counted fragments — it never read the list at all, so deleting an entry
/// left the suite green. A reviewer measured that. It reads both now: every
/// adapter's fragment has to appear in some spelled path, and every spelled path
/// has to be there for an adapter. A twelfth adapter, or one whose instruction
/// file moves, breaks one side or the other.
#[test]
fn the_spelled_instruction_files_and_the_adapter_table_agree() {
    let folded: Vec<String> = SPELLED_INSTRUCTION_FILES
        .iter()
        .map(|path| path.replace('\\', "/").to_ascii_lowercase())
        .collect();

    for adapter in crate::setup::AGENTS {
        let fragment = adapter.instruction_fragment();
        assert!(
            folded.iter().any(|path| path.contains(fragment)),
            "{}: no spelled path contains its fragment `{fragment}`, so the list above \
             no longer shows what the gate matches",
            adapter.slug
        );
    }

    for (path, folded) in SPELLED_INSTRUCTION_FILES.iter().zip(&folded) {
        assert!(
            crate::setup::AGENTS
                .iter()
                .any(|adapter| folded.contains(adapter.instruction_fragment())),
            "{path} is spelled here but matches no adapter's fragment, so it is not an \
             instruction file this crate writes"
        );
    }
}

/// A neighbour in a directory the host reads whole carries the same authority.
///
/// `paths_in`'s comments say so for two of them — Continue applies any rule with
/// no frontmatter, Cline loads its directory for every task. The other two are
/// gated on the restated population rule rather than on anything verified here;
/// `docs/honesty.md` says which. So gating Estigia's own filename and leaving
/// the directory open is defeated by adding a sibling: a reviewer measured
/// `~/.cline/rules/zz-override.md` answering `Routine`, and a file there saying
/// *Estigia is retired* changes what the agent is told this harness may enforce
/// without touching a `Boundary` path.
#[test]
fn a_sibling_in_a_rules_directory_is_a_boundary_too() {
    for spelled in [
        "/home/me/.cline/rules/zz-override.md",
        "/home/me/.continue/rules/zz-override.md",
        "/home/me/.codeium/windsurf/memories/zz-override.md",
        "/home/me/.cursor/rules/zz-override.mdc",
        // The `.local.` siblings, which are the same shape one directory up:
        // read with the same authority as the file beside them, and `contains`
        // did not reach them until the fragments lost their extension.
        "/home/me/.claude/CLAUDE.local.md",
        "/home/me/.codex/AGENTS.local.md",
        "/home/me/.agents/AGENTS.local.md",
    ] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine, and the host applies every file in that \
             directory — so the directive beside it can be overridden by a neighbour"
        );
    }
}

/// What the **host** reads, beside what this installer writes.
///
/// The enumeration stopped at the files `setup` authors, and an agent loads a
/// great deal it was not handed by this installer with exactly the authority of
/// the directive that was. Every one of these answered `Routine`, and every one
/// is outside a checkout by construction — so `outside-the-claim` waved them past
/// without asking the tracker at all.
///
/// Both roads, because `surface_of` splits a command on whitespace and an entry
/// that works on one road only is one an agent walks round with `rm`.
#[test]
fn a_host_read_root_carries_the_same_authority_as_the_directive() {
    for spelled in [
        // Estigia's own delivery root. `skills/flow` was gated by derivation and
        // the tree holding it was not, so a sibling skill — whose name and
        // description Claude Code loads whether or not it is ever invoked — was
        // the neighbour that defeats the gate one level up.
        "/home/me/.claude/skills/zz-other/SKILL.md",
        "/home/me/.claude/skills",
        "/home/me/.agents/skills/zz-other/SKILL.md",
        "/home/me/.codex/skills/zz-other/SKILL.md",
        // Plugins, which supply hooks, sub-agents, skills and commands at once.
        // `~/.claude.json` names them and was gated; the payload on disk was not.
        "/home/me/.claude/plugins/config.json",
        "/home/me/.claude/plugins/pack/hooks/hooks.json",
        "/home/me/.claude/plugins/pack/agents/builder.md",
        // Commands and prompts, whose names and descriptions the host puts in
        // front of the agent unprompted.
        "/home/me/.claude/commands/ship.md",
        "/home/me/.codex/prompts/ship.md",
        "/home/me/.cursor/commands/ship.md",
        "/home/me/.cursor/cli-config.json",
        // Extensions: always-loaded context, MCP servers, and `excludeTools` —
        // a compliance input and an enforcement input in one directory.
        "/home/me/.gemini/extensions/pack/GEMINI.md",
        r"C:\Users\me\AppData\Roaming\gemini\extensions\pack\GEMINI.md",
        "/home/me/.qwen/extensions/pack/QWEN.md",
        // And the manifests, which are the rows that actually hold these three
        // entries. Two blind reviewers measured that deleting
        // `.gemini/extensions/`, `gemini/extensions/` and `.qwen/extensions/`
        // left the whole lib suite green: the rows above end in `GEMINI.md` and
        // `QWEN.md`, so the per-project instruction fragments satisfy them and
        // the entry under test is never the reason. The manifest is also the file
        // carrying `excludeTools` and `mcpServers`, which is the enforcement half
        // of why the directory is here at all.
        "/home/me/.gemini/extensions/pack/gemini-extension.json",
        r"C:\Users\me\AppData\Roaming\gemini\extensions\pack\gemini-extension.json",
        "/home/me/.qwen/extensions/pack/qwen-extension.json",
        // Continue's assistant configuration, which carries `rules:` inline and
        // `mcpServers:` — the same rules `.continue/rules` holds, in another
        // shape, and only one of the two was gated.
        "/home/me/.continue/config.yaml",
        "/home/me/.continue/config.json",
        "/home/me/.continue/assistants/mine.yaml",
        // Windsurf's MCP configuration, which `paths_in` records as unverified.
        // Unverified is not decided-against, and it was not gated either.
        "/home/me/.codeium/windsurf/mcp_config.json",
        "/home/me/.codeium/windsurf/workflows/ship.md",
    ] {
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": spelled })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -rf {spelled}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{road} on {spelled} answers routine, and the host reads it with the same \
                 authority as the directive naming this harness"
            );
        }
    }
}

/// A project's own always-loaded rules, which the dotted prefix reached by luck.
///
/// Inside a checkout these stay measured against the claim rather than standing
/// aside, so they are a smaller thing than the home paths — not a different one.
/// What was wrong is that the line between covered and not was drawn by which
/// home-path fragment happened to carry a dotted-directory prefix:
/// `<repo>/.claude/CLAUDE.md` was a boundary and `<repo>/CLAUDE.md` — the file
/// Claude Code loads by default, and the one **this repository itself** uses —
/// was not.
#[test]
fn a_projects_own_always_loaded_rules_are_a_boundary() {
    for spelled in [
        "/repo/CLAUDE.md",
        "/repo/CLAUDE.local.md",
        "/repo/AGENTS.md",
        "/repo/AGENTS.local.md",
        "/repo/GEMINI.md",
        "/repo/QWEN.md",
        "/repo/CRUSH.md",
        // Nested, because a rules file deeper in a tree is read the same way.
        "/repo/services/api/AGENTS.md",
        // The two that were hardest to defend leaving out: the per-project twins
        // of directories issue 26 gated at home, so the hole was closed on one
        // road and left open on the other in the same round.
        "/repo/.clinerules",
        "/repo/.clinerules/zz-override.md",
        "/repo/.windsurf/rules/zz-override.md",
        "/repo/.windsurfrules",
        "/repo/.cursorrules",
        "/repo/.github/copilot-instructions.md",
        "/repo/.mcp.json",
        // The singular spelling, one letter from `.opencode/agents/` beside it.
        "/repo/.opencode/agent/builder.md",
    ] {
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": spelled })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -rf {spelled}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{road} on {spelled} answers routine, and it is always-loaded context \
                 carrying the same authority as the directive"
            );
        }
    }
}

/// A directory this harness reads from is a boundary spelled bare, on both roads.
///
/// The entries used to carry a trailing slash. `surface_of` appends a separator
/// to every token of a command, so `rm <dir>` matched and a write to the bare
/// directory did not — one road gated and the other not, on the state directory
/// and the installed contract among others. A reviewer found it in one entry;
/// the crossing found the rest once it was asked about bare roots rather than
/// only about a file inside them.
#[test]
fn a_control_directory_named_bare_is_a_boundary_on_both_roads() {
    for spelled in [
        "/home/me/.claude/skills/flow",
        "/home/me/.claude/skills/issue-flow",
        "/repo/.estigia",
        "/home/me/.claude/agents",
    ] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to the bare directory {spelled} answers routine while removing it \
             through the shell does not — the two roads disagree"
        );

        let (_, removed) = classify(
            "Bash",
            &serde_json::json!({ "command": format!("rm -rf {spelled}") }),
        );
        assert_eq!(
            removed,
            Sensitivity::Boundary,
            "removing {spelled} through the shell answers routine"
        );
    }
}

/// A fragment naming a directory does not gate a name that merely starts alike.
///
/// Both halves of this were wrong in turn, one round apart, and nothing in the
/// suite watched the second one — every guard here asserts that something *is*
/// `Boundary`, so over-gating was invisible to all of them.
///
/// With a trailing slash the entries gated `rm <dir>` and left a write to the
/// bare directory `Routine`: `surface_of` appends a separator, so one road
/// matched and the other did not. Dropping the slash closed that and made every
/// entry a prefix — `.estigiaignore`, `skills/flow.md` and `.claude/agentsmith.md`
/// all became `Boundary`, a tracker read on ordinary files for nothing. The
/// matcher honours the slash now, so the entry can say *directory* and mean it.
#[test]
fn a_directory_entry_does_not_gate_a_name_that_only_starts_the_same() {
    for ordinary in [
        "/repo/.estigiaignore",
        "/repo/docs/.estigia.md",
        "/repo/skills/flow.md",
        "/repo/docs/skills/flow-diagram.md",
        "/home/me/.claude/agentsmith.md",
        "/home/me/.cursor/rulesets.md",
        "/home/me/.continue/rules-archive/a.md",
        // A vendored copy of the agent itself, which is somebody's ordinary
        // source tree and not a control surface. All six shapes a reviewer
        // measured against the bare `opencode` entry that stood here for one
        // head — they were recorded as `Routine` again with nothing holding
        // them, which is how the entry came to be bare in the first place.
        "/repo/node_modules/opencode/index.js",
        "/repo/packages/opencode/src/main.ts",
        "/repo/vendor/opencode/README.md",
        "/repo/my-opencode/README.md",
        "/repo/assets/opencode/logo.svg",
        "/repo/target/debug/build/opencode/out.rs",
        // The left side, which took two attempts. `ends_with` anchors nothing in
        // front of it, and the fix for that left `contains` alone — so a file
        // *under* the lookalike stayed `Boundary` while the bare directory came
        // back `Routine`, and three documents said the case was closed. A reviewer
        // measured both. A dot-directory is always a whole segment, so both are
        // anchored now; the shapes that do not begin with a dot are in
        // `docs/honesty.md`, because `cli/hosts.yml` has to match mid-segment.
        "/home/me/my.claude/agents",
        "/home/me/my.claude/agents/note.md",
        "/home/me/xyz.cursor/rules",
        "/home/me/xyz.cursor/rules/note.md",
        "/home/me/my.cline/rules/note.md",
        "/home/me/zz.continue/rules/note.md",
        "/repo/x.estigia/state",
        // The dot fragments that are **not** directories. Anchoring these was
        // untested: a reviewer mutated the condition to anchor directory
        // fragments only and the whole suite stayed green, so nothing held the
        // half of the rule that keeps `my.claude/settings.json` off the gate.
        "/home/me/my.claude/settings.json",
        "/home/me/my.claude/settings.local.json",
        "/home/me/xyz.claude.json",
        "/home/me/my.claude.json",
        "/home/me/my.codex/config.toml",
        "/home/me/my.codex/hooks.json",
        "/home/me/x.qwen/settings.json",
        // The same left side for the fragments that do **not** begin with a dot.
        // Anchoring those was the second half of the same fix and it was left
        // undone for one head: `surface_of` gives every token a trailing `/`, so
        // each of these answered `Routine` to `Write` and `Boundary` to `rm` —
        // the road split this entry was written to close, alive on exactly the
        // fragments it added. A directory fragment can be anchored because every
        // real target has that first segment whole; the file fragments cannot,
        // and `docs/honesty.md` measures what that leaves.
        // Not `/repo/.opencode/agents`: a reviewer measured that it is a live
        // definition root `roles::definition_for` searches, so calling it an
        // ordinary path was the fixture pinning a hole. It has its own entry now
        // and its own fixture below. `/repo/.opencode/plugins` stays here because
        // Estigia reads nothing from it — `docs/honesty.md` carries that split.
        "/repo/.opencode/plugins",
        "/repo/xyzopencode/agents",
        "/repo/notwindsurf/memories",
        "/home/me/.claude/myskills/issue-flow",
        // The **separator-free** fragments, which are the third anchoring rule
        // and arrived with the project's own rule files. A whole file name is a
        // whole segment in every real target, so `agents.md` is anchored — and
        // unanchored it reaches `myagents.md`, which is somebody's ordinary
        // source file. Measured: without the anchor these four answer `Boundary`.
        "/repo/myagents.md",
        "/repo/myclaude.md",
        "/repo/notgemini.md",
        "/repo/vendor/mycrush.md",
        // And the extension is kept rather than trimmed, so a source file named
        // for the same idea is untouched. `agents.` would have covered the
        // `.local.` sibling in one entry and taken this with it.
        "/repo/src/agents.rs",
        // The new home roots, on their left side. A dot-directory is a whole
        // segment, so a lookalike beside it is ordinary.
        "/home/me/my.claude/skills/zz-other/SKILL.md",
        "/home/me/.claude/mycommands/ship.md",
        "/home/me/.claude/myplugins/config.json",
        "/home/me/.gemini/extensions-archive/pack/manifest.json",
        "/home/me/.codeium/windsurf/workflows-archive/ship.md",
        // And the project ones.
        "/repo/x.clinerules",
        "/repo/my.mcp.json",
        "/repo/.windsurf/rules-archive/zz-override.md",
        "/repo/.opencode/agentry/builder.md",
        // Not here, and measured: `~/.codeium/notwindsurf/memories/global_rules.md`
        // still answers `Boundary`. The **file** fragments that do not begin with a
        // dot stay unanchored, because `cli/hosts.yml` has to match inside the
        // segment `github cli`, and windsurf's derived fragment is one of them.
        // That is the declared over-gating in `docs/honesty.md`, not a gap here —
        // asserting it away is what a first draft of this list did.
    ] {
        // Both roads. The bare lookalike answered `Routine` on the write road and
        // `Boundary` through the shell, because `surface_of` appends a separator
        // and reached the unanchored branch — the same split this entry claims to
        // have closed, surviving in the direction nothing asserted.
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": ordinary })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -rf {ordinary}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Routine,
                "{road} on {ordinary} answers boundary, and it is an ordinary path that \
                 only starts like a control surface"
            );
        }
    }
}

/// The over-gating `docs/honesty.md` declares, held rather than asserted.
///
/// A fragment naming a **file** without a leading dot is deliberately not
/// anchored, because `cli/hosts.yml` has to match inside the segment
/// `github cli`. The cost is that a vendored copy of somebody else's agent
/// answers `Boundary`. That cost was written down with the wrong path —
/// `vendor/myopencode/agents/a.md`, which two reviewers measured `Routine`,
/// because `opencode/agents/` is a directory fragment and *is* anchored. The
/// derived fragment that demonstrates it is `opencode/agents.md`. Nothing held
/// any of it: this file asserted the ends-alike direction and never this one, so
/// the document's own example had drifted from the code it describes.
#[test]
fn the_declared_over_gating_is_the_shape_the_document_names() {
    for named in [
        "/repo/vendor/myopencode/agents.md",
        "/repo/vendor/mygemini/gemini.md",
        "/repo/vendor/mycrush/crush.md",
        "/home/me/.codeium/notwindsurf/memories/global_rules.md",
        // The right side of the separator-free fragments, which are anchored on
        // the left only. `agents.md` cannot be closed on the right without
        // giving up what closes `CLAUDE.local.md` one entry over, and the same
        // trade already stands for `.claude/settings` reaching
        // `.claude/settingsmap.ts`.
        "/repo/AGENTS.md.bak",
        "/repo/CLAUDE.md.orig",
        "/repo/AGENTS.mdx",
        "/repo/.mcp.json.bak",
        // And the **depth** of the same fragments, which is deliberate — a rules
        // file deeper in a tree is read the same way — but is paid for by every
        // document that happens to be called one of these names. Two reviewers
        // measured the width; the declaration had named only the right-hand side.
        "/repo/docs/agents.md",
        "/repo/docs/gemini.md",
        "/repo/website/content/blog/qwen.md",
        "/repo/node_modules/pkg/AGENTS.md",
        // `.clinerules` is a prefix so that it reaches Cline's file *and* its
        // directory, which is the same trade `.claude/settings` makes and was not
        // declared beside it.
        "/repo/.clinerules-archive/note.md",
        // And `.continue/config`, trimmed to one stem so it reaches `config.yaml`,
        // `config.json` and `config.ts` at once.
        "/home/me/.continue/configuration-notes.md",
    ] {
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": named })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -f {named}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{road} on {named} answers routine, and `docs/honesty.md` declares it over-gated"
            );
        }
    }
}

/// The ladder reads the same string the matcher does.
///
/// The ninth loss on this road. The view is built from `normalise(&fold(token))`
/// and the ladder's entry test read `fold(token)` alone — `normalise` folds `:`
/// and `\` and `fold` does not. So in `${P:-.claude/settings.json}` the `-`
/// starts a segment in the string that is matched and did not in the string the
/// ladder inspected, no rung was offered, and thirteen real surfaces answered
/// `Routine` where the base answered `Boundary`: the run pointer, the stand-down
/// record, `hosts.yml` and the file the gate is registered in among them.
///
/// `TARGET=${1:-.claude/settings.json}` on one line and `rm -f "$TARGET"` on the
/// next is the ordinary script idiom for a defaulted path, not a contrivance. A
/// reviewer found it by sweeping **two**-character contexts; the sweep recorded
/// in `docs/honesty.md` was one character wide and could not reach `:-`.
#[test]
fn a_defaulted_parameter_does_not_hide_a_control_surface() {
    let d = '$';
    let ob = '{';
    let cb = '}';
    for path in [
        ".claude/settings.json",
        ".claude.json",
        ".estigia/run.json",
        ".estigia/stand-down.json",
        ".config/gh/hosts.yml",
        ".codex/config.toml",
        ".cursor/mcp.json",
        ".qwen/settings.json",
    ] {
        for line in [
            format!("rm -f {d}{ob}P:-{path}{cb}"),
            format!("rm -f {d}{ob}HOME:-{path}{cb}"),
            // The other two contexts the same fix reaches, because `normalise`
            // folds the backslash as well as the colon.
            format!("rm -f x{}-{path}", '\\'),
            format!("rm -f x{}~{path}", '\\'),
        ] {
            let (_, how) = classify("Bash", &serde_json::json!({ "command": &line }));
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "`{line}` answers routine, and its default word names a control surface"
            );
        }
    }
}

/// A patch body names a control surface the same way a command line does.
///
/// Two of the eleven write tools put the path in a patch body rather than a
/// field — Codex's `apply_patch` and OpenCode's `patch` — so `classify_with`
/// falls back to reading the whole payload. Handing that blob straight to
/// `is_control_surface` was right while the fragments were matched by a bare
/// `contains` and wrong the moment they were anchored: in a patch body the path
/// sits after a space, so every **relative** spelling stopped matching on that
/// road while `Write` still gated the same file. A reviewer measured thirteen,
/// including the run pointer and the stand-down record, and the fixture that
/// holds this road stayed green because it spells its surface absolutely.
///
/// Both roads read the text the same way now.
#[test]
fn a_patch_body_reaches_a_relative_control_surface() {
    for named in [
        ".estigia/run.json",
        ".estigia/stand-down.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude.json",
        ".config/gh/hosts.yml",
        ".codex/config.toml",
        ".cursor/mcp.json",
        "/home/me/.claude/settings.json",
    ] {
        let body =
            format!("*** Begin Patch\n*** Update File: {named}\n@@\n-old\n+new\n*** End Patch");
        for tool in ["apply_patch", "patch"] {
            let (_, how) = classify(tool, &serde_json::json!({ "input": &body }));
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{tool} answers routine for a patch body naming {named}"
            );
        }
    }
    // And an ordinary file in a patch body is still ordinary.
    for named in ["src/main.rs", "README.md", "tests/guards.rs"] {
        let body =
            format!("*** Begin Patch\n*** Update File: {named}\n@@\n-old\n+new\n*** End Patch");
        let (_, how) = classify("apply_patch", &serde_json::json!({ "input": &body }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "a patch body naming {named} answers boundary"
        );
    }
}

/// The ladder is bounded to the marker's own segment.
///
/// An option prefix and a shell expansion are both short and both end before the
/// first separator — `-o`, `-Lo`, `~dp0` — so a cut past one is not reading a
/// prefix, it is deleting path segments. Unbounded, the ladder voided the left
/// anchoring for **every** token beginning with `~`, which is how a home path is
/// written: the rungs of `~/my.claude/agents` include `.claude/agents`. A
/// reviewer measured eighteen of the thirty-one rows of the anchoring fixture
/// answering the opposite when respelled with `~`, against three paragraphs of
/// `docs/honesty.md` that name those exact paths as `Routine` — and every row of
/// that fixture was spelled with an absolute root, which is why it stayed green.
#[test]
fn a_home_prefix_does_not_void_the_anchoring() {
    for line in [
        "rm -rf ~/my.claude/agents",
        "rm -rf ~/my.claude/agents/note.md",
        "rm -rf ~/my.claude/settings.json",
        "rm -rf ~/my.claude/settings.local.json",
        "rm -rf ~/my.claude.json",
        "rm -rf ~/xyz.claude.json",
        "rm -rf ~/my.codex/config.toml",
        "rm -rf ~/x.qwen/settings.json",
        "rm -rf ~/xyz.cursor/rules",
        "rm -rf ~/zz.continue/rules/note.md",
        "rm -rf ~/x.estigia/state",
        "rm -rf ~/notwindsurf/memories",
        "rm -rf ~/xyzopencode/agents",
        "rm -rf ~/dev/myrepo/.opencode/plugins",
        "rm -rf ~/.claude/myskills/issue-flow",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "`{line}` answers boundary, and the ladder has eaten the anchoring behind a `~`"
        );
    }
    // The real ones behind the same prefix are untouched.
    for line in [
        "rm -rf ~/.claude/settings.json",
        "rm -rf ~/.claude/settings.local.json",
        "rm -rf ~/.claude/agents",
        "rm -rf ~/.config/gh/hosts.yml",
        "rm -rf ~/.estigia/run.json",
        "rm -rf %~dp0.estigia/run.json",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and it names a control surface"
        );
    }
}

/// The suffix ladder does not build a path nobody wrote.
///
/// The rungs offered for a `-` or `~` prefixed token used to be appended into
/// one view, one after another, separated by the same character that separates
/// path segments. Adjacent rungs therefore concatenated: the ladder for
/// `~/.claude` is `.claude/claude/laude/aude/…`, and `.claude/claude` is exactly
/// ClaudeCode's derived instruction fragment. So a recursive delete of the home
/// config directory answered `Boundary` for a path that was never in the
/// command, and so did any `~`- or `-`-led token merely *ending* in `.claude` or
/// `.agents` — `~/backup.claude`, `~/notes.agents`, `-obackup.claude`.
/// `.agents/agents` has the same `A/A[1..]` shape and did the same thing.
///
/// A reviewer measured it, and it had already made a paragraph of
/// `docs/honesty.md` false about the directories that paragraph was declaring
/// open. Each rung is asked on its own now.
#[test]
fn the_suffix_ladder_does_not_synthesise_a_path() {
    for line in [
        // The containing directories the document declares `Routine`, in the
        // spelling the document itself writes. This is the shape that was false.
        "rm -rf ~/.claude",
        "rm -rf ~/.agents",
        "rm -rf ~/.codex",
        "mv ~/.claude /tmp/x",
        // A name that merely ends like a fragment, behind a prefix marker.
        "rm -rf ~/backup.claude",
        "cp -r ~/backup.claude /tmp/",
        "rm -rf ~/projects/legacy.claude",
        "rm -rf ~/notes.agents",
        "7z x a.7z -obackup.claude",
        "curl -sS https://x -o-.claude",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "`{line}` answers boundary, and the ladder has built a path out of its own rungs"
        );
    }
    // And the ladder still reaches what it is for.
    for line in [
        "7z x a.7z -o.estigia",
        "wget -O.claude/settings.json https://x",
        "rm -rf %~dp0.estigia/run.json",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and its option prefix is holding a control surface"
        );
    }
}

/// A later operand does not erase the surface named by an earlier one.
///
/// The fifth loss on this road, and the worst, because the commands are the
/// ordinary way to move a config file aside rather than an adversarial spelling.
/// Folding whitespace to a separator made the whole line read as one path, and
/// the parent-segment collapse then reached **across an operand boundary**: `mv
/// .estigia ..` became `mv/.estigia/../` and collapsed to `mv/`, deleting the
/// surface being moved. At the base the tokens were joined with a space, so
/// `/../` never formed there and a bare `contains` gated it. A reviewer measured
/// 66 of these.
///
/// Each token is normalised on its own now and joined afterwards, so a `..` can
/// only collapse what its own operand names.
#[test]
fn a_later_operand_does_not_collapse_an_earlier_surface() {
    for line in [
        "mv .estigia ..",
        "mv .estigia ../backup",
        "cp -r .estigia ../snapshot",
        "rm .estigia ../other",
        "mv /home/me/.claude/settings.json ..",
        "mv /home/me/.claude/settings.json ../backup",
        "cp /home/me/.config/gh/hosts.yml ../keep",
        "mv /home/me/.codex/config.toml ../old",
        "install .claude/settings.json ../out",
        "cp .estigia/run.json ../../elsewhere",
        // And the collapse still works within one operand, which is the whole
        // reason it exists: this names the file the gate is registered in.
        "rm /home/me/.claude/skills/../settings.json",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and an operand of it names a control surface"
        );
    }
}

/// An option prefix is read past its quotes, and past an expansion too.
///
/// The sixth loss, and the same family as the fourth one quote wide: the suffix
/// rule filtered on the **raw** token beginning with `-`, so a leading quote
/// defeated it while the fold still left the option letters between the
/// separator and the fragment. Quoting `-o<dir>` is the documented 7-Zip habit,
/// for exactly the reason the rule exists — the directory may hold a space.
///
/// `~` sits beside `-` for the same reason and was measured on the same round:
/// `%~dp0` expands *with* a trailing separator, so `%~dp0.estigia\run.json` is
/// the correct batch idiom and puts a digit where the anchoring wants a
/// separator. The rule reads segments off the folded token now, so a quote, a
/// percent or anything else in the folded set gets out of the way first.
#[test]
fn a_quoted_or_expanded_option_prefix_does_not_hide_a_control_surface() {
    let dq = '"';
    let sq = '\'';
    let pct = '%';
    for line in [
        format!("wget {dq}-O.claude/settings.json{dq} https://x"),
        format!("wget {sq}-O.claude/settings.json{sq} https://x"),
        format!("7z x a.7z {sq}-o.estigia/run.json{sq}"),
        format!("7z x a.7z {dq}-o.estigia/stand-down.json{dq}"),
        format!("wget {dq}-Oskills/flow/SKILL.md{dq} https://x"),
        format!("wget {sq}-O.config/gh/hosts.yml{sq} https://x"),
        format!("wget {sq}-O.claude.json{sq} https://x"),
        format!("rm -rf {pct}~dp0.estigia/run.json"),
        format!("cp a {pct}~dp0.estigia/run.json"),
        format!("echo x > {pct}~dp0.claude/settings.json"),
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": &line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and its option prefix is holding a control surface"
        );
    }
}

/// A short option carrying its value attached, and a brace expansion.
///
/// The fourth family of shell-road loss this branch has paid for, and the one
/// that needed a rule rather than another folded character: the thing standing
/// between the separator and the fragment is an ordinary letter. `-` cannot be
/// folded, because `hooks/pre-push`, `.estigia/stand-down.json` and cursor's
/// derived fragment all carry one and folding would cut them in half. So every
/// split point of a token beginning with `-` is offered to the matcher instead.
///
/// `7z` is why this is not optional: its extract-to spelling is `-oDIR` and a
/// space there is a syntax error, so the only correct way to write "extract into
/// the state directory" was the way that was not gated. A reviewer measured all
/// of these against the base, where a bare `contains` had gated every one.
#[test]
fn an_option_prefix_does_not_hide_a_control_surface() {
    for (line, why) in [
        (
            "7z x pack.7z -o.estigia",
            "7z, whose only spelling is attached",
        ),
        ("unzip pack.zip -d.estigia", "unzip -d"),
        ("tar -xf pack.tar -C.estigia", "tar -C"),
        (
            "tar -xf pack.tar -C.config/opencode",
            "tar -C into a config root",
        ),
        ("curl -o.estigia/run.json https://x", "curl -o"),
        (
            "curl -so.estigia/stand-down.json https://x",
            "curl with clustered flags",
        ),
        ("curl -Lo.claude/settings.json https://x", "curl -Lo"),
        (
            "curl -o.config/gh/hosts.yml https://x",
            "curl -o onto the hosts file",
        ),
        (
            "curl -oskills/flow/SKILL.md https://x",
            "a value with no dot to anchor on",
        ),
        ("wget -O.claude/settings.json https://x", "wget -O"),
        (
            "wget -q -O.config/gh/hosts.yml https://x",
            "wget with a flag before it",
        ),
        ("cp -t.estigia evil.json", "cp -t"),
        ("install -D.estigia/run.json", "install -D"),
        (
            "curl -o.opencode/agents/reviewer.md https://x",
            "a definition root",
        ),
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and it names a control surface through {why}"
        );
    }
}

/// Brace expansion puts a real path one character past a separator.
///
/// Measured by a reviewer alongside the option prefix, and closed the other way:
/// no fragment carries a brace, so the braces simply joined the folded set.
#[test]
fn a_brace_expansion_does_not_hide_a_control_surface() {
    for line in [
        "rm -rf ~/{.estigia/run.json,foo}",
        "rm -rf ~/{.claude/settings.json,foo}",
        "rm ~/{.config/gh/hosts.yml,x}",
        "cp a ~/{.claude/settings.json}",
        "rm -rf {.estigia/,dist}",
        "rm -rf ~/{.claude/agents,build}",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and a brace expansion is naming a control surface"
        );
    }
}

/// The option-prefix rule is generous, and this prices it.
///
/// Offering every split point of a dash token can only add matches, so the cost
/// it could carry is a false `Boundary` on an ordinary line — a live tracker
/// read on every invocation. Measured across the write-heavy commands a run
/// actually issues, including the ones whose flags take attached paths.
#[test]
fn an_option_prefix_does_not_gate_an_ordinary_line() {
    for line in [
        "tar -xf pack.tar -C build",
        "unzip pack.zip -d build",
        "7z x pack.7z -obuild",
        "curl -o out.json https://example.com",
        "wget -O out.html https://example.com",
        "cp -t dist src/a.js",
        "install -m 755 script /usr/local/bin",
        "dd if=/dev/zero of=out.img bs=1M",
        "node --max-old-space-size=4096 index.js",
        "rsync -av src/ dest/",
        "make -j8 all",
        "go build -o bin/app ./cmd",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "`{line}` answers boundary, and it names no control surface"
        );
    }
}

/// Every character in the folded set, one at a time, spelled rather than read.
///
/// The set was justified four characters at a time, each added because a
/// reviewer measured a spelling that had lost its boundary — and the ones nobody
/// had measured were justified by the sentence *"the rest terminate a word the
/// same way"*. A reviewer mutated them and found ten of fourteen could be
/// dropped with the whole suite green. The brace was the proof the reasoning was
/// not enough: it was missing from the set entirely and cost a real boundary.
///
/// The list is **spelled here** and crossed against the constant below. A first
/// draft iterated `NOT_IN_A_PATH_SEGMENT` instead, which reads like coverage and
/// is none: dropping a character from the constant also drops it from the test,
/// so eleven of the sixteen mutations still survived green. A fixture that reads
/// the thing it is checking asserts only that the code equals itself.
///
/// The shape is the one every loss had in common: an ordinary letter, the
/// character, then the fragment — unreachable by the left anchoring unless the
/// character folds to a separator.
const FOLDED_CHARACTERS: &[char] = &[
    '"', '\'', '`', '<', '>', '=', '|', ';', '&', '(', ')', '$', '*', ',', '{', '}', '%', '^',
];

#[test]
fn every_folded_character_reaches_a_control_surface_behind_it() {
    for divider in FOLDED_CHARACTERS {
        let line = format!("rm -rf x{divider}.estigia/stand-down.json");
        let (_, how) = classify("Bash", &serde_json::json!({ "command": &line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, so {divider:?} is in the folded set without being folded"
        );
    }
}

/// The spelled list and the constant agree.
///
/// So a character added to the set without a row above fails here rather than
/// arriving untested, and one removed from the set fails rather than quietly
/// taking its own coverage with it.
#[test]
fn the_spelled_folded_characters_and_the_constant_agree() {
    let spelled: String = FOLDED_CHARACTERS.iter().collect();
    assert_eq!(
        spelled,
        super::NOT_IN_A_PATH_SEGMENT,
        "the folded set and the list the fixture walks have diverged"
    );
}

/// The two halves of the wrap, and which of them is holding anything.
///
/// The trailing half **is** load-bearing, and only became so when the
/// option-prefix rule landed: the suffixes it offers are appended after the
/// wrap, so without the trailing separator the first suffix runs straight into
/// the folded line and the last segment of a command stops being a whole one.
/// Two fixtures redden without it, and it is the separator between tokens
/// that carries it rather than a wrap at the end of the line. An earlier draft of `docs/honesty.md` said
/// it was not load-bearing — true when written, false one commit later, and
/// nobody re-measured it. That is the failure this crate keeps paying for.
///
/// The leading half is inert and structurally cannot hide anything, because
/// `anchored` already tests `starts_with` as well as `contains`. Dropping it can
/// only *add* matches, never remove one. Measured over every fragment the gate
/// consults, five spellings and five write verbs — 975 command lines, zero
/// answers changed. It stays for symmetry, and this says so rather than letting
/// a reader infer a guard that is not there.
#[test]
fn the_trailing_wrap_keeps_the_last_segment_whole() {
    for line in [
        "rm -rf /home/me/.claude/agents",
        "rm -rf /repo/.estigia",
        "curl -o/home/me/.claude/settings.json https://x",
        "7z x pack.7z -o/repo/.estigia",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and its last segment names a control surface"
        );
    }
}

/// A control surface is reached however the line is punctuated.
///
/// `surface_of` folds every character a path segment cannot contain into a
/// separator, and this holds that direction. It exists because two narrower
/// attempts did not: splitting on whitespace and appending a separator left the
/// left anchoring unreachable for a relative operand, and wrapping each token
/// fixed only the operands whose first character *is* the fragment's. A quote, a
/// redirect written without a space, an operand joined to a long flag and a
/// drive-relative prefix all sit between the token boundary and the fragment. A
/// reviewer measured every family here against the base commit, where a bare
/// `contains` had gated them; the suite was green over all of it, because every
/// fixture in this file spelled its commands plainly.
#[test]
fn punctuation_does_not_hide_a_control_surface() {
    let dq = '"';
    let sq = '\'';
    for (line, why) in [
        (
            format!("rm -rf {dq}.estigia/stand-down.json{dq}"),
            "double quotes",
        ),
        (
            format!("rm -rf {sq}.estigia/stand-down.json{sq}"),
            "single quotes",
        ),
        (
            format!("rm -rf {dq}.claude/settings.json{dq}"),
            "double quotes, settings",
        ),
        (
            format!("rm -rf {dq}.estigia/{dq}"),
            "quoted with a trailing separator",
        ),
        (
            format!("mv {dq}.claude/settings.json{dq} /tmp/x"),
            "quoted, moved",
        ),
        (
            "echo x >.claude/settings.json".to_string(),
            "redirect, no space",
        ),
        (
            "echo x>.claude/settings.json".to_string(),
            "redirect, no spaces at all",
        ),
        (
            "echo x >>.estigia/run.json".to_string(),
            "appending redirect",
        ),
        (
            ": >.claude/settings.json".to_string(),
            "truncating redirect",
        ),
        (
            "sed -i --file=.claude/settings.json -e s/a/b/".to_string(),
            "operand joined to a long flag",
        ),
        (
            "tee --output=.claude/settings.json".to_string(),
            "another long flag",
        ),
        // A quoted **bare directory**, which a reviewer measured separately: the
        // trailing separator the fragment carries has to be supplied by the fold
        // and the leading one by the quote, so both ends are reached at once.
        (
            format!("rm -rf {dq}/repo/.opencode/agents{dq}"),
            "quoted bare directory",
        ),
        (
            format!("rm -rf {dq}/home/me/.claude/agents{dq}"),
            "quoted bare home directory",
        ),
        (
            format!("rm -rf {sq}/repo/.estigia{sq}"),
            "quoted bare state directory",
        ),
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": &line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, and it names a control surface through {why}"
        );
    }
}

/// The drive-relative Windows spelling, on both roads.
///
/// `C:.estigia\stand-down.json` is the current directory *of that drive*, not
/// its root, and it names the same file as the bare relative spelling. It is the
/// one punctuation family that reaches the write road as well, because the drive
/// prefix sits where the left anchoring looks for a separator.
#[test]
fn a_drive_relative_spelling_is_measured_like_a_relative_one() {
    for named in [
        r"C:.estigia\stand-down.json",
        r"C:.estigia\runs\a.json",
        r"C:.claude\settings.json",
        r"c:.claude/settings.local.json",
        r"D:.claude\agents\reviewer.md",
    ] {
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": named })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -rf {named}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{road} on the drive-relative {named} answers routine, and it names a control surface"
            );
        }
    }
}

/// An ordinary line stays `Routine` after the folding.
///
/// The other direction of the same change: reading punctuation as separators
/// makes the view longer and more permissive, so it has to be shown not to gate
/// the commands a run issues all day. A false `Boundary` here is a live tracker
/// read on every build.
#[test]
fn folding_punctuation_does_not_gate_an_ordinary_line() {
    for line in [
        "cargo build --release",
        "cargo test --all-targets",
        "git commit -m wip",
        "git log --oneline -20",
        "rm -rf target/debug",
        "npm run build",
        "mkdir -p src/harness",
        "echo done > /tmp/out.txt",
        "grep -rn settings src/",
        "sed -i s/a/b/ README.md",
    ] {
        let (_, how) = classify("Bash", &serde_json::json!({ "command": line }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "`{line}` answers boundary, and it names no control surface"
        );
    }
}

/// A relative operand reaches the same answer as an absolute one.
///
/// `surface_of` joins a command's tokens with **spaces**, so an anchored
/// fragment finds no separator in front of a relative operand and none of them
/// is at position 0 — the verb is. Every fixture in this file spelled its paths
/// absolutely, so `rm -rf .estigia` and `echo x > .claude/settings.json` went
/// `Boundary` to `Routine` with the whole suite green, while `Write` still
/// answered `Boundary` for the same path. That is the road split the anchoring
/// was added to close, reappearing inverted on the spelling an agent inside a
/// repository types most.
#[test]
fn a_relative_operand_is_measured_like_an_absolute_one() {
    for named in [
        ".estigia",
        ".estigia/state.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude/agents",
        ".claude/agents/reviewer.md",
        ".opencode/agents/reviewer.md",
        ".config/gh/hosts.yml",
        ".codex/hooks.json",
        ".cursor/rules/estigia.md",
    ] {
        for (road, payload) in [
            ("Write", serde_json::json!({ "file_path": named })),
            (
                "Bash",
                serde_json::json!({ "command": format!("rm -rf {named}") }),
            ),
            (
                "Bash",
                serde_json::json!({ "command": format!("echo x > {named}") }),
            ),
        ] {
            let (_, how) = classify(road, &payload);
            assert_eq!(
                how,
                Sensitivity::Boundary,
                "{road} on the relative spelling {named} answers routine, and the absolute spelling of the same path is a boundary"
            );
        }
    }
}

/// Every root `roles::definition_for` searches, including the repository's own.
///
/// A definition carries a tool allowlist and a definition that is not found is
/// `Ok(None)`, which `declared_policy` reads as *every tool allowed*. The
/// repository-local OpenCode root was the one nothing reached: `.claude/agents/`
/// covered its Claude twin, `opencode/agents/` is anchored so `.opencode` is not
/// it, and the two roots sit in one `vec!` gated by two different fragments.
/// Both documents claimed every root was watched while this one was `Routine`.
#[test]
fn every_definition_root_is_a_boundary_on_both_roads() {
    for root in [
        "/repo/.claude/agents",
        "/repo/.opencode/agents",
        "/home/me/.claude/agents",
        "/home/me/.config/opencode/agents",
        "/xdg/opencode/agents",
    ] {
        for named in [root.to_string(), format!("{root}/reviewer.md")] {
            for (road, payload) in [
                ("Write", serde_json::json!({ "file_path": &named })),
                (
                    "Bash",
                    serde_json::json!({ "command": format!("rm -rf {named}") }),
                ),
            ] {
                let (_, how) = classify(road, &payload);
                assert_eq!(
                    how,
                    Sensitivity::Boundary,
                    "{road} on {named} answers routine, and it is a root `definition_for` reads a tool allowlist from"
                );
            }
        }
    }
}

/// The directory itself, and what is under it, on both roads.
///
/// The other half of the same rule: a fragment naming a directory has to reach
/// the directory. This is what the trailing slash cost before the matcher
/// honoured it — measured on the state directory, the installed contract, the
/// agent definitions and a rules directory.
#[test]
fn a_directory_entry_reaches_the_directory_itself_on_both_roads() {
    for named in [
        "/home/me/.claude/skills/flow",
        "/repo/.estigia",
        "/home/me/.claude/agents",
        "/home/me/.cline/rules",
        "/home/me/.config/opencode",
    ] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": named }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to the bare directory {named} answers routine"
        );

        let (_, removed) = classify(
            "Bash",
            &serde_json::json!({ "command": format!("rm -rf {named}") }),
        );
        assert_eq!(
            removed,
            Sensitivity::Boundary,
            "removing {named} through the shell answers routine"
        );
    }
}

/// An existing entry's sensitivity did not change on the way past.
///
/// `.config/opencode/` covered everything under it. A first attempt at the
/// `XDG_CONFIG_HOME` fix replaced it with three tails, and a reviewer measured
/// what that quietly gave up: what Estigia does not write under that directory
/// went `Boundary` to `Routine` — a loosening, presented in the entry as an
/// over-gating fix. No count, deliberately: three drafts said *nine* without
/// anybody enumerating them, and `docs/honesty.md` retracted it while this
/// comment kept it. One fact in two places is a fact that disagrees. The directory entry is back beside the tails, which
/// cover the relocated case it could not.
#[test]
fn the_opencode_config_directory_is_still_covered_whole() {
    for under in [
        "/home/me/.config/opencode/mcp.json",
        "/home/me/.config/opencode/themes/dark.json",
    ] {
        let (_, how) = classify("Write", &serde_json::json!({ "file_path": under }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "{under} was covered before this change and answers routine now"
        );
    }
}

/// The covered checkout is resolved the same way the target is.
///
/// `covers` leaves a path it cannot canonicalise literal. A covered checkout
/// that is not on disk — or one reached through a link, which is what macOS
/// hands every temporary directory as `/var/folders` against `/private/var` —
/// left the two sides of the comparison in different vocabularies, and a write
/// **into** the checkout read as outside. This crate had already measured that
/// asymmetry one module over, in `transport::branch`'s own note about 8.3 names
/// and `/private/var`.
#[test]
fn a_checkout_reached_by_another_name_still_covers_its_own_writes() {
    let root = tempfile::tempdir().expect("a root");
    let real = root.path().join("real");
    std::fs::create_dir_all(real.join("repo").join("src")).expect("a checkout");
    let alias = root.path().join("alias");

    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&alias)
        .arg(&real)
        .output()
        .is_ok_and(|out| out.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&real, &alias).is_ok();
    if !made {
        eprintln!("skipped: this platform would not create a directory link");
        return;
    }

    // The claim names one spelling and the write names the other, in both
    // directions — neither is more correct than the other, and a run is handed
    // whichever its worktree was created under.
    for (claimed, written) in [(&alias, &real), (&real, &alias)] {
        let run = sworn(164, &claimed.join("repo"));
        let target = written.join("repo").join("src").join("new.rs");
        assert!(
            !writes_outside_the_claim(
                &run,
                &Action::Write {
                    target: target.display().to_string()
                }
            ),
            "a write into the claimed checkout read as outside it, claimed as {}, written as {}",
            claimed.display(),
            written.display()
        );
    }

    // The case that makes the asymmetry visible on a machine whose temporary
    // directory is already canonical — which is most Windows boxes, and is why
    // this was green here and would have been red on the `macos-latest` lane.
    // The covered checkout does not exist, so `covers` keeps it literal, while
    // the target resolves through the link. Two vocabularies, one path.
    let unborn = alias.join("never-created");
    let absent = sworn(164, &unborn);
    assert!(
        !writes_outside_the_claim(
            &absent,
            &Action::Write {
                target: unborn.join("x.rs").display().to_string()
            }
        ),
        "a write into a covered checkout that is not on disk yet read as outside it"
    );
}

/// `..` after a link lands where the platform says, not where the spelling does.
///
/// The fourth door onto one defect, and the one that showed the premise was
/// wrong rather than the enumeration incomplete. The collapse was lexical, on
/// the grounds that *`..` is a spelling and not a boundary* — true on Windows,
/// whose own path resolver collapses it before touching the filesystem, and
/// false on POSIX, which follows the link first and then applies `..` to what it
/// resolved to.
///
/// So `<outside>/dlink/../planted.rs`, with `dlink` pointing at `<repo>/src`,
/// reads lexically as `<outside>/planted.rs` and lands at `<repo>/planted.rs`.
/// A repository write, with the gate standing aside, on the four POSIX targets
/// this crate ships.
///
/// Unix only, because on Windows the lexical reading **is** the landing and the
/// same fixture would be asserting the opposite. A skip that says so is honest;
/// the platform difference is the subject.
#[cfg(unix)]
#[test]
fn a_parent_segment_after_a_link_is_resolved_the_way_posix_resolves_it() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("a checkout");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&outside).expect("a directory that is not the checkout");

    let link = outside.join("dlink");
    std::os::unix::fs::symlink(repo.join("src"), &link).expect("a link into the checkout");

    let run = sworn(164, &repo);
    let spelled = link.join("..").join("planted.rs");
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: spelled.display().to_string()
            }
        ),
        "a write that lands in the claimed checkout was taken out of the gate by `..` after a link"
    );

    // Measured rather than argued: this is where the platform puts it.
    std::fs::write(&spelled, "planted\n").expect("the spelling really does write there");
    assert!(
        repo.join("planted.rs").is_file(),
        "the fixture does not reproduce the resolution order it is about"
    );
}

/// A covered checkout this process cannot place rules nothing out.
///
/// `placed` answers `None` for a path with a component that is there and will
/// not resolve. Applied to the **covered** side that has to mean *cannot be
/// ruled out* — inside — and the arm saying so was unmeasured: reversing it, so
/// an unplaceable checkout covers nothing and every write into it stands aside,
/// left the whole suite green.
#[test]
fn a_covered_checkout_that_cannot_be_placed_still_covers() {
    let root = tempfile::tempdir().expect("a root");
    let broken = root.path().join("dangling");
    #[cfg(windows)]
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&broken)
        .arg(root.path().join("never-created"))
        .output()
        .is_ok_and(|out| out.status.success());
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(root.path().join("never-created"), &broken).is_ok();
    if !made {
        eprintln!("skipped: this platform would not create a dangling link");
        return;
    }
    assert!(
        crate::paths::placed(&broken).is_none(),
        "the fixture does not reproduce an unplaceable path"
    );

    // The target has to be placeable, or the early return for an unplaceable
    // *target* answers first and the covered arm is never reached — which is
    // what an earlier version of this fixture did, leaving the mutant alive.
    let elsewhere = root.path().join("scratch").join("note.md");
    assert!(
        crate::paths::placed(&elsewhere).is_some(),
        "the fixture must ask about a target this process can place"
    );

    let run = sworn(164, &broken);
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: elsewhere.display().to_string()
            }
        ),
        "a run whose covered checkout cannot be placed had a write waved through"
    );
}

/// And the same parent segment on Windows, which resolves it the other way.
///
/// The `cfg!(unix)` split is load-bearing on **both** sides and only one had a
/// fixture: flipping it to `true` left the entire Windows suite green, while a
/// junction plus `..` then placed the write outside a checkout it lands in.
/// Windows collapses `..` before touching the filesystem, so the spelling *is*
/// the landing here — and asserting POSIX's answer on this platform removes the
/// gate exactly as asserting Windows' answer removed it there.
#[cfg(windows)]
#[test]
fn a_parent_segment_after_a_link_is_resolved_the_way_windows_resolves_it() {
    let root = tempfile::tempdir().expect("a root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("a checkout");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(&outside).expect("a directory that is not the checkout");

    // The link lives **inside** the checkout and points out of it. Windows pops
    // the `..` lexically, so the write lands back inside; POSIX would follow the
    // link first and land outside.
    let link = repo.join("link");
    let made = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .is_ok_and(|out| out.status.success());
    if !made {
        eprintln!("skipped: this platform would not create a directory link");
        return;
    }

    let run = sworn(164, &repo);
    let spelled = link.join("..").join("planted.rs");
    assert!(
        !writes_outside_the_claim(
            &run,
            &Action::Write {
                target: spelled.display().to_string()
            }
        ),
        "a write that lands in the claimed checkout was taken out of the gate by `..` after a link"
    );

    std::fs::write(&spelled, "planted\n").expect("the spelling really does write there");
    assert!(
        repo.join("planted.rs").is_file(),
        "the fixture does not reproduce the resolution order it is about"
    );
}

/// `gh`'s hosts file is a boundary write on both spellings and both roads.
///
/// It is the one path issue 2 asked to be **named** rather than left in a
/// class, because it decides which account every tracker call acts as. Nothing
/// held it: deleting both entries from `CONTROL_SURFACE` left every suite green,
/// and only the population fingerprint moved — which fires on any byte in the
/// list and measures no behaviour.
///
/// Both roads, because the first attempt covered only one. `surface_of` splits a
/// command on whitespace and appends `/` to each token, so a fragment carrying a
/// space could never fire through the shell: the Windows spelling answered
/// `Boundary` to the write tool and `Routine` to `rm`, which is the road an
/// agent would actually take.
#[test]
fn the_hosts_file_that_names_the_account_is_a_boundary_on_both_roads() {
    for path in [
        "/home/somebody/.config/gh/hosts.yml",
        r"C:\Users\somebody\AppData\Roaming\GitHub CLI\hosts.yml",
    ] {
        let (_, write) = classify("Write", &json!({ "file_path": path }));
        assert_eq!(
            write,
            Sensitivity::Boundary,
            "{path} is not measured when written through the write tool"
        );
        for verb in ["rm", "mv"] {
            let (_, shell) = classify("Bash", &json!({ "command": format!("{verb} \"{path}\"") }));
            assert_eq!(
                shell,
                Sensitivity::Boundary,
                "{path} is not measured when reached through `{verb}`"
            );
        }
    }
}
