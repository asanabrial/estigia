use super::*;

fn lens(raw: &str) -> Result<RoleResult, Refusal> {
    parse(Role::Risk, raw.as_bytes())
}

const SEVERE: &str = r#"{"findings":[{"id":"f1","class":"severe","evidence":"x.rs:3 drops it","impact":"loses a write","location":"x.rs:3","evidence_class":"deterministic","causal_disposition":"introduced"}]}"#;

fn code(result: Result<RoleResult, Refusal>) -> &'static str {
    result.expect_err("a refusal").code
}

#[test]
fn a_valid_answer_is_read_for_every_role() {
    let RoleResult::Findings(found) = lens(SEVERE).expect("a lens answer") else {
        panic!("a lens answers with findings");
    };
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].evidence_class.as_deref(), Some("deterministic"));
    assert_eq!(
        lens(r#"{"findings":[]}"#).expect("nothing found"),
        RoleResult::Findings(vec![])
    );

    let refuted = parse(
        Role::Refuter,
        br#"{"outcomes":[{"id":"f1","outcome":"corroborated"},{"id":"f2","outcome":"inconclusive"}]}"#,
    )
    .expect("a refuter answer");
    assert!(matches!(refuted, RoleResult::Outcomes(ref all) if all.len() == 2));

    assert_eq!(
        parse(
            Role::Validator,
            br#"{"inspection":"available","original_criteria":"pass","correction_regression":"fail"}"#
        )
        .expect("an inspected candidate"),
        RoleResult::Validation(Validation::Available {
            original_criteria: Criterion::Pass,
            correction_regression: Criterion::Fail,
        })
    );
    assert_eq!(
        parse(
            Role::Validator,
            br#"{"inspection":"unavailable","reason":"the tree would not read"}"#
        )
        .expect("an uninspected candidate"),
        RoleResult::Validation(Validation::Unavailable {
            reason: "the tree would not read".to_owned()
        })
    );
}

#[test]
fn a_json_fence_and_whitespace_are_allowed_and_nothing_else_is() {
    assert!(lens(&format!("\n  ```json\n{SEVERE}\n```\n")).is_ok());
    assert!(lens(&format!("```\n{SEVERE}\n```")).is_ok());
    for refused in [
        format!("Here is my review: {SEVERE}"),
        format!("{SEVERE}\nHope that helps."),
        format!("```yaml\n{SEVERE}\n```"),
        format!("```json\n{SEVERE}\n```\nthanks"),
        format!("```json\n{SEVERE}"),
        format!("{SEVERE}{SEVERE}"),
        String::new(),
    ] {
        assert_eq!(
            code(lens(&refused)),
            "reviewer-result-malformed",
            "{refused}"
        );
    }
    assert_eq!(
        code(parse(Role::Risk, &[0xff, 0xfe])),
        "reviewer-result-malformed"
    );
}

#[test]
fn a_field_the_schema_does_not_name_is_refused() {
    assert_eq!(
        code(lens(r#"{"findings":[],"verdict":"accepted"}"#)),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(lens(
            &SEVERE.replace("\"impact\"", "\"extra\":1,\"impact\"")
        )),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(parse(Role::Refuter, br#"{"outcomes":[],"findings":[]}"#)),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(parse(
            Role::Validator,
            br#"{"inspection":"unavailable","reason":"r","attempt":1}"#
        )),
        "reviewer-result-malformed"
    );
}

#[test]
fn a_word_outside_the_vocabulary_is_refused() {
    for (from, to) in [
        ("\"severe\"", "\"critical\""),
        ("\"deterministic\"", "\"certain\""),
        ("\"introduced\"", "\"mine\""),
    ] {
        assert_eq!(
            code(lens(&SEVERE.replace(from, to))),
            "reviewer-result-malformed",
            "{to}"
        );
    }
    assert_eq!(
        code(lens(&SEVERE.replace("}]}", ",\"origin\":\"invented\"}]}"))),
        "reviewer-result-malformed"
    );
}

#[test]
fn a_lens_finding_owes_a_location_the_transport_can_read() {
    let warning = |location: &str| {
        format!(
            r#"{{"findings":[{{"id":"w","class":"warning","evidence":"a real thing","impact":"a real cost"{location}}}]}}"#
        )
    };
    assert_eq!(code(lens(&warning(""))), "reviewer-result-malformed");
    for bad in [
        "line three",
        "x.rs",
        "x.rs:0",
        "x.rs:5-2",
        "/etc/passwd:1",
        " ",
    ] {
        assert_eq!(
            code(lens(&warning(&format!(r#","location":"{bad}""#)))),
            "reviewer-result-malformed",
            "{bad}"
        );
    }
    assert!(lens(&warning(r#","location":" x.rs:3-5 ""#)).is_ok());
    assert_eq!(
        code(parse(Role::Lens, warning("").as_bytes())),
        "reviewer-result-malformed"
    );
}

#[test]
fn a_whitespace_only_vocabulary_word_is_refused_not_read_as_absent() {
    for field in ["evidence_class", "causal_disposition", "origin"] {
        let raw = format!(
            r#"{{"findings":[{{"id":"w","class":"warning","evidence":"a real thing","impact":"a real cost","location":"x.rs:2","{field}":" "}}]}}"#
        );
        assert_eq!(code(lens(&raw)), "reviewer-result-malformed", "{field}");
    }
}

#[test]
fn a_citation_the_transport_reads_as_malformed_is_refused() {
    let with = |evidence: &str, impact: &str| {
        format!(
            r#"{{"findings":[{{"id":"w","class":"warning","evidence":"{evidence}","impact":"{impact}","location":"x.rs:2"}}]}}"#
        )
    };
    for (evidence, impact) in [
        ("src/x.rs:12:5 drops the write", "a real cost"),
        ("a real thing", "see /etc/passwd:1 for it"),
        ("a real thing", "see ../secrets.rs:4 for it"),
    ] {
        let refusal = lens(&with(evidence, impact)).expect_err("a malformed citation");
        assert_eq!(
            refusal.code, "reviewer-result-malformed",
            "{evidence} {impact}"
        );
    }
    assert!(lens(&with("src/x.rs:12 drops it", "version 1.2:3 costs it")).is_ok());
}

#[test]
fn a_severe_finding_owes_its_two_declarations() {
    assert_eq!(
        code(lens(
            &SEVERE.replace(",\"evidence_class\":\"deterministic\"", "")
        )),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(lens(
            &SEVERE.replace(",\"causal_disposition\":\"introduced\"", "")
        )),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(lens(&SEVERE.replace("\"deterministic\"", "\"\""))),
        "reviewer-result-malformed"
    );
    let warning = r#"{"findings":[{"id":"w","class":"warning","evidence":"a real thing","impact":"a real cost","location":"x.rs:2"}]}"#;
    assert!(lens(warning).is_ok());
}

#[test]
fn a_finding_that_says_nothing_or_repeats_an_id_is_refused() {
    assert_eq!(
        code(lens(&SEVERE.replace("loses a write", "n/a"))),
        "reviewer-result-malformed"
    );
    assert_eq!(
        code(lens(&SEVERE.replace("\"f1\"", "\" \""))),
        "reviewer-result-malformed"
    );
    let twice = SEVERE.replace("]}", &format!(",{}]}}", &SEVERE[13..SEVERE.len() - 2]));
    assert_eq!(code(lens(&twice)), "reviewer-result-malformed");
}

#[test]
fn a_refuter_outcome_outside_the_vocabulary_is_refused() {
    for raw in [
        r#"{"outcomes":[{"id":"f1","outcome":"confirmed"}]}"#,
        r#"{"outcomes":[{"id":"f1","outcome":"refuted"},{"id":"f1","outcome":"refuted"}]}"#,
        r#"{"outcomes":[{"id":"","outcome":"refuted"}]}"#,
    ] {
        assert_eq!(
            code(parse(Role::Refuter, raw.as_bytes())),
            "reviewer-result-malformed",
            "{raw}"
        );
    }
}

#[test]
fn the_validator_answers_the_way_the_transport_admits() {
    for raw in [
        r#"{"inspection":"available"}"#,
        r#"{"inspection":"available","original_criteria":"pass"}"#,
        r#"{"inspection":"available","original_criteria":"maybe","correction_regression":"pass"}"#,
        r#"{"inspection":"unavailable"}"#,
        r#"{"inspection":"unavailable","reason":"  "}"#,
        r#"{"inspection":"unavailable","reason":"r","original_criteria":"pass"}"#,
        r#"{"inspection":"pending"}"#,
    ] {
        assert_eq!(
            code(parse(Role::Validator, raw.as_bytes())),
            "reviewer-result-malformed",
            "{raw}"
        );
    }
}

#[test]
fn a_malformed_answer_is_refused_and_never_read_as_a_pass() {
    let refusal = lens("nonsense").expect_err("not an answer");
    assert_eq!(refusal.class, crate::outcome::RefusalClass::Refused);
}

#[test]
fn prose_after_a_closed_fence_is_refused_and_not_read_as_an_empty_sweep() {
    let raw = "```json\n{\"findings\": []}\n```\n\nNothing to flag.";
    assert_eq!(code(lens(raw)), "reviewer-result-malformed");
}
