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
        operation_window: Default::default(),
        issue_requirement: crate::config::IssueRequirement::Off,
        workflow: Some(crate::config::Workflow::Autonomous),
        window: RENEWAL_WINDOW,
        tracker: crate::config::Tracker::Github { repo: None },
        repository_host: crate::config::RepositoryHost::Github { repo: None },
        boundaries: Vec::new(),
    }
}

/// The leak: a conditional `set_var` that only
/// writes on `Some` leaves a prior pin for every later call that names none.
/// `apply_gh_repo(None)` is the production clear; without it this assert fails.
#[test]
fn a_named_gh_repo_pin_does_not_survive_a_call_that_names_none() {
    crate::test_env::with_process_env("GH_REPO", "stale/from-earlier-test", || {
        // The old shape, still the defect if nothing clears:
        if let Some(named) = None::<&str> {
            // SAFETY: inside with_process_env's lock; this arm never runs.
            unsafe { std::env::set_var("GH_REPO", named) };
        }
        assert_eq!(
            std::env::var("GH_REPO").ok().as_deref(),
            Some("stale/from-earlier-test"),
            "precondition lost: the conditional write no longer leaves a pin"
        );
        apply_gh_repo(None);
        assert!(
            std::env::var_os("GH_REPO").is_none(),
            "apply_gh_repo(None) left GH_REPO set — the leak at gate / \
             tracker_answer_for_pointer / mcp still sticks"
        );
        apply_gh_repo(Some("acme/app"));
        assert_eq!(std::env::var("GH_REPO").ok().as_deref(), Some("acme/app"));
        apply_gh_repo(None);
        assert!(std::env::var_os("GH_REPO").is_none());
    });
}

fn sworn(issue: u64, repo_dir: &Path) -> Run {
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(issue));
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
        "gh pr merge --squash 54",
        "gh pr merge -d 54",
        "gh pr merge 007",
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

#[test]
fn an_unidentified_pr_merge_refusal_names_the_shape_that_clears_it() {
    let refusal = delivery_pr_unidentified(None);
    let Resolution::NoCommand { detail, .. } = &refusal.resolution else {
        panic!("the way out of an unidentified merge is not a gap a command fills: {refusal}");
    };
    // The word order, because the parser requires it and nothing said so.
    assert!(
        detail.contains("before"),
        "the resolution does not say the number precedes the flags: {refusal}"
    );
    // And the spelling, because retaining `7` from `007` was interpretation —
    // the one thing this function exists not to do.
    assert!(
        detail.contains("leading zero"),
        "the resolution does not say how the number must be spelled: {refusal}"
    );
}

#[test]
fn an_unidentified_pr_merge_refusal_names_the_byte_that_disqualified_it() {
    let (action, sensitivity) =
        classify("Bash", &json!({"command": "gh pr merge 136 --merge 2>&1"}));
    assert_eq!(sensitivity, Sensitivity::Boundary);
    let Action::Boundary {
        command,
        pr: None,
        pr_unidentified_reason,
        ..
    } = &action
    else {
        panic!("a redirected merge retained a PR identity: {action:?}");
    };
    assert_eq!(command, "gh pr merge", "the classifier lost the command");
    // The byte that disqualified the command, carried to the refusal. Without
    // it the refusal says "does not name one positive numeric pull request" —
    // a sentence that is false of a command naming PR 136 — and the reader
    // goes looking for the number, which is the two-hour detour that was
    // measured.
    let Some(PrUnidentified::Byte(reason)) = pr_unidentified_reason else {
        panic!("a redirected merge was unidentified without the byte that did it");
    };
    let reason = reason.as_str();
    assert!(
        reason.contains('>'),
        "the disqualifying byte is not named: {reason}"
    );
    // And the refusal the reader actually sees says the same thing.
    let refusal = delivery_pr_unidentified(Some(reason.to_owned()));
    assert!(
        refusal.message.contains(reason),
        "the message does not name the byte that disqualified the command: {refusal}"
    );
    let Resolution::NoCommand { detail, .. } = &refusal.resolution else {
        panic!("the way out of an unidentified merge is not a gap a command fills: {refusal}");
    };
    assert!(
        detail.contains(reason),
        "the byte that disqualified the command is not in the way out: {refusal}"
    );
}

#[test]
fn a_merge_naming_a_repository_carries_that_reason_and_a_plain_merge_carries_none() {
    for command in [
        "gh pr merge 54 --repo foreign/repo",
        "gh pr merge 54 --repo=foreign/repo",
        "gh pr merge 54 -R foreign/repo",
        "gh pr merge 54 -Rforeign/repo",
        "gh pr merge 54 -dRforeign/repo",
    ] {
        let (action, _) = classify("Bash", &json!({"command": command}));
        assert!(
            matches!(
                action,
                Action::Boundary {
                    pr: None,
                    pr_unidentified_reason: Some(PrUnidentified::RepositoryOption),
                    ..
                }
            ),
            "{command} -> {action:?}"
        );
    }
    let (plain, _) = classify("Bash", &json!({"command": "gh pr merge 54 --merge"}));
    assert!(
        matches!(
            plain,
            Action::Boundary {
                pr: Some(54),
                pr_unidentified_reason: None,
                ..
            }
        ),
        "{plain:?}"
    );
    let (quoted, _) = classify("Bash", &json!({"command": "gh pr merge 54 --repo 'o/r'"}));
    assert!(
        matches!(
            quoted,
            Action::Boundary {
                pr_unidentified_reason: Some(PrUnidentified::Byte(_)),
                ..
            }
        ),
        "a disqualifying byte must still be named first: {quoted:?}"
    );
}

#[test]
fn the_repository_option_refusal_names_the_option_and_the_way_out() {
    let refusal = delivery_pr_repository_option();
    assert_eq!(refusal.code, "delivery-pr-repository-option");
    assert!(refusal.message.contains("--repo"), "{refusal}");
    assert!(
        !refusal.message.contains("does not name"),
        "the number is there; the refusal must not say it is missing: {refusal}"
    );
    let Resolution::NoCommand { detail, .. } = &refusal.resolution else {
        panic!("the way out is the operator's, not a command: {refusal}");
    };
    assert!(detail.contains("without `--repo`"), "{refusal}");
    assert_eq!(
        super::refusal_class::class_of(refusal.code),
        Some(crate::outcome::RefusalClass::Refused)
    );
}

#[test]
fn a_redirected_merge_reports_what_it_read_without_a_source_reading() {
    let (_, sensitivity, observed) = classify_observing(
        "Bash",
        &json!({"command": "gh pr merge 136 --merge 2>&1"}),
        &[],
    );
    assert_eq!(sensitivity, Sensitivity::Boundary);
    assert_eq!(
        observed.matched,
        crate::outcome::Established::Known("gh pr merge".to_owned()),
        "the observed fragment is not the one the classifier matched: {observed:?}"
    );
    assert_eq!(
        observed.literal,
        crate::outcome::Established::Known("gh pr merge 136 --merge 2>&1".to_owned()),
        "the observed literal is not the full raw command line: {observed:?}"
    );
    let disqualifying_byte = observed
        .disqualifying_byte
        .as_deref()
        .expect("a redirected merge names the byte that disqualified it");
    assert!(
        disqualifying_byte.contains('>'),
        "the observed disqualifying byte is not the redirect: {observed:?}"
    );
}

#[test]
fn boundary_source_distinguishes_the_built_in_list_from_an_operator_declared_one() {
    let (_, _, built_in) =
        classify_observing("Bash", &json!({"command": "git push origin main"}), &[]);
    assert_eq!(
        built_in.boundary_source,
        Some("irreversible"),
        "a built-in boundary is not reported as coming from the irreversible list: {built_in:?}"
    );

    let (_, _, declared) = classify_observing(
        "Bash",
        &json!({"command": "npm publish"}),
        &["npm publish".to_owned()],
    );
    assert_eq!(
        declared.boundary_source,
        Some("operator-declared"),
        "an operator-declared boundary is not distinguished from the built-in list: {declared:?}"
    );
}

/// `DISARMS_THE_HARNESS` is searched against `as_estigia(&normalized)`,
/// which folds any word whose basename is `estigia`/`estigia.exe` to the
/// bare word — not the shared `normalized` value most other arms report
/// verbatim. A path prefix is the case that shows the difference: the line
/// this arm actually searched contains the fragment it matched; the shared
/// `normalized` line, reported before this fix, does not.
#[test]
fn the_disarm_arm_reports_the_line_it_actually_searched() {
    let (_, sensitivity, observed) = classify_observing(
        "Bash",
        &json!({"command": "/opt/bin/estigia.exe uninstall"}),
        &[],
    );
    assert_eq!(sensitivity, Sensitivity::Boundary);
    assert_eq!(
        observed.matched,
        crate::outcome::Established::Known("estigia uninstall".to_owned()),
        "the disarm arm did not report the fragment it matched: {observed:?}"
    );
    let crate::outcome::Established::Known(against) = &observed.against else {
        panic!("the disarm arm's `against` was not established: {observed:?}");
    };
    assert!(
        against.contains("estigia uninstall"),
        "`against` does not contain the fragment `matched` names, so it is not the line the \
         disarm arm actually searched: {observed:?}"
    );
}

#[test]
fn every_gated_spelling_has_a_command_line_written_out() {
    const BOUNDARIES: &[&str] = &[
        "git push origin main",
        "git merge --no-ff feature",
        "git pull origin main --no-rebase",
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
        "git switch --create work",
        "git switch --force-create work",
        "git switch --orphan work",
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
                &crate::identity::Identity::Number(12),
                crate::config::Integration::Branch,
                None,
                &Run::new("claude-abcd1234".to_owned()),
                Path::new("/repo"),
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
    let source = crate::source_text::code_of(include_str!("mod.rs"));
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
    // Each listed spelling reaches the boundary arm. This is the "these are
    // gated" half; `IRREVERSIBLE_POPULATION` owns the "these are not the only
    // spellings" half.
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
fn git_pull_is_a_boundary_like_git_merge_and_only_one_of_the_two_delivers() {
    let (merge_action, merge_how) =
        classify("Bash", &json!({ "command": "git merge origin/main" }));
    let (pull_action, pull_how) = classify(
        "Bash",
        &json!({ "command": "git pull origin main --no-rebase --no-edit" }),
    );
    assert_eq!(merge_how, Sensitivity::Boundary);
    assert_eq!(pull_how, Sensitivity::Boundary);
    let Action::Boundary {
        command: merge_fragment,
        ..
    } = &merge_action
    else {
        panic!("the merge spelling reached no boundary");
    };
    let Action::Boundary {
        command: pull_fragment,
        ..
    } = &pull_action
    else {
        panic!("the pull spelling reached no boundary");
    };

    assert!(DELIVERS.contains(&merge_fragment.as_str()));
    assert!(
        !DELIVERS.contains(&pull_fragment.as_str()),
        "the fragment recorded for a pull line was {pull_fragment}, which `DELIVERS` holds; \
         an integration must reach `Action::Boundary` so the claim is verified, without the \
         recorded fragment joining `DELIVERS`, which is what `stale_verdict` and `out_of_phase` read"
    );
}

#[test]
fn an_integration_clears_stale_verdict_rather_than_being_refused_as_a_delivery() {
    let rig = integration_rig();
    let (action, how) = classify(
        "Bash",
        &json!({ "command": "git pull origin main --no-rebase --no-edit" }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "the pull spelling reached no boundary, so the claim is not verified"
    );
    let Action::Boundary { command, .. } = &action else {
        panic!("the pull spelling reached no boundary");
    };
    assert!(!DELIVERS.contains(&command.as_str()));
    assert!(
        stale_verdict(&action, &rig.run, &rig.primary, &|_| {
            panic!("an integration never asks for the PR head")
        })
        .is_none(),
        "an integration line was refused as a stale delivery: the guard reads `IRREVERSIBLE` \
         rather than `DELIVERS`, or the recorded fragment joined `DELIVERS`"
    );
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
    // `git tag --list` was not this defect and was fixed separately: the fragment
    // really is the whole command there, so telling a listing from a creation
    // means reading the flags, and `git tag -d` stays a boundary.
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
fn git_tag_listing_is_not_a_boundary_and_creation_still_is() {
    for listing in [
        "git tag",
        "git tag -l",
        "git tag --list",
        "git tag -l 'v*'",
        "git tag --list 'v*'",
        "cd /x && git tag --list",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": listing }));
        assert_eq!(how, Sensitivity::Routine, "{listing}");
        assert!(matches!(action, Action::Untouched), "{listing}");
    }
    for boundary in [
        "git tag v1.2.3",
        "git tag -a v1.0.0 -m x",
        "git tag -d v1.2.3",
        "cd /x && git tag v1.0",
        "git tag --list && git tag v1.0",
        "git tag -a v1.0.0 -m 'release --list'",
        "git tag -a v1 -m -l",
        "git tag -a v1 -m '--list=v*'",
        "git tag --list=v1",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}");
        assert!(matches!(action, Action::Boundary { .. }), "{boundary}");
    }
}

#[test]
fn a_command_name_written_as_data_is_not_a_command() {
    for read in [
        r#"gh issue view 61 --json comments --jq '.comments[] | split("issue-flow: ")[1]'"#,
        "gh issue view 61 --jq 'git push origin main'",
        "grep 'git tag v1' f",
        "echo \"git tag v1\"",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": read }));
        assert_eq!(how, Sensitivity::Routine, "{read}: {action:?}");
        assert!(matches!(action, Action::Untouched), "{read}: {action:?}");
    }
}

#[test]
fn a_command_name_written_as_a_heredoc_body_is_not_a_command() {
    for command in [
        "cat > /tmp/note.md <<EOF\n\
         a refusal quoting estigia stand-down and git push\n\
         EOF",
        "cat <<EOF\n\
         a refusal quoting estigia stand-down and git push\n\
         EOF",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": command }));
        assert_eq!(how, Sensitivity::Routine, "{action:?}");
        assert!(!matches!(action, Action::Boundary { .. }), "{action:?}");
    }
}

#[test]
fn a_gated_command_is_still_refused_in_first_position_and_after_a_separator() {
    for boundary in [
        "git push origin main",
        "cd /x && git push origin main",
        "git tag v1.2.3",
        "cd /x && git tag v1.2.3",
        "estigia uninstall",
        "true; estigia uninstall",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}");
        assert!(matches!(action, Action::Boundary { .. }), "{boundary}");
    }
}

#[test]
fn a_gated_command_survives_every_interpreter_spelling() {
    for boundary in [
        "sh -c 'git push origin main'",
        "bash -c \"git push origin main\"",
        "su -c 'git push origin main'",
        "script -c 'git push origin main'",
        "eval 'git push origin main'",
        "python -c 'import os; os.system(\"git push origin main\")'",
        "ssh host 'git push origin main'",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}: {action:?}");
        assert!(
            matches!(action, Action::Boundary { .. }),
            "{boundary}: {action:?}"
        );
    }
    for disarm in [
        "sh -c 'estigia uninstall'",
        "eval 'estigia uninstall'",
        "bash -c \"estigia uninstall\"",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": disarm }));
        assert_eq!(how, Sensitivity::Boundary, "{disarm}: {action:?}");
    }
}

#[test]
fn a_gated_command_survives_heredoc_command_substitution() {
    for boundary in [
        "cat <<EOF\n$(git push origin main)\nEOF",
        "cat <<EOF\n`git push origin main`\nEOF",
        "cat << EOF\n$(git push origin main)\nEOF",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}: {action:?}");
    }
    let (action, how) = classify(
        "Bash",
        &json!({ "command": "cat <<EOF\n$(estigia uninstall)\nEOF" }),
    );
    assert_eq!(how, Sensitivity::Boundary, "{action:?}");
}

#[test]
fn a_comment_does_not_swallow_the_command_after_it() {
    for boundary in [
        "echo \"checking\"  # don't run this twice\ngit push origin main",
        "# don't\ngit push origin main",
        "echo x # a comment\ngit tag v1.2.3",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}: {action:?}");
    }
    let (action, how) = classify("Bash", &json!({ "command": "# don't\nestigia uninstall" }));
    assert_eq!(how, Sensitivity::Boundary, "{action:?}");
}

#[test]
fn a_quoted_listing_flag_still_lists() {
    for listing in ["git tag '--list'", "git tag '-l'", "git tag \"--list\""] {
        let (action, how) = classify("Bash", &json!({ "command": listing }));
        assert_eq!(how, Sensitivity::Routine, "{listing}: {action:?}");
    }
}

/// The floor: every spelling an executor can be handed a gated command through
/// stays a boundary, whatever the data suppression does to a read-only line.
///
/// Each of these is a shape a blind panel found the masking approach letting
/// through. They are the differential floor against the whole-text scan: the
/// suppression may only subtract a span when the command line is one simple
/// command that cannot run its arguments.
#[test]
fn an_executor_spelling_keeps_its_gated_command() {
    for boundary in [
        "echo 'git push origin main' | sh",
        "printf 'git push origin main' | bash",
        "sh <<EOF\ngit push origin main\nEOF",
        "bash <<'EOF'\ngit push origin main\nEOF",
        "bash <<< 'git push origin main'",
        "bash -lc 'git push origin main'",
        "dash -c 'git push origin main'",
        "eval 'git push origin main'",
        "ssh host 'git push origin main'",
        "sh -c 'git push origin main'",
        "php -r 'system(\"git push origin main\");'",
        "lua -e 'os.execute(\"git push origin main\")'",
        "rg --pre 'git push origin main' f",
        "sort --compress-program 'git push origin main' f",
        "gh codespace ssh -c box -- 'git push origin main'",
        "gh cs ssh -c box -- 'git push origin main'",
        "gh extension exec 'git push origin main'",
        "gh 'codespace' 'ssh' -c box -- 'git push origin main'",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": boundary }));
        assert_eq!(how, Sensitivity::Boundary, "{boundary}: {action:?}");
        assert!(
            matches!(action, Action::Boundary { .. }),
            "{boundary}: {action:?}"
        );
    }
    for disarm in [
        "sh -euc 'estigia uninstall'",
        "\"estigia\" uninstall",
        "git config \"core.hooksPath\" /dev/null",
        "eval 'estigia uninstall'",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": disarm }));
        assert_eq!(how, Sensitivity::Boundary, "{disarm}: {action:?}");
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
fn a_quote_opened_in_one_command_key_does_not_blank_another() {
    for input in [
        json!({"command": "echo '", "commandLine": "git push #'"}),
        json!({
            "command": "cat <<'EOF'",
            "commandLine": "git push origin main\nEOF",
        }),
        json!({"command": ["echo", "'"], "commandLine": "git push #'"}),
    ] {
        let (_, how) = classify("Bash", &input);
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "a boundary was blanked across keys: {input}"
        );
    }

    for single in [
        json!({"command": "git push #'"}),
        json!({"command": "git push origin main\nEOF"}),
    ] {
        let (_, how) = classify("Bash", &single);
        assert_eq!(how, Sensitivity::Boundary, "{single}");
    }

    for data in [
        json!({"command": "echo 'git push'"}),
        json!({"command": "gh issue comment 1 --body 'run git push later'"}),
    ] {
        let (_, how) = classify("Bash", &data);
        assert_ne!(how, Sensitivity::Boundary, "over-refused: {data}");
    }
}

#[test]
fn no_compound_or_redirected_merge_earns_the_integration_exemption() {
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |arguments: &[&str]| {
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
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);

    let root = tempfile::tempdir().expect("a gate root");
    let mut context = context(root.path());
    context.repo_dir = repo.path().to_path_buf();
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let issue = 33;
    let run_id = "claude-abcd1234";
    let mut base_run = Run::new(run_id.to_owned());
    base_run.issue = Some(crate::identity::Identity::Number(issue));
    base_run.state = Some("in-progress".to_owned());
    base_run.repo_dir = Some(repo.path().to_path_buf());
    // The exemption reads `run.worktree` alone (see `docs/honesty.md`'s
    // correction), so this is what has to be set for either exemption branch
    // to even be reachable — the pointer this whole finding is about.
    base_run.worktree = Some(repo.path().to_path_buf());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        run_id,
        issue,
        "in-progress",
    )]);

    // Every shape the finding names, verbatim.
    let refused = [
        "git merge origin/main && gh pr merge 66 --squash",
        "git merge origin/main && git tag v1.0",
        "git merge origin/main; gh release create v1.0",
        "git merge origin/main\ngh pr merge 66",
        "git -C elsewhere merge origin/main",
        "git --git-dir=.git merge origin/main",
        "git -c core.hooksPath=/dev/null merge origin/main",
        "sudo git merge origin/main",
        "git merge origin/main other",
        "git merge $UPSTREAM",
        "git merge 'origin/main'",
    ];
    let allowed = [
        "git merge origin/main",
        "git merge --no-edit origin/main",
        "git merge --no-ff origin/main",
        "git merge -- origin/main",
        "git merge --continue",
        "git merge --abort",
    ];

    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        for command in refused {
            let (action, how) = classify("Bash", &json!({"command": command}));
            assert_eq!(how, Sensitivity::Boundary, "{command}");
            let mut run = base_run.clone();
            let decision = decide(&context, &mut run, &action, Sensitivity::Boundary);
            assert!(
                decision.denies(),
                "{command} earned the integration exemption: {decision:?}"
            );
        }
        for command in allowed {
            let (action, how) = classify("Bash", &json!({"command": command}));
            assert_eq!(how, Sensitivity::Boundary, "{command}");
            let mut run = base_run.clone();
            let decision = decide(&context, &mut run, &action, Sensitivity::Boundary);
            assert!(
                matches!(decision, Decision::Allow(_)),
                "{command} was refused: {decision:?}"
            );
        }
    });
}

#[test]
fn a_payload_without_one_literal_command_refuses_the_integration_exemption() {
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |arguments: &[&str]| {
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
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);

    let root = tempfile::tempdir().expect("a gate root");
    let mut context = context(root.path());
    context.repo_dir = repo.path().to_path_buf();
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let issue = 33;
    let run_id = "claude-fedc4321";
    let mut base_run = Run::new(run_id.to_owned());
    base_run.issue = Some(crate::identity::Identity::Number(issue));
    base_run.state = Some("in-progress".to_owned());
    base_run.repo_dir = Some(repo.path().to_path_buf());
    base_run.worktree = Some(repo.path().to_path_buf());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        run_id,
        issue,
        "in-progress",
    )]);

    // Each of these reaches `shell_command_argument` with no single JSON
    // string donating a literal: an argv array; two keys that both
    // normalise to a command, as plain strings; and the same ambiguity with
    // one of the two values an argv array instead. `git merge origin/main`
    // from `in-progress` is exactly the deadlock this was opened to
    // close for the exemption's intended case — a plain literal string —
    // and must stay refused `out-of-phase` for every shape here, because
    // none of them gives the gate a literal to prove.
    let payloads = [
        json!({"command": ["git", "merge", "origin/main"]}),
        json!({
            "command": "git merge origin/main",
            "commandLine": "git status",
        }),
        json!({
            "command": ["git", "merge", "origin/main"],
            "commandLine": ["git", "status"],
        }),
    ];

    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        for input in &payloads {
            let (action, how) = classify("Bash", input);
            assert_eq!(how, Sensitivity::Boundary, "{input}");
            assert!(
                matches!(
                    action,
                    Action::Boundary {
                        is_exact_integration_merge: false,
                        runs_in: crate::harness::RunsIn::Session,
                        target: crate::harness::CommitTarget::Checkout,
                        ..
                    }
                ),
                "{input} earned a literal the payload never gave it: {action:?}"
            );
            let mut run = base_run.clone();
            let decision = decide(&context, &mut run, &action, Sensitivity::Boundary);
            assert!(
                decision.denies(),
                "{input} earned the integration exemption without one literal command: {decision:?}"
            );
        }
    });
}

/// A `git merge` in the run's own checkout whose shape is not the one
/// [`exact_integration_merge`] proves is refused with the shape named, not with
/// a delivery reason that is false for it. The line a run actually reached was
/// `git -C <worktree> merge --no-edit origin/main 2>&1 | tail -3`; the pipe is
/// what disqualified it, and nothing said so.
#[test]
fn a_near_miss_integration_names_the_shape_instead_of_a_delivery_reason() {
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |arguments: &[&str]| {
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
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);

    let root = tempfile::tempdir().expect("a gate root");
    let mut context = context(root.path());
    context.repo_dir = repo.path().to_path_buf();
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");

    let issue = 33;
    let run_id = "claude-9191abcd";
    let mut base_run = Run::new(run_id.to_owned());
    base_run.issue = Some(crate::identity::Identity::Number(issue));
    base_run.state = Some("in-progress".to_owned());
    base_run.repo_dir = Some(repo.path().to_path_buf());
    base_run.worktree = Some(repo.path().to_path_buf());

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::live_holder_issue(
        run_id,
        issue,
        "in-progress",
    )]);

    let worktree = crate::test_env::shell_path(repo.path());
    let piped = format!("git -C {worktree} merge --no-edit origin/main 2>&1 | tail -3");
    let bare = format!("git -C {worktree} merge --no-edit origin/main");

    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let (action, how) = classify("Bash", &json!({"command": piped}));
        assert_eq!(how, Sensitivity::Boundary, "{piped}");
        let mut run = base_run.clone();
        let Decision::Deny(refusal) = decide(&context, &mut run, &action, Sensitivity::Boundary)
        else {
            panic!("the piped merge earned the integration exemption: {action:?}");
        };
        assert_eq!(
            refusal.code, "integration-shape-not-exact",
            "the piped merge was refused under a reason that is false for it: {refusal:?}"
        );
        assert!(
            refusal
                .message
                .contains("git -C <worktree> merge --no-edit origin/<base>"),
            "the refusal does not name the accepted literal: {}",
            refusal.message
        );

        let (action, how) = classify("Bash", &json!({"command": bare}));
        assert_eq!(how, Sensitivity::Boundary, "{bare}");
        let mut run = base_run.clone();
        let decision = decide(&context, &mut run, &action, Sensitivity::Boundary);
        assert!(
            matches!(decision, Decision::Allow(_)),
            "the bare integration was refused: {decision:?}"
        );
    });
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

    let proven = |repo_dir: &std::path::Path, target: &str| {
        is_safe_local_fast_forward_in(repo_dir, target, Vec::new())
    };

    assert!(proven(repo.path(), "origin/main"));
    assert!(proven(repo.path(), "refs/remotes/origin/main"));
    assert!(proven(repo.path(), &upstream));
    assert!(
        !proven(repo.path(), &tag_object),
        "an object ID that peeled to a different commit ID was accepted"
    );
    assert!(!proven(repo.path(), "origin/other"));

    std::fs::write(repo.path().join("untracked"), "dirty").expect("a dirty worktree");
    assert!(!proven(repo.path(), "origin/main"));
    std::fs::remove_file(repo.path().join("untracked")).expect("the worktree is clean again");

    git(&["checkout", "--detach", "--quiet"]);
    assert!(!proven(repo.path(), "origin/main"));
    git(&["checkout", "--quiet", &branch]);

    git(&["commit", "--allow-empty", "--quiet", "-m", "side"]);
    let side = git(&["rev-parse", "HEAD"]);
    git(&["reset", "--hard", "--quiet", &base]);
    assert!(
        !proven(repo.path(), &side),
        "a commit outside the upstream ancestry was accepted"
    );

    // The environment guard is the narrow list, not a `GIT_` prefix: the
    // editor, pager and askpass a host injects name programs, not locations,
    // and must not withdraw the exemption, while every name that redirects
    // where Git reaches must.
    let steered = |name: &str| {
        is_safe_local_fast_forward_in(
            repo.path(),
            "origin/main",
            vec![std::ffi::OsString::from(name)],
        )
    };
    for name in ["GIT_EDITOR", "git_editor", "GIT_PAGER", "GIT_ASKPASS"] {
        assert!(
            steered(name),
            "{name} names a program, not a location, and must not withdraw the fast-forward exemption"
        );
    }
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_SHALLOW_FILE",
        "GIT_REPLACE_REF_BASE",
        "BASH_ENV",
        "ENV",
        "BASH_FUNC_git%%",
    ] {
        for spelling in [name.to_ascii_lowercase(), name.to_ascii_uppercase()] {
            assert!(
                !steered(&spelling),
                "{spelling} could redirect which repository the merge reaches"
            );
        }
    }
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
fn an_integration_survives_an_editor_the_host_injected_and_not_a_redirected_repository() {
    // `GIT_EDITOR` is what this host injects into every shell. A program
    // name, not a location — it cannot change which branch a merge moves, so
    // neither exemption may refuse on it: the integration exemption and the
    // `--ff-only` one both use this list, and the `--ff-only` proof is driven
    // with these names in `local_fast_forward_proof_fails_closed_on_repository_state`.
    for name in ["GIT_EDITOR", "GIT_PAGER", "GIT_ASKPASS"] {
        assert!(
            integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
                name
            )]),
            "{name} names a program, not a location, and must not refuse the integration exemption"
        );
    }

    // The twenty names that do redirect where Git looks, each refused
    // lowercase and uppercase — a list, not a prefix, so every one of them
    // has to be named to be caught, and this list is not claimed complete:
    // it is drawn from the same class `GIT_DIR` is, not proved to exhaust
    // it. `GIT_CONFIG` and its four siblings are the environment spelling
    // of `git -c`, added because `core.worktree` and `core.hooksPath` steer
    // exactly as `GIT_DIR` does and `proof_git_command` strips every
    // `GIT_*` from the proof child, so the proof cannot see them either
    // way. `GIT_CONFIG_PARAMETERS` is how git itself passes `-c` values on
    // to the child processes it spawns. `GIT_SHALLOW_FILE` redirects which
    // commits a shallow clone can see, and `GIT_REPLACE_REF_BASE` redirects
    // where replace refs are read from — both can change what an object ID
    // this proof read resolves to without moving the ID itself.
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_INDEX_FILE",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
        "GIT_DISCOVERY_ACROSS_FILESYSTEM",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_SHALLOW_FILE",
        "GIT_REPLACE_REF_BASE",
        "BASH_ENV",
        "ENV",
        "BASH_FUNC_git%%",
    ] {
        assert!(
            !integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
                name.to_ascii_lowercase()
            )]),
            "{name} (lowercase) could redirect which repository the merge reaches"
        );
        assert!(
            !integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
                name.to_ascii_uppercase()
            )]),
            "{name} (uppercase) could redirect which repository the merge reaches"
        );
    }

    // `GIT_CONFIG_KEY_<n>` and `GIT_CONFIG_VALUE_<n>` are numbered and carry
    // no fixed suffix a list could name, so they are refused by prefix
    // instead — the one place this check reads one.
    for name in [
        "GIT_CONFIG_KEY_0",
        "GIT_CONFIG_VALUE_0",
        "GIT_CONFIG_KEY_12",
        "GIT_CONFIG_VALUE_12",
    ] {
        assert!(
            !integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
                name.to_ascii_lowercase()
            )]),
            "{name} (lowercase) could redirect which repository the merge reaches"
        );
        assert!(
            !integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
                name.to_ascii_uppercase()
            )]),
            "{name} (uppercase) could redirect which repository the merge reaches"
        );
    }

    // A name that merely starts with `GIT_CONFIG` but is not one of the five
    // exact names nor a numbered pair must not be refused by accident —
    // proving the prefix match above is narrow to the two numbered families
    // and not to the whole `GIT_CONFIG` word.
    assert!(
        integration_environment_does_not_redirect_the_repository([std::ffi::OsString::from(
            "GIT_CONFIGURATION_HINT"
        )]),
        "a name merely starting with GIT_CONFIG was refused as though it were the numbered family"
    );
}

#[test]
fn the_integration_refusal_never_names_the_state_it_says_to_move_to() {
    // `is_integration_in_the_runs_own_checkout` is evaluated twice — once at
    // the `gate` call site, once inside `out_of_phase` — each spawning its own
    // `git`, so the two calls are not guaranteed to agree. This drives
    // `out_of_phase` directly, bypassing the call site's own `state ==
    // "in-progress"` disjunct entirely, to prove the branch inside
    // `out_of_phase` cannot render its self-contradicting sentence even when
    // the second evaluation alone would have let it through.
    let repo = tempfile::tempdir().expect("a temporary repository");
    let git = |arguments: &[&str]| {
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
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);

    let mut run = Run::new("claude-abcd1234".to_owned());
    run.worktree = Some(repo.path().to_path_buf());

    let refuse = |state: &str| {
        out_of_phase(
            "git merge",
            state,
            &crate::identity::Identity::Number(12),
            crate::config::Integration::Branch,
            None,
            &run,
            repo.path(),
        )
    };

    let in_progress = refuse("in-progress");
    assert!(
        in_progress
            .as_ref()
            .is_none_or(|refusal| refusal.code != "integration-out-of-phase"),
        "in-progress answered integration-out-of-phase: {in_progress:?}"
    );

    for state in ["analysis", "ready", "blocked"] {
        let refusal = refuse(state).unwrap_or_else(|| panic!("{state} was not refused"));
        assert_eq!(
            refusal.code, "integration-out-of-phase",
            "{state} did not take the integration branch: {refusal:?}"
        );
        let text = format!("{} {}", refusal.message, refusal.resolution);
        assert!(
            !text.contains("is in in-progress"),
            "{state} named in-progress as the way out: {text}"
        );
        assert!(
            !text.contains("Publish"),
            "{state} fell through to the generic refusal's resolution: {text}"
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
    // The contract, and only the contract. That refusal was moved above
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
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
        target: crate::harness::CommitTarget::Checkout,
    };
    let decision = gate(&context, &mut run, &action, Sensitivity::Boundary);
    assert!(
        decision.denies(),
        "a boundary rode on a cached answer: {decision:?}"
    );
}

#[test]
fn a_write_to_the_repository_configuration_never_rides_the_window() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    std::fs::create_dir_all(&context.repo_dir).expect("a repository directory");
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let path = context.repo_dir.join(".git/estigia/estigia.local.md");
    let (action, how) = classify_with(
        "Edit",
        &json!({
            "file_path": path.display().to_string(),
            "old_string": "| Tracker | github |\n",
            "new_string": "| Tracker | linear |\n",
        }),
        &[],
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a write to the repository configuration was not classified as a boundary"
    );

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(12)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        gate(&context, &mut run, &action, how)
    });
    match decision {
        // The exact code `not_current_live_holder_issue` produces, not merely
        // "any code but the missing-contract one": `assert_ne!` there would
        // have admitted any other pre-window denial just as easily, proving
        // nothing about which road produced it. This is the refusal a
        // genuine `verify-claim` round trip gives for the script above, so
        // getting it is what shows the tracker was actually asked.
        Decision::Deny(refusal) => assert_eq!(
            refusal.code, "not-current-live-holder",
            "the write did not deny with the tracker's own answer, so this does not show the \
             window was bypassed: {refusal:?}"
        ),
        other => {
            panic!("a write to the repository configuration did not deny: {other:?}")
        }
    }
}

#[test]
fn an_edit_confined_to_a_config_row_is_a_boundary_too() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    std::fs::create_dir_all(&context.repo_dir).expect("a repository directory");
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let path = context.repo_dir.join(".git/estigia/estigia.local.md");
    let (action, how) = classify_with(
        "Edit",
        &json!({
            "file_path": path.display().to_string(),
            "old_string": "| Repair ceiling | 3 |",
            "new_string": "| Repair ceiling | 2 |",
        }),
        &[],
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "`Repair ceiling` alone was still read as routine — acceptance criterion 3's cost, \
         paid for by gating the whole file"
    );

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(12)]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        gate(&context, &mut run, &action, how)
    });
    match decision {
        Decision::Deny(refusal) => assert_eq!(
            refusal.code, "not-current-live-holder",
            "a `Repair ceiling` edit did not deny with the tracker's own answer: {refusal:?}"
        ),
        other => panic!("a `Repair ceiling` edit rode the renewal window: {other:?}"),
    }
}

#[test]
fn a_rate_limited_tracker_is_refused_by_name() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    std::fs::create_dir_all(&context.repo_dir).expect("a repository directory");
    let mut run = sworn(12, &context.repo_dir);

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[json!({
        "matches": "issue view",
        "stderr": "GraphQL: API rate limit already exceeded for user ID 1.",
        "status": 1,
    })]);
    let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        gate(&context, &mut run, &action, Sensitivity::Routine)
    });
    match decision {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "rate-limited", "{refusal:?}");
            assert!(
                refusal.message.contains("rate limit") && refusal.message.contains("unknown"),
                "the refusal names neither the limit nor that its reset is unknown: {}",
                refusal.message
            );
            assert_eq!(refusal.outcome, crate::outcome::MutationOutcome::NotStarted);
            assert_ne!(
                refusal.replay,
                crate::outcome::Replayability::ExactReplaySafe
            );
        }
        other => panic!("a rate-limited tracker did not refuse the write: {other:?}"),
    }
    assert_eq!(
        run.verified_at, None,
        "a rate limit was recorded as a verify"
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

/// `prefixed` is the one place every `Deny` `decide_observing` produces
/// funnels through, and calling it correctly is what a panel found nothing
/// in the tree actually measured: deleting its attachment left this whole
/// suite green. Calling `gate_observing` directly — the same production
/// entry point `harness::hook` uses — reaches `prefixed`'s own contribution
/// with no CLI-layer fallback sitting behind it to mask a broken one.
#[test]
fn a_denial_through_the_ordinary_claim_road_carries_what_it_was_given() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    let mut run = sworn(12, &context.repo_dir);

    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    let observed = crate::outcome::Observed {
        command: Some(crate::outcome::CommandObserved {
            matched: crate::outcome::Established::Known("a-boundary-fragment".to_owned()),
            against: crate::outcome::Established::Known("a-boundary-fragment ARG".to_owned()),
            literal: crate::outcome::Established::Known("a-boundary-fragment ARG".to_owned()),
            boundary_source: Some("irreversible"),
            disqualifying_byte: None,
            same_tier_fragments: Vec::new(),
            write_targets: Vec::new(),
        }),
        checkout: Some(crate::outcome::CheckoutObserved {
            adjudicated: context.repo_dir.to_string_lossy().into_owned(),
            source: crate::outcome::CheckoutSource::LaunchDirectory,
            dropped: Vec::new(),
        }),
        push_refs: Vec::new(),
    };
    match gate_observing(&context, &mut run, &action, Sensitivity::Routine, &observed) {
        Decision::Deny(refusal) => {
            assert_eq!(refusal.code, "control-surface-not-installed");
            let carried = refusal.observed.as_deref().unwrap_or_else(|| {
                panic!(
                    "the reading `gate_observing` was given was not attached to its own \
                     denial: {refusal}"
                )
            });
            assert_eq!(
                carried, &observed,
                "the attached reading does not match what `gate_observing` was given"
            );
        }
        other => panic!("an unreadable control surface allowed a write: {other:?}"),
    }
}

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
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    // Counted where the call now is. The gate stopped spawning the transport —
    // `tracker::invoke` is gone from this module entirely — and asks the port in
    // this process, so the structural property is counted against the dispatch.
    assert_eq!(
        source.matches("tracker::invoke(").count(),
        0,
        "the gate spawns the transport again"
    );
    // Three now, and all three reads. The second is
    // `tracker_answer_for_pointer` — the one question
    // `guard::adjudicate_action`'s reconciliation and `doctor`'s
    // `stale-run-pointer` row both ask, held to `gate`'s own shape rather than
    // written a third time: same `Context::live`, same `GH_REPO`, same
    // `verify-claim`. The third is `queue_read`, the one `list-state` read
    // `estigia goal` asks only when the run is not live, once per queue, so a
    // fourth call site *is* still the copy this file's own history keeps
    // finding; this test still holds the count to exactly the ones this module
    // owns.
    let calls = source.matches("dispatch::dispatch(").count();
    assert_eq!(
        calls, 3,
        "the gate grew a transport call this test does not know about"
    );
    assert!(
        source.contains("\"verify-claim\""),
        "the one call the gate makes is no longer the read it is allowed to make"
    );
    assert!(
        source.contains("\"list-state\""),
        "the claimable read `estigia goal` asks is no longer the list it is allowed to read"
    );
    let mutating = ["\"claim\"", "\"transition\"", "\"comment\"", "\"reclaim\""];
    for operation in mutating {
        assert!(
            !source.contains(operation),
            "the gate names the mutating operation {operation}"
        );
    }
}

/// A blocked item is dischargeable only when **every** clause it recorded
/// holds, and a queue of conditions nothing settles is not a discharged queue.
///
/// Mutation: count the blocked queue instead of its discharged items; treat a
/// clause that does not hold as one that does; drop a clause the read cannot
/// settle instead of reporting it as one that does not hold; or let one
/// unreadable clause fail the whole queue into `null`. Every one of the five
/// reddens this.
#[test]
fn dischargeable_work_counts_only_the_items_whose_every_clause_holds() {
    let root = tempfile::tempdir().expect("a root");
    let mut context = context(root.path());
    let repo_dir = root.path().join("checkout");
    std::fs::create_dir_all(&repo_dir).expect("the checkout the tracker reads in");
    context.repo_dir = repo_dir;
    context.tracker = crate::config::Tracker::Github {
        repo: Some(crate::config::RepoRef::parse("o/r").expect("a usable repo ref")),
    };

    let store_path = root.path().join("state.db");
    let repo = crate::transport::store::Repo {
        host: "github.com".to_owned(),
        owner: "o".to_owned(),
        name: "r".to_owned(),
    };
    let mut store =
        crate::transport::store::Store::open(&store_path).expect("the scratch store opens");
    let seed =
        |store: &mut crate::transport::store::Store, number: u64, state: &str, body: &str| {
            store
                .import(
                    &repo,
                    number,
                    &serde_json::json!({
                        "body": body,
                        "labels": [{ "name": format!("status:{state}") }],
                        "comments": [],
                    }),
                    "2026-01-01T00:00Z",
                )
                .expect("the issue imports");
        };
    seed(&mut store, 13, "ready", "A clause target that is ready.");
    seed(&mut store, 14, "done", "A clause target that is done.");
    seed(
        &mut store,
        15,
        "blocked",
        "<!-- issue-flow: blocked-until issue=14 state=done -->",
    );
    seed(
        &mut store,
        16,
        "blocked",
        "<!-- issue-flow: blocked-until issue=14 state=done -->\n\
         <!-- issue-flow: blocked-until issue=13 state=done -->",
    );
    seed(
        &mut store,
        17,
        "blocked",
        "a person's decision, recorded as prose and no clause",
    );
    seed(
        &mut store,
        18,
        "blocked",
        "<!-- issue-flow: blocked-until issue=14 state=done -->\n\
         <!-- issue-flow: blocked-until issue=14 -->",
    );
    seed(
        &mut store,
        19,
        "blocked",
        "<!-- issue-flow: blocked-until issue=99 state=done -->",
    );
    drop(store);

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[serde_json::json!({
        "matches": "issues?state=open",
        "stdout": serde_json::json!([[
            { "number": 15, "title": "one transition away", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" },
            { "number": 16, "title": "its condition still holds", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" },
            { "number": 17, "title": "waiting on a person", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" },
            { "number": 18, "title": "one clause a typo", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" },
            { "number": 19, "title": "a clause nobody can read", "labels": [],
              "created_at": "2026-01-01T00:00:00Z" }
        ]]).to_string(),
        "status": 0,
    })]);
    let counted = crate::transport::store::with_store_at(&store_path, || {
        crate::test_env::with_scripted_gh(bin.path(), &script, || {
            super::queue_read(
                &context.skill_root,
                &context.repo_dir,
                &context.tracker,
                "opencode-abc12345",
                "blocked",
            )
        })
    });
    let counted = counted.expect("the blocked queue was read");
    assert_eq!(
        counted["dischargeable"], 1,
        "only #15's recorded condition holds: #16 records one clause that holds and one that does \
         not, #17 recorded no clause at all, #18 carries a typo beside a clause that holds, and \
         #19's clause names an issue this read cannot resolve — and none of the four may turn the \
         queue's own count into a number it is not"
    );
    assert_eq!(
        counted["pending"], 2,
        "#16 holds one clause of two and #19's clause cannot be settled: both may still open by \
         themselves"
    );
    assert_eq!(
        counted["unconditioned"], 2,
        "#17 recorded no clause and #18 carries one that is not well formed: both are a person's \
         to settle"
    );
}

#[test]
fn a_window_of_zero_asks_every_time() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut context = context(root.path());
    context.window = Duration::from_secs(0);
    // With nothing installed this test passes without the window existing at
    // all: `control-surface-not-installed` answers above it now, and
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
    // `branch + worktree` is a transport operation that verifies the claim
    // before it makes the checkout. An agent typing `git worktree add` instead
    // has made the first write of a delivery with nothing checked.
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
    // No worktree recorded: none of these commands runs inside a checkout
    // this run's own pointer names, so the ordinary refusal applies even to
    // `git merge`.
    let run = Run::new("claude-abcd1234".to_owned());
    let repo_dir = Path::new("/repo");
    for command in DELIVERS {
        for state in ["analysis", "ready", "in-progress", "blocked"] {
            let refusal = out_of_phase(
                command,
                state,
                &crate::identity::Identity::Number(12),
                crate::config::Integration::Branch,
                None,
                &run,
                repo_dir,
            )
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
                out_of_phase(
                    command,
                    state,
                    &crate::identity::Identity::Number(12),
                    crate::config::Integration::Branch,
                    None,
                    &run,
                    repo_dir,
                )
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
    let run = Run::new("claude-abcd1234".to_owned());
    let repo_dir = Path::new("/repo");
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
                out_of_phase(
                    command,
                    state,
                    &crate::identity::Identity::Number(12),
                    crate::config::Integration::Branch,
                    None,
                    &run,
                    repo_dir,
                )
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
    let run = Run::new("claude-abcd1234".to_owned());
    let repo_dir = Path::new("/repo");
    for command in ["npm publish", "terraform apply", "make deploy"] {
        assert!(
            out_of_phase(
                command,
                "in-progress",
                &crate::identity::Identity::Number(12),
                crate::config::Integration::Branch,
                None,
                &run,
                repo_dir,
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
    let source = crate::source_text::code_of(include_str!("mod.rs"));
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
    run.issue = Some(crate::identity::Identity::Number(12));

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
    held.issue = Some(crate::identity::Identity::Number(12));
    held.state = Some("in-progress".to_owned());
    session::store(root.path(), &held).expect("the pointer writes");
    session::store(root.path(), &Run::new("claude-nothing0".to_owned()))
        .expect("the pointer writes");

    let holdings = session::holdings(root.path());
    assert_eq!(holdings.len(), 1);
    assert_eq!(holdings[0].run_id, "claude-holding0");
    assert_eq!(
        holdings[0].issue,
        Some(crate::identity::Identity::Number(12))
    );
}

#[test]
fn holdings_reads_the_quietest_run_first() {
    // The one most likely to be dead is the one worth asking the tracker about.
    let root = tempfile::tempdir().expect("a temporary root");
    let mut recent = Run::new("claude-recent00".to_owned());
    recent.issue = Some(crate::identity::Identity::Number(1));
    recent.mark_verified();
    let mut old = Run::new("claude-old00000".to_owned());
    old.issue = Some(crate::identity::Identity::Number(2));
    old.verified_at = recent.verified_at.map(|seconds| seconds - 9000);
    session::store(root.path(), &recent).expect("writes");
    session::store(root.path(), &old).expect("writes");

    let holdings = session::holdings(root.path());
    assert_eq!(
        holdings.first().map(|run| run.issue.clone()),
        Some(Some(crate::identity::Identity::Number(2)))
    );
    assert!(session::silence(&holdings[0]).seconds().unwrap_or(0) > 8000);
}

#[test]
fn a_machine_with_no_state_directory_reports_no_holdings() {
    let root = tempfile::tempdir().expect("a temporary root");
    assert!(session::holdings(&root.path().join("never-created")).is_empty());
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
    // pointer the filesystem refuses to read at all — a measured
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
    // on the machine where it was found — two byte-identical `SKILL.md`, one
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
        "| Setting | Value here |\n|---|---|\n| Model routing | developer=opus |\n",
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
        "| Setting | Value here |\n|---|---|\n| Delegated workers | developer |\n| Change size | 120 |\n",
    )
    .expect("the operator's own file");

    let canonical = discover_skill_root_in(&options).expect("a root");
    let (config, _) =
        crate::skill::installed_config_in_keeping_what_parses(&canonical, home.path());

    assert!(
        config.models.installs("developer"),
        "the gate is adjudicating against a default the operator overrode"
    );
    assert_eq!(config.models.for_target("developer"), Some("inherit"));
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
        "| Setting | Value here |\n|---|---|\n| Model routing | developer=opus |\n",
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

    session::record(
        &context.state_root,
        &serde_json::json!({"verdict": "allow"}),
    );
    assert_eq!(
        session::decision_records(&context.state_root).len(),
        1,
        "a decision left no record"
    );

    // Appended, not replaced: this is a history rather than a state.
    session::record(&context.state_root, &serde_json::json!({"verdict": "deny"}));
    let records = session::decision_records(&context.state_root);
    assert_eq!(
        records.len(),
        2,
        "the decision was replaced rather than appended"
    );
    assert!(
        records
            .iter()
            .any(|record| record.get("verdict").and_then(|value| value.as_str()) == Some("deny")),
        "the appended decision is not on the record"
    );
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
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
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
    let run = Run::new("claude-abcd1234".to_owned());
    let repo_dir = Path::new("/repo");
    let reason = |refusal: Option<Refusal>| refusal.map(|r| r.code);
    let landing = |integration, flag| {
        reason(out_of_phase(
            "gh pr merge",
            "in-progress",
            &crate::identity::Identity::Number(12),
            integration,
            flag,
            &run,
            repo_dir,
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
            reason(out_of_phase(
                "gh pr merge",
                "review",
                &crate::identity::Identity::Number(12),
                integration,
                None,
                &run,
                repo_dir,
            )),
            None,
            "{integration:?} refused a reviewed delivery"
        );
        assert_eq!(
            reason(out_of_phase(
                "git status",
                "in-progress",
                &crate::identity::Identity::Number(12),
                integration,
                None,
                &run,
                repo_dir,
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
    // whose calls Estigia can gate — and `status` reports every adapter it
    // cannot gate as contract only. "Every repository write from this run is
    // now measured against that claim" was therefore false wherever the agent
    // is one of those, and it was the sentence somebody reads at the moment
    // they swear. The same sentence was in the MCP tool description, where the
    // reader is the model.
    //
    // The same fault as the push guard's line, in the other message that
    // describes the oath. Pinned in both places so neither drifts back.
    let claim_command = crate::source_text::code_of(include_str!("../cli/mod.rs"));
    let mcp_tool = crate::source_text::code_of(include_str!("mcp/tools.rs"));
    for (what, source) in [
        ("the claim command", &claim_command),
        ("the MCP tool", &mcp_tool),
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
        claim_command.contains("a repository write the gate sees"),
        "the claim command stopped saying what the oath covers"
    );
    assert!(
        mcp_tool.contains("a repository write the gate sees"),
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
    let source = crate::source_text::code_of(include_str!("mod.rs"));
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

/// One spelling the parser answers to, and what it is a spelling of.
struct Answered {
    /// The words after `estigia` a command line would carry.
    spelling: String,
    /// The same command's canonical spelling, so an alias knows what it runs.
    canonical: String,
    /// Whether the command has no subcommands of its own.
    leaf: bool,
}

/// Every spelling the parser answers to, at every depth: each command's
/// canonical path and the same path with an alias in that command's own
/// position, so an alias on a parent (`config`) reaches the children under it.
fn answered_spellings(parser: &clap::Command) -> Vec<Answered> {
    fn walk(
        command: &clap::Command,
        canonical_prefix: &[String],
        spelled_prefix: &[String],
        found: &mut Vec<Answered>,
    ) {
        for subcommand in command.get_subcommands() {
            let canonical_name = subcommand.get_name().to_owned();
            let mut canonical_path = canonical_prefix.to_vec();
            canonical_path.push(canonical_name.clone());
            let canonical = canonical_path.join(" ");
            let leaf = subcommand.get_subcommands().next().is_none();
            let mut names = vec![canonical_name];
            names.extend(subcommand.get_all_aliases().map(str::to_owned));
            for name in names {
                let mut spelled_path = spelled_prefix.to_vec();
                spelled_path.push(name);
                found.push(Answered {
                    spelling: spelled_path.join(" "),
                    canonical: canonical.clone(),
                    leaf,
                });
                if !leaf {
                    walk(subcommand, &canonical_path, &spelled_path, found);
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(parser, &[], &[], &mut found);
    found
}

/// The words after `estigia` a listed fragment names, up to its first flag.
fn named_path(fragment: &str) -> Option<String> {
    fragment.strip_prefix("estigia ").map(|tail| {
        tail.split_whitespace()
            .take_while(|word| !word.starts_with('-'))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

#[test]
fn the_gate_measures_the_commands_that_switch_it_off() {
    use clap::CommandFactory;
    let parser = crate::cli::args::Cli::command();
    let answered = answered_spellings(&parser);
    assert!(
        answered.len() > 20,
        "the parser answered {} spellings, which is not this CLI",
        answered.len()
    );

    // Every listed subcommand is one the CLI has, and each reaches the
    // classifier as a boundary rather than as nothing.
    for line in super::DISARMS_THE_HARNESS {
        // Estigia's own are crossed against the command table, so a made-up
        // member is a guard about nothing. The rest are not this binary's
        // verbs at all — `core.hooksPath` points git away from every hook in
        // the repository and never says the word `estigia` — and what makes
        // one of those honest is the classification below, which every fragment
        // here is held to.
        if let Some(path) = named_path(line) {
            assert!(
                answered.iter().any(|name| name.spelling == path),
                "`{path}` is gated and is not a command this binary has"
            );
        }

        let (action, how) = crate::harness::classify("Bash", &serde_json::json!({"command": line}));
        if let Some(person_only) = crate::harness::person_only::COMMANDS
            .iter()
            .find(|command| line.ends_with(command.verb))
        {
            assert!(
                matches!(&action, crate::harness::Action::Write { target }
                    if target == person_only.target),
                "`{line}` is a person's command and reached the classifier as {action:?}"
            );
            continue;
        }
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
    // the CLI answers to for a listed command has to be listed too — at every
    // depth, not the top level alone. `setup` is declared with
    // `visible_aliases = ["install"]`, so `estigia install --uninstall` took
    // Estigia out and classified as `Untouched` — the same operation as
    // `estigia setup --uninstall`, under a name the product prints in its own
    // `--help`.
    //
    // Read out of the parser rather than out of the argument definitions,
    // because the parser is what answers a spelling at run time and it knows
    // which commands hang under which; a list of aliases written beside the
    // list of commands is the second spelling of one question and would drift
    // the moment somebody adds a third name.
    let mut checked = 0;
    for name in answered
        .iter()
        .filter(|name| name.spelling != name.canonical)
    {
        let canonical = format!("estigia {}", name.canonical);
        if !super::DISARMS_THE_HARNESS.contains(&canonical.as_str()) {
            continue;
        }
        checked += 1;
        let spelling = format!("estigia {}", name.spelling);
        assert!(
            super::DISARMS_THE_HARNESS.contains(&spelling.as_str()),
            "`{spelling}` runs `{canonical}`, which is gated, and is not itself gated"
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
    // The alphabet is really there: if the parser stopped answering aliases,
    // "every alias is covered" would be true and empty.
    assert!(
        checked >= 1,
        "only {checked} alias(es) of a gated command were found, which is fewer than \
         `cli/args.rs` declares \u{2014} this crossing has stopped reading them"
    );

    // And the direction the top-level walk could not ask, because it never
    // reached a subcommand: a leaf under a verb the list names is either gated
    // or declared a read. `estigia config edit` and `estigia config forget`
    // write what the listed `estigia config set` writes and matched no
    // population, so the gate answered `Untouched` — `Outside(NotWatched)`,
    // which is no gate at all. Walking the parser's tree is what makes the next
    // writing subcommand fail here rather than in a review.
    let mut under_a_gated_verb = 0;
    for name in answered.iter().filter(|name| name.leaf) {
        let mut path = name.canonical.split_whitespace();
        let canonical_verb = path.next().expect("a canonical spelling has a verb");
        // Only the leaves *below* the verb: `estigia guard` itself is a leaf
        // too, and installing the guard is not disarming it. What this walks is
        // the subcommand tree the top-level crossing could not reach.
        if path.next().is_none() {
            continue;
        }
        if !super::DISARMS_THE_HARNESS
            .iter()
            .any(|line| super::disarming_verb(line) == Some(canonical_verb))
        {
            continue;
        }
        under_a_gated_verb += 1;
        let spelling = format!("estigia {}", name.spelling);
        let gated = super::DISARMS_THE_HARNESS.contains(&spelling.as_str());
        let read = super::DISARM_READS.contains(&spelling.as_str());
        assert!(
            gated != read,
            "`{spelling}` is a leaf under `estigia {canonical_verb}`, which the disarm list \
             names, and is {} \u{2014} every leaf there is either gated or declared a read",
            if gated {
                "both gated and declared a read"
            } else {
                "neither"
            }
        );
        let (_, how) = crate::harness::classify("Bash", &serde_json::json!({"command": spelling}));
        let expected = if gated {
            crate::harness::Sensitivity::Boundary
        } else {
            crate::harness::Sensitivity::Routine
        };
        assert_eq!(
            how,
            expected,
            "`{spelling}` is declared {} and the gate reads it as {how:?}",
            if gated { "gated" } else { "a read" }
        );
    }
    // The floor for the walk above: if it stopped reaching subcommands, every
    // assertion in it would be true and empty.
    assert!(
        under_a_gated_verb >= 5,
        "only {under_a_gated_verb} leaf/leaves were found under a verb the list names, which is \
         fewer than `estigia config` has \u{2014} this walk has stopped reaching subcommands"
    );
    // And every declared read is a leaf this CLI really has: a made-up read
    // clears nothing.
    for read in super::DISARM_READS {
        assert!(
            answered
                .iter()
                .any(|name| name.leaf && format!("estigia {}", name.spelling) == *read),
            "`{read}` is declared a read and is not a leaf this CLI has"
        );
    }

    // Under a wrapper too, and with the absolute path a hook writes.
    for line in [
        "sudo estigia uninstall",
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
fn config_edit_and_config_forget_are_gated_where_config_set_is() {
    // `estigia config edit` and `estigia config forget` write what the listed
    // `estigia config set` writes — the installed contract, and a repository's
    // own rows — and neither matched any population, so the gate read them
    // `Untouched`: `Outside(NotWatched)`, which is no gate at all rather than a
    // weaker one. Measured through `estigia gate` before this change,
    // `estigia config edit` answered `no boundary matched this command`.
    let spellings = [
        "estigia config set \"Model routing\" developer=opus",
        "estigia config edit",
        "estigia config forget --repo /tmp/somewhere",
    ];
    for spelling in spellings {
        let (action, how) = classify("Bash", &json!({ "command": spelling }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{spelling}` reconfigures the harness and the gate reads it as {how:?}"
        );
        assert!(
            matches!(action, Action::Boundary { .. }),
            "`{spelling}` reached the classifier as {action:?}"
        );
    }

    // Over one tree state, through the gate itself: each reaches the tracker
    // and is denied by its answer, rather than standing aside as unwatched.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    std::fs::create_dir_all(&context.repo_dir).expect("a repository directory");
    let mut run = sworn(12, &context.repo_dir);
    run.mark_verified();

    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(12)]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        for spelling in spellings {
            let (action, how) = classify("Bash", &json!({ "command": spelling }));
            match gate(&context, &mut run.clone(), &action, how) {
                Decision::Deny(refusal) => assert_eq!(
                    refusal.code, "not-current-live-holder",
                    "`{spelling}` did not deny with the tracker's own answer, so this does not \
                     show the gate asked it: {refusal:?}"
                ),
                other => {
                    panic!("`{spelling}` reconfigures the harness and the gate answered {other:?}")
                }
            }
        }
    });
}

#[test]
fn a_global_option_before_the_estigia_verb_does_not_hide_a_disarm() {
    for spelling in [
        "estigia --json config edit",
        "estigia --json config set \"Model routing\" developer=opus",
        "estigia --json uninstall",
        "sudo estigia --json uninstall --all",
        r"C:\Users\me\.cargo\bin\estigia.exe --json guard --uninstall",
    ] {
        let (action, how) = classify("Bash", &json!({ "command": spelling }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{spelling}` disarms the harness with a global option in front and the gate reads it \
             as {how:?}"
        );
        assert!(
            matches!(action, Action::Boundary { .. }),
            "`{spelling}` reached the classifier as {action:?}"
        );
    }

    for spelling in ["estigia --json status", "estigia --json config list"] {
        let (_, how) = classify("Bash", &json!({ "command": spelling }));
        assert_eq!(
            how,
            Sensitivity::Routine,
            "`{spelling}` only reads and was gated as {how:?}"
        );
    }
}

#[test]
fn a_write_under_a_wrapper_is_still_a_write() {
    // Each listed wrapper reaches the command it runs, and a read under the
    // same wrapper stays untouched.
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
fn a_shell_write_to_the_repository_configuration_matches_the_write_road() {
    // A guarded row, named on the command line itself — `printf` is what
    // `writes_a_file` reads as a redirect, and the row text has to be
    // *visible on the line* for the shell road to read it at all.
    let shell = serde_json::json!({
        "command": "printf '| Tracker | linear |\\n' > .git/estigia/estigia.local.md"
    });
    let tool = serde_json::json!({
        "file_path": ".git/estigia/estigia.local.md",
        "old_string": "| Tracker | github |",
        "new_string": "| Tracker | linear |",
    });
    assert_eq!(
        classify_with("Bash", &shell, &[]).1,
        Sensitivity::Boundary,
        "a shell write naming `Tracker` did not raise to a boundary"
    );
    assert_eq!(
        classify_with("Bash", &shell, &[]).1,
        classify_with("Edit", &tool, &[]).1,
        "the shell road and the `Edit` road disagreed about the same guarded row"
    );

    // The fail-closed default is a boundary on both roads too.
    let opaque_shell =
        serde_json::json!({ "command": "cp secret.md .git/estigia/estigia.local.md" });
    let opaque_tool = serde_json::json!({ "file_path": ".git/estigia/estigia.local.md" });
    assert_eq!(
        classify_with("Bash", &opaque_shell, &[]).1,
        Sensitivity::Boundary,
        "a shell write naming no row at all was read as routine"
    );
    assert_eq!(
        classify_with("Edit", &opaque_tool, &[]).1,
        Sensitivity::Boundary,
        "the shell road and the `Edit` road disagreed about the fail-closed default"
    );

    // `Repair ceiling` alone is a boundary on both roads now, whichever tool
    // travels it — the two roads no longer disagree here the way they used
    // to before this fix.
    let board_shell = serde_json::json!({
        "command": "printf '| Repair ceiling | 2 |\\n' > .git/estigia/estigia.local.md"
    });
    let board_tool = serde_json::json!({
        "file_path": ".git/estigia/estigia.local.md",
        "old_string": "| Repair ceiling | 3 |",
        "new_string": "| Repair ceiling | 2 |",
    });
    assert_eq!(
        classify_with("Bash", &board_shell, &[]).1,
        Sensitivity::Boundary,
        "a shell write naming only `Repair ceiling` was not a boundary"
    );
    assert_eq!(
        classify_with("Edit", &board_tool, &[]).1,
        Sensitivity::Boundary,
        "an `Edit` naming only `Repair ceiling` was not a boundary"
    );

    // And a genuinely replacing `Write` agrees too.
    let replacing = serde_json::json!({
        "file_path": ".git/estigia/estigia.local.md",
        "content": "| Repair ceiling | 2 |\n",
    });
    assert_eq!(
        classify_with("Write", &replacing, &[]).1,
        Sensitivity::Boundary,
        "a replacing Write naming only `Repair ceiling` was not a boundary"
    );
}

#[test]
fn naming_the_repository_configuration_never_hides_a_control_surface_write() {
    let baseline = serde_json::json!({ "command": "rm ~/.claude/settings.json" });
    assert_eq!(
        classify_with("Bash", &baseline, &[]).1,
        Sensitivity::Boundary,
        "the baseline disarmament stopped being a boundary"
    );

    let with_mention = serde_json::json!({
        "command": "rm ~/.claude/settings.json; printf \"| Repair ceiling | 2 |\" > \
                     .git/estigia/estigia.local.md"
    });
    assert_eq!(
        classify_with("Bash", &with_mention, &[]).1,
        Sensitivity::Boundary,
        "appending a repository-configuration mention downgraded a control-surface write"
    );
}

#[test]
fn the_repository_configuration_is_seen_wherever_it_sits_on_the_line() {
    for command in [
        "printf x > .git/estigia/estigia.local.md 2>/dev/null",
        "printf x > .git/estigia/estigia.local.md && echo ok",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Boundary,
            "{command:?} did not raise to a boundary with the config path as a non-final token"
        );
    }
}

#[test]
fn a_value_cell_edit_naming_no_label_is_a_boundary() {
    let edit = |old: &str, new: &str| {
        classify(
            "Edit",
            &serde_json::json!({
                "file_path": ".git/estigia/estigia.local.md",
                "old_string": old,
                "new_string": new,
            }),
        )
    };

    let (_, one_line) = edit(
        "| github | github |",
        "| github evil/repo | github (renewed) |",
    );
    assert_eq!(
        one_line,
        Sensitivity::Boundary,
        "the one-line value-cell edit naming no label was not a boundary"
    );

    let (_, two_line) = edit("| github |\n| github |", "| github evil/repo |\n| github |");
    assert_eq!(
        two_line,
        Sensitivity::Boundary,
        "the two-line value-cell edit naming no label was not a boundary"
    );
}

#[test]
fn a_doubled_space_guarded_label_is_a_boundary() {
    let (_, how) = classify(
        "Edit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "| Repair ceiling | 3 |",
            "new_string": "| Evidence  standard | reading |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a doubled-space guarded label was not read as a boundary"
    );
}

#[test]
fn a_multiedit_decoy_pair_is_a_boundary() {
    let (_, how) = classify(
        "MultiEdit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "edits": [
                { "old_string": "reading", "new_string": "measuring" },
                {
                    "old_string": "| Repair ceiling | 3 |",
                    "new_string": "| Repair ceiling | 2 |",
                },
            ],
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a MultiEdit raising Evidence standard rode as routine behind a decoy board edit"
    );
}

#[test]
fn a_pipe_less_fragment_beside_a_resolvable_row_is_a_boundary() {
    let (_, how) = classify(
        "Edit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "measuring",
            "new_string": "reading\n| Repair ceiling | 3 |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "lowering Evidence standard rode as routine behind a decoy Repair ceiling line"
    );
}

#[test]
fn a_truncating_write_is_a_boundary() {
    for content in [
        "| Repair ceiling | 2 |\n",
        "",
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Repair ceiling | 2 | 3 |\n",
        "| Tracker | linear |\n",
        "just some bytes, no table row in here at all",
    ] {
        let (_, how) = classify(
            "Write",
            &serde_json::json!({ "file_path": ".git/estigia/estigia.local.md", "content": content }),
        );
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "a Write with content {content:?} was not a boundary"
        );
    }
}

#[test]
fn a_fence_marker_on_its_own_line_is_a_boundary() {
    let (_, how) = classify(
        "Edit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "| Repair ceiling | 2 | 3 |",
            "new_string": "<!-- estigia:config:start -->\n| Repair ceiling | 2 | 3 |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a fence marker on its own line beside an accountable row was not a boundary"
    );
}

#[test]
fn a_fence_marker_inside_a_row_line_is_a_boundary() {
    let cases: &[(&str, &str)] = &[
        (
            "| Repair ceiling | 2 | 3 |",
            "| Repair ceiling | 2 | 3 | <!-- estigia:config:start -->",
        ),
        (
            "| Repair ceiling | 3 |",
            "| Repair ceiling | 3 | <!-- estigia:config:end -->",
        ),
        (
            "| Repair ceiling | 3 |",
            "| Repair ceiling | 3 | <!-- issue-flow:config:end -->",
        ),
    ];
    for (old, new) in cases {
        let (_, how) = classify(
            "Edit",
            &json!({
                "file_path": ".git/estigia/estigia.local.md",
                "old_string": old,
                "new_string": new,
            }),
        );
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "a fence marker inside a row's own line rode as routine: old={old:?} new={new:?}"
        );
    }
}

#[test]
fn the_line_break_deletion_is_a_boundary() {
    let (_, how) = classify(
        "Edit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "| Change size | 800 | 800 |\n",
            "new_string": "| Change size | 800 | 800 |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "deleting the line break between two rows rode as routine"
    );
}

#[test]
fn a_fence_marker_assembled_at_the_junction_with_the_preserved_tail_is_a_boundary() {
    let (_, how) = classify(
        "Edit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "| Repair ceiling | 3 | <!-- estigia:conf",
            "new_string": "| Repair ceiling | 2 | <!-- estigia:conf",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a marker fragment left to be completed by the preserved tail rode as routine"
    );
}

#[test]
fn a_fence_marker_assembled_across_two_multiedit_entries_is_a_boundary() {
    let (_, how) = classify(
        "MultiEdit",
        &serde_json::json!({
            "file_path": ".git/estigia/estigia.local.md",
            "edits": [
                {
                    "old_string": "| Repair ceiling | 3 |",
                    "new_string": "| Repair ceiling | 3 | <!-- estigia:conf",
                },
                {
                    "old_string": "| Repair ceiling | 3 |",
                    "new_string": "| Repair ceiling | ig:end --> |",
                },
            ],
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a marker split across two MultiEdit entries rode as routine"
    );
}

#[test]
fn an_ordinary_third_cell_is_a_boundary_like_every_other_write_here() {
    let (_, how) = classify(
        "Edit",
        &json!({
            "file_path": ".git/estigia/estigia.local.md",
            "old_string": "| Repair ceiling | 3 |",
            "new_string": "| Repair ceiling | 3 | x |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "every write to the repository configuration is a boundary now, including an ordinary \
         third cell — see `an_edit_confined_to_a_config_row_is_a_boundary_too` for the \
         acceptance-criterion-3 cost this pays"
    );
}

#[test]
fn a_path_on_both_surfaces_stays_a_whole_file_boundary() {
    let path = "~/.estigia/estigia.local.md";
    assert!(
        super::is_control_surface(path),
        "the path stopped being a control surface"
    );
    assert!(
        super::is_repository_configuration(path),
        "the path stopped being a shape the repository-configuration branch would also answer"
    );

    let (_, how) = classify(
        "Edit",
        &json!({
            "file_path": path,
            "old_string": "| Repair ceiling | 3 |",
            "new_string": "| Repair ceiling | 2 |",
        }),
    );
    assert_eq!(
        how,
        Sensitivity::Boundary,
        "a path on both surfaces was not a whole-file boundary"
    );
}

#[test]
fn the_repository_configuration_is_recognised_under_a_submodule_and_a_separate_git_dir() {
    let root = tempfile::tempdir().expect("a temporary root");

    // A submodule: `<super>/.git/modules/<name>/…`.
    let module = root.path().join("parent/.git/modules/child");
    let child = root.path().join("parent/child");
    std::fs::create_dir_all(&module).expect("the submodule git directory");
    std::fs::create_dir_all(&child).expect("the submodule checkout");
    std::fs::write(
        child.join(".git"),
        format!("gitdir: {}\n", module.display()),
    )
    .expect("the submodule pointer");
    let submodule_path = crate::skill::repository_config_path(&child);
    assert!(
        super::is_repository_configuration(&submodule_path.display().to_string()),
        "a submodule-shaped path was not recognised: {}",
        submodule_path.display()
    );

    // A `--separate-git-dir` checkout: `<name>.git/…`, no `commondir`.
    let separate = root.path().join("elsewhere.git");
    let worktree = root.path().join("worktree");
    std::fs::create_dir_all(&separate).expect("the separate git directory");
    std::fs::create_dir_all(&worktree).expect("the checkout");
    std::fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", separate.display()),
    )
    .expect("the separate-git-dir pointer");
    let separate_path = crate::skill::repository_config_path(&worktree);
    assert!(
        super::is_repository_configuration(&separate_path.display().to_string()),
        "a --separate-git-dir-shaped path was not recognised: {}",
        separate_path.display()
    );
}

#[test]
fn the_repository_configuration_is_recognised_by_shape_not_by_exact_spelling() {
    let windows = r"H:\repo\.git\estigia\estigia.local.md";
    let posix = "/repo/.git/estigia/estigia.local.md";
    let dotted = "/repo/sub/../.git/estigia/estigia.local.md";
    let bare = ".git/estigia/estigia.local.md";
    for spelling in [windows, posix, dotted, bare] {
        assert!(
            super::is_repository_configuration(spelling),
            "{spelling} is the repository configuration and was not recognised"
        );
    }
    assert_eq!(
        classify("Write", &json!({ "file_path": windows })).1,
        classify("Write", &json!({ "file_path": posix })).1,
        "the Windows and POSIX spellings of the same file answered differently"
    );

    // A path that merely looks similar is not the file: a different name
    // under `.git/estigia/`, and the true filename sitting somewhere else
    // entirely.
    for lookalike in ["/repo/.git/estigia/notes.md", "/repo/estigia.local.md"] {
        assert!(
            !super::is_repository_configuration(lookalike),
            "{lookalike} was read as the repository configuration"
        );
    }

    // Already a `Boundary` today, through the skill-root derivation
    // `matches_a_fragment` already carries — asserted still true, and that
    // it is not this change's own branch answering it.
    let skill_root_path = "~/.agents/skills/issue-flow/estigia.local.md";
    assert!(
        !super::is_repository_configuration(skill_root_path),
        "the skill-root path was matched by the new repository-configuration branch"
    );
    assert!(
        super::is_control_surface(skill_root_path),
        "the skill-root path stopped being a boundary"
    );
}

#[test]
fn read_only_git_stash_subcommands_are_not_writes() {
    let is_write = |command: &str| {
        let input = serde_json::json!({ "command": command });
        matches!(
            classify_with("Bash", &input, &[]).0,
            Action::Write { .. } | Action::Boundary { .. }
        )
    };
    for command in [
        "git stash list",
        "git stash show -p stash@{0}",
        "git stash list | head -3",
        "git stash list; git stash show",
    ] {
        assert!(!is_write(command), "{command} was read as a write");
    }
    for command in [
        "git stash",
        "git stash push",
        "git stash pop",
        "git stash apply",
        "git stash drop",
        "git stash clear",
        "git stash listing",
        "git stash list; git stash pop",
        "git stash list && git stash drop",
    ] {
        assert!(is_write(command), "{command} was not read as a write");
    }
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
fn an_attached_api_method_is_read_as_a_method() {
    for command in [
        "gh api -XPUT repos/o/r/pulls/7/merge",
        "gh api -XDELETE repos/o/r/git/refs/heads/main",
        "gh api repos/o/r/pulls/7/merge --paginate -XPUT",
        "gh api --method=PATCH repos/o/r/issues/1",
        "gh api repos/o/r/issues -f t=x",
        "gh api repos/o/r/issues --input body.json",
        "gh api repos/o/r/issues --paginate|grep x;gh api -XPUT repos/o/r/pulls/7/merge",
        "(gh api -XPUT repos/o/r/pulls/7/merge)",
        "gh api -X GET a -f b=c; gh api -f t=x repos/o/r/issues",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Boundary { .. }
            ),
            "{command} reached the tracker unmeasured"
        );
    }
}

#[test]
fn a_flag_of_another_command_does_not_make_an_api_read_a_write() {
    for command in [
        "gh api repos/o/r/issues --paginate | grep -F foo",
        "gh api -X GET repos/o/r/issues -f state=open",
        "gh api -XGET repos/o/r/issues -f state=open",
        "gh api repos/o/r/issues; git log -f x",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).0,
            Action::Untouched,
            "{command} only reads"
        );
    }
}

#[test]
fn a_separator_inside_quotes_does_not_split_an_api_write() {
    for command in [
        "gh api \"repos/o/r/issues?per_page=1&page=2\" -f title=x",
        "gh api 'repos/o/r/issues?a=1;b=2' -f title=x",
        "gh api repos/o/r/issues -f 'title=a|b' -XPOST",
        "gh api repos/o/r/issues -f title=a\\&b",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Boundary { .. }
            ),
            "{command} reached the tracker unmeasured"
        );
    }
}

#[test]
fn issue_creation_is_read_per_command_and_through_quotes() {
    let names = |line: &str| {
        super::issue_creation::names_issue_creation(&super::without_leading_options(
            &line.to_ascii_lowercase(),
        ))
    };
    for line in [
        "gh api \"repos/o/r/issues?per_page=1&page=2\" -f title=x",
        "gh api repos/o/r/issues --method=post",
        "gh api repos/o/r/issues -x=post",
    ] {
        assert!(names(line), "`{line}` files an issue and was not named");
    }
    for line in [
        "gh api repos/o/r/issues --paginate | grep -F foo",
        "gh api repos/o/r/issues; git log -f x",
    ] {
        assert!(
            !names(line),
            "`{line}` does not file an issue and was named"
        );
    }
}

#[test]
fn a_substitution_inside_double_quotes_is_still_a_command_boundary() {
    for command in [
        "echo \"$(gh pr merge 7)\"",
        "echo \"`gh api -XPUT repos/o/r/pulls/7/merge`\"",
        "echo \"x $(gh api repos/o/r/issues -f title=x) y\"",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Boundary { .. }
            ),
            "{command} reached the tracker unmeasured"
        );
        assert!(
            super::operation_window::shell_commands(command)
                .iter()
                .any(|piece| piece.starts_with("gh ")),
            "{command} hid its substitution from the splitter"
        );
    }
}

#[test]
fn unbalanced_quotes_split_the_line_as_if_unquoted() {
    for command in [
        "echo it's; gh pr merge 7",
        "echo \"unterminated; gh api -XPUT repos/o/r/pulls/7/merge",
    ] {
        let input = serde_json::json!({ "command": command });
        assert!(
            matches!(
                classify_with("Bash", &input, &[]).0,
                Action::Boundary { .. }
            ),
            "{command} reached the tracker unmeasured"
        );
        assert!(
            super::operation_window::shell_commands(command)
                .iter()
                .any(|piece| piece.starts_with("gh ")),
            "{command} swallowed its tail into a quote"
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

    // Creating one is a write of its own: plain and porcelain creation are
    // gated, so the operation schedule can deny them outside its period.
    let creating = serde_json::json!({ "command": "git branch feat/x" });
    assert!(
        matches!(
            classify_with("Bash", &creating, &[]).0,
            Action::Write { .. }
        ),
        "plain branch creation was invisible to the gate"
    );
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

    // The repository's own configuration is the file gated for
    // writes, and it is deliberately not on the control surface — so the
    // neutralising verbs read it on their own, or the same line was a boundary
    // on the write road and `Untouched` here.
    for command in [
        "chmod 000 /repo/.git/estigia/estigia.local.md",
        "chattr -i /repo/.git/estigia/estigia.local.md",
        "chown nobody /repo/.git/estigia/estigia.local.md",
    ] {
        let input = serde_json::json!({ "command": command });
        assert_eq!(
            classify_with("Bash", &input, &[]).1,
            Sensitivity::Boundary,
            "{command} neutralised the repository configuration unmeasured"
        );
    }

    // And only together with a surface. A harness that gates `chmod +x
    // build.sh` is one nobody keeps on.
    for command in [
        "chmod +x build.sh",
        "chmod 644 src/main.rs",
        "chown me /tmp/x",
        "chmod 000 /repo/.git/estigia/notes.md",
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
fn a_neutralised_repository_configuration_is_a_boundary_the_gate_enforces() {
    // The classifier answer is half of it; the other half is that the gate acts
    // on it. A `chmod` on the repository's own configuration is the same file
    // gated for writes, and it must deny where a write does rather
    // than ride the renewal window because the verb is not a write this crate
    // spells.
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.skill_root).expect("a skill root");
    std::fs::write(
        context.skill_root.join(crate::skill::CONTRACT),
        "the contract this gate reads\n",
    )
    .expect("the contract is installed");
    std::fs::create_dir_all(&context.repo_dir).expect("a repository directory");
    let path = context.repo_dir.join(".git/estigia/estigia.local.md");

    for verb in ["chmod 000", "chattr -i", "chown nobody"] {
        let command = format!("{verb} {}", path.display());
        let (action, how) = classify_with("Bash", &json!({ "command": command }), &[]);
        assert_eq!(how, Sensitivity::Boundary, "{command} was read as routine");

        let mut run = sworn(12, &context.repo_dir);
        run.mark_verified();
        let bin = crate::test_env::scripted_gh();
        let script =
            crate::test_env::answers(&[crate::test_env::not_current_live_holder_issue(12)]);
        let decision = crate::test_env::with_scripted_gh(bin.path(), &script, || {
            gate(&context, &mut run, &action, how)
        });
        match decision {
            Decision::Deny(refusal) => assert_eq!(
                refusal.code, "not-current-live-holder",
                "{command} did not deny with the tracker's own answer, so this does not show the \
                 boundary was enforced: {refusal:?}"
            ),
            other => panic!("{command} rode the renewal window: {other:?}"),
        }
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
    run.issue = Some(crate::identity::Identity::Number(7));
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
        run.issue = Some(crate::identity::Identity::Number(7));
        run.worktree = Some(std::path::PathBuf::from("/w/7"));
    });
    assert_eq!(after.issue, Some(crate::identity::Identity::Number(7)));
    let read = session::load(root.path(), "claude-abcd1234");
    assert_eq!(
        read.issue,
        Some(crate::identity::Identity::Number(7)),
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
    held.issue = Some(crate::identity::Identity::Number(7));
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
    let next = session::update(root.path(), "claude-abcd1234", |run| {
        run.issue = Some(crate::identity::Identity::Number(8))
    });
    assert_eq!(next.issue, Some(crate::identity::Identity::Number(8)));
    assert_eq!(
        session::load(root.path(), "claude-abcd1234").issue,
        Some(crate::identity::Identity::Number(8)),
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
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    let reader_must_not_run =
        |_: u64| -> Result<String, crate::transport::Failure> { panic!("reader must not run") };
    let reviewed_pr_head =
        |_: u64| -> Result<String, crate::transport::Failure> { Ok(reviewed.clone()) };

    // Nothing has moved: the delivery is not this check's business.
    assert!(
        stale_verdict(
            &Action::Boundary {
                command: "gh pr merge".to_owned(),
                pr: Some(54),
                pr_unidentified_reason: None,
                local_fast_forward_target: None,
                is_exact_integration_merge: false,
                runs_in: crate::harness::RunsIn::Session,
                target: crate::harness::CommitTarget::Checkout,
            },
            &run,
            repo.path(),
            &reviewed_pr_head,
        )
        .is_none(),
        "a delivery on the head that was reviewed was refused"
    );

    // A run with nothing published is left exactly as it was.
    let mut unpublished = run.clone();
    unpublished.review_transaction = None;
    let missing = stale_verdict(
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        &unpublished,
        repo.path(),
        &reader_must_not_run,
    )
    .expect("a PR merge spent no complete receipt");
    assert_eq!(missing.code, "complete-review-transaction-missing");

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
                    pr_unidentified_reason: None,
                    local_fast_forward_target: None,
                    is_exact_integration_merge: false,
                    runs_in: crate::harness::RunsIn::Session,
                    target: crate::harness::CommitTarget::Checkout,
                },
                &run,
                repo.path(),
                &reader_must_not_run,
            )
            .is_none(),
            "`{allowed}` was refused, which is how a run fixes what a review found"
        );
    }

    let moved_pr_head = |_: u64| -> Result<String, crate::transport::Failure> { Ok(now.clone()) };
    for delivery in ["gh pr merge", "git merge"] {
        let refusal = stale_verdict(
            &Action::Boundary {
                command: delivery.to_owned(),
                pr: (delivery == "gh pr merge").then_some(54),
                pr_unidentified_reason: None,
                local_fast_forward_target: None,
                is_exact_integration_merge: false,
                runs_in: crate::harness::RunsIn::Session,
                target: crate::harness::CommitTarget::Checkout,
            },
            &run,
            repo.path(),
            &moved_pr_head,
        )
        .unwrap_or_else(|| panic!("`{delivery}` delivered on a verdict bound to other bytes"));
        assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
        assert!(
            refusal.message.contains(&reviewed[..7]) && refusal.message.contains(&now[..7]),
            "the refusal does not name both heads: {}",
            refusal.message
        );
    }

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
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        &run,
        &unrelated,
        &reader_must_not_run,
    )
    .expect("an unrelated clone spent this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-repository");
    assert_eq!(refusal.class, crate::outcome::RefusalClass::Refused);
    assert!(refusal.message.contains(&unrelated.display().to_string()));
    assert!(
        !refusal.message.contains("HEAD"),
        "the refusal named the invoking checkout's HEAD: {}",
        refusal.message
    );

    let unreadable = repo.path().join("not-a-checkout");
    std::fs::create_dir(&unreadable).expect("an ordinary directory");
    std::fs::write(unreadable.join(".git"), "not a gitdir pointer")
        .expect("an unreadable nested checkout marker");
    let refusal = stale_verdict(
        &Action::Boundary {
            command: "git merge".to_owned(),
            pr: None,
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        &run,
        &unreadable,
        &reader_must_not_run,
    )
    .expect("an unreadable head spent this run's verdict");
    assert_eq!(refusal.code, "review-head-unreadable");
    assert!(
        refusal.message.contains("cannot be read")
            && refusal.message.contains(&unreadable.display().to_string())
    );

    let refusal = stale_verdict(
        &Action::Boundary {
            command: "gh pr merge".to_owned(),
            pr: Some(54),
            pr_unidentified_reason: None,
            local_fast_forward_target: None,
            is_exact_integration_merge: false,
            runs_in: crate::harness::RunsIn::Session,
            target: crate::harness::CommitTarget::Checkout,
        },
        &run,
        &unreadable,
        &reader_must_not_run,
    )
    .expect("an unreadable checkout spent this run's verdict");
    assert_eq!(refusal.code, "review-checkout-unreadable");
    assert_eq!(refusal.class, crate::outcome::RefusalClass::Unavailable);
    assert!(refusal.message.contains(&unreadable.display().to_string()));
}

struct IntegrationRig {
    _parent: tempfile::TempDir,
    primary: std::path::PathBuf,
    topic: std::path::PathBuf,
    run: Run,
}

fn integration_rig() -> IntegrationRig {
    let parent = tempfile::tempdir().expect("a parent for both checkouts");
    let primary = parent.path().join("primary");
    let topic = parent.path().join("topic~claude-abcd1234");
    std::fs::create_dir(&primary).expect("the primary checkout directory");
    let git = |directory: &std::path::Path, args: &[&str]| {
        let answer = std::process::Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            answer.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&answer.stderr)
        );
        String::from_utf8_lossy(&answer.stdout).trim().to_owned()
    };
    git(&primary, &["init", "-q", "-b", "main"]);
    std::fs::write(primary.join("a.txt"), "one").expect("a file");
    git(&primary, &["add", "-A"]);
    git(&primary, &["commit", "-qm", "one"]);
    let reviewed = git(&primary, &["rev-parse", "HEAD"]);
    git(
        &primary,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "topic",
            topic.to_str().expect("utf-8 path"),
        ],
    );
    std::fs::write(primary.join("a.txt"), "two").expect("a change");
    git(&primary, &["commit", "-qam", "two"]);
    assert_ne!(git(&primary, &["rev-parse", "HEAD"]), reviewed);

    let mut run = Run::new("claude-abcd1234".to_owned());
    run.repo_dir = Some(primary.clone());
    run.worktree = Some(topic.clone());
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed,
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    IntegrationRig {
        _parent: parent,
        primary,
        topic,
        run,
    }
}

fn verdict_for(rig: &IntegrationRig, command: &str) -> Option<crate::outcome::Refusal> {
    let (action, _) = classify("Bash", &serde_json::json!({ "command": command }));
    stale_verdict(&action, &rig.run, &rig.primary, &|_| {
        panic!("a git merge never asks for the PR head")
    })
}

#[test]
fn an_integrating_merge_in_the_runs_own_checkout_spends_no_verdict() {
    let rig = integration_rig();
    let topic = crate::test_env::shell_path(&rig.topic);
    for command in [
        format!("cd {topic} && git merge origin/main"),
        format!("cd {topic} && git merge --no-edit origin/main"),
        format!("cd '{topic}' && git merge origin/main"),
        format!("git -C {topic} merge --no-edit origin/main"),
    ] {
        assert!(
            verdict_for(&rig, &command).is_none(),
            "`{command}` was refused although it integrates the run's own checkout"
        );
    }
}

#[test]
fn a_merge_outside_the_runs_own_checkout_is_still_a_delivery() {
    let rig = integration_rig();
    let primary = crate::test_env::shell_path(&rig.primary);
    for command in [
        "git merge origin/main".to_owned(),
        format!("cd {primary} && git merge origin/main"),
        format!("git -C {primary} merge origin/main"),
    ] {
        let refusal = verdict_for(&rig, &command)
            .unwrap_or_else(|| panic!("`{command}` delivered on a verdict bound to other bytes"));
        assert_eq!(refusal.code, "verdict-bound-to-other-bytes", "{command}");
    }
}

#[test]
fn a_checkout_of_another_repository_is_refused_as_such_on_every_delivery_path() {
    use crate::outcome::RefusalClass;
    let rig = integration_rig();
    let other = tempfile::tempdir().expect("another repository");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(other.path())
            .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
            .args(args)
            .output()
            .expect("git runs")
    };
    assert!(git(&["init", "-q"]).status.success());
    assert!(
        git(&["commit", "-q", "--allow-empty", "-m", "x"])
            .status
            .success()
    );
    let (action, _) = classify("Bash", &json!({ "command": "git merge feature" }));
    let refusal = stale_verdict(&action, &rig.run, other.path(), &|_| {
        panic!("a merge never asks for the PR head")
    })
    .expect("another repository is refused");
    assert_eq!(refusal.code, "verdict-bound-to-other-repository");
    assert_eq!(refusal.class, RefusalClass::Refused);
}

#[test]
fn an_unreadable_run_record_is_classed_unavailable_on_the_text_an_agent_reads() {
    let root = tempfile::tempdir().expect("a temporary root");
    let context = context(root.path());
    std::fs::create_dir_all(&context.state_root).expect("the state directory");
    std::fs::write(
        context.state_root.join("claude-corrupt1.json"),
        "{\"run_id\": \"claude-corrupt1\", \"issue\":",
    )
    .expect("plant a half-written pointer");
    let mut run = session::load(&context.state_root, "claude-corrupt1");
    let action = Action::Write {
        target: "src/x.rs".to_owned(),
    };
    let Decision::Deny(refusal) = gate(&context, &mut run, &action, Sensitivity::Routine) else {
        panic!("an unreadable record was not refused");
    };
    assert_eq!(refusal.code, "run-pointer-unreadable");
    assert_eq!(refusal.class, crate::outcome::RefusalClass::Unavailable);
    assert!(refusal.agent_text("call").contains("Class: unavailable"));
}

#[test]
fn a_class_of_unknown_never_pairs_with_a_nothing_was_written_line() {
    let refusal = crate::outcome::Refusal::not_started(
        "a-code",
        "m",
        crate::outcome::Resolution::run("estigia status"),
    )
    .classed(crate::outcome::RefusalClass::Unknown);
    assert_eq!(refusal.outcome, crate::outcome::MutationOutcome::Unknown);
    let text = refusal.agent_text("call");
    assert!(text.contains("Class: unknown"));
    assert!(!text.contains("nothing was written"), "{text}");
}

#[test]
fn a_head_that_moved_is_stale_and_one_that_cannot_be_read_is_unavailable() {
    use crate::outcome::RefusalClass;
    let rig = integration_rig();
    let moved = verdict_for(&rig, "git merge origin/main").expect("a moved head is refused");
    assert_eq!(moved.code, "verdict-bound-to-other-bytes");
    assert_eq!(moved.class, RefusalClass::Stale);

    let elsewhere = tempfile::tempdir().expect("a directory that is no repository");
    let (action, _) = classify("Bash", &json!({ "command": "git merge origin/main" }));
    let unreadable = stale_verdict(&action, &rig.run, elsewhere.path(), &|_| {
        panic!("a merge never asks for the PR head")
    })
    .expect("an unreadable head is refused");
    assert_eq!(
        unreadable.code, "review-head-unreadable",
        "an unreadable head was reported as `{}`, a verdict bound to other bytes",
        unreadable.code
    );
    assert_eq!(unreadable.class, RefusalClass::Unavailable);
    assert_ne!(unreadable.class, moved.class);

    for refusal in [
        &moved,
        &unreadable,
        &delivery_pr_unidentified(None),
        &complete_review_transaction_missing(1),
        &delivery_pr_mismatch(1, 2),
        &complete_review_transaction_not_selected(),
    ] {
        assert_eq!(
            super::refusal_class::class_of(refusal.code),
            Some(refusal.class),
            "`{}` is raised with a class the table does not hold for it",
            refusal.code
        );
    }
}

#[test]
fn a_merge_whose_directory_cannot_be_read_names_that_and_not_the_session() {
    let rig = integration_rig();
    for command in [
        "cd $WORKTREE && git merge origin/main",
        "cd \"$WORKTREE\" && git merge origin/main",
        "git -C \"$WORKTREE\" merge origin/main",
        "cd ~/topic && git merge origin/main",
        "cd a && cd b && git merge origin/main",
        "(cd somewhere; git merge origin/main)",
    ] {
        let refusal = verdict_for(&rig, command).unwrap_or_else(|| {
            panic!("`{command}` was judged against a directory it may not run in")
        });
        assert_eq!(refusal.code, "review-checkout-unreadable", "{command}");
        assert_eq!(
            refusal.class,
            crate::outcome::RefusalClass::Unavailable,
            "{command}"
        );
        assert!(
            refusal
                .message
                .contains("cannot tell which checkout it runs in"),
            "{command}: {}",
            refusal.message
        );
        assert!(
            !refusal.message.contains(&rig.primary.display().to_string()),
            "{command} named the session directory: {}",
            refusal.message
        );
    }
}

struct DeliveredRig {
    _parent: tempfile::TempDir,
    primary: std::path::PathBuf,
    reviewed_worktree: std::path::PathBuf,
    stale_worktree: std::path::PathBuf,
    merge: String,
    stranger: String,
    run: Run,
}

fn delivered_rig() -> DeliveredRig {
    let parent = tempfile::tempdir().expect("a parent for the checkouts");
    let primary = parent.path().join("primary");
    let reviewed_worktree = parent.path().join("reviewed");
    let stale_worktree = parent.path().join("stale");
    std::fs::create_dir(&primary).expect("the primary checkout directory");
    let git = |directory: &std::path::Path, args: &[&str]| {
        let answer = std::process::Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(["-c", "user.email=t@example.invalid", "-c", "user.name=t"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            answer.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&answer.stderr)
        );
        String::from_utf8_lossy(&answer.stdout).trim().to_owned()
    };
    let commit = |directory: &std::path::Path, file: &str, text: &str| {
        std::fs::write(directory.join(file), text).expect("a file");
        git(directory, &["add", "-A"]);
        git(directory, &["commit", "-qm", text]);
        git(directory, &["rev-parse", "HEAD"])
    };
    git(&primary, &["init", "-q", "-b", "main"]);
    let base = commit(&primary, "a.txt", "base");
    git(
        &primary,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            stale_worktree.to_str().expect("utf-8 path"),
        ],
    );
    git(
        &primary,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "topic",
            reviewed_worktree.to_str().expect("utf-8 path"),
        ],
    );
    let head = commit(&reviewed_worktree, "b.txt", "reviewed");
    commit(&primary, "c.txt", "base moved");
    git(
        &primary,
        &["merge", "-q", "--no-ff", "-m", "merge", "topic"],
    );
    let merge = git(&primary, &["rev-parse", "HEAD"]);
    assert_eq!(git(&primary, &["rev-parse", "HEAD^2"]), head);
    git(&primary, &["branch", "other", &base]);
    git(&primary, &["checkout", "-q", "other"]);
    commit(&primary, "d.txt", "elsewhere");
    git(&primary, &["checkout", "-q", "-b", "stranger-side", &base]);
    let side = commit(&primary, "e.txt", "side");
    git(&primary, &["checkout", "-q", "main"]);
    git(
        &primary,
        &["merge", "-q", "--no-ff", "-m", "stranger", &side],
    );
    let stranger = git(&primary, &["rev-parse", "HEAD"]);
    git(&primary, &["reset", "-q", "--hard", &merge]);

    let mut run = Run::new("claude-abcd1234".to_owned());
    run.repo_dir = Some(primary.clone());
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head,
        base,
        digest: "c".repeat(64),
    });
    DeliveredRig {
        _parent: parent,
        primary,
        reviewed_worktree,
        stale_worktree,
        merge,
        stranger,
        run,
    }
}

fn delivered_verdict(
    rig: &DeliveredRig,
    session: &std::path::Path,
    command: &str,
) -> Option<crate::outcome::Refusal> {
    let (action, _) = classify("Bash", &json!({ "command": command }));
    stale_verdict(&action, &rig.run, session, &|_| {
        panic!("a git command never asks for the PR head")
    })
}

#[test]
fn a_tag_of_the_runs_own_merge_commit_is_not_a_stale_verdict() {
    let rig = delivered_rig();
    for command in [
        format!("git tag -a v1.0 -F notes.txt {}", rig.merge),
        format!("git tag -a \"v1.0\" -F \"notes.txt\" \"{}\"", rig.merge),
        "git tag -a v1.0 -F notes.txt".to_owned(),
        "git tag v1.0 main".to_owned(),
    ] {
        assert!(
            delivered_verdict(&rig, &rig.primary, &command).is_none(),
            "`{command}` tags the merge of the reviewed head from the checkout that is at it"
        );
    }
    for command in [
        format!(
            "git -C {} tag -a v1.0 -F notes.txt {}",
            crate::test_env::shell_path(&rig.reviewed_worktree),
            rig.merge
        ),
        format!(
            "git -C {} tag -a v1.0 -F notes.txt",
            crate::test_env::shell_path(&rig.reviewed_worktree)
        ),
        format!(
            "cd {} && git tag -a v1.0 {}",
            crate::test_env::shell_path(&rig.reviewed_worktree),
            rig.merge
        ),
    ] {
        assert!(
            delivered_verdict(&rig, &rig.primary, &command).is_none(),
            "`{command}` was judged against the session's checkout, not the directory it names"
        );
    }
}

#[test]
fn a_tag_of_any_other_commit_is_still_a_stale_verdict() {
    let rig = delivered_rig();
    let mut moved_base = rig.run.clone();
    moved_base
        .review_transaction
        .as_mut()
        .expect("a receipt")
        .base = rig.stranger.clone();
    let cases = [
        (rig.run.clone(), format!("git tag v1.0 {}", rig.stranger)),
        (rig.run.clone(), "git tag v1.0 other".to_owned()),
        (rig.run.clone(), "git tag v1.0 HEAD~1".to_owned()),
        (
            rig.run.clone(),
            format!(
                "git -C {} tag v1.0 {}",
                crate::test_env::shell_path(&rig.primary),
                rig.stranger
            ),
        ),
        (moved_base, format!("git tag v1.0 {}", rig.merge)),
    ];
    for (run, command) in cases {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        let refusal = stale_verdict(&action, &run, &rig.primary, &|_| {
            panic!("a git command never asks for the PR head")
        })
        .unwrap_or_else(|| panic!("`{command}` tagged a commit that is not the run's merge"));
        assert_eq!(refusal.code, "verdict-bound-to-other-bytes", "{command}");
    }
    let at_stranger = {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&rig.primary)
            .args(["reset", "-q", "--hard", &rig.stranger])
            .output()
            .expect("git runs");
        delivered_verdict(&rig, &rig.primary, "git tag v1.0")
    };
    assert_eq!(
        at_stranger
            .expect("a merge of another head was accepted")
            .code,
        "verdict-bound-to-other-bytes"
    );
}

#[test]
fn a_fast_forward_to_the_runs_own_merge_is_not_a_stale_verdict() {
    let rig = delivered_rig();
    assert!(
        delivered_verdict(&rig, &rig.stale_worktree, "git merge --ff-only main").is_none(),
        "a fast-forward whose new head is the merge of the reviewed head was refused"
    );
    let refusal = delivered_verdict(&rig, &rig.stale_worktree, "git merge --ff-only other")
        .expect("a fast-forward to a commit that is not the run's merge was accepted");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
}

#[test]
fn a_tag_names_a_commit_only_when_the_line_is_plain_words() {
    use super::CommitTarget::{Checkout, Named, Unread};
    let target = |command: &str| {
        let (action, _) = classify("Bash", &json!({ "command": command }));
        match action {
            Action::Boundary { target, .. } => target,
            other => panic!("`{command}` was read as {other:?}"),
        }
    };
    for (command, expected) in [
        ("git tag -a v1 -F notes.txt abc123", Named("abc123".into())),
        ("git tag -am 'x y' v1 abc123", Unread),
        ("git tag -am x v1 abc123", Named("abc123".into())),
        ("git tag --message x v1 main", Named("main".into())),
        ("git --no-pager tag v1 main", Named("main".into())),
        (
            "git tag -a \"v1\" -F \"n.txt\" \"abc123\"",
            Named("abc123".into()),
        ),
        ("git tag -a v1", Checkout),
        ("git tag -a v1 --", Checkout),
        ("git tag v1 a b", Unread),
        ("git tag v1 abc123 && git push", Unread),
        ("git tag v1 $(git rev-parse HEAD~1)", Unread),
        ("git tag v1 --contains abc123", Unread),
        ("git tag -a v1 -F", Unread),
        (
            "git merge --ff-only origin/main",
            Named("origin/main".into()),
        ),
        ("git merge origin/main", Checkout),
        ("gh release create v1", Checkout),
    ] {
        assert_eq!(target(command), expected, "{command}");
    }
}

#[test]
fn a_git_command_with_a_named_directory_is_judged_against_that_directory() {
    let rig = delivered_rig();
    let refusal = delivered_verdict(
        &rig,
        &rig.reviewed_worktree,
        &format!(
            "git -C {} tag v1.0",
            crate::test_env::shell_path(&rig.stale_worktree)
        ),
    )
    .expect("the session's checkout at the reviewed head spent a verdict for another directory");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
    assert!(
        refusal
            .message
            .contains(&crate::test_env::shell_path(&rig.stale_worktree)),
        "{}",
        refusal.message
    );
}

#[test]
fn a_tag_whose_directory_cannot_be_read_names_that_and_not_the_session() {
    let rig = delivered_rig();
    for command in [
        "cd $WORKTREE && git tag v1.0",
        "git -C \"$WORKTREE\" tag v1.0",
        "git -C ~/topic tag -a v1.0",
        "cd a && cd b && git tag v1.0",
        "(cd somewhere; git tag v1.0)",
    ] {
        let refusal = delivered_verdict(&rig, &rig.primary, command).unwrap_or_else(|| {
            panic!("`{command}` was judged against a directory it may not run in")
        });
        assert_eq!(refusal.code, "review-checkout-unreadable", "{command}");
        assert!(
            !refusal.message.contains(&rig.primary.display().to_string()),
            "{command} named the session directory: {}",
            refusal.message
        );
    }
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
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    let delivery = Action::Boundary {
        command: "gh pr merge".to_owned(),
        pr: Some(54),
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
        target: crate::harness::CommitTarget::Checkout,
    };
    let reviewed_pr_head =
        |_: u64| -> Result<String, crate::transport::Failure> { Ok(reviewed.clone()) };
    assert!(
        stale_verdict(&delivery, &run, &repo, &reviewed_pr_head).is_none(),
        "a steered environment refused a delivery on the checkout that was reviewed"
    );
    let refusal = stale_verdict(&delivery, &run, &unrelated, &|_: u64| -> Result<
        String,
        crate::transport::Failure,
    > {
        panic!("reader must not run for an unrelated clone")
    })
    .expect("a steered environment let an unrelated clone spend this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-repository");
    assert!(refusal.message.contains(&unrelated.display().to_string()));
}

#[test]
fn a_gh_pr_merge_is_bound_to_the_pr_head_the_tracker_reports_not_the_invoking_checkout() {
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
    let base = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert!(!base.is_empty(), "the fixture has no head");

    std::fs::write(repo.path().join("a.txt"), "two").expect("a second commit");
    git(&["add", "-A"]);
    git(&["commit", "-qm", "two"]);
    let reviewed = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert_ne!(reviewed, base, "the fixture did not advance past the base");

    let checked_out = git(&["checkout", "-q", &base]);
    assert!(
        checked_out.status.success(),
        "the fixture could not check out the base commit"
    );
    let checkout_head = String::from_utf8_lossy(&git(&["rev-parse", "HEAD"]).stdout)
        .trim()
        .to_owned();
    assert_ne!(
        checkout_head, reviewed,
        "the invoking checkout matches the reviewed head"
    );

    let mut run = Run::new("claude-abcd1234".to_owned());
    run.repo_dir = Some(repo.path().to_path_buf());
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: reviewed.clone(),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    let delivery = Action::Boundary {
        command: "gh pr merge".to_owned(),
        pr: Some(54),
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
        target: crate::harness::CommitTarget::Checkout,
    };

    let matches_reviewed =
        |_: u64| -> Result<String, crate::transport::Failure> { Ok(reviewed.clone()) };
    assert!(
        stale_verdict(&delivery, &run, repo.path(), &matches_reviewed).is_none(),
        "a PR head matching the reviewed one was refused because the invoking checkout had moved"
    );

    let moved = "d".repeat(40);
    let moved_pr_head = |_: u64| -> Result<String, crate::transport::Failure> { Ok(moved.clone()) };
    let refusal = stale_verdict(&delivery, &run, repo.path(), &moved_pr_head)
        .expect("a moved PR head spent this run's verdict");
    assert_eq!(refusal.code, "verdict-bound-to-other-bytes");
    assert!(refusal.message.contains(&reviewed[..7]));
    assert!(refusal.message.contains(&moved[..7]));
    assert!(
        !refusal.message.contains(&checkout_head[..7]),
        "the refusal named the invoking checkout's HEAD: {}",
        refusal.message
    );

    let unreadable = |_: u64| -> Result<String, crate::transport::Failure> {
        Err(crate::transport::Failure::Read(
            "gh pr view failed".to_owned(),
        ))
    };
    let refusal = stale_verdict(&delivery, &run, repo.path(), &unreadable)
        .expect("an unreadable PR head spent this run's verdict");
    assert_eq!(refusal.code, "read-failed");
    assert!(!refusal.message.contains("other-bytes"));

    let timed_out = |_: u64| -> Result<String, crate::transport::Failure> {
        Err(crate::transport::Failure::Timeout(
            "gh pr view did not answer in time".to_owned(),
        ))
    };
    let refusal = stale_verdict(&delivery, &run, repo.path(), &timed_out)
        .expect("a non-Read transport failure spent this run's verdict");
    assert_eq!(refusal.code, "ambiguous-write");
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
    // to a `pre-push` hook on standard input, for free. The local half of it is
    // now read before `adjudicate_action` is called, resolving
    // *which run's claim* a push belongs to when several hold one checkout —
    // but only to select **who**. The action `adjudicate_action` is handed is
    // still the same three words, so both doors still see the same *what*.
    //
    // For an adapter that gets the contract and no tool gate, this hook is the
    // only enforcement there is.
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
    // ref. Pinned on `decide`'s own body, not the arm's — the arm no longer
    // spells `Action::Boundary` itself, it calls `decide`, and `decide` is the
    // one place left that names the action a push is adjudicated as.
    let source = crate::source_text::code_of(include_str!("guard.rs"));
    let decide_start = source
        .find("pub fn decide_pushing(")
        .expect("decide is still a function in this file");
    let decide_end = source[decide_start..]
        .find("\n}\n")
        .map(|offset| decide_start + offset)
        .expect("decide's own closing brace is found");
    let decide_body = &source[decide_start..decide_end];
    assert!(
        decide_body.contains("command: \"git push\".to_owned()")
            && decide_body.contains("pr: None"),
        "`decide` stopped naming the three-word, unaimed action a push is adjudicated as"
    );
    assert!(
        decide_body.contains("-> Adjudication"),
        "`decide`'s signature moved; if the new shape reads the refs to classify the \
         destination rather than only to select a holder, the honesty contract entry \
         saying nothing adjudicates a destination is out of date"
    );
    // The refs are **read and written down** — the ledger names which push was
    // decided on rather than only that one was — and they are still **not
    // adjudicated**: what `decide` is handed is the checkout, and what it
    // answers about is the three words pinned above.
    //
    // The first version of this assertion said nobody read them, and it went on
    // passing after they were read, because the reading moved into a helper and
    // out of the window it looked at. A pin on the absence of a word is a pin a
    // rename defeats.
    let handler = crate::source_text::code_of(include_str!("../cli/mod.rs"));
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
        arm.contains("harness::guard::decide_pushing("),
        "the pre-push arm no longer decides through the one function `decide` is — a \
         second copy of the action it adjudicates may have come back"
    );
    assert!(
        arm.contains("refs_being_pushed()"),
        "the refs stopped reaching the ledger, so a push can be adjudicated and not named"
    );
    // The gap is only half closed. `decide` takes the refs too — but to select
    // **who** among several holders this checkout's push belongs to, never to decide **what** the push is: the three words pinned above
    // are unchanged, and `git push` is still absent from `DELIVERS`. If the
    // signature moves again, read whether the new argument answers the "who"
    // question this pin already covers or the "what" question it does not.
}

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

#[test]
fn the_control_surface_matcher_answers_its_corpus() {
    for (path, boundary) in super::CONTROL_SURFACE_CORPUS {
        let write_road = super::is_control_surface(path);
        assert_eq!(
            write_road, *boundary,
            "the write road answers `{path}` differently from the corpus's record; record the \
             new answer in `CONTROL_SURFACE_CORPUS` if it is intended, which reopens the \
             `control-surface-corpus` claim for re-affirmation"
        );
        let shell_road = super::surface_of(path) == Sensitivity::Boundary;
        assert_eq!(
            shell_road, *boundary,
            "the shell road answers `{path}` differently from the corpus's record"
        );
    }
}

const PI_FILES_THAT_UNLOAD_THE_GATE: &[&str] = &[
    "/home/me/.pi/agent/settings.json",
    "/home/me/.pi/agent/mcp.json",
    "/home/me/.pi/agent/extensions/estigia-workflow-authority.ts",
    "/home/me/.pi/agent/prompts/claim.md",
    "/work/repo/.pi/prompts/claim.md",
    "/work/repo/.pi/settings.json",
    "/work/repo/.pi/mcp.json",
    "/work/repo/.pi/extensions/anything.ts",
    ".pi/settings.json",
    ".pi/mcp.json",
    ".pi/extensions/anything.ts",
];

#[test]
fn every_file_that_can_unload_the_pi_gate_is_a_boundary_on_both_roads() {
    for spelled in PI_FILES_THAT_UNLOAD_THE_GATE {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine, and Pi reads it to decide what to load"
        );
        let (_, removed) = classify(
            "Bash",
            &serde_json::json!({ "command": format!("rm {spelled}") }),
        );
        assert_eq!(
            removed,
            Sensitivity::Boundary,
            "`rm {spelled}` answers routine"
        );
    }
    for lookalike in ["/work/repo/.pix/mcp.json", "/work/repo/my.pi/settings.json"] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": lookalike }));
        assert_ne!(
            written,
            Sensitivity::Boundary,
            "{lookalike} is not a Pi control file"
        );
    }
}

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
        pr_unidentified_reason: None,
        local_fast_forward_target: None,
        is_exact_integration_merge: false,
        runs_in: crate::harness::RunsIn::Session,
        target: crate::harness::CommitTarget::Checkout,
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
    nowhere.issue = Some(crate::identity::Identity::Number(164));
    assert!(!writes_outside_the_claim(
        &nowhere,
        &Action::Write {
            target: scratch.display().to_string()
        }
    ));
}

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

#[test]
fn the_local_settings_and_the_agent_definitions_are_boundaries_on_both_roads() {
    // No lock here, on purpose: `home_dir()` is read exactly once, into
    // `home`, and nothing after it reads `HOME`, `USERPROFILE`, `APPDATA` or
    // `XDG_CONFIG_HOME` again to decide a boundary from a spelled string.
    //
    // That was true of the whole of `classify` once:
    // `matches_relocated_claude_config` now reads `CLAUDE_CONFIG_DIR` too, to
    // answer a question no fragment can — whether a target lands under a
    // *relocated* Claude Code root — and it does so unlocked, the same way
    // `definition_for` already reads `XDG_CONFIG_HOME` unlocked (`test_env.rs`
    // carries that reasoning in full). It is tolerable here for the same
    // shape of reason: every target this loop builds sits under the real
    // `home` this process was started with, and a concurrent writer of
    // `CLAUDE_CONFIG_DIR` on another thread points it at a temporary
    // directory of its own, never at anything under `home` — so the answer
    // this loop compares against cannot change no matter when that other
    // write lands.
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
            "a write to {spelled} answers routine, so the gate can be switched off \
             through it"
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

/// `.claude/settings`, `.claude/agents/`
/// and `.claude/skills/` are spelled with the `.claude` directory name baked
/// into the fragment, so a `$CLAUDE_CONFIG_DIR` naming anything else hid all
/// three from `is_control_surface` — the write a relocated `setup` makes
/// ordinary read as `Routine`. `docs/honesty.md`'s entry named
/// `.claude.json`, `CLAUDE.md` and Estigia's own skill directory as already
/// safe, because those three fragments carry no directory name at all; this
/// test is about the three that do.
#[test]
fn a_relocated_claude_config_dirs_settings_agents_and_skills_are_boundaries_on_both_roads() {
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    for relative in [
        ["settings.json"].as_slice(),
        ["settings.local.json"].as_slice(),
        ["agents", "reviewer.md"].as_slice(),
        ["skills", "issue-flow", "SKILL.md"].as_slice(),
    ] {
        let mut target = relocated.path().to_path_buf();
        for part in relative {
            target.push(part);
        }
        let spelled = target.display().to_string();

        crate::test_env::with_process_env(
            "CLAUDE_CONFIG_DIR",
            &relocated.path().display().to_string(),
            || {
                let (_, written) = classify(
                    "Write",
                    &serde_json::json!({ "file_path": spelled.clone() }),
                );
                assert_eq!(
                    written,
                    Sensitivity::Boundary,
                    "a write to {spelled}, under a relocated CLAUDE_CONFIG_DIR, answers \
                     routine, so the gate can be switched off through it"
                );

                let (_, removed) = classify(
                    "Bash",
                    &serde_json::json!({ "command": format!("rm {spelled}") }),
                );
                assert_eq!(
                    removed,
                    Sensitivity::Boundary,
                    "removing {spelled} through the shell, under a relocated \
                     CLAUDE_CONFIG_DIR, answers routine"
                );
            },
        );
    }
}

/// The resolved-root check must reach no further than the root it was given —
/// a project's own, ordinarily-`Routine` `settings.json` must not become a
/// boundary merely because *some* session, somewhere, relocated Claude Code's
/// configuration to a directory of the same name elsewhere on disk.
#[test]
fn a_relocated_claude_config_dir_does_not_widen_the_gate_past_its_own_root() {
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let elsewhere = tempfile::tempdir().expect("an unrelated project");
    let target = elsewhere.path().join("settings.json");
    let spelled = target.display().to_string();

    crate::test_env::with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || {
            let (_, written) = classify(
                "Write",
                &serde_json::json!({ "file_path": spelled.clone() }),
            );
            assert_eq!(
                written,
                Sensitivity::Routine,
                "a project's own settings.json, outside the relocated CLAUDE_CONFIG_DIR, \
                 answers boundary — the resolved-root check reached past its own root"
            );
        },
    );
}

#[test]
fn a_relocated_pi_agent_dirs_control_files_are_boundaries_on_both_roads_and_no_wider() {
    let relocated = tempfile::tempdir().expect("a relocated PI_CODING_AGENT_DIR");
    let elsewhere = tempfile::tempdir().expect("an unrelated project");
    crate::test_env::with_process_env(
        "PI_CODING_AGENT_DIR",
        &relocated.path().display().to_string(),
        || {
            for relative in [
                ["settings.json"].as_slice(),
                ["mcp.json"].as_slice(),
                ["extensions", "estigia-workflow-authority.ts"].as_slice(),
                ["AGENTS.md"].as_slice(),
                ["AGENTS.override.md"].as_slice(),
                ["agents", "developer.md"].as_slice(),
                ["prompts", "claim.md"].as_slice(),
            ] {
                let mut target = relocated.path().to_path_buf();
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
                    "a write to {spelled}, under a relocated PI_CODING_AGENT_DIR, answers routine"
                );
                let (_, removed) = classify(
                    "Bash",
                    &serde_json::json!({ "command": format!("rm {spelled}") }),
                );
                assert_eq!(
                    removed,
                    Sensitivity::Boundary,
                    "`rm {spelled}` under a relocated PI_CODING_AGENT_DIR answers routine"
                );
            }
            let unrelated = elsewhere.path().join("settings.json").display().to_string();
            let (_, written) = classify("Write", &serde_json::json!({ "file_path": unrelated }));
            assert_eq!(written, Sensitivity::Routine);
        },
    );
}

/// A relocated root whose own name carries a space. The shell road folds every
/// whitespace run into a separator before it asks, so a root spelled with one
/// stopped matching there while the write road — which spells the path as it
/// is — still answered `Boundary`. The two roads must agree.
#[test]
fn a_relocated_claude_config_dir_with_a_space_is_a_boundary_on_both_roads() {
    let base = tempfile::tempdir().expect("a temporary directory");
    let relocated = base.path().join("estigia relocated");
    std::fs::create_dir(&relocated).expect("a relocated root with a space");

    for relative in [
        ["settings.json"].as_slice(),
        ["agents", "reviewer.md"].as_slice(),
    ] {
        let mut target = relocated.clone();
        for part in relative {
            target.push(part);
        }
        let spelled = target.display().to_string();

        crate::test_env::with_process_env(
            "CLAUDE_CONFIG_DIR",
            &relocated.display().to_string(),
            || {
                let (_, written) = classify(
                    "Write",
                    &serde_json::json!({ "file_path": spelled.clone() }),
                );
                assert_eq!(
                    written,
                    Sensitivity::Boundary,
                    "a write to {spelled}, under a relocated CLAUDE_CONFIG_DIR with a space, \
                     answers routine"
                );

                for (label, command) in [
                    ("quoted", format!("rm \"{spelled}\"")),
                    ("bare", format!("rm {spelled}")),
                    ("single-quoted", format!("rm '{spelled}'")),
                    ("escaped", format!("rm {}", spelled.replace(' ', "\\ "))),
                ] {
                    let (_, removed) = classify("Bash", &serde_json::json!({ "command": command }));
                    assert_eq!(
                        removed,
                        Sensitivity::Boundary,
                        "removing {spelled} through the shell with the {label} spelling, under a \
                         relocated CLAUDE_CONFIG_DIR with a space, answers routine — the shell \
                         road folds the space into a separator and the root stops matching"
                    );
                }
            },
        );
    }
}

/// The root is a whole path prefix, not a substring. A path that carries the
/// root's text but is not under it must stay `Routine`, or the check reaches
/// past the root it was given.
#[test]
fn a_path_that_merely_carries_the_relocated_root_is_not_a_boundary() {
    let relocated = tempfile::tempdir().expect("a relocated CLAUDE_CONFIG_DIR");
    let elsewhere = tempfile::tempdir().expect("an unrelated project");
    let target = elsewhere
        .path()
        .join(
            relocated
                .path()
                .components()
                .filter(|part| matches!(part, std::path::Component::Normal(_)))
                .collect::<std::path::PathBuf>(),
        )
        .join("settings.json");
    let spelled = target.display().to_string();

    crate::test_env::with_process_env(
        "CLAUDE_CONFIG_DIR",
        &relocated.path().display().to_string(),
        || {
            let (_, written) = classify(
                "Write",
                &serde_json::json!({ "file_path": spelled.clone() }),
            );
            assert_eq!(
                written,
                Sensitivity::Routine,
                "a path that merely carries the relocated root's text, without landing under \
                 it, answers boundary"
            );

            let (_, removed) = classify(
                "Bash",
                &serde_json::json!({ "command": format!("rm {spelled}") }),
            );
            assert_eq!(
                removed,
                Sensitivity::Routine,
                "removing a path that merely carries the relocated root's text, without \
                 landing under it, answers boundary"
            );
        },
    );
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
    "/home/me/.pi/agent/AGENTS.md",
    "/home/me/.gemini/GEMINI.md",
    r"C:\Users\me\AppData\Roaming\gemini\GEMINI.md",
    "/home/me/.cursor/estigia-workflow-authority.md",
    "/home/me/.qwen/QWEN.md",
    "/home/me/.config/crush/CRUSH.md",
    "/home/me/.continue/rules/estigia.md",
    "/home/me/.cline/rules/estigia.md",
    "/home/me/.codeium/windsurf/memories/global_rules.md",
    "/home/me/.zcode/AGENTS.md",
    "/home/me/.dsh/AGENTS.md",
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

#[test]
fn the_spelled_instruction_files_and_the_adapter_table_agree() {
    let folded: Vec<String> = SPELLED_INSTRUCTION_FILES
        .iter()
        .map(|path| path.replace('\\', "/").to_ascii_lowercase())
        .collect();

    for adapter in crate::setup::every_adapter() {
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
            crate::setup::every_adapter()
                .any(|adapter| folded.contains(adapter.instruction_fragment())),
            "{path} is spelled here but matches no adapter's fragment, so it is not an \
             instruction file this crate writes"
        );
    }
}

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
        "/home/me/.zcode/AGENTS.local.md",
        "/home/me/.dsh/AGENTS.local.md",
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

#[test]
fn a_retired_hosts_instruction_directory_is_still_a_boundary() {
    for spelled in [
        "/home/me/.codeium/windsurf/memories/global_rules.md",
        "/home/me/.codeium/windsurf/memories/zz-override.md",
        "/home/me/.codeium/windsurf/memories",
    ] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine, and a machine that installed the retired \
             host still has it read"
        );
    }
}

#[test]
fn the_rules_directories_of_continue_and_cline_stay_gated_without_an_adapter_deriving_them() {
    for spelled in [
        "/home/me/.cline/rules/zz-override.md",
        "/home/me/.continue/rules/zz-override.md",
        "/home/me/.cline/rules",
        "/home/me/.continue/rules",
    ] {
        let (_, written) = classify("Write", &serde_json::json!({ "file_path": spelled }));
        assert_eq!(
            written,
            Sensitivity::Boundary,
            "a write to {spelled} answers routine"
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
        // ZCode's configuration and the `~/.agents/mcp.json` fallback it reads
        // when that file holds no servers of its own; both are host-read.
        "/home/me/.zcode/cli/config.json",
        "/home/me/.agents/mcp.json",
        // The DeepSeek Harness definitions root. `.dsh/agents` is deliberately a
        // right-unanchored prefix; this row is under the directory it names.
        "/home/me/.dsh/agents/reviewer.md",
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
        // of directories gated at home, so the hole was closed on one
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

#[test]
fn a_defaulted_parameter_does_not_hide_a_control_surface() {
    let d = '$';
    let ob = '{';
    let cb = '}';
    for path in [
        ".claude/settings.json",
        ".claude.json",
        ".estigia/run.json",
        ".estigia/decisions.jsonl",
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

#[test]
fn a_patch_body_reaches_a_relative_control_surface() {
    for named in [
        ".estigia/run.json",
        ".estigia/decisions.jsonl",
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

#[test]
fn a_quoted_or_expanded_option_prefix_does_not_hide_a_control_surface() {
    let dq = '"';
    let sq = '\'';
    let pct = '%';
    for line in [
        format!("wget {dq}-O.claude/settings.json{dq} https://x"),
        format!("wget {sq}-O.claude/settings.json{sq} https://x"),
        format!("7z x a.7z {sq}-o.estigia/run.json{sq}"),
        format!("7z x a.7z {dq}-o.estigia/decisions.jsonl{dq}"),
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
            "curl -so.estigia/decisions.jsonl https://x",
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
        let line = format!("rm -rf x{divider}.estigia/decisions.jsonl");
        let (_, how) = classify("Bash", &serde_json::json!({ "command": &line }));
        assert_eq!(
            how,
            Sensitivity::Boundary,
            "`{line}` answers routine, so {divider:?} is in the folded set without being folded"
        );
    }
}

#[test]
fn the_spelled_folded_characters_and_the_constant_agree() {
    let spelled: String = FOLDED_CHARACTERS.iter().collect();
    assert_eq!(
        spelled,
        super::NOT_IN_A_PATH_SEGMENT,
        "the folded set and the list the fixture walks have diverged"
    );
}

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

#[test]
fn punctuation_does_not_hide_a_control_surface() {
    let dq = '"';
    let sq = '\'';
    for (line, why) in [
        (
            format!("rm -rf {dq}.estigia/decisions.jsonl{dq}"),
            "double quotes",
        ),
        (
            format!("rm -rf {sq}.estigia/decisions.jsonl{sq}"),
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

#[test]
fn a_drive_relative_spelling_is_measured_like_a_relative_one() {
    for named in [
        r"C:.estigia\decisions.jsonl",
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

#[test]
fn a_program_run_behind_a_head_that_only_looks_read_only_stays_a_boundary() {
    for command in [
        "a=/cat sh -c 'git push origin main'",
        "</bin/cat sh -c 'git push origin main'",
        "sh${IFS}-c${IFS}'$1'${IFS}x/cat 'git push origin main'",
    ] {
        let (_action, sensitivity) = classify("Bash", &json!({"command": command}));
        assert_eq!(sensitivity, Sensitivity::Boundary, "{command}");
    }
}

#[test]
fn a_here_document_terminator_is_compared_in_the_case_bash_reads() {
    for command in [
        "cat <<EOF <<'X'\neof\n$(git push origin main)\nEOF\nX",
        "Cat <<'EOF'\n$(git push origin main)\nEOF",
    ] {
        let (_action, sensitivity) = classify("Bash", &json!({"command": command}));
        assert_eq!(sensitivity, Sensitivity::Boundary, "{command}");
    }
}

#[test]
fn a_here_document_read_differently_by_the_shell_stays_a_boundary() {
    for command in [
        "cat <<EOF\n$\\\n(git push origin main)\nEOF",
        "cat <<END-X\nEND-X\ngit push origin main\nEND",
        "cat <<EOF\n${ git push origin main; }\nEOF",
        "cat <<\"E\\\\\"\nE\\\ngit push origin main\nE\\\\",
        "cat <<A <<'B'\nA\r\n$(git push origin main)\nB",
        "printf -v 'a[$(git push origin main)]' x",
        "test -v 'a[$(git push origin main)]'",
    ] {
        let (_action, sensitivity) = classify("Bash", &json!({"command": command}));
        assert_eq!(sensitivity, Sensitivity::Boundary, "{command}");
    }
}

fn class_of(line: &str) -> Action {
    classify("Bash", &json!({ "command": line })).0
}

fn boundary_command(line: &str) -> Option<String> {
    match class_of(line) {
        Action::Boundary { command, .. } => Some(command),
        _ => None,
    }
}

#[test]
fn a_quoted_command_word_is_the_word_it_spells() {
    for (quoted, plain) in [
        ("gh pr 'merge' 7 --squash", "gh pr merge 7 --squash"),
        (
            "gh api -X 'PUT' repos/o/r/pulls/7/merge",
            "gh api -X PUT repos/o/r/pulls/7/merge",
        ),
        ("git 'push' origin main", "git push origin main"),
        ("git \"merge\" feat", "git merge feat"),
        ("git pu'sh' origin main", "git push origin main"),
        ("\"gi\"t push origin main", "git push origin main"),
        ("git $'push' origin main", "git push origin main"),
        (
            "estigia config 'set' \"Unclaimed write policy\" off",
            "estigia config set \"Unclaimed write policy\" off",
        ),
    ] {
        assert!(boundary_command(plain).is_some(), "control: {plain}");
        assert!(boundary_command(quoted).is_some(), "{quoted}");
    }
}

#[test]
fn a_quoted_gh_verb_and_a_quoted_commit_are_not_untouched() {
    for line in [
        "gh issue \"close\" 5",
        "gh release 'create' v1",
        "git 'commit' -m x",
    ] {
        assert!(!matches!(class_of(line), Action::Untouched), "{line}");
    }
}

#[test]
fn quoted_words_inside_a_substitution_still_run() {
    for line in [
        "echo $(git 'push' origin main)",
        "echo \"$(git 'push' origin main)\"",
        "echo `git 'push' origin main`",
        "x=\"$(echo \"$(git 'push' origin main)\")\"",
    ] {
        assert!(boundary_command(line).is_some(), "{line}");
    }
}

#[test]
fn an_unbalanced_quote_classifies_as_it_did() {
    assert!(boundary_command("git push 'origin main").is_some());
    assert!(boundary_command("echo 'it''s'; git 'push' origin main").is_some());
}

#[test]
fn quoted_data_keeps_its_classification() {
    for line in [
        "gh issue view 5 --jq '.title | split(\" \")'",
        "gh issue comment 5 --body 'git push origin main'",
        "gh issue comment 5 --body \"git 'push' origin main\"",
        "cat > x <<'EOF'\ngit push origin main\nEOF",
        "echo 'git push origin main'",
        "grep 'merge' notes.txt",
    ] {
        assert!(!matches!(class_of(line), Action::Boundary { .. }), "{line}");
    }
}

#[test]
fn a_quoted_word_that_is_not_the_verb_is_not_folded_into_one() {
    for line in ["git commit -m 'push'", "echo 'push'"] {
        assert!(boundary_command(line).is_none(), "{line}");
    }
}

#[test]
fn the_real_command_behind_a_prefix_is_read_whole() {
    use crate::harness::shell::{runs_a_program_after_pre, writes_a_file};

    let missed: Vec<&str> = [
        "timeout 60 bash -c 'echo x > src/lib.rs'",
        "sudo sh -c 'echo x > src/lib.rs'",
        "ls | xargs sh -c 'echo x > src/lib.rs'",
        "sudo sed -i s/a/b/ src/x.rs",
        "sudo -u bob python3 -c 'open(1)'",
        "env FOO=1 sed -i s/a/b/ src/x.rs",
        "sudo nice -n 5 curl -o src/x https://e.test",
        "if true; then rm -rf src; fi",
        "{ rm src/x; }",
        "! rm x",
        "LC_ALL=C rm -rf src",
        "for f in *; do cp a src/b; done",
        "while true; do sed -i s/a/b/ x; done",
        "FOO=1 BAR=2 sh -c 'echo x > f'",
        "if true; then sudo rm x; fi",
    ]
    .into_iter()
    .filter(|line| class_of(line) == Action::Untouched)
    .collect();
    assert!(missed.is_empty(), "left untouched: {missed:?}");

    assert!(runs_a_program_after_pre("LC_ALL=C rg --pre ./x foo"));
    assert!(runs_a_program_after_pre("sudo rg --pre=./x foo"));

    for line in [
        "timeout 60 grep -i foo src",
        "env FOO=1 cargo test",
        "if true; then echo hi; fi",
        "sudo sed s/a/b/ src/x.rs",
        "LC_ALL=C grep -i foo src",
        "{ echo hi; }",
        "! test -f x",
    ] {
        assert_eq!(writes_a_file(line), None, "{line:?}");
    }
}

#[test]
fn a_drive_lettered_directory_is_a_plain_directory_and_a_drive_relative_one_is_not() {
    let directory = "C:/Users/runneradmin/AppData/Local/Temp/.tmpX/topic~claude-abcd1234";
    for command in [
        format!("cd {directory} && git merge origin/main"),
        format!("cd '{directory}' && git merge origin/main"),
        format!("git -C {directory} merge origin/main"),
    ] {
        let (runs_in, exact, _) = super::directed_integration_merge(&command, true);
        assert_eq!(runs_in, RunsIn::Named(directory.to_owned()), "{command}");
        assert!(exact, "{command}");
    }
    for refused in [
        r"C:\Users\runner\x",
        "C:x",
        "C:",
        "1:/x",
        "host:/x",
        "/a:/b",
        "C:/a:b",
    ] {
        let command = format!("cd {refused} && git merge origin/main");
        let (runs_in, exact, _) = super::directed_integration_merge(&command, true);
        assert_eq!(runs_in, RunsIn::Unestablished, "{command}");
        assert!(!exact, "{command}");
    }
}

#[test]
fn a_merge_reads_its_ci_release_and_an_unreadable_store_refuses() {
    let (action, _) = classify("Bash", &json!({"command": "gh pr merge 54 --merge"}));
    let mut run = Run::new("claude-abcd1234".to_owned());
    run.issue = Some(crate::identity::Identity::Number(12));
    run.review_transaction = Some(crate::transport::claim::ReviewTransaction {
        epoch: "a".repeat(32),
        pr: 54,
        head: "e".repeat(40),
        base: "b".repeat(40),
        digest: "c".repeat(64),
    });
    let answering = |answer: Result<bool, crate::transport::Failure>| {
        let answer = std::cell::RefCell::new(Some(answer));
        super::unreleased_merge(&action, &run, &|issue, receipt| {
            assert_eq!(issue, 12);
            assert_eq!(receipt.pr, 54);
            answer.borrow_mut().take().expect("read once")
        })
    };

    assert!(answering(Ok(true)).is_none());
    assert_eq!(
        answering(Ok(false)).map(|refusal| refusal.code),
        Some("merge-without-ci-release")
    );
    let unreadable = answering(Err(crate::transport::Failure::Read(
        "the store could not be read".to_owned(),
    )))
    .expect("an unreadable store refuses");
    assert_eq!(unreadable.code, "read-failed");

    let (push, _) = classify("Bash", &json!({"command": "git push"}));
    assert!(super::unreleased_merge(&push, &run, &|_, _| Ok(false)).is_none());
}
