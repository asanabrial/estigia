use super::*;

#[test]
fn a_marker_renders_with_encoding_first_and_empty_attributes_dropped() {
    assert_eq!(
        render("standdown", &[("run-id", "claude-abcd1234")]),
        Some("<!-- issue-flow: standdown encoding=pct run-id=claude-abcd1234 -->".to_owned())
    );
    // An absent value is dropped rather than written empty: `run-id=` would read
    // as a marker addressed to a run whose name is the empty string.
    assert_eq!(
        render("note", &[("run-id", "")]),
        Some("<!-- issue-flow: note encoding=pct -->".to_owned())
    );
    // `_` becomes `-`, the way the original's `**attrs` spelling forces.
    assert_eq!(
        render("note", &[("run_id", "x")]),
        Some("<!-- issue-flow: note encoding=pct run-id=x -->".to_owned())
    );
}

#[test]
fn a_kind_or_key_outside_the_alphabet_renders_nothing() {
    // Where the transport raises `invalid-marker-attribute`. Refusing to render
    // is the safe half: a marker nobody can parse beats one that parses as
    // something else.
    assert_eq!(render("Standdown", &[]), None);
    assert_eq!(render("stand down", &[]), None);
    assert_eq!(render("note", &[("Run-Id", "x")]), None);
    assert_eq!(render("note", &[("1st", "x")]), None);
}

#[test]
fn a_marker_round_trips_through_percent_encoding() {
    // The safe set is `-._~:/+@` plus alphanumerics, and it is exact: widening
    // it by one character changes what a marker says, and a marker decides who
    // holds an issue.
    for value in [
        "claude-abcd1234",
        "a b",
        "sí",
        "100%",
        "<!-- nested -->",
        "a=b",
        "https://example.test/x?y",
        "tab\there",
    ] {
        let rendered = render("standdown", &[("run-id", value)]).expect("it renders");
        let parsed = parse(&rendered);
        assert_eq!(parsed.len(), 1, "{value:?} -> {rendered}");
        assert_eq!(
            parsed[0].get("run-id").map(String::as_str),
            Some(value),
            "{value:?} did not survive the round trip through {rendered}"
        );
        assert_eq!(parsed[0].get("kind").map(String::as_str), Some("standdown"));
    }
}

#[test]
fn quoting_a_marker_does_not_issue_it() {
    // The defect this exists for: an agent pasting a stand-down it received into
    // a note would *issue* a stand-down. Neutering the opener is what makes
    // quoting evidence safe.
    let quoted = "I was told:\n<!-- issue-flow: standdown encoding=pct run-id=me -->\nand stopped.";
    let escaped = escape_control_input(quoted);
    assert!(escaped.contains("&lt;!--"), "{escaped}");
    assert!(parse(&escaped).is_empty(), "a quoted marker still parsed");

    // And an ordinary HTML comment is left exactly as written.
    let ordinary = "before <!-- just a note --> after";
    assert_eq!(escape_control_input(ordinary), ordinary);
    // Including one that only looks close.
    let close = "<!-- issueflow: standdown -->";
    assert_eq!(escape_control_input(close), close);
}

#[test]
fn the_two_vocabularies_overlap_only_where_they_are_meant_to() {
    // The incident these constants were named for: a `reclaim` marker satisfied
    // the control check, so the displaced run stood down, while never releasing
    // the dead run's claim. Two sets that must agree cannot be written twice.
    // Against the table `is_control_for` reads, not against a second list of
    // the same words. This compared `RELEASE_KINDS` to a `CONTROL_KINDS` that
    // production had stopped consulting, so the overlap it guarded was between
    // one live vocabulary and one dead one.
    let control: Vec<&str> = crate::transport::ownership::control_kinds().collect();
    let both: Vec<&&str> = RELEASE_KINDS
        .iter()
        .filter(|kind| control.contains(kind))
        .collect();
    assert_eq!(both, vec![&"standdown", &"reclaim"]);
    assert!(RELEASE_KINDS.contains(&"release"));
    assert!(control.contains(&"adjudication"));
}

/// A percent escape is two hex digits, and nothing else is one.
///
/// `u8::from_str_radix` accepts a leading sign, so `%+4` came back as the byte
/// `0x04` here while `urllib.parse.unquote` left it as the three characters it
/// is. Measured against the transport's own decoder, input by input.
///
/// What that costs is not cosmetic. A marker is read back out of comments —
/// anybody can write one, and the whole adjudication is a reading of comments —
/// and the value it carries is the run-id. Two implementations decoding one
/// differently is the two of them disagreeing about **who holds the issue**.
#[test]
fn a_percent_escape_is_two_hex_digits_and_nothing_else_is() {
    let decoded = |raw: &str| {
        super::parse(&format!(
            "<!-- issue-flow: claim encoding=pct run-id={raw} -->"
        ))
        .first()
        .and_then(|marker| marker.get("run-id").cloned())
        .unwrap_or_default()
    };

    // The floor: a real escape still decodes, in either case of hex. Without
    // it, refusing every escape would satisfy the rest.
    assert_eq!(
        decoded("a%2Ab"),
        "a*b",
        "an upper-case escape stopped decoding"
    );
    assert_eq!(
        decoded("a%2ab"),
        "a*b",
        "a lower-case escape stopped decoding"
    );
    assert_eq!(
        decoded("a%%41b"),
        "a%Ab",
        "an escaped percent stopped decoding"
    );

    // And what the transport leaves alone, this leaves alone. Each of these was
    // read from `urllib.parse.unquote` rather than reasoned about.
    for (raw, expected) in [
        ("a%+4b", "a%+4b"),
        ("a%-4b", "a%-4b"),
        ("a%4", "a%4"),
        ("a%zzb", "a%zzb"),
    ] {
        assert_eq!(
            decoded(raw),
            expected,
            "`{raw}` is not an escape to the transport and this decoded it"
        );
    }
}

/// A `>` before the close means there is no marker here.
///
/// The transport's pattern is `<!--\s*issue-flow:\s*(kind)\s+([^>]*?)\s*-->`,
/// and `[^>]*?` cannot cross one. This side took everything up to the first
/// `-->` and read a marker out of it, so
/// `<!-- issue-flow: claim encoding=pct run-id=a>b -->` was a **claim only this
/// side could see**.
///
/// The direction is the one that costs. A claim one implementation honours and
/// the other does not is the two of them disagreeing about who holds an issue —
/// the same shape as the `standdown-->` case the parser's own note records, and
/// found the same way: by feeding the grammar a body neither renderer emits.
/// Neither does: `>` percent-encodes to `%3E`. Markers carrying one are
/// hand-written, and the timeline is full of hand-written comments.
#[test]
fn a_close_angle_before_the_close_means_there_is_no_marker() {
    let kinds = |body: &str| -> Vec<String> {
        super::parse(body)
            .iter()
            .filter_map(|marker| marker.get("kind").cloned())
            .collect()
    };

    // The floor: the same body without the `>` is still one marker, so this is
    // not a change that stopped reading markers.
    assert_eq!(
        kinds("<!-- issue-flow: claim encoding=pct run-id=ab -->"),
        vec!["claim".to_owned()],
        "an ordinary marker stopped being read"
    );

    assert!(
        kinds("<!-- issue-flow: claim encoding=pct run-id=a>b -->").is_empty(),
        "a body the transport reads no marker in is a claim on this side"
    );

    // And the one after it is still found, which is what the transport does:
    // its `finditer` fails at the first and matches the second.
    assert_eq!(
        kinds(
            "<!-- issue-flow: claim encoding=pct run-id=a>b --> \
             <!-- issue-flow: note encoding=pct run-id=c -->"
        ),
        vec!["note".to_owned()],
        "skipping the unreadable one took the readable one with it"
    );
}
