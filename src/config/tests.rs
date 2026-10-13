use std::path::PathBuf;

/// An em dash only ends a value when it is a word of its own: `clean` drops a
/// trailing explanation, and the rule that keeps it from eating half a value
/// is that the dash has to have whitespace on **both** sides. Not a made-up
/// value: a macOS text field turns `--` into `—` while somebody types, and
/// `Irreversible commands` is a list of command lines.
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

/// The stricter reader's rule, in both directions: the three ways a hand-typed
/// cell fails to reach the transport were otherwise checked by nothing, and the
/// consequence is `config list` reporting a row the transport never reads.
#[test]
fn a_row_that_does_not_reach_the_transport_is_not_read_as_one_that_does() {
    use crate::config::{Setting, reaches_the_transport};

    // Reaches: the label itself, and a label carrying a person's own suffix.
    for cell in ["Tracker", "  Tracker  ", "Tracker (mine)"] {
        assert!(
            reaches_the_transport(cell, Setting::Tracker),
            "`{cell}` reaches the transport and was read as not reaching it"
        );
    }

    // Does not: the backticks a person writing markdown puts round a name, and
    // a run of whitespace inside it. This crate matches both; the rule the
    // transport applies does not, and that is the difference worth reporting.
    for cell in ["`Tracker`", "Track  er", "track", ""] {
        assert!(
            !reaches_the_transport(cell, Setting::Tracker),
            "`{cell}` does not reach the transport and was read as one that does"
        );
    }

    // And it is a rule about *this* setting, not about any label: a cell that
    // names one setting does not reach the transport as another.
    assert!(
        !reaches_the_transport("Tracker", Setting::Worktree),
        "one setting's label was read as another's"
    );
}

use std::time::Duration;

use super::*;
use crate::config::PublicationLane;

fn table(rows: &[(&str, &str)]) -> String {
    let mut out = String::from("| Setting | Value here | Skill default |\n|---|---|---|\n");
    for (label, value) in rows {
        out.push_str(&format!("| {label} | {value} | ignored |\n"));
    }
    out
}

/// Every variant, listed under an exhaustive `match` so the **compiler** keeps
/// the list honest: a new variant stops the build at the one place that decides
/// whether it reaches the table.
fn every_variant() -> Vec<Setting> {
    let placed = |setting: Setting| -> Setting {
        match setting {
            Setting::Profile
            | Setting::Delivery
            | Setting::Transitions
            | Setting::Worktree
            | Setting::Tracker
            | Setting::RepositoryHost
            | Setting::Models
            | Setting::Integration
            | Setting::Window
            | Setting::GitOperationWindow
            | Setting::Evidence
            | Setting::PublicationLane
            | Setting::ReviewBudget
            | Setting::ChangeSize
            | Setting::CorrectionsPerLineage
            | Setting::Boundaries
            | Setting::Summary
            | Setting::Body
            | Setting::IssueRequirement
            | Setting::Workflow
            | Setting::LoopPlugin
            | Setting::GoalLoopPlugin => setting,
        }
    };
    SETTINGS.iter().copied().map(placed).collect()
}

#[test]
fn no_setting_answers_to_a_label_another_one_owns() {
    // `from_label` takes the first match, so a name shared by two rows resolves
    // by **position in the table** — which is not a thing anybody writing an
    // alias is thinking about. `Task body language` is the old spelling of one
    // row and was nearly the new name of another.
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
        Setting::Profile,
        Setting::Delivery,
        Setting::Transitions,
        Setting::Worktree,
        Setting::Tracker,
        Setting::RepositoryHost,
        Setting::Models,
        Setting::Integration,
        Setting::Window,
        Setting::GitOperationWindow,
        Setting::Evidence,
        Setting::PublicationLane,
        Setting::ReviewBudget,
        Setting::ChangeSize,
        Setting::CorrectionsPerLineage,
        Setting::Boundaries,
        Setting::Summary,
        Setting::Body,
        Setting::IssueRequirement,
        Setting::Workflow,
        Setting::LoopPlugin,
        Setting::GoalLoopPlugin,
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
    use crate::config::{
        AGENT_SETTINGS, EVERYWHERE_SETTINGS, MACHINE_SETTINGS, REPOSITORY_SETTINGS, Scope,
    };

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
            (Scope::Repository, REPOSITORY_SETTINGS.contains(setting)),
        ];
        let listed: Vec<Scope> = lists
            .iter()
            .filter(|(_, holds)| *holds)
            .map(|(scope, _)| *scope)
            .collect();
        assert_eq!(
            listed.len(),
            1,
            "{setting:?} is in {} of the four lists",
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
        AGENT_SETTINGS.len()
            + EVERYWHERE_SETTINGS.len()
            + MACHINE_SETTINGS.len()
            + REPOSITORY_SETTINGS.len(),
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

/// A closed list offers every value its type can hold: a closed row draws its
/// answers with the one in force marked, so a value nobody offered is a row
/// with no mark anywhere on it — the operator reads "nothing is set" where
/// something is.
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
            Setting::Integration => Some(rendered(
                Integration::all(),
                |c, v| c.integration = v,
                setting,
            )),
            Setting::Evidence => Some(rendered(Evidence::all(), |c, v| c.evidence = v, setting)),
            Setting::PublicationLane => Some(rendered(
                PublicationLane::all(),
                |c, v| c.publication_lane = v,
                setting,
            )),
            Setting::IssueRequirement => Some(rendered(
                crate::config::IssueRequirement::all(),
                |c, v| c.issue_requirement = v,
                setting,
            )),
            Setting::Workflow => Some(rendered(
                crate::config::Workflow::all(),
                |c, v| c.workflow = v,
                setting,
            )),
            Setting::LoopPlugin => Some(rendered(
                LoopPlugin::all(),
                |c, v| c.loop_plugin = v,
                setting,
            )),
            Setting::GoalLoopPlugin => Some(rendered(
                LoopPlugin::all(),
                |c, v| c.goal_loop_plugin = v,
                setting,
            )),
            // Open vocabularies: a path, a model name, a language, a duration,
            // a count of lines, a list of commands, a tracker this build may not
            // know. There is nothing to enumerate, and the row has somewhere to
            // type instead.
            Setting::ReviewBudget
            | Setting::ChangeSize
            | Setting::CorrectionsPerLineage
            | Setting::Profile
            | Setting::Delivery
            | Setting::Transitions
            | Setting::Worktree
            | Setting::Tracker
            | Setting::RepositoryHost
            | Setting::Models
            | Setting::Window
            | Setting::GitOperationWindow
            | Setting::Boundaries
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
    assert_eq!(
        config.repository_host,
        RepositoryHost::Github { repo: None }
    );
    assert_eq!(config.worktree, WorktreeRoot::Auto);
    assert_eq!(config.summary_language.as_str(), "English");
    assert_eq!(config.body_language.as_str(), "English");
    assert!(config.delivery.is_autonomous());
    assert!(config.transitions.is_autonomous());
}

/// A fresh installation ships the balanced posture, not the cautious one.
///
/// The defect this pins: the portable table shipped `ask` for all the
/// authorisations, so an installation that wanted Estigia to work unattended
/// had to move rows by hand and know which. `ask` is still one word
/// away — `cautious` — and an explicit row still beats it.
#[test]
fn a_fresh_installation_reads_the_balanced_posture() {
    let config = Config::default();
    assert_eq!(Setting::Profile.value_of(&config), "balanced");
    for setting in [Setting::Delivery, Setting::Transitions] {
        assert_eq!(
            setting.value_of(&config),
            "auto",
            "a fresh installation does not ship the balanced posture for {setting:?}"
        );
    }
    // The posture is the authorisations and nothing else: naming a profile
    // never writes a role into `Model routing`, because naming a role is what
    // installs a definition that carries `Write`, `Edit` and `Bash`.
    assert!(
        config.models.by_role.is_empty(),
        "the balanced posture named a model role"
    );
}

/// The two profiles round-trip, and an explicit authorisation wins.
///
/// Written through the same reader a table goes through, not only through
/// `apply`: `Profile` is applied first in the table's order precisely so a
/// later authorisation row is the one that survives.
#[test]
fn the_profile_round_trips_both_ways_and_an_explicit_row_wins() {
    let mut config = Config::default();
    Setting::Profile
        .apply(&mut config, "cautious")
        .expect("cautious is offered and accepted");
    assert_eq!(Setting::Profile.value_of(&config), "cautious");
    for setting in [Setting::Delivery, Setting::Transitions] {
        assert_eq!(setting.value_of(&config), "ask", "{setting:?}");
    }

    Setting::Profile
        .apply(&mut config, "balanced")
        .expect("balanced is offered and accepted");
    assert_eq!(Setting::Profile.value_of(&config), "balanced");
    for setting in [Setting::Delivery, Setting::Transitions] {
        assert_eq!(setting.value_of(&config), "auto", "{setting:?}");
    }

    // `custom` is accepted — the row reads it whenever the two disagree —
    // and writing it back changes no row.
    let before = config.clone();
    Setting::Profile
        .apply(&mut config, "custom")
        .expect("custom is accepted");
    assert_eq!(config, before, "`custom` moved an authorisation");

    // A profile row and the row it expands into, in the table's own order: the
    // explicit authorisation is read last and is what survives.
    let table = "| Profile | cautious |\n\
                 | Delivery authorisation | auto |\n";
    let read = Config::read(table, None).expect("a table");
    assert_eq!(Setting::Delivery.value_of(&read), "auto");
    assert_eq!(Setting::Transitions.value_of(&read), "ask");
    assert_eq!(Setting::Profile.value_of(&read), "custom");
}

/// The upgrade the issue says is most likely to fail silently, measured.
///
/// An installation that carries its own contract keeps every row it wrote:
/// the old table named both authorisations, so reading it is unchanged.
/// A table that is *missing* a row takes the new value for that row on the
/// next read (and so on the next `setup`, which re-renders the whole block),
/// without a word — which is the honest description of a defaulted row.
#[test]
fn an_existing_table_keeps_its_rows_and_a_missing_row_takes_the_new_default() {
    let old = "| Delivery authorisation | ask |\n\
               | Transition authorisation | ask |\n";
    let kept = Config::read(old, None).expect("an old table");
    assert_eq!(
        kept.profile(),
        Some(Profile::Cautious),
        "an installation carrying the old defaults did not keep them"
    );

    let missing = "| Delivery authorisation | ask |\n";
    let read = Config::read(missing, None).expect("a table missing a row");
    assert_eq!(
        Setting::Transitions.value_of(&read),
        "auto",
        "a row nobody wrote did not take the new default"
    );
    assert_eq!(read.profile(), None, "ask/auto is not a profile");
}

/// Issue #180: only setting Tracker keeps one place to configure — the host
/// inherits the pin until a Repository host row overrides it.
#[test]
fn a_tracker_pin_without_a_host_row_is_the_host_pin_too() {
    let table = "| Tracker | github acme/app |\n";
    let config = Config::read(table, None).expect("a table");
    assert_eq!(
        config.tracker.named_repo().as_deref(),
        Some("acme/app"),
        "tracker pin was not read"
    );
    assert_eq!(
        config.repository_host.named_repo().as_deref(),
        Some("acme/app"),
        "host did not inherit the tracker pin"
    );
    let both = "| Tracker | github acme/app |\n| Repository host | github other/repo |\n";
    let split = Config::read(both, None).expect("both rows");
    assert_eq!(split.tracker.named_repo().as_deref(), Some("acme/app"));
    assert_eq!(
        split.repository_host.named_repo().as_deref(),
        Some("other/repo"),
        "an explicit host row must not be overwritten by the tracker"
    );
}

#[test]
fn a_linear_tracker_names_a_team_and_is_read_only() {
    let config = Config::read("| Tracker | linear ENG |\n", None).expect("a table");
    assert_eq!(
        config.tracker,
        Tracker::Linear {
            team: "ENG".to_owned()
        }
    );
    assert_eq!(config.tracker.as_value(), "linear ENG");
    assert_eq!(config.tracker.transport(), None, "Linear is read-only");
    assert!(config.tracker.is_read_only());
    assert_eq!(config.tracker.team(), Some("ENG"));
    assert_eq!(config.tracker.named_repo(), None);
    // The host stays GitHub: Linear names no repository, and the draft pull
    // request is opened there.
    assert_eq!(
        config.repository_host,
        RepositoryHost::Github { repo: None }
    );
}

#[test]
fn a_linear_tracker_without_a_team_key_is_refused() {
    let refusal = Config::read("| Tracker | linear |\n", None).expect_err("no team key");
    assert_eq!(refusal.code, "config-value-unrecognised");
}

#[test]
fn a_rendered_table_reads_back_identically() {
    let config = Config {
        tracker: Tracker::Github {
            repo: Some(RepoRef::parse("asanabrial/estigia").unwrap()),
        },
        repository_host: RepositoryHost::Github {
            repo: Some(RepoRef::parse("asanabrial/estigia").unwrap()),
        },
        models: ModelRouting::parse("developer=opus, judge=haiku").expect("a routing"),
        integration: Integration::Trunk,
        window: std::time::Duration::from_secs(30),
        git_operation_window: GitOperationWindow::parse("enabled 22:00-06:00 inverted")
            .expect("a schedule"),
        evidence: Evidence::Measuring,
        publication_lane: crate::config::PublicationLane::Off,
        issue_requirement: crate::config::IssueRequirement::Ask,
        workflow: crate::config::Workflow::Autonomous,
        // Not the default, so a round trip that dropped the row would show.
        loop_plugin: LoopPlugin::On,
        goal_loop_plugin: LoopPlugin::On,
        review_budget: crate::config::ReviewBudget::Tokens(60_000),
        change_size: 250,
        corrections_per_lineage: 5,
        boundaries: vec!["npm publish".to_owned()],
        worktree: WorktreeRoot::Path(PathBuf::from(if cfg!(windows) {
            r"C:\work\trees"
        } else {
            "/work/trees"
        })),
        summary_language: Language::parse("Español").unwrap(),
        body_language: Language::parse("Deutsch").unwrap(),
        delivery: Authority::Auto,
        transitions: Authority::Auto,
    };
    let rendered = config.render_rows();
    let reread = Config::read(&rendered, None).expect("a table Estigia wrote must read back");
    assert_eq!(reread, config);
}

#[test]
fn the_local_file_overrides_the_versioned_table() {
    let versioned = table(&[("Integration", "trunk"), ("Tracker", "github")]);
    let local = table(&[("Evidence standard", "measuring")]);
    let config = Config::read(&versioned, Some(&local)).unwrap();
    assert_eq!(config.integration, Integration::Trunk);
    assert_eq!(config.evidence, Evidence::Measuring);
    // A row the local file does not mention keeps the versioned value.
    assert_eq!(config.tracker, Tracker::Github { repo: None });
}

#[test]
fn both_older_names_for_the_summary_language_are_still_read() {
    // Under either older spelling the value governed the summary sentence and
    // nothing else; read as the *new* `Issue body language` an answer would
    // move onto a row that decides something different.
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
        table(&[("Integration", "trunk")])
    );
    let config = Config::read(&local, None).unwrap();
    assert_eq!(config.integration, Integration::Trunk);
}

#[test]
fn an_unrecognised_value_is_refused_and_names_what_is_accepted() {
    let local = table(&[("Integration", "fast-forward")]);
    let refusal = Config::read(&local, None).unwrap_err();
    assert_eq!(refusal.code, "config-value-unrecognised");
    let rendered = refusal.to_string();
    // Backticked, because that marker is what tells a two-word value apart
    // from a description of one — and what lets a test check that every value
    // named here is a value the parser takes.
    assert!(rendered.contains("`branch`"), "{rendered}");
    // Nothing was written, so re-running the same command is safe.
    assert!(refusal.outcome.is_clean());
}

#[test]
fn the_refuter_and_the_validator_route_and_an_unknown_role_key_is_refused_naming_them() {
    let routing = |cell: &str| {
        Config::read(&table(&[("Model routing", cell)]), None).map(|config| config.models)
    };
    let routed = routing("reviewer=opus, refuter=haiku, validator=sonnet").expect("routes");
    assert_eq!(routed.for_target("refuter"), Some("haiku"));
    assert_eq!(routed.for_target("validator"), Some("sonnet"));
    let refusal = routing("reviewer=opus, lens=haiku").unwrap_err();
    assert_eq!(refusal.code, "model-routing-key-dropped");
    let rendered = refusal.to_string();
    for key in crate::config::ModelRouting::targets() {
        assert!(rendered.contains(key), "{key} is not named: {rendered}");
    }
}

#[test]
fn a_relative_worktree_location_is_refused_rather_than_resolved() {
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
    let mut config = Config::default();
    config.set_profile(Profile::Cautious);
    assert!(
        config
            .render_rows()
            .contains("| Delivery authorisation | ask | auto |"),
        "a bare `ask` did not render as the word the row offers"
    );
}

#[test]
fn a_zero_timeout_is_not_a_timeout() {
    let local = table(&[("Transition authorisation", "ask 0m")]);
    assert!(Config::read(&local, None).is_err());
}

#[test]
fn every_tracker_names_a_binding_that_ships() {
    for tracker in [
        Tracker::Github { repo: None },
        Tracker::Linear {
            team: "ENG".to_owned(),
        },
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
    // The mechanics live in `crate::fence`; what belongs here is the one thing
    // this fence declares that a generic one cannot know: the superseded
    // markers.
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
    // The upgrade path end to end: an operator on issue-flow who had
    // `trunk` still has it after the block is renamed.
    let original = [
        "<!-- issue-flow:config:start -->",
        "",
        "| Setting | Value here | Skill default |",
        "|---|---|---|",
        "| Integration | trunk | branch |",
        "",
        "<!-- issue-flow:config:end -->",
        "",
    ]
    .join(
        "
",
    );

    let config = Config::read(&original, None).expect("the old table reads");
    assert_eq!(config.integration, Integration::Trunk);

    let upgraded = CONFIG_FENCE.upsert(&original, &config.render_rows());
    assert_eq!(
        Config::read(&upgraded, None)
            .expect("the new table reads")
            .integration,
        Integration::Trunk
    );
}

/// The literals one setting's message tells an operator to type: backticks
/// mark them, so `by task` — two words a person types verbatim — is
/// distinguishable from "an absolute directory", which is a description. A
/// literal holding a `<placeholder>` is a shape, not a value, and is left out.
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
    // `accepted()` is the only place an operator learns what to write; a value
    // named there that the parser rejects is a dead end in the one message
    // whose whole job is not to be one.
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
    assert_eq!(offered(Setting::ChangeSize), vec!["800"]);
    // Including the two-word one, which is why the marker exists.
    assert!(offered(Setting::Delivery).contains(&"ask 30m".to_owned()));
    // A shape is not a value: `github <owner>/<name>` is left out and the bare
    // `github` beside it is kept.
    assert_eq!(offered(Setting::Tracker), vec!["github"]);
    // And a free-text setting still has to offer an example that works.
    assert_eq!(offered(Setting::Summary), vec!["English"]);
    assert_eq!(offered(Setting::Body), vec!["English"]);
}

#[test]
fn every_timeout_unit_the_grammar_accepts_is_one_a_test_holds() {
    // Mutation testing: deleting the `s` arm and deleting the `h` arm both
    // left the suite green.
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

    // A space before the unit is accepted, because `parse_duration` trims it.
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
    // Mutation testing: every boundary in the row reader could be flipped
    // without a test noticing. These are what stand between a heading and a
    // setting.
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
| Integration | trunk |
";
    assert_eq!(
        read(aligned).expect("an aligned table parses").integration,
        Integration::Trunk
    );

    // And a row whose cells are empty is not a separator — it is a blank row,
    // which names no setting and must not be mistaken for one.
    let blank = "| Setting | Value here |
|---|---|
|  |  |
| Integration | trunk |
";
    assert_eq!(
        read(blank).expect("a blank row is skipped").integration,
        Integration::Trunk
    );
}

#[test]
fn the_selected_review_protocol_document_ships() {
    // The review sentence in the installed contract names this document, so it
    // must be a file Estigia actually installs.
    let document = "protocols/rdd.md";
    assert!(
        crate::skill::FILES.iter().any(|file| file.path == document),
        "{document} is named by the review sentence and is not installed"
    );
    assert!(
        crate::skill::SELECTED_BY_SETTING
            .iter()
            .any(|prefix| document.starts_with(prefix)),
        "{document} is chosen unconditionally and sits outside SELECTED_BY_SETTING, so the \
         seam guards will apply the wrong reachability rule"
    );
}

#[test]
fn the_remaining_axes_are_independent() {
    // The modelling error this replaced: one enum, so choosing one excluded the
    // others while the documents said in prose that they compose.
    let local = table(&[
        ("Integration", "trunk"),
        ("Model routing", "developer=opus"),
        ("Tracker", "github asanabrial/estigia"),
    ]);
    let config = Config::read(&local, None).expect("the axes read together");
    assert_eq!(config.integration, Integration::Trunk);
    assert!(
        config.models.installs("developer") && !config.models.installs("analyst"),
        "naming `developer` in the roles section did not install it alone"
    );
    assert_eq!(
        config.tracker,
        Tracker::Github {
            repo: Some(RepoRef::parse("asanabrial/estigia").unwrap())
        }
    );

    // And each survives a round trip on its own.
    for mode in Integration::all() {
        let config = Config {
            integration: mode,
            ..Config::default()
        };
        let reread = Config::read(&config.render_rows(), None).expect("a table Estigia wrote");
        assert_eq!(reread.integration, mode);
    }
}

#[test]
fn an_operator_may_add_a_boundary_and_may_not_take_one_away() {
    // Estigia's built-in list cannot know a repository ships with `npm
    // publish`; until this row existed that ran as a routine write.
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
    // Found by `tests/differential.rs` on its first run: the transport read
    // `npm publish` out of a decorated cell and Estigia refused the whole
    // contract — one file, two answers, no error.
    let decorated = |value: &str| {
        let document = format!("| Irreversible commands | {value} |\n");
        settings::rows(&document)
            .into_iter()
            .next()
            .expect("one row")
            .1
    };

    // The three shapes the shipped table itself teaches.
    assert_eq!(decorated("`npm publish`"), "npm publish");
    assert_eq!(decorated("**npm publish**"), "npm publish");
    assert_eq!(
        decorated("`npm publish` — because releases are one-way"),
        "npm publish"
    );
    assert_eq!(decorated("npm publish (the usual choice)"), "npm publish");
    assert_eq!(decorated("  npm publish  "), "npm publish");

    // And what must NOT be cut: a value is not an explanation because it holds
    // a dash or a bracket. Over-trimming here would silently change a setting.
    assert_eq!(decorated("git push --force"), "git push --force");
    assert_eq!(decorated("git—push"), "git—push");
    assert_eq!(decorated("two blind"), "two blind");
    assert_eq!(decorated("npm publish"), "npm publish");
}

#[test]
fn a_retired_planning_value_reads_as_direct_and_is_reported_once() {
    // An installed contract naming the SDD protocol must keep loading: the row
    // is retired, not unreadable, and `setup` rewrites it on its next run. The
    // report is what keeps the rewrite from being silent.
    let planning = |cell: &str| {
        let local = table(&[("Planning", cell)]);
        Config::read_folding_retired_workers(&local, None)
    };

    for cell in [
        "sdd",
        "sdd lite",
        "sdd openspec",
        "sdd lite openspec",
        "sdd openspec lite",
        "SDD  Lite   OpenSpec",
        "sdd short",
        "spec-driven",
        "auto",
        "per-issue",
    ] {
        assert!(settings::planning_is_retired(cell), "{cell:?}");
        let (_, report) = planning(cell).expect("a retired protocol still loads");
        let report = report.expect("the retirement is reported");
        assert_eq!(report.code, "planning-protocol-retired", "{cell:?}");
        assert!(report.message.contains("direct"), "{cell:?}: {report}");
    }

    // The strict read loads it too, silently: an installed table must not stop
    // reading because a different protocol was retired.
    let local = table(&[("Planning", "sdd lite")]);
    assert!(Config::read(&local, None).is_ok());

    // A value that names no protocol is not a retirement: the row is gone, so
    // it is the operator's own note and is neither folded nor reported.
    for cell in ["direct", "waterfall", "sdd deluxe", "direct lite"] {
        assert!(!settings::planning_is_retired(cell), "{cell:?}");
        let (_, report) = planning(cell).expect("an unknown row is the operator's note");
        assert!(report.is_none(), "{cell:?} was reported as a retirement");
    }

    // Exactly one warning, however many rows or documents carry the value.
    let twice = table(&[("Planning", "sdd lite"), ("Planning", "sdd")]);
    let (_, report) = Config::read_folding_retired_workers(&twice, None).expect("loads");
    let report = report.expect("the retirement is reported");
    assert_eq!(report.code, "planning-protocol-retired");
    let (_, report) = Config::read_folding_retired_workers(
        &table(&[("Planning", "sdd")]),
        Some(&table(&[("Planning", "sdd lite")])),
    )
    .expect("loads");
    assert!(report.is_some(), "the second document lost the report");
}

#[test]
fn an_installed_retired_setting_row_is_reported_and_changes_nothing() {
    for (label, code) in [
        ("Delivery route", "delivery-route-retired"),
        ("Review delegation", "review-delegation-retired"),
        ("Merge strategy", "merge-strategy-retired"),
        ("Review protocol", "review-protocol-retired"),
    ] {
        let row = table(&[(label, "whatever")]);
        let (config, report) =
            Config::read_folding_retired_workers(&row, None).expect("a retired row still loads");
        assert_eq!(config, Config::default(), "{label} changed the answer");
        assert_eq!(
            report.expect("the retirement is reported").code,
            code,
            "{label}"
        );

        // The strict read loads it too, silently: an installed table must not
        // stop reading because a row was retired.
        assert!(
            Config::read(&row, None).is_ok(),
            "{label}: the strict read refused a retired row"
        );

        // The local override reports it the same way.
        let (_, report) = Config::read_folding_retired_workers(&table(&[]), Some(&row))
            .expect("a local override still loads");
        assert_eq!(
            report.expect("the local row is reported").code,
            code,
            "{label}"
        );

        // And so does every scoped read, whichever file the row sits in.
        for scope in [Scope::Agent, Scope::Everywhere, Scope::Repository] {
            let (_, _, report) =
                Config::read_scope_over_folding_retired_workers(&Config::default(), &row, scope)
                    .expect("a scoped override still loads");
            assert_eq!(
                report.expect("the scoped row is reported").code,
                code,
                "{label} in {scope:?}"
            );
        }
    }
}

#[test]
fn the_retired_phase_routing_keys_are_dropped_and_named() {
    // `Model routing` no longer takes the SDD phase keys. A table naming one
    // keeps its other entries: the tolerant read drops the key alone, and the
    // strict read names it rather than losing the whole cell.
    let (kept, dropped) = ModelRouting::parse_keeping_what_parses("explore=opus, developer=sonnet");
    assert_eq!(kept.for_target("developer"), Some("sonnet"));
    assert!(
        dropped.iter().any(|entry| entry.key == "explore"),
        "the retired phase key was kept"
    );

    let local = table(&[("Model routing", "explore=opus, developer=sonnet")]);
    let refusal = Config::read(&local, None).expect_err("the strict read names the dropped key");
    assert_eq!(refusal.code, "model-routing-key-dropped");
}

#[test]
fn direct_work_delegation_thresholds_match_the_writing_refusal() {
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
        rows.get("One batch of reads or lookups"),
        Some(&(
            "At most 3 calls and about 10k tokens stays inline",
            "More than about 5 sequential lookups delegates one explorer with a ~2k-token handoff"
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

    // The writing threshold has two copies — this prose row and
    // `claim::TOPOLOGY_WRITE_THRESHOLD`, which the refusal's own sentence
    // attributes to `SKILL.md` — and nothing before this crossed them. A prose
    // row edited to `3+` reddens the byte-exact assertion above and never
    // reaches here; what this catches is the const moving while the prose does
    // not, leaving the refusal's citation false.
    let writing_delegate = rows.get("Writing").expect("the writing row exists").1;
    let threshold: usize = writing_delegate
        .split(|c: char| !c.is_ascii_digit())
        .find(|segment| !segment.is_empty())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| {
            panic!("the writing row's delegate cell holds no digit run: {writing_delegate}")
        });
    assert_eq!(
        threshold,
        crate::transport::claim::TOPOLOGY_WRITE_THRESHOLD,
        "SKILL.md's writing threshold and claim::TOPOLOGY_WRITE_THRESHOLD disagree"
    );

    let normalize = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let expected = concat!(
        "Use the smallest useful topology and keep one writer. Delegate only when the expected work crosses\n",
        "one of these boundaries:\n",
        "| Work | Stay inline | Delegate |\n",
        "|---|---|---|\n",
        "| One batch of reads or lookups | At most 3 calls and about 10k tokens stays inline | More than about 5 sequential lookups delegates one explorer with a ~2k-token handoff |\n",
        "| Reading for a write or broad research | - | Delegate reading that prepares a write, broad research, or context compression |\n",
        "| Writing | One already-understood mechanical file may stay inline | 2+ non-trivial files delegate one writer |\n",
        "The topology actually used is recorded at publication, not merely required here: `publish_review`/`republish_review` take `--topology` and refuse a bare `inline` declared past this table's writing row with no stated reason. See the binding contract for the exact mechanics and what it cannot prove.\n",
        "The writing row's threshold itself is **stated, not adjudicated**: no gate can judge \"non-trivial\", so nothing refuses a write that crosses it. What the gate does record, independent of the declaration above, is narrower: when a call's `agent_type` shows this run's own context wrote after a delegated one had already run this session, that target is named at `SessionStart` for the rest of the session and is gone at `SessionEnd` — a floor, since a write before that first delegated call is recorded as neither this run's own nor a delegate's.\n",
        "Tests, builds, installs, and native review actions may each use a fresh per-action worker without changing the route. Child workers do not gain orchestration authority.\n",
        "Two definitions ship for exactly this: `developer`, which writes and runs the suite inside the checkout its launch names, and `analyst`, which is repository-read-only. Handing a bounded piece of work to a fresh context happens under every run. Each is installed only where `Model routing` names its role, so no definition is there to launch until the routing names it, and the brief is composed in the prompt until then; naming the role installs it, and the model beside it says only what it runs on. The row's other value, `by task`, names no role and installs nothing: the shape of the work decides the model and the effort, and the launcher records what it chose. A delegated context is measured against **this** run's claim, so what the definition tells it not to do is a rule the launch keeps rather than one the gate enforces on it separately.\n",
        "Crossing a threshold selects delegated work only. Size, file count, and risk do not select a protocol."
    );
    assert_eq!(
        normalize(section),
        normalize(expected),
        "the direct-work owner section changed without updating its semantic binding"
    );
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

    let some = routing("developer=opus, judge=haiku").expect("a routing");
    assert_eq!(some.for_target(Role::Developer.as_str()), Some("opus"));
    assert_eq!(some.for_target(Role::Judge.as_str()), Some("haiku"));
    // A role nobody named runs on whatever the agent picks, which is what
    // "unset" has to mean per role and not only for the cell as a whole.
    assert_eq!(some.for_target(Role::Reviewer.as_str()), None);

    // Rendered in role order, not insertion order: a table that reorders
    // itself between two writes shows a diff where nothing changed.
    assert_eq!(some.as_value(), "developer=opus, judge=haiku");
    let reversed = routing("judge=haiku,developer=opus").expect("a routing");
    assert_eq!(reversed.as_value(), some.as_value());
    assert_eq!(reversed, some);

    // Every shape that is not a routing is refused rather than partly read.
    for bad in [
        "opus",                   // no role
        "architect=opus",         // a role nothing spawns
        "developer=",             // named without a model
        "developer=opus, =haiku", // a model without a role
        "judge=a, judge=b",       // said twice, and only one can happen
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
        .chain(["orchestrate", "apply"])
    {
        assert!(
            targets.contains(&target),
            "{target:?} is accepted but not offered"
        );
    }

    let mut routing = ModelRouting::parse("reviewer=custom, developer=opus").expect("a routing");
    assert!(routing.assign("orchestrate", "gpt-5.6"));
    assert_eq!(routing.for_target("reviewer"), Some("custom"));
    assert_eq!(routing.for_target("orchestrate"), Some("gpt-5.6"));
    assert_eq!(
        routing.as_value(),
        "developer=opus, reviewer=custom, orchestrate=gpt-5.6"
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
fn visible_model_targets_are_the_universal_routes_and_the_roles_in_ui_order() {
    let names = || {
        ModelRouting::visible_targets()
            .into_iter()
            .map(|target| target.name)
            .collect::<Vec<_>>()
    };
    let delegated = [
        "developer",
        "analyst",
        "reviewer",
        "refuter",
        "validator",
        "judge",
    ];

    assert_eq!(
        names(),
        ["orchestrate", "apply"]
            .into_iter()
            .chain(delegated)
            .collect::<Vec<_>>()
    );
}

#[test]
fn visible_model_target_descriptors_are_unique_and_accepted_by_persistence() {
    let targets = ModelRouting::visible_targets();
    let mut names = targets.iter().map(|target| target.name).collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), targets.len(), "duplicate visible target");

    for target in targets {
        let mut routing = ModelRouting::default();
        assert!(
            routing.assign(target.name, "custom/model"),
            "visible target {:?} is not persisted",
            target.name
        );
    }
}

#[test]
fn one_model_id_cannot_cross_the_persisted_entry_delimiters() {
    for model in [
        "provider,model",
        "provider;model",
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
fn a_model_is_named_per_role_and_round_trips_the_table() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    let mixed = routing("judge=haiku, analyst=sonnet, orchestrate=opus").expect("a routing");
    assert_eq!(mixed.for_target("judge"), Some("haiku"));
    assert_eq!(mixed.for_target("analyst"), Some("sonnet"));
    assert_eq!(mixed.for_target("orchestrate"), Some("opus"));

    // Rendered in the canonical order, so two writes of the same routing
    // produce the same cell and a diff means something changed.
    assert_eq!(
        routing("analyst=a, judge=b, orchestrate=c")
            .expect("a routing")
            .as_value(),
        "analyst=a, judge=b, orchestrate=c"
    );

    // And it survives the table.
    let config = Config {
        models: mixed.clone(),
        ..Config::default()
    };
    let reread = Config::read(&config.render_rows(), None).expect("Estigia wrote this");
    assert_eq!(reread.models, mixed);
}

/// A workflow state is not a routing key, because it never named a context:
/// where the issue sits says *what is happening to the work*, not *what is
/// doing it* — there is nothing to render into, which is why this family is
/// refused rather than wired up.
#[test]
fn a_workflow_state_is_no_longer_a_routing_key() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    for state in crate::config::STATES {
        assert!(
            routing(&format!("{state}=opus")).is_err(),
            "`{state}` is a workflow state and the routing still takes it"
        );
        assert!(
            !ModelRouting::targets().contains(state),
            "`{state}` is still offered as a routing target"
        );
        // As a word: `reviewer` is a role and contains `review`, so a
        // substring test here would fail on a sentence that is telling the
        // truth.
        assert!(
            !Setting::Models
                .accepted()
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '-')
                .any(|word| word == *state),
            "`{state}` is refused by the parser and still named by the help"
        );
    }

    // Refused whole rather than partly read, like every other unknown key.
    assert!(routing("judge=haiku, review=opus").is_err());

    // The keys that survive are the ones that reach something.
    for key in ["developer", "orchestrate", "analyst"] {
        assert!(routing(&format!("{key}=opus")).is_ok(), "{key} was refused");
    }
}

/// Model and effort are two separate decisions, so they are two separate lists
/// in the one cell: models first, then, after a `;`, the efforts. The slash
/// that used to make one value carry both is retired.
#[test]
fn an_effort_is_named_in_its_own_list_beside_the_models() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    let sized =
        routing("apply=sonnet, orchestrate=opus; apply=low, orchestrate=high").expect("a routing");
    assert_eq!(sized.for_target("apply"), Some("sonnet"));
    assert_eq!(sized.effort_for("apply"), Some(Effort::Low));
    assert_eq!(sized.for_target("orchestrate"), Some("opus"));
    assert_eq!(sized.effort_for("orchestrate"), Some(Effort::High));

    // Named without an effort stays without one: writing the host's default
    // down would freeze it into every installation that never asked for it.
    let bare = routing("apply=sonnet").expect("a routing");
    assert_eq!(bare.effort_for("apply"), None);
    assert_eq!(bare.as_value(), "apply=sonnet");
    // Roles render first, then the universal phases in their own fixed order.
    assert_eq!(
        sized.as_value(),
        "apply=sonnet, orchestrate=opus; apply=low, orchestrate=high"
    );

    // A provider-qualified ID is one model, not a model at an effort — the
    // slash no longer means anything to this parser.
    let qualified = routing("apply=anthropic/claude-opus-4").expect("a routing");
    assert_eq!(
        qualified.for_target("apply"),
        Some("anthropic/claude-opus-4")
    );
    assert_eq!(qualified.effort_for("apply"), None);
    // And the same ID with the effort in its own list keeps both.
    let both = routing("apply=anthropic/claude-opus-4; apply=max").expect("a routing");
    assert_eq!(both.for_target("apply"), Some("anthropic/claude-opus-4"));
    assert_eq!(both.effort_for("apply"), Some(Effort::Max));

    // Every word the host takes, and nothing else.
    for effort in Effort::all() {
        let named =
            routing(&format!("apply=sonnet; apply={}", effort.as_str())).expect("a routing");
        assert_eq!(named.effort_for("apply"), Some(effort));
    }
    // A word that is not an effort is refused, not absorbed into the model.
    assert!(routing("apply=sonnet; apply=enormous").is_err());

    // An effort with no model of its own is a route: the target runs on the
    // inherited model at the named effort, which is how a role is installed
    // before a model is chosen for it.
    let inherited = routing("; apply=high").expect("a routing");
    assert_eq!(inherited.for_target("apply"), None);
    assert_eq!(inherited.effort_for("apply"), Some(Effort::High));
    assert!(inherited.installs("apply"));
    assert_eq!(inherited.as_value(), "; apply=high");

    // A second effort for one target is a contradiction, dropped like a
    // repeated model, and the strict read refuses the whole cell.
    assert!(routing("apply=sonnet; apply=low, apply=high").is_err());
    let (kept, dropped) = crate::config::ModelRouting::parse_keeping_what_parses(
        "apply=sonnet; apply=low, apply=high",
    );
    assert_eq!(kept.effort_for("apply"), Some(Effort::Low));
    assert_eq!(dropped.len(), 1);

    // The retired `model/effort` spelling is refused, not read as an opaque
    // model id, so a stored route is reported rather than silently rewritten.
    assert!(routing("apply=sonnet/high").is_err());
    let (kept, dropped) =
        crate::config::ModelRouting::parse_keeping_what_parses("apply=sonnet/high, developer=opus");
    assert_eq!(kept.for_target("apply"), None);
    assert_eq!(kept.for_target("developer"), Some("opus"));
    assert_eq!(dropped.len(), 1);

    // And both halves survive the table they are written into.
    let reread = Config::read(
        &Config {
            models: sized.clone(),
            ..Config::default()
        }
        .render_rows(),
        None,
    )
    .expect("Estigia wrote this");
    assert_eq!(reread.models, sized);
}

/// The row's third value, crossed against what the table renders.
///
/// The sentence, the parser and the renderer have to be one thing, and a value
/// the screen offers has to be a value the table writes back byte for byte.
#[test]
fn every_value_the_model_routing_row_takes_is_crossed_against_what_it_renders() {
    let shapes = [
        ("unset", "unset"),
        (
            "developer=opus, apply=sonnet; apply=low",
            "developer=opus, apply=sonnet; apply=low",
        ),
        (crate::config::BY_TASK, "by task"),
    ];
    let mut crossed = 0;
    for (written, rendered) in shapes {
        let mut config = Config::default();
        Setting::Models
            .apply(&mut config, written)
            .unwrap_or_else(|refusal| panic!("`{written}` is a value the row takes: {refusal}"));
        assert_eq!(
            Setting::Models.value_of(&config),
            rendered,
            "`{written}` renders as something else"
        );
        let table = config.render_rows();
        assert!(
            table.contains(&format!("| Model routing | {rendered} |")),
            "`{written}` did not reach the rendered table:\n{table}"
        );
        let back = Config::read(&table, None).expect("Estigia wrote this table");
        assert_eq!(
            Setting::Models.value_of(&back),
            rendered,
            "`{written}` did not survive being written and read back"
        );
        crossed += 1;
    }
    assert_eq!(
        crossed, 3,
        "the walk did not cross every value the row takes"
    );

    // The third value is not a word the rest of the table already answers to:
    // `auto` means permission in the three authorisation rows, and a second
    // meaning for it here is one an operator would read wrong.
    assert_eq!(crate::config::BY_TASK, "by task");
    for setting in SETTINGS {
        if matches!(setting, Setting::Models) {
            continue;
        }
        for answer in setting.answers().choices {
            assert_ne!(
                *answer,
                crate::config::BY_TASK,
                "{setting:?} already answers to `{answer}`"
            );
        }
    }
}

/// The third value is the whole cell, and never a piece of one.
#[test]
fn a_by_task_cell_never_merges_with_the_pairs() {
    let routing = |cell: &str| crate::config::ModelRouting::parse(cell);
    let by_task = routing(crate::config::BY_TASK).expect("the third value parses");
    assert!(by_task.by_task);
    assert!(by_task.by_role.is_empty());
    assert_eq!(by_task.as_value(), crate::config::BY_TASK);
    assert!(routing("BY TASK").expect("case-insensitive").by_task);

    // Mixing it with pairs is a dropped entry, not a silent merge.
    for mixed in [
        "by task, developer=opus",
        "developer=opus, by task",
        "by task; developer=low",
    ] {
        assert!(routing(mixed).is_none(), "`{mixed}` was read as a routing");
    }
    let (kept, dropped) =
        crate::config::ModelRouting::parse_keeping_what_parses("by task, developer=opus");
    assert!(!kept.by_task, "a mixed cell set the third value");
    assert_eq!(dropped.len(), 1, "the mixed piece was not named");
}

#[test]
fn the_states_a_routing_accepts_are_the_ones_the_contract_declares() {
    // Written in two places — here and the contract's own sentence — so they
    // are crossed rather than trusted.
    let contract = include_str!("../../skill/SKILL.md");
    for state in crate::config::STATES {
        assert!(
            contract.contains(&format!("`{state}`")),
            "`{state}` is a workflow state and the contract never names it"
        );
    }
    // And the other direction, on the sentence that lists them: a state the
    // contract declares and this does not is one no run can be moved to.
    // Extracted rather than typed here a second time, so a state added to the
    // sentence and not to `STATES` is what reddens this, not a copy of the
    // sentence going stale beside it.
    let sentence = contract
        .lines()
        .find(|line| line.contains("Keep exactly one of"))
        .expect("the contract states its workflow states in one sentence");
    let mut declared = crate::markdown::markdown_inline_code_spans(sentence);
    declared.sort();
    declared.dedup();
    assert!(
        declared.len() >= crate::config::STATES.len(),
        "the workflow-states sentence names fewer backticked states than `STATES` holds: {declared:?}"
    );
    for declared in declared {
        assert!(
            crate::config::STATES.contains(&declared.as_str()),
            "the contract declares `{declared}` and this crate does not hold it"
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

    // A count of arrows is not coverage. Rewriting `merged and verified -> \`done\``
    // as `-> \`review\`` left six destinations and the library suite green, with
    // `done` unreachable from any decision gate.
    let missing: Vec<_> = crate::config::STATES
        .iter()
        .copied()
        .filter(|state| !destinations.iter().any(|destination| destination == state))
        .collect();
    assert!(
        missing.is_empty(),
        "a named state is not a disposition destination: {missing:?}",
    );
    assert!(
        gates.contains("ordinary delivery permission -> `review`"),
        "the contract does not keep ordinary delivery permission in review"
    );
    assert!(
        gates.contains(
            "a human adjudication that is not a routine wait on the ordinary review, CI, identity and delivery-permission gates -> `blocked`"
        ),
        "the contract does not name where a non-routine human adjudication waits go"
    );
    assert!(
        contract.contains("built work cleared to continue delivery returns to `review`"),
        "the contract does not restore discharged built work to delivery"
    );
    assert!(
        contract.contains("work requiring implementation returns to `ready`"),
        "the contract does not restore discharged implementation work to ready"
    );
    assert!(
        contract.contains(
            "re-derives the tier and re-acquires what it licenses against the exact latest receipt"
        ),
        "the contract does not say a discharged decision still needs a panel, so a run will invent a verdict"
    );
    assert!(
        contract.contains("a person's decision is not a verdict"),
        "the contract does not forbid recording the person as the missing verdict"
    );
}

/// A companion document points at the Decision Gates table rather than
/// carrying its own copy of it. **Load-bearing strings:** the literals
/// asserted here in `docs/honesty.md` and `repository-delivery.md` are
/// deliberate — rewording either reddens this test, because a pointer nobody
/// is required to keep is a pointer that quietly leaves.
#[test]
fn the_decision_gate_is_not_restated_as_a_second_table() {
    let delivery = include_str!("../../skill/references/repository-delivery.md");
    assert!(
        !delivery.contains("| Destination |"),
        "repository-delivery restates the decision gate as its own table"
    );
    assert!(
        delivery.contains("Decision Gates"),
        "repository-delivery does not send a reader to the owner"
    );

    let honesty = crate::test_env::honesty_contract();
    assert!(
        honesty.contains("Decision Gates table in `skill/SKILL.md`"),
        "the honesty contract does not name the owner of the routing"
    );

    for (path, text) in [
        (
            "skill/policies/blind-judges.md",
            include_str!("../../skill/policies/blind-judges.md"),
        ),
        (
            "skill/policies/judgment-day.md",
            include_str!("../../skill/policies/judgment-day.md"),
        ),
        (
            "skill/references/safety-incidents.md",
            include_str!("../../skill/references/safety-incidents.md"),
        ),
        ("docs/honesty.md", honesty.as_str()),
        ("skill/references/repository-delivery.md", delivery),
    ] {
        assert!(
            !text.contains("-> `"),
            "{path} restates a disposition arrow the Decision Gates table owns"
        );
    }
}

#[test]
fn a_model_can_be_named_per_target_and_the_most_specific_wins() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };

    // The shape an operator actually asks for: applying writes with one model,
    // orchestration with a third.
    let sized = routing("apply=sonnet-5, orchestrate=fable-5").expect("a routing");
    assert_eq!(sized.for_target("apply"), Some("sonnet-5"));
    assert_eq!(sized.for_target("orchestrate"), Some("fable-5"));

    // Models are opaque strings on purpose: another agent's list is not this
    // one's, and a hard-coded set would refuse the model somebody has.
    let elsewhere = routing("orchestrate=gpt-5.6, apply=kimi-k3").expect("a routing");
    assert_eq!(elsewhere.for_target("orchestrate"), Some("gpt-5.6"));
    assert_eq!(elsewhere.for_target("apply"), Some("kimi-k3"));

    // A universal phase and a role are two keys and never one fallback for the
    // other.
    let all = routing("apply=opus, developer=sonnet").expect("a routing");
    assert_eq!(all.for_target("apply"), Some("opus"));
    assert_eq!(all.for_target("developer"), Some("sonnet"));
    assert_eq!(all.for_target("orchestrate"), None);

    // Every universal phase is a key, and a word that is none of them is not.
    for phase in ["apply", "orchestrate"] {
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

    // Longer is refused, not clamped: a setting that can loosen a guard rail
    // turns it into a preference.
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

/// A `Delegated workers` row this build retired is folded into the routing and
/// named, never dropped in silence.
#[test]
fn a_retired_delegated_workers_row_folds_into_the_routing_and_says_so() {
    let local = |cell: &str| table(&[("Delegated workers", cell)]);

    // `none` and its synonyms mean "no definition", and fold to nothing.
    for spelling in ["none", "None", "off", "unset", "  "] {
        assert!(
            !Config::read_keeping_what_parses(&local(spelling), None)
                .models
                .installs("developer"),
            "{spelling:?} installed a role"
        );
    }

    // A name folds in as `inherit` — the row said a definition exists, and the
    // model it runs on was never this row's answer.
    for spelling in [
        "developer analyst",
        "analyst developer",
        "analyst  developer",
        "developer analyst developer",
    ] {
        let folded = Config::read_keeping_what_parses(&local(spelling), None);
        assert!(folded.models.installs("developer"), "{spelling:?}");
        assert_eq!(
            folded.models.for_target("developer"),
            Some("inherit"),
            "{spelling:?}"
        );
        assert!(folded.models.installs("analyst"), "{spelling:?}");
    }

    // The strict read refuses the row rather than folding it silently; the
    // folded routing and its report are what the tolerant read hands a writer.
    let strict = Config::read(&local("developer"), None).expect_err("the retired row was accepted");
    assert_eq!(strict.code, "delegated-workers-row-retired");
    assert!(strict.message.contains("Delegated workers"), "{strict}");
    assert!(strict.message.contains("developer"), "{strict}");

    // A name the row never took is reported, not guessed at.
    let bad = Config::read(&local("implementer"), None).expect_err("an unknown name was accepted");
    assert_eq!(bad.code, "delegated-workers-row-retired");

    // A role the routing already names is left alone, not overwritten.
    let both = table(&[
        ("Delegated workers", "developer"),
        ("Model routing", "developer=opus"),
    ]);
    assert_eq!(
        Config::read_keeping_what_parses(&both, None)
            .models
            .for_target("developer"),
        Some("opus")
    );

    // Order does not matter: a routing row written below the retired one leaves
    // the fold in place rather than wiping a role it never named.
    let reversed = table(&[
        ("Delegated workers", "developer"),
        ("Model routing", "analyst=haiku"),
    ]);
    let folded = Config::read_keeping_what_parses(&reversed, None);
    assert_eq!(folded.models.for_target("developer"), Some("inherit"));
    assert_eq!(folded.models.for_target("analyst"), Some("haiku"));
}

/// The read that folds a retired `Delegated workers` row rather than refusing it
/// keeps every row it can and still refuses one a person mistyped.
///
/// The two are separate channels, because a caller deciding whether a contract
/// may be rewritten over must not read "this build defines the row's meaning"
/// as "nobody can read what this row said".
#[test]
fn the_folding_read_folds_the_retired_row_and_still_refuses_a_bad_one() {
    let (folded, report) = Config::read_folding_retired_workers(
        &table(&[("Delegated workers", "developer analyst")]),
        None,
    )
    .expect("the retired row folds rather than refusing");
    assert!(folded.models.installs("developer") && folded.models.installs("analyst"));
    let report = report.expect("the fold is reported");
    assert_eq!(report.code, "delegated-workers-row-retired");
    assert!(report.message.contains("Delegated workers"), "{report}");

    let bad = table(&[
        ("Delegated workers", "developer"),
        ("Irreversible commands", ", ,"),
    ]);
    let refusal = Config::read_folding_retired_workers(&bad, None)
        .expect_err("a mistyped row was accepted beside a retired one");
    assert_ne!(
        refusal.code, "delegated-workers-row-retired",
        "a bad row was reported as the retired one, so the fold laundered it"
    );
}

/// A routing key this build no longer accepts is dropped on its own, with a
/// report naming it and its replacement where a rename is what removed it.
#[test]
fn a_retired_routing_key_is_dropped_with_a_report_and_the_rest_kept() {
    let (kept, dropped) =
        ModelRouting::parse_keeping_what_parses("implementer=opus, reviewer=sonnet, auditor=haiku");
    assert_eq!(kept.for_target("reviewer"), Some("sonnet"));
    assert_eq!(kept.for_target("implementer"), None);
    assert_eq!(kept.for_target("auditor"), None);
    assert_eq!(kept.as_value(), "reviewer=sonnet");
    assert_eq!(
        dropped,
        vec![
            crate::config::DroppedRoute {
                key: "implementer".to_owned(),
                replacement: Some("developer"),
            },
            crate::config::DroppedRoute {
                key: "auditor".to_owned(),
                replacement: None,
            },
        ]
    );

    // The strict read refuses the cell whole; only the tolerant read keeps it.
    assert!(ModelRouting::parse("implementer=opus, reviewer=sonnet").is_none());
}

/// The five names Estigia never spawns are no longer accepted; `analyst` is,
/// because the routing is what installs its definition.
#[test]
fn an_orchestrators_own_vocabulary_is_no_longer_accepted() {
    let routing = |cell: &str| {
        let local = table(&[("Model routing", cell)]);
        Config::read(&local, None).map(|config| config.models)
    };
    for role in ["strategist", "builder", "refactorer", "auditor"] {
        assert!(
            routing(&format!("{role}=opus")).is_err(),
            "{role} was accepted"
        );
    }

    // `analyst` still routes: its key lives in Estigia's own vocabulary now,
    // which is what the ordering constraint was for.
    let analyst = routing("analyst=haiku").expect("`analyst` routes");
    assert_eq!(analyst.for_target("analyst"), Some("haiku"));

    // Estigia's own roles still route.
    let both = routing("reviewer=opus").expect("a routing");
    assert_eq!(both.for_target(Role::Reviewer.as_str()), Some("opus"));

    // And a word belonging to no vocabulary is still refused.
    assert!(routing("wizard=opus").is_err());

    // No key is claimed twice across the vocabularies, or one would shadow
    // another and an operator could not tell which model they had set.
    let mut all: Vec<&str> = Role::all().into_iter().map(Role::as_str).collect();
    all.extend(crate::config::STATES);
    all.extend(["orchestrate", "apply"]);
    let unique: std::collections::BTreeSet<&str> = all.iter().copied().collect();
    assert_eq!(all.len(), unique.len(), "two vocabularies claim one key");
}

#[test]
fn the_two_step_lists_are_the_table_in_two_halves() {
    // Both say "in table order" in their own documentation, and nothing checked
    // it: every setting appears exactly once across the three lists, and each
    // list is the table with the other half removed.
    let table: Vec<Setting> = SETTINGS.to_vec();
    for (name, half) in [
        ("AGENT_SETTINGS", AGENT_SETTINGS),
        ("EVERYWHERE_SETTINGS", EVERYWHERE_SETTINGS),
        ("MACHINE_SETTINGS", MACHINE_SETTINGS),
        ("REPOSITORY_SETTINGS", crate::config::REPOSITORY_SETTINGS),
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
    // nothing left out.
    let mut all: Vec<Setting> = AGENT_SETTINGS.to_vec();
    all.extend_from_slice(EVERYWHERE_SETTINGS);
    all.extend_from_slice(MACHINE_SETTINGS);
    all.extend_from_slice(crate::config::REPOSITORY_SETTINGS);
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

/// The exact rows that reach `GateContext` as gate-enforced settings, and no
/// others — crossed against `SETTINGS` so another row cannot quietly join the
/// list, or leave it, without this failing. `RepositoryHost` joined at issue
/// #180 (carried for MCP host binding; claim allow/deny still uses Tracker).
#[test]
fn the_gate_reads_exactly_the_rows_it_decides_from() {
    let guarded: Vec<Setting> = SETTINGS
        .iter()
        .copied()
        .filter(|setting| setting.decides_what_the_gate_enforces())
        .collect();
    assert_eq!(
        guarded,
        vec![
            Setting::Tracker,
            Setting::RepositoryHost,
            Setting::Workflow,
            Setting::IssueRequirement,
            Setting::Integration,
            Setting::Boundaries,
            Setting::Window,
            Setting::GitOperationWindow,
            Setting::Evidence,
        ],
        "the rows the gate reads to decide changed without this test being told"
    );
}

#[test]
fn a_value_the_table_cannot_carry_is_refused_before_it_is_written() {
    // `make deploy | tee log` was written, read back as `make deploy`, and
    // refused as `setting-shadowed-by-local-file` — the right detection with
    // the wrong cause.
    for (setting, value) in [
        (Setting::Boundaries, "make deploy | tee log"),
        (Setting::Models, "sonnet | opus"),
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

    // And the markers around the row, one level out: a value carrying the
    // closing marker was **accepted**, and the next `config set` appended a
    // second table under a second pair of markers.
    //
    // Every marker the fence knows, from the fence, so a pair added there is
    // covered here the day it is added.
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
    // back different. Every accepted value of every setting, from the settings
    // themselves, so a new one is covered the day it is added.
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
            let written = if setting.scope() == crate::config::Scope::Repository {
                crate::config::settings::render_some_repository_rows(&config, &[*setting])
            } else {
                crate::config::settings::render_rows(&config)
            };
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
/// compared whole words, so a first cell with anything after the label is a
/// row the transport honours and Estigia does not.
#[test]
fn a_row_the_binding_acts_on_is_a_row_this_reader_finds() {
    let contract = |row: &str| {
        format!(
            "<!-- estigia:config:start -->\n\
             | Setting | Value here | Skill default |\n|---|---|---|\n\
             {row}\n<!-- estigia:config:end -->\n"
        )
    };
    let boundaries = |row: &str| {
        crate::config::Config::read(&contract(row), None)
            .expect("the table reads")
            .boundaries
    };

    // The floor: the exact spelling still arrives, and an unrelated row still
    // does not. Without both, matching everything would pass this.
    assert_eq!(
        boundaries("| Irreversible commands | npm publish |"),
        vec!["npm publish".to_owned()],
        "the exact spelling stopped being read"
    );
    assert!(
        boundaries("| Renewal window | 30s |").is_empty(),
        "a row about something else is being read as this setting"
    );

    for row in [
        "| Irreversible commands (ours) | npm publish |",
        "| Irreversible commands — the one-way doors | npm publish |",
    ] {
        assert!(
            !boundaries(row).is_empty(),
            "the transport acts on `{row}` and this reader reports the row switched off"
        );
    }
}

/// A label this crate forgives is one the transport reads too: the live
/// context reads *this crate's own rendering* of the configuration, layered by
/// the one reader that knows which document overrides which, so whatever the
/// operator typed, both sides answer the value this crate read out of it.
#[test]
fn a_label_this_crate_forgives_is_one_the_transport_reads_too() {
    let marked =
        |rows: &str| format!("<!-- estigia:config:start -->\n{rows}<!-- estigia:config:end -->\n");

    for label in [
        "Git operation window",
        "Git operation window (mine)",
        "`Git operation window`",
        "Git  operation  window",
    ] {
        let home = tempfile::tempdir().expect("a home");
        let skill = home.path().join("skill");
        crate::skill::install(&skill, &crate::config::Config::default(), false)
            .expect("the skill installs");
        std::fs::write(
            skill.join(crate::config::LOCAL_FILE),
            marked(&format!(
                "| Setting | Value |\n|---|---|\n| {label} | disabled |\n"
            )),
        )
        .expect("the operator's own file");

        // The floor: this crate reads the row at all. A label neither reader
        // recognises would satisfy the assertion below by measuring nothing.
        let told = crate::skill::installed_config_in_keeping_what_parses(&skill, home.path()).0;
        assert_eq!(
            super::Setting::GitOperationWindow.value_of(&told),
            "disabled",
            "{label} is not read as a git operation window here, so this case poses nothing"
        );

        let live = crate::transport::Context::live(skill.clone(), home.path().to_path_buf(), None);
        assert_eq!(
            live.get("git operation window"),
            Some("disabled"),
            "{label}: the gate reports a git operation window the transport will never read"
        );
    }
}

#[test]
fn every_answer_a_setting_offers_says_what_it_means() {
    // Every offered answer: a table filled in for fifteen rows and left blank
    // on the sixteenth teaches the operator the help is not there.
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
                    // Not the word back: what is refused is an explanation
                    // that **is** the answer.
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
    // The sentence an operator reads once named a handful of examples. Read out
    // of the two lists rather than trusting the sentence, so a key added to
    // either lands here.
    let accepted = Setting::Models.accepted();
    let mut unnamed: Vec<&str> = Vec::new();
    let mut named = 0;
    for key in crate::config::ModelRouting::targets() {
        // The bare word, because the backticks in this sentence belong to
        // values somebody can write and these are **keys**. Quoting them made
        // `every_value_a_setting_offers_is_a_value_it_accepts` try to apply
        // `developer` as a routing and refuse it — a help text and a guard
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
        named + unnamed.len() >= 6,
        "only {} key(s) were read out of the two lists",
        named + unnamed.len()
    );
    assert!(
        unnamed.is_empty(),
        "the routing takes these and never names them: {unnamed:?}"
    );

    // And each one really is taken, so the sentence is not naming words the
    // parser would refuse — the other way this help can lie.
    for key in ["orchestrate", "apply", "analyst", "developer"] {
        let mut config = Config::default();
        Setting::Models
            .apply(&mut config, &format!("{key}=opus"))
            .unwrap_or_else(|_| panic!("{key} is named as a key and the parser refuses it"));
    }
}

#[test]
fn the_operator_table_no_longer_declares_a_repair_ceiling() {
    assert!(
        SETTINGS
            .iter()
            .all(|setting| setting.label() != "Repair ceiling"),
        "the retired ceiling is still a setting"
    );
    assert!(Setting::from_label("Repair ceiling").is_none());
    assert!(!Config::default().render_rows().contains("Repair ceiling"));
}

#[test]
fn an_installed_repair_ceiling_row_is_reported_wherever_it_sits_and_changes_nothing() {
    for value in ["3", "none", "5"] {
        let row = table(&[("Repair ceiling", value)]);
        let code = |report: Option<crate::outcome::Refusal>, place: &str| {
            let report = report.unwrap_or_else(|| panic!("{place} {value}: the row was dropped"));
            assert_eq!(report.code, "repair-ceiling-retired", "{place} {value}");
        };

        let (config, report) =
            Config::read_folding_retired_workers(&row, None).expect("a retired row still loads");
        code(report, "shared table");
        assert_eq!(config, Config::default(), "the row changed the answer");
        let (_, report) = Config::read_folding_retired_workers(&table(&[]), Some(&row))
            .expect("a local override still loads");
        code(report, "local override");
        let (_, report) = Config::read_keeping_what_parses_reporting(&row, None);
        code(report, "tolerant read");
        for scope in [Scope::Agent, Scope::Everywhere, Scope::Repository] {
            let (config, _, report) =
                Config::read_scope_over_folding_retired_workers(&Config::default(), &row, scope)
                    .expect("a scoped override still loads");
            code(report, &format!("{scope:?} file"));
            assert_eq!(
                config,
                Config::default(),
                "{scope:?}: the row changed the answer"
            );
        }
    }
}

#[test]
fn the_operator_table_declares_a_review_budget() {
    assert!(
        SETTINGS
            .iter()
            .any(|setting| setting.label() == "Review budget"),
        "a review has no declared per-context token figure"
    );
    assert_eq!(
        Setting::ReviewBudget.default_value(),
        "150000",
        "the shipped default is not the 150,000 tokens per context the table documents"
    );
}

#[test]
fn review_budget_none_is_off_and_zero_is_refused() {
    let mut config = Config::default();
    Setting::ReviewBudget
        .apply(&mut config, "none")
        .expect("none switches it off");
    assert_eq!(config.review_budget, crate::config::ReviewBudget::Off);
    assert!(Setting::ReviewBudget.apply(&mut config, "0").is_err());
}

#[test]
fn corrections_per_lineage_defaults_to_three() {
    assert_eq!(Setting::CorrectionsPerLineage.default_value(), "3");
    assert_eq!(Config::default().corrections_per_lineage, 3);
}

#[test]
fn corrections_per_lineage_takes_a_positive_whole_number_and_refuses_the_rest() {
    let mut config = Config::default();
    Setting::CorrectionsPerLineage
        .apply(&mut config, " 1 ")
        .expect("one is a positive whole number");
    assert_eq!(config.corrections_per_lineage, 1);
    for refused in ["0", "-1", "2.5", "three", ""] {
        assert!(
            Setting::CorrectionsPerLineage
                .apply(&mut config, refused)
                .is_err(),
            "{refused:?} was accepted"
        );
        assert_eq!(
            config.corrections_per_lineage, 1,
            "{refused:?} moved the row"
        );
    }
}

#[test]
fn the_rendered_judge_material_does_not_name_the_ceiling() {
    let blind = include_str!("../../skill/agents/review-blind.md");
    for needle in [
        "Repair ceiling",
        "repair ceiling",
        "confirmed-finding",
        "repair rounds",
        "repair_rounds",
        "author-elected",
    ] {
        assert!(!blind.contains(needle), "judge material names {needle:?}");
    }
}

#[test]
fn seal_has_exactly_two_values() {
    assert_eq!(Seal::all().len(), 2);
    assert_eq!(Seal::Sealed.as_value(), "sealed");
    assert_eq!(Seal::Unsealed.as_value(), "unsealed");
}

#[test]
fn a_github_remote_names_its_repository_in_every_shape_git_writes_it() {
    for url in [
        "https://github.com/asanabrial/estigia",
        "https://github.com/asanabrial/estigia.git",
        "git@github.com:asanabrial/estigia.git",
        "git@github.com:asanabrial/estigia",
        "ssh://git@github.com/asanabrial/estigia",
        "ssh://git@github.com/asanabrial/estigia.git\n",
    ] {
        let repo = RepoRef::from_github_remote(url).unwrap_or_else(|| panic!("{url:?}"));
        assert_eq!(
            (repo.owner.as_str(), repo.name.as_str()),
            ("asanabrial", "estigia")
        );
    }
    for url in [
        "https://gitlab.com/asanabrial/estigia.git",
        "git@bitbucket.org:asanabrial/estigia.git",
        "https://github.com/asanabrial",
        "https://github.com/a/b/c",
        "/srv/git/estigia.git",
        "",
    ] {
        assert!(RepoRef::from_github_remote(url).is_none(), "{url:?}");
    }
}

#[test]
fn the_loop_rows_name_every_adapter_that_reads_them() {
    let loop_on = Setting::LoopPlugin
        .means("on")
        .expect("the loop row explains on");
    for fragment in [
        "`/flow-loop`",
        "`/flow-status`",
        "`Stop`",
        "`flow-loop.js`",
        "`estigia-loop.ts`",
        "Codex",
    ] {
        assert!(
            loop_on.contains(fragment),
            "{fragment} missing from {loop_on:?}"
        );
    }

    let goal_loop_on = Setting::GoalLoopPlugin
        .means("on")
        .expect("the goal and loop row explains on");
    for fragment in ["`goal-loop.js`", "`estigia-goal-loop.ts`", "Pi", "OpenCode"] {
        assert!(
            goal_loop_on.contains(fragment),
            "{fragment} missing from {goal_loop_on:?}"
        );
    }
}

#[test]
fn the_loop_rows_describe_themselves_without_naming_a_subset_of_adapters() {
    for setting in [Setting::LoopPlugin, Setting::GoalLoopPlugin] {
        let about = setting.about();
        for adapter in ["OpenCode", "Codex", "Pi", "Claude Code"] {
            assert!(
                !about.contains(adapter),
                "{setting:?} names {adapter} and will go stale: {about:?}"
            );
        }
    }
    assert!(Setting::GoalLoopPlugin.about().contains("`/goal`"));
    assert!(Setting::LoopPlugin.about().contains("claimable"));
}
