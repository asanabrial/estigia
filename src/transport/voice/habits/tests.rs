use super::Habits;

const ALL_ONES: &str = r#"{
    "accents": { "dropped": { "más": 1.0, "así": 1.0, "también": 1.0, "podríamos": 1.0, "aquí": 1.0 }, "other": 1.0 },
    "lowercase_start": 1.0,
    "final_stop_dropped": 1.0,
    "opening_marks_dropped": 1.0,
    "space_before_closing_mark": 0.0
}"#;

fn habits(json: &str) -> Habits {
    Habits::parse(json.as_bytes()).expect("a valid habits file")
}

#[test]
fn every_habit_at_one_is_applied_and_the_code_span_is_kept() {
    let given = "¿Podríamos usar también `más()` aquí? Así queda más limpio.";
    assert_eq!(
        habits(ALL_ONES).apply(given),
        "podriamos usar tambien `más()` aqui? Asi queda mas limpio"
    );
}

#[test]
fn a_space_goes_before_a_closing_mark_that_follows_a_letter() {
    let spaced = habits(r#"{ "space_before_closing_mark": 1.0 }"#);
    assert_eq!(
        spaced.apply("¿Qué pasa? Nada!! Ok?"),
        "¿Qué pasa ? Nada !! Ok ?"
    );
}

#[test]
fn every_probability_at_zero_returns_the_text_byte_for_byte() {
    let zero = habits(
        r#"{ "accents": { "dropped": { "más": 0.0 }, "other": 0.0 }, "lowercase_start": 0.0,
             "final_stop_dropped": 0.0, "opening_marks_dropped": 0.0,
             "space_before_closing_mark": 0.0, "typo": { "share": 0.0, "min_words": 1 } }"#,
    );
    let given = "¿Podríamos usar también `más()` aquí? Así queda más limpio.";
    assert_eq!(zero.apply(given), given);
    assert_eq!(habits("{}").apply(given), given);
}

#[test]
fn protected_text_comes_back_byte_for_byte_under_all_ones() {
    let everything = habits(
        r#"{ "accents": { "dropped": {}, "other": 1.0 }, "lowercase_start": 1.0,
             "final_stop_dropped": 1.0, "opening_marks_dropped": 1.0,
             "space_before_closing_mark": 1.0, "typo": { "share": 1.0, "min_words": 1 } }"#,
    );
    for protected in [
        "```rust\nfn más() { ¿x? }\n```",
        "https://example.test/más/página?x=1",
        "\"decía: ¿qué más?\"",
        "@último",
        "camelCaseÁrbol",
        "snake_case_más",
        "ruta/más",
        "v1.2.3",
        "a->más",
    ] {
        let text = format!("Luego {protected} después");
        let got = everything.apply(&text);
        assert!(got.contains(protected), "{protected}: {got}");
    }
}

#[test]
fn a_first_token_that_is_protected_is_not_lowercased() {
    let lower = habits(r#"{ "lowercase_start": 1.0 }"#);
    assert_eq!(lower.apply("`Code` first"), "`Code` first");
    assert_eq!(lower.apply("camelCase first"), "camelCase first");
    assert_eq!(lower.apply("Plain first"), "plain first");
}

#[test]
fn a_final_stop_is_dropped_only_when_it_is_a_single_full_stop() {
    let stop = habits(r#"{ "final_stop_dropped": 1.0 }"#);
    assert_eq!(stop.apply("done."), "done");
    assert_eq!(stop.apply("done..."), "done...");
    assert_eq!(stop.apply("done?"), "done?");
    assert_eq!(stop.apply("done!"), "done!");
}

#[test]
fn a_decomposed_accent_is_removed_and_a_tilde_or_diaeresis_never_is() {
    let accents = habits(r#"{ "accents": { "dropped": {}, "other": 1.0 } }"#);
    assert_eq!(accents.apply("ma\u{301}s"), "mas");
    assert_eq!(accents.apply("año pingüino"), "año pingüino");
    assert_eq!(accents.apply("MÁS"), "MÁS");
    assert_eq!(accents.apply("Más"), "Mas");
}

#[test]
fn the_dropped_table_is_matched_without_regard_to_case() {
    let table = habits(r#"{ "accents": { "dropped": { "Más": 1.0 }, "other": 0.0 } }"#);
    assert_eq!(
        table.apply("Más y más, pero también"),
        "Mas y mas, pero también"
    );
}

#[test]
fn the_same_text_under_the_same_habits_is_written_the_same_way() {
    let half = habits(r#"{ "accents": { "dropped": {}, "other": 0.5 } }"#);
    let given = "más así también aquí allí además";
    assert_eq!(half.apply(given), half.apply(given));
}

#[test]
fn another_habits_file_may_write_the_same_text_another_way() {
    let given = (0..40)
        .map(|n| format!("más{n} además"))
        .collect::<Vec<_>>()
        .join(" ");
    let one = habits(r#"{ "accents": { "dropped": {}, "other": 0.5 } }"#);
    let other = habits(r#"{ "accents": { "dropped": {}, "other": 0.5 }, "lowercase_start": 0.0 }"#);
    assert_ne!(one.apply(&given), other.apply(&given));
}

#[test]
fn half_the_eligible_occurrences_change_at_a_probability_of_one_half() {
    let half = habits(r#"{ "accents": { "dropped": {}, "other": 0.5 } }"#);
    let total = 600;
    let changed = (0..total)
        .filter(|n| {
            let given = format!("Caso {n}: además");
            half.apply(&given) != given
        })
        .count();
    let share = changed as f64 / total as f64;
    assert!((0.4..=0.6).contains(&share), "{share}");
}

#[test]
fn a_typo_swaps_two_adjacent_inner_letters_of_one_long_enough_word() {
    let typo = habits(r#"{ "typo": { "share": 1.0, "min_words": 4 } }"#);
    let given = "una revision completa del cambio";
    let got = typo.apply(given);
    assert_ne!(got, given);
    let before: Vec<&str> = given.split(' ').collect();
    let after: Vec<&str> = got.split(' ').collect();
    let moved: Vec<usize> = (0..before.len())
        .filter(|at| before[*at] != after[*at])
        .collect();
    assert_eq!(moved.len(), 1, "{got}");
    let (a, b) = (before[moved[0]], after[moved[0]]);
    assert!(a.len() >= 6, "{a}");
    assert_eq!(a.as_bytes()[0], b.as_bytes()[0]);
    assert_eq!(a.as_bytes()[a.len() - 1], b.as_bytes()[b.len() - 1]);
    let mut sorted_a: Vec<char> = a.chars().collect();
    let mut sorted_b: Vec<char> = b.chars().collect();
    sorted_a.sort_unstable();
    sorted_b.sort_unstable();
    assert_eq!(sorted_a, sorted_b);
}

#[test]
fn a_text_shorter_than_the_minimum_gets_no_typo_and_a_listed_word_never_does() {
    let typo = habits(
        r#"{ "accents": { "dropped": { "podriamos": 0.0 }, "other": 0.0 },
             "typo": { "share": 1.0, "min_words": 3 } }"#,
    );
    assert_eq!(typo.apply("revision completa"), "revision completa");
    assert_eq!(
        typo.apply("podriamos podriamos podriamos"),
        "podriamos podriamos podriamos"
    );
}

#[test]
fn a_file_that_is_not_the_shape_is_refused_with_its_cause() {
    for bad in [
        "not json",
        r#"{ "unknown": 1 }"#,
        r#"{ "lowercase_start": 1.5 }"#,
        r#"{ "lowercase_start": -0.1 }"#,
        r#"{ "accents": { "dropped": { "más": 2.0 } } }"#,
        r#"{ "accents": { "other": -1.0 } }"#,
        r#"{ "typo": { "share": 3.0, "min_words": 1 } }"#,
        r#"{ "typo": { "share": 0.5, "extra": 1 } }"#,
    ] {
        assert!(Habits::parse(bad.as_bytes()).is_err(), "{bad}");
    }
    assert!(Habits::parse(&[0xff, 0xfe]).is_err());
}
