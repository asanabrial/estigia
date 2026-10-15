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

fn dirty_trees(main: &Path, trees: &Path, count: usize) -> Vec<PathBuf> {
    (1..=count)
        .map(|n| {
            let tree = add_tree(main, trees, &format!("issue-{n}-dirty"));
            std::fs::write(tree.join("new.txt"), "a\n").unwrap();
            tree
        })
        .collect()
}

fn registered_of(root: &Path, main: &Path) -> Registered {
    branch::registered_worktrees(&context(root, main)).expect("the registry reads")
}

#[test]
fn only_five_dirty_orphans_are_named_and_the_rest_are_counted() {
    let (root, main, trees) = fixture();
    dirty_trees(&main, &trees, 6);

    let notice = orphan_notice(&context(root.path(), &main), &[]).expect("a notice");
    assert!(
        notice.starts_with("6 checkouts no run holds carry uncommitted work: "),
        "{notice}"
    );
    assert!(notice.contains("issue-5-dirty"), "{notice}");
    assert!(
        !notice.contains("issue-6-dirty"),
        "a sixth checkout was named: {notice}"
    );
    assert!(notice.contains("), and 1 more."), "{notice}");
}

#[test]
fn a_dirty_orphan_is_named_with_its_file_count_and_a_detached_one_says_so() {
    let (root, main, trees) = fixture();
    let one = add_tree(&main, &trees, "issue-1-dirty");
    std::fs::write(one.join("a.txt"), "a\n").unwrap();
    std::fs::write(one.join("b.txt"), "b\n").unwrap();
    let detached = trees.join("detached");
    git(
        &main,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            detached.to_str().unwrap(),
        ],
    );
    std::fs::write(detached.join("a.txt"), "a\n").unwrap();

    let notice = orphan_notice(&context(root.path(), &main), &[]).expect("a notice");
    assert!(
        notice.contains("(issue-1-dirty, 2 uncommitted files)"),
        "{notice}"
    );
    assert!(
        notice.contains("(detached, 1 uncommitted file)"),
        "{notice}"
    );
    assert!(!notice.contains(", and "), "{notice}");
}

#[test]
fn checkouts_the_budget_did_not_reach_are_counted_and_never_read_as_clean() {
    let (root, main, trees) = fixture();
    dirty_trees(&main, &trees, 3);
    let registered = registered_of(root.path(), &main);

    let mut asked = 0;
    let partial = orphan_notice_of(&registered, &[], || {
        asked += 1;
        asked > 1
    })
    .expect("a notice");
    assert!(
        partial.starts_with("1 checkout no run holds carries uncommitted work: "),
        "{partial}"
    );
    assert!(
        partial.contains(
            " 2 checkouts could not be read in time, so what they hold is unknown. `estigia changes`"
        ),
        "{partial}"
    );

    let none = orphan_notice_of(&registered, &[], || true).expect("a notice");
    assert!(
        none.starts_with("3 checkouts could not be read in time, so what they hold is unknown."),
        "{none}"
    );
    assert!(!none.contains("uncommitted work"), "{none}");
}

#[test]
fn one_checkout_the_budget_did_not_reach_is_named_in_the_singular() {
    let (root, main, trees) = fixture();
    add_tree(&main, &trees, "issue-1-clean");
    let registered = registered_of(root.path(), &main);

    let notice = orphan_notice_of(&registered, &[], || true).expect("a notice");
    assert!(
        notice.starts_with("1 checkout could not be read in time, so what it holds is unknown."),
        "{notice}"
    );
    assert_eq!(orphan_notice_of(&registered, &[], || false), None);
}

#[test]
fn a_registered_checkout_git_cannot_read_is_counted_as_unread() {
    let (root, main, trees) = fixture();
    let gone = add_tree(&main, &trees, "issue-1-gone");
    let dirty = add_tree(&main, &trees, "issue-2-dirty");
    std::fs::write(dirty.join("new.txt"), "a\n").unwrap();
    std::fs::remove_dir_all(&gone).unwrap();

    let notice = orphan_notice(&context(root.path(), &main), &[]).expect("a notice");
    assert!(
        notice.contains("1 checkout no run holds carries uncommitted work: "),
        "{notice}"
    );
    assert!(
        notice.contains(
            ". 1 checkout could not be read in time, so what it holds is unknown. `estigia changes`"
        ),
        "{notice}"
    );
    assert!(!notice.contains("issue-1-gone"), "{notice}");
}

#[test]
fn the_base_checkout_is_not_named_when_the_session_runs_in_a_linked_checkout() {
    let (root, main, trees) = fixture();
    let linked = add_tree(&main, &trees, "issue-1-session");
    std::fs::write(main.join("base.txt"), "a\n").unwrap();

    let notice = orphan_notice(&context(root.path(), &linked), &[]);
    assert_eq!(notice, None);
}

#[test]
fn a_bare_primary_is_neither_named_nor_counted_as_unread() {
    let (root, main, _trees) = fixture();
    let bare = root.path().join("bare.git");
    git(
        root.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            &crate::test_env::shell_path(&main),
            &crate::test_env::shell_path(&bare),
        ],
    );
    let linked = root.path().join("linked");
    git(
        &bare,
        &[
            "worktree",
            "add",
            "--quiet",
            &crate::test_env::shell_path(&linked),
            "main",
        ],
    );
    assert_eq!(registered_of(root.path(), &linked).len(), 2);

    assert_eq!(orphan_notice(&context(root.path(), &linked), &[]), None);
    std::fs::write(linked.join("new.txt"), "a\n").unwrap();
    let notice = orphan_notice(&context(root.path(), &linked), &[]).expect("a notice");
    assert!(
        notice.starts_with("1 checkout no run holds carries uncommitted work: "),
        "{notice}"
    );
    assert!(!notice.contains("could not be read"), "{notice}");
}

#[test]
fn renamed_added_and_conflicted_files_carry_their_markers() {
    let (_root, main, trees) = fixture();
    let tree = add_tree(&main, &trees, "issue-1-markers");
    git(&tree, &["mv", "kept.txt", "renamed.txt"]);
    std::fs::write(tree.join("added.txt"), "a\nb\nc\n").unwrap();
    git(&tree, &["add", "added.txt"]);

    let files = uncommitted(&tree).expect("the checkout reads");
    assert_eq!(files.len(), 2, "{files:?}");
    let renamed = files.iter().find(|change| change.marker == 'R');
    assert_eq!(
        renamed.map(|change| change.path.as_str()),
        Some("renamed.txt"),
        "{files:?}"
    );
    let added = files.iter().find(|change| change.marker == 'A');
    assert_eq!(
        added.map(|change| (change.path.as_str(), change.added, change.removed)),
        Some(("added.txt", Some(3), Some(0))),
        "{files:?}"
    );

    let (_root, main, trees) = fixture();
    let other = add_tree(&main, &trees, "issue-2-conflict");
    std::fs::write(main.join("kept.txt"), "main side\n").unwrap();
    git(&main, &["commit", "--quiet", "-am", "main side"]);
    std::fs::write(other.join("kept.txt"), "other side\n").unwrap();
    git(&other, &["commit", "--quiet", "-am", "other side"]);
    let merge = std::process::Command::new("git")
        .arg("-C")
        .arg(&other)
        .args(["merge", "--no-edit", "main"])
        .output()
        .expect("git runs");
    assert!(!merge.status.success(), "the merge was meant to conflict");

    let files = uncommitted(&other).expect("the checkout reads");
    let conflicted: Vec<_> = files
        .iter()
        .filter(|change| change.path == "kept.txt")
        .collect();
    assert_eq!(conflicted.len(), 1, "{files:?}");
    assert_eq!(conflicted[0].marker, 'M', "{files:?}");
}

#[test]
fn an_untracked_file_git_or_its_size_cannot_count_has_no_line_count() {
    let (_root, main, trees) = fixture();
    let tree = add_tree(&main, &trees, "issue-1-untracked");
    std::fs::write(tree.join("binary.bin"), b"a\n\0b\n").unwrap();
    std::fs::write(
        tree.join("huge.txt"),
        vec![b'\n'; (COUNTED_BYTES + 1) as usize],
    )
    .unwrap();
    std::fs::write(tree.join("limit.txt"), vec![b'\n'; COUNTED_BYTES as usize]).unwrap();
    std::fs::write(tree.join("unterminated.txt"), "a\nb").unwrap();

    let files = uncommitted(&tree).expect("the checkout reads");
    let added = |name: &str| {
        files
            .iter()
            .find(|change| change.path == name)
            .unwrap_or_else(|| panic!("{name} was omitted: {files:?}"))
            .added
    };
    assert_eq!(added("binary.bin"), None);
    assert_eq!(added("huge.txt"), None);
    assert_eq!(added("limit.txt"), Some(COUNTED_BYTES));
    assert_eq!(added("unterminated.txt"), Some(2));
}

#[test]
fn commits_not_on_the_upstream_are_counted_and_no_upstream_counts_nothing() {
    let (root, main, trees) = fixture();
    let origin = root.path().join("origin.git");
    git(
        root.path(),
        &[
            "init",
            "--quiet",
            "--bare",
            &crate::test_env::shell_path(&origin),
        ],
    );
    git(
        &main,
        &[
            "remote",
            "add",
            "origin",
            &crate::test_env::shell_path(&origin),
        ],
    );
    git(&main, &["push", "--quiet", "-u", "origin", "main"]);
    for n in 1..=2 {
        std::fs::write(main.join("kept.txt"), format!("{n}\n")).unwrap();
        git(&main, &["commit", "--quiet", "-am", &format!("local {n}")]);
    }
    let unpushed = add_tree(&main, &trees, "issue-1-no-upstream");

    let surveyed = survey(&context(root.path(), &main), &[]).expect("the survey reads");
    let ahead = |path: &Path| {
        surveyed
            .iter()
            .find(|tree| crate::paths::same_directory(&tree.path, path))
            .unwrap_or_else(|| panic!("{} was omitted", path.display()))
            .ahead
    };
    assert_eq!(ahead(&main), Some(2));
    assert_eq!(ahead(&unpushed), None);
}
