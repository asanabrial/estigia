use super::*;

/// A takeover shows what it claims, or it does not count.
///
/// Measured by mutation: making `reclaim_proof_is_valid` answer `true` for
/// every event left the whole suite green. It refuses three things and not one
/// of them was crossed — so evidence written for one takeover would have
/// justified another, and a reclaim naming nothing it displaces would have
/// landed on whoever happened to be holding.
///
/// This is the authority the whole crate is about: taking an issue away from a
/// live run. The flag that asks for it was unmeasured too, one round earlier
/// (`a_switch_nobody_passed_is_off`); this is the evidence behind the flag.
#[test]
fn a_reclaim_that_shows_less_than_it_claims_is_not_valid() {
    let evidence = "f".repeat(64);
    let bound = |event: &Event| {
        forced_reclaim_hash(
            event.operation_id.as_deref(),
            event.evidence_body_hash.as_deref().unwrap_or_default(),
            &event.run_id,
            event.runtime.as_deref(),
            event.horizon.as_deref(),
            event.from.as_deref(),
            event.from_operation.as_deref(),
        )
    };
    let forced = |body: Option<&str>, hash: Option<&str>| Event {
        created_at: "2026-07-26T00:54:00Z".to_owned(),
        position: 1,
        run_id: "claude-aaaaaaaaaaaa".to_owned(),
        runtime: Some("claude".to_owned()),
        horizon: Some("2099-01-01T00:00Z".to_owned()),
        kind: "reclaim".to_owned(),
        from: Some("claude-bbbbbbbbbbbb".to_owned()),
        operation_id: Some("0".repeat(32)),
        from_operation: Some("1".repeat(32)),
        evidence_hash: hash.map(ToOwned::to_owned),
        evidence_body_hash: body.map(ToOwned::to_owned),
        forced: true,
        evidence_required: true,
        marker_index: Some(0),
        comment: Comment {
            id: Some("IC_1".to_owned()),
            created_at: "2026-07-26T00:54:00Z".to_owned(),
            body: String::new(),
            viewer_did_author: true,
            includes_created_edit: false,
        },
    };

    // The floor first: a takeover that shows everything is valid, or every
    // assertion below would pass against a function that refuses everything.
    let mut whole = forced(Some(&evidence), None);
    whole.evidence_hash = Some(bound(&whole));
    assert!(
        reclaim_proof_is_valid(&whole),
        "a forced reclaim carrying bound evidence was refused"
    );

    // Forced, evidence required, and nothing written under the heading.
    assert!(
        !reclaim_proof_is_valid(&forced(None, Some(&bound(&whole)))),
        "a forced reclaim with no evidence body was accepted"
    );

    // Evidence that is not bound to *this* event: the same digest, declared
    // under a hash minted for a different takeover.
    let mut borrowed = forced(Some(&evidence), None);
    borrowed.evidence_hash = Some(bound(&{
        let mut other = whole.clone();
        other.from = Some("claude-cccccccccccc".to_owned());
        other
    }));
    assert!(
        !reclaim_proof_is_valid(&borrowed),
        "evidence minted for another takeover justified this one"
    );

    // And a modern reclaim that names no epoch displaces whatever is there.
    let mut unnamed = whole;
    unnamed.from_operation = None;
    unnamed.evidence_hash = Some(bound(&unnamed));
    assert!(
        !reclaim_proof_is_valid(&unnamed),
        "a reclaim that names nothing it displaces was accepted"
    );
}

/// An operation id reserves one of the kinds this table names, and no other.
///
/// Measured by mutation: answering `true` for every word left the suite green.
/// Both readers of this predicate walk markers written into issue comments —
/// text that arrives from the tracker rather than from here — so a kind nobody
/// declared would have reserved an epoch and taken part in the reduction.
#[test]
fn a_kind_nobody_declared_reserves_no_operation() {
    // The floor: every kind the table names is one, or the refusals below would
    // pass against a predicate that says no to everything.
    for (declared, _) in OPERATION_FIELDS {
        assert!(
            is_operation_kind(declared),
            "`{declared}` is in the table and is not read as a kind"
        );
    }
    for outsider in ["", "claimed", "CLAIM", "reclaim ", "release", "hola"] {
        assert!(
            !is_operation_kind(outsider),
            "`{outsider}` is not a kind this table names and was read as one"
        );
    }
}

#[test]
fn the_two_precisions_compare_as_instants_and_not_as_strings() {
    // The defect this whole type exists for. As strings, `00:54:00Z` sorts
    // BEFORE `00:54Z`, because '0' precedes 'Z' — so a run that had just spoken
    // would be read as silent and lose its claim.
    let seconds = "2026-07-26T00:54:00Z";
    let minutes = "2026-07-26T00:54Z";
    assert!(seconds < minutes, "the string ordering is the trap");
    assert_eq!(
        parse_stamp(seconds),
        parse_stamp(minutes),
        "the same instant read two ways"
    );
    assert!(parse_stamp("2026-07-26T00:54:01Z") > parse_stamp(minutes));
}

#[test]
fn a_stamp_that_is_not_one_orders_first_rather_than_by_accident() {
    // A horizon is free text a person may have written in prose. It has to
    // compare as *unknown*, not as whatever its letters happen to sort as.
    for value in [
        "",
        "soon",
        "tomorrow",
        "2026-13-01T00:00Z",
        // Not `T24:00Z`: see below.
        "2026-07-26T24:01Z",
    ] {
        assert_eq!(parse_stamp(value), None, "{value:?} parsed");
        assert_eq!(stamp_order(value), UNKNOWN, "{value:?}");
    }
    assert!(stamp_order("2026-07-26T00:54Z") > UNKNOWN);

    // A bare date IS readable — midnight on that day — and this test used to
    // assert the opposite until the differential said otherwise. Calling a
    // written horizon unreadable orders it first, which reads a live claim as
    // expired.
    assert_eq!(parse_stamp("2026-07-26"), parse_stamp("2026-07-26T00:00Z"));

    // And hour 24 is ISO-8601's end of day, which is the same mistake in the
    // same list: this asserted it unreadable until the differential asked the
    // transport. `datetime.fromisoformat` reads `T24:00Z` as the next day's
    // midnight and refuses `T24:01`, so both are here — one of each.
    //
    // Reading it as unreadable is not a small difference. `stamp_rank` puts an
    // unreadable stamp last, so a run whose horizon is written that way loses a
    // race on this side and wins it on the transport's.
    assert_eq!(
        parse_stamp("2026-07-26T24:00Z"),
        parse_stamp("2026-07-27T00:00Z"),
        "the end of a day is the beginning of the next one"
    );
    assert_eq!(
        parse_stamp("2026-07-26T24:00:00Z"),
        parse_stamp("2026-07-27T00:00Z")
    );
}

#[test]
fn a_deadline_takes_a_horizon_already_behind_the_acquisition_as_written() {
    let at = |value: &str| parse_stamp(value).expect("a stamp");
    let acquired = at("2026-07-26T10:00Z");
    let behind = at("2026-07-26T09:00Z");

    // Declared behind the acquisition: taken as written. Silence cannot extend
    // what was never granted, even if the run has spoken since.
    assert_eq!(
        ownership_deadline(Some(behind), Some(acquired), Some(at("2026-07-26T11:00Z"))),
        Some(behind)
    );

    // Ahead of it, and the run has spoken since: whichever reaches further.
    let ahead = at("2026-07-26T20:00Z");
    let spoke = at("2026-07-26T11:00Z");
    assert_eq!(
        ownership_deadline(Some(ahead), Some(acquired), Some(spoke)),
        Some(ahead),
        "the horizon reached further"
    );
    assert_eq!(
        ownership_deadline(Some(at("2026-07-26T12:00Z")), Some(acquired), Some(spoke)),
        Some(spoke + 4 * 60 * 60),
        "the activity window reached further"
    );

    // Either one alone.
    assert_eq!(ownership_deadline(Some(ahead), None, None), Some(ahead));
    assert_eq!(
        ownership_deadline(None, None, Some(spoke)),
        Some(spoke + 4 * 60 * 60)
    );
    assert_eq!(ownership_deadline(None, None, None), None);
}

#[test]
fn a_legacy_marker_passes_and_a_half_written_modern_one_does_not() {
    let mark = |pairs: &[(&str, &str)]| -> super::super::markers::Marker {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    };
    let operation = "0123456789abcdef0123456789abcdef";

    // No operation attributes at all: written before operations existed, and
    // rejecting it would erase ownership somebody genuinely holds.
    assert!(valid_acquisition_marker(&mark(&[
        ("kind", "claim"),
        ("run-id", "claude-abcd1234")
    ])));

    // Carrying one means claiming the newer contract, so all of it is required.
    assert!(valid_acquisition_marker(&mark(&[
        ("kind", "claim"),
        ("run-id", "claude-abcd1234"),
        ("op-id", operation),
        ("runtime", "claude"),
        ("horizon", "2026-07-26T00:54Z"),
    ])));
    // Dropping `op-id` does **not** fail: with no operation attribute left the
    // marker is legacy again, which is the compatibility the rule is built on.
    assert!(valid_acquisition_marker(&mark(&[
        ("kind", "claim"),
        ("run-id", "claude-abcd1234"),
        ("runtime", "claude"),
    ])));

    for missing in ["runtime", "horizon"] {
        let mut attributes = vec![
            ("kind", "claim"),
            ("run-id", "claude-abcd1234"),
            ("op-id", operation),
            ("runtime", "claude"),
            ("horizon", "2026-07-26T00:54Z"),
        ];
        attributes.retain(|(key, _)| *key != missing);
        assert!(
            !valid_acquisition_marker(&mark(&attributes)),
            "a marker without {missing} passed"
        );
    }

    // A reclaim needs what it displaces, which a claim does not.
    let reclaim = |extra: &[(&str, &str)]| {
        let mut attributes = vec![
            ("kind", "reclaim"),
            ("run-id", "claude-abcd1234"),
            ("op-id", operation),
            ("runtime", "claude"),
            ("horizon", "2026-07-26T00:54Z"),
        ];
        attributes.extend_from_slice(extra);
        valid_acquisition_marker(&mark(&attributes))
    };
    assert!(!reclaim(&[]));
    assert!(!reclaim(&[("from", "claude-other000")]));
    assert!(reclaim(&[
        ("from", "claude-other000"),
        ("from-op", operation)
    ]));

    // And an operation id has to be exactly what it claims to be.
    assert!(!is_operation_id("short"));
    assert!(!is_operation_id(&operation.to_uppercase()));
    assert!(!is_operation_id(&format!("{operation}0")));
    assert!(is_operation_id(operation));
}

#[test]
fn a_prose_claim_needs_both_halves_and_neither_alone() {
    // The rule this exists for: a mention like "already claimed by @someone
    // months ago" satisfies neither half, and must not unseat a real claim.
    let horizon = "expects to report by 2026-07-26T00:54Z";

    assert_eq!(
        claim_prose(&format!("Claimed by claude-abcd1234, {horizon}.")),
        Some("claude-abcd1234".to_owned())
    );
    // A trailing dot is **part of the run-id** here, because the original's
    // character class is greedy and includes `.`. Written down rather than
    // trimmed: the two sides have to read one string the same way, and this test
    // asserted the tidier answer until the code disagreed with it.
    assert_eq!(
        claim_prose(&format!("Claimed by claude-abcd1234. {horizon}")),
        Some("claude-abcd1234.".to_owned())
    );
    // Decoration in front is still a claim: a bullet, a quote, a heading.
    assert_eq!(
        claim_prose(&format!("> **Claimed by** claude-abcd1234 — {horizon}")),
        None,
        "the phrase is split by decoration and no longer opens the comment"
    );
    assert_eq!(
        claim_prose(&format!("*  claimed by claude-abcd1234, {horizon}")),
        Some("claude-abcd1234".to_owned())
    );

    // The phrase without the horizon clause.
    assert_eq!(claim_prose("Claimed by claude-abcd1234."), None);
    // The horizon clause without the phrase opening the comment.
    assert_eq!(
        claim_prose(&format!("Already claimed by someone months ago. {horizon}")),
        None
    );
    // Neither.
    assert_eq!(claim_prose("Just a note."), None);
}

#[test]
fn the_newest_event_before_a_position_is_found_by_halving() {
    let history = [2usize, 5, 9, 14];
    let at = |value: &usize| *value;
    assert_eq!(latest_event_before(&history, 1, at), None);
    assert_eq!(latest_event_before(&history, 2, at), None);
    assert_eq!(latest_event_before(&history, 3, at), Some(&2));
    assert_eq!(latest_event_before(&history, 9, at), Some(&5));
    assert_eq!(latest_event_before(&history, 100, at), Some(&14));
    assert_eq!(latest_event_before::<usize>(&[], 5, at), None);
}

#[test]
fn an_epoch_falls_back_so_a_legacy_timeline_still_has_stable_identities() {
    // An operation id when there is one.
    assert_eq!(
        ownership_epoch(
            Some("0123456789abcdef0123456789abcdef"),
            Some("IC_1"),
            "t",
            "b"
        ),
        "0123456789abcdef0123456789abcdef"
    );
    // Otherwise the comment's own id.
    assert_eq!(ownership_epoch(None, Some("IC_1"), "t", "b"), "legacy-IC_1");

    // And with neither, a digest over the timestamp and body — **stable**, which
    // is the whole point: invent a new identity per read and no release could
    // ever match the acquisition it is meant to end.
    let once = ownership_epoch(None, None, "2026-07-26T00:54Z", "Claimed.");
    assert_eq!(
        once,
        ownership_epoch(None, None, "2026-07-26T00:54Z", "Claimed.")
    );
    assert_eq!(once.len(), "legacy-".len() + 32);
    assert_ne!(
        once,
        ownership_epoch(None, None, "2026-07-26T00:55Z", "Claimed.")
    );
    assert_ne!(
        once,
        ownership_epoch(None, None, "2026-07-26T00:54Z", "Other.")
    );
}

fn comment(id: &str, at: &str, body: &str) -> Comment {
    Comment {
        id: Some(id.to_owned()),
        created_at: at.to_owned(),
        body: body.to_owned(),
        viewer_did_author: true,
        includes_created_edit: false,
    }
}

#[test]
fn an_operation_is_reserved_at_its_first_marker_and_never_rewritten() {
    // The whole point of an operation id is that retrying names the *same*
    // event. A later correction that could rewrite it would turn one claim into
    // two.
    let operation = "0123456789abcdef0123456789abcdef";
    let mark = |horizon: &str| {
        super::super::markers::render(
            "claim",
            &[
                ("run-id", "claude-a"),
                ("op-id", operation),
                ("horizon", horizon),
            ],
        )
        .expect("it renders")
    };
    let comments = [
        comment("IC_1", "2026-07-26T00:01Z", &mark("2026-07-26T09:00Z")),
        comment("IC_2", "2026-07-26T00:02Z", &mark("2026-07-27T09:00Z")),
    ];
    let first = first_operation_markers(&comments);
    let (position, index, mark, unedited) = first.get(operation).expect("it was reserved");
    assert_eq!(
        (*position, *index),
        (0, 0),
        "a later marker took the reservation"
    );
    assert_eq!(
        mark.get("horizon").map(String::as_str),
        Some("2026-07-26T09:00Z")
    );
    assert!(unedited);

    // An untrusted author reserves nothing.
    let mut foreign = comments[0].clone();
    foreign.viewer_did_author = false;
    assert!(first_operation_markers(&[foreign]).is_empty());
}

#[test]
fn activity_is_this_run_speaking_and_not_somebody_naming_it() {
    // A marker *about* a run is not that run speaking. Reading it as activity
    // would keep a dead claim alive on the strength of somebody else writing
    // about it.
    let beat = |run: &str| {
        super::super::markers::render("heartbeat", &[("run-id", run)]).expect("it renders")
    };
    let comments = [
        comment("IC_1", "2026-07-26T01:00Z", &beat("claude-a")),
        comment("IC_2", "2026-07-26T02:00Z", &beat("claude-b")),
        comment("IC_3", "2026-07-26T03:00Z", "just prose about claude-a"),
    ];
    assert_eq!(
        last_activity_by(&comments, "claude-a", ""),
        "2026-07-26T01:00Z"
    );
    assert_eq!(
        last_activity_by(&comments, "claude-b", ""),
        "2026-07-26T02:00Z"
    );
    assert_eq!(last_activity_by(&comments, "claude-c", ""), "");

    // `after` is exclusive, and an edited comment is never trusted.
    assert_eq!(
        last_activity_by(&comments, "claude-a", "2026-07-26T01:00Z"),
        ""
    );
    let mut edited = comments[0].clone();
    edited.includes_created_edit = true;
    assert_eq!(last_activity_by(&[edited], "claude-a", ""), "");

    // A standdown names a run without being that run speaking.
    let named = comment(
        "IC_4",
        "2026-07-26T04:00Z",
        &super::super::markers::render("standdown", &[("run-id", "claude-a")]).expect("renders"),
    );
    assert_eq!(last_activity_by(&[named], "claude-a", ""), "");
}

#[test]
fn the_evidence_digest_covers_what_sits_between_the_heading_and_the_marker() {
    let marker =
        super::super::markers::render("reclaim", &[("run-id", "claude-a")]).expect("renders");
    let body = format!("{FORCED_EVIDENCE_HEADING}\n\nThe holder went silent.\n\n{marker}\n");
    let digest = forced_evidence_digest(&body, Some(0)).expect("there is evidence");
    assert_eq!(digest, sha256_hex(b"The holder went silent."));

    // No heading, or nothing under it, is no digest — never an empty one.
    assert_eq!(
        forced_evidence_digest(&format!("{marker}\n"), Some(0)),
        None
    );
    assert_eq!(
        forced_evidence_digest(&format!("{FORCED_EVIDENCE_HEADING}\n\n{marker}"), Some(0)),
        None
    );

    // Evidence carrying a marker of its own is refused: a body could otherwise
    // quote one into its evidence and change what the digest covers without
    // changing what a reader sees.
    let quoted = format!(
        "{FORCED_EVIDENCE_HEADING}\n\n{}\n\n{marker}\n",
        super::super::markers::render("standdown", &[("run-id", "claude-b")]).expect("renders")
    );
    assert_eq!(forced_evidence_digest(&quoted, Some(0)), None);
}

#[test]
fn a_release_names_its_target_by_the_attribute_its_kind_uses() {
    let mark = |pairs: &[(&str, &str)]| -> super::super::markers::Marker {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    };
    // A stand-down names the run being told to stop, or the run somebody else
    // is releasing.
    assert_eq!(
        released_by(&mark(&[("kind", "standdown"), ("run-id", "claude-a")])),
        vec!["claude-a".to_owned()]
    );
    assert_eq!(
        released_by(&mark(&[
            ("kind", "standdown"),
            ("run-id", "claude-a"),
            ("target", "claude-b")
        ])),
        vec!["claude-a".to_owned(), "claude-b".to_owned()]
    );
    // A reclaim names only who it displaces: its `run-id` is the run taking
    // over, and releasing *that* would release the new holder.
    assert_eq!(
        released_by(&mark(&[
            ("kind", "reclaim"),
            ("run-id", "claude-new"),
            ("from", "claude-old")
        ])),
        vec!["claude-old".to_owned()]
    );
    // A claim releases nobody.
    assert!(released_by(&mark(&[("kind", "claim"), ("run-id", "claude-a")])).is_empty());
}

#[test]
fn a_legacy_release_is_remembered_by_position_and_a_scoped_one_is_not() {
    let standdown = |run: &str, extra: &[(&str, &str)]| {
        let mut attributes = vec![("run-id", run)];
        attributes.extend_from_slice(extra);
        super::super::markers::render("standdown", &attributes).expect("renders")
    };
    let operation = "0123456789abcdef0123456789abcdef";
    let comments = [
        comment("IC_1", "2026-07-26T01:00Z", &standdown("claude-a", &[])),
        comment("IC_2", "2026-07-26T02:00Z", &standdown("claude-a", &[])),
        // Scoped to an epoch: folded elsewhere, never as a broad release.
        comment(
            "IC_3",
            "2026-07-26T03:00Z",
            &standdown("claude-b", &[("target-op", operation)]),
        ),
    ];
    let positions =
        legacy_release_positions(&comments, &ownership_events(&comments), &Default::default());
    assert_eq!(
        positions.get("claude-a"),
        Some(&1),
        "the later one must win"
    );
    assert_eq!(
        positions.get("claude-b"),
        None,
        "a scoped release counted as legacy"
    );

    // An untrusted or edited comment releases nothing.
    let mut foreign = comments[0].clone();
    foreign.viewer_did_author = false;
    assert!(
        legacy_release_positions(
            &[foreign.clone()],
            &ownership_events(&[foreign]),
            &Default::default()
        )
        .is_empty()
    );
}

#[test]
fn only_the_comments_that_carry_ownership_decide_whether_the_order_holds() {
    // A note posted out of order says nothing about whether the claims did, and
    // reading it as disorder would throw away a safe shortcut for no reason.
    let claim =
        |run: &str| super::super::markers::render("claim", &[("run-id", run)]).expect("renders");
    let ordered = [
        comment("IC_1", "2026-07-26T01:00Z", &claim("claude-a")),
        comment("IC_2", "2026-07-26T02:00Z", "just a note, posted late"),
        comment("IC_3", "2026-07-26T03:00Z", &claim("claude-b")),
    ];
    assert!(comments_are_chronological(&ordered));

    // An out-of-order note does not make the timeline out of order.
    let mut noisy = ordered.clone();
    noisy[1].created_at = "2026-07-25T00:00Z".to_owned();
    assert!(comments_are_chronological(&noisy));

    // Two acquisitions out of order do.
    let mut shuffled = ordered.clone();
    shuffled[2].created_at = "2026-07-26T00:30Z".to_owned();
    assert!(!comments_are_chronological(&shuffled));

    // And somebody else's comment is never looked at.
    let mut foreign = ordered;
    foreign[2].viewer_did_author = false;
    foreign[2].created_at = "2026-07-25T00:00Z".to_owned();
    assert!(comments_are_chronological(&foreign));
}

/// A time nobody can read has not proved it got there first.
///
/// The earliest live claim wins and `live.first()` is the winner, so whatever
/// an unreadable stamp sorts as, it wins. It sorted **first**: one constant was
/// serving two uses that want opposite answers — against a cursor,
/// unreadable-as-oldest leaves a run looking silent and expires its claim,
/// which fails closed; in a race it is a queue jump, and the run that genuinely
/// got there first is the one refused.
#[test]
fn an_unreadable_time_loses_a_race_rather_than_winning_it() {
    let readable = "2026-08-04T18:00:00Z";
    let unreadable = "not a time at all";

    // Against a cursor: unreadable is still the old end, which is the answer
    // that expires a claim rather than extending it.
    assert!(
        super::stamp_order(unreadable) < super::stamp_order(readable),
        "an unreadable stamp stopped reading as the oldest, so silence stopped expiring a claim"
    );
    // In a race: unreadable is the *last* place, not the first.
    assert!(
        super::stamp_rank(unreadable) > super::stamp_rank(readable),
        "an unreadable stamp wins a race it never proved it got to first"
    );
    // And the two really are different questions, or one of them is wrong.
    assert_ne!(
        super::stamp_order(unreadable),
        super::stamp_rank(unreadable)
    );
}

/// The offsets ISO-8601 allows are offsets this build reads.
///
/// `2026-08-04T18:00:00+02:00` is an ordinary timestamp and read as unreadable,
/// which is what made the queue jump above reachable rather than theoretical:
/// any tracker or proxy answering in local time handed the race to whoever it
/// answered that way.
#[test]
fn a_timestamp_with_an_offset_is_the_moment_it_names() {
    let noon = super::parse_stamp("2026-08-04T12:00:00Z").expect("a plain stamp");
    for (written, expected) in [
        ("2026-08-04T14:00:00+02:00", noon),
        ("2026-08-04T07:00:00-05:00", noon),
        ("2026-08-04T14:00:00+0200", noon),
        ("2026-08-04T12:00:00+00:00", noon),
        ("2026-08-04T12:00:00Z", noon),
    ] {
        assert_eq!(
            super::parse_stamp(written),
            Some(expected),
            "{written} is not the moment it names"
        );
    }

    // A shape nobody recognises stays unreadable rather than being read as UTC:
    // guessing here would move a claim by hours in whichever direction the
    // guess happened to fall.
    for written in [
        "2026-08-04T12:00:00+2",
        "2026-08-04T12:00:00+99:99",
        "2026-08-04 12:00:00Z",
    ] {
        assert_eq!(
            super::parse_stamp(written),
            None,
            "{written} was read as a moment this build cannot actually place"
        );
    }
}
