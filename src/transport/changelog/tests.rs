use super::*;

#[test]
fn the_version_must_open_the_heading_not_merely_appear_in_it() {
    // The live failure this anchor exists for. An entry whose entire point is
    // that 6.9.8 did *not* ship in it was matched for 6.9.8, ahead of the
    // genuine heading below — and the tag would have carried notes describing a
    // different change and explicitly disclaiming the version it was named for.
    let text = "\
# Changelog

### 2026-07-25 — (sin bump de versión) — (sigue en v6.9.8)

Groundwork only.

### v6.9.8 (2026-07-26) — PATCH: the real one

The actual fix.
";
    let found = section(text, "6.9.8").expect("the genuine entry");
    assert!(found.heading.contains("PATCH: the real one"), "{found:?}");
    assert_eq!(found.body, "The actual fix.");
}

#[test]
fn a_pre_release_is_a_different_version() {
    // Rejecting only another digit let a query for `6.9.8` match `v6.9.8-rc1`,
    // because `-` is neither a digit nor a dot — so a release candidate sitting
    // above the real entry would win.
    let text = "## v6.9.8-rc1\n\nA candidate.\n";
    assert_eq!(section(text, "6.9.8"), Err(Trouble::Missing));

    // And the same for a longer number, which a substring search matches.
    let text = "## 1.2.30\n\nAnother.\n";
    assert_eq!(section(text, "1.2.3"), Err(Trouble::Missing));

    // A build suffix too.
    assert_eq!(
        section("## 1.2.3+build\n\nx\n", "1.2.3"),
        Err(Trouble::Missing)
    );
}

#[test]
fn the_shapes_a_changelog_is_allowed_to_take() {
    // Format-tolerant on purpose: conventions vary and this must not impose one.
    for heading in [
        "# 1.2.3",
        "###### v1.2.3",
        "## [1.2.3]",
        "## [v1.2.3] - 2026-01-01",
        "## Version 1.2.3",
        "## release   v1.2.3 — with a description",
    ] {
        let text = format!("{heading}\n\nThe body.\n");
        let found = section(&text, "1.2.3")
            .unwrap_or_else(|error| panic!("{heading:?} was not read: {error:?}"));
        assert_eq!(found.body, "The body.", "{heading:?}");
        // The heading is kept whole: cutting it at the version would drop the
        // date and description its author wrote there.
        assert_eq!(found.heading, heading.trim(), "{heading:?}");
    }
}

#[test]
fn a_sub_heading_stays_inside_the_entry() {
    // The section ends at the next heading of the same or shallower level.
    let text = "\
## v1.2.3

Intro.

### Fixed

A thing.

## v1.2.2

Older.
";
    let found = section(text, "1.2.3").expect("the entry");
    assert!(found.body.contains("### Fixed"), "{found:?}");
    assert!(found.body.contains("A thing."));
    assert!(
        !found.body.contains("Older."),
        "the entry swallowed the next one"
    );
}

#[test]
fn two_headings_for_one_version_is_refused_rather_than_resolved() {
    // Whichever is chosen becomes permanent in an immutable tag, and the topmost
    // is not reliably the real one.
    let text = "## v1.2.3 (draft)\n\nOld.\n\n## v1.2.3\n\nNew.\n";
    match section(text, "1.2.3") {
        Err(Trouble::Ambiguous(headings)) => {
            assert_eq!(headings.len(), 2);
            assert!(headings[0].contains("draft"), "{headings:?}");
        }
        other => panic!("two headings were resolved silently: {other:?}"),
    }
}

#[test]
fn a_heading_with_nothing_under_it_reads_as_an_empty_body() {
    // The caller refuses on this separately: a tag message of one title line is
    // not release notes.
    let text = "## v1.2.3\n\n## v1.2.2\n\nOlder.\n";
    let found = section(text, "1.2.3").expect("the entry");
    assert_eq!(found.body, "");
}
