use super::*;

fn git(at: &Path, arguments: &[&str]) {
    run(
        &["git"]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect::<Vec<_>>(),
        Some(at),
        How::write(),
    )
    .unwrap_or_else(|failure| panic!("git {arguments:?} failed: {}", failure.detail()));
}

fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().expect("a fixture root");
    let main = root.path().join("main");
    let trees = root.path().join("trees");
    std::fs::create_dir_all(&main).expect("a main checkout");
    std::fs::create_dir_all(&trees).expect("a trees directory");
    git(&main, &["init", "--quiet", "-b", "main"]);
    git(&main, &["config", "user.email", "estigia@example.invalid"]);
    git(&main, &["config", "user.name", "Estigia test"]);
    std::fs::write(main.join("kept.txt"), "one\ntwo\n").expect("content");
    std::fs::write(main.join("gone.txt"), "bye\n").expect("content");
    git(&main, &["add", "."]);
    git(&main, &["commit", "--quiet", "-m", "main"]);
    (root, main, trees)
}

fn add_tree(main: &Path, trees: &Path, branch: &str) -> PathBuf {
    let path = trees.join(branch);
    git(
        main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            branch,
            path.to_str().unwrap(),
        ],
    );
    path
}

fn context(root: &Path, main: &Path) -> Context {
    let skill = root.join("skill");
    std::fs::create_dir_all(&skill).expect("a skill root");
    Context::live(skill, main.to_path_buf(), None)
}

#[test]
fn a_dirty_orphan_checkout_is_reported_with_its_markers_and_counts() {
    let (root, main, trees) = fixture();
    let dirty = add_tree(&main, &trees, "issue-5-dirty");
    let clean = add_tree(&main, &trees, "issue-6-held");
    std::fs::write(dirty.join("kept.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    std::fs::remove_file(dirty.join("gone.txt")).unwrap();
    std::fs::write(dirty.join("new.txt"), "a\nb\nc\n").unwrap();

    let mut holder = Run::new("claude-held1234".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(6));
    holder.worktree = Some(clean.clone());

    let trees = survey(&context(root.path(), &main), &[holder]).expect("the survey reads");
    let find = |path: &Path| {
        trees
            .iter()
            .find(|tree| crate::paths::same_directory(&tree.path, path))
            .unwrap_or_else(|| panic!("{} was omitted: {trees:?}", path.display()))
    };

    assert_eq!(trees.len(), 3, "the main checkout and both worktrees");
    let dirty = find(&dirty);
    assert!(dirty.orphan, "no run holds it");
    assert_eq!(dirty.branch.as_deref(), Some("issue-5-dirty"));
    assert_eq!(dirty.head, Some(Head::Branch));
    let by = |marker: char| dirty.files.iter().find(|change| change.marker == marker);
    let modified = by('M').expect("the edit is a modification");
    assert_eq!(
        (modified.path.as_str(), modified.added, modified.removed),
        ("kept.txt", Some(2), Some(0))
    );
    assert_eq!(by('D').expect("the removal").path, "gone.txt");
    let untracked = by('U').expect("the new file");
    assert_eq!(
        (untracked.path.as_str(), untracked.added),
        ("new.txt", Some(3))
    );
    assert!(
        dirty.modified.is_some(),
        "a dirty file has a modification time"
    );

    let held = find(&clean);
    assert!(!held.orphan);
    assert_eq!(
        held.holder,
        Some(Holder {
            run_id: "claude-held1234".to_owned(),
            issue: Some(6)
        })
    );
    assert!(held.files.is_empty());
    assert!(find(&main).orphan, "the main checkout is listed too");
}

#[test]
fn a_checkout_git_cannot_read_is_reported_unreadable_and_not_omitted() {
    let (root, main, _trees) = fixture();
    let plain = root.path().join("not-a-checkout");
    std::fs::create_dir_all(&plain).unwrap();
    let mut holder = Run::new("claude-lost1234".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(7));
    holder.worktree = Some(plain.clone());

    let trees = survey(&context(root.path(), &main), &[holder]).expect("the survey reads");
    let lost = trees
        .iter()
        .find(|tree| crate::paths::same_directory(&tree.path, &plain))
        .unwrap_or_else(|| panic!("an unreadable checkout was omitted: {trees:?}"));
    assert!(lost.unreadable.is_some(), "{lost:?}");
    assert!(lost.files.is_empty());
}

#[test]
fn a_checkout_with_no_commits_yet_says_so() {
    let (root, main, _trees) = fixture();
    let fresh = root.path().join("fresh");
    std::fs::create_dir_all(&fresh).unwrap();
    git(&fresh, &["init", "--quiet", "-b", "main"]);
    std::fs::write(fresh.join("first.txt"), "x\n").unwrap();
    let mut holder = Run::new("claude-fresh123".to_owned());
    holder.worktree = Some(fresh.clone());

    let trees = survey(&context(root.path(), &main), &[holder]).expect("the survey reads");
    let fresh = trees
        .iter()
        .find(|tree| crate::paths::same_directory(&tree.path, &fresh))
        .expect("listed");
    assert_eq!(fresh.head, Some(Head::NoCommits));
    assert_eq!(fresh.files.len(), 1);
}

#[test]
fn a_dirty_orphan_checkout_is_named_with_the_command_that_lists_it() {
    let (root, main, trees) = fixture();
    let dirty = add_tree(&main, &trees, "issue-5-dirty");
    let clean = add_tree(&main, &trees, "issue-6-clean");
    let held = add_tree(&main, &trees, "issue-7-held");
    std::fs::write(
        dirty.join("new.txt"),
        "a
",
    )
    .unwrap();
    std::fs::write(
        held.join("new.txt"),
        "a
",
    )
    .unwrap();
    std::fs::write(
        main.join("base.txt"),
        "a
",
    )
    .unwrap();
    let mut holder = Run::new("claude-held1234".to_owned());
    holder.issue = Some(crate::identity::Identity::Number(7));
    holder.worktree = Some(held);

    let notice = orphan_notice(&context(root.path(), &main), &[holder]).expect("a notice");
    assert!(
        notice.contains(&crate::test_env::shell_path(&dirty)),
        "{notice}"
    );
    assert!(notice.contains("issue-5-dirty"), "{notice}");
    assert!(notice.contains("estigia changes"), "{notice}");
    assert!(!notice.contains("issue-6-clean"), "{notice}");
    assert!(
        !notice.contains("issue-7-held"),
        "a held checkout is not an orphan: {notice}"
    );
    assert!(
        !notice.contains("base.txt") && !notice.contains(&crate::test_env::shell_path(&main)),
        "{notice}"
    );
    drop(clean);
}

#[test]
fn no_dirty_orphan_means_no_notice() {
    let (root, main, trees) = fixture();
    add_tree(&main, &trees, "issue-6-clean");
    assert_eq!(orphan_notice(&context(root.path(), &main), &[]), None);
}

#[test]
fn a_dirty_claim_checkout_is_named_at_done_and_release() {
    let (_root, main, trees) = fixture();
    let own = add_tree(&main, &trees, "issue-8-own");
    assert_eq!(held_notice(&own), None, "a clean checkout says nothing");
    std::fs::write(
        own.join("kept.txt"),
        "changed
",
    )
    .unwrap();
    std::fs::write(
        own.join("new.txt"),
        "a
",
    )
    .unwrap();
    let notice = held_notice(&own).expect("a notice");
    assert!(
        notice.contains("kept.txt") && notice.contains("new.txt"),
        "{notice}"
    );
    assert!(notice.contains(&own.display().to_string()), "{notice}");
    assert!(notice.contains("estigia changes"), "{notice}");
    assert_eq!(held_notice(&own.join("not-there")), None);
}
