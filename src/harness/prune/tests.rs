use super::*;

#[test]
fn correction_configured_upstream_presence_keeps_the_checkout() {
    for alternative_remote in [false, true] {
        let (root, clone, trees) = sweep_fixture();
        let upstream = if alternative_remote {
            let other = root.path().join("other.git");
            git(
                root.path(),
                &[
                    "clone",
                    "--quiet",
                    "--bare",
                    root.path().join("remote.git").to_str().unwrap(),
                    other.to_str().unwrap(),
                ],
            );
            git(&clone, &["remote", "add", "other", other.to_str().unwrap()]);
            git(&clone, &["fetch", "--quiet", "other"]);
            "other/issue-2-open"
        } else {
            git(&clone, &["push", "origin", "issue-2-open:renamed"]);
            "origin/renamed"
        };
        git(
            &clone,
            &["branch", "--set-upstream-to", upstream, "issue-2-open"],
        );
        git(
            &root.path().join("work"),
            &["push", "origin", "--delete", "issue-2-open"],
        );
        let context = Context::live(root.path().join("skill"), clone, None);
        let view = View {
            context: &context,
            base: "main",
            holdings: &[],
            root: Some(&trees),
            ownership: Ok(Vec::new()),
            ownership_root: Some(root.path().join("runs")),
        };
        let decision = orphan_decision(&view);
        assert!(!decision.delete, "{upstream}: {decision:?}");
        assert!(
            decision.reason.contains("remote branch still exists"),
            "{decision:?}"
        );
        assert!(confirm(&view, &[decision]).is_empty());
        assert!(trees.join("issue-2-open").exists());
        git(
            &context.repo_dir,
            &["push", "origin", "issue-2-open:archive"],
        );
        git(
            &context.repo_dir,
            &[
                "push",
                if alternative_remote {
                    "other"
                } else {
                    "origin"
                },
                "--delete",
                if alternative_remote {
                    "issue-2-open"
                } else {
                    "renamed"
                },
            ],
        );
        let gone = orphan_decision(&view);
        assert!(gone.delete, "{upstream}: {gone:?}");
        let removed = confirm(&view, &[gone]);
        assert_eq!(removed[0]["outcome"], "removed", "{removed:?}");
    }
}

#[test]
fn correction_confirmation_rereads_new_holders_and_failed_ownership() {
    for case in [
        "session",
        "pointer",
        "unreadable",
        "registry",
        "boundary-session",
        "boundary-pointer",
        "boundary-unreadable",
    ] {
        let (root, clone, trees) = sweep_fixture();
        git(&clone, &["push", "origin", "issue-2-open:archive"]);
        git(&clone, &["push", "origin", "--delete", "issue-2-open"]);
        let context = Context::live(root.path().join("skill"), clone, None);
        let pointers = root.path().join("runs");
        let view = View {
            context: &context,
            base: "main",
            holdings: &[],
            root: Some(&trees),
            ownership: Ok(Vec::new()),
            ownership_root: Some(pointers.clone()),
        };
        let decision = orphan_decision(&view);
        assert!(decision.delete, "{decision:?}");
        let change_ownership = || {
            std::fs::create_dir_all(&pointers).unwrap();
            match case {
                "session" | "boundary-session" => {
                    let store = crate::transport::store::Store::open(
                        &crate::transport::store::beside(&pointers),
                    )
                    .unwrap();
                    store
                        .session_seen(
                            &crate::transport::store::SessionSeen {
                                run_id: "opencode-late",
                                runtime: "opencode",
                                session_id: "late",
                                repo_dir: trees.join("issue-2-open").to_str(),
                                wake: None,
                                pane: None,
                            },
                            1,
                        )
                        .unwrap();
                }
                "pointer" | "boundary-pointer" => {
                    let mut pointer = Run::new("opencode-late".to_owned());
                    pointer.issue = Some(crate::identity::Identity::Number(3));
                    pointer.worktree = Some(trees.join("issue-2-open"));
                    std::fs::write(
                        pointers.join("opencode-late.json"),
                        serde_json::to_vec(&pointer).unwrap(),
                    )
                    .unwrap();
                }
                "registry" => std::fs::write(
                    crate::transport::store::beside(&pointers),
                    "unreadable registry",
                )
                .unwrap(),
                _ => std::fs::write(pointers.join("opencode-late.json"), "unreadable pointer")
                    .unwrap(),
            }
        };
        let answers = if case.starts_with("boundary-") {
            vec![confirm_worktree_after(&view, &decision, change_ownership)]
        } else {
            change_ownership();
            confirm(&view, &[decision])
        };
        assert_eq!(answers[0]["outcome"], "kept", "{case}: {answers:?}");
        let reason = answers[0]["detail"].as_str().unwrap();
        assert!(
            reason.contains(if case.ends_with("session") {
                "session"
            } else {
                "ownership"
            }),
            "{case}: {answers:?}"
        );
        assert!(trees.join("issue-2-open").exists());
    }
}

#[test]
fn correction_missing_local_and_ambiguous_upstreams_are_kept() {
    for case in ["untracked", "local", "unknown", "missing-merge", "multiple"] {
        let (root, clone, trees) = sweep_fixture();
        git(&clone, &["push", "origin", "issue-2-open:archive"]);
        git(&clone, &["push", "origin", "--delete", "issue-2-open"]);
        match case {
            "untracked" => git(&clone, &["branch", "--unset-upstream", "issue-2-open"]),
            "local" => git(&clone, &["config", "branch.issue-2-open.remote", "."]),
            "unknown" => git(&clone, &["config", "branch.issue-2-open.remote", "unknown"]),
            "missing-merge" => git(&clone, &["config", "--unset", "branch.issue-2-open.merge"]),
            _ => git(
                &clone,
                &[
                    "config",
                    "--add",
                    "branch.issue-2-open.merge",
                    "refs/heads/archive",
                ],
            ),
        }
        let context = Context::live(root.path().join("skill"), clone, None);
        let view = View {
            context: &context,
            base: "main",
            holdings: &[],
            root: Some(&trees),
            ownership: Ok(Vec::new()),
            ownership_root: Some(root.path().join("runs")),
        };
        let decision = orphan_decision(&view);
        assert!(!decision.delete, "{case}: {decision:?}");
        assert!(
            decision.reason.contains(if case == "untracked" {
                "no configured upstream"
            } else {
                "tracking identity"
            }),
            "{case}: {decision:?}"
        );
        assert!(confirm(&view, &[decision]).is_empty());
        assert!(trees.join("issue-2-open").exists());
    }
}

#[test]
fn correction_revision_and_branch_movement_after_proof_are_kept() {
    for move_branch in [false, true] {
        let (root, clone, trees) = sweep_fixture();
        git(&clone, &["push", "origin", "issue-2-open:archive"]);
        git(&clone, &["push", "origin", "--delete", "issue-2-open"]);
        let context = Context::live(root.path().join("skill"), clone, None);
        let view = View {
            context: &context,
            base: "main",
            holdings: &[],
            root: Some(&trees),
            ownership: Ok(Vec::new()),
            ownership_root: Some(root.path().join("runs")),
        };
        let decision = orphan_decision(&view);
        assert!(decision.delete);
        let target = trees.join("issue-2-open");
        let answer = confirm_worktree_after(&view, &decision, || {
            if move_branch {
                git(&target, &["switch", "--quiet", "-c", "moved"]);
            } else {
                git(
                    &target,
                    &[
                        "commit",
                        "--quiet",
                        "--allow-empty",
                        "-m",
                        "late unique commit",
                    ],
                );
            }
        });
        assert_eq!(answer["outcome"], "kept", "{answer:?}");
        assert!(
            answer["detail"].as_str().unwrap().contains(if move_branch {
                "branch changed"
            } else {
                "not at the delivered head"
            }),
            "{answer:?}"
        );
        assert!(target.exists());
    }
}

#[test]
fn a_remote_gone_checkout_reachable_from_another_remote_is_removed_only_on_confirmation() {
    let (root, clone, trees) = sweep_fixture();
    git(&clone, &["push", "origin", "issue-2-open:archive"]);
    git(
        &root.path().join("work"),
        &["push", "origin", "--delete", "issue-2-open"],
    );
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).unwrap();
    let context = Context::live(skill, clone.clone(), None);
    let view = View {
        context: &context,
        base: "main",
        holdings: &[],
        root: Some(&trees),
        ownership: Ok(Vec::new()),
        ownership_root: Some(root.path().join("runs")),
    };
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
        serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
    ]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let swept = sweep(&view).unwrap();
        let orphan = swept
            .iter()
            .find(|entry| {
                entry.kind == Kind::Worktree && entry.branch.as_deref() == Some("issue-2-open")
            })
            .unwrap();
        assert!(orphan.delete, "remote-gone checkout was kept: {orphan:?}");
        assert!(
            trees.join("issue-2-open").exists(),
            "discovery removed a checkout"
        );
        let answer = confirm(&view, std::slice::from_ref(orphan));
        assert_eq!(answer[0]["worktree_removed"], true, "{answer:?}");
        assert_eq!(answer[0]["branch_deleted"], true, "{answer:?}");
        assert!(!trees.join("issue-2-open").exists());
        let local = run(
            &["git", "show-ref", "--verify", "refs/heads/issue-2-open"],
            Some(&clone),
            How::tolerated(),
        )
        .unwrap();
        assert_ne!(local.status, 0);
        let remote = run(
            &["git", "ls-remote", "--heads", "origin", "archive"],
            Some(&clone),
            How::read(),
        )
        .unwrap();
        assert!(remote.stdout.contains("refs/heads/archive"));
    });
}

fn orphan_decision(view: &View<'_>) -> Swept {
    let bin = crate::test_env::scripted_gh();
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
    ]);
    crate::test_env::with_scripted_gh(bin.path(), &script, || {
        observe_worktrees(view, &mut BTreeMap::new())
            .unwrap()
            .into_iter()
            .find(|entry| entry.branch.as_deref() == Some("issue-2-open"))
            .and_then(decide)
            .unwrap()
    })
}

#[test]
fn remote_gone_eligibility_keeps_dirty_unique_held_present_and_unreadable_checkouts() {
    for (case, reason) in [
        ("dirty", "scratch.txt"),
        ("unique", "commits not on the remote"),
        ("pointer", "ownership is held or unknown"),
        ("primary-pointer", "ownership is held or unknown"),
        ("unlocated-pointer", "ownership is held or unknown"),
        ("session", "session not known ended"),
        ("unreadable-pointer", "ownership is held or unknown"),
        ("unreadable-ownership", "ownership could not be read"),
        ("present", "remote branch still exists"),
        ("unreadable-remote", "remote state could not be read"),
        ("unreadable-local", "checkout state could not be read"),
    ] {
        let (root, clone, trees) = sweep_fixture();
        let target = trees.join("issue-2-open");
        git(&clone, &["push", "origin", "issue-2-open:archive"]);
        if case != "present" {
            git(
                &root.path().join("work"),
                &["push", "origin", "--delete", "issue-2-open"],
            );
        }
        let mut holdings = Vec::new();
        let mut ownership = Ok(Vec::new());
        match case {
            "dirty" => std::fs::write(target.join("scratch.txt"), "keep\n").unwrap(),
            "unique" => {
                std::fs::write(target.join("local.txt"), "unique\n").unwrap();
                git(&target, &["add", "."]);
                git(&target, &["commit", "--quiet", "-m", "unique"]);
            }
            "pointer" | "primary-pointer" | "unlocated-pointer" | "unreadable-pointer" => {
                let mut pointer = Run::new("opencode-held".to_owned());
                pointer.issue = Some(crate::identity::Identity::Number(3));
                if case == "primary-pointer" {
                    pointer.repo_dir = Some(target.clone());
                } else if case != "unlocated-pointer" {
                    pointer.worktree = Some(target.clone());
                }
                pointer.unreadable = case == "unreadable-pointer";
                holdings.push(pointer);
            }
            "session" => ownership = Ok(vec![target.clone()]),
            "unreadable-ownership" => ownership = Err("fixture registry read failed".to_owned()),
            "unreadable-remote" => git(
                &clone,
                &[
                    "remote",
                    "set-url",
                    "origin",
                    root.path().join("missing.git").to_str().unwrap(),
                ],
            ),
            "unreadable-local" => std::fs::write(target.join(".git"), "gitdir: missing\n").unwrap(),
            _ => {}
        }
        let context = Context::live(root.path().join("skill"), clone.clone(), None);
        let view = View {
            context: &context,
            base: "main",
            holdings: &holdings,
            root: Some(&trees),
            ownership,
            ownership_root: Some(root.path().join("runs")),
        };
        let decision = orphan_decision(&view);
        assert!(!decision.delete, "{case}: {decision:?}");
        assert!(decision.reason.contains(reason), "{case}: {decision:?}");
        assert!(confirm(&view, &[decision]).is_empty());
        assert!(target.exists());
        let local = run(
            &["git", "show-ref", "--verify", "refs/heads/issue-2-open"],
            Some(&clone),
            How::read(),
        )
        .unwrap();
        assert_eq!(local.status, 0);
    }
}

#[test]
fn confirmation_rechecks_remote_gone_work_before_removing_it() {
    let (root, clone, trees) = sweep_fixture();
    git(&clone, &["push", "origin", "issue-2-open:archive"]);
    git(&clone, &["push", "origin", "--delete", "issue-2-open"]);
    let context = Context::live(root.path().join("skill"), clone.clone(), None);
    let view = View {
        context: &context,
        base: "main",
        holdings: &[],
        root: Some(&trees),
        ownership: Ok(Vec::new()),
        ownership_root: Some(root.path().join("runs")),
    };
    let decision = orphan_decision(&view);
    assert!(decision.delete);
    git(&clone, &["push", "origin", "issue-2-open"]);
    let answers = confirm(&view, std::slice::from_ref(&decision));
    assert_eq!(answers[0]["outcome"], "kept");
    assert!(
        answers[0]["detail"]
            .as_str()
            .unwrap()
            .contains("still exists")
    );
    assert!(trees.join("issue-2-open").exists());
    git(&clone, &["push", "origin", "--delete", "issue-2-open"]);
    std::fs::write(trees.join("issue-2-open").join("late.txt"), "late\n").unwrap();
    let answers = confirm(&view, &[decision]);
    assert_eq!(answers[0]["outcome"], "kept");
    assert!(answers[0]["detail"].as_str().unwrap().contains("late.txt"));
    assert!(trees.join("issue-2-open").exists());
}

#[test]
fn ownership_reads_nonended_sessions_and_refuses_corrupt_pointers_and_store() {
    let root = tempfile::tempdir().unwrap();
    let pointers = root.path().join("runs");
    std::fs::create_dir(&pointers).unwrap();
    let path = crate::transport::store::beside(&pointers);
    let store = crate::transport::store::Store::open(&path).unwrap();
    let checkout = root.path().join("checkout");
    store
        .session_seen(
            &crate::transport::store::SessionSeen {
                run_id: "opencode-held",
                runtime: "opencode",
                session_id: "held",
                repo_dir: checkout.to_str(),
                wake: None,
                pane: None,
            },
            1,
        )
        .unwrap();
    assert_eq!(ownership(&pointers).unwrap(), vec![checkout]);
    store.session_ended("opencode-held", None, 2).unwrap();
    assert!(ownership(&pointers).unwrap().is_empty());
    std::fs::write(pointers.join("opencode-corrupt.json"), "not json").unwrap();
    assert!(
        ownership(&pointers)
            .unwrap_err()
            .contains("ownership could not be read")
    );
    let bad = tempfile::tempdir().unwrap();
    let bad_pointers = bad.path().join("runs");
    std::fs::create_dir(&bad_pointers).unwrap();
    std::fs::write(crate::transport::store::beside(&bad_pointers), "not sqlite").unwrap();
    assert!(ownership(&bad_pointers).is_err());
}

fn observed(kind: Kind, name: &str, sweepable: Option<&str>) -> Observed {
    Observed {
        kind,
        name: name.to_owned(),
        branch: Some(name.to_owned()),
        issue: issue_of_branch(name),
        sweepable: sweepable.map(str::to_owned),
        unpublished: false,
        uncommitted: Vec::new(),
        held: None,
        kept: None,
    }
}

#[test]
fn a_branch_name_carries_its_issue_and_a_name_that_does_not_carries_none() {
    assert_eq!(
        issue_of_branch("issue-777-branch-worktree-cleanup"),
        Some(777)
    );
    assert_eq!(issue_of_branch("issue-51-split-pipe-tests"), Some(51));
    assert_eq!(issue_of_branch("main"), None);
    assert_eq!(issue_of_branch("fix/12"), None);
    assert_eq!(issue_of_branch("issue-not-a-number"), None);
}

#[test]
fn only_what_is_merged_or_closed_is_listed_at_all() {
    assert_eq!(
        decide(observed(Kind::RemoteHead, "issue-1-open", None)),
        None
    );
    let listed = decide(observed(
        Kind::RemoteHead,
        "issue-1-merged",
        Some("`issue-1-merged` is merged into main"),
    ))
    .expect("a merged head is listed");
    assert!(listed.delete);
}

#[test]
fn a_live_claim_keeps_what_it_holds() {
    let mut held = observed(
        Kind::RemoteHead,
        "issue-1-merged",
        Some("`issue-1-merged` is merged into main"),
    );
    held.held = Some("claude-abc12345".to_owned());
    let listed = decide(held).expect("a held head is still listed");
    assert!(
        !listed.delete,
        "a live claim's head was marked for deletion"
    );
    assert!(
        listed.reason.contains("claude-abc12345"),
        "the keep reason does not name the holder: {}",
        listed.reason
    );
}

#[test]
fn unpublished_commits_keep_a_checkout_even_when_a_claim_holds_it() {
    let mut dirty = observed(
        Kind::Worktree,
        "/trees/issue-1-merged",
        Some("issue #1 is closed"),
    );
    dirty.unpublished = true;
    dirty.held = Some("claude-abc12345".to_owned());
    let listed = decide(dirty).expect("a dirty checkout is listed");
    assert!(!listed.delete, "a checkout with unpublished work was swept");
    assert!(
        listed.reason.contains("not on the remote"),
        "the keep reason does not name the unpublished work: {}",
        listed.reason
    );
}

#[test]
fn a_template_prefix_is_the_part_before_its_first_scope() {
    let trees = crate::test_env::outside_the_checkout("/tmp/trees");
    assert_eq!(
        root_prefix(Some(&format!("{trees}/<repo>/<branch>"))),
        Some(std::path::PathBuf::from(format!("{trees}/")))
    );
    assert_eq!(
        root_prefix(Some(&trees)),
        Some(std::path::PathBuf::from(&trees))
    );
    assert_eq!(root_prefix(Some("unset")), None);
    assert_eq!(root_prefix(Some("relative/<branch>")), None);
    assert_eq!(root_prefix(None), None);
}

/// One `git` call in a fixture, panicking on failure like the neighbouring
/// fixtures do: a machine without a usable `git` cannot measure this at all.
fn git(at: &std::path::Path, arguments: &[&str]) {
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

/// A clone with a local remote, three issue branches, and four checkouts: one
/// merged, one open, one closed, and one detached — the four shapes the sweep
/// has to tell apart.
fn sweep_fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    static TEMPLATES: crate::test_env::Templates<()> = crate::test_env::Templates::new();
    let (root, ()) = TEMPLATES.copy("sweep", build_sweep_fixture);
    let clone = root.path().join("clone");
    let trees = root.path().join("trees");
    (root, clone, trees)
}

fn build_sweep_fixture(root: &std::path::Path) {
    let remote = root.join("remote.git");
    let work = root.join("work");
    let clone = root.join("clone");
    let trees = root.join("trees");

    git(
        root,
        &["init", "--quiet", "--bare", remote.to_str().unwrap()],
    );
    std::fs::create_dir_all(&work).expect("a work directory");
    git(&work, &["init", "--quiet", "-b", "main"]);
    git(&work, &["config", "user.email", "estigia@example.invalid"]);
    git(&work, &["config", "user.name", "Estigia test"]);
    std::fs::write(work.join("main.txt"), "main\n").expect("main content");
    git(&work, &["add", "main.txt"]);
    git(&work, &["commit", "--quiet", "-m", "main"]);
    git(
        &work,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&work, &["push", "--quiet", "-u", "origin", "main"]);

    for (branch, merged) in [
        ("issue-1-merged", true),
        ("issue-2-open", false),
        ("issue-3-closed", false),
        ("issue-4-held", true),
    ] {
        git(&work, &["switch", "--quiet", "main"]);
        git(&work, &["switch", "--quiet", "-c", branch]);
        std::fs::write(work.join(format!("{branch}.txt")), "work\n").expect("branch content");
        git(&work, &["add", "."]);
        git(&work, &["commit", "--quiet", "-m", branch]);
        git(&work, &["push", "--quiet", "-u", "origin", branch]);
        if merged {
            git(&work, &["switch", "--quiet", "main"]);
            git(
                &work,
                &["merge", "--quiet", "--no-ff", "-m", "merge", branch],
            );
            git(&work, &["push", "--quiet", "origin", "main"]);
        }
    }
    git(&work, &["switch", "--quiet", "main"]);

    git(
        root,
        &[
            "clone",
            "--quiet",
            remote.to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    git(&clone, &["config", "user.email", "estigia@example.invalid"]);
    git(&clone, &["config", "user.name", "Estigia test"]);
    git(
        &clone,
        &[
            "fetch",
            "--quiet",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    // The bare remote's own HEAD is `master`, which it never had, so the clone
    // arrives with no local `main` and a detached head.
    git(&clone, &["switch", "--quiet", "-c", "main", "origin/main"]);
    std::fs::create_dir_all(&trees).expect("a trees directory");
    for branch in [
        "issue-1-merged",
        "issue-2-open",
        "issue-3-closed",
        "issue-4-held",
    ] {
        git(
            &clone,
            &[
                "worktree",
                "add",
                "--quiet",
                trees.join(branch).to_str().unwrap(),
                branch,
            ],
        );
    }
    git(
        &clone,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            trees.join("panel-1").to_str().unwrap(),
            "main",
        ],
    );
    // The closed issue's checkout carries a commit the remote does not have.
    std::fs::write(trees.join("issue-3-closed").join("local.txt"), "local\n").expect("local work");
    git(&trees.join("issue-3-closed"), &["add", "."]);
    git(
        &trees.join("issue-3-closed"),
        &["commit", "--quiet", "-m", "unpublished"],
    );
}

#[test]
fn the_sweep_lists_merged_and_closed_and_keeps_held_and_unpublished() {
    let (root, clone, trees) = sweep_fixture();
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    let context = crate::transport::Context::live(skill, clone.clone(), None);

    let mut live = crate::harness::session::Run::new("claude-live1234".to_owned());
    live.issue = Some(crate::identity::Identity::Number(4));
    live.worktree = Some(trees.join("issue-4-held"));
    live.repo_dir = Some(clone);
    live.verified_at = Some(crate::harness::session::now_seconds().unwrap_or(0));
    let holdings = vec![live];

    let view = View {
        context: &context,
        base: "main",
        holdings: &holdings,
        root: Some(trees.as_path()),
        ownership: Ok(Vec::new()),
        ownership_root: Some(root.path().join("runs")),
    };
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
        serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
    ]);
    let bin = crate::test_env::scripted_gh();
    let swept = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        sweep(&view).expect("the sweep reads the fixture")
    });

    let find = |kind: Kind, name: &str| {
        swept
            .iter()
            .find(|entry| entry.kind == kind && entry.name.contains(name))
            .unwrap_or_else(|| panic!("{kind:?} {name} was not listed at all: {swept:?}"))
    };

    assert!(
        find(Kind::RemoteHead, "issue-1-merged").delete,
        "a merged head was kept"
    );
    assert!(
        !swept
            .iter()
            .any(|entry| entry.kind == Kind::RemoteHead && entry.name == "issue-2-open"),
        "an open head was listed: {swept:?}"
    );
    assert!(
        find(Kind::RemoteHead, "issue-3-closed").delete,
        "a closed head was kept"
    );
    assert!(
        find(Kind::Worktree, "issue-1-merged").delete,
        "a merged checkout was kept"
    );
    let open = find(Kind::Worktree, "issue-2-open");
    assert!(!open.delete, "an open checkout was swept");
    assert!(
        open.reason.contains("neither merged nor closed"),
        "the open checkout is kept without a reason: {}",
        open.reason
    );
    assert!(
        !find(Kind::Worktree, "issue-3-closed").delete,
        "a checkout carrying unpublished work was swept"
    );
    assert!(
        !find(Kind::Worktree, "issue-4-held").delete,
        "a checkout a live claim holds was swept"
    );
    assert!(
        find(Kind::Worktree, "panel-1").delete,
        "a detached checkout no branch owns was kept"
    );
}

#[test]
fn a_kept_checkout_names_the_uncommitted_files_it_carries() {
    let (root, clone, trees) = sweep_fixture();
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    let context = crate::transport::Context::live(skill, clone, None);
    std::fs::write(trees.join("issue-2-open").join("scratch.txt"), "x\n").expect("scratch");
    let view = View {
        context: &context,
        base: "main",
        holdings: &[],
        root: Some(trees.as_path()),
        ownership: Ok(Vec::new()),
        ownership_root: Some(root.path().join("runs")),
    };
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
        serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
    ]);
    let bin = crate::test_env::scripted_gh();
    let swept = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        sweep(&view).expect("the sweep reads the fixture")
    });
    let open = swept
        .iter()
        .find(|entry| entry.name.contains("issue-2-open"))
        .expect("an open checkout with work is listed");
    assert!(!open.delete);
    assert!(
        open.reason.contains("scratch.txt") && open.reason.contains("uncommitted"),
        "the reason does not name the file: {}",
        open.reason
    );
}

#[test]
fn a_checkout_with_no_reason_to_sweep_is_still_listed_as_kept() {
    let mut open = observed(Kind::Worktree, "/trees/issue-2-open", None);
    open.uncommitted = vec!["M a.txt".to_owned()];
    open.unpublished = true;
    let listed = decide(open).expect("a checkout is always listed");
    assert!(!listed.delete);
    assert!(listed.reason.contains("a.txt"), "{}", listed.reason);
    assert_eq!(
        decide(observed(Kind::RemoteHead, "issue-2-open", None)),
        None
    );
}

/// A delivery that skips `release` leaves a checkout whose issue is
/// closed. The doctor row reports it red and names `estigia prune`.
#[test]
fn the_doctor_row_reports_a_closed_checkout_red() {
    let (root, clone, _trees) = sweep_fixture();
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
        serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
    ]);
    let bin = crate::test_env::scripted_gh();
    let check = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        crate::harness::doctor::sweep(Some(&skill), &clone, &[])
    });
    let crate::harness::doctor::Health::Broken { detail, resolution } = &check.health else {
        panic!(
            "a closed checkout left by a skipped release was not reported red: {:?}",
            check.health
        );
    };
    assert!(
        detail.contains("issue-1-merged") || detail.contains("issue-3-closed"),
        "the row named no swept checkout: {detail}"
    );
    assert!(
        format!("{resolution}").contains("estigia prune --confirm"),
        "the row does not name the command that clears it: {resolution}"
    );
}

#[test]
fn confirm_removes_only_what_the_sweep_marked_for_deletion() {
    let (root, clone, trees) = sweep_fixture();
    let skill = root.path().join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    let context = crate::transport::Context::live(skill, clone.clone(), None);

    let mut live = crate::harness::session::Run::new("claude-live1234".to_owned());
    live.issue = Some(crate::identity::Identity::Number(4));
    live.worktree = Some(trees.join("issue-4-held"));
    live.repo_dir = Some(clone.clone());
    let holdings = vec![live];

    let view = View {
        context: &context,
        base: "main",
        holdings: &holdings,
        root: Some(trees.as_path()),
        ownership: Ok(Vec::new()),
        ownership_root: Some(root.path().join("runs")),
    };
    let script = crate::test_env::answers(&[
        crate::test_env::open_but_unmatched_issue(1),
        crate::test_env::open_but_unmatched_issue(2),
        crate::test_env::closed_issue(3),
        crate::test_env::open_but_unmatched_issue(4),
        serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
    ]);
    let bin = crate::test_env::scripted_gh();
    let answers = crate::test_env::with_scripted_gh(bin.path(), &script, || {
        let swept = sweep(&view).expect("the sweep reads the fixture");
        confirm(&view, &swept)
    });

    // The merged head and the closed head are gone from the remote.
    let heads = crate::transport::run(
        &["git", "ls-remote", "--heads", "origin"],
        Some(&clone),
        crate::transport::How::read(),
    )
    .expect("the remote answers")
    .stdout;
    assert!(
        !heads.contains("issue-1-merged"),
        "a merged head survived --confirm: {heads}"
    );
    assert!(
        !heads.contains("issue-3-closed"),
        "a closed head survived --confirm: {heads}"
    );
    assert!(
        heads.contains("issue-4-held"),
        "a head a live claim holds was deleted: {heads}"
    );

    // The merged checkout is gone; the held one is not.
    assert!(
        !trees.join("issue-1-merged").exists(),
        "a merged checkout survived --confirm"
    );
    assert!(
        trees.join("issue-4-held").exists(),
        "a held checkout was removed"
    );
    assert!(
        !trees.join("panel-1").exists(),
        "a detached checkout survived --confirm"
    );
    assert!(
        answers
            .iter()
            .any(|answer| answer["branch"] == "issue-1-merged"
                && answer["deleted"] == serde_json::json!(true)),
        "the remote deletion was not read back: {answers:?}"
    );
}
