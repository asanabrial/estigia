use super::*;

fn manifest(entries: &[(&str, &str, &str)]) -> manifest::Manifest {
    entries
        .iter()
        .map(|(path, mode, blob)| ((*path).to_owned(), ((*mode).to_owned(), (*blob).to_owned())))
        .collect()
}

#[test]
fn a_supplied_target_that_covers_less_than_the_branch_delivers_is_named_path_by_path() {
    // The whole point of `--native-start`, and the port had no comparison at
    // all while the MCP server offered agents the argument. The three lists are
    // not interchangeable: `unreviewed` is the one that ships without
    // authority, and reporting it as `differing` would read as a rebase.
    let target = manifest(&[
        ("src/a.rs", "100644", "aaa"),
        ("src/b.rs", "100644", "bbb"),
        ("src/c.rs", "100644", "ccc"),
    ]);
    let claimed = manifest(&[
        ("src/a.rs", "100644", "aaa"),
        ("src/b.rs", "100755", "bbb"),
        ("src/gone.rs", "100644", "ddd"),
    ]);
    let found = difference(&target, &claimed);
    assert_eq!(found.unreviewed, [&"src/c.rs".to_owned()]);
    assert_eq!(found.not_delivered, [&"src/gone.rs".to_owned()]);
    assert_eq!(
        found.differing,
        [&"src/b.rs".to_owned()],
        "a mode flip is a difference: an executable bit is half of what a delivery is"
    );

    // The same target twice is no difference at all, or every review is a stop.
    let same = difference(&target, &target);
    assert!(
        same.unreviewed.is_empty() && same.not_delivered.is_empty() && same.differing.is_empty(),
        "a target disagreed with itself"
    );
}

#[test]
fn an_unreadable_supplied_target_is_a_read_failure_and_never_a_mismatch() {
    // Both are refusals, and saying the wrong one is a lie about what happened:
    // a mismatch says the reviewer was shown the wrong thing. An unreadable
    // file says nothing was compared.
    let root = std::env::temp_dir().join("estigia-native-start-probe");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("a directory");

    let absent = root.join("absent.json");
    assert!(matches!(supplied_manifest(&absent), Err(Failure::Read(_))));

    for (name, body) in [
        ("not-json.json", "{"),
        ("no-list.json", "{\"manifest\": {}}"),
        (
            "malformed.json",
            "[{\"path\": \"a\", \"mode\": \"100644\"}]",
        ),
    ] {
        let path = root.join(name);
        std::fs::write(&path, body).expect("the probe is written");
        assert!(
            matches!(supplied_manifest(&path), Err(Failure::Read(_))),
            "{name} was not read as a read failure"
        );
    }

    // Both shapes the transport accepts: a bare list, and one under `manifest`.
    let entry = "[{\"path\": \"src/a.rs\", \"mode\": \"100644\", \"blob\": \"aaa\"}]";
    for (name, body) in [
        ("bare.json", entry.to_owned()),
        ("wrapped.json", format!("{{\"manifest\": {entry}}}")),
    ] {
        let path = root.join(name);
        std::fs::write(&path, &body).expect("the probe is written");
        assert_eq!(
            supplied_manifest(&path).expect("a manifest"),
            manifest(&[("src/a.rs", "100644", "aaa")]),
            "{name} did not read back"
        );
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_dirty_target_cannot_be_bound_to_a_published_head() {
    let dirty = serde_json::json!({"uncommitted": ["src/lib.rs"]});
    assert!(
        dirty["uncommitted"]
            .as_array()
            .is_some_and(|paths| !paths.is_empty()),
        "the fixture stopped describing a dirty target"
    );

    let source = include_str!("../target.rs");
    let body = source
        .split_once("pub fn clean_target(")
        .expect("the clean publication boundary exists")
        .1;
    assert!(
        body.contains("dirty-review-target") && body.contains("expected_target("),
        "publication no longer derives the complete target and refuses its dirty suffix"
    );
}

#[test]
fn a_clean_publication_snapshot_is_bounded_by_the_same_head_and_empty_status() {
    let head = "a".repeat(40);
    let target = serde_json::json!({"head": head, "uncommitted": []});
    coherent_clean_snapshot(&head, "", &target, &head, "").expect("one clean snapshot");

    let moved = coherent_clean_snapshot(&head, "", &target, &"b".repeat(40), "")
        .expect_err("HEAD moved while the manifest was read");
    assert_eq!(
        moved.envelope()["reason"],
        "review-target-moved-during-read"
    );

    for (before, after) in [(" M src/lib.rs\0", ""), ("", "?? later\0")] {
        let dirty = coherent_clean_snapshot(&head, before, &target, &head, after)
            .expect_err("dirty at one boundary");
        assert_eq!(dirty.envelope()["reason"], "dirty-review-target");
    }

    let mixed = serde_json::json!({"head": head, "uncommitted": ["src/lib.rs"]});
    let dirty =
        coherent_clean_snapshot(&head, "", &mixed, &head, "").expect_err("the inner read saw dirt");
    assert_eq!(dirty.envelope()["reason"], "dirty-review-target");
}
