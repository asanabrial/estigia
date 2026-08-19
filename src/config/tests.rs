use std::path::PathBuf;

/// An em dash only ends a value when it is a word of its own.
///
/// `clean` drops a trailing explanation — `squash — because we always squash` is
/// an answer and a sentence — and the rule that keeps it from eating half a
/// value is that the dash has to have whitespace on **both** sides. Measured by
/// mutation: making `follows_whitespace` answer `true` for every position left
/// the whole suite green, and with it gone `npm publish —dry-run` becomes
/// `npm publish`.
///
/// Not a made-up value: a macOS text field turns `--` into `—` while somebody
/// types, and `Irreversible commands` is a list of command lines.
#[test]
fn a_dash_inside_a_value_does_not_end_it() {
    let read = |cell: &str| {
        crate::config::table_rows(&format!(
            "| Setting | Value |\n|---|---|\n| Irreversible commands | {cell} |\n"
        ))
        .into_iter()
        .next()
        .expect("the row is read")
        .1
    };

    // The floor: a dash that *is* a word ends the value, or nothing below is
    // measuring the rule that keeps one from doing it.
    assert_eq!(
        read("npm publish — because releases are one-way"),
        "npm publish",
        "a trailing explanation was kept as part of the value"
    );

    // And one that is not a word of its own is part of what was typed.
    for whole in [
        "npm publish —dry-run",
        "cargo—publish",
        "npm publish--dry-run",
    ] {
        assert_eq!(
            read(whole),
            whole,
            "a value carrying a dash was cut where the dash is"
        );
    }
}

/// The stricter reader's rule, in both directions.
///
/// Measured by mutation: `reaches_the_transport` could answer `true` for every
/// row with the whole suite green. Its one caller asserts it in the direction
/// that says **yes** — every declared label reaches — and the three ways a
/// hand-typed cell fails to reach were checked by nothing, though the function's
/// own documentation tabulates them.
///
/// The consequence is the one that table names: `estigia config list` reports a
/// board and nothing is mirrored to any board, with nothing anywhere saying why.
#[test]
fn a_row_that_does_not_reach_the_transport_is_not_read_as_one_that_does() {
    use crate::config::{Setting, reaches_the_transport};

    // Reaches: the label itself, and a label carrying a person's own suffix.
    for cell in ["Project board", "  Project board  ", "Project board (mine)"] {
        assert!(
            reaches_the_transport(cell, Setting::Board),
            "`{cell}` reaches the transport and was read as not reaching it"
        );
    }

    // Does not: the backticks a person writing markdown puts round a name, and
    // a run of whitespace inside it. This crate matches both; the rule the
    // transport applies does not, and that is the difference worth reporting.
    for cell in ["`Project board`", "Project  board", "board", ""] {
        assert!(
            !reaches_the_transport(cell, Setting::Board),
            "`{cell}` does not reach the transport and was read as one that does"
        );
    }

    // And it is a rule about *this* setting, not about any label: a cell that
    // names one setting does not reach the transport as another.
    assert!(
        !reaches_the_transport("Project board", Setting::Worktree),
        "one setting's label was read as another's"
    );
}

use std::time::Duration;

use super::*;

fn table(rows: &[(&str, &str)]) -> String {
    let mut out = String::from("| Setting | Value here | Skill default |\n|---|---|---|\n");
    for (label, value) in rows {
        out.push_str(&format!("| {label} | {value} | ignored |\n"));
    }
    out
}

/// Every variant, listed under an exhaustive `match` so the **compiler** keeps
/// the list honest.
///
/// A hand-written list cannot: a variant added to the enum and to neither list
/// passes a length check silently, and the setting it names is then one nobody
/// can write and nobody is told about. The `match` has no wildcard, so a new
/// variant stops the build here — at the one place that decides whether it
/// reaches the table, and therefore whether `estigia setup --interactive` ever
/// asks about it.
fn every_variant() -> Vec<Setting> {
    let placed = |setting: Setting| -> Setting {
        match setting {
            Setting::Delivery
            | Setting::Route
            | Setting::Review
            | Setting::Transitions
            | Setting::Merge
            | Setting::Worktree
            | Setting::Tracker
            | Setting::Planning
            | Setting::Models
            | Setting::Integration
            | Setting::Window
            | Setting::ReviewProtocol
            | Setting::Judges
            | Setting::Evidence
            | Setting::ChangeSize
            | Setting::Boundaries
            | Setting::Board
            | Setting::Summary
            | Setting::Body => setting,
        }
    };
    SETTINGS.iter().copied().map(placed).collect()
}

#[test]
fn no_setting_answers_to_a_label_another_one_owns() {
    // `from_label` walks `SETTINGS` and takes the first match, so a name shared
    // by two rows resolves by **position in the table** — which is not a thing
    // anybody writing an alias is thinking about.
    //
    // Reachable, and nearly reached. `Task body language` is the old spelling of
    // `Summary language` and is an alias of it, and the row that was almost
    // called `Task body language` is the new one for the issue body. Shipped
    // that way, an operator's existing value would have resolved to whichever
    // of the two came first, and moving a row in `SETTINGS` — a change nobody
    // would think to test — would have silently changed which setting their
    // file was configuring.
    let mut owner: std::collections::BTreeMap<String, Setting> = std::collections::BTreeMap::new();
    for setting in SETTINGS {
        for name in std::iter::once(setting.label()).chain(setting.aliases().iter().copied()) {
            let key = name.trim().trim_matches('`').to_ascii_lowercase();
            if let Some(other) = owner.insert(key, *setting)
                && other != *setting
            {
                panic!(
                    "{name:?} names both {other:?} and {setting:?}, so which one a file \
                     configures is decided by their order in SETTINGS"
                );
            }
        }
    }
    // And every one of those names resolves back to the setting that claimed it.
    for setting in SETTINGS {
        for name in std::iter::once(setting.label()).chain(setting.aliases().iter().copied()) {
            assert_eq!(
                Setting::from_label(name),
                Some(*setting),
                "{name:?} is claimed by {setting:?} and does not read back as it"
            );
        }
    }
}

#[test]
fn every_setting_is_in_the_table() {
    // Cross-checked against the exhaustive match above, so a variant that
    // reaches neither the table nor this list cannot pass quietly.
    assert_eq!(
        every_variant().len(),
        SETTINGS.len(),
        "the table and the enum disagree"
    );
    let all = [
        Setting::Delivery,
        Setting::Route,
        Setting::Review,
        Setting::Transitions,
        Setting::Merge,
        Setting::Worktree,
        Setting::Tracker,
        Setting::Planning,
        Setting::Models,
        Setting::Integration,
        Setting::Window,
        Setting::ReviewProtocol,
        Setting::Judges,
        Setting::Evidence,
        Setting::ChangeSize,
        Setting::Boundaries,
        Setting::Board,
        Setting::Summary,
        Setting::Body,
    ];
    for setting in all {
        assert!(
            SETTINGS.contains(&setting),
            "{setting:?} is not in SETTINGS, so it is never written or parsed"
        );
    }
    assert_eq!(SETTINGS.len(), all.len());
}

#[test]
fn every_setting_belongs_to_exactly_one_of_the_two_lists() {
    use crate::config::{AGENT_SETTINGS, EVERYWHERE_SETTINGS, MACHINE_SETTINGS, Scope};

    // The screen asks the agent-scoped rows once per agent, on a step of setup,
    // and the rest once for the repository, on the options page. A setting in
    // neither list is a setting the screen never offers, and one in both is a
    // row an operator can answer twice with two different answers, of which
    // only the later write survives.
    for setting in SETTINGS {
        // Three now, since the languages stopped being the repository's: what
        // somebody writes in does not change because they opened a different
        // checkout.
        let lists = [
            (Scope::Agent, AGENT_SETTINGS.contains(setting)),
            (Scope::Everywhere, EVERYWHERE_SETTINGS.contains(setting)),
            (Scope::Machine, MACHINE_SETTINGS.contains(setting)),
        ];
        let listed: Vec<Scope> = lists
            .iter()
            .filter(|(_, holds)| *holds)
            .map(|(scope, _)| *scope)
            .collect();
        assert_eq!(
            listed.len(),
            1,
            "{setting:?} is in {} of the three lists",
            listed.len()
        );
        // And the lists agree with the setting's own answer, so a row moved in
        // one place and not the other cannot pass.
        assert_eq!(
            setting.scope(),
            listed[0],
            "{setting:?} is listed under one scope and declares the other"
        );
    }
    assert_eq!(
        AGENT_SETTINGS.len() + EVERYWHERE_SETTINGS.len() + MACHINE_SETTINGS.len(),
        SETTINGS.len()
    );

    // Every row says what it is for. A row whose only explanation is its own
    // label is a row an operator has to guess at, which is what the screen was
    // rebuilt to stop.
    for setting in SETTINGS {
        let about = setting.about();
        assert!(!about.is_empty(), "{setting:?} says nothing about itself");
        assert_ne!(
            about.to_ascii_lowercase(),
            setting.label().to_ascii_lowercase(),
            "{setting:?} explains itself by repeating its label"
        );
    }
}

/// A closed list offers every value its type can hold.
///
/// The converse of `every_offered_answer_round_trips_…`, and the half the
/// screen leans on. That test walks the *offers* and proves each one is
/// storable, which says nothing about a value the **type** can hold and the
/// list never names. A closed row draws its answers with the one in force
/// marked, so a value nobody offered is a row with no mark anywhere on it —
/// the operator reads "nothing is set" where something is, and steps away
/// from it with one arrow key.
///
/// Held here rather than by inspection because the two halves live apart: a
/// variant is added to an enum in this module, and the list that has to grow
/// with it is a `match` arm in another. Nothing but this test connects them.
#[test]
fn a_closed_list_offers_every_value_its_type_can_hold() {
    fn rendered<T>(values: Vec<T>, put: impl Fn(&mut Config, T), setting: Setting) -> Vec<String> {
        values
            .into_iter()
            .map(|value| {
                let mut config = Config::default();
                put(&mut config, value);
                setting.value_of(&config)
            })
            .collect()
    }

    for setting in SETTINGS {
        let setting = *setting;
        // Exhaustive on purpose: a new setting has to say here whether its
        // vocabulary is enumerable, and a `_` arm would let it answer by
        // saying nothing.
        let every = match setting {
            Setting::Route => Some(rendered(DeliveryRoute::all(), |c, v| c.route = v, setting)),
            Setting::Merge => Some(rendered(MergeStrategy::all(), |c, v| c.merge = v, setting)),
            Setting::Planning => Some(rendered(Planning::all(), |c, v| c.planning = v, setting)),
            Setting::Integration => Some(rendered(
                Integration::all(),
                |c, v| c.integration = v,
                setting,
            )),
            Setting::ReviewProtocol => Some(rendered(
                ReviewProtocol::all(),
                |c, v| c.review_protocol = v,
                setting,
            )),
            Setting::Judges => Some(rendered(Judges::all(), |c, v| c.judges = v, setting)),
            Setting::Evidence => Some(rendered(Evidence::all(), |c, v| c.evidence = v, setting)),
            // Open vocabularies: a path, a board name, a language, a duration,
            // a count of lines, a list of commands, a tracker this build may not
            // know. There is nothing to enumerate, and the row has somewhere to
            // type instead.
            Setting::ChangeSize
            | Setting::Delivery
            | Setting::Review
            | Setting::Transitions
            | Setting::Worktree
            | Setting::Tracker
            | Setting::Models
            | Setting::Window
            | Setting::Boundaries
            | Setting::Board
            | Setting::Summary
            | Setting::Body => None,
        };

        let answers = setting.answers();
        // The two ways of saying the same thing, said the same way. A setting
        // enumerated here and offered as open would be a list promising a
        // field to type in that the screen never draws, and one offered as
        // closed with nothing enumerated is this guard passing by abstaining.
        assert_eq!(
            answers.closed,
            every.is_some(),
            "{setting:?} calls its list {} and this guard enumerates it {}",
            if answers.closed { "closed" } else { "open" },
            if every.is_some() { "closed" } else { "open" }
        );

        for value in every.into_iter().flatten() {
            assert!(
                answers.at(&value).is_some(),
                "{setting:?} can hold {value:?} and never offers it \u{2014} the row would draw \
                 every answer unmarked, which reads as nothing set"
            );
        }
    }
}

#[test]
fn every_offered_answer_round_trips_and_a_closed_list_is_the_whole_vocabulary() {
    for setting in SETTINGS {
        let answers = setting.answers();
        for choice in answers.choices {
            let mut config = Config::default();
            setting
                .apply(&mut config, choice)
                .unwrap_or_else(|refusal| {
                    panic!("{setting:?} offers {choice:?} and refuses it: {refusal}")
                });
            // Written back the same way, or the screen shows a list whose
            // selected entry is never the one that is set — the value column
            // would say `ask` while the list highlighted `ask 15m`.
            assert_eq!(
                setting.value_of(&config),
                *choice,
                "{setting:?} offers {choice:?} and stores it as something else"
            );
        }

        // A closed list is a promise that the arrow keys reach everything. The
        // default has to be on it, or the screen opens on a value its own list
        // does not contain and one press jumps somewhere unrelated.
        if answers.closed {
            let default = setting.default_value();
            assert!(
                answers.at(&default).is_some(),
                "{setting:?} calls its list closed and the default {default:?} is not on it"
            );
            // And stepping is a cycle over exactly those answers.
            let mut walked = vec![default.clone()];
            let mut at = default;
            for _ in 1..answers.choices.len() {
                at = setting
                    .answers()
                    .step(&at, 1)
                    .expect("a closed list steps")
                    .to_owned();
                walked.push(at.clone());
            }
            walked.sort();
            let mut all: Vec<String> = answers.choices.iter().map(|c| (*c).to_owned()).collect();
            all.sort();
            assert_eq!(
                walked, all,
                "{setting:?} does not cycle through all of them"
            );
        }

        // Every setting offers something, closed or not. A row with no answers
        // and no prose is a row somebody has to guess at.
        assert!(
            !answers.choices.is_empty(),
            "{setting:?} offers nothing to choose and nothing to copy"
        );
    }
}

#[test]
fn every_label_resolves_back_to_its_setting() {
    for setting in SETTINGS {
        assert_eq!(Setting::from_label(setting.label()), Some(*setting));
        for alias in setting.aliases() {
            assert_eq!(Setting::from_label(alias), Some(*setting));
        }
    }
}

#[test]
fn defaults_match_the_portable_table_issue_flow_shipped() {
    let config = Config::default();
    assert_eq!(config.tracker, Tracker::Github { repo: None });
    assert_eq!(config.merge, MergeStrategy::MergeCommit);
    assert_eq!(config.worktree, WorktreeRoot::Auto);
    assert_eq!(config.board, None);
    assert_eq!(config.summary_language.as_str(), "English");
    assert_eq!(config.body_language.as_str(), "English");
    assert!(!config.delivery.is_autonomous());
    assert!(!config.review.is_autonomous());
    assert!(!config.transitions.is_autonomous());
}

#[test]
fn a_rendered_table_reads_back_identically() {
    let config = Config {
        tracker: Tracker::Github {
            repo: Some(RepoRef::parse("asanabrial/estigia").unwrap()),
        },
        planning: Planning::Sdd {
            openspec: true,
            lite: false,
        },
        models: ModelRouting::parse("implementer=opus, judge=haiku").expect("a routing"),
        integration: Integration::Trunk,
        window: std::time::Duration::from_secs(30),
        review_protocol: ReviewProtocol::ReceiptDriven,
        judges: Judges::TwoBlind,
        evidence: Evidence::Measuring,
        // Not the default, so a round trip that dropped the row would show.
        change_size: 250,
        boundaries: vec!["npm publish".to_owned()],
        worktree: WorktreeRoot::Path(PathBuf::from(if cfg!(windows) {
            r"C:\work\trees"
        } else {
            "/work/trees"
        })),
        summary_language: Language::parse("Español").unwrap(),
        body_language: Language::parse("Deutsch").unwrap(),
        delivery: Authority::Auto,
        route: DeliveryRoute::Direct,
        review: Authority::Ask {
            timeout: Duration::from_secs(1800),
        },
        transitions: Authority::Auto,
        merge: MergeStrategy::Squash,
        board: Some(BoardRef::parse("acme/7").unwrap()),
    };
    let rendered = config.render_rows();
    let reread = Config::read(&rendered, None).expect("a table Estigia wrote must read back");
    assert_eq!(reread, config);
}

#[test]
fn the_local_file_overrides_the_versioned_table() {
    let versioned = table(&[("Merge strategy", "merge commit"), ("Tracker", "github")]);
    let local = table(&[("Merge strategy", "squash")]);
    let config = Config::read(&versioned, Some(&local)).unwrap();
    assert_eq!(config.merge, MergeStrategy::Squash);
    // A row the local file does not mention keeps the versioned value.
    assert_eq!(config.tracker, Tracker::Github { repo: None });
}

#[test]
fn both_older_names_for_the_summary_language_are_still_read() {
    // Two spellings preceded `Summary language`: issue-flow's, and Estigia's
    // own `Task body language`. Under either of them the value governed the
    // summary sentence and nothing else, so both go on meaning that. Read as
    // the *new* `Issue body language` instead, an operator's answer would move
    // onto a row that decides something different and leave the row it did
    // decide sitting at the default — a setting changed by an upgrade, silently.
    for older in [
        "\"Description for dumb humans\" sentence language",
        "Task body language",
    ] {
        let local = table(&[(older, "Español")]);
        let config = Config::read(&local, None).unwrap();
        assert_eq!(
            config.summary_language.as_str(),
            "Español",
            "{older} stopped reaching the row it always set"
        );
        assert_eq!(
            config.body_language.as_str(),
            "English",
            "{older} was read as the new row, which it never governed"
        );
    }
}

#[test]
fn rows_estigia_never_published_are_left_alone() {
    // The operator's file is theirs. An unknown row is a note, not a defect.
    let local = format!(
        "{}\n\nSome prose of my own.\n\n| Reviewer rota | Tuesdays | |\n",
        table(&[("Merge strategy", "rebase")])
    );
    let config = Config::read(&local, None).unwrap();
    assert_eq!(config.merge, MergeStrategy::Rebase);
}

#[test]
fn an_unrecognised_value_is_refused_and_names_what_is_accepted() {
    let local = table(&[("Merge strategy", "fast-forward")]);
    let refusal = Config::read(&local, None).unwrap_err();
    assert_eq!(refusal.code, "config-value-unrecognised");
    let rendered = refusal.to_string();
    // Backticked, because that marker is what tells a two-word value apart
    // from a description of one — and what lets a test check that every value
    // named here is a value the parser takes.
    assert!(
        rendered.contains("`merge commit`, `squash`, or `rebase`"),
        "{rendered}"
    );
    // Nothing was written, so re-running the same command is safe.
    assert!(refusal.outcome.is_clean());
}

#[test]
fn a_relative_worktree_location_is_refused_rather_than_resolved() {
    // issue-flow's table would have accepted this and discovered it halfway
    // through a checkout.
    let local = table(&[("Worktree location", "../trees")]);
    let refusal = Config::read(&local, None).unwrap_err();
    assert_eq!(refusal.code, "worktree-location-not-absolute");
}

#[test]
fn ask_carries_a_timeout_and_a_written_one_survives_a_round_trip() {
    let local = table(&[("Transition authorisation", "ask 45m")]);
    let config = Config::read(&local, None).unwrap();
    assert_eq!(
        config.transitions,
        Authority::Ask {
            timeout: Duration::from_secs(45 * 60)
        }
    );
    assert!(
        config
            .render_rows()
            .contains("| Transition authorisation | ask 45m |")
    );
}

#[test]
fn a_bare_ask_renders_bare() {
    let config = Config::default();
    assert!(
        config
            .render_rows()
            .contains("| Delivery authorisation | ask | ask |")
    );
}

#[test]
fn a_zero_timeout_is_not_a_timeout() {
    let local = table(&[("Review delegation", "ask 0m")]);
    assert!(Config::read(&local, None).is_err());
}

#[test]
fn every_tracker_names_a_binding_that_ships() {
    for tracker in [
        Tracker::Github { repo: None },
        Tracker::Linear,
        Tracker::Trello,
    ] {
        let binding = tracker.binding();
        assert!(
            crate::skill::FILES.iter().any(|file| file.path == binding),
            "{binding} is named by {tracker:?} but is not installed"
        );
    }
}

#[test]
fn a_malformed_repository_reference_is_refused() {
    let local = table(&[("Tracker", "github not-a-repo")]);
    let refusal = Config::read(&local, None).unwrap_err();
    assert_eq!(refusal.code, "repo-ref-malformed");
}

#[test]
fn a_configuration_block_written_by_issue_flow_is_replaced_rather_than_doubled() {
    // The mechanics of a fenced block live in `crate::fence` and are tested
    // there. What belongs here is the one thing this fence declares that a
    // generic one cannot know: which markers it supersedes. Getting this wrong
    // leaves two configuration tables in one SKILL.md, and the agent reads
    // whichever it reaches first.
    let original = concat!(
        "# Skill

",
        "<!-- issue-flow:config:start -->

old table

<!-- issue-flow:config:end -->
"
    );
    let updated = CONFIG_FENCE.upsert(original, "new table");
    assert!(!updated.contains("issue-flow:config"));
    assert!(!updated.contains("old table"));
    assert_eq!(updated.matches(BLOCK_BEGIN).count(), 1);
}

#[test]
fn a_contract_carrying_an_issue_flow_table_reads_its_values_through_the_upgrade() {
    // The upgrade path end to end: an operator on issue-flow who had `squash`
    // must still have `squash` after the block is renamed.
    let original = [
        "<!-- issue-flow:config:start -->",
        "",
        "| Setting | Value here | Skill default |",
        "|---|---|---|",
        "| Merge strategy | squash | merge commit |",
        "",
        "<!-- issue-flow:config:end -->",
        "",
    ]
    .join(
        "
",
    );

    let config = Config::read(&original, None).expect("the old table reads");
    assert_eq!(config.merge, MergeStrategy::Squash);

    let upgraded = CONFIG_FENCE.upsert(&original, &config.render_rows());
    assert_eq!(
        Config::read(&upgraded, None)
            .expect("the new table reads")
            .merge,
        MergeStrategy::Squash
    );
}

/// The literals one setting's message tells an operator to type.
///
/// Backticks mark them, and that is a rule rather than a convention: the
/// messages are prose written for a person, and without a marker there is no
/// way to tell `merge commit` — two words a person types verbatim — from
/// "an absolute directory", which is a description of a value and not one.
///
/// A literal holding a `<placeholder>` is a shape, not a value, and is left out.
fn offered(setting: Setting) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = setting.accepted();
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        let literal = rest[..close].trim();
        if !literal.is_empty() && !literal.contains('<') {
            found.push(literal.to_owned());
        }
        rest = &rest[close + 1..];
    }
    found
}

#[test]
fn every_value_a_setting_offers_is_a_value_it_accepts() {
    // The ratchet, applied to configuration. `accepted()` is the message an
    // operator reads after being refused, and it is the only place they learn
    // what to write. A value named there that the parser rejects is a dead end
    // in the one message whose whole job is not to be one.
    //
    // The same shape as a gate that registers and decides nothing, and as a
    // tool that sends a flag the transport rejects: the end is written by hand,
    // the path is code, and nothing made them agree.
    for setting in SETTINGS {
        let offers = offered(*setting);
        assert!(
            !offers.is_empty(),
            "`{}` offers nothing a person can type: {}",
            setting.label(),
            setting.accepted()
        );
        for value in offers {
            let mut config = Config::default();
            assert!(
                setting.apply(&mut config, &value).is_ok(),
                "`{}` tells an operator to write `{value}` and refuses it",
                setting.label()
            );
        }
    }
}

#[test]
fn the_value_extractor_finds_what_is_actually_offered() {
    // The guard on the guard: an extractor that finds nothing would let every
    // setting above pass without checking a thing.
    assert_eq!(
        offered(Setting::Merge),
        vec!["merge commit", "squash", "rebase"]
    );
    // Including the two-word one, which is why the marker exists.
    assert!(offered(Setting::Delivery).contains(&"ask 30m".to_owned()));
    // A shape is not a value: `github <owner>/<name>` is left out and the bare
    // `github` beside it is kept.
    assert_eq!(
        offered(Setting::Tracker),
        vec!["github", "linear", "trello"]
    );
    // And a free-text setting still has to offer an example that works.
    assert_eq!(offered(Setting::Summary), vec!["English"]);
    assert_eq!(offered(Setting::Body), vec!["English"]);
}

#[test]
fn every_timeout_unit_the_grammar_accepts_is_one_a_test_holds() {
    // Mutation testing found this: deleting the `s` arm and deleting the `h` arm
    // both left the suite green. Two thirds of a grammar people are told they
    // can write, and nothing would have noticed them going away.
    for (written, expected) in [
        ("ask 45s", 45),
        ("ask 45m", 45 * 60),
        ("ask 2h", 2 * 60 * 60),
    ] {
        let local = table(&[("Transition authorisation", written)]);
        let config = Config::read(&local, None)
            .unwrap_or_else(|error| panic!("`{written}` was refused: {error}"));
        assert_eq!(
            config.transitions,
            Authority::Ask {
                timeout: Duration::from_secs(expected)
            },
            "`{written}`"
        );
        // And it survives being written back out, which is where a unit that
        // parses but does not render would show up.
        assert!(
            config
                .render_rows()
                .contains(written.trim_start_matches("ask ")),
            "`{written}` did not render back"
        );
    }

    // A unit nobody offers is refused rather than guessed at.
    for wrong in ["ask 45d", "ask 45", "ask m", "ask 45x"] {
        let local = table(&[("Transition authorisation", wrong)]);
        assert!(
            Config::read(&local, None).is_err(),
            "`{wrong}` was accepted"
        );
    }

    // A space before the unit is accepted, because `parse_duration` trims it and
    // somebody writing `45 m` means forty-five minutes. Written down rather than
    // left to be found: leniency nobody recorded is leniency somebody later
    // removes as a bug.
    let spaced = table(&[("Transition authorisation", "ask 45 m")]);
    assert_eq!(
        Config::read(&spaced, None)
            .expect("a spaced unit parses")
            .transitions,
        Authority::Ask {
            timeout: Duration::from_secs(45 * 60)
        }
    );
}

#[test]
fn a_table_row_is_read_only_when_it_carries_two_cells_of_content() {
    // The other family mutation testing surfaced: every boundary in the row
    // reader — the cell count, the separator test, its negation — could be
    // flipped without a test noticing. These are what stand between a heading
    // and a setting.
    let read = |text: &str| Config::read(text, None);

    // A one-column row is not a setting, and neither is the separator or the
    // header. A file made only of those parses to the defaults.
    let scaffolding = "| Setting | Value here | Skill default |
|---|---|---|
| alone |
";
    assert_eq!(
        read(scaffolding).expect("scaffolding parses"),
        Config::default()
    );

    // A separator written with alignment colons is still a separator.
    let aligned = "| Setting | Value here |
|:---|---:|
| Merge strategy | squash |
";
    assert_eq!(
        read(aligned).expect("an aligned table parses").merge,
        MergeStrategy::Squash
    );

    // And a row whose cells are empty is not a separator — it is a blank row,
    // which names no setting and must not be mistaken for one.
    let blank = "| Setting | Value here |
|---|---|
|  |  |
| Merge strategy | rebase |
";
    assert_eq!(
        read(blank).expect("a blank row is skipped").merge,
        MergeStrategy::Rebase
    );
}

#[test]
fn a_tracker_that_takes_no_argument_refuses_one_rather_than_dropping_it() {
    // `linear` and `trello` are guarded on the rest being empty, and both guards
    // could be replaced with `true` without a test noticing. Then `linear
    // owner/repo` — which somebody writes by analogy with `github owner/repo` —
    // would be accepted with the repository silently discarded, and the run
    // would point at a tracker nobody named.
    for written in ["linear owner/repo", "trello Some Board", "linear x"] {
        let local = table(&[("Tracker", written)]);
        assert!(
            Config::read(&local, None).is_err(),
            "`{written}` was accepted and its argument dropped"
        );
    }
    // And the bare forms still parse.
    for (written, expected) in [("linear", Tracker::Linear), ("trello", Tracker::Trello)] {
        let local = table(&[("Tracker", written)]);
        assert_eq!(
            Config::read(&local, None).expect("a bare tracker").tracker,
            expected
        );
    }
}

#[test]
fn every_selected_document_ships() {
    // Three axes choose a document by setting. The contract now links the
    // protocol and policy documents conditionally, but this remains the guard
    // that each configured value names a file Estigia actually installs.
    let named: Vec<(String, Option<&str>)> = Planning::all()
        .into_iter()
        .map(|planning| (format!("{planning:?}"), planning.document()))
        .chain(
            ReviewProtocol::all()
                .into_iter()
                .map(|protocol| (format!("{protocol:?}"), protocol.document())),
        )
        .chain(
            Judges::all()
                .into_iter()
                .map(|judges| (format!("{judges:?}"), judges.document())),
        )
        .collect();

    let mut selected = 0;
    for (chooser, document) in named {
        let Some(document) = document else { continue };
        selected += 1;
        assert!(
            crate::skill::FILES.iter().any(|file| file.path == document),
            "{document} is named by {chooser} and is not installed"
        );
        assert!(
            crate::skill::SELECTED_BY_SETTING
                .iter()
                .any(|prefix| document.starts_with(prefix)),
            "{document} is chosen by a setting and sits outside SELECTED_BY_SETTING, so the \
             seam guards will apply the wrong reachability rule"
        );
    }
    assert!(
        selected >= 3,
        "only {selected} documents are selected by a setting"
    );
}

#[test]
fn the_three_axes_are_independent() {
    // The modelling error this replaced: `issue-flow`, `sdd` and `rdd` sat in
    // one enum, so choosing one excluded the others — while the documents said
    // in prose that they compose. They answer different questions. issue-flow is
    // not among them at all: it is the substrate, and there is no configuration
    // that turns it off.
    let local = table(&[
        ("Planning", "sdd openspec"),
        ("Review protocol", "receipt-driven"),
        ("Blind judges", "two blind"),
        ("Tracker", "linear"),
    ]);
    let config = Config::read(&local, None).expect("four axes read together");
    assert_eq!(
        config.planning,
        Planning::Sdd {
            openspec: true,
            lite: false
        }
    );
    assert_eq!(config.review_protocol, ReviewProtocol::ReceiptDriven);
    assert_eq!(config.judges, Judges::TwoBlind);
    assert_eq!(config.tracker, Tracker::Linear);

    // And each survives a round trip on its own.
    for planning in Planning::all() {
        for protocol in ReviewProtocol::all() {
            let config = Config {
                planning,
                review_protocol: protocol,
                ..Config::default()
            };
            let reread = Config::read(&config.render_rows(), None).expect("a table Estigia wrote");
            assert_eq!(reread.planning, planning);
            assert_eq!(reread.review_protocol, protocol);
        }
    }

    for wrong in ["issue-flow", "sdd rdd", "waterfall"] {
        let local = table(&[("Planning", wrong)]);
        assert!(
            Config::read(&local, None).is_err(),
            "`{wrong}` was accepted as a planning protocol"
        );
    }
}

#[test]
fn every_judge_policy_names_a_document_that_ships() {
    // The third axis chosen by a setting rather than by a link, held to the same
    // seam as bindings and methodologies: a value that names a document Estigia
    // does not install sends the agent to a file that is not there.
    for judges in Judges::all() {
        let Some(document) = judges.document() else {
            continue;
        };
        assert!(
            crate::skill::FILES.iter().any(|file| file.path == document),
            "{document} is named by {judges:?} and is not installed"
        );
    }
}

#[test]
fn the_judge_policy_survives_a_round_trip() {
    for judges in Judges::all() {
        let config = Config {
            judges,
            ..Config::default()
        };
        let reread = Config::read(&config.render_rows(), None).expect("a table Estigia wrote");
        assert_eq!(reread.judges, judges);
    }

    for wrong in ["three", "blind judges", "two blind extra"] {
        let local = table(&[("Blind judges", wrong)]);
        assert!(
            Config::read(&local, None).is_err(),
            "`{wrong}` was accepted"
        );
    }
    let setting = Setting::Judges;
    assert!(setting.answers().choices.contains(&"five blind"));
    assert_eq!(Config::default().judges, Judges::Single);
    let mut alias = Config::default();
    setting.apply(&mut alias, "five").expect("the alias parses");
    assert_eq!(alias.judges, Judges::FiveBlind);
    assert!(
        setting
            .means("five blind")
            .is_some_and(|meaning| meaning.contains("3-of-5"))
    );
}

#[test]
fn an_operator_may_add_a_boundary_and_may_not_take_one_away() {
    // Estigia's built-in list knows git and GitHub. It cannot know that a
    // repository ships with `npm publish`, and until this existed that ran as a
    // routine write — able to ride the renewal window, which is the one thing a
    // boundary must never do.
    let local = table(&[("Irreversible commands", "npm publish, terraform apply")]);
    let config = Config::read(&local, None).expect("a declared boundary reads");
    assert_eq!(config.boundaries, ["npm publish", "terraform apply"]);

    // It reaches the classifier, and it reaches it as a boundary rather than a
    // write — a setting the gate does not consult is a setting that does
    // nothing.
    let (action, how) = crate::harness::classify_with(
        "Bash",
        &serde_json::json!({"command": "npm publish --access public"}),
        &config.boundaries,
    );
    assert_eq!(how, crate::harness::Sensitivity::Boundary);
    assert!(matches!(action, crate::harness::Action::Boundary { .. }));

    // Additive only: the built-ins hold whatever the operator wrote, so no
    // value of this setting can make the gate looser than it ships.
    let (_, how) = crate::harness::classify_with(
        "Bash",
        &serde_json::json!({"command": "git push origin main"}),
        &config.boundaries,
    );
    assert_eq!(how, crate::harness::Sensitivity::Boundary);

    // Round trip, and the empty case reads as `none` rather than as a blank.
    let reread = Config::read(&config.render_rows(), None).expect("it writes back");
    assert_eq!(reread.boundaries, config.boundaries);
    assert!(
        Config::default()
            .render_rows()
            .contains("| Irreversible commands | none |"),
        "the empty case does not render as `none`"
    );

    // A list of nothing but separators is a boundary the operator believes they
    // declared, so it is refused rather than silently dropped.
    assert!(Config::read(&table(&[("Irreversible commands", ", ,")]), None).is_err());
}

#[test]
fn a_hand_written_cell_reads_the_same_as_the_transport_reads_it() {
    // Found by `tests/differential.rs` on its first run, and it was not a
    // formatting quibble: the transport read `squash` out of a decorated cell
    // and Estigia refused the whole contract — then the gate fell back to
    // `unwrap_or_default()` and enforced the defaults while the transport
    // honoured what the operator wrote. One file, two answers, no error.
    let decorated = |value: &str| {
        let document = format!("| Merge strategy | {value} |\n");
        settings::rows(&document)
            .into_iter()
            .next()
            .expect("one row")
            .1
    };

    // The three shapes the shipped table itself teaches.
    assert_eq!(decorated("`squash`"), "squash");
    assert_eq!(decorated("**squash**"), "squash");
    assert_eq!(decorated("`squash` — keeps history off the base"), "squash");
    assert_eq!(decorated("squash (the usual choice)"), "squash");
    assert_eq!(decorated("  squash  "), "squash");

    // And what must NOT be cut: a value is not an explanation because it holds
    // a dash or a bracket. Over-trimming here would silently change a setting.
    assert_eq!(decorated("merge commit"), "merge commit");
    assert_eq!(decorated("merge—commit"), "merge—commit");
    assert_eq!(decorated("two blind"), "two blind");
    assert_eq!(decorated("npm publish"), "npm publish");
}

#[test]
fn sdd_lite_is_an_axis_of_its_own_and_not_a_third_value() {
    let planning = |cell: &str| {
        let local = table(&[("Planning", cell)]);
        Config::read(&local, None).map(|config| config.planning)
    };

    assert_eq!(
        planning("sdd").unwrap(),
        Planning::Sdd {
            openspec: false,
            lite: false
        }
    );
    assert_eq!(
        planning("sdd lite").unwrap(),
        Planning::Sdd {
            openspec: false,
            lite: true
        }
    );
    assert_eq!(
        planning("sdd openspec").unwrap(),
        Planning::Sdd {
            openspec: true,
            lite: false
        }
    );

    // Both axes at once, and — the point — in either order. Where the artifacts
    // live and how many there are are two questions; an order that mattered
    // would be one somebody gets wrong with no way to tell.
    let both = Planning::Sdd {
        openspec: true,
        lite: true,
    };
    assert_eq!(planning("sdd lite openspec").unwrap(), both);
    assert_eq!(planning("sdd openspec lite").unwrap(), both);
    assert_eq!(planning("SDD  Lite   OpenSpec").unwrap(), both);
    // `short` says the same thing.
    assert_eq!(
        planning("sdd short").unwrap(),
        Planning::Sdd {
            openspec: false,
            lite: true
        }
    );

    // Every one of them survives a round trip through the table it is written
    // into, which is the only thing that makes the setting real.
    for planning in Planning::all() {
        let config = Config {
            planning,
            ..Config::default()
        };
        let reread = Config::read(&config.render_rows(), None).expect("Estigia wrote this");
        assert_eq!(reread.planning, planning, "{planning:?} did not survive");
    }

    // A word neither axis knows is refused rather than ignored: silently
    // dropping it would install a methodology nobody chose.
    assert!(planning("sdd deluxe").is_err());
    assert!(
        planning("direct lite").is_err(),
        "direct has no phases to shorten"
    );
}

#[test]
fn deciding_per_issue_is_handed_the_rule_that_decides() {
    // The gap `auto` was added to close, held here so it cannot reopen.
    //
    // `protocols/sdd.md` carries the only rule that answers *whether to plan
    // this change* — "Ambiguity, and nothing else. Not size, and not risk" — and
    // `Planning::document` is what puts a protocol in front of a run. Under
    // `direct` it answers `None`, correctly: nothing decides, because the
    // operator already did.
    //
    // A value that defers the decision to the run and *also* answers `None` here
    // would be asking for a judgement while withholding the sentence that makes
    // it, and the run would answer from whatever it already believed. That is
    // not a per-issue protocol; it is improvisation with an operator's blessing
    // on it. Written as an assertion rather than a comment because the two lines
    // that must agree — the match arm and this claim — sit in different files.
    for planning in Planning::all() {
        let expected = match planning {
            Planning::Direct => None,
            Planning::Sdd { .. } => Some("protocols/sdd.md"),
        };
        assert_eq!(
            planning.document(),
            expected,
            "{planning:?} is handed the wrong protocol document"
        );
    }

    // And the document it names is one that ships, or the contract points a run
    // at a file that is not there.
    let named = Planning::Sdd {
        openspec: false,
        lite: false,
    }
    .document()
    .expect("`sdd` names a protocol");
    assert!(
        crate::skill::FILES.iter().any(|file| file.path == named),
        "`sdd` names {named}, which the payload does not ship"
    );
}

#[test]
fn direct_work_delegation_thresholds_do_not_select_sdd() {
    let contract = include_str!("../../skill/SKILL.md");
    let section = contract
        .split_once("## Direct Work Delegation\n")
        .map(|(_, rest)| rest.split("\n## ").next().unwrap_or(rest))
        .expect("the always-loaded contract owns the direct-work delegation rule");

    let rows: std::collections::BTreeMap<_, _> = section
        .lines()
        .filter_map(|line| {
            let cells: Vec<_> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            (cells.len() == 3 && cells[0] != "Work" && !cells[0].starts_with("---"))
                .then(|| (cells[0], (cells[1], cells[2])))
        })
        .collect();

    assert_eq!(
        rows.get("Bounded decide/verify reads"),
        Some(&(
            "Expected understanding of 1-3 files may stay inline",
            "Expected understanding of 4+ files delegates one narrow mapper"
        ))
    );
    assert_eq!(
        rows.get("Reading for a write or broad research"),
        Some(&(
            "-",
            "Delegate reading that prepares a write, broad research, or context compression"
        ))
    );
    assert_eq!(
        rows.get("Writing"),
        Some(&(
            "One already-understood mechanical file may stay inline",
            "2+ non-trivial files delegate one writer"
        ))
    );
    assert_eq!(rows.len(), 3, "the threshold table gained a second policy");

    let normalize = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let expected = concat!(
        "Use the smallest useful topology and keep one writer. Delegate only when the expected work crosses\n",
        "one of these boundaries:\n",
        "| Work | Stay inline | Delegate |\n",
        "|---|---|---|\n",
        "| Bounded decide/verify reads | Expected understanding of 1-3 files may stay inline | Expected understanding of 4+ files delegates one narrow mapper |\n",
        "| Reading for a write or broad research | - | Delegate reading that prepares a write, broad research, or context compression |\n",
        "| Writing | One already-understood mechanical file may stay inline | 2+ non-trivial files delegate one writer |\n",
        "Tests, builds, installs, and native review actions may each use a fresh per-action worker without changing the route. Child workers do not gain orchestration authority.\n",
        "Crossing a threshold selects delegated direct work only. It MUST NOT select `sdd`, create SDD state or artifacts, or invoke an `sdd-*` phase. Size, file count, and risk do not select a planning protocol, and this rule does not change an operator's configured `Planning` mode."
    );
    assert_eq!(
        normalize(section),
        normalize(expected),
        "the direct-work owner section changed without updating its semantic binding"
    );
}

#[test]
fn a_phase_the_protocol_never_runs_is_a_model_nobody_will_consult() {
    // `Model routing` took `explore=opus` under `sdd lite` and reported it set.
    // The short form runs spec and tasks and nothing else, so that was an
    // operator's deliberate choice landing on a phase that never happens — and
    // the row said nothing, because every phase key looked alike to it.
    let routing = crate::config::ModelRouting::parse(
        "explore=opus, spec=sonnet, design=haiku, apply=fable, implementer=opus",
    )
    .expect("a routing over four phases and a role");

    // The short form: three of the five planning phases are dead.
    assert_eq!(
        routing.inert_phases(Planning::Sdd {
            openspec: false,
            lite: true
        }),
        vec!["explore", "design"],
        "the short form did not name the phases it will never reach"
    );

    // The full form runs all five, so nothing named is inert.
    assert!(
        routing
            .inert_phases(Planning::Sdd {
                openspec: false,
                lite: false
            })
            .is_empty(),
        "full SDD reported a planning phase as unreachable"
    );

    // `direct` runs no planning phase at all, so every one named is inert.
    assert_eq!(
        routing.inert_phases(Planning::Direct),
        vec!["explore", "spec", "design"],
        "`direct` runs no phase and did not say so"
    );

    // `auto` is a spelling of `sdd`, not a protocol of its own, so it inherits
    // the full form's shape and rules nothing out in advance. There was briefly
    // a `Planning::Auto` here with four values of its own; it asked an operator
    // to choose between `sdd` and the per-change decision `sdd` already makes.
    let spelled = Setting::Planning
        .answers()
        .choices
        .iter()
        .any(|choice| choice.starts_with("auto"));
    assert!(!spelled, "`auto` is offered as a protocol of its own again");
    let mut config = Config::default();
    Setting::Planning
        .apply(&mut config, "auto")
        .expect("`auto` is still a word an operator may reach for");
    assert_eq!(
        config.planning,
        Planning::Sdd {
            openspec: false,
            lite: false
        },
        "`auto` stopped resolving to the protocol that already decides per change"
    );

    // `apply` is named in the routing above and reported by none of them, which
    // is the distinction `PLANNED_PHASES` exists to draw: writing the code
    // happens under every protocol, `direct` included.
    for planning in Planning::all() {
        assert!(
            !routing.inert_phases(planning).contains(&"apply"),
            "{planning:?} called `apply` unreachable, and every protocol applies"
        );
    }
}

#[test]
fn a_neighbouring_row_moving_never_makes_a_table_stop_loading() {
    // The refusal this deliberately is not. `Planning` and `Model routing` are
    // set in either order, so rejecting an inert phase at parse time would mean
    // a table that read yesterday stops reading today because a *different* row
    // moved — and the repair would need a tool that now refuses to load it. The
    // `Project board` incident is the same shape and cost exactly that.
    for planning in Planning::all() {
        let mut config = Config {
            planning,
            ..Config::default()
        };
        Setting::Models
            .apply(&mut config, "explore=opus, design=haiku")
            .unwrap_or_else(|error| {
                panic!("a routing naming every phase was refused under {planning:?}: {error}")
            });
        let reread = Config::read(&config.render_rows(), None)
            .unwrap_or_else(|error| panic!("{planning:?} wrote a table it cannot read: {error}"));
        assert_eq!(
            reread.models.as_value(),
            "explore=opus, design=haiku",
            "{planning:?} lost a routing on the way through the table"
        );
    }
}

#[test]
fn a_model_can_be_named_for_each_delegated_role() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    // Empty by default, and `unset` says so out loud.
    assert!(Config::default().models.by_role.is_empty());
    assert_eq!(Config::default().models.as_value(), "unset");
    assert!(routing("unset").expect("unset parses").by_role.is_empty());

    let some = routing("implementer=opus, judge=haiku").expect("a routing");
    assert_eq!(some.get(Role::Implementer), Some("opus"));
    assert_eq!(some.get(Role::Judge), Some("haiku"));
    // A role nobody named runs on whatever the agent picks, which is what
    // "unset" has to mean per role and not only for the cell as a whole.
    assert_eq!(some.get(Role::Reviewer), None);

    // Rendered in role order, not insertion order: a table that reorders
    // itself between two writes shows a diff where nothing changed.
    assert_eq!(some.as_value(), "implementer=opus, judge=haiku");
    let reversed = routing("judge=haiku,implementer=opus").expect("a routing");
    assert_eq!(reversed.as_value(), some.as_value());
    assert_eq!(reversed, some);

    // Every shape that is not a routing is refused rather than partly read.
    // Silently dropping a piece would leave a role on whatever the agent picks
    // while the table says otherwise.
    for bad in [
        "opus",                     // no role
        "architect=opus",           // a role nothing spawns
        "implementer=",             // named without a model
        "implementer=opus, =haiku", // a model without a role
        "judge=a, judge=b",         // said twice, and only one can happen
    ] {
        assert!(routing(bad).is_err(), "{bad:?} was accepted");
    }

    // And it survives the table it is written into.
    let config = Config {
        models: some.clone(),
        ..Config::default()
    };
    let reread = Config::read(&config.render_rows(), None).expect("Estigia wrote this");
    assert_eq!(reread.models, some);
}

#[test]
fn model_routing_targets_and_mutations_have_one_canonical_owner() {
    let targets = ModelRouting::targets();
    assert_eq!(targets.first().copied(), Some("orchestrate"));
    let mut unique = targets.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        targets.len(),
        "a routing target appears twice"
    );
    for target in Role::all()
        .into_iter()
        .map(Role::as_str)
        .chain(crate::config::STATES.iter().copied())
        .chain(crate::config::SDD_PHASES.iter().copied())
        .chain(crate::config::ORCHESTRATED_ROLES.iter().copied())
    {
        assert!(
            targets.contains(&target),
            "{target:?} is accepted but not offered"
        );
    }

    let mut routing = ModelRouting::parse("reviewer=custom, design=opus").expect("a routing");
    assert!(routing.assign("orchestrate", "gpt-5.6"));
    assert_eq!(routing.for_target("reviewer"), Some("custom"));
    assert_eq!(routing.for_target("orchestrate"), Some("gpt-5.6"));
    assert_eq!(
        routing.as_value(),
        "reviewer=custom, design=opus, orchestrate=gpt-5.6"
    );
    assert!(!routing.assign("not-a-target", "model"));
    assert!(!routing.assign("apply", "  "));

    assert!(routing.remove("orchestrate"));
    assert!(!routing.as_value().contains("orchestrate=unset"));
    assert_eq!(routing.for_target("reviewer"), Some("custom"));
    routing.clear();
    assert_eq!(routing.as_value(), "unset");
}

#[test]
fn visible_model_targets_follow_the_active_planning_protocol_in_ui_order() {
    let names = |planning| {
        ModelRouting::visible_targets(planning)
            .into_iter()
            .map(|target| target.name)
            .collect::<Vec<_>>()
    };
    let delegated = [
        "implementer",
        "reviewer",
        "judge",
        "strategist",
        "analyst",
        "builder",
        "refactorer",
        "validator",
        "auditor",
    ];

    assert_eq!(
        names(Planning::Direct),
        ["orchestrate", "apply"]
            .into_iter()
            .chain(delegated)
            .collect::<Vec<_>>()
    );
    for openspec in [false, true] {
        assert_eq!(
            names(Planning::Sdd {
                openspec,
                lite: false,
            }),
            [
                "orchestrate",
                "explore",
                "propose",
                "spec",
                "design",
                "tasks",
                "apply",
            ]
            .into_iter()
            .chain(delegated)
            .collect::<Vec<_>>()
        );
        assert_eq!(
            names(Planning::Sdd {
                openspec,
                lite: true,
            }),
            ["orchestrate", "spec", "tasks", "apply"]
                .into_iter()
                .chain(delegated)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn visible_model_target_descriptors_are_unique_and_accepted_by_persistence() {
    for planning in Planning::all() {
        let targets = ModelRouting::visible_targets(planning);
        let mut names = targets.iter().map(|target| target.name).collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), targets.len(), "duplicate under {planning:?}");

        for target in targets {
            let mut routing = ModelRouting::default();
            assert!(
                routing.assign(target.name, "custom/model"),
                "visible target {:?} is not persisted",
                target.name
            );
        }
    }
}

#[test]
fn one_model_id_cannot_cross_the_persisted_entry_delimiters() {
    for model in [
        "provider,model",
        "provider|model",
        "provider\rmodel",
        "provider\nmodel",
    ] {
        let mut routing = ModelRouting::default();
        assert!(!routing.assign("orchestrate", model), "accepted {model:?}");
        assert!(
            ModelRouting::parse(&format!("orchestrate={model}")).is_none(),
            "parsed {model:?}"
        );
    }
}

#[test]
fn a_model_can_be_named_per_phase_as_well_as_per_role() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    let mixed = routing("judge=haiku, analysis=opus, in-progress=sonnet").expect("a routing");
    assert_eq!(mixed.for_state("analysis"), Some("opus"));
    assert_eq!(mixed.for_state("in-progress"), Some("sonnet"));
    assert_eq!(mixed.for_state("done"), None);
    assert_eq!(mixed.get(Role::Judge), Some("haiku"));

    // The role wins over the phase. A judge working while the issue sits in
    // `review` is still a judge, and letting the phase override it would
    // silently undo the more specific of the two settings.
    let both = routing("judge=haiku, review=opus").expect("a routing");
    assert_eq!(
        both.resolve(Some(Role::Judge), Some("review")),
        Some("haiku")
    );
    // The phase answers when nothing more specific did.
    assert_eq!(
        both.resolve(Some(Role::Reviewer), Some("review")),
        Some("opus")
    );
    assert_eq!(both.resolve(None, Some("review")), Some("opus"));
    assert_eq!(both.resolve(None, Some("ready")), None);

    // Rendered roles-then-states in a fixed order, so two writes of the same
    // routing produce the same cell and a diff means something changed.
    assert_eq!(
        routing("done=a, judge=b, analysis=c")
            .expect("a routing")
            .as_value(),
        "judge=b, analysis=c, done=a"
    );

    // Every state the binding declares is a key, and nothing else is.
    for state in crate::config::STATES {
        assert!(
            routing(&format!("{state}=opus")).is_ok(),
            "{state} is a workflow state and was refused"
        );
    }
    assert!(
        routing("shipped=opus").is_err(),
        "a state nothing produces was accepted"
    );

    // And it survives the table.
    let config = Config {
        models: mixed.clone(),
        ..Config::default()
    };
    let reread = Config::read(&config.render_rows(), None).expect("Estigia wrote this");
    assert_eq!(reread.models, mixed);
}

#[test]
fn the_states_a_routing_accepts_are_the_ones_the_contract_declares() {
    // Written in two places — here and the contract's own sentence — so they
    // are crossed rather than trusted. A renamed state would make a routing
    // match nothing, and a model nobody selected is indistinguishable from one
    // nobody configured.
    let contract = include_str!("../../skill/SKILL.md");
    for state in crate::config::STATES {
        assert!(
            contract.contains(&format!("`{state}`")),
            "`{state}` is a routing key and the contract never names it"
        );
    }
    // And the other direction, on the sentence that lists them: a state the
    // contract declares and this does not is one an operator cannot route.
    for declared in [
        "analysis",
        "ready",
        "in-progress",
        "review",
        "blocked",
        "done",
    ] {
        assert!(
            crate::config::STATES.contains(&declared),
            "the contract declares `{declared}` and no model can be named for it"
        );
    }
}

#[test]
fn every_named_disposition_uses_a_transport_state() {
    let contract = include_str!("../../skill/SKILL.md");
    let gates = contract
        .split_once("## Decision Gates")
        .expect("the contract has decision gates")
        .1
        .split_once("## Execution Steps")
        .expect("execution follows the decision gates")
        .0;
    let mut destinations = Vec::new();

    for fragment in gates.split("-> `").skip(1) {
        let destination = fragment
            .split_once('`')
            .expect("a disposition closes its state code span")
            .0;
        assert!(
            crate::config::STATES.contains(&destination),
            "the contract routes a disposition to unknown state `{destination}`"
        );
        destinations.push(destination);
    }

    assert!(
        destinations.len() >= crate::config::STATES.len(),
        "the disposition crossing found too few destinations"
    );
    assert!(
        gates.contains("ordinary delivery permission -> `review`"),
        "the contract does not keep ordinary delivery permission in review"
    );
    assert!(
        gates.contains(
            "exceptional human adjudication outside ordinary delivery gates -> `blocked`"
        ),
        "the contract does not name where exceptional human adjudication waits go"
    );
    assert!(
        contract.contains("built work cleared to continue delivery returns to `review`"),
        "the contract does not restore discharged built work to delivery"
    );
    assert!(
        contract.contains("work requiring implementation returns to `ready`"),
        "the contract does not restore discharged implementation work to ready"
    );
}

#[test]
fn a_model_can_be_named_per_sdd_phase_and_the_most_specific_wins() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    // The shape an operator actually asks for: design thinks with one model,
    // applying writes with a cheaper one, orchestration with a third.
    let sdd = routing("design=opus-5, apply=sonnet-5, orchestrate=fable-5").expect("a routing");
    assert_eq!(sdd.for_phase("design"), Some("opus-5"));
    assert_eq!(sdd.for_phase("apply"), Some("sonnet-5"));
    assert_eq!(sdd.for_phase("orchestrate"), Some("fable-5"));

    // Models are opaque strings on purpose: another agent's list is not this
    // one's, and a hard-coded set would refuse the model somebody has.
    let elsewhere = routing("orchestrate=gpt-5.6, apply=kimi-k3").expect("a routing");
    assert_eq!(elsewhere.for_phase("orchestrate"), Some("gpt-5.6"));
    assert_eq!(elsewhere.for_phase("apply"), Some("kimi-k3"));

    // Most specific first: phase beats role beats state. Somebody who named a
    // model for `design` did it to choose what designs.
    let all = routing("design=opus, implementer=sonnet, in-progress=haiku").expect("a routing");
    assert_eq!(
        all.resolve_in(Some(Role::Implementer), Some("in-progress"), Some("design")),
        Some("opus")
    );
    assert_eq!(
        all.resolve_in(Some(Role::Implementer), Some("in-progress"), None),
        Some("sonnet")
    );
    assert_eq!(
        all.resolve_in(None, Some("in-progress"), None),
        Some("haiku")
    );
    assert_eq!(all.resolve_in(None, Some("done"), Some("spec")), None);

    // Every phase is a key, and a word that is none of the three kinds is not.
    for phase in crate::config::SDD_PHASES {
        assert!(
            routing(&format!("{phase}=m")).is_ok(),
            "{phase} was refused"
        );
    }
    assert!(routing("vibes=m").is_err());

    // And a routing mixing all three kinds survives the table.
    let reread = Config::read(
        &Config {
            models: all.clone(),
            ..Config::default()
        }
        .render_rows(),
        None,
    )
    .expect("Estigia wrote this");
    assert_eq!(reread.models, all);
}

#[test]
fn the_renewal_window_can_be_narrowed_and_never_widened() {
    let window = |cell: &str| {
        let local = table(&[("Renewal window", cell)]);
        Config::read(&local, None).map(|config| config.window)
    };
    let built_in = crate::harness::RENEWAL_WINDOW;

    assert_eq!(Config::default().window, built_in);
    assert_eq!(window("default").unwrap(), built_in);
    // Shorter: asks the tracker more often, which is the cheap half of a trade
    // somebody on a busy repository legitimately wants.
    assert_eq!(window("30s").unwrap(), Duration::from_secs(30));
    assert_eq!(window("1m").unwrap(), Duration::from_secs(60));
    // Exactly the built-in is fine; it is the same window.
    assert_eq!(window("2m").unwrap(), built_in);

    // Longer is refused, not clamped. An operator who asked for ten minutes
    // and silently got two would believe the gate is looser than it is — and a
    // setting that can loosen a guard rail turns it into a preference.
    for longer in ["3m", "10m", "1h", "121s"] {
        assert!(window(longer).is_err(), "{longer} widened the window");
    }
    // And zero is not a window: it would ask the tracker before every keystroke
    // rather than tightening anything meaningful.
    assert!(window("0s").is_err());
    assert!(window("nonsense").is_err());

    // It survives the table, in both spellings.
    for config in [
        Config::default(),
        Config {
            window: Duration::from_secs(45),
            ..Config::default()
        },
    ] {
        let reread = Config::read(&config.render_rows(), None).expect("Estigia wrote this");
        assert_eq!(reread.window, config.window);
    }
}

#[test]
fn an_orchestrators_own_vocabulary_is_accepted() {
    // Somebody running an orchestrator alongside Estigia thinks in its names. A
    // setting that refuses the word they have in front of them is one they
    // conclude does not work — and the cost of accepting a key Estigia never
    // spawns is nothing, because an unread key routes nobody.
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };
    for role in crate::config::ORCHESTRATED_ROLES {
        assert!(
            routing(&format!("{role}=opus")).is_ok(),
            "{role} was refused"
        );
    }

    let forge = routing("strategist=opus-5, builder=sonnet-5, auditor=haiku").expect("a routing");
    assert_eq!(forge.for_phase("strategist"), Some("opus-5"));
    assert_eq!(forge.for_phase("builder"), Some("sonnet-5"));
    assert_eq!(forge.for_phase("auditor"), Some("haiku"));

    // Estigia's own three still win where they overlap in meaning, because
    // `reviewer` is a role this configuration really does create.
    let both = routing("reviewer=opus, validator=sonnet").expect("a routing");
    assert_eq!(both.get(Role::Reviewer), Some("opus"));
    assert_eq!(both.for_phase("validator"), Some("sonnet"));

    // And a word belonging to no vocabulary is still refused.
    assert!(routing("wizard=opus").is_err());

    // No key is claimed twice across the three lists, or one would shadow
    // another and an operator could not tell which model they had set.
    let mut all: Vec<&str> = Role::all().into_iter().map(Role::as_str).collect();
    all.extend(crate::config::STATES);
    all.extend(crate::config::SDD_PHASES);
    all.extend(crate::config::ORCHESTRATED_ROLES);
    let unique: std::collections::BTreeSet<&str> = all.iter().copied().collect();
    assert_eq!(all.len(), unique.len(), "two vocabularies claim one key");
}

#[test]
fn the_two_step_lists_are_the_table_in_two_halves() {
    // Both say "in table order" in their own documentation, and nothing checked
    // it. `Renewal window` moved from one list to the other and went in at the
    // front, so the setup screen offered the repository's rows in one order
    // while the contract wrote them in another — a difference an operator sees
    // twice and can do nothing about.
    //
    // Stated as the split it is: every setting appears exactly once across the
    // two, and each list is the table with the other half removed.
    let table: Vec<Setting> = SETTINGS.to_vec();
    for (name, half) in [
        ("AGENT_SETTINGS", AGENT_SETTINGS),
        ("EVERYWHERE_SETTINGS", EVERYWHERE_SETTINGS),
        ("MACHINE_SETTINGS", MACHINE_SETTINGS),
    ] {
        let expected: Vec<Setting> = table
            .iter()
            .copied()
            .filter(|setting| half.contains(setting))
            .collect();
        assert_eq!(
            half.to_vec(),
            expected,
            "{name} is not the table with the other half removed"
        );
    }

    // And the two halves are the whole table, with nothing counted twice and
    // nothing left out — a setting in neither would exist in the contract and
    // on no step of the screen that writes it.
    let mut all: Vec<Setting> = AGENT_SETTINGS.to_vec();
    all.extend_from_slice(EVERYWHERE_SETTINGS);
    all.extend_from_slice(MACHINE_SETTINGS);
    all.sort_by_key(|setting| table.iter().position(|other| other == setting));
    assert_eq!(
        all, table,
        "the three lists do not add up to the table the contract writes"
    );

    // Each half by its own declared scope, which is what puts a row on a step.
    for setting in AGENT_SETTINGS {
        assert_eq!(setting.scope(), Scope::Agent, "{setting:?}");
    }
    for setting in EVERYWHERE_SETTINGS {
        assert_eq!(setting.scope(), Scope::Everywhere, "{setting:?}");
    }
}

#[test]
fn a_value_the_table_cannot_carry_is_refused_before_it_is_written() {
    // A cell is delimited by `|` and lives on one line, and nothing escapes
    // either. So `make deploy | tee log` — a plausible command, and a plausible
    // thing to declare a one-way door — was written, read back as `make deploy`,
    // and refused as `setting-shadowed-by-local-file`: the right detection with
    // the wrong cause, sending the operator to look for a row in a file that
    // does not exist.
    for (setting, value) in [
        (Setting::Boundaries, "make deploy | tee log"),
        (Setting::Board, "acme/7 | mirror"),
        (Setting::Summary, "Espa\u{00f1}ol\nand more"),
    ] {
        let refusal = setting
            .apply(&mut Config::default(), value)
            .expect_err("the table cannot hold this");
        assert_eq!(refusal.code, "config-value-untableable", "{setting:?}");
        // And says which character, because "invalid" sends somebody looking at
        // the wrong half of what they typed.
        let said = format!("{refusal}");
        assert!(
            said.contains("`|`") || said.contains("line break"),
            "{setting:?}: it does not say what cannot be held: {said}"
        );
    }

    // And the markers around the row, for the same reason one level out. This
    // half was missing, and it was worse than the one that was there: a value
    // carrying the closing marker was **accepted**, closed the block in the
    // middle of its own table, and the next `config set` appended a second
    // table under a second pair of markers. The contract ended up holding two
    // of every setting — and an agent reads whichever it reaches first.
    //
    // Every marker the fence knows, from the fence, so a pair added there is
    // covered here the day it is added. The superseded pair delimits nothing
    // this build writes and everything an installation upgraded from issue-flow
    // still has.
    let fence = crate::config::CONFIG_FENCE;
    let markers: Vec<&str> = [fence.begin, fence.end]
        .into_iter()
        .chain(
            fence
                .superseded
                .iter()
                .flat_map(|(open, close)| [*open, *close]),
        )
        .collect();
    assert!(
        markers.len() >= 4,
        "the fence names too few markers to check"
    );
    for marker in markers {
        let refusal = Setting::Summary
            .apply(&mut Config::default(), &format!("English {marker}"))
            .expect_err("a marker cannot be held either");
        assert_eq!(refusal.code, "config-value-untableable");
        assert!(
            format!("{refusal}").contains(marker),
            "it does not say which marker: {refusal}"
        );
    }

    // The same values without it are still accepted: this refuses a character,
    // not a shape.
    Setting::Boundaries
        .apply(&mut Config::default(), "make deploy, tee log")
        .expect("commas are how the list separates");
    // And an ordinary HTML comment is not a marker. Refusing every `<!--`
    // would be refusing a shape, which is what the line above exists to say.
    Setting::Summary
        .apply(&mut Config::default(), "English <!-- a note -->")
        .expect("a comment that delimits nothing is not a delimiter");
}

#[test]
fn every_answer_an_operator_can_give_survives_being_written_and_read_back() {
    // A property rather than a comparison: the table is written by this crate
    // and read by this crate, so nothing else would notice a value that comes
    // back different. It would look like the operator never set it — and the
    // row would still be on screen saying they had.
    //
    // Every accepted value of every setting, from the settings themselves, so a
    // new one is covered the day it is added rather than the day somebody
    // remembers this test.
    let mut checked = 0;
    let mut lost: Vec<String> = Vec::new();
    for setting in SETTINGS {
        for value in setting.answers().choices {
            let mut config = Config::default();
            if setting.apply(&mut config, value).is_err() {
                // An open vocabulary offers `unset` beside a shape to type;
                // the ones that do not parse here are covered by the ones that
                // do.
                continue;
            }
            checked += 1;
            let written = crate::config::settings::render_rows(&config);
            let read = crate::config::table_rows(&written);
            let back = read
                .iter()
                .find(|(key, _)| key == setting.label())
                .map(|(_, value)| value.clone());
            let expected = setting.value_of(&config);
            if back.as_deref() != Some(expected.as_str()) {
                lost.push(format!(
                    "{}: set {value:?}, written {expected:?}, read back {back:?}",
                    setting.label()
                ));
            }
        }
    }
    assert!(
        checked > 20,
        "no accepted value was exercised, so this refutes nothing: {checked}"
    );
    assert!(
        lost.is_empty(),
        "{} answers do not survive the table: {lost:?}",
        lost.len()
    );
}

/// A row the binding acts on is a row this reader finds.
///
/// The binding looks a setting up by **prefix** — `cfg` walks the parsed table
/// and returns the first key that `startswith` the label — and this reader
/// compared whole words. So a first cell with anything after the label is a row
/// the transport honours and Estigia does not, and the direction is the
/// dangerous one.
///
/// Measured end to end on the installed pair, with
/// `| Project board (mine) | acme/7 |` in the contract:
///
/// ```text
/// python github.py config  →  board.enabled=True owner=acme number=7
/// estigia config list      →  Project board  none
/// ```
///
/// One file, one row, two answers: the transport mirrors every issue to that
/// board while the gate and `config list` report it switched off.
#[test]
fn a_row_the_binding_acts_on_is_a_row_this_reader_finds() {
    let contract = |row: &str| {
        format!(
            "<!-- estigia:config:start -->\n\
             | Setting | Value here | Skill default |\n|---|---|---|\n\
             {row}\n<!-- estigia:config:end -->\n"
        )
    };
    let board = |row: &str| {
        crate::config::Config::read(&contract(row), None)
            .expect("the table reads")
            .board
    };

    // The floor: the exact spelling still arrives, and an unrelated row still
    // does not. Without both, matching everything would pass this.
    assert_ne!(
        board("| Project board | acme/7 |"),
        None,
        "the exact spelling stopped being read"
    );
    assert_eq!(
        board("| Renewal window | 30s |"),
        None,
        "a row about something else is being read as this setting"
    );

    for row in [
        "| Project board (mine) | acme/7 |",
        "| Project boards | acme/7 |",
        "| Project board — the one we use | acme/7 |",
    ] {
        assert_ne!(
            board(row),
            None,
            "the transport acts on `{row}` and this reader reports the board switched off"
        );
    }
}

/// A label this crate forgives is one the transport reads too.
///
/// The reverse of what stood here, and the reverse is the fix. Two readers read
/// one table and this crate was the more forgiving: it collapses runs of
/// whitespace and strips the backticks a person writing markdown puts round a
/// name, and the transport's own lookup does neither. Measured on the installed
/// pair, `| ``Project board`` | acme/7 |`:
///
/// ```text
/// Project board          gate acme/7   transport acme/7
/// `Project board`        gate acme/7   transport ''
/// Project  board         gate acme/7   transport ''
/// ```
///
/// So `estigia config list` reported a board and the transport mirrored nothing
/// to any board, and `doctor` carried a **BROKEN** row saying so — a report
/// instead of a fix, because closing it by making this crate stricter would have
/// dropped the operator's declared boundaries for every other row.
///
/// It is closed the third way, which nobody had written down: the live context
/// no longer reads the operator's cells at all. It reads *this crate's own
/// rendering* of the configuration, layered by the one reader that knows which
/// document overrides which. Whatever the operator typed, both sides now answer
/// the value this crate read out of it — so the `doctor` row went with the gap,
/// and this is what stands in its place.
#[test]
fn a_label_this_crate_forgives_is_one_the_transport_reads_too() {
    let marked =
        |rows: &str| format!("<!-- estigia:config:start -->\n{rows}<!-- estigia:config:end -->\n");

    for label in [
        "Project board",
        "Project board (mine)",
        "`Project board`",
        "Project  board",
    ] {
        let home = tempfile::tempdir().expect("a home");
        let skill = home.path().join("skill");
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");
        std::fs::write(
            skill.join(crate::config::LOCAL_FILE),
            marked(&format!(
                "| Setting | Value |\n|---|---|\n| {label} | acme/7 |\n"
            )),
        )
        .expect("the operator's own file");

        // The floor: this crate reads the row at all. A label neither reader
        // recognises would satisfy the assertion below by measuring nothing.
        let told = crate::skill::installed_config_in_keeping_what_parses(&skill, home.path()).0;
        assert_eq!(
            super::Setting::Board.value_of(&told),
            "acme/7",
            "{label} is not read as a board here, so this case poses nothing"
        );

        let live = crate::transport::Context::live(skill.clone(), home.path().to_path_buf(), None);
        assert_eq!(
            live.get("project board"),
            Some("acme/7"),
            "{label}: the gate reports a board the transport will never mirror to"
        );
    }
}

#[test]
fn every_answer_a_setting_offers_says_what_it_means() {
    // The ratchet applied to a screen. Every refusal in this crate names the
    // values it would have accepted, on the grounds that *a value the operator
    // cannot discover is a value they cannot supply* — and the screen offered
    // `standard` and `receipt-driven` with a sentence about the row and none
    // about either word. Discovering the spelling is not discovering the choice.
    //
    // Every offered answer, because a table filled in for fifteen rows and left
    // blank on the sixteenth is worse than none: the operator learns the help is
    // there and then meets the row where it is not.
    let mut silent: Vec<String> = Vec::new();
    let mut explained = 0;
    for setting in SETTINGS {
        for answer in setting.answers().choices {
            // The rows whose real answers are typed offer one word as a
            // placeholder — `unset` is the absence of a path, not a choice
            // between paths — and there is nothing to say about the word.
            let placeholder = matches!(setting, Setting::Worktree | Setting::Models);
            match setting.means(answer) {
                Some(meaning) => {
                    explained += 1;
                    assert!(
                        !placeholder,
                        "{setting:?} is a typed row and explains its placeholder {answer:?}"
                    );
                    // Not the word back. Using it in the sentence is right —
                    // *work integrates on a branch* explains `branch` — so what
                    // is refused is an explanation that **is** the answer.
                    assert!(
                        meaning.len() > 12 && !meaning.eq_ignore_ascii_case(answer),
                        "{setting:?}'s {answer:?} is explained by repeating itself: {meaning:?}"
                    );
                }
                None => {
                    if !placeholder {
                        silent.push(format!(
                            "{setting:?} offers {answer:?} and never says what \
                                             choosing it does"
                        ));
                    }
                }
            }
        }
    }
    // The floor: the walk found answers to explain. An empty vocabulary would
    // satisfy the assertion below without a word being written.
    assert!(
        explained >= 20,
        "only {explained} answer(s) were explained, so this proves little"
    );
    assert!(silent.is_empty(), "{silent:?}");
}

#[test]
fn every_key_the_routing_takes_is_a_key_it_names() {
    // `Model routing` accepts four families of key — a delegated role, a
    // workflow state, a phase of thinking, and the name of a sub-agent an
    // orchestration skill spawns — and the sentence an operator reads named
    // seven examples out of twenty-two. `orchestrate` was one of the fifteen it
    // left out: the key somebody asks for first, working since the day it was
    // written, mentioned nowhere.
    //
    // The ratchet this crate applies to every refusal, applied to the row it
    // matters most on: a value the operator cannot discover is a value they
    // cannot supply. Read out of the four lists rather than trusting the
    // sentence, so a key added to any of them lands here.
    let accepted = Setting::Models.accepted();
    let mut unnamed: Vec<&str> = Vec::new();
    let mut named = 0;
    for key in crate::config::Role::all()
        .into_iter()
        .map(crate::config::Role::as_str)
        .chain(crate::config::STATES.iter().copied())
        .chain(crate::config::SDD_PHASES.iter().copied())
        .chain(crate::config::ORCHESTRATED_ROLES.iter().copied())
    {
        // The bare word, because the backticks in this sentence belong to
        // values somebody can write and these are **keys**. Quoting them made
        // `every_value_a_setting_offers_is_a_value_it_accepts` try to apply
        // `implementer` as a routing and refuse it — a help text and a guard
        // disagreeing about what a backtick means.
        if accepted.contains(key) {
            named += 1;
        } else {
            unnamed.push(key);
        }
    }
    // The floor: the walk found the keys. An empty vocabulary would be named in
    // full by a sentence that said nothing.
    assert!(
        named + unnamed.len() >= 20,
        "only {} key(s) were read out of the four lists",
        named + unnamed.len()
    );
    assert!(
        unnamed.is_empty(),
        "the routing takes these and never names them: {unnamed:?}"
    );

    // And each one really is taken, so the sentence is not naming words the
    // parser would refuse — the other way this help can lie.
    for key in ["orchestrate", "builder", "in-progress", "implementer"] {
        let mut config = Config::default();
        Setting::Models
            .apply(&mut config, &format!("{key}=opus"))
            .unwrap_or_else(|_| panic!("{key} is named as a key and the parser refuses it"));
    }
}

/// A board spec the transport cannot address is refused before it is written.
///
/// `config set`'s one promise is *"validating it before anything is written"*,
/// and this row had a rule of its own: `BoardRef::parse` refused an empty value
/// and one holding `|` — the character that would break the markdown table —
/// and nothing else. Measured on the installed binary:
///
/// ```text
/// estigia config set "Project board" acme/no-numero
/// Project board is now acme/no-numero
/// ```
///
/// Accepted, written, and reported by `config list` as set — while the reader
/// calls it `unparseable board spec` and turns the mirror **off**. The only
/// trace is a `skip_reason` inside an operation's answer, and a mirror that
/// never fires reports exactly what a mirror with nothing to do reports.
///
/// Asked of the reader now, through `board_spec_fault`, so the writer cannot
/// drift from the rule that decides.
#[test]
fn a_board_the_transport_cannot_address_is_refused_before_it_is_written() {
    for spec in ["acme/no-numero", "Roadmap", "acme/", "/7", "acme/1__0"] {
        let refusal = BoardRef::parse(spec).expect_err(&format!(
            "{spec:?} was accepted and the mirror would be off"
        ));
        assert_eq!(refusal.code, "board-ref-malformed", "{spec:?}");
        assert!(
            refusal.to_string().contains("<owner>/<number>"),
            "{spec:?} was refused without saying what a board looks like: {refusal}"
        );
    }

    // The floor, and it is most of the row: every shape the transport does
    // address is still accepted, or this would be a rule that refuses the
    // feature. `none` is the operator declining, and the underscore and sign
    // forms are the ones `board_number` reads on purpose.
    for spec in ["acme/7", "acme/+7", "acme/1_0", "none", "None"] {
        assert!(
            BoardRef::parse(spec).is_ok(),
            "{spec:?} is a board the transport reads and this refused it"
        );
    }

    // And the two readers agree by construction: what one refuses, the other
    // would have disabled itself over.
    let context = crate::transport::Context {
        skill_dir: std::path::PathBuf::from("/skill"),
        repo_dir: std::path::PathBuf::from("/repo"),
        config: Vec::new(),
        repo: None,
    };
    for spec in ["acme/no-numero", "Roadmap", "acme/7", "none"] {
        let board = crate::transport::board::Board::parse(spec, &context, false);
        assert_eq!(
            BoardRef::parse(spec).is_err(),
            board.skip_reason.is_some(),
            "{spec:?}: the writer and the reader disagree about whether it is a board"
        );
    }
}
