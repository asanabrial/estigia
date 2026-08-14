use super::*;

fn manifest(entries: &[(&str, &str, &str)]) -> Manifest {
    entries
        .iter()
        .map(|(mode, blob, path)| ((*path).to_owned(), ((*mode).to_owned(), (*blob).to_owned())))
        .collect()
}

#[test]
fn the_mode_is_part_of_what_is_delivered() {
    // A file that becomes executable, or a regular file replaced by a symlink,
    // changes what ships while every byte of content stays identical. A digest
    // over blobs alone would call that no change at all.
    let regular = manifest(&[("100644", "aaa", "run.sh")]);
    let executable = manifest(&[("100755", "aaa", "run.sh")]);
    let link = manifest(&[("120000", "aaa", "run.sh")]);
    assert_ne!(manifest_digest(&regular), manifest_digest(&executable));
    assert_ne!(manifest_digest(&executable), manifest_digest(&link));
}

#[test]
fn the_digest_is_independent_of_the_order_entries_arrived_in() {
    // It is compared across machines, so it cannot depend on how a listing
    // happened to be walked.
    let one = manifest(&[("100644", "a", "x"), ("100644", "b", "y")]);
    let other = manifest(&[("100644", "b", "y"), ("100644", "a", "x")]);
    assert_eq!(manifest_digest(&one), manifest_digest(&other));

    // And it changes for a rename and for a deletion, not only for content.
    let renamed = manifest(&[("100644", "a", "z"), ("100644", "b", "y")]);
    let deleted = manifest(&[("100644", "a", "x")]);
    assert_ne!(manifest_digest(&one), manifest_digest(&renamed));
    assert_ne!(manifest_digest(&one), manifest_digest(&deleted));
}

#[test]
fn a_listing_that_cannot_be_read_is_refused_rather_than_shrunk() {
    // A target that quietly loses entries reports success for a review of less
    // than what ships — the exact failure this file exists to prevent.
    assert_eq!(
        tree_manifest(&["100644 blob aaa	src/x.rs"]).map(|m| m.len()),
        Ok(1)
    );
    assert!(matches!(
        tree_manifest(&["100644 blob aaa"]),
        Err(Trouble::Unreadable(_))
    ));
    assert!(matches!(
        tree_manifest(&["040000 tree bbb	src"]),
        Err(Trouble::Unreadable(_))
    ));
    assert!(matches!(
        tree_manifest(&["100644 blob aaa	x", "100755 blob bbb	x"]),
        Err(Trouble::Duplicate(_))
    ));
    // An empty answer is not a delivery target.
    assert_eq!(tree_manifest(&[]), Err(Trouble::Empty));
    assert_eq!(tree_manifest(&[""]), Err(Trouble::Empty));
}

#[test]
fn the_target_is_the_commit_plus_whatever_is_still_uncommitted() {
    // Reviewing only the committed prefix, or only the dirty suffix, both
    // authorise something other than the delivery.
    let committed = manifest(&[
        ("100644", "aaa", "kept.rs"),
        ("100644", "bbb", "edited.rs"),
        ("100644", "ccc", "removed.rs"),
    ]);
    let hash = |path: &str| Some(("100644".to_owned(), format!("new-{path}")));
    let status = [
        (" M".to_owned(), "edited.rs".to_owned()),
        (" D".to_owned(), "removed.rs".to_owned()),
        ("??".to_owned(), "added.rs".to_owned()),
    ];
    let target = overlay_worktree(&committed, &status, hash).expect("it overlays");

    assert_eq!(target.len(), 3);
    assert_eq!(target.get("kept.rs"), committed.get("kept.rs"));
    assert_eq!(
        target.get("edited.rs"),
        Some(&("100644".to_owned(), "new-edited.rs".to_owned()))
    );
    assert!(!target.contains_key("removed.rs"));
    assert!(target.contains_key("added.rs"));
}

#[test]
fn a_path_that_vanished_between_the_status_and_the_hash_is_dropped() {
    let committed = manifest(&[("100644", "aaa", "gone.rs")]);
    let target = overlay_worktree(
        &committed,
        &[(" M".to_owned(), "gone.rs".to_owned())],
        |_| None,
    )
    .expect("it overlays");
    assert!(target.is_empty());
}

#[test]
fn a_status_code_this_does_not_know_is_refused_and_never_skipped() {
    // Silently ignoring one would drop a changed path from the target and
    // report success for a review of less than what ships.
    let committed = manifest(&[("100644", "aaa", "x.rs")]);
    assert!(matches!(
        overlay_worktree(&committed, &[("!!".to_owned(), "x.rs".to_owned())], |_| {
            None
        }),
        Err(Trouble::Unreadable(_))
    ));
    assert!(matches!(
        overlay_worktree(&committed, &[(" M".to_owned(), String::new())], |_| None),
        Err(Trouble::Unreadable(_))
    ));
}
