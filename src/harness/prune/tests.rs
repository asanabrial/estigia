use super::*;

fn observed(kind: Kind, name: &str, sweepable: Option<&str>) -> Observed {
    Observed {
        kind,
        name: name.to_owned(),
        branch: Some(name.to_owned()),
        issue: issue_of_branch(name),
        sweepable: sweepable.map(str::to_owned),
        unpublished: false,
        held: None,
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
    let root = tempfile::tempdir().expect("a fixture root");
    let remote = root.path().join("remote.git");
    let work = root.path().join("work");
    let clone = root.path().join("clone");
    let trees = root.path().join("trees");

    git(
        root.path(),
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
        root.path(),
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

    (root, clone, trees)
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
    assert!(
        !swept
            .iter()
            .any(|entry| entry.kind == Kind::Worktree && entry.name.contains("issue-2-open")),
        "an open checkout was listed: {swept:?}"
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
