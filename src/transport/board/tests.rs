use super::*;

fn context() -> Context {
    Context {
        skill_dir: std::path::PathBuf::from("/skill"),
        repo_dir: std::path::PathBuf::from("/repo"),
        config: Vec::new(),
        repo: None,
    }
}

fn meta_with(columns: &[(&str, &str)]) -> Meta {
    Meta {
        project_id: "P".to_owned(),
        field_id: "F".to_owned(),
        columns: columns
            .iter()
            .enumerate()
            .map(|(at, (name, description))| Column {
                id: format!("opt{at}"),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn a_spec_that_is_not_a_board_disables_the_mirror_without_failing() {
    // Two situations, and this held them as one. Silence is right for the
    // first and was wrong for the second: `acme` sat in the list beside `none`,
    // so a spec the operator **set** disabled the mirror with nothing said.
    for spec in ["", "none", "None"] {
        let board = Board::parse(spec, &context(), true);
        assert!(!board.enabled, "{spec:?} was read as a board");
        assert_eq!(board.owner, None, "{spec:?}");
        assert_eq!(
            board.skip_reason, None,
            "{spec:?} is somebody declining a board, and was answered with a complaint"
        );
    }

    // Asked for and unaddressable. `config list` reports it set either way, so
    // the mirror going off without a word is the operator's setting being
    // ignored in silence — the two shapes below are what people actually
    // paste: the project's number, and its node id.
    for spec in ["acme", "7", "PVT_kwDOABCDE"] {
        let board = Board::parse(spec, &context(), true);
        assert!(!board.enabled, "{spec:?} was read as a board");
        assert!(
            board
                .skip_reason
                .as_deref()
                .is_some_and(|why| why.contains(spec) && why.contains("<owner>/<number>")),
            "{spec:?} disabled the mirror and said nothing: {:?}",
            board.skip_reason
        );
    }

    let board = Board::parse("acme/7", &context(), true);
    assert!(board.enabled);
    assert_eq!(board.owner.as_deref(), Some("acme"));
    assert_eq!(board.number, Some(7));

    // Addressed and unparseable: disabled, the owner survives, and the reason is
    // recorded — the label is the authoritative store, so this is a skip and
    // never an error.
    let broken = Board::parse("acme/seven", &context(), true);
    assert!(!broken.enabled);
    assert_eq!(broken.owner.as_deref(), Some("acme"));
    assert!(broken.skip_reason.is_some());
}

#[test]
fn a_column_matches_by_name_or_by_description() {
    // The rule that keeps `done` working. Real boards describe `Analysis`..
    // `Blocked` with their exact `status:*` labels and then describe `Done` as
    // `closed`, because that column also tracks the tracker's own closed flag —
    // so a matcher demanding `status:done` would fail on the one transition that
    // matters most.
    let meta = meta_with(&[
        ("In Progress", "status:in-progress"),
        ("Done", "closed"),
        ("Ready", ""),
        ("Blocked", "blocked"),
    ]);

    // By description.
    assert_eq!(
        Board::column_for(&meta, "in-progress").map(|c| c.name),
        Some("In Progress".to_owned())
    );
    // By name, with the space folded to a dash — which is also how `In Progress`
    // would be found if its description were empty.
    assert_eq!(
        Board::column_for(&meta, "ready").map(|c| c.name),
        Some("Ready".to_owned())
    );
    // By a bare description that is the state itself.
    assert_eq!(
        Board::column_for(&meta, "blocked").map(|c| c.name),
        Some("Blocked".to_owned())
    );
    // `done` is found by *name*, because its description says `closed`.
    assert_eq!(
        Board::column_for(&meta, "done").map(|c| c.name),
        Some("Done".to_owned())
    );
    assert_eq!(Board::column_for(&meta, "review"), None);
}

#[test]
fn a_disabled_board_reports_a_skip_rather_than_attempting_anything() {
    // The property the whole module rests on: `transition` mirrors before it
    // moves the label, so this call must never be able to stop what follows.
    let mut board = Board::parse("none", &context(), true);
    let answer = board.set_status(12, "review", "acme/repo");
    assert_eq!(answer["attempted"], false);
    assert_eq!(answer["skipped"], "no board configured");
    assert_eq!(board.read_status(12), None);
}

#[test]
fn the_cache_lives_in_a_directory_this_account_owns() {
    // Not the shared temp root. On a host with a world-writable temp directory
    // another local account could pre-plant this file and point the mutation at
    // project and field ids of its choosing — bounded by what this token can
    // already write, so not a privilege escalation, but a confused deputy
    // writing to the wrong board.
    let board = Board::parse("acme/7", &context(), true);
    let path = board.cache_path();
    let parent = path.parent().expect("the cache has a directory");
    assert_ne!(
        parent,
        std::env::temp_dir(),
        "the cache sits directly in the shared temp root"
    );
    assert!(
        parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("issue-flow-")),
        "{}",
        parent.display()
    );
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("board-acme-7.json")
    );
}

#[test]
fn resolved_ids_survive_a_round_trip_through_the_cache_format() {
    // The cache is read by a later process, so its shape is a contract with
    // itself. A field lost here is a mirror that silently re-queries forever, or
    // worse, one that addresses a column by an id it no longer has.
    let meta = meta_with(&[("Done", "closed"), ("Ready", "status:ready")]);
    let written = write_meta(&meta);
    assert_eq!(read_meta(&written), Some(meta));

    // And a cache that is not the shape this version writes is simply not used.
    assert_eq!(read_meta(&serde_json::json!({ "project_id": "P" })), None);
    assert_eq!(read_meta(&serde_json::Value::Null), None);
}

#[test]
fn a_clock_that_will_not_answer_does_not_make_the_mirror_eternally_fresh() {
    // The check was `now_seconds() - at < CACHE_SECONDS` against a clock that
    // defaulted to zero when it could not be read. Zero minus any real
    // timestamp is a large negative number, and every large negative number is
    // under a day — so a machine whose clock would not answer held a mirror
    // that never went stale, and went on transitioning issues through column
    // ids the board may have stopped using.
    let now = 1_700_000_000.0;

    // The premise: this is a real cache hit, and this is a real expiry.
    assert!(super::mirror_is_fresh(Some(now - 60.0), Some(now)));
    assert!(!super::mirror_is_fresh(
        Some(now - super::CACHE_SECONDS - 1.0),
        Some(now)
    ));

    // A clock that will not say what time it is: re-query, both ways round.
    assert!(
        !super::mirror_is_fresh(Some(now - 60.0), None),
        "a clock nobody could read made a cached mirror fresh forever"
    );
    assert!(
        !super::mirror_is_fresh(None, Some(now)),
        "an entry stamped by a clock nobody could read was trusted"
    );

    // And the arithmetic that made it fail open, named: the old expression on
    // an unreadable clock.
    assert!(
        0.0 - (now - 60.0) < super::CACHE_SECONDS,
        "the epoch no longer reads as inside the window — this test's premise has moved"
    );

    // What this does not prove: that the reader still calls `fresh`. It is the
    // type that carries that — `now_seconds` returns an `Option` now, so the
    // old inline arithmetic cannot come back without somebody writing the
    // default out in full. The gate's clock got a structural guard instead,
    // because there a wrong answer opens a door rather than staling a mirror.
}

/// A mirror stamped ahead of the clock is not fresh.
///
/// The window was `now - at < CACHE_SECONDS`, and every negative age is under
/// it — so a mirror written while the clock was a year ahead stayed fresh for a
/// year. What it holds is the board's project id, field id and **option ids**,
/// and holding those past a rename or a deletion is the failure this function's
/// own doc names: issues transitioned through columns the board has stopped
/// using.
///
/// Both ends are asserted, and the floor is the ordinary answer: a fix that
/// simply stopped trusting the mirror would pass the future case and cost a
/// query on every call.
#[test]
fn a_mirror_stamped_ahead_of_the_clock_is_not_fresh() {
    let now = 1_800_000_000.0_f64;

    assert!(
        super::mirror_is_fresh(Some(now - 3_600.0), Some(now)),
        "an hour old is inside the day-long window and has to stay there"
    );
    assert!(
        super::mirror_is_fresh(Some(now), Some(now)),
        "written this instant is not ahead of the clock"
    );
    assert!(
        !super::mirror_is_fresh(Some(now - 172_800.0), Some(now)),
        "two days old is past the window"
    );
    assert!(
        !super::mirror_is_fresh(Some(now + 1.0), Some(now)),
        "a second ahead of the clock is a stamp this machine did not make now"
    );
    assert!(
        !super::mirror_is_fresh(Some(now + 31_536_000.0), Some(now)),
        "a year ahead was fresh for a year"
    );
}

#[test]
fn a_card_from_another_repository_is_not_this_issue() {
    let foreign = serde_json::json!({"id": "ITEM", "content": {"number": 73, "repository": {"nameWithOwner": "asanabrial/investora"}}});
    assert_eq!(
        pick_item(std::slice::from_ref(&foreign), 73, "asanabrial/estigia"),
        ItemPick::Foreign {
            belongs_to: "asanabrial/investora".to_owned()
        }
    );
    let ours = serde_json::json!({"id": "OURS", "content": {"number": 73, "repository": {"nameWithOwner": "asanabrial/estigia"}}});
    assert_eq!(
        pick_item(&[foreign, ours], 73, "asanabrial/estigia"),
        ItemPick::Ours {
            id: "OURS".to_owned()
        }
    );
    assert_eq!(pick_item(&[], 73, "asanabrial/estigia"), ItemPick::Absent);
}

#[test]
fn a_foreign_item_report_includes_the_board_key() {
    let report = foreign_item_report(
        73,
        "asanabrial/12",
        "asanabrial/investora",
        "asanabrial/estigia",
    );
    assert!(
        report
            .as_object()
            .is_some_and(|keys| keys.contains_key("board")),
        "the refusal has to name the board that held the foreign card"
    );
    assert_eq!(report["board"], "asanabrial/12");
    assert_eq!(report["reason"], "board-item-foreign-repository");
    assert_eq!(
        report["action"],
        "estigia config set --repo \"Project board\" \"none\""
    );
    let detail = report["detail"].as_str().expect("detail is a sentence");
    assert!(detail.contains("73"), "{detail}");
    assert!(detail.contains("asanabrial/12"), "{detail}");
    assert!(detail.contains("asanabrial/investora"), "{detail}");
    assert!(detail.contains("asanabrial/estigia"), "{detail}");
}

/// A card that does not say where it comes from is not this repository's.
///
/// The picker was written `belongs.is_empty() || belongs == home`, so a content
/// node carrying no `repository` — a draft item, or a field the token cannot
/// read — was taken as ours and mirrored. That is the defect this whole change
/// exists to stop, surviving in the one case `pick_item`'s own doc comment
/// calls out: *unknown repository is not clearance*. It also left the
/// `an unnamed repository` arm unreachable, which is what a dead branch usually
/// means — the guard above it says the opposite of what it was written for.
///
/// Turn the fix off by restoring the `belongs.is_empty() ||` prefix and this
/// reddens with `Ours`.
#[test]
fn a_card_that_names_no_repository_is_not_ours() {
    let node = serde_json::json!({ "id": "PVTI_unnamed", "content": { "number": 73 } });
    assert_eq!(
        pick_item(&[node], 73, "asanabrial/estigia"),
        ItemPick::Foreign {
            belongs_to: "an unnamed repository".to_owned()
        },
        "a card that does not name its repository was taken as ours"
    );
}

/// No identity for this repository means no card can be matched to it.
///
/// `board_home` answers through `unwrap_or_default()`, so a failed identity read
/// arrives as an empty string. Before the guard this test holds, that emptiness
/// reached the picker, where — under the corrected rule — every card becomes
/// foreign and the mirror reports a sentence about the *card*. What is true is
/// that the question could not be asked.
#[test]
fn an_unreadable_repository_identity_mirrors_nothing() {
    let node = serde_json::json!({
        "id": "PVTI_ours",
        "content": { "number": 73, "repository": { "nameWithOwner": "asanabrial/estigia" } }
    });
    assert_eq!(
        pick_item(&[node], 73, ""),
        ItemPick::Foreign {
            belongs_to: "asanabrial/estigia".to_owned()
        },
        "an empty home matched a card, so a failed identity read could still move one"
    );
}
