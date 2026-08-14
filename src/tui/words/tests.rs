use super::{SPANISH, TONGUES, Tongue, preference_path, remember, remembered, substitute};

/// Everything the screen puts in front of somebody, found both ways.
///
/// The literals written inline are read out of the source, because the failure
/// this guards is a **new** line arriving and a hand-written list cannot notice
/// about itself that somebody added a sentence to a panel. The rest arrive
/// through a method — a step's title, a setting's explanation, an adapter's
/// caveat — and are walked from the lists that declare them, which is the same
/// trick from the other end: `SETTINGS` grows, and this grows with it.
///
/// A setting's **label** is here now, and it was not. The old rule kept it out
/// on the grounds that a screen whose words the operator's file does not
/// contain is worse than an English one — which is a real worry answered in the
/// wrong place. A screen shows a name and stores a key, the way a dropdown
/// shows a label and sends an id, and the key stays visible where it is needed:
/// the commands this screen prints for running without it carry the English.
///
/// **Not** here: the values a setting accepts. Those are the cell of the row
/// rather than its heading, and `estigia config set "Merge strategy" rebase`
/// takes the value verbatim — a translated `rebase` would be a value the
/// command refuses.
fn lines_the_screen_says() -> Vec<String> {
    use crate::setup::Applies;
    use crate::tui::app::{MENU, SCREEN_ROWS, STEPS};

    let mut found = vec![super::super::HELP.to_owned()];
    found.push(crate::tui::app::OPTIONS_QUESTION.to_owned());
    found.push(crate::tui::app::TYPE_IT.to_owned());
    for entry in MENU {
        found.push(entry.label.to_owned());
        found.push(entry.about.to_owned());
    }
    for step in STEPS {
        found.push(step.title().to_owned());
        found.push(step.question().to_owned());
    }
    for setting in crate::config::SETTINGS {
        found.push(setting.label().to_owned());
        found.push(setting.about().to_owned());
        found.push(setting.accepted().to_owned());
    }
    // Every variant, not only the ones an adapter happens to produce today.
    // `Inert` is produced by nothing right now — its own note says so — and
    // walking the adapters therefore reported its caption as a translation for
    // a line nobody says. The day something returns it, the screen would have
    // shown one English word and no guard would have moved.
    for applies in [
        Applies::Held,
        Applies::Asked("a caption this test does not read"),
        Applies::Inert("a caption this test does not read"),
    ] {
        found.extend(applies.tag().map(ToOwned::to_owned));
    }
    for adapter in crate::setup::AGENTS {
        for setting in crate::config::SETTINGS {
            found.extend(adapter.applies(*setting).because().map(ToOwned::to_owned));
        }
    }
    // A preference's own words come from `Screen`, which reaches `t!` with a
    // literal — so they are already in the source scan. Its *answers* are the
    // languages themselves, which are names rather than prose.
    for screen in SCREEN_ROWS {
        let _ = screen;
    }

    for source in [include_str!("../../tui.rs"), include_str!("../app.rs")] {
        // Both macros, by name. `fill!(` was missed for a while by a scan that
        // looked for `t!(` and reasoned that `fill!` ends in one — it does not,
        // there is no `t` in it, and every interpolated line on the screen went
        // uncovered while the guard reported success.
        for opener in ["t!(", "fill!("] {
            let mut at = 0;
            while let Some(hit) = source[at..].find(opener) {
                let start = at + hit;
                at = start + opener.len();
                // A name that merely ends in the opener is a different macro.
                let before = source[..start].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '!') {
                    continue;
                }
                let Some(quote) = source[at..].find('"').map(|hit| at + hit) else {
                    continue;
                };
                // A `)` before the first quote means the argument was not a
                // literal — `t!(tongue, HELP)`, which is covered above.
                if source[at..quote].contains(')') {
                    continue;
                }
                let mut text = String::new();
                let mut walk = source[quote + 1..].char_indices();
                let mut end = quote + 1;
                while let Some((offset, character)) = walk.next() {
                    if character == '\\' {
                        if let Some((_, escaped)) = walk.next() {
                            text.push(match escaped {
                                'n' => '\n',
                                't' => '\t',
                                other => other,
                            });
                        }
                        continue;
                    }
                    if character == '"' {
                        end = quote + 1 + offset;
                        break;
                    }
                    text.push(character);
                }
                at = end + 1;
                if !text.is_empty() {
                    found.push(text);
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// The `{name}` holes in a line.
fn holes(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + close];
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            found.push(name.to_owned());
        }
        rest = &rest[open + close + 1..];
    }
    found.sort();
    found.dedup();
    found
}

#[test]
fn every_line_the_screen_says_has_been_translated() {
    // The fallback in `say` returns the English, so a missing row is not a
    // crash — it is one English sentence in the middle of a Spanish panel,
    // which is exactly the kind of half-done that nobody reports and everybody
    // sees. This is what makes the fallback unreachable.
    let said = lines_the_screen_says();
    assert!(
        said.len() > 40,
        "the source scan found only {} lines, so it is not finding them",
        said.len()
    );
    // Every language on offer, not only the one that happened to be first.
    // A language is offered when it is complete: an operator who picks one and
    // gets half an English screen has been told the tool is broken, and this
    // is what keeps that from being possible.
    for tongue in TONGUES {
        let Some(table) = tongue.table() else {
            continue;
        };
        let missing: Vec<&String> = said
            .iter()
            .filter(|line| !table.iter().any(|(key, _)| key == *line))
            .collect();
        assert!(
            missing.is_empty(),
            "the screen says these and {} has no words for them: {missing:#?}",
            tongue.name()
        );
    }
}

#[test]
fn every_translation_is_for_a_line_the_screen_actually_says() {
    // The other direction. A row for a sentence nobody shows is a translation
    // somebody wrote, reviewed and will maintain, for nothing — and worse, it
    // reads as coverage. Left alone these accumulate every time a panel is
    // reworded, and the table stops being a description of the screen.
    let said = lines_the_screen_says();
    for tongue in TONGUES {
        let Some(table) = tongue.table() else {
            continue;
        };
        let orphans: Vec<&str> = table
            .iter()
            .map(|(key, _)| *key)
            .filter(|key| !said.iter().any(|line| line == key))
            .collect();
        assert!(
            orphans.is_empty(),
            "{} carries lines the screen never says: {orphans:#?}",
            tongue.name()
        );
    }
}

#[test]
fn every_translation_carries_the_same_holes_as_its_english() {
    // Named holes, not positional, because a translation reorders — Spanish
    // puts the count where English puts the noun. That is the whole reason the
    // scheme is `{count}` and not `{0}`, and it only holds if both sides carry
    // the same set: a translation that drops `{count}` renders the brace-free
    // sentence with the number simply gone, and one that misspells it shows
    // `{cont}` to the operator.
    // Every language on offer: a rule that holds for one table and not
    // the next is a rule the next language will break silently.
    for (english, spanish) in TONGUES.iter().filter_map(|t| t.table()).flatten() {
        assert_eq!(
            holes(english),
            holes(spanish),
            "{english:?} and its translation do not carry the same holes"
        );
    }
}

#[test]
fn no_line_is_translated_twice() {
    // `say` takes the first match, so a second row for the same English is a
    // translation that is never used and cannot be told apart from one that is.
    let mut seen = std::collections::BTreeSet::new();
    for (english, _) in SPANISH {
        assert!(seen.insert(*english), "{english:?} is translated twice");
    }
}

#[test]
fn help_uses_the_translation_table_like_every_other_runtime_line() {
    let translated = Tongue::Spanish
        .table()
        .expect("Spanish has a translation table")
        .iter()
        .find(|(english, _)| *english == crate::tui::HELP)
        .map(|(_, spanish)| *spanish)
        .expect("Help is translated in the table");
    assert_eq!(
        Tongue::Spanish.say(crate::tui::HELP),
        translated,
        "Help bypasses the canonical runtime translation table"
    );
    assert!(
        !include_str!("../words.rs").contains("SPANISH_HELP"),
        "Help still has a special-case translation source"
    );
}

#[test]
fn a_translation_is_never_left_as_its_english() {
    // A row copied and not translated is worse than a missing row: the missing
    // one fails the guard above, and this one passes it while showing English.
    // Every language on offer: a rule that holds for one table and not
    // the next is a rule the next language will break silently.
    for (english, spanish) in TONGUES.iter().filter_map(|t| t.table()).flatten() {
        assert_ne!(
            english, spanish,
            "{english:?} is listed as translated and is the English"
        );
    }
}

#[test]
fn substituting_puts_the_values_where_the_holes_are() {
    assert_eq!(
        substitute(
            "{count} of {known} chosen",
            &[("count", "2"), ("known", "11")]
        ),
        "2 of 11 chosen"
    );
    // A hole nobody filled is left standing rather than removed: a number that
    // silently disappears is a sentence that reads as complete and is not.
    assert_eq!(substitute("{count} chosen", &[]), "{count} chosen");
    // And a value carrying braces is not re-scanned, so it cannot fill a hole
    // of its own.
    assert_eq!(
        substitute("{a}{b}", &[("a", "{b}"), ("b", "x")]),
        "{b}x",
        "a filled value was filled again"
    );
}

#[test]
fn a_language_is_asked_for_by_the_name_it_calls_itself() {
    // Somebody looking for Spanish on a screen they cannot read is looking for
    // `Español`, not for `Spanish`.
    assert_eq!(Tongue::from_name("Español"), Some(Tongue::Spanish));
    assert_eq!(Tongue::from_name("español"), Some(Tongue::Spanish));
    assert_eq!(Tongue::from_name("  English "), Some(Tongue::English));
    // And a language this screen has no words for is not offered as though it
    // had them: it would render in English with nothing saying why.
    assert_eq!(Tongue::from_name("Deutsch"), None);
    for tongue in TONGUES {
        assert_eq!(Tongue::from_name(tongue.name()), Some(*tongue));
    }
}

#[test]
fn the_screens_language_survives_the_run_that_chose_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    // Nothing remembered is English, which is what a first run has to be.
    assert_eq!(remembered(Some(home.path())), Tongue::English);

    remember(Some(home.path()), Tongue::Spanish).expect("the preference is written");
    assert_eq!(remembered(Some(home.path())), Tongue::Spanish);
    // One line, holding the name a person would type — a whole file format for
    // one word is a format somebody has to learn to fix by hand.
    let written = std::fs::read_to_string(preference_path(home.path())).expect("the file");
    assert_eq!(written.trim(), "Español");

    // A file holding something this screen cannot speak is English rather than
    // a refusal. The declared asymmetry is about guard rails, and a language is
    // not one: refusing to open the screen over a cosmetic answer would cost
    // somebody the tool.
    std::fs::write(preference_path(home.path()), "Klingon").expect("their file");
    assert_eq!(remembered(Some(home.path())), Tongue::English);
}

#[test]
fn the_english_screen_says_exactly_what_the_source_says() {
    // `say` on English is the identity, so a table entry can never change what
    // an English screen shows. Cheap to hold and worth holding: it means the
    // translation work cannot regress the language every test asserts against.
    for (english, _) in SPANISH {
        assert_eq!(Tongue::English.say(english), *english);
    }
}

/// An absent home resolves to the same file the screen writes.
///
/// `None` is the **ordinary** case in a real run — only a test sets `home_dir`
/// — so a caller that resolved nothing removed this file in the suite and
/// nothing on a machine. That was found by running the product, and it then
/// survived a mutation sweep: turning the fallback off left every test green,
/// because the fixture always takes the branch that works.
///
/// So the fallback lives beside the functions that write and read the file now,
/// and this is what holds it there.
#[test]
fn an_absent_home_still_names_the_file_the_screen_writes() {
    let home = std::path::Path::new("/h");
    assert_eq!(
        super::preference_path_for(Some(home)).as_deref(),
        Some(super::preference_path(home).as_path()),
        "a named home was not used as given"
    );

    // Absent resolves rather than answering nothing, and to the same place
    // `remembered` looks when it is handed `None`.
    let resolved = super::preference_path_for(None);
    match crate::paths::home_dir() {
        Ok(real) => assert_eq!(
            resolved.as_deref(),
            Some(super::preference_path(&real).as_path()),
            "an absent home named no file, so nothing would be removed on a real machine"
        ),
        // A machine with no home resolves to nothing, and inventing a path
        // there would delete something at a guess.
        Err(_) => assert!(resolved.is_none()),
    }
}

#[test]
fn a_translation_names_the_same_keys_as_the_line_it_translates() {
    // The two guards above check that a translation **exists** and that nothing
    // translates a line the screen never says. Neither can check that it says
    // the same thing — and for prose, nothing here could.
    //
    // For *keys* it can, because a key is a symbol rather than a word. When the
    // keymap was rewritten, four English lines were renamed and their Spanish
    // sides kept the old key: `enter next step` was still translated as
    // `⇥ paso siguiente`, so an operator reading the screen in Spanish was
    // taught a key that had stopped working. Both guards passed, because a
    // translation was present and its line was said.
    let keys: &[(&str, &[&str])] = &[
        ("⇥", &["⇥"]),
        ("⏎", &["⏎"]),
        ("enter", &["intro", "⏎"]),
        ("space", &["espacio"]),
        ("backspace", &["retroceso"]),
        ("esc", &["esc"]),
    ];
    // Whole words, because `implementer=opus` contains "enter" and `backspace`
    // contains "space". A guard that fires on those teaches people to silence
    // it, which is worse than not having one.
    let names = |text: &str, word: &str| {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .any(|token| token == word)
            || (!word.chars().all(char::is_alphanumeric) && text.contains(word))
    };
    let mut wrong: Vec<String> = Vec::new();
    // Every language on offer: a rule that holds for one table and not
    // the next is a rule the next language will break silently.
    for (english, spanish) in TONGUES.iter().filter_map(|t| t.table()).flatten() {
        for (key, accepted) in keys {
            let says = names(english, key);
            let echoed = accepted.iter().any(|word| names(spanish, word));
            if says && !echoed {
                wrong.push(format!(
                    "{english:?} names {key:?} and {spanish:?} does not"
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "a translation teaches a different key from the line it translates:\n  {}",
        wrong.join("\n  ")
    );
}
