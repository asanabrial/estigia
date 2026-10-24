use super::app::{
    AGENT_TUI_SETTINGS, AGENT_TUI_SETTINGS_WITH_LOOP_ONLY, AGENT_TUI_SETTINGS_WITHOUT_LOOP_PLUGIN,
    Action, App, Focus, Key, Page, TUI_SETTINGS, TYPE_IT,
};
use super::words::Tongue;
use crate::config::{
    AGENT_SETTINGS, Config, EVERYWHERE_SETTINGS, OPTIONS_SETTINGS, SETTINGS, Scope, Setting,
};
use crate::setup::Installed;

#[cfg(windows)]
const ELSEWHERE: &str = "H:/somewhere/else";
#[cfg(unix)]
const ELSEWHERE: &str = "/somewhere/else";

#[cfg(windows)]
const ABSENT: &str = "H:/no/such/place";
#[cfg(unix)]
const ABSENT: &str = "/no/such/place";

fn acknowledge(app: &mut App, summary: &str) {
    let read_back: std::collections::BTreeMap<&'static str, Config> = app
        .chosen()
        .into_iter()
        .map(|adapter| (adapter.slug, app.configs[adapter.slug].clone()))
        .collect();
    app.installed_now(super::InstallReceipt {
        summary: summary.to_owned(),
        contract_read_back: read_back.clone(),
        agent_read_back: read_back.clone(),
        local_read_back: std::collections::BTreeMap::new(),
        unlayered_read_back: read_back.clone(),
        acknowledged: read_back
            .keys()
            .map(|slug| (*slug, crate::config::SETTINGS.to_vec()))
            .collect(),
        completed: read_back.keys().copied().collect(),
        read_back,
        repository: None,
        repository_settings: Vec::new(),
    });
}

#[test]
fn a_one_row_repository_receipt_preserves_each_agents_inherited_rows() {
    let claude = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    let codex = crate::setup::find_agent("codex").expect("Codex is an adapter");
    let mut claude_config = Config::default();
    Setting::Tracker
        .apply(&mut claude_config, "github other/issues")
        .expect("the Claude tracker is accepted");
    let mut codex_config = Config::default();
    Setting::Tracker
        .apply(&mut codex_config, "github acme/issues")
        .expect("the Codex tracker is accepted");
    let installed = std::collections::BTreeMap::from([
        (claude.slug, claude_config.clone()),
        (codex.slug, codex_config.clone()),
    ]);
    let mut app = App::with_configs(&installed, &[]);
    let mut repository = claude_config;
    Setting::Evidence
        .apply(&mut repository, "measuring")
        .expect("the repository evidence standard is accepted");

    let mut receipt = super::InstallReceipt::empty("repository saved".to_owned());
    receipt.repository = Some(repository);
    receipt.repository_settings = vec![Setting::Evidence];
    app.installed_partially(receipt);

    assert_eq!(
        Setting::Evidence.value_of(&app.installed[claude.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Evidence.value_of(&app.installed[codex.slug]),
        "measuring"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "github other/issues"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[codex.slug]),
        "github acme/issues",
        "the repository receipt copied Claude's inherited Tracker onto Codex"
    );
}

/// The agents list, with nothing ticked and the cursor on **all chosen agents**.
fn setup() -> App {
    let mut app = App::new(Config::default());
    walk_the_menu_to(&mut app, Page::Agents);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Agents);
    app
}

/// An agent's detail page, with the first two agents ticked and the second one open.
fn per_agent() -> App {
    let mut app = setup();
    app.press(Key::Down);
    app.press(Key::Char(' '));
    app.press(Key::Down);
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Detail);
    app
}

/// The detail page of the first ticked agent, with an explicit set of already configured adapters.
fn per_agent_with(configured: &[&str]) -> App {
    let mut app = App::with_agents(Config::default(), configured);
    agent_detail(&mut app);
    app
}

/// From the menu to the detail page of the first ticked agent.
fn agent_detail(app: &mut App) {
    walk_the_menu_to(app, Page::Agents);
    app.press(Key::Enter);
    walk_agents_to_a_chosen_one(app);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Detail);
    assert!(!app.everyone);
}

/// From the menu to the detail page of **all chosen agents**.
fn everyone_detail(app: &mut App) {
    walk_the_menu_to(app, Page::Agents);
    app.press(Key::Enter);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Detail);
    assert!(app.everyone);
}

/// The options page, reached by the keys that reach it.
fn options() -> App {
    let mut app = per_agent();
    to_options(&mut app);
    app
}

/// Walks from wherever this is to the options page, through the keys.
fn to_options(app: &mut App) {
    to_menu(app);
    walk_the_menu_to(app, Page::Options);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Options);
}

/// Walks back to the agents list.
fn to_setup(app: &mut App) {
    to_menu(app);
    walk_the_menu_to(app, Page::Agents);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Agents);
}

fn to_menu(app: &mut App) {
    for _ in 0..3 {
        if app.page == Page::Home {
            return;
        }
        app.press(Key::Esc);
    }
    assert_eq!(app.page, Page::Home, "the walk never reached the menu");
}

/// Presses `Down` on the agents list until the cursor is on an agent that is ticked.
fn walk_agents_to_a_chosen_one(app: &mut App) {
    for _ in 0..=crate::setup::AGENTS.len() {
        if !app.everyone && app.agents[app.agent].1 {
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never reached a ticked agent");
}

/// Goes back to the list and opens the next ticked agent, which is not this one.
/// Written as "until it changes" so these tests say what they mean, which is
/// *another agent*, and stop depending on how many agents are ticked.
fn to_another_agent(app: &mut App) {
    let from = app.current();
    app.press(Key::Esc);
    assert_eq!(app.page, Page::Agents);
    for _ in 0..=crate::setup::AGENTS.len() + 1 {
        app.press(Key::Down);
        if !app.everyone && app.agents[app.agent].1 && app.current() != from {
            app.press(Key::Enter);
            assert_eq!(app.page, Page::Detail);
            return;
        }
    }
    panic!("`Down` never reached a ticked agent other than {from}");
}

/// Goes back to the list and opens `wanted`, which has to be ticked.
fn back_to_agent(app: &mut App, wanted: &str) {
    app.press(Key::Esc);
    assert_eq!(app.page, Page::Agents);
    for _ in 0..=crate::setup::AGENTS.len() + 1 {
        if !app.everyone && app.current() == wanted {
            app.press(Key::Enter);
            assert_eq!(app.page, Page::Detail);
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never came to {wanted}");
}

/// Goes back to the list and opens **all chosen agents**.
fn to_everyone(app: &mut App) {
    app.press(Key::Esc);
    assert_eq!(app.page, Page::Agents);
    for _ in 0..=crate::setup::AGENTS.len() + 1 {
        if app.everyone {
            app.press(Key::Enter);
            assert_eq!(app.page, Page::Detail);
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never came round to all chosen agents");
}

fn walk_the_menu_to(app: &mut App, wanted: Page) {
    for _ in 0..=super::app::MENU.len() {
        if app.entry().goes == super::app::Goes::To(wanted) {
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never reached the entry that opens {wanted:?}");
}

/// Presses `key` on the agents list until the cursor is on `wanted`.
fn walk_agents_to(app: &mut App, wanted: &str, key: Key) {
    for _ in 0..=crate::setup::AGENTS.len() * 2 {
        if !app.everyone && app.current() == wanted {
            return;
        }
        app.press(key);
    }
    panic!("{key:?} never reached {wanted}");
}

/// Presses `Down` until the cursor is on a row that answers a setting.
fn walk_onto_a_setting(app: &mut App) {
    for _ in 0..=SETTINGS.len() * 2 {
        if app.setting_at_cursor().is_some() {
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never reached a row that answers a setting");
}

fn walk_to(app: &mut App, wanted: Setting) {
    for _ in 0..=SETTINGS.len() {
        if app.setting() == wanted {
            return;
        }
        app.press(Key::Down);
    }
    panic!(
        "{:?} is not on the {:?} page, so this walk would never have ended",
        wanted, app.page
    );
}

fn walk_to_model_target(app: &mut App, wanted: &str) {
    for _ in 0..app.row_count() {
        if app
            .model_target_at_cursor()
            .is_some_and(|target| target.name == wanted)
        {
            return;
        }
        app.press(Key::Down);
    }
    panic!("model target {wanted:?} is not visible");
}

fn walk_to_model_profile(app: &mut App) {
    for _ in 0..app.row_count() {
        if app.model_profile_at_cursor() {
            return;
        }
        app.press(Key::Down);
    }
    panic!("the model profile row is not visible");
}

fn type_in(app: &mut App, text: &str) {
    for character in text.chars() {
        app.press(Key::Char(character));
    }
}

fn answer(app: &mut App, value: &str) {
    app.press(Key::Char(' '));
    if app.focus == Focus::Picking {
        let entries = app.picker();
        if let Some(at) = entries.iter().position(|entry| entry == value) {
            walk_picker_to(app, at);
            app.press(Key::Enter);
            return;
        }
        let at = entries
            .iter()
            .position(|entry| entry == TYPE_IT)
            .expect("a value that is not on the list needs somewhere to type");
        walk_picker_to(app, at);
        app.press(Key::Enter);
    }
    assert_eq!(app.focus, Focus::Editing, "no field opened");
    for _ in 0..64 {
        app.press(Key::Backspace);
    }
    type_in(app, value);
    app.press(Key::Enter);
}

/// Opens the text field for the row under the cursor.
fn open_the_field(app: &mut App) {
    // Space opens what a row offers; `Enter` accepts the step.
    app.press(Key::Char(' '));
    if app.focus == Focus::Picking {
        let at = if app.model_target_at_cursor().is_some() {
            app.picker()
                .iter()
                .position(|entry| entry == "type a model ID…")
                .expect("a model row offers custom input")
        } else {
            app.picker()
                .iter()
                .position(|entry| entry == TYPE_IT)
                .expect("this row offers somewhere to type")
        };
        walk_picker_to(app, at);
        app.press(Key::Enter);
    }
    assert_eq!(app.focus, Focus::Editing, "no field opened");
}

/// Moves the open picker to one of its entries.
fn walk_picker_to(app: &mut App, at: usize) {
    for _ in 0..app.picker().len() {
        if app.pick == at {
            return;
        }
        app.press(Key::Down);
    }
    panic!("the picker never reached entry {at}");
}

/// Chooses one exact displayed entry from the open picker.
fn choose_picker_entry(app: &mut App, wanted: &str) -> Action {
    let entries = app.picker();
    let at = entries
        .iter()
        .position(|entry| entry == wanted)
        .unwrap_or_else(|| panic!("{wanted:?} is not in {entries:?}"));
    walk_picker_to(app, at);
    app.press(Key::Enter)
}

fn another_value(app: &App, setting: Setting) -> Option<String> {
    setting
        .accepted()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .filter(|word| !word.is_empty())
        .find(|word| {
            let mut probe = app.config().clone();
            setting.apply(&mut probe, word).is_ok()
                && setting.value_of(&probe) != setting.value_of(app.config())
        })
        .map(ToOwned::to_owned)
}

#[test]
fn every_setting_is_reachable_from_an_agents_page_or_the_options_page() {
    let mut seen = Vec::new();
    // The OpenCode page is its own fixture: `Loop plugin` is offered
    // only where a plugin Estigia writes is read, so the two agents ticked by
    // `per_agent()` never meet it.
    for mut app in [per_agent(), options(), per_agent_with(&["opencode"])] {
        let mut wrapped = false;
        for _ in 0..app.row_count() {
            if let Some(setting) = app.setting_at_cursor()
                && !seen.contains(&setting)
            {
                seen.push(setting);
            }
            if app.model_target_at_cursor().is_some() && !seen.contains(&Setting::Models) {
                seen.push(Setting::Models);
            }
            app.press(Key::Down);
            if app.cursor() == 0 {
                wrapped = true;
                break;
            }
        }
        assert!(wrapped, "the cursor did not wrap");
        app.press(Key::Up);
        assert!(
            app.cursor() > 0 && app.cursor() < app.row_count(),
            "it did not wrap back"
        );
    }
    let reachable: Vec<Setting> = SETTINGS.to_vec();
    for setting in &reachable {
        assert!(seen.contains(setting), "{setting:?} cannot be reached");
    }
    assert_eq!(
        seen.len(),
        reachable.len(),
        "a setting is missing or duplicated"
    );
}

#[test]
fn an_agents_page_asks_only_what_differs_by_agent() {
    // The whole point of the split. A repository has one tracker whichever
    // agent looks at it, and asking eleven times invites eleven answers.
    let app = per_agent();
    assert_eq!(
        app.rows(),
        &[
            Setting::Profile,
            Setting::Delivery,
            Setting::Transitions,
            Setting::LoopPlugin,
        ]
    );
    assert!(!app.rows().contains(&Setting::Tracker));
    assert!(!app.rows().contains(&Setting::Models));
    assert_eq!(
        app.model_targets()
            .into_iter()
            .map(|target| target.name)
            .collect::<Vec<_>>(),
        [
            "orchestrate",
            "apply",
            "developer",
            "analyst",
            "reviewer",
            "refuter",
            "validator",
            "judge",
        ]
    );

    let app = options();
    assert_eq!(app.rows(), OPTIONS_SETTINGS);
    assert!(app.rows().contains(&Setting::Tracker));
    assert!(!app.rows().contains(&Setting::Models));
}

/// Every list a screen walks is [`SETTINGS`] filtered by a predicate, in the
/// table's own order — never a second order written by hand, which is what
/// drifted before this was checked.
#[test]
fn every_screen_list_is_the_table_filtered_by_its_own_predicate() {
    fn kept(predicate: impl Fn(&Setting) -> bool) -> Vec<Setting> {
        SETTINGS
            .iter()
            .copied()
            .filter(|setting| predicate(setting))
            .collect()
    }

    assert_eq!(
        OPTIONS_SETTINGS.to_vec(),
        kept(|setting| setting.scope() != Scope::Agent),
        "the options page is not the table without the agent rows"
    );
    assert_eq!(
        TUI_SETTINGS.to_vec(),
        kept(|setting| *setting != Setting::Models),
        "the contract screen is not the table without the model row it projects"
    );

    let agent = |loop_row: bool, goal_loop_row: bool| -> Vec<Setting> {
        kept(|setting| {
            setting.scope() == Scope::Agent
                && *setting != Setting::Models
                && match setting {
                    Setting::LoopPlugin => loop_row,
                    Setting::GoalLoopPlugin => goal_loop_row,
                    _ => true,
                }
        })
    };
    assert_eq!(AGENT_TUI_SETTINGS.to_vec(), agent(true, true));
    assert_eq!(
        AGENT_TUI_SETTINGS_WITH_LOOP_ONLY.to_vec(),
        agent(true, false)
    );
    assert_eq!(
        AGENT_TUI_SETTINGS_WITHOUT_LOOP_PLUGIN.to_vec(),
        agent(false, false)
    );
}

#[test]
fn the_loop_plugin_row_is_offered_where_the_agent_can_run_the_loop() {
    // The decision this row was sized on: a switch that does nothing is not
    // drawn. `applies_to_agent` is the predicate, and the two lists the screen
    // picks between follow it.
    let claude = per_agent_with(&["claude-code"]);
    assert_eq!(claude.current(), "claude-code");
    assert!(Setting::LoopPlugin.applies_to_agent(claude.adapter()));
    assert!(claude.rows().contains(&Setting::LoopPlugin));
    assert!(
        !claude.rows().contains(&Setting::GoalLoopPlugin),
        "Claude Code was offered the generic plugin it cannot read"
    );

    let opencode = per_agent_with(&["opencode"]);
    assert_eq!(opencode.current(), "opencode");
    assert!(Setting::LoopPlugin.applies_to_agent(opencode.adapter()));
    assert!(opencode.rows().contains(&Setting::LoopPlugin));
    assert!(opencode.rows().contains(&Setting::GoalLoopPlugin));

    let pi = per_agent_with(&["pi"]);
    assert_eq!(pi.current(), "pi");
    assert!(pi.rows().contains(&Setting::LoopPlugin));
    assert!(
        pi.rows().contains(&Setting::GoalLoopPlugin),
        "Pi was not offered the generic extension it reads"
    );

    let codex = per_agent_with(&["codex"]);
    assert_eq!(codex.current(), "codex");
    assert!(codex.rows().contains(&Setting::LoopPlugin));
    assert!(
        !codex.rows().contains(&Setting::GoalLoopPlugin),
        "Codex was offered the generic plugin it cannot read"
    );
}

#[test]
fn the_loop_plugin_row_is_offered_on_all_chosen_agents_only_when_every_one_acts_on_it() {
    // **All chosen agents** writes the answer to every chosen adapter,
    // so a row only some of them act on is not offered there.
    let mut mixed = per_agent_with(&["opencode", "cursor"]);
    mixed.everyone = true;
    assert!(
        !mixed.rows().contains(&Setting::LoopPlugin),
        "all chosen agents was offered a row only some chosen adapters act on"
    );

    let mut opencode_only = per_agent_with(&["opencode"]);
    opencode_only.everyone = true;
    assert!(
        opencode_only.rows().contains(&Setting::LoopPlugin),
        "all chosen agents hid a row every chosen adapter acts on"
    );

    let mut opencode_and_codex = per_agent_with(&["opencode", "codex"]);
    opencode_and_codex.everyone = true;
    assert!(opencode_and_codex.rows().contains(&Setting::LoopPlugin));
    assert!(
        !opencode_and_codex.rows().contains(&Setting::GoalLoopPlugin),
        "all chosen agents was offered the generic plugin with Codex chosen"
    );
}

#[test]
fn an_answer_on_the_options_page_lands_in_every_agents_table() {
    let mut app = options();
    walk_to(&mut app, Setting::Evidence);
    answer(&mut app, "measuring");

    for config in app.configs.values() {
        assert_eq!(
            config.evidence,
            crate::config::Evidence::Measuring,
            "a repository-wide answer reached only some of the tables"
        );
    }
    assert!(app.changed(Setting::Evidence));
    assert!(
        !app.disagrees(Setting::Evidence),
        "they were left disagreeing"
    );
    assert!(app.dirty());
}

#[test]
fn an_agents_page_edits_the_agent_it_is_open_on_and_nobody_else() {
    // "What does *this* agent do" is the question the page is asked. One
    // shared table forced anybody who wanted two agents to differ to leave the
    // screen and learn `config set`.
    let mut app = per_agent();
    let first = app.current();
    to_another_agent(&mut app);
    let second = app.current();
    assert_ne!(first, second, "the list did not open another agent");

    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");

    assert_eq!(
        app.configs[second].delivery,
        crate::config::Authority::Ask {
            timeout: crate::config::DEFAULT_ASK_TIMEOUT
        }
    );
    assert_eq!(
        app.configs[first].delivery,
        Config::default().delivery,
        "editing one agent changed another"
    );

    // Walking back shows the first agent's value again, not the edit just made.
    back_to_agent(&mut app, first);
    assert_eq!(app.current(), first);
    walk_to(&mut app, Setting::Delivery);
    assert_eq!(app.setting().value_of(app.config()), "auto");
    assert!(!app.changed(Setting::Delivery));
}

#[test]
fn an_agent_that_is_not_ticked_does_not_open() {
    // Opening it would invite edits to a table this run is not going to
    // write, which is an answer the operator will believe they gave.
    let mut app = setup();
    app.press(Key::Down);
    assert!(!app.everyone);
    assert!(!app.agents[app.agent].1);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Agents);
    assert!(app.message.is_some(), "the refusal said nothing");

    app.press(Key::Up);
    assert!(app.everyone);
    app.press(Key::Enter);
    assert_eq!(
        app.page,
        Page::Agents,
        "all chosen agents opened with nobody ticked"
    );
    assert!(app.message.is_some());
}

#[test]
fn an_agent_estigia_cannot_gate_says_so_on_every_row_that_would_need_one() {
    let ungated = crate::setup::AGENTS
        .iter()
        .position(|adapter| !adapter.can_gate_tools())
        .expect("at least one adapter is contract-only");

    let mut app = setup();
    app.everyone = false;
    app.agent = ungated;
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.current(), crate::setup::AGENTS[ungated].slug);

    // Captioned, because what happens to a contract-only answer afterwards is
    // different. Every row is editable: no `Inert` answer is left to close one.
    for setting in [Setting::Delivery, Setting::Transitions] {
        let applies = app.adapter().applies(setting);
        assert!(
            applies.because().is_some(),
            "{setting:?} is unenforced here and the screen does not say so"
        );
    }

    // A gated agent holds every row: the gate is what stops and asks.
    let gated = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.can_gate_tools())
        .expect("at least one adapter is gated");
    for setting in SETTINGS {
        assert_eq!(
            gated.applies(*setting),
            crate::setup::Applies::Held,
            "{setting:?} has the wrong capability caveat for an agent Estigia gates"
        );
    }
}

#[test]
fn a_model_row_opens_its_catalog_directly_and_space_confirms_the_model() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");

    assert_eq!(app.press(Key::Char(' ')), Action::None);
    assert_eq!(app.focus, Focus::Picking);
    assert!(app.picker().iter().any(|entry| entry == "opus"));
    for removed in ["Advanced", "edit whole route…", "clear all assignments"] {
        assert!(!app.picker().iter().any(|entry| entry == removed));
    }

    let at = app
        .picker()
        .iter()
        .position(|entry| entry == "opus")
        .expect("Claude's curated model");
    walk_picker_to(&mut app, at);
    app.press(Key::Char(' '));

    assert_eq!(app.focus, Focus::List);
    assert_eq!(app.model_value("orchestrate"), "opus");
    assert_eq!(app.shown_value(Setting::Models), "orchestrate=opus");
}

#[test]
fn model_picker_always_offers_custom_and_target_local_inherit() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: crate::config::ModelRouting::parse(
                "reviewer=keep, orchestrate=private/current",
            )
            .expect("a model route"),
            ..Config::default()
        },
    );
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));

    for entry in ["private/current", "type a model ID…", "inherit"] {
        assert!(
            app.picker().iter().any(|offered| offered == entry),
            "missing {entry:?} from {:?}",
            app.picker()
        );
    }
    choose_picker_entry(&mut app, "inherit");

    assert_eq!(app.shown_value(Setting::Models), "reviewer=keep");
    assert_eq!(app.model_value("orchestrate"), "inherit");
}

#[test]
fn space_confirms_an_ordinary_picker_but_remains_literal_in_the_model_editor() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Delivery);
    app.press(Key::Char(' '));
    let at = app
        .picker()
        .iter()
        .position(|entry| entry == "ask")
        .expect("the delivery answer");
    walk_picker_to(&mut app, at);
    app.press(Key::Char(' '));
    assert_eq!(app.shown_value(Setting::Delivery), "ask");

    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    type_in(&mut app, "provider");
    app.press(Key::Char(' '));
    type_in(&mut app, "model");
    assert_eq!(app.draft, "provider model");
}

#[test]
fn uniform_model_rows_aggregate_each_target_without_borrowing_a_catalog() {
    let config = |route| Config {
        models: crate::config::ModelRouting::parse(route).expect("a model route"),
        ..Config::default()
    };
    let installed = std::collections::BTreeMap::from([
        ("claude-code", config("orchestrate=opus, reviewer=shared")),
        ("codex", config("orchestrate=gpt-5.6-sol, reviewer=shared")),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    everyone_detail(&mut app);

    assert_eq!(app.model_value("orchestrate"), "different values");
    assert_eq!(app.model_value("reviewer"), "shared");
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    assert_eq!(app.picker(), ["type a model ID…", "inherit",]);
}

#[test]
fn model_routing_disagreement_is_named_on_every_target() {
    let config = |route| Config {
        models: crate::config::ModelRouting::parse(route).expect("a model route"),
        ..Config::default()
    };
    let installed = std::collections::BTreeMap::from([
        ("claude-code", config("orchestrate=opus")),
        ("codex", config("orchestrate=gpt-5.6-sol")),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    everyone_detail(&mut app);

    let names = app
        .model_targets()
        .into_iter()
        .map(|target| target.name)
        .collect::<Vec<_>>();
    for fixed in ["orchestrate", "apply", "developer", "analyst"] {
        assert!(names.contains(&fixed), "missing fixed target {fixed:?}");
    }
    walk_to_model_target(&mut app, "orchestrate");
    assert!(
        paint(&app, 100, 30).contains("Model routing differs across selected agents"),
        "the rows do not explain why the targets differ"
    );
}

#[test]
fn model_targets_are_the_fixed_phase_and_role_set() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let app = App::one_table(Some(claude), Config::default());
    assert_eq!(
        app.model_targets()
            .into_iter()
            .map(|target| target.name)
            .collect::<Vec<_>>(),
        [
            "orchestrate",
            "apply",
            "developer",
            "analyst",
            "reviewer",
            "refuter",
            "validator",
            "judge",
        ]
    );
}

#[test]
fn no_tui_page_offers_a_planning_option() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let pages = [
        per_agent(),
        App::one_table(Some(claude), Config::default()),
        options(),
    ];
    for mut app in pages {
        for _ in 0..app.row_count() {
            app.press(Key::Down);
        }
        let painted = paint(&app, 120, 40);
        assert!(
            !painted.contains("Planning"),
            "the screen paints a planning option:\n{painted}"
        );
        assert!(
            !painted.to_lowercase().contains("sdd"),
            "the screen paints an SDD option:\n{painted}"
        );
    }
}

#[test]
fn uniform_model_routing_reaches_only_selected_agents() {
    let mut app = per_agent();
    let selected = app
        .chosen()
        .into_iter()
        .map(|adapter| adapter.slug)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 2, "the fixture needs two selected agents");
    let unselected = app
        .agents
        .iter()
        .find(|(_, on)| !*on)
        .map(|(adapter, _)| adapter.slug)
        .expect("a third, unselected agent");
    Setting::Models
        .apply(
            app.configs.get_mut(unselected).expect("its table"),
            "reviewer=opus",
        )
        .expect("the conflicting Model routing value is valid");
    let untouched = app.configs[unselected].clone();
    app.everyone = true;

    app.set(Setting::Models, "developer=inherit")
        .unwrap_or_else(|refusal| panic!("a routing is accepted: {refusal}"));

    for slug in &selected {
        assert_eq!(
            Setting::Models.value_of(&app.configs[*slug]),
            "developer=inherit",
            "uniform Model routing did not reach selected agent {slug}"
        );
    }
    assert_eq!(
        app.configs[unselected].render_rows(),
        untouched.render_rows(),
        "uniform Model routing changed unselected agent {unselected}"
    );
    assert_eq!(
        app.model_targets()
            .into_iter()
            .map(|target| target.name)
            .collect::<Vec<_>>(),
        [
            "orchestrate",
            "apply",
            "developer",
            "analyst",
            "reviewer",
            "refuter",
            "validator",
            "judge",
        ]
    );

    let before = app.configs.clone();
    assert!(app.set(Setting::Models, "not-a-routing").is_err());
    assert_eq!(
        app.configs, before,
        "a rejected uniform Model routing value partially changed the tables"
    );
}

#[test]
fn settings_and_models_render_as_separate_nonselectable_blocks() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    let screen = paint(&app, 80, 24);

    assert!(screen.contains("MODELS"));
    assert!(screen.contains("orchestrate"));
    assert_eq!(
        app.row_count(),
        app.rows().len() + 1 + app.model_targets().len()
    );
}

#[test]
fn the_model_routing_row_presents_phases_and_roles_as_two_sections() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_target(&mut app, "orchestrate");

    let top = paint(&app, 80, 24);
    assert!(
        top.contains("PHASES"),
        "the phases section heads the routing list:\n{top}"
    );

    walk_to_model_target(&mut app, "developer");
    let scrolled = paint(&app, 80, 24);
    assert!(
        scrolled.contains("ROLES"),
        "the roles section is reached by scrolling to a role:\n{scrolled}"
    );
    assert!(
        scrolled.contains("developer"),
        "the delegated role renders under its section:\n{scrolled}"
    );

    let sections: Vec<crate::config::ModelSection> = app
        .model_targets()
        .iter()
        .map(|target| target.kind.section())
        .collect();
    let first_roles = sections
        .iter()
        .position(|section| *section == crate::config::ModelSection::Roles)
        .expect("a roles section");
    assert!(
        sections[..first_roles]
            .iter()
            .all(|section| *section == crate::config::ModelSection::Phases),
        "no role is listed among the phases"
    );
    assert!(
        sections[first_roles..]
            .iter()
            .all(|section| *section == crate::config::ModelSection::Roles),
        "no phase is listed among the roles"
    );
    assert!(
        !app.model_targets()
            .iter()
            .any(|target| target.name == "PHASES" || target.name == "ROLES"),
        "a section heading is not a selectable target"
    );
}

#[test]
fn an_inherited_model_target_opens_an_empty_custom_id_field() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(app.shown_value(Setting::Models), "unset");

    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the answers did not open");
    choose_picker_entry(&mut app, "type a model ID…");
    assert_eq!(app.focus, Focus::Editing, "the field did not open");
    assert_eq!(app.draft, "", "inherit became editable content");

    type_in(&mut app, "gpt-5.6");
    app.press(Key::Enter);

    assert_eq!(app.focus, Focus::List, "a valid model left the editor open");
    assert_eq!(
        app.shown_value(Setting::Models),
        "orchestrate=gpt-5.6",
        "the typed model did not update its target"
    );
}

#[test]
fn a_model_row_opens_models_without_an_intermediate_target_or_advanced_stage() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the selector did not open");

    let entries = app.picker();
    assert!(entries.iter().any(|entry| entry == "opus"));
    for removed in [
        "orchestrate",
        "Advanced",
        "edit whole route…",
        "clear all assignments",
    ] {
        assert!(
            !entries.iter().any(|entry| entry == removed),
            "leaked {removed:?}"
        );
    }
}

#[test]
fn a_configured_model_route_opens_prefilled_for_incremental_editing() {
    let route = "orchestrate=gpt-5.6";
    let mut app = App::new(Config {
        models: crate::config::ModelRouting::parse(route).expect("a model route"),
        ..Config::default()
    });
    walk_the_menu_to(&mut app, Page::Agents);
    app.press(Key::Enter);
    app.press(Key::Down);
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Detail);
    walk_to_model_target(&mut app, "orchestrate");

    open_the_field(&mut app);
    assert_eq!(
        app.draft, "gpt-5.6",
        "the target's model was discarded instead of opening for a small edit"
    );
}

#[test]
fn model_suggestions_follow_the_concrete_agent_and_accept_injected_opencode_models() {
    let mut app = per_agent_with(&["claude-code", "codex", "opencode"]);
    assert_eq!(app.current(), "claude-code");
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    let claude = app.picker();
    for model in ["fable", "opus", "sonnet", "haiku"] {
        assert!(claude.iter().any(|entry| entry == model), "{claude:?}");
    }
    assert!(!claude.iter().any(|entry| entry == "gpt-6.1-sol"));

    app.press(Key::Esc);
    back_to_agent(&mut app, "codex");
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    let codex = app.picker();
    assert!(codex.iter().any(|entry| entry == "gpt-6.1-sol"));
    assert!(!codex.iter().any(|entry| entry == "fable"));
    assert_ne!(claude, codex, "two hosts were given one universal catalog");

    app.press(Key::Esc);
    back_to_agent(&mut app, "opencode");
    walk_to_model_target(&mut app, "orchestrate");
    let opencode = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "opencode")
        .expect("the OpenCode adapter");
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode),
        "OpenCode's host catalog was not requested lazily"
    );
    app.model_catalog_loaded(
        opencode,
        vec![
            "zai/glm-5".to_owned(),
            "anthropic/claude-sonnet-4-5".to_owned(),
            "bad/model,other".to_owned(),
            "bad/model|other".to_owned(),
        ],
    );
    let offered = app.picker();
    assert!(offered.iter().any(|entry| entry == "zai/glm-5"));
    assert!(
        offered
            .iter()
            .any(|entry| entry == "anthropic/claude-sonnet-4-5")
    );
    for foreign in ["fable", "opus", "sonnet", "haiku", "gpt-6.1-sol"] {
        assert!(
            !offered.iter().any(|entry| entry == foreign),
            "OpenCode borrowed {foreign:?}: {offered:?}"
        );
    }
    for malformed in ["bad/model,other", "bad/model|other"] {
        assert!(
            !offered.iter().any(|entry| entry == malformed),
            "OpenCode offered an unpersistable model ID: {offered:?}"
        );
    }
}

#[test]
fn an_unavailable_opencode_catalog_still_allows_a_custom_model() {
    let opencode = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "opencode")
        .expect("the OpenCode adapter");
    let mut app = App::one_table(Some(opencode), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode)
    );
    app.model_catalog_loaded(opencode, Vec::new());
    assert_eq!(app.picker(), ["type a model ID…", "inherit",]);

    choose_picker_entry(&mut app, "type a model ID…");
    type_in(&mut app, "provider/future-model");
    app.press(Key::Enter);
    assert_eq!(
        app.shown_value(Setting::Models),
        "orchestrate=provider/future-model"
    );
}

#[test]
fn choosing_a_target_and_model_changes_only_that_assignment() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: crate::config::ModelRouting::parse("reviewer=custom").expect("a model route"),
            ..Config::default()
        },
    );
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "opus");

    assert_eq!(
        app.shown_value(Setting::Models),
        "reviewer=custom, orchestrate=opus"
    );
    assert_eq!(
        app.focus,
        Focus::List,
        "the direct model picker did not close"
    );
}

#[test]
fn a_default_profile_replaces_the_route_and_custom_preserves_target_edits() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: crate::config::ModelRouting::parse("judge=private/model")
                .expect("a custom route"),
            ..Config::default()
        },
    );
    walk_to_model_profile(&mut app);
    assert_eq!(app.model_profile_value(), "custom");

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "balanced");
    assert_eq!(app.model_profile_value(), "balanced");
    assert_eq!(
        app.config().models.as_value(),
        claude.model_profiles()[0]
            .routing()
            .expect("the built-in profile is valid")
            .as_value(),
        "a preset was merged with a stale custom assignment"
    );

    walk_to_model_target(&mut app, "apply");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "haiku");
    assert_eq!(app.model_profile_value(), "custom");
    assert_eq!(app.config().models.for_target("apply"), Some("haiku"));

    walk_to_model_profile(&mut app);
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "custom");
    assert_eq!(app.config().models.for_target("apply"), Some("haiku"));
}

#[test]
fn profile_restore_and_dirty_marker_cover_route_changes() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let installed =
        crate::config::ModelRouting::parse("judge=private/model").expect("a custom route");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: installed.clone(),
            ..Config::default()
        },
    );
    walk_to_model_profile(&mut app);
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "balanced");
    assert!(app.model_profile_changed());
    let screen = paint(&app, 80, 24);
    let profile = screen
        .lines()
        .find(|line| line.contains("Profile"))
        .expect("the profile row is visible");
    assert!(
        profile.contains('*'),
        "the route change was unmarked:\n{screen}"
    );

    app.press(Key::Char('r'));
    assert_eq!(app.config().models, installed);
    assert!(!app.model_profile_changed());
    assert!(
        app.message
            .as_deref()
            .is_some_and(|message| message.contains("restored"))
    );
}

#[test]
fn profile_and_target_pickers_anchor_to_their_rendered_rows() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_profile(&mut app);
    app.press(Key::Char(' '));
    let profile_screen = paint(&app, 80, 24);
    assert!(profile_screen.contains("Model profile"));

    app.press(Key::Esc);
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    let target_screen = paint(&app, 80, 24);
    assert!(target_screen.contains("Models for orchestrate"));
    assert_ne!(
        profile_screen, target_screen,
        "both pickers anchored to one row"
    );
}

#[test]
fn dynamic_and_shared_model_views_do_not_invent_default_profiles() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    let dynamic = App::one_table(Some(opencode), Config::default());
    let shared = App::one_table(None, Config::default());

    assert!(!dynamic.has_model_profiles());
    assert!(!shared.has_model_profiles());
}

#[test]
fn a_custom_current_model_is_shown_and_custom_input_updates_only_its_target() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: crate::config::ModelRouting::parse(
                "reviewer=keep, orchestrate=private/current",
            )
            .expect("a model route"),
            ..Config::default()
        },
    );
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    assert!(
        app.picker().iter().any(|entry| entry == "private/current"),
        "the configured model disappeared because it is absent from the catalog"
    );

    choose_picker_entry(&mut app, "type a model ID…");
    assert_eq!(app.focus, Focus::Editing);
    assert_eq!(app.draft, "private/current");
    for _ in 0..app.draft.chars().count() {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "future/model");
    app.press(Key::Enter);
    assert_eq!(
        app.shown_value(Setting::Models),
        "reviewer=keep, orchestrate=future/model"
    );
}

#[test]
fn an_effort_is_cycled_on_the_row_and_is_not_in_the_picker() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let mut app = App::one_table(Some(claude), Config::default());

    walk_to_model_target(&mut app, "apply");
    app.press(Key::Char(' '));
    let entries = app.picker();
    assert!(
        !entries.iter().any(|entry| entry.starts_with("effort")),
        "the picker still offers an effort: {entries:?}"
    );
    app.press(Key::Esc);

    app.press(Key::Right);
    assert_eq!(app.shown_value(Setting::Models), "; apply=low");
    assert_eq!(app.model_value("apply"), "inherit \u{b7} low");
    for expected in ["medium", "high", "xhigh", "max"] {
        app.press(Key::Char('l'));
        assert_eq!(
            app.model_value("apply"),
            format!("inherit \u{b7} {expected}")
        );
    }
    app.press(Key::Right);
    assert_eq!(
        app.model_value("apply"),
        "inherit",
        "max did not wrap to the host"
    );
    assert_eq!(app.shown_value(Setting::Models), "unset");

    app.press(Key::Left);
    assert_eq!(app.model_value("apply"), "inherit \u{b7} max");
    app.press(Key::Char('h'));
    assert_eq!(app.model_value("apply"), "inherit \u{b7} xhigh");

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "opus");
    assert_eq!(app.shown_value(Setting::Models), "apply=opus; apply=xhigh");
    assert!(app.model_changed("apply"));

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "sonnet");
    assert_eq!(
        app.shown_value(Setting::Models),
        "apply=sonnet; apply=xhigh"
    );
    app.press(Key::Right);
    app.press(Key::Right);
    assert_eq!(app.shown_value(Setting::Models), "apply=sonnet");
    assert_eq!(app.model_value("apply"), "sonnet");
}

#[test]
fn a_divergent_row_cycles_its_effort_from_the_host_default() {
    let config = |route| Config {
        models: crate::config::ModelRouting::parse(route).expect("a model route"),
        ..Config::default()
    };
    let installed = std::collections::BTreeMap::from([
        ("claude-code", config("orchestrate=opus; orchestrate=max")),
        ("codex", config("orchestrate=gpt-5.5")),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    everyone_detail(&mut app);
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(app.model_value("orchestrate"), "different values");

    app.press(Key::Right);
    assert_eq!(app.model_value("orchestrate"), "different values");
    for slug in ["claude-code", "codex"] {
        assert_eq!(
            app.configs[slug]
                .models
                .route("orchestrate")
                .and_then(|route| route.effort),
            Some(crate::config::Effort::Low),
            "{slug} did not start from the host default"
        );
    }
    assert_eq!(
        app.configs["claude-code"]
            .models
            .route("orchestrate")
            .and_then(|r| r.model.clone()),
        Some("opus".to_owned()),
        "cycling an effort moved a model"
    );
}

#[test]
fn the_custom_field_edits_only_the_model() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let mut app = App::one_table(Some(claude), Config::default());

    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    type_in(&mut app, "anthropic/claude-opus-4");
    app.press(Key::Enter);
    assert_eq!(app.focus, Focus::List);
    assert_eq!(
        app.shown_value(Setting::Models),
        "orchestrate=anthropic/claude-opus-4",
        "a provider-qualified ID did not survive the field"
    );

    // Reopening prefills the model value, so editing it is editing what is
    // there rather than retyping the half the field remembered.
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    assert_eq!(app.draft, "anthropic/claude-opus-4");
    app.press(Key::Esc);
    app.press(Key::Esc);

    // The semicolon is a structural delimiter and is refused where it is typed.
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    type_in(&mut app, "opus; high");
    app.press(Key::Enter);
    assert_eq!(
        app.focus,
        Focus::Editing,
        "accepted a value carrying the effort-list delimiter"
    );
    assert_eq!(app.shown_value(Setting::Models), "unset");
    assert!(
        app.message
            .as_deref()
            .is_some_and(|message| message.contains("no comma, semicolon, pipe, or line break")),
        "the refusal did not name the model-ID grammar: {:?}",
        app.message
    );
}

/// Restoring a row brings its effort back with it.
#[test]
fn an_effort_only_change_is_marked_and_restored() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let installed = Config {
        models: crate::config::ModelRouting::parse("orchestrate=opus; orchestrate=max")
            .expect("a model route"),
        ..Config::default()
    };
    let mut app = App::one_table(Some(claude), installed);

    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(app.model_value("orchestrate"), "opus \u{b7} max");
    assert!(!app.model_changed("orchestrate"));

    app.press(Key::Right);
    app.press(Key::Right);
    assert_eq!(
        app.shown_value(Setting::Models),
        "orchestrate=opus; orchestrate=low"
    );
    assert!(
        app.model_changed("orchestrate"),
        "only the effort moved and the row reported no change"
    );

    app.press(Key::Char('r'));
    assert_eq!(
        app.shown_value(Setting::Models),
        "orchestrate=opus; orchestrate=max",
        "restore brought back the model and dropped the effort"
    );
    assert!(!app.model_changed("orchestrate"));
}

#[test]
fn custom_model_editor_rejects_persisted_entry_delimiters() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    for model in [
        "provider,model",
        "provider;model",
        "provider|model",
        "provider\rmodel",
        "provider\nmodel",
    ] {
        let mut app = App::one_table(Some(claude), Config::default());
        walk_to_model_target(&mut app, "orchestrate");
        app.press(Key::Char(' '));
        choose_picker_entry(&mut app, "type a model ID…");
        type_in(&mut app, model);
        app.press(Key::Enter);

        assert_eq!(app.focus, Focus::Editing, "accepted {model:?}");
        assert_eq!(app.shown_value(Setting::Models), "unset", "wrote {model:?}");
        assert!(
            app.message.as_deref().is_some_and(|message| {
                message.contains("no comma, semicolon, pipe, or line break")
            }),
            "the refusal did not name the model-ID grammar: {:?}",
            app.message
        );
    }
}

#[test]
fn inherit_removes_only_one_model_assignment_without_writing_target_unset() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            models: crate::config::ModelRouting::parse("reviewer=keep, orchestrate=opus")
                .expect("a model route"),
            ..Config::default()
        },
    );
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "inherit");
    let one_left = app.shown_value(Setting::Models);
    assert_eq!(one_left, "reviewer=keep");
    assert!(!one_left.contains("orchestrate=unset"));
}

#[test]
fn shared_uniform_and_neutral_model_routing_never_borrow_a_host_catalog() {
    let neutral = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "agents")
        .expect("the neutral adapter");
    let mut uniform = per_agent_with(&["claude-code"]);
    uniform.everyone = true;
    let shared = App::one_table(None, Config::default());
    let neutral = App::one_table(Some(neutral), Config::default());

    for (name, mut app, note) in [
        ("uniform", uniform, "no single agent model catalog"),
        ("shared", shared, "no single agent model catalog"),
        ("neutral", neutral, "no verified model catalog"),
    ] {
        walk_to_model_target(&mut app, "orchestrate");
        app.press(Key::Char(' '));
        let offered = app.picker();
        assert_eq!(
            offered,
            ["type a model ID…", "inherit",],
            "{name} borrowed a concrete host catalog"
        );
        assert!(
            app.model_picker_note()
                .is_some_and(|message| message.contains(note)),
            "{name} did not explain why it has no catalog"
        );
    }
}

#[test]
fn uniform_model_operations_use_only_chosen_agents_routes() {
    let mut app = uniform_models_with_distinct_installed_routes();
    let unchosen = "judge=unchosen-current, orchestrate=unchosen-current";

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    for _ in 0..app.draft.chars().count() {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "shared/new");
    app.press(Key::Enter);

    assert_eq!(
        app.configs["claude-code"].models.as_value(),
        "reviewer=claude-current, orchestrate=shared/new"
    );
    assert_eq!(
        app.configs["codex"].models.as_value(),
        "developer=codex-current, orchestrate=shared/new"
    );
    assert_eq!(app.configs["opencode"].models.as_value(), unchosen);

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "inherit");
    assert_eq!(
        app.configs["claude-code"].models.as_value(),
        "reviewer=claude-current"
    );
    assert_eq!(
        app.configs["codex"].models.as_value(),
        "developer=codex-current"
    );
    assert_eq!(app.configs["opencode"].models.as_value(), unchosen);

    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    type_in(&mut app, "changed");
    app.press(Key::Enter);
    app.press(Key::Char('r'));
    assert_eq!(
        app.configs["claude-code"].models.as_value(),
        "reviewer=claude-current, orchestrate=claude-installed"
    );
    assert_eq!(
        app.configs["codex"].models.as_value(),
        "developer=codex-current, orchestrate=codex-installed"
    );
    assert_eq!(app.configs["opencode"].models.as_value(), unchosen);
}

fn uniform_models_with_distinct_installed_routes() -> App {
    let config = |route| Config {
        models: crate::config::ModelRouting::parse(route).expect("a model route"),
        ..Config::default()
    };
    let installed = std::collections::BTreeMap::from([
        (
            "claude-code",
            config("reviewer=claude-installed, orchestrate=claude-installed"),
        ),
        (
            "codex",
            config("developer=codex-installed, orchestrate=codex-installed"),
        ),
        (
            "opencode",
            config("judge=unchosen-installed, orchestrate=unchosen-installed"),
        ),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    agent_detail(&mut app);

    app.configs
        .get_mut("claude-code")
        .expect("Claude table")
        .models =
        crate::config::ModelRouting::parse("reviewer=claude-current, orchestrate=claude-current")
            .expect("Claude routing");
    app.configs.get_mut("codex").expect("Codex table").models =
        crate::config::ModelRouting::parse("developer=codex-current, orchestrate=codex-current")
            .expect("Codex routing");
    app.configs
        .get_mut("opencode")
        .expect("unchosen OpenCode table")
        .models =
        crate::config::ModelRouting::parse("judge=unchosen-current, orchestrate=unchosen-current")
            .expect("unchosen routing");
    app.everyone = true;
    walk_to_model_target(&mut app, "orchestrate");
    app
}

#[test]
fn uniform_model_detail_names_every_selected_agent() {
    let mut app = per_agent_with(&["claude-code", "codex"]);
    app.everyone = true;
    walk_to_model_target(&mut app, "orchestrate");

    let screen = paint(&app, 120, 40);
    assert!(
        screen.contains("every selected agent"),
        "uniform Model routing still describes one displayed agent:\n{screen}"
    );
}

#[test]
fn uniform_model_detail_reports_divergent_installed_routes_in_both_languages() {
    for (tongue, expected) in [
        (
            crate::tui::words::Tongue::English,
            "every selected agent — installed: different values",
        ),
        (
            crate::tui::words::Tongue::Spanish,
            "cada agente seleccionado — instalado: valores diferentes",
        ),
    ] {
        let mut app = uniform_models_with_distinct_installed_routes();
        app.tongue = tongue;
        let screen = paint(&app, 120, 40);
        assert!(
            screen.contains(expected),
            "divergent installed Model routes were mislabeled in {tongue:?}:\n{screen}"
        );
        assert!(
            !screen.contains("claude-installed") && !screen.contains("codex-installed"),
            "one selected agent's installed route was presented as shared:\n{screen}"
        );
    }
}

#[test]
fn current_and_installed_model_assignments_share_one_bilingual_renderer() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    for (tongue, inherited, divergent) in [
        (
            crate::tui::words::Tongue::English,
            "inherit",
            "different values",
        ),
        (
            crate::tui::words::Tongue::Spanish,
            "heredar",
            "valores diferentes",
        ),
    ] {
        let exact = "Provider/Model_X";
        let mut app = App::one_table(
            Some(claude),
            Config {
                models: crate::config::ModelRouting::parse(&format!("orchestrate={exact}"))
                    .expect("an opaque model ID"),
                ..Config::default()
            },
        );
        app.tongue = tongue;
        walk_to_model_target(&mut app, "orchestrate");
        assert_eq!(app.model_value("orchestrate"), exact);
        let screen = paint(&app, 120, 40);
        assert!(
            screen.matches(exact).count() >= 2,
            "current and installed exact IDs were not preserved byte-for-byte in {tongue:?}:\n{screen}"
        );

        let mut app = App::one_table(Some(claude), Config::default());
        app.tongue = tongue;
        walk_to_model_target(&mut app, "orchestrate");
        assert_eq!(app.model_value("orchestrate"), inherited);
        let screen = paint(&app, 120, 40);
        assert!(
            screen.matches(inherited).count() >= 2,
            "current and installed inheritance disagree in {tongue:?}:\n{screen}"
        );
        if tongue == crate::tui::words::Tongue::Spanish {
            assert!(
                !screen.contains("inherit"),
                "the English inheritance sentinel leaked onto the Spanish screen:\n{screen}"
            );
        }

        let mut app = uniform_models_with_distinct_installed_routes();
        app.tongue = tongue;
        assert_eq!(app.model_value("orchestrate"), divergent);
        let screen = paint(&app, 120, 40);
        assert!(
            screen.matches(divergent).count() >= 2,
            "current and installed divergence disagree in {tongue:?}:\n{screen}"
        );
    }
}

#[test]
fn escape_unwinds_custom_model_editor_then_model_picker_then_rows() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");

    app.press(Key::Esc);
    assert_eq!(app.focus, Focus::Picking);
    assert!(app.picker().iter().any(|entry| entry == "type a model ID…"));
    app.press(Key::Esc);
    assert_eq!(app.focus, Focus::List);
}

#[test]
fn removed_advanced_model_actions_are_absent_from_the_picker() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    for removed in ["Advanced", "edit whole route…", "clear all assignments"] {
        assert!(!app.picker().iter().any(|entry| entry == removed));
    }
}

#[test]
fn model_picker_titles_details_and_keys_follow_the_screen_language() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(Some(claude), Config::default());
    app.tongue = crate::tui::words::Tongue::Spanish;
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    let models = paint(&app, 120, 34);
    assert!(models.contains("Modelos para orchestrate"));
    assert!(models.contains("sugerencias de modelos de Claude Code son orientativas"));
    assert!(models.contains("⏎ / espacio elige"));

    choose_picker_entry(&mut app, "escribe un ID de modelo…");
    let editing = paint(&app, 200, 34);
    assert!(editing.contains("sin coma, punto y coma, barra vertical ni salto de línea"));
    assert!(editing.contains("Esc cancela"));
}

#[test]
fn spanish_help_explains_the_structured_model_selector() {
    let mut app = App::new(Config::default());
    app.page = Page::Help;
    app.tongue = crate::tui::words::Tongue::Spanish;
    let help = paint(&app, 120, 90);

    for fact in [
        "Los destinos de modelo son un bloque aparte",
        "bajo los ajustes, con orchestrate",
        "abre directamente el catálogo orientativo",
        "heredar elimina solo ese destino",
        "CLI conserva la edición de la celda completa",
        "Solo Claude Code recibe actualmente definiciones",
    ] {
        assert!(help.contains(fact), "Spanish Help omits {fact:?}:\n{help}");
    }
}

#[test]
fn rendered_help_has_bilingual_semantic_parity_and_no_retired_model_flow() {
    let render = |tongue| {
        let mut app = App::new(Config::default());
        app.page = Page::Help;
        app.tongue = tongue;
        paint(&app, 200, 100)
    };
    let english = render(crate::tui::words::Tongue::English);
    let spanish = render(crate::tui::words::Tongue::Spanish);

    for (said, markers) in [
        (
            &english,
            [
                "Agents",
                "Push guard",
                "Marks",
                "a separate block beneath the settings",
                "Only Claude Code currently receives host-routable reviewer",
                "display label is translated",
                "printed CLI commands stay canonical",
            ],
        ),
        (
            &spanish,
            [
                "Agentes",
                "Guardia",
                "Marcas",
                "Los destinos de modelo son un bloque aparte",
                "Solo Claude Code recibe actualmente definiciones enrutables por el anfitrión de",
                "etiqueta visible se traduce",
                "valores aceptados y los comandos CLI impresos",
            ],
        ),
    ] {
        for marker in markers {
            assert!(
                said.contains(marker),
                "rendered Help omits {marker:?}:\n{said}"
            );
        }
    }
    for retired in [
        "opens in two stages",
        "Reparto de modelos se abre en dos etapas",
        "elige primero un destino",
        "edit the complete route from the picker",
    ] {
        assert!(
            !english.contains(retired) && !spanish.contains(retired),
            "rendered Help still describes retired behavior {retired:?}"
        );
    }
}

#[test]
fn worktree_placeholder_opens_empty_and_a_custom_path_opens_prefilled() {
    let mut unset = options();
    walk_to(&mut unset, Setting::Worktree);
    open_the_field(&mut unset);
    assert_eq!(
        unset.draft, "",
        "the worktree placeholder became editable content"
    );

    let root = tempfile::tempdir().expect("a worktree root");
    let path = root.path().display().to_string();
    let mut configured = options();
    walk_to(&mut configured, Setting::Worktree);
    configured
        .set(Setting::Worktree, &path)
        .expect("an absolute worktree path");
    open_the_field(&mut configured);
    assert_eq!(
        configured.draft, path,
        "an actual worktree path was discarded instead of opening for a small edit"
    );
}

#[test]
fn a_literal_unset_language_opens_prefilled_as_free_text() {
    let mut app = options();
    walk_to(&mut app, Setting::Summary);
    app.set(Setting::Summary, "unset")
        .expect("a free-text language name");
    assert_eq!(app.shown_value(Setting::Summary), "unset");

    open_the_field(&mut app);
    assert_eq!(
        app.draft, "unset",
        "a literal free-text value was mistaken for a placeholder"
    );
}

#[test]
fn walking_away_from_an_edit_is_not_making_it() {
    let mut app = per_agent();
    let before = app.setting().value_of(app.config());
    open_the_field(&mut app);
    type_in(&mut app, "-something-else");
    app.press(Key::Esc);
    assert_eq!(app.focus, Focus::List);
    assert_eq!(
        app.setting().value_of(app.config()),
        before,
        "leaving the editor changed the value"
    );
    assert!(!app.dirty());
}

#[test]
fn a_refused_value_keeps_the_editor_open_with_what_was_typed() {
    let mut app = per_agent();
    answer(&mut app, "not-a-value-any-setting-takes");

    assert_eq!(
        app.focus,
        Focus::Editing,
        "the editor closed and lost the answer to punish a typo"
    );
    assert_eq!(app.draft, "not-a-value-any-setting-takes");
    let message = app.message.as_deref().expect("the refusal is shown");
    assert!(
        !message.is_empty(),
        "the value was refused without saying why"
    );
    assert!(!app.dirty(), "a refused value reached the configuration");
}

#[test]
fn a_repository_wide_value_is_checked_before_any_table_is_written() {
    let mut app = options();
    walk_to(&mut app, Setting::Tracker);
    answer(&mut app, "not-a-tracker");
    assert_eq!(app.focus, Focus::Editing);
    assert!(app.message.is_some());
    for config in app.configs.values() {
        assert_eq!(config.tracker, Config::default().tracker);
    }
    assert!(!app.dirty());
}

#[test]
fn an_accepted_value_lands_and_the_row_says_it_changed() {
    let mut app = per_agent();
    let setting = app.setting();
    let Some(other) = another_value(&app, setting) else {
        return; // A free-text row with one sensible value; nothing to prove.
    };
    answer(&mut app, &other);

    assert_eq!(
        app.focus,
        Focus::List,
        "the editor stayed open on a good value"
    );
    assert_eq!(setting.value_of(app.config()), other);
    assert!(app.changed(setting), "the row does not show as changed");
    assert!(app.dirty());
}

#[test]
fn restoring_undoes_one_row_and_not_the_screen() {
    let mut app = per_agent();
    // Two rows that do not write each other. `Profile` is derived over the
    // two authorisations, so restoring it would also restore `Delivery`, and
    // a pair that shares a field cannot show one restoration sparing another.
    let first = Setting::Delivery;
    let second = Setting::Transitions;
    let mut moved = Vec::new();
    for setting in [first, second] {
        if let Some(other) = another_value(&app, setting)
            && app.set(setting, &other).is_ok()
        {
            moved.push(setting);
        }
    }
    if moved.len() < 2 {
        return; // Nothing to tell apart.
    }

    walk_to(&mut app, first);
    app.press(Key::Char('r'));
    assert!(!app.changed(first), "the row was not restored");
    assert!(
        app.changed(second),
        "restoring one row threw away an edit on another"
    );
}

#[test]
fn saving_needs_an_agent_and_says_so_where_one_is_chosen() {
    let mut app = setup();
    for page in [Page::Home, Page::Agents, Page::Options] {
        let mut app = app.clone();
        app.page = page;
        assert_eq!(app.press(Key::Char('s')), Action::None);
        assert!(!app.reviewing, "an empty save asked to be confirmed");
        assert_eq!(
            app.page,
            Page::Agents,
            "the refusal sent nobody to where agents are ticked"
        );
        assert!(app.message.is_some(), "an empty save went unremarked");
    }

    app.press(Key::Down);
    app.press(Key::Char(' '));
    assert_eq!(app.chosen().len(), 1);
    app.press(Key::Char(' '));
    assert!(app.chosen().is_empty(), "space did not untick");
    app.press(Key::Char(' '));

    // And now `s` is a plan, from every page that has the key: it asks first, and `Enter` writes.
    for page in [Page::Home, Page::Agents, Page::Options] {
        let mut app = app.clone();
        app.page = page;
        assert_eq!(app.press(Key::Char('s')), Action::None);
        assert!(app.reviewing, "{page:?} did not lay the review over itself");
        assert_eq!(app.press(Key::Enter), Action::Save);
        assert!(!app.reviewing);
    }
}

#[test]
fn quitting_with_unsaved_edits_warns_once_and_never_traps() {
    let mut app = per_agent();
    let setting = app.setting();
    let Some(other) = another_value(&app, setting) else {
        return;
    };
    app.set(setting, &other).expect("the value is accepted");
    assert!(app.dirty());

    // The first press warns instead of discarding.
    assert_eq!(app.press(Key::Char('q')), Action::None);
    assert!(app.message.is_some(), "the edits would have gone silently");
    // The second leaves. Refusing to quit is how a TUI traps somebody.
    assert_eq!(app.press(Key::Char('q')), Action::Quit);
    // And installing from that warned state is offered, not just discarding.
    let mut again = app.clone();
    again.message = Some("warned".to_owned());
    assert_eq!(again.press(Key::Char('s')), Action::None);
    assert!(again.reviewing);
    assert_eq!(again.press(Key::Enter), Action::Save);
}

#[test]
fn an_edit_on_one_agent_is_not_lost_by_walking_to_another() {
    let mut app = per_agent();
    let edited = app.current();
    app.set(Setting::Delivery, "ask").expect("accepted");
    to_another_agent(&mut app);
    assert_ne!(app.current(), edited, "the walk did not move");
    assert!(app.dirty(), "an edit on another agent was forgotten");
}

#[test]
fn the_vim_keys_do_what_the_arrows_do() {
    // Offered in the footer; a key that quietly did nothing would read as
    // broken.
    let mut arrows = per_agent();
    let mut letters = per_agent();
    arrows.press(Key::Down);
    letters.press(Key::Char('j'));
    assert_eq!(arrows.selected, letters.selected);
    arrows.press(Key::Up);
    letters.press(Key::Char('k'));
    assert_eq!(arrows.selected, letters.selected);
    arrows.press(Key::Right);
    letters.press(Key::Char('l'));
    assert_eq!(
        arrows.setting().value_of(arrows.config()),
        letters.setting().value_of(letters.config())
    );
    arrows.press(Key::Left);
    letters.press(Key::Char('h'));
    assert_eq!(
        arrows.setting().value_of(arrows.config()),
        letters.setting().value_of(letters.config())
    );
}

/// Every cell of one frame, as one string.
fn paint(app: &App, width: u16, height: u16) -> String {
    let mut app = app.clone();
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test terminal");
    terminal
        .draw(|frame| super::draw(frame, &mut app))
        .expect("the frame paints");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

#[test]
fn every_page_paints_and_says_what_it_shows() {
    let base = per_agent();
    let mut agents = base.clone();
    agents.press(Key::Esc);
    assert_eq!(agents.page, Page::Agents);
    let painted = paint(&agents, 110, 34);
    assert!(
        painted.contains("All chosen agents"),
        "the first row of the list is not drawn: {painted}"
    );
    for adapter in &agents.agents {
        assert!(
            painted.contains(adapter.0.display_name),
            "{} is not on the list",
            adapter.0.display_name
        );
    }

    // On a page of rows, the row's own explanation and every answer it takes —
    // so nobody has to remember a vocabulary to change a setting.
    let painted = paint(&base, 160, 34);
    assert!(painted.contains(base.setting().label()));
    assert!(
        painted.contains(base.setting().about()),
        "the row does not say what it is for"
    );
    for choice in base.setting().answers().choices {
        assert!(
            painted.contains(choice),
            "{choice:?} is an answer this row takes and the screen does not show it"
        );
    }
    // The brackets that say the arrow keys do something, on the row under the
    // cursor and nowhere else.
    assert!(painted.contains('\u{2039}') && painted.contains('\u{203a}'));

    // The open picker is a list of the answers, over everything else.
    let mut picking = base.clone();
    picking.press(Key::Char(' '));
    assert_eq!(picking.focus, Focus::Picking);
    let painted = paint(&picking, 110, 34);
    for choice in picking.setting().answers().choices {
        assert!(painted.contains(choice), "the picker is missing {choice:?}");
    }
    assert!(
        painted.contains("choose"),
        "the picker does not say what its keys are"
    );

    // And what is typed shows, once a field is open.
    let mut editing = base.clone();
    open_the_field(&mut editing);
    type_in(&mut editing, "zzmarker");
    assert!(
        paint(&editing, 110, 34).contains("zzmarker"),
        "the field is invisible"
    );

    // The review names what a save would write, over the page it was asked from.
    let mut reviewing = base;
    reviewing.press(Key::Char('s'));
    assert!(reviewing.reviewing);
    let painted = paint(&reviewing, 110, 34);
    assert!(
        painted.contains("install"),
        "the review does not say what will be written: {painted}"
    );
}

#[test]
fn the_screen_says_which_agents_it_can_actually_hold() {
    let app = setup();
    let painted = paint(&app, 110, 34);
    assert!(painted.contains("gated"), "nothing says which are held");
    assert!(
        painted.contains("contract only"),
        "the ungated ones are shown as though they were gated"
    );
}

#[test]
fn the_agents_already_configured_open_ticked() {
    // A second run must show what is installed rather than an empty list: an
    // operator who reruns setup and presses `s` would otherwise install into
    // nothing while believing they confirmed what they had.
    let app = App::with_agents(Config::default(), &["codex"]);
    let chosen: Vec<&str> = app.chosen().iter().map(|a| a.slug).collect();
    assert_eq!(chosen, vec!["codex"]);
}

#[test]
fn each_agent_opens_on_its_own_installed_table() {
    // A second run has to show what each agent already has. Seeding them all
    // from one table is the flattening defect, moved into the screen.
    let mut installed = std::collections::BTreeMap::new();
    installed.insert(
        "claude-code",
        Config {
            integration: crate::config::Integration::Trunk,
            ..Config::default()
        },
    );
    installed.insert("codex", Config::default());

    let app = App::with_configs(&installed, &["claude-code"]);
    assert_eq!(
        app.configs["claude-code"].integration,
        crate::config::Integration::Trunk
    );
    assert_eq!(
        app.configs["codex"].integration,
        Config::default().integration
    );
    // Nothing is "changed" before anybody changes it, per agent.
    assert!(!app.dirty());
}

#[test]
fn the_menu_is_the_first_screen_and_reaches_everything() {
    use super::app::{Goes, MENU};
    let mut app = App::new(Config::default());
    assert_eq!(app.page, Page::Home, "a job was put in front of the menu");

    // Every entry is reachable by moving down, and it wraps.
    let mut seen = vec![app.entry()];
    for _ in 1..MENU.len() {
        app.press(Key::Down);
        seen.push(app.entry());
    }
    assert_eq!(seen.len(), MENU.len());
    app.press(Key::Down);
    assert_eq!(app.entry, 0, "the menu did not wrap");

    // Each entry does what it says: a page opens, an action leaves.
    for (index, entry) in MENU.iter().enumerate() {
        let mut app = App::new(Config::default());
        app.entry = index;
        let action = app.press(Key::Enter);
        match entry.goes {
            Goes::To(page) => {
                assert_eq!(action, Action::None);
                assert_eq!(app.page, page, "{} opened the wrong page", entry.label);
            }
            Goes::Doing(_) => {
                assert_ne!(
                    action,
                    Action::None,
                    "{} names an action nothing performs, so the key does nothing",
                    entry.label
                );
                if entry.goes == Goes::Doing(super::app::Deed::Doctor) {
                    assert_eq!(app.page, Page::Doctor);
                    assert!(app.is_busy(), "the checks run with the screen not waiting");
                } else {
                    assert_eq!(app.page, Page::Home, "an action left the menu");
                }
            }
        }
    }

    // The push guard is on it. An operator who never finds it has a gate with
    // a hole in it and no way to know.
    assert!(
        MENU.iter()
            .any(|entry| entry.goes == Goes::Doing(super::app::Deed::Guard)),
        "the one boundary no agent can go around is not on the menu"
    );
}

#[test]
fn each_page_keeps_its_cursor_for_the_session() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Transitions);
    let detail_at = app.cursor();
    to_options(&mut app);
    walk_to(&mut app, Setting::Evidence);
    let options_at = app.cursor();
    assert_ne!(detail_at, options_at);
    to_setup(&mut app);
    app.press(Key::Down);
    let agents_at = (app.everyone, app.agent);

    to_menu(&mut app);
    walk_the_menu_to(&mut app, Page::Options);
    app.press(Key::Enter);
    assert_eq!(app.cursor(), options_at, "the options page forgot its row");
    to_menu(&mut app);
    walk_the_menu_to(&mut app, Page::Agents);
    app.press(Key::Enter);
    assert_eq!(
        (app.everyone, app.agent),
        agents_at,
        "the list forgot its row"
    );
    app.press(Key::Enter);
    assert_eq!(app.cursor(), detail_at, "the detail page forgot its row");
}

#[test]
fn esc_walks_back_to_the_menu_rather_than_out() {
    // Somebody who opened the wrong page should not have to quit the tool.
    let mut app = setup();
    assert_eq!(app.page, Page::Agents);
    assert_eq!(app.press(Key::Esc), Action::None);
    assert_eq!(app.page, Page::Home);

    // The read-only pages return on any key, so nobody has to guess theirs —
    // but `q` still leaves, from everywhere.
    for page in [Page::Doctor, Page::Help] {
        let mut app = App::new(Config::default());
        app.page = page;
        app.press(Key::Char('x'));
        assert_eq!(app.page, Page::Home, "{page:?} trapped the cursor");

        let mut app = App::new(Config::default());
        app.page = page;
        assert_eq!(app.press(Key::Char('q')), Action::Quit);
    }
    // And from the menu.
    let mut app = App::new(Config::default());
    assert_eq!(app.press(Key::Esc), Action::Quit);
}

#[test]
fn every_panel_is_drawn_in_the_one_style_this_screen_has() {
    // how it looks.
    let source = crate::source_text::code_of(include_str!("../tui.rs"));
    let built_by_hand = source.matches("Block::default()").count();
    assert_eq!(
        built_by_hand, 1,
        "a panel is built outside `panel()`, so it will not match the others"
    );
    assert!(source.contains("BorderType::Rounded"));

    // Every list uses the same highlight, or the selected row means one thing
    // on one pane and another elsewhere.
    assert_eq!(
        source.matches("highlight_style(").count(),
        source.matches("List::new(").count(),
        "a list is drawn without the shared highlight"
    );
}

#[test]
fn the_landing_screen_is_a_mark_and_a_menu_with_no_frame_around_it() {
    let app = App::new(Config::default());
    let wide = paint(&app, 100, 30);

    // The header every page carries.
    assert!(wide.contains("ESTIGIA"), "no header");
    assert!(
        wide.contains("Menu"),
        "the header does not say where you are"
    );
    assert!(wide.contains("agents chosen"), "no context in the header");

    // The menu, its heading, and the version under it.
    assert!(wide.contains("ACTIONS"), "the menu has no heading");
    assert!(wide.contains("Agents") && wide.contains("Push guard"));
    assert!(
        wide.contains(crate::build_info::VERSION),
        "the version is not shown"
    );
    // `Quit` is on it: somebody who arrived at a full-screen program has no way
    // to know `q` works without being told.
    assert!(wide.contains("Quit"), "quitting is not offered");

    // The cursor is part of the line, not a highlight bar.
    assert!(wide.contains('\u{25b8}'), "no cursor mark");

    // No frame: a box here would be a second border straight under the
    // header's, for no gain.
    assert!(
        !wide.contains('\u{256d}') && !wide.contains('\u{250c}'),
        "the landing screen has a border"
    );

    // Narrow: the entries are why the screen exists and are always drawn, so
    // the mark is what goes.
    let narrow = paint(&app, 44, 14);
    assert!(
        narrow.contains("Agents"),
        "the menu went instead of the mark"
    );
    assert!(narrow.contains("Push guard"));
}

#[test]
fn config_edit_is_one_table_and_never_asks_a_question_it_was_already_told() {
    use super::app::Purpose;

    // `config edit --agent x` was handed its table on the command line. Walking
    // somebody through "which agents?" would take an answer and throw it away,
    // and an install step would offer to write agents this call cannot write.
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("claude-code is an adapter");
    let mut app = App::one_table(Some(adapter), Config::default());
    assert_eq!(app.page, Page::Detail);
    assert!(app.pinned());
    assert_eq!(app.current(), "claude-code");

    // Persisted Model routing is projected as target rows, and the model rows
    // are the only place a target is offered.
    assert!(!app.rows().contains(&Setting::Models));
    assert!(!app.model_targets().is_empty());
    let rows = app.rows().to_vec();
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Detail, "Enter left a screen with one table");
    assert_eq!(app.focus, Focus::Picking, "Enter did not open the row");
    app.press(Key::Esc);
    assert_eq!(app.focus, Focus::List);
    assert_eq!(app.rows(), rows);

    // An answer lands in the one table, for a row this agent owns. A
    // machine-wide row is not one agent's to answer: `config set --agent`
    // refuses it, and this screen does not offer it — the shared screen below
    // does.
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    assert_eq!(app.configs.len(), 1, "a second table appeared from nowhere");
    assert_eq!(
        Setting::Delivery.value_of(app.config()),
        "ask",
        "the answer did not land in the one table"
    );
    assert!(!app.rows().contains(&Setting::Tracker));
    assert!(!app.rows().contains(&Setting::Summary));

    // And Esc leaves rather than opening a menu this screen never came from.
    app.message = None;
    assert_eq!(app.press(Key::Esc), Action::None, "it warns once");
    assert_eq!(app.press(Key::Esc), Action::Quit);

    // The shared contract is read by every agent, so no one of them gets to say
    // a row in it is worth nothing.
    let shared = App::one_table(None, Config::default());
    assert_eq!(shared.purpose, Purpose::OneTable(None));
    for setting in SETTINGS {
        assert_eq!(shared.applies(*setting), crate::setup::Applies::Held);
    }
    // It still paints.
    assert!(paint(&shared, 100, 30).contains("CONTRACT"));
    assert!(paint(&shared, 100, 30).contains(Setting::Tracker.label()));
}

#[test]
fn an_arrow_changes_the_setting_under_the_cursor_with_nothing_to_type() {
    let mut app = options();
    walk_to(&mut app, Setting::Evidence);
    let answers = Setting::Evidence.answers();
    assert!(answers.closed, "this test is about a closed vocabulary");

    // Forward through every one of them, and round.
    let mut walked = vec![app.setting().value_of(app.config())];
    for _ in 1..=answers.choices.len() {
        app.press(Key::Right);
        walked.push(app.setting().value_of(app.config()));
        assert_eq!(app.focus, Focus::List, "an arrow opened something");
    }
    for choice in answers.choices {
        assert!(
            walked.contains(&(*choice).to_owned()),
            "{choice} unreachable"
        );
    }
    assert_eq!(
        walked.first(),
        walked.last(),
        "the answers do not come back round"
    );

    // And back the other way.
    let here = app.setting().value_of(app.config());
    app.press(Key::Left);
    let back = app.setting().value_of(app.config());
    assert_ne!(here, back);
    app.press(Key::Right);
    assert_eq!(app.setting().value_of(app.config()), here);
}

#[test]
fn a_row_with_no_agent_definitions_root_still_takes_an_answer() {
    // Codex can gate tool calls and has no agent-definitions root. No row is
    // inert any more — `Delegated workers` was the one such row and it is gone
    // with its effect folded into `Model routing` — so this row still takes an
    // answer rather than being closed.
    let codex = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "codex")
        .expect("the Codex adapter");
    assert!(!codex.has_agent_definitions());
    assert_eq!(
        codex.applies(Setting::Delivery),
        crate::setup::Applies::Held,
        "a rootless adapter stopped holding the rows its gate enforces"
    );
    let mut app = App::one_table(Some(codex), Config::default());
    walk_to(&mut app, Setting::Delivery);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");
}

#[test]
fn a_row_that_decides_something_here_takes_an_answer() {
    let mut app = options();
    walk_to(&mut app, Setting::Evidence);
    let before = app.setting().value_of(app.config());

    // Through the picker.
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");
    app.press(Key::Down);
    app.press(Key::Enter);
    let after = app.setting().value_of(app.config());
    assert_ne!(after, before, "an answer this row takes was not written");
    assert!(
        app.message.is_none(),
        "a row that decides something here was answered with a refusal: {:?}",
        app.message
    );

    // And through cycling.
    let stepped = app.setting().value_of(app.config());
    app.press(Key::Right);
    assert_ne!(
        app.setting().value_of(app.config()),
        stepped,
        "stepping a row this screen answers was refused"
    );
}

#[test]
fn evidence_standard_is_not_refused_on_the_repository_options_page() {
    // `App::new` opens with `agent: 0`, the neutral adapter — the one with no
    // agent-definitions root — and going straight to the Options page never
    // moves it. `Evidence` is `Scope::Everywhere`: the
    // gate renders and enforces the reserved reviewer's grant from the embedded
    // template on every adapter, whether or not that adapter has an
    // agent-definitions root at all. The row takes an answer here.
    let mut app = App::new(Config::default());
    to_options(&mut app);
    assert_eq!(app.adapter().slug, "agents", "not on the neutral adapter");
    walk_to(&mut app, Setting::Evidence);

    app.press(Key::Char(' '));
    assert_eq!(
        app.focus,
        Focus::Picking,
        "the repository's Evidence row was refused"
    );
}

#[test]
fn all_installed_agents_is_not_answered_from_the_last_agent_visited() {
    // Opening all chosen agents leaves `self.agent` on whichever adapter
    // the cursor last passed, which need not be ticked. `set()` writes this
    // page's answer to every ticked adapter, so the row must open here.
    let mut app = per_agent_with(&["claude-code", "codex"]);
    to_everyone(&mut app);
    assert!(app.everyone);
    app.agent = crate::setup::AGENTS.len() - 1;
    assert!(
        !app.agents[app.agent].1,
        "the stale cursor names a ticked agent"
    );

    walk_to(&mut app, Setting::Transitions);
    app.press(Key::Char(' '));
    assert_eq!(
        app.focus,
        Focus::Picking,
        "all chosen agents refused a row whose answer reaches every ticked adapter"
    );
}

#[test]
fn the_restore_key_restores_a_diverged_row() {
    let codex = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "codex")
        .expect("the Codex adapter");
    let mut app = App::one_table(Some(codex), Config::default());
    walk_to(&mut app, Setting::Delivery);

    // `set()` itself does not consult `applies()` — only the keys that reach
    // it do — so diverging from installed by hand gives `r` something to
    // restore, and a guard that did nothing would be caught changing it back.
    app.set(Setting::Delivery, "ask")
        .expect("a valid Delivery value");
    assert_ne!(
        app.setting().value_of(app.config()),
        Setting::Delivery.value_of(&app.installed[app.current()]),
        "the test did not actually diverge from installed"
    );

    app.press(Key::Char('r'));
    assert_eq!(
        app.setting().value_of(app.config()),
        Setting::Delivery.value_of(&app.installed[app.current()]),
        "`r` did not restore the row to what is installed"
    );
}

#[test]
fn a_row_reached_on_all_chosen_agents_takes_an_answer_for_a_rootless_selection() {
    // Ticking only codex — a rootless adapter — and opening all installed
    // agents must still open the row: no row is closed to editing any more,
    // and the write reaches the one adapter that is ticked.
    let mut app = per_agent_with(&["codex"]);
    to_everyone(&mut app);
    assert_eq!(app.chosen().len(), 1, "codex was not the only agent ticked");

    walk_to(&mut app, Setting::Transitions);
    app.press(Key::Char(' '));
    assert_eq!(
        app.focus,
        Focus::Picking,
        "a row reached on all chosen agents was not offered"
    );
}

#[test]
fn the_folder_keys_are_letters_on_a_row_that_takes_no_folder() {
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");

    let where_it_was = app.pick;
    app.press(Key::Char('n'));
    assert_ne!(
        app.focus,
        Focus::Naming,
        "`n` on a row that takes no folder asked for a folder name"
    );
    app.press(Key::Char('l'));
    app.press(Key::Char('h'));
    assert!(
        app.browsing.is_none(),
        "a folder walk was started on a row that takes no folder"
    );
    assert_eq!(app.pick, where_it_was, "a letter moved the picker's cursor");

    // The floor: on the row that *does* take a directory, the same key does
    // what it is for — or none of the assertions above would mean anything.
    let mut walking = options();
    walk_to(&mut walking, Setting::Worktree);
    walking.press(Key::Char(' '));
    assert_eq!(
        walking.focus,
        Focus::Picking,
        "the folder picker did not open"
    );
    walking.press(Key::Char('n'));
    assert_eq!(
        walking.focus,
        Focus::Naming,
        "`n` on a folder row does not offer to name one"
    );
}

#[test]
fn the_picker_shows_the_answers_and_a_way_past_them_when_there_is_one() {
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);

    // The one answer the list holds, and — because a tracker also takes
    // `github owner/name`, which no list can hold — somewhere to type. A
    // picker with no way to reach a value outside its own list is worse than
    // none.
    let entries = app.picker();
    for choice in Setting::Tracker.answers().choices {
        assert!(entries.iter().any(|entry| entry == choice), "{choice}");
    }
    assert_eq!(entries.last().map(String::as_str), Some(TYPE_IT));

    // It opens on what is set, so the answer already chosen is under the cursor.
    assert_eq!(entries[app.pick], app.setting().value_of(app.config()));

    // Backing out changes nothing, and does not leave the program.
    let before = app.setting().value_of(app.config());
    assert_eq!(app.press(Key::Char('q')), Action::None, "the picker quit");
    assert_eq!(app.focus, Focus::List);
    assert_eq!(app.setting().value_of(app.config()), before);

    // Choosing the one answer the list holds is not re-asserted here: with a
    // single tracker the default already is that answer, so an `answer(&mut
    // app, "github")` step here would assert an identity rather than a
    // transition. A picker actually moving between two list answers is
    // crossed on a setting whose list has more than one —
    // `an_answer_on_the_options_page_lands_in_every_agents_table` answers
    // `Setting::Evidence` with `"measuring"` against a `reading` default.

    // The way past the list reaches a value the list never had.
    answer(&mut app, "github asanabrial/estigia");
    assert!(matches!(
        &app.config().tracker,
        crate::config::Tracker::Github { repo: Some(_) }
    ));
    // Which then shows on the picker itself, rather than the list opening on
    // an answer nobody chose.
    app.press(Key::Char(' '));
    assert_eq!(
        app.picker()[app.pick],
        app.setting().value_of(app.config()),
        "the picker lost the typed value"
    );
}

#[test]
fn bottom_model_picker_tracks_the_rendered_judge_row_in_each_layout() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "judge");
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the model picker did not open");
    assert_bottom_model_picker_geometry(&app);

    for (width, height) in [(40, 10), (20, 6), (4, 3), (1, 1)] {
        let _ = paint(&app, width, height);
    }
}

#[test]
fn choosing_a_model_shows_the_mutation_in_its_row_under_each_layout() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "judge");

    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the model picker did not open");
    assert_bottom_model_picker_geometry(&app);

    choose_picker_entry(&mut app, "fable");
    assert_eq!(app.model_value("judge"), "fable");
    let lines = rows(&app, 80, 24);
    let (top, bottom, left, right) = panel_bounds(&lines, " MODELS ");
    let (_, assignment) = assignment_row(&lines, top, bottom, left, right, "judge");
    assert!(
        assignment.contains("fable") && assignment.contains('*'),
        "the judge mutation is not visible in its assignment row:\n{}",
        lines.join("\n")
    );
}

#[test]
fn space_on_the_agent_choice_still_toggles_the_agent_instead_of_opening_a_row() {
    let mut app = setup();
    app.press(Key::Down);
    let before = app.agents[app.agent].1;
    app.press(Key::Char(' '));
    assert_eq!(app.agents[app.agent].1, !before);
    assert_eq!(app.page, Page::Agents);
    assert_eq!(app.focus, Focus::List);
    assert!(app.modal.is_none());
}

#[test]
fn a_truncated_model_panel_shows_its_scrollbar_while_settings_are_focused() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Transitions);
    assert!(app.model_target_at_cursor().is_none());
    let painted = paint(&app, 80, 24);
    assert!(
        painted.contains('\u{250a}'),
        "the truncated, unfocused model panel looks complete:\n{painted}"
    );
}

#[test]
fn a_closed_row_offers_nowhere_to_type_a_value_it_would_refuse() {
    // Nothing outside a closed list is accepted, so a field on that row would
    // be a place to type a value that will be refused.
    let mut app = options();
    walk_to(&mut app, Setting::Integration);
    app.press(Key::Char(' '));
    assert!(
        !app.picker().iter().any(|entry| entry == TYPE_IT),
        "a closed row offers somewhere to type a value it will refuse"
    );

    // An open one does, because its answers are examples rather than the whole
    // set — and without the field there would be no way to say the rest.
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert!(
        app.picker().iter().any(|entry| entry == TYPE_IT),
        "an open row offers only the answers it happens to list"
    );
}

#[test]
fn the_read_only_pages_scroll_rather_than_hiding_their_own_bottom_half() {
    for page in [Page::Help, Page::Doctor] {
        let mut app = App::new(Config::default());
        app.page = page;
        app.report = Some(
            (1..=60)
                .map(|n| format!("line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let top = paint(&app, 80, 24);

        app.press(Key::Down);
        app.press(Key::Down);
        assert_eq!(app.page, page, "an arrow key left the page");
        assert_eq!(app.scroll, 2);
        let lower = paint(&app, 80, 24);
        assert_ne!(top, lower, "{page:?} did not move");

        // Far past the end stops at the end rather than scrolling the text out
        // of the window entirely.
        for _ in 0..500 {
            app.press(Key::Down);
        }
        let bottom = paint(&app, 80, 24);
        assert!(
            bottom.contains("the end"),
            "{page:?} does not say when it is at the bottom"
        );
        assert!(!bottom.trim().is_empty(), "{page:?} scrolled itself blank");

        // And anything else still returns, so nobody is stuck.
        app.press(Key::Char('x'));
        assert_eq!(app.page, Page::Home);
        assert_eq!(
            app.scroll, 0,
            "the next page opens where the last one ended"
        );
    }
}

fn scrolled_to_the_end(app: &mut App, width: u16, height: u16) -> String {
    for _ in 0..500 {
        app.press(Key::Down);
    }
    paint(app, width, height)
}

#[test]
fn help_scrolls_to_its_last_words_when_lines_wrap() {
    for (tongue, last) in [
        (Tongue::English, "changes nothing else."),
        (Tongue::Spanish, "más."),
    ] {
        let mut app = App::new(Config::default());
        app.tongue = tongue;
        app.page = Page::Help;
        let screen = scrolled_to_the_end(&mut app, 60, 20);
        assert!(
            screen.contains(last),
            "{tongue:?} Help cannot be scrolled to its end:\n{screen}"
        );
        assert!(screen.contains("the end") || screen.contains("el final"));
    }
}

#[test]
fn a_long_report_row_can_be_scrolled_to_its_end() {
    let mut app = App::new(Config::default());
    app.page = Page::Doctor;
    let row = format!("{} TAILMARK", "word ".repeat(40));
    app.report = Some(format!("one\ntwo\n{row}"));
    let before = paint(&app, 40, 9);
    assert!(
        before.contains("more below"),
        "wrapped overflow was not offered as scrolling:\n{before}"
    );
    let screen = scrolled_to_the_end(&mut app, 40, 9);
    assert!(
        screen.contains("TAILMARK"),
        "the row end is unreachable:\n{screen}"
    );
}

#[test]
fn text_that_fits_without_wrapping_offers_no_scrolling() {
    let mut app = App::new(Config::default());
    app.page = Page::Doctor;
    app.report = Some("one\ntwo\nthree".to_owned());
    let screen = paint(&app, 60, 12);
    assert!(!screen.contains("more below") && !screen.contains("the end"));
}

#[test]
fn all_installed_agents_with_nothing_ticked_does_not_open_and_says_why() {
    let mut app = setup();
    assert_eq!(app.press(Key::Enter), Action::None);
    assert_eq!(app.page, Page::Agents);
    assert!(app.rows().is_empty(), "it offered rows for nobody");
    assert_eq!(app.setting_at_cursor(), None);
    assert!(!app.dirty());

    let painted = paint(&app, 80, 24);
    assert!(
        painted.contains("choose at least one agent"),
        "it refused and showed nothing: {painted}"
    );
}

#[test]
fn a_warning_is_visible_on_whichever_page_raised_it() {
    // The panel that carries messages is skipped when a page has nothing to
    // say. A warning raised on such a page would then be a key that, as far as
    // the operator can tell, did nothing at all.
    let mut app = per_agent();
    app.set(Setting::Delivery, "ask").expect("accepted");
    for page in [Page::Home, Page::Agents, Page::Detail] {
        let mut app = app.clone();
        app.page = page;
        assert_eq!(app.press(Key::Char('q')), Action::None, "it left at once");
        let message = app.message.clone().expect("something was raised");
        let painted = paint(&app, 80, 24);
        assert!(
            painted.contains(&message),
            "{page:?} raised {message:?} and did not show it: {painted}"
        );
    }

    // The options page has the same detail panel and the same warning.
    let mut app = options();
    app.set(Setting::Evidence, "measuring").expect("accepted");
    assert_eq!(app.press(Key::Char('q')), Action::None, "it left at once");
    let message = app.message.clone().expect("something was raised");
    let painted = paint(&app, 80, 24);
    assert!(
        painted.contains(&message),
        "the options page raised {message:?} and did not show it: {painted}"
    );
}

#[test]
fn the_options_page_is_on_the_menu_and_holds_what_is_not_about_an_agent() {
    let mut app = options();
    assert_eq!(app.rows(), OPTIONS_SETTINGS);
    for setting in AGENT_SETTINGS {
        assert!(
            !app.rows().contains(setting),
            "{setting:?} differs by agent and is offered where there is no agent"
        );
    }
    // And an agent's page no longer offers them, or the same row could be answered twice
    // with two answers, of which only the later write survives.
    for setting in per_agent().rows() {
        assert!(
            !OPTIONS_SETTINGS.contains(setting),
            "{setting:?} is on an agent's page and on the options page"
        );
    }

    // It paints, and names the row under the cursor.
    let painted = paint(&app, 96, 30);
    assert!(
        painted.contains("Options"),
        "the header does not say where you are"
    );
    assert!(painted.contains(Setting::Tracker.label()));
    // And where the answer lands, which is the reason these rows are apart from
    // the per-agent ones: one answer, into every chosen agent's own table.
    assert!(
        painted.contains(&format!("each of the {} chosen agent", app.chosen().len())),
        "the page does not say where an answer here goes: {painted}"
    );
    let mut nobody = app.clone();
    nobody.agents.iter_mut().for_each(|(_, on)| *on = false);
    assert!(
        paint(&nobody, 96, 30).contains("nowhere to write"),
        "with nothing ticked the page offers rows and does not say they land nowhere"
    );
    // The keys it actually has, and not the ones it does not.
    assert!(painted.contains("s apply"));

    // `s` reviews, from here, without walking back to the agents.
    let mut asking = app.clone();
    assert_eq!(asking.press(Key::Char('s')), Action::None);
    assert!(asking.reviewing);
    assert_eq!(asking.press(Key::Enter), Action::Save);

    // Esc goes back to the menu it was opened from rather than out.
    assert_eq!(app.press(Key::Esc), Action::None);
    assert_eq!(app.page, Page::Home);

    // And it reopens on the row it was left on, with nothing half-open behind it.
    let mut app = options();
    app.press(Key::Down);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    let at = app.cursor();
    to_menu(&mut app);
    to_options(&mut app);
    assert_eq!(app.cursor(), at, "it forgot where the cursor was");
    assert_eq!(
        app.focus,
        Focus::List,
        "a picker was left open across visits"
    );
}

#[test]
fn a_list_longer_than_its_panel_says_there_is_more() {
    // A list that runs past the bottom looks like a list that ends there, and
    // the eleventh agent is then a row nobody knows to go looking for.
    let app = setup();
    assert!(
        app.agents.len() > 5,
        "this test needs more adapters than a short window holds"
    );
    let short = paint(&app, 80, 8);
    assert!(
        short.contains('\u{250a}') || short.contains('\u{2588}'),
        "a list taller than its panel is drawn as though it were not: {short}"
    );
    // And a list that fits carries no bar to explain.
    let tall = paint(&app, 80, 40);
    assert!(
        !tall.contains('\u{250a}'),
        "a list that fits was given a scrollbar"
    );
}

#[test]
fn a_rerun_tells_what_is_already_installed_from_what_is_about_to_be() {
    // A tick means two different things — *this is installed* and *install
    // this* — and a screen that draws them identically cannot answer the
    // question people rerun setup to ask, which is what they already have.
    let mut app = App::with_agents(Config::default(), &["claude-code"]);
    app.page = Page::Agents;
    assert_eq!(app.installed_agents, vec!["claude-code"]);

    // Tick one that was not installed.
    walk_agents_to(&mut app, "codex", Key::Down);
    app.press(Key::Char(' '));
    let painted = paint(&app, 96, 26);
    assert!(painted.contains("installed"), "{painted}");
    assert!(
        painted.contains("will be installed"),
        "a new tick reads exactly like one that was already there: {painted}"
    );

    // Unticking one that is installed says what that does — which is nothing.
    // Setup does not uninstall, and a bare empty box implies it might.
    walk_agents_to(&mut app, "claude-code", Key::Up);
    app.press(Key::Char(' '));
    assert!(
        paint(&app, 96, 26).contains("will be left alone"),
        "unticking an installed agent looks like removing it"
    );
    // And the list of what was installed does not move when ticks do.
    assert_eq!(app.installed_agents, vec!["claude-code"]);
}

#[test]
fn the_menu_says_when_the_agents_are_carrying_an_unsaved_answer() {
    let menu_row = |app: &App| {
        rows(app, 96, 26)
            .into_iter()
            .find(|row| row.contains("Agents"))
            .expect("the menu names the agents entry")
    };
    let mut app = App::with_agents(Config::default(), &["claude-code"]);
    assert!(!app.agents_touched(), "dirty before anything moved");
    assert!(!menu_row(&app).contains('\u{2022}'));

    // An agent ticked is a change to the agents.
    to_setup(&mut app);
    app.press(Key::Down);
    app.press(Key::Char(' '));
    assert!(app.agents_touched());
    to_menu(&mut app);
    assert!(menu_row(&app).contains('\u{2022}'), "no mark on the menu");

    // So is an agent's row moved.
    let mut app = App::with_agents(Config::default(), &["claude-code"]);
    app.set(Setting::Delivery, "ask").expect("accepted");
    assert!(app.agents_touched());
    assert!(menu_row(&app).contains('\u{2022}'));

    // A repository row is not the agents' — it is on the options page — and
    // moving one must not mark an entry that does not hold it.
    let mut app = App::with_agents(Config::default(), &["claude-code"]);
    assert!(!app.options_touched(), "nothing repository-wide has moved");
    app.set(Setting::Evidence, "measuring").expect("accepted");
    assert!(app.options_touched());
    assert!(
        !app.agents_touched(),
        "the agents claim a repository row they do not hold"
    );
}

#[test]
fn the_menu_says_when_the_options_page_is_carrying_an_unsaved_answer() {
    let mut app = options();
    assert!(!app.options_touched());

    walk_to(&mut app, Setting::Evidence);
    answer(&mut app, "measuring");
    assert!(app.options_touched());

    to_menu(&mut app);
    let row = rows(&app, 96, 26)
        .into_iter()
        .find(|row| row.contains("Options"))
        .expect("the menu names the options entry");
    assert!(
        row.contains('\u{2022}'),
        "an unsaved repository answer is invisible from the menu: {row}"
    );

    // And writing it takes the mark away, because the answer is on disk now.
    // The verdict is dismissed first: it is drawn over the middle of whatever
    // is behind it, so a menu read through it is a menu half of which is a box.
    acknowledge(&mut app, "wrote it");
    app.press(Key::Char('x'));
    assert_eq!(app.modal, None);
    assert!(!app.options_touched());
    let row = rows(&app, 96, 26)
        .into_iter()
        .find(|row| row.contains("Options"))
        .expect("the menu names the options entry");
    assert!(
        !row.contains('\u{2022}'),
        "the mark outlived the write it was about: {row}"
    );
}

#[test]
fn a_digit_is_a_digit_wherever_it_is_typed() {
    // Nothing numbers the pages: a digit does nothing on a list, and is text in a field.
    let mut app = setup();
    for digit in '1'..='9' {
        assert_eq!(app.press(Key::Char(digit)), Action::None);
        assert_eq!(app.page, Page::Agents, "{digit} went somewhere");
    }

    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");
    open_the_field(&mut app);
    for _ in 0..64 {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "gpt-4");
    assert_eq!(app.draft, "gpt-4", "a digit escaped the field");
    assert_eq!(app.page, Page::Detail);
}

#[test]
fn a_row_this_repository_already_customises_is_marked_apart_from_one_just_moved() {
    // Two different facts. Without the second, a rerun cannot tell a row this
    // repository deliberately set from one nobody has ever touched.
    let mut installed = std::collections::BTreeMap::new();
    installed.insert(
        "claude-code",
        Config {
            change_size: 400,
            ..Config::default()
        },
    );
    let mut app = App::with_configs(&installed, &["claude-code"]);
    app.page = Page::Options;
    assert!(!app.changed(Setting::ChangeSize), "nothing has moved yet");

    let painted = paint(&app, 96, 40);
    assert!(
        painted.contains('\u{00b7}'),
        "an installed row that differs from the default is drawn as untouched: {painted}"
    );
    assert!(
        !painted.contains('*'),
        "nothing has moved and the screen says something has"
    );

    // Now move one, and the two marks are both on screen and different.
    walk_to(&mut app, Setting::Integration);
    answer(&mut app, "trunk");
    let painted = paint(&app, 96, 40);
    assert!(
        painted.contains('*') && painted.contains('\u{00b7}'),
        "{painted}"
    );
}

#[test]
fn restoring_a_repository_row_puts_back_the_value_that_was_on_screen() {
    // A repository-wide row shows one agent's answer. Restoring from a
    // different agent's table would put back a value that was never on screen —
    // and with two agents installed differently, that is the wrong one.
    let mut installed = std::collections::BTreeMap::new();
    installed.insert(
        "claude-code",
        Config {
            integration: crate::config::Integration::Trunk,
            ..Config::default()
        },
    );
    installed.insert("codex", Config::default());
    let mut app = App::with_configs(&installed, &["claude-code"]);
    app.page = Page::Options;
    walk_to(&mut app, Setting::Integration);
    let shown = app.shown_value(Setting::Integration);
    assert_eq!(shown, "trunk", "the chosen agent's answer is what shows");

    answer(&mut app, "branch");
    assert!(app.changed(Setting::Integration));
    app.press(Key::Char('r'));
    assert_eq!(
        app.shown_value(Setting::Integration),
        shown,
        "restore put back a value that was never on screen"
    );
    assert!(!app.changed(Setting::Integration));
}

/// One frame, row by row, so a test can say what shared a line with what.
fn rows(app: &App, width: u16, height: u16) -> Vec<String> {
    let mut app = app.clone();
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test terminal");
    terminal
        .draw(|frame| super::draw(frame, &mut app))
        .expect("the frame paints");
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|row| {
            (0..width)
                .map(|column| buffer[(column, row)].symbol().to_owned())
                .collect()
        })
        .collect()
}

fn char_position(line: &str, needle: &str) -> Option<usize> {
    let line = line.chars().collect::<Vec<_>>();
    let needle = needle.chars().collect::<Vec<_>>();
    line.windows(needle.len())
        .position(|window| window == needle)
}

fn panel_bounds(lines: &[String], title: &str) -> (usize, usize, usize, usize) {
    let (top, title_at) = lines
        .iter()
        .enumerate()
        .find_map(|(row, line)| char_position(line, title).map(|column| (row, column)))
        .unwrap_or_else(|| panic!("panel {title:?} is not rendered:\n{}", lines.join("\n")));
    let top_chars = lines[top].chars().collect::<Vec<_>>();
    let left = top_chars[..title_at]
        .iter()
        .rposition(|character| *character == '\u{256d}')
        .expect("the panel has a left corner");
    let right = title_at
        + top_chars[title_at..]
            .iter()
            .position(|character| *character == '\u{256e}')
            .expect("the panel has a right corner");
    let bottom = (top + 1..lines.len())
        .find(|row| {
            let chars = lines[*row].chars().collect::<Vec<_>>();
            chars.get(left) == Some(&'\u{2570}') && chars.get(right) == Some(&'\u{256f}')
        })
        .expect("the panel has a matching bottom border");
    (top, bottom, left, right)
}

fn assignment_row(
    lines: &[String],
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
    target: &str,
) -> (usize, String) {
    (top + 1..bottom)
        .filter_map(|row| {
            let chars = lines[row].chars().collect::<Vec<_>>();
            let cell = chars[left + 1..right].iter().collect::<String>();
            (cell.contains(target) && !cell.contains("Models for")).then_some((row, cell))
        })
        .next()
        .unwrap_or_else(|| {
            panic!(
                "{target:?} has no assignment row inside the model panel:\n{}",
                lines.join("\n")
            )
        })
}

fn assert_bottom_model_picker_geometry(app: &App) {
    let title = " MODELS ";
    let mut without_picker = app.clone();
    without_picker.focus = Focus::List;
    let panel_frame = rows(&without_picker, 80, 24);
    let (_, _, model_left, model_right) = panel_bounds(&panel_frame, title);

    let lines = rows(app, 80, 24);
    let painted = lines.join("\n");
    let model_top = panel_frame
        .iter()
        .position(|line| char_position(line, title).is_some())
        .expect("the model panel title");
    let model_bottom = (model_top + 1..panel_frame.len())
        .find(|row| {
            let chars = panel_frame[*row].chars().collect::<Vec<_>>();
            chars.get(model_left) == Some(&'\u{2570}')
                && chars.get(model_right) == Some(&'\u{256f}')
        })
        .expect("the model panel bottom");
    let (judge, assignment) = assignment_row(
        &lines,
        model_top,
        model_bottom,
        model_left,
        model_right,
        "judge",
    );
    assert!(
        assignment.contains("inherit") || assignment.contains("fable"),
        "judge was found outside its assignment row:\n{painted}"
    );
    assert_eq!(
        lines[judge].chars().nth(model_right - 1),
        Some('\u{2588}'),
        "the scrollbar thumb does not point at bottom-row judge:\n{painted}"
    );

    let (picker_top, picker_bottom, picker_left, picker_right) =
        panel_bounds(&lines, " Models for judge ");
    assert!(
        picker_left >= model_left
            && picker_right <= model_right
            && picker_top >= model_top
            && picker_bottom <= model_bottom,
        "the judge picker is not clamped inside the model panel:\n{painted}"
    );
    assert!(
        picker_top == judge + 1 || picker_bottom + 1 == judge,
        "the judge picker is not adjacent to its assignment row:\n{painted}"
    );
}

#[test]
fn a_narrow_window_gives_up_the_label_before_it_gives_up_the_answer() {
    // Every row is a question and its answer, and only the answer changes
    // between runs — the question is spelled out again in the detail panel
    // below either way. Padded to a fixed column the label took the whole row:
    // at 44 columns the per-agent step listed its settings with nothing beside
    // any of them, which is the one thing that step exists to show.
    for (name, mut app) in [("per agent", per_agent()), ("options", options())] {
        walk_onto_a_setting(&mut app);
        let painted = rows(&app, 44, 16);
        let value = app.shown_value(app.setting());
        let head: String = app.setting().label().chars().take(6).collect();
        let row = painted
            .iter()
            .find(|row| row.contains(&head) && row.contains(&value))
            .unwrap_or_else(|| {
                panic!(
                    "{name}: no setting row carries {head} with {value}:\n{}",
                    painted.join("\n")
                )
            });
        // And not run into it. A label cut to the column exactly touched the
        // value, and the pair read as one word.
        assert!(
            !row.contains(&format!("\u{2026}{value}")),
            "{name}: the cut label touches the answer: {row}"
        );
    }
}

#[test]
fn no_window_a_terminal_can_be_makes_the_screen_panic() {
    // frame that panics takes the tool down mid-setup, so this walks every page
    // across the whole cramped end and samples the rest.
    let mut screens = vec![
        ("menu", App::new(Config::default())),
        ("per agent", per_agent()),
        ("options", options()),
    ];
    let mut picking = per_agent();
    picking.press(Key::Char(' '));
    assert_eq!(picking.focus, Focus::Picking);
    screens.push(("picking", picking));

    let mut agents = per_agent();
    agents.press(Key::Esc);
    assert_eq!(agents.page, Page::Agents);
    screens.push(("agents", agents));

    let mut reviewing = per_agent();
    reviewing.press(Key::Char('s'));
    assert!(reviewing.reviewing);
    screens.push(("review", reviewing));

    for page in [Page::Doctor, Page::Help] {
        let mut app = App::new(Config::default());
        app.page = page;
        screens.push(("text", app));
    }

    let mut modal = App::new(Config::default());
    modal.install_failed("MODALMARK".to_owned(), "long detail ".repeat(60));
    let mut removal = App::new(Config::default());
    removal.removal_asked(
        crate::tui::Removal::Agent("claude-code"),
        (0..6)
            .map(|n| format!("/very/long/path/{}/REMOVALMARK{n}", "segment/".repeat(12)))
            .collect(),
    );
    let root = tempfile::tempdir().expect("a temporary root");
    let mut browsing = options();
    walk_to(&mut browsing, Setting::Worktree);
    browsing.press(Key::Char(' '));
    assert_eq!(browsing.focus, Focus::Picking);
    browsing.browsing = Some(
        root.path()
            .join("a".repeat(30))
            .join("b".repeat(30))
            .join("目录目录目录目录目录目录"),
    );
    browsing.folders = vec!["文件夹文件夹".to_owned()];
    let mut report = App::new(Config::default());
    report.report = Some(
        (0..40)
            .map(|n| format!("REPORTMARK{n}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    for (mark, app) in [
        ("MODALMARK", &modal),
        ("REMOVALMARK0", &removal),
        ("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", &browsing),
        ("REPORTMARK0", &report),
    ] {
        let screen = paint(app, 80, 24);
        assert!(screen.contains(mark), "{mark} is not on screen:\n{screen}");
    }
    screens.push(("modal", modal));
    screens.push(("removal", removal));
    screens.push(("browsing", browsing));
    screens.push(("report", report));

    let widths: Vec<u16> = (1..=24).chain([30, 44, 60, 80, 110]).collect();
    let heights: Vec<u16> = (1..=14).chain([18, 24, 34]).collect();
    for (name, app) in &screens {
        for width in &widths {
            for height in &heights {
                let _ = (name, paint(app, *width, *height));
            }
        }
    }
}

#[test]
fn the_footer_drops_whole_keys_rather_than_cutting_the_last_one_off() {
    for width in [44u16, 60, 80, 110] {
        let app = per_agent();
        let painted = rows(&app, width, 16);
        let footer = painted.last().expect("a footer row").trim().to_owned();
        assert!(
            footer.contains("s apply"),
            "at {width} columns the footer never says how to apply: {footer}"
        );
        assert!(
            !footer.contains("\u{2026}"),
            "at {width} columns a key is cut in half: {footer}"
        );
        assert!(
            footer.chars().count() <= usize::from(width),
            "the footer is wider than the window: {footer}"
        );
    }

    // And the same on the read-only pages, which have their own footer.
    let mut app = App::new(Config::default());
    app.page = Page::Help;
    let painted = rows(&app, 40, 16);
    let footer = painted.last().expect("a footer row").trim().to_owned();
    assert!(
        footer.contains("q quit"),
        "the help page never says how to leave: {footer}"
    );
}

/// Every layout fault found so far was found by painting the screen small and
/// reading it, not by reading the code that paints it.
#[test]
#[ignore = "a human reads the output; nothing is asserted"]
fn look() {
    let mut review = per_agent();
    review.press(Key::Char('s'));
    let mut doctor = App::new(Config::default());
    doctor.page = Page::Help;
    let mut spanish_menu = App::new(Config::default());
    spanish_menu.tongue = crate::tui::words::Tongue::Spanish;
    let mut spanish_options = options();
    spanish_options.tongue = crate::tui::words::Tongue::Spanish;
    for (name, app, width, height) in [
        ("menu", App::new(Config::default()), 60u16, 16u16),
        ("per agent", per_agent(), 44, 14),
        ("per agent", per_agent(), 80, 24),
        ("options", options(), 50, 14),
        ("menu es", spanish_menu, 60, 16),
        ("options es", spanish_options, 76, 22),
        ("review", review, 60, 20),
        ("help", doctor, 40, 12),
    ] {
        println!("=== {name} {width}x{height} ===");
        for row in rows(&app, width, height) {
            println!("|{}|", row.trim_end());
        }
    }
}

#[test]
fn the_answers_the_journey_takes_are_the_answers_it_would_write() {
    let mut app = per_agent();
    let chosen: Vec<&'static str> = app.chosen().iter().map(|a| a.slug).collect();
    assert!(
        chosen.len() > 1,
        "one agent cannot show a value going astray"
    );
    let here = app.current();
    let elsewhere = chosen
        .iter()
        .find(|slug| **slug != here)
        .expect("a second ticked agent");

    // A row that differs by agent, set on the agent under the cursor. `Profile`
    // is left out: it expands into the two authorisations, so a single-row
    // edit moving two is the overlap its own test pins, not this one's.
    let mine = *AGENT_SETTINGS
        .iter()
        .filter(|setting| **setting != Setting::Profile)
        .find(|setting| another_value(&app, **setting).is_some())
        .expect("some per-agent row offers a second answer");
    let mine_was = app.shown_value(mine);
    let mine_now = another_value(&app, mine).expect("a second answer");
    walk_to(&mut app, mine);
    answer(&mut app, &mine_now);

    to_options(&mut app);
    let ours = *OPTIONS_SETTINGS
        .iter()
        .find(|setting| another_value(&app, **setting).is_some())
        .expect("some repository row offers a second answer");
    let ours_now = another_value(&app, ours).expect("a second answer");
    walk_to(&mut app, ours);
    answer(&mut app, &ours_now);

    // Offered for saving from the options page it was answered on, without
    // walking back to the agents.
    let mut options_page = app.clone();
    assert_eq!(options_page.press(Key::Char('s')), Action::None);
    assert_eq!(
        options_page.press(Key::Enter),
        Action::Save,
        "the options page cannot write what it took"
    );

    // And from the agents list, which writes the same tables.
    to_setup(&mut app);
    assert_eq!(app.press(Key::Char('s')), Action::None);
    assert_eq!(
        app.press(Key::Enter),
        Action::Save,
        "the journey cannot end"
    );

    // Now the answers, in the tables that would be written.
    assert_eq!(
        mine.value_of(&app.configs[here]),
        mine_now,
        "{mine:?} was set for {here} and did not land there"
    );
    assert_eq!(
        mine.value_of(&app.configs[elsewhere]),
        mine_was,
        "{mine:?} was set for {here} and moved {elsewhere} too"
    );
    for slug in &chosen {
        assert_eq!(
            ours.value_of(&app.configs[*slug]),
            ours_now,
            "{ours:?} belongs to the repository and did not reach {slug}"
        );
    }

    // And nothing else moved, on either agent. `Profile` is derived over the
    // two authorisations, so it moves whenever one of them is edited; the
    // question here is whether a stored row moved on its own.
    for setting in SETTINGS {
        if *setting == mine || *setting == ours || *setting == Setting::Profile {
            continue;
        }
        for slug in &chosen {
            assert_eq!(
                setting.value_of(&app.configs[*slug]),
                setting.value_of(&app.installed[*slug]),
                "{setting:?} moved on {slug} and nobody asked it to"
            );
        }
    }
}

#[test]
fn installing_says_whether_it_worked_and_says_it_over_the_screen() {
    let mut app = per_agent();
    let page = app.page;
    let selected = app.selected;
    acknowledge(&mut app, "installed into 2 agents, 34 files");

    let modal = app.modal.clone().expect("no verdict was shown");
    assert!(modal.ok, "a run that landed was reported as a failure");
    assert_eq!(modal.title, "installed into 2 agents, 34 files");
    assert_eq!(modal.detail, None, "a run that worked needs no way out");
    assert_eq!(app.page, page, "the verdict moved the screen under it");

    // Any key closes it, and the page and the cursor are the ones the save was asked from.
    assert_eq!(app.press(Key::Down), Action::None);
    assert_eq!(
        app.modal, None,
        "the verdict outlived the key that closed it"
    );
    assert_eq!(app.page, page, "a finished install left the page");
    assert_eq!(
        app.selected, selected,
        "the key that closed it moved the cursor"
    );

    // A refusal is the same box, saying the other thing — and it carries the
    // way out, which is the half of a refusal that a bare "it failed" loses.
    let mut app = per_agent();
    app.install_failed(
        "nothing was written (skill-root-unknown)".to_owned(),
        "[operator-knowledge] where this agent reads skills".to_owned(),
    );
    let modal = app.modal.clone().expect("no verdict was shown");
    assert!(!modal.ok, "a refusal was reported as a success");
    assert!(
        modal.title.contains("skill-root-unknown"),
        "the code did not reach the box"
    );
    assert!(
        modal
            .detail
            .is_some_and(|detail| detail.contains("operator-knowledge")),
        "the way out did not reach the box"
    );

    // And it leaves them where the thing to fix is.
    assert_eq!(app.press(Key::Char('x')), Action::None);
    assert_eq!(app.modal, None, "the verdict outlived the key");
    assert_eq!(app.page, page, "a refusal sent them away from the fix");
    assert_eq!(
        app.selected, selected,
        "a refusal moved the cursor under it"
    );
}

#[test]
fn a_successful_retry_replaces_the_pending_refusal() {
    let mut app = per_agent();
    app.everyone = true;
    app.set(Setting::Delivery, "ask")
        .expect("Delivery authorisation is accepted");
    let mut attempts = 0;
    let mut installer = |plan: &super::Plan| {
        attempts += 1;
        if attempts == 1 {
            return Err(crate::skill::no_skill_root().into());
        }
        let read_back: std::collections::BTreeMap<&'static str, Config> = plan
            .agents
            .iter()
            .map(|adapter| (adapter.slug, plan.rows[adapter.slug].clone()))
            .collect();
        let contract_read_back = std::collections::BTreeMap::new();
        Ok(super::InstallReceipt {
            summary: "installed after retry".to_owned(),
            agent_read_back: read_back.clone(),
            local_read_back: std::collections::BTreeMap::new(),
            unlayered_read_back: read_back.clone(),
            acknowledged: read_back
                .keys()
                .map(|slug| (*slug, crate::config::SETTINGS.to_vec()))
                .collect(),
            completed: read_back.keys().copied().collect(),
            read_back,
            contract_read_back,
            repository: None,
            repository_settings: Vec::new(),
        })
    };
    let mut refused = None;

    super::install_from_screen(&mut app, &mut installer, &mut refused);
    assert!(refused.is_some(), "the first refusal was not retained");
    assert!(
        app.modal.as_ref().is_some_and(|modal| !modal.ok),
        "the first attempt was not shown as a failure"
    );
    app.press(Key::Char('x'));

    super::install_from_screen(&mut app, &mut installer, &mut refused);
    assert_eq!(attempts, 2);
    assert!(
        refused.is_none(),
        "the successful verified retry left the obsolete refusal pending"
    );
    assert!(
        app.modal
            .as_ref()
            .is_some_and(|modal| modal.ok && modal.title == "installed after retry"),
        "the final summary does not report the successful retry"
    );
    let final_result: Result<(), crate::outcome::Refusal> = match refused {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    };
    assert!(
        final_result.is_ok(),
        "the obsolete refusal escaped as the final result"
    );
}

#[test]
fn partial_read_back_advances_only_proven_settings_and_leaves_the_rest_dirty() {
    let mut app = per_agent();
    app.everyone = true;
    app.set(Setting::Delivery, "ask")
        .expect("Delivery is accepted");
    app.set(Setting::Models, "orchestrate=provider/planner")
        .expect("the model is accepted");
    let selected = app.chosen()[0].slug;
    let proven = app.configs[selected].clone();
    let mut receipt = super::InstallReceipt::empty(String::new());
    receipt.read_back.insert(selected, proven.clone());
    receipt.unlayered_read_back.insert(selected, proven.clone());
    receipt
        .contract_read_back
        .insert(selected, Config::default());
    receipt.agent_read_back.insert(selected, proven);
    receipt
        .acknowledged
        .insert(selected, vec![Setting::Delivery]);

    app.installed_partially(receipt);

    assert_eq!(
        Setting::Delivery.value_of(&app.installed[selected]),
        "ask",
        "the proven setting remained dirty"
    );
    assert_eq!(
        app.installed[selected].models.for_target("orchestrate"),
        None,
        "an unproven setting advanced with its neighbour"
    );
    assert!(
        app.dirty(),
        "the unproven model edit no longer warns on quit"
    );
}

#[test]
fn read_back_without_lifecycle_completion_never_marks_an_agent_installed() {
    let selected = "claude-code";
    let mut app = App::with_configs(
        &std::collections::BTreeMap::from([(selected, Config::default())]),
        &[],
    );
    let mut receipt = super::InstallReceipt::empty(String::new());
    receipt.read_back.insert(selected, Config::default());
    receipt
        .unlayered_read_back
        .insert(selected, Config::default());
    receipt.acknowledged.insert(selected, Vec::new());

    app.installed_partially(receipt);

    assert!(
        app.installed_agents.is_empty(),
        "a readable table was mistaken for a completed adapter lifecycle"
    );
}

#[test]
fn partial_lifecycle_evidence_completes_only_the_first_agent_before_and_after_second_writes() {
    for boundary in [
        crate::setup::SetupFailureBoundary::BeforeSkill,
        crate::setup::SetupFailureBoundary::AfterDirective,
    ] {
        let home = tempfile::tempdir().expect("a temporary home");
        let options = crate::setup::SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            ..crate::setup::SetupOptions::default()
        };
        let first = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
        let second = crate::setup::find_agent("cursor").expect("Cursor is an adapter");
        let mut selected = Config::default();
        Setting::Delivery
            .apply(&mut selected, "ask")
            .expect("Delivery is accepted");
        let opened = std::collections::BTreeMap::from([
            (first.slug, Config::default()),
            (second.slug, Config::default()),
        ]);
        let plan = super::Plan {
            agents: vec![first, second],
            opened: opened.clone(),
            rows: std::collections::BTreeMap::from([
                (first.slug, selected.clone()),
                (second.slug, selected.clone()),
            ]),
            repository: std::path::PathBuf::new(),
        };
        crate::setup::inject_setup_failure(second.slug, boundary);

        let failure = crate::cli::install_planned(&plan, &options, false)
            .expect_err("the injected second-adapter failure was reported as success");

        assert_eq!(
            failure.receipt.completed,
            std::collections::BTreeSet::from([first.slug]),
            "{boundary:?} confused readable config with lifecycle completion"
        );
        assert!(
            failure.receipt.acknowledged.values().all(Vec::is_empty),
            "{boundary:?} acknowledged an override that setup never reached"
        );
        let mut app = App::with_configs(&opened, &[]);
        app.configs.insert(first.slug, selected.clone());
        app.configs.insert(second.slug, selected.clone());
        app.installed_partially(*failure.receipt);
        assert_eq!(app.installed_agents, vec![first.slug]);
        assert!(app.dirty(), "{boundary:?} cleared unproven setting edits");
    }
}

#[test]
fn a_finished_install_leaves_nothing_the_screen_still_calls_unsaved() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    assert!(app.dirty(), "the fixture changed nothing");

    let mut failed = app.clone();
    failed.install_failed("no".to_owned(), "because".to_owned());
    assert!(
        failed.dirty(),
        "a refusal wrote nothing and the screen called the table saved"
    );

    acknowledge(&mut app, "wrote it");
    assert!(
        !app.dirty(),
        "the screen still calls the written table unsaved"
    );
    assert!(
        !app.agents_touched(),
        "the menu still marks the agents unsaved after the install"
    );
    assert!(
        !app.options_touched(),
        "the menu still marks the options page unsaved after the install"
    );
}

/// it without a warning.
#[test]
fn save_acknowledges_only_agents_persisted_and_read_back() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapters = ["claude-code", "codex"].map(|slug| {
        crate::setup::AGENTS
            .iter()
            .find(|adapter| adapter.slug == slug)
            .unwrap_or_else(|| panic!("{slug} is an agent this build knows"))
    });
    let mut installed = std::collections::BTreeMap::new();
    for adapter in adapters {
        crate::setup::setup(adapter, &Config::default(), &options)
            .unwrap_or_else(|refusal| panic!("{} installs: {refusal}", adapter.slug));
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("the installed agent resolves")
            .skill_root;
        installed.insert(
            adapter.slug,
            crate::skill::installed_config_for(&root, Some(adapter.slug))
                .expect("the installed table reads back"),
        );
    }

    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    app.bare = installed.clone();
    app.everyone = true;
    app.set(Setting::Delivery, "ask")
        .expect("the shared delivery answer is accepted");
    app.set(Setting::Models, "orchestrate=provider/planner")
        .expect("the shared model answer is accepted");

    let codex = app
        .agents
        .iter_mut()
        .find(|(adapter, _)| adapter.slug == "codex")
        .expect("codex is on the agent page");
    codex.1 = false;
    let plan = app.plan();
    assert_eq!(
        plan.agents
            .iter()
            .map(|adapter| adapter.slug)
            .collect::<Vec<_>>(),
        vec!["claude-code"],
        "the deselected agent is still on its way to the writer"
    );

    let (receipt, report) =
        crate::cli::install_planned(&plan, &options, false).expect("the selected agent installs");
    assert_eq!(
        receipt.read_back.keys().copied().collect::<Vec<_>>(),
        vec!["claude-code"],
        "the writer acknowledged an agent it did not read back"
    );
    app.installed_now(receipt);

    let read = |slug| {
        let adapter = crate::setup::AGENTS
            .iter()
            .find(|adapter| adapter.slug == slug)
            .expect("the named adapter");
        let root = crate::setup::resolve_paths(adapter, &options)
            .expect("the installed agent resolves")
            .skill_root;
        crate::skill::installed_config_for(&root, Some(slug))
            .expect("the installed table reads back")
    };
    assert_eq!(
        Setting::Delivery.value_of(&read("claude-code")),
        "ask",
        "the selected agent did not persist"
    );
    assert_eq!(
        Setting::Delivery.value_of(&read("codex")),
        "auto",
        "the deselected agent reached disk"
    );
    assert_eq!(
        Setting::Delivery.value_of(&app.configs["codex"]),
        "ask",
        "deselecting discarded the in-memory edit"
    );
    assert_eq!(
        Setting::Delivery.value_of(&app.installed["codex"]),
        "auto",
        "the screen acknowledged a table the writer did not persist"
    );
    assert!(app.dirty(), "the deselected edit is reported as installed");
    app.press(Key::Char('x'));
    assert_eq!(app.press(Key::Char('q')), Action::None);
    assert!(
        app.message
            .as_deref()
            .is_some_and(|message| message.contains("unsaved changes")),
        "quit did not warn about the deselected edit"
    );

    let claude_root = crate::setup::resolve_paths(adapters[0], &options)
        .expect("Claude Code resolves")
        .skill_root;
    let contract = std::fs::read_to_string(claude_root.join(crate::skill::CONTRACT))
        .expect("the installed contract reads");
    assert_eq!(
        contract.matches("| Model routing |").count(),
        1,
        "the persisted model route is not exactly one canonical setting row"
    );
    assert!(
        !contract
            .lines()
            .any(|line| line.starts_with("| orchestrate |")),
        "a projected phase escaped as a synthetic persisted setting"
    );
    assert_eq!(
        report
            .lines()
            .filter(|line| line.contains("estigia config set \"Model routing\""))
            .count(),
        1,
        "the report did not print exactly one reproducible model command"
    );
}

#[test]
fn two_saves_keep_delivery_when_a_model_is_saved_second() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let initial = Config {
        models: crate::config::ModelRouting::parse("reviewer=hidden/model")
            .expect("a stored assignment"),
        ..Config::default()
    };
    crate::setup::setup(adapter, &initial, &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    let installed = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the initial contract reads");
    let mut app = App::with_configs(
        &std::collections::BTreeMap::from([(adapter.slug, installed.clone())]),
        &[adapter.slug],
    );
    app.bare.insert(adapter.slug, installed);
    app.everyone = true;

    app.set(Setting::Delivery, "ask")
        .expect("Delivery is accepted");
    let plan = app.plan();
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the first save persists Delivery");
    app.installed_now(receipt);

    app.set(
        Setting::Models,
        "reviewer=hidden/model, orchestrate=provider/planner",
    )
    .expect("the visible model is accepted");
    let plan = app.plan();
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the second save persists the model");
    app.installed_now(receipt);

    let final_config = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the final contract reads");
    assert_eq!(
        Setting::Delivery.value_of(&final_config),
        "ask",
        "the second save reconstructed the contract from the pre-session bare table"
    );
    assert_eq!(
        final_config.models.for_target("orchestrate"),
        Some("provider/planner")
    );
    assert_eq!(
        final_config.models.for_target("reviewer"),
        Some("hidden/model"),
        "an assignment was lost while composing the saves"
    );
}

#[test]
fn two_saves_keep_the_model_when_delivery_is_saved_second() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let initial = Config {
        models: crate::config::ModelRouting::parse("reviewer=hidden/model")
            .expect("a stored assignment"),
        ..Config::default()
    };
    crate::setup::setup(adapter, &initial, &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    let installed = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the initial contract reads");
    let mut app = App::with_configs(
        &std::collections::BTreeMap::from([(adapter.slug, installed.clone())]),
        &[adapter.slug],
    );
    app.bare.insert(adapter.slug, installed);
    app.everyone = true;

    app.set(
        Setting::Models,
        "reviewer=hidden/model, orchestrate=provider/planner",
    )
    .expect("the visible model is accepted");
    let plan = app.plan();
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the first save persists the model");
    app.installed_now(receipt);

    app.set(Setting::Delivery, "ask")
        .expect("Delivery is accepted");
    let plan = app.plan();
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the second save persists Delivery");
    app.installed_now(receipt);

    let final_config = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the final contract reads");
    assert_eq!(
        final_config.models.for_target("orchestrate"),
        Some("provider/planner"),
        "the second save reconstructed the contract from the pre-session bare table"
    );
    assert_eq!(Setting::Delivery.value_of(&final_config), "ask");
    assert_eq!(
        final_config.models.for_target("reviewer"),
        Some("hidden/model"),
        "an assignment was lost while composing the saves"
    );
}

#[test]
fn interactive_dry_run_writes_nothing_and_acknowledges_nothing() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repository = tempfile::tempdir().expect("a repository");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "opencode")
        .expect("the OpenCode adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let override_path = crate::skill::agent_override(&root, adapter.slug);
    crate::setup::write_agent_configuration_wholly(
        &override_path,
        adapter.slug,
        &Config::default(),
    )
    .expect("the initial agent override");
    let repository_path = crate::skill::repository_config_path(repository.path());
    crate::setup::write_repository_configuration(
        &repository_path,
        &Config::default(),
        EVERYWHERE_SETTINGS,
    )
    .expect("the initial repository table");

    let contract_path = root.join(crate::skill::CONTRACT);
    let before_contract = std::fs::read(&contract_path).expect("the contract reads");
    let before_override = std::fs::read(&override_path).expect("the override reads");
    let before_repository = std::fs::read(&repository_path).expect("the repository table reads");
    let bare = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the unlayered config reads");
    let effective = crate::skill::layer_repository(&bare, repository.path())
        .expect("the repository layer reads");
    let mut app = App::with_configs(
        &std::collections::BTreeMap::from([(adapter.slug, effective)]),
        &[adapter.slug],
    );
    app.bare.insert(adapter.slug, bare);
    app.repository = repository.path().display().to_string();
    app.everyone = true;
    app.set(Setting::Delivery, "ask")
        .expect("Delivery authorisation is accepted");
    app.set(Setting::Models, "orchestrate=provider/preview")
        .expect("the model is accepted");
    app.set(Setting::Evidence, "measuring")
        .expect("the repository answer is accepted");
    assert!(app.dirty(), "the preview has no change to protect");
    let installed_before = app.installed.clone();
    let bare_before = app.bare.clone();

    let dry_run = crate::setup::SetupOptions {
        dry_run: true,
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let plan = app.plan();
    let (receipt, _) = crate::cli::install_planned(&plan, &dry_run, false)
        .expect("an unwritten preview must not fail read-back");
    assert!(receipt.summary.starts_with("would install"));
    assert!(
        receipt.read_back.is_empty()
            && receipt.unlayered_read_back.is_empty()
            && receipt.contract_read_back.is_empty()
            && receipt.agent_read_back.is_empty()
            && receipt.local_read_back.is_empty()
            && receipt.acknowledged.is_empty()
            && receipt.repository.is_none(),
        "a dry-run receipt carries acknowledgement evidence"
    );
    app.installed_now(receipt);

    assert_eq!(
        std::fs::read(&contract_path).expect("the contract still reads"),
        before_contract,
        "interactive dry-run changed the skill contract"
    );
    assert_eq!(
        std::fs::read(&override_path).expect("the override still reads"),
        before_override,
        "interactive dry-run changed the per-agent override"
    );
    assert_eq!(
        std::fs::read(&repository_path).expect("the repository table still reads"),
        before_repository,
        "interactive dry-run changed the repository table"
    );
    assert_eq!(
        app.installed, installed_before,
        "preview advanced installed"
    );
    assert_eq!(app.bare, bare_before, "preview advanced bare");
    assert!(app.dirty(), "preview cleared the unsaved state");
}

#[test]
fn a_no_edit_save_never_promotes_a_local_override_into_the_shared_contract() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let local = root.join(crate::config::LOCAL_FILE);
    std::fs::write(
        &local,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
             | Model routing | developer=opus | unset |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the local override is written");
    let contract = root.join(crate::skill::CONTRACT);
    let before = std::fs::read(&contract).expect("the shared contract reads");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the local layer reads");
    assert_eq!(Setting::Models.value_of(&effective), "developer=opus");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, effective)]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("a no-edit save");

    assert_eq!(
        std::fs::read(&contract).expect("the shared contract still reads"),
        before,
        "the local answer was promoted into shared SKILL.md"
    );
    assert!(
        !crate::skill::agent_override(&root, adapter.slug).exists(),
        "a no-edit local answer was duplicated into the per-agent override"
    );
}

#[test]
fn a_local_override_reads_without_promotion() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    let contract = root.join(crate::skill::CONTRACT);

    let local = root.join(crate::config::LOCAL_FILE);
    std::fs::write(
        &local,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
             | Planning | sdd lite | direct |\n\
             | Model routing | developer=opus | unset |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the local override is written");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the local layer reads");
    assert_eq!(
        Setting::Models.value_of(&effective),
        "developer=opus",
        "the local override's own row did not read"
    );
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir(repository.path().join(".git")).expect("a git directory");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, {
            let mut row = effective;
            Setting::IssueRequirement
                .apply(&mut row, "required")
                .expect("IssueRequirement is accepted");
            row
        })]),
        repository: repository.path().to_path_buf(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("the effective host installs");

    let after = std::fs::read_to_string(&contract).expect("the portable contract still reads");
    assert!(
        !after.contains("developer=opus"),
        "the local host configuration was promoted into SKILL.md"
    );
    assert!(
        !after.contains("| Planning |"),
        "the retired row was written back into the portable contract"
    );
}

#[test]
fn a_per_agent_override_reads_without_rewriting_its_contract() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    let contract = root.join(crate::skill::CONTRACT);

    std::fs::write(
        crate::skill::agent_override(&root, adapter.slug),
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
             | Planning | sdd lite | direct |\n\
             | Model routing | developer=sonnet | unset |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the per-agent override is written");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the per-agent layer reads");
    assert_eq!(
        Setting::Models.value_of(&effective),
        "developer=sonnet",
        "the per-agent override's own row did not read"
    );
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir(repository.path().join(".git")).expect("a git directory");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, {
            let mut row = effective;
            Setting::IssueRequirement
                .apply(&mut row, "required")
                .expect("IssueRequirement is accepted");
            row
        })]),
        repository: repository.path().to_path_buf(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("the effective host installs");

    let after = std::fs::read_to_string(&contract).expect("the portable contract still reads");
    assert!(
        !after.contains("developer=sonnet"),
        "the per-agent host configuration was promoted into SKILL.md"
    );
    assert!(
        !after.contains("| Planning |"),
        "the retired row was written back into the portable contract"
    );
}

#[test]
fn an_opencode_edit_lands_only_in_its_override_and_leaves_shared_peers_unchanged() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let opencode = crate::setup::find_agent("opencode").expect("OpenCode is an adapter");
    let neutral = crate::setup::find_agent("agents").expect("the neutral root is an adapter");
    let cursor = crate::setup::find_agent("cursor").expect("Cursor is an adapter");
    for adapter in [opencode, neutral, cursor] {
        crate::setup::setup(adapter, &Config::default(), &options)
            .unwrap_or_else(|refusal| panic!("{} installs: {refusal}", adapter.slug));
    }
    let root = crate::setup::resolve_paths(opencode, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let contract = root.join(crate::skill::CONTRACT);
    let before_contract = std::fs::read(&contract).expect("the shared contract reads");
    let before_neutral = crate::skill::installed_config_for(&root, Some(neutral.slug))
        .expect("the neutral root reads the shared root");
    let before_cursor = crate::skill::installed_config_for(&root, Some(cursor.slug))
        .expect("Cursor reads the shared root");
    let mut chosen = Config::default();
    Setting::Delivery
        .apply(&mut chosen, "ask")
        .expect("Delivery authorisation is accepted");
    Setting::Models
        .apply(&mut chosen, "orchestrate=provider/opencode")
        .expect("the model is accepted");
    let plan = super::Plan {
        agents: vec![opencode],
        opened: std::collections::BTreeMap::from([(opencode.slug, Config::default())]),
        rows: std::collections::BTreeMap::from([(opencode.slug, chosen.clone())]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("OpenCode installs");

    assert_eq!(
        std::fs::read(&contract).expect("the shared contract still reads"),
        before_contract,
        "an OpenCode-only edit rewrote the shared contract"
    );
    assert_eq!(
        crate::skill::installed_config_for(&root, Some(neutral.slug))
            .expect("the neutral root still reads"),
        before_neutral,
        "the unselected neutral root inherited OpenCode's edit"
    );
    assert_eq!(
        crate::skill::installed_config_for(&root, Some(cursor.slug)).expect("Cursor still reads"),
        before_cursor,
        "unselected Cursor inherited OpenCode's edit"
    );
    let own = crate::skill::installed_config_for(&root, Some(opencode.slug))
        .expect("OpenCode reads its override");
    assert_eq!(Setting::Delivery.value_of(&own), "ask");
    assert_eq!(
        own.models.for_target("orchestrate"),
        Some("provider/opencode")
    );
    let override_text = std::fs::read_to_string(crate::skill::agent_override(&root, opencode.slug))
        .expect("OpenCode's override was written");
    assert!(override_text.contains("orchestrate=provider/opencode"));
}

#[test]
fn a_private_root_keeps_agent_answers_in_its_contract_without_an_override_copy() {
    let home = tempfile::tempdir().expect("a temporary home");
    let options = crate::setup::SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let adapter = crate::setup::find_agent("claude-code").expect("Claude Code is an adapter");
    crate::setup::setup(adapter, &Config::default(), &options).expect("the initial install");
    let root = crate::setup::resolve_paths(adapter, &options)
        .expect("Claude Code resolves")
        .skill_root;
    let mut chosen = Config::default();
    Setting::Delivery
        .apply(&mut chosen, "ask")
        .expect("Delivery is accepted");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, Config::default())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, chosen)]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("Claude Code installs");

    assert_eq!(
        Setting::Delivery
            .value_of(&crate::skill::contract_config(&root).expect("the private contract reads")),
        "ask"
    );
    assert!(
        !crate::skill::agent_override(&root, adapter.slug).exists(),
        "a private-root answer was duplicated into a redundant override"
    );
}

/// doctor's checks are a page *body*, thirty lines and more, and they were
/// landing in that same slot: the operator pressed a key to go back and the menu
/// they returned to was buried under what they had just read.
#[test]
fn a_page_takes_its_report_with_it_when_it_hands_back_to_the_menu() {
    let body = (1..=40)
        .map(|row| format!("line {row}\n"))
        .collect::<String>();

    for page in [Page::Doctor, Page::Help] {
        let mut app = per_agent();
        app.page = page;
        app.report = Some(body.clone());
        app.press(Key::Char('x'));
        assert_eq!(app.page, Page::Home, "{page:?} trapped the cursor");
        assert_eq!(
            app.report, None,
            "{page:?} left its body on the menu it returned to"
        );
    }

    // The guard's notice is the one that stays: it is raised on the menu and
    // never leaves it, so nothing here should reach it.
    let mut app = App::new(Config::default());
    app.report = Some("push guard installed".to_owned());
    app.press(Key::Down);
    assert_eq!(
        app.report, None,
        "moving the menu cursor is what clears its own notice"
    );
}

#[test]
fn every_page_is_painted_in_the_language_the_screen_was_set_to() {
    use crate::tui::words::Tongue;

    // Six sentences the English screen definitely says, one per page, chosen
    // because each is drawn by a different function.
    let english = [
        "ACTIONS",
        "agents chosen",
        "which agents",
        "what may each",
        "whichever agent asks",
        "s install",
    ];

    for (name, mut app) in [
        ("menu", App::new(Config::default())),
        ("agents", setup()),
        ("per agent", per_agent()),
        ("options", options()),
    ] {
        let said = paint(&app, 110, 34);
        app.tongue = Tongue::Spanish;
        let dicho = paint(&app, 110, 34);
        assert_ne!(said, dicho, "{name} paints the same in both languages");

        // cells: a screen whose words their file does not contain would be
        // worse than an English one.
        for line in english {
            if said.contains(line) {
                assert!(
                    !dicho.contains(line),
                    "{name} still says {line:?} on a Spanish screen"
                );
            }
        }
        // And the labels are translated, which they were not: a screen shows a
        // name and stores a key, the way a dropdown shows a label and sends an
        // id. The worry the old rule answered — *`config set` cannot name it* —
        // was real and is answered where it belongs, below.
        if name == "options" {
            assert!(
                dicho.contains(app.tongue.say(Setting::Tracker.label())),
                "a setting's label is still English on a Spanish screen"
            );
            assert!(
                !dicho.contains(Setting::Tracker.label()),
                "the English label is on the screen as well, which is both at once"
            );
        }
    }

    // The help page is a page rather than a line, and it is the one most likely
    // to be forgotten: nothing else reads it.
    let mut help = App::new(Config::default());
    help.page = Page::Help;
    let said = paint(&help, 100, 40);
    help.tongue = Tongue::Spanish;
    let dicho = paint(&help, 100, 40);
    assert_ne!(said, dicho, "the help page is English in both languages");
    assert!(dicho.contains("Teclas"), "the help page did not translate");

    // As is the verdict box, which is drawn over whatever is behind it.
    let mut done = per_agent();
    acknowledge(&mut done, "wrote it");
    let said = paint(&done, 100, 30);
    done.tongue = Tongue::Spanish;
    assert_ne!(
        said,
        paint(&done, 100, 30),
        "the verdict box is untranslated"
    );
}

#[test]
fn the_screens_language_is_on_the_options_page_and_in_no_contract() {
    use crate::tui::words::Tongue;

    let mut app = options();
    // Above the contract rows: the page opens on the first of the two that say
    // what it is showing, because they decide what the rest of it means.
    assert_eq!(
        app.screen_at_cursor(),
        Some(crate::tui::app::Screen::Language),
        "the options page no longer opens on what it is showing"
    );
    assert_eq!(app.screen_at_cursor(), Some(super::app::Screen::Language));
    assert_eq!(
        app.setting_at_cursor(),
        None,
        "a preference answered as a contract row would write a language into every agent"
    );

    // The arrow changes it, and it takes effect at once rather than on `s`.
    assert_eq!(app.tongue, Tongue::English);
    assert_eq!(
        app.press(Key::Right),
        Action::Remember,
        "nothing was asked of the caller"
    );
    assert_eq!(app.tongue, Tongue::Spanish);
    assert!(
        !app.dirty(),
        "changing the screen's language marked a contract unsaved"
    );
    assert!(
        !app.options_touched(),
        "the menu says a preference is an unwritten contract answer"
    );

    // And no contract row moved, on any agent.
    for config in app.configs.values() {
        assert_eq!(
            *config,
            Config::default(),
            "a preference reached a contract"
        );
    }

    // The picker offers the languages this screen has words for, and nowhere to
    // type: one it does not carry would render in English with nothing saying why.
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    let entries = app.picker();
    assert_eq!(entries, vec!["English".to_owned(), "Español".to_owned()]);
    assert!(
        !entries.iter().any(|entry| entry.contains('…')),
        "a closed list offers somewhere to type a language it would not speak"
    );
    walk_picker_to(&mut app, 0);
    assert_eq!(app.press(Key::Enter), Action::Remember);
    assert_eq!(app.tongue, Tongue::English);

    // `r` has nothing to restore here and says so rather than doing nothing.
    app.press(Key::Char('r'));
    assert!(
        app.message.is_some(),
        "a key the footer offers did nothing at all on this row"
    );

    // It is drawn under its own border, which is the sentence: everything above
    // it is written by `s`, and this happened already.
    let painted = paint(&app, 96, 30);
    assert!(painted.contains("OPTIONS"), "{painted}");
    assert!(painted.contains("Interface language"));
    assert!(painted.contains("English"));
}

#[test]
fn the_footer_keeps_the_key_that_finishes_at_every_width() {
    use crate::tui::words::Tongue;

    let mut agents = setup();
    agents.press(Key::Char(' '));
    let mut install = per_agent();
    install.press(Key::Enter);
    install.press(Key::Enter);
    let mut picking = per_agent();
    picking.press(Key::Enter);
    let mut editing = per_agent();
    walk_to_model_target(&mut editing, "orchestrate");
    open_the_field(&mut editing);

    // Not the menu: it builds its own footer inline rather than through
    // `keys_for`, so asking `keys_for` about it would be asking about a list
    // nothing draws.
    let pages = [
        ("agents", agents),
        ("per agent", per_agent()),
        ("install", install),
        ("options", options()),
        ("config edit", App::one_table(None, Config::default())),
        ("picking", picking),
        ("editing", editing),
    ];

    for (name, base) in pages {
        for tongue in [Tongue::English, Tongue::Spanish] {
            let mut app = base.clone();
            app.tongue = tongue;
            let hints = super::keys_for(&app);
            let finishes = hints.last().expect("every page offers a key").clone();
            for width in 1..=120u16 {
                let shown = super::fit_keys(&hints, width);
                if shown.trim().is_empty() {
                    continue;
                }
                // Either the whole key is there, or the window is too narrow
                // for any key at all and what is shown is the start of it.
                let whole = shown.contains(&finishes);
                // Under two columns there is no room for the mark either, so
                // what is shown is the bare start of the key.
                let clipped = finishes.starts_with(shown.trim_end_matches('\u{2026}'));
                assert!(
                    whole || clipped,
                    "{name} in {tongue:?} at {width} columns says {shown:?} and never \
                     {finishes:?} — the key the page is for is the one it dropped"
                );
                assert!(
                    shown.chars().count() <= usize::from(width),
                    "{name} in {tongue:?} at {width}: the footer is wider than the window"
                );
            }
        }
    }
}

#[test]
fn every_page_lists_the_key_that_writes_last() {
    for (name, app) in [
        ("agents", setup()),
        ("detail", per_agent()),
        ("options", options()),
        ("config edit", App::one_table(None, Config::default())),
    ] {
        let hints = super::keys_for(&app);
        let writes: Vec<&String> = hints
            .iter()
            .filter(|hint| hint.contains("s apply"))
            .collect();
        assert_eq!(
            writes.len(),
            1,
            "{name} names the key that writes {} times",
            writes.len()
        );
        assert_eq!(
            hints.last(),
            writes.first().copied(),
            "{name} does not end on the key that writes, so a narrow window drops it"
        );
    }
}

#[test]
fn config_edit_asks_only_for_rows_the_file_it_writes_can_hold() {
    for adapter in crate::setup::AGENTS {
        let app = App::one_table(Some(adapter), Config::default());
        // What the write keeps, taken from the **writer itself** rather than
        // from the renderer it happens to call. Asking the renderer directly
        // was the first shape of this guard, and it could not see the writer
        // switching to a different one — which is the very drift it is here
        // to catch, in the direction nobody was watching.
        let home = tempfile::tempdir().expect("a temporary home");
        let kept = if adapter.discovers_skills() {
            let file = home.path().join("SKILL.md");
            std::fs::write(
                &file,
                crate::config::CONFIG_FENCE.upsert("", &Config::default().render_rows()),
            )
            .expect("a contract");
            std::fs::read_to_string(&file).expect("readable")
        } else {
            let file = home.path().join(format!("estigia.{}.md", adapter.slug));
            // Twice: the file that does not exist yet takes one branch of the
            // writer and the one that does takes the other, and a guard that
            // only ever meets the first cannot see the second drift.
            crate::setup::write_agent_configuration_wholly(&file, adapter.slug, &Config::default())
                .expect("a fresh file");
            crate::setup::write_agent_configuration_wholly(&file, adapter.slug, &Config::default())
                .expect("an existing file");
            std::fs::read_to_string(&file).expect("readable")
        };
        let kept: Vec<String> = crate::config::table_rows(&kept)
            .into_iter()
            .map(|(label, _)| label)
            .collect();
        let asked: Vec<String> = app
            .rows()
            .iter()
            .map(|setting| setting.label().to_owned())
            .collect();
        assert!(
            asked.iter().all(|label| kept.contains(label)),
            "{}: the screen asks for rows the file it writes does not keep",
            adapter.slug
        );
        // And only the rows the agent owns: a machine-wide or repository row is
        // not one agent's to answer, and `config set --agent` refuses it.
        assert!(
            asked.iter().all(|label| Setting::from_label(label)
                .is_some_and(|setting| setting.scope() == crate::config::Scope::Agent)),
            "{}: the per-agent screen asks for a row that is not the agent's to answer",
            adapter.slug
        );
        assert!(!asked.iter().any(|label| label == "Model routing"));
        assert!(kept.iter().any(|label| label == "Model routing"));
        assert!(!app.model_targets().is_empty());
        // An adapter whose file is its own asks for every row that file keeps,
        // minus the synthetic model row. An adapter reading the shared contract
        // asks only for the rows it owns; the shared screen below asks for the
        // rest.
        if !adapter.discovers_skills() {
            assert_eq!(asked.len() + 1, kept.len());
        }
    }

    // The shared screen asks for the whole table, minus the synthetic model
    // row. Repository-only rows are offered too: the contract holds no row for
    // them, so a save writes them to the checkout's own file instead.
    let shared = App::one_table(None, Config::default());
    let contract: Vec<String> = crate::config::table_rows(&Config::default().render_rows())
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    let asked: Vec<String> = shared
        .rows()
        .iter()
        .map(|setting| setting.label().to_owned())
        .collect();
    assert_eq!(asked.len() + 1, SETTINGS.len());
    assert!(asked.iter().any(|label| label == Setting::Tracker.label()));
    assert!(asked.iter().any(|label| label == Setting::Summary.label()));
    assert!(asked.iter().any(|label| label == Setting::Workflow.label()));
    assert!(
        asked
            .iter()
            .any(|label| label == Setting::IssueRequirement.label())
    );
    for label in &asked {
        let setting = Setting::from_label(label).expect("a row the screen offers names a setting");
        assert!(
            contract.contains(label) || setting.scope() == crate::config::Scope::Repository,
            "{label} is offered by the shared screen and neither the contract nor a repository \
             file holds it"
        );
    }

    // And end to end on an adapter that has a file of its own: an answer given
    // on this screen is an answer that comes back.
    let adapter = crate::setup::AGENTS
        .iter()
        .find(|adapter| !adapter.discovers_skills())
        .expect("some adapter shares the neutral root");
    let mut app = App::one_table(Some(adapter), Config::default());
    let setting = app.rows()[0];
    let other = another_value(&app, setting).expect("some row offers a second answer");
    walk_to(&mut app, setting);
    answer(&mut app, &other);

    let written = app.config().render_agent_rows();
    let back = Config::read(&written, None).expect("Estigia wrote this");
    assert_eq!(
        setting.value_of(&back),
        other,
        "{}: an answer this screen took did not survive the write",
        setting.label()
    );
}

#[test]
fn no_state_this_screen_can_reach_is_one_escape_cannot_leave() {
    use std::collections::BTreeSet;

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Backspace,
        Key::Char('s'),
        Key::Char('q'),
        Key::Char('?'),
    ];

    let fresh = App::new(Config::default());
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(format!("{fresh:?}"));
    let mut queue = vec![fresh];
    let mut visited: Vec<App> = Vec::new();

    while let Some(app) = queue.pop() {
        visited.push(app.clone());
        // Bounded, because the walk is exhaustive over a state that carries a
        // configuration: the point is coverage of the *shapes*, not of every
        // value a table can hold.
        if visited.len() >= 4000 {
            break;
        }
        for key in keys {
            let mut next = app.clone();
            let _ = next.press(key);
            if seen.insert(format!("{next:?}")) {
                queue.push(next);
            }
        }
    }
    assert!(
        visited.len() > 500,
        "the walk stopped exploring: {} states",
        visited.len()
    );

    let mut stuck: Vec<String> = Vec::new();
    for app in &visited {
        let mut walk = app.clone();
        let mut home = false;
        for _ in 0..8 {
            let _ = walk.press(Key::Esc);
            if walk.page == Page::Home {
                home = true;
                break;
            }
        }
        if !home {
            stuck.push(format!("{:?}", walk.page));
        }
    }
    assert!(
        stuck.is_empty(),
        "{} of {} states cannot be escaped back to the menu, on pages {:?}",
        stuck.len(),
        visited.len(),
        stuck.iter().collect::<BTreeSet<_>>()
    );
}

#[test]
fn scrolling_past_the_bottom_does_not_have_to_be_scrolled_back() {
    let mut app = App::new(Config::default());
    app.page = Page::Help;

    // A window showing all but the last three lines.
    let furthest = 3;
    for _ in 0..20 {
        app.press(Key::Down);
    }
    assert_eq!(
        app.showing(furthest),
        furthest,
        "the draw showed the wrong line"
    );

    // One press back moves, because the position is the one that was displayed.
    app.press(Key::Up);
    assert_eq!(
        app.showing(furthest),
        furthest - 1,
        "the screen did not answer the first press after an overshoot"
    );

    // And it still cannot be scrolled below the top.
    for _ in 0..20 {
        app.press(Key::Up);
    }
    assert_eq!(app.showing(furthest), 0);

    // A page shorter than the window cannot be scrolled at all.
    app.press(Key::Down);
    assert_eq!(
        app.showing(0),
        0,
        "a page with nothing below scrolled anyway"
    );
}

#[test]
fn nothing_a_person_can_press_discards_an_unsaved_answer_without_asking() {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Backspace,
        Key::Char('s'),
        Key::Char('q'),
        Key::Char('r'),
        Key::Char('?'),
    ];
    let fresh = App::new(Config::default());
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(format!("{fresh:?}"));
    let mut queue = VecDeque::from(vec![fresh]);
    let mut visited: Vec<App> = Vec::new();
    while let Some(app) = queue.pop_front() {
        visited.push(app.clone());
        if visited.len() >= 6000 {
            break;
        }
        for key in keys {
            let mut next = app.clone();
            let _ = next.press(key);
            if seen.insert(format!("{next:?}")) {
                queue.push_back(next);
            }
        }
    }

    // Measured by region, because a walk that reached one page is a walk that
    // proved something about one page.
    let mut per_page: BTreeMap<String, usize> = BTreeMap::new();
    for app in &visited {
        *per_page.entry(format!("{:?}", app.page)).or_default() += 1;
    }
    for page in ["Home", "Agents", "Detail", "Options"] {
        assert!(
            per_page.get(page).copied().unwrap_or_default() > 100,
            "the walk barely reached {page}: {per_page:?}"
        );
    }

    let mut carrying = 0;
    let mut silent: BTreeMap<String, usize> = BTreeMap::new();
    for app in &visited {
        if !app.dirty() {
            continue;
        }
        carrying += 1;
        // The warning was the last thing said: the second press is meant to discard.
        if app.warned {
            continue;
        }
        for quit in [Key::Char('q'), Key::Interrupt] {
            let mut walk = app.clone();
            if matches!(walk.press(quit), Action::Quit) {
                *silent.entry(format!("{:?}", app.page)).or_default() += 1;
            }
        }
    }
    assert!(
        carrying > 100,
        "the walk never reached a state with an unsaved answer, so it refutes \
         nothing: {carrying}"
    );
    assert!(
        silent.is_empty(),
        "these states discard an unsaved answer on one press of `q`: {silent:?}"
    );
}

#[test]
fn every_state_this_screen_can_reach_paints_at_every_size_a_terminal_takes() {
    use std::collections::{BTreeSet, VecDeque};

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Backspace,
        Key::Char('s'),
        Key::Char('?'),
    ];
    let mut config = Config::default();
    Setting::ChangeSize
        .apply(&mut config, "250")
        .expect("a change size");
    Setting::Delivery
        .apply(&mut config, "ask 30m")
        .expect("an authorisation");

    let fresh = App::new(config);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(format!("{fresh:?}"));
    let mut queue = VecDeque::from(vec![fresh]);
    let mut sampled: Vec<App> = Vec::new();
    let mut walked = 0;
    while let Some(app) = queue.pop_front() {
        walked += 1;
        if walked >= 4000 {
            break;
        }
        // Sampled across the walk rather than taken from its front, which would
        // be one page's worth.
        if walked % 25 == 0 && sampled.len() < 150 {
            sampled.push(app.clone());
        }
        for key in keys {
            let mut next = app.clone();
            let _ = next.press(key);
            if seen.insert(format!("{next:?}")) {
                queue.push_back(next);
            }
        }
    }
    let pages: BTreeSet<String> = sampled
        .iter()
        .map(|app| format!("{:?}", app.page))
        .collect();
    assert!(
        sampled.len() >= 150 && pages.len() >= 3,
        "the sample is not spread over the screen: {} states on {pages:?}",
        sampled.len()
    );

    // Ordinary, cramped, and smaller than anything can be drawn in.
    for (width, height) in [(80, 24), (120, 40), (40, 10), (20, 6), (4, 3), (1, 1)] {
        for app in &sampled {
            let _ = paint(app, width, height);
        }
    }
}

#[test]
fn one_model_answer_for_every_selected_agent_is_what_uniform_means() {
    // The step exists because two agents genuinely differ, and that is the
    // interesting case rather than the common one: answering six rows once per
    // agent is six chances to set the same thing four times and get it wrong on
    // the third. So the screen offers the other reading, and this is the whole
    // of what choosing it changes — where a write lands.
    let mut app = per_agent();
    let chosen: std::collections::BTreeSet<&str> =
        app.chosen().iter().map(|adapter| adapter.slug).collect();
    app.everyone = true;
    app.set(Setting::Models, "developer=opus")
        .expect("a model routing");
    let wrong: Vec<&'static str> = app
        .configs
        .iter()
        .filter(|(slug, config)| {
            let changed = Setting::Models.value_of(config) == "developer=opus";
            changed != chosen.contains(**slug)
        })
        .map(|(slug, _)| *slug)
        .collect();
    assert!(
        wrong.is_empty(),
        "uniform Model routing changed a table outside the chosen set or missed one: {wrong:?}"
    );

    let mut app = setup();
    app.everyone = false;
    app.set(Setting::Models, "developer=kimi")
        .expect("a model routing");
    let here = app.current();
    let spread: Vec<&'static str> = app
        .configs
        .iter()
        .filter(|(slug, config)| {
            **slug != here && Setting::Models.value_of(config) == "developer=kimi"
        })
        .map(|(slug, _)| *slug)
        .collect();
    assert!(
        spread.is_empty(),
        "an agent row reached tables nobody pointed it at: {spread:?}"
    );

    // The repository's rows do not care about the switch: they spread either
    // way, because what they are about does not become a different fact when
    // another agent asks.
    for uniform in [true, false] {
        let mut app = setup();
        app.everyone = uniform;
        app.set(Setting::Tracker, "github").expect("a tracker");
        assert!(
            app.configs
                .values()
                .all(|config| Setting::Tracker.value_of(config) == "github"),
            "a repository row stopped spreading with uniform={uniform}"
        );
    }
}

#[test]
fn the_agents_list_has_all_installed_agents_first_and_wraps() {
    let mut app = per_agent();
    app.press(Key::Esc);
    assert_eq!(app.page, Page::Agents);
    assert!(
        !app.everyone,
        "the list forgot the agent it was opened from"
    );

    // Round the whole list and back: one stop for everyone, then each adapter.
    let mut seen_everyone = 0;
    let mut agents = std::collections::BTreeSet::new();
    for _ in 0..=crate::setup::AGENTS.len() {
        app.press(Key::Down);
        if app.everyone {
            seen_everyone += 1;
        } else {
            agents.insert(app.current());
        }
    }
    assert_eq!(seen_everyone, 1, "all chosen agents is not one stop");
    assert_eq!(
        agents.len(),
        crate::setup::AGENTS.len(),
        "an adapter is not a stop of the list"
    );

    // And it walks the other way: the row above the first adapter is everyone's.
    app.everyone = false;
    app.agent = 0;
    app.press(Key::Up);
    assert!(
        app.everyone,
        "walking up from the first agent skipped all chosen agents"
    );
}

#[test]
fn the_shared_answer_does_not_show_one_agent_s_value_as_everyone_s() {
    // Introduced with the shared answer itself, and found by asking what the
    // *read* path knew about it. The write spread; the read did not. Two agents
    // made to differ, then the screen switched to `EVERY AGENT` — and it showed
    // the value of whichever table the cursor happened to be on, with nothing
    // saying the other held something else. Only the rows somebody touched
    // afterwards ever became true.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    let second = app.chosen()[1].slug;
    back_to_agent(&mut app, first);
    app.set(Setting::Delivery, "ask")
        .expect("a delivery authority");
    assert_ne!(
        Setting::Delivery.value_of(&app.configs[first]),
        Setting::Delivery.value_of(&app.configs[second]),
        "the two agents agree, so this test measures nothing"
    );
    assert!(
        !app.disagrees(Setting::Delivery),
        "each agent answering for itself cannot disagree with anybody"
    );

    app.everyone = true;
    assert!(
        app.disagrees(Setting::Delivery),
        "the screen offered one agent's answer as everyone's"
    );

    // And once they do agree, it stops saying so — or the mark is noise that
    // teaches people to ignore it.
    app.set(Setting::Delivery, "auto")
        .expect("a delivery authority");
    assert!(
        !app.disagrees(Setting::Delivery),
        "a row every agent now shares is still reported as split"
    );

    // The third door of the same family, and it needs a shape a shared write
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    back_to_agent(&mut app, first);
    app.set(Setting::Delivery, "ask")
        .expect("a delivery authority");
    to_another_agent(&mut app);
    app.everyone = true;
    assert_eq!(
        Setting::Delivery.value_of(app.config()),
        Setting::Delivery.value_of(&app.installed[app.current()]),
        "the cursor's own table already differs, so this half measures nothing"
    );
    assert!(
        app.changed(Setting::Delivery),
        "an unsaved edit in another agent's table is reported as saved"
    );
}

#[test]
fn restoring_under_the_shared_answer_puts_back_what_was_on_screen() {
    // that had never been on screen.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    let second = app.chosen()[1].slug;

    // Make the two tables differ *as installed*, which is the only way the two
    // readings can point at different values.
    back_to_agent(&mut app, first);
    app.set(Setting::Delivery, "ask")
        .expect("a delivery authority");
    app.installed = app.configs.clone();
    to_another_agent(&mut app);
    assert_eq!(
        app.current(),
        second,
        "the walk did not reach the other agent"
    );

    app.everyone = true;
    let on_screen = app.shown_value(Setting::Delivery);
    assert_ne!(
        on_screen,
        Setting::Delivery.value_of(&app.installed[second]),
        "both tables read the same, so this test measures nothing"
    );

    // Change it, then restore.
    app.set(Setting::Delivery, "auto")
        .expect("a delivery authority");
    walk_to(&mut app, Setting::Delivery);
    app.press(Key::Char('r'));
    assert_eq!(
        app.shown_value(Setting::Delivery),
        on_screen,
        "`r` put back a value that was never on screen"
    );
}

#[test]
fn every_key_the_footer_names_does_something_where_it_is_named() {
    // other four were found by reading, which is not a guard.
    //
    // What makes this checkable is that a hint begins with its key. Pressing it
    // has to move *something* — the page, the focus, the cursor, the
    // tables — or return an action. Doing nothing is the failure.
    let key_of = |hint: &str| -> Option<Key> {
        let head = hint.split_whitespace().next()?;
        Some(match head {
            "←→" => Key::Left,
            "↑↓" => Key::Up,
            "⏎" => Key::Enter,
            "Esc" => Key::Esc,
            "space" | "espacio" => Key::Char(' '),
            "enter" | "intro" => Key::Enter,
            "backspace" | "retroceso" => Key::Backspace,
            other => Key::Char(other.chars().next()?),
        })
    };
    let shape = |app: &App| {
        format!(
            "{:?}|{:?}|{}|{}|{}|{}|{}|{:?}|{}|{:?}|{:?}|{:?}|{}",
            app.page,
            app.focus,
            app.selected,
            app.options_at,
            app.agent,
            app.everyone,
            app.pick,
            app.draft,
            app.dirty(),
            app.message,
            app.configs,
            app.agents.iter().map(|(_, on)| *on).collect::<Vec<_>>(),
            app.reviewing,
        )
    };

    let mut agents = setup();
    agents.press(Key::Down);
    agents.press(Key::Char(' '));
    agents.press(Key::Up);
    let mut review = per_agent();
    review.press(Key::Char('s'));
    let mut picking = per_agent();
    picking.press(Key::Char(' '));
    let mut editing = per_agent();
    walk_to_model_target(&mut editing, "orchestrate");
    open_the_field(&mut editing);

    for (name, base) in [
        ("agents", agents),
        ("detail", per_agent()),
        ("review", review),
        ("options", options()),
        ("picking", picking),
        ("editing", editing),
    ] {
        let hints = super::keys_for(&base);
        assert!(!hints.is_empty(), "{name}: no keys are offered at all");
        for hint in &hints {
            let Some(key) = key_of(hint) else {
                panic!("{name}: {hint:?} does not begin with a key");
            };
            let mut answered = false;
            let rows = if base.page == Page::Agents {
                base.agents.len() + 1
            } else {
                base.row_count().max(1)
            };
            for row in 0..rows {
                let mut app = base.clone();
                for _ in 0..row {
                    app.press(Key::Down);
                }
                let before = shape(&app);
                if app.press(key) != Action::None || shape(&app) != before {
                    answered = true;
                    break;
                }
            }
            assert!(
                answered,
                "{name}: the footer offers {hint:?} and pressing it does nothing on any row"
            );
        }
    }
}

#[test]
fn a_screen_preference_exists_on_the_options_page_and_nowhere_else() {
    // agent shows all seventeen rows, so its twelfth is past
    let mut app = App::one_table(None, Config::default());
    assert!(
        app.row_count() > EVERYWHERE_SETTINGS.len(),
        "this table is too short to reach past the repository's rows, so the \
         guard cannot be measured here"
    );
    for row in 0..app.row_count() {
        app.selected = row;
        assert_eq!(
            app.screen_at_cursor(),
            None,
            "row {row} of a one-table page is read as a preference of the screen"
        );
        assert!(
            app.setting_at_cursor().is_some() || app.model_target_at_cursor().is_some(),
            "row {row} of a one-table page answers as neither a setting nor a model target"
        );
    }

    // And on the page they do belong to, they are found — or the assertion
    // above would hold for the trivial reason that nothing is ever a
    // preference.
    let mut options = options();
    options.options_at = 0;
    assert!(
        options.screen_at_cursor().is_some(),
        "the options page no longer offers the two rows that say what it is showing"
    );
    // And the settings still start where those rows end, so the cursor covers
    // both panels without a gap or an overlap.
    options.options_at = crate::tui::app::SCREEN_ROWS.len();
    assert!(
        options.screen_at_cursor().is_none() && options.setting_at_cursor().is_some(),
        "the first row under the two is not a setting"
    );
}

#[test]
fn leaving_with_answers_nobody_wrote_asks_once_from_every_door() {
    // What it protects is the answer somebody typed and has not installed. The
    let dirty = || {
        let mut app = per_agent();
        walk_to(&mut app, Setting::Delivery);
        answer(&mut app, "ask");
        app.message = None;
        app
    };
    /// A way to the door, and the key pressed there.
    type Door = (&'static str, fn(App) -> App, Key);
    let doors: [Door; 6] = [
        ("a settings page, q", |app| app, Key::Char('q')),
        (
            "the menu, esc",
            |mut app| {
                to_menu(&mut app);
                app.message = None;
                app
            },
            Key::Esc,
        ),
        (
            "a review, ctrl-c",
            |mut app| {
                app.press(Key::Char('s'));
                app
            },
            Key::Interrupt,
        ),
        (
            "config edit, esc",
            |_| {
                let mut app = App::one_table(None, Config::default());
                app.set(Setting::Delivery, "ask").expect("accepted");
                app
            },
            Key::Esc,
        ),
        (
            "the menu's own quit",
            |mut app| {
                app.page = Page::Home;
                while app.entry().goes != super::app::Goes::Doing(super::app::Deed::Quit) {
                    app.press(Key::Down);
                }
                app.message = None;
                app
            },
            Key::Enter,
        ),
        (
            "doctor, q",
            |mut app| {
                app.page = Page::Doctor;
                app
            },
            Key::Char('q'),
        ),
    ];

    for (name, reach, key) in doors {
        let mut app = reach(dirty());
        assert!(
            app.dirty(),
            "{name}: nothing is unsaved, so nothing is at risk"
        );
        assert_eq!(
            app.press(key),
            Action::None,
            "{name}: it left with answers nobody had written"
        );
        assert!(
            app.message.is_some(),
            "{name}: it stayed put and said nothing, which reads as a dead key"
        );
        let page = app.page;
        let action = app.press(key);
        assert!(
            action != Action::None || app.page != page,
            "{name}: the warning cannot be got past"
        );
    }
}

#[test]
fn the_picker_offers_what_the_row_is_set_to_even_when_it_is_not_on_the_list() {
    // their own value is not in, and choosing anything is then a change they
    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    let offered = app.picker();
    assert!(
        !offered.iter().any(|entry| entry == ELSEWHERE),
        "the fixture value is already on the list, so this measures nothing"
    );

    answer(&mut app, ELSEWHERE);
    assert_eq!(
        app.shown_value(Setting::Worktree),
        ELSEWHERE,
        "the answer did not land, so the picker below is asked about the old value"
    );
    let entries = app.picker();
    assert_eq!(
        entries.first().map(String::as_str),
        Some(ELSEWHERE),
        "the picker does not offer what the row is set to: {entries:?}"
    );
}

#[test]
fn a_key_is_acted_on_when_it_is_pressed_and_not_when_it_is_let_go() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let event = |kind| {
        Event::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind,
            state: KeyEventState::NONE,
        })
    };
    assert_eq!(
        super::key_of(&event(KeyEventKind::Press)),
        Some(Key::Char('a')),
        "a pressed key was ignored, so nothing reaches the screen at all"
    );
    for ignored in [KeyEventKind::Release, KeyEventKind::Repeat] {
        assert_eq!(
            super::key_of(&event(ignored)),
            None,
            "{ignored:?} is acted on, so every key does its work twice"
        );
    }

    // And an event that is not a key at all — a resize, a paste — is not one
    // this screen presses.
    assert_eq!(super::key_of(&Event::Resize(80, 24)), None);
}

#[test]
fn a_row_the_agents_do_not_agree_on_says_so_on_the_row() {
    // The visible half of the guard that stopped this screen offering one
    // agent's answer as everyone's. `disagrees` was measured when it was
    // holds something else.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    back_to_agent(&mut app, first);
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    app.everyone = true;
    assert!(
        app.disagrees(Setting::Delivery),
        "the two agents agree, so there is nothing for the row to say"
    );

    let painted = paint(&app, 110, 34);
    assert!(
        painted.contains("differs by agent"),
        "a row the agents do not agree on is drawn as though they did: {painted}"
    );

    // And it is not said of a row they do agree on, or the mark is noise on
    // every line and stops meaning anything.
    let quiet = per_agent();
    let painted = paint(&quiet, 110, 34);
    assert!(
        !painted.contains("differs by agent"),
        "the mark is drawn on rows nobody disagrees about: {painted}"
    );
}

#[test]
fn a_row_about_this_machine_reaches_every_table_and_is_read_from_all_of_them() {
    let mut app = per_agent();
    let first = app.chosen()[0].slug;

    // Answered on the options page, where the machine's rows live.
    let mut options = options();
    walk_to(&mut options, Setting::Summary);
    assert_eq!(
        Setting::Summary.scope(),
        crate::config::Scope::Machine,
        "this row is no longer the machine's, so the test measures another rule"
    );
    answer(&mut options, "Esperanto");

    // `set`: every table heard it.
    let deaf: Vec<&'static str> = options
        .configs
        .iter()
        .filter(|(_, config)| Setting::Summary.value_of(config) != "Esperanto")
        .map(|(slug, _)| *slug)
        .collect();
    assert!(
        deaf.is_empty(),
        "a machine row was answered once and these tables did not hear it: {deaf:?}"
    );

    assert_eq!(
        options.shown_value(Setting::Summary),
        "Esperanto",
        "the row shows something other than the answer every table now holds"
    );
    let mut split = options.clone();
    let cursor = split.current().to_owned();
    for (slug, config) in split.configs.iter_mut() {
        let _ = Setting::Summary.apply(
            config,
            if *slug == cursor {
                "Volapuk"
            } else {
                "Esperanto"
            },
        );
    }
    assert!(
        split.disagrees(Setting::Summary),
        "the tables were not made to differ, so the assertion below measures nothing"
    );
    assert_ne!(
        split.shown_value(Setting::Summary),
        "Volapuk",
        "the page offered the cursor's own answer as what every table holds"
    );

    // `changed`: an unsaved edit in another agent's table is still unsaved.
    app.everyone = false;
    back_to_agent(&mut app, first);
    assert!(
        !app.changed(Setting::Summary),
        "nothing has been edited yet, so the assertion below would hold anyway"
    );
    let _ = app.set(Setting::Summary, "Esperanto");
    assert!(
        app.changed(Setting::Summary),
        "a machine row that was edited is reported as installed"
    );
}

#[test]
fn the_screen_shows_the_name_of_a_row_in_its_own_language() {
    // A dropdown shows a label and sends an id, and this screen does the same:
    // the row says `Autorización de transición` and the contract keeps
    // `Transition authorisation`.
    let mut app = per_agent();
    app.tongue = crate::tui::words::Tongue::Spanish;
    walk_to(&mut app, Setting::Transitions);
    answer(&mut app, "ask");

    let painted = paint(&app, 120, 34);
    assert!(
        painted.contains(app.tongue.say(Setting::Transitions.label())),
        "the row is not shown by its name: {painted}"
    );

    app.press(Key::Char('s'));
    let reviewed = paint(&app, 120, 34);
    assert!(
        reviewed.contains(app.tongue.say(Setting::Transitions.label())),
        "the review does not name the setting in the screen's language: {reviewed}"
    );
}

#[test]
fn a_row_that_takes_a_directory_offers_the_ones_that_are_there() {
    // Typing an absolute path into a field is the worst way to answer a row
    // whose answer is a directory. The picker already had arrows, `Enter` and
    // somewhere to type — so the folders go on it, and choosing one walks in.
    // it draws.
    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    assert!(
        Setting::Worktree.takes_a_directory(),
        "this row no longer takes a directory, so the test measures another rule"
    );
    let bare = app.picker();
    assert!(
        !bare.iter().any(|entry| entry == "H:/one"),
        "the fixture folder is already offered, so the assertion below is free"
    );

    app.folders = vec!["H:/one".to_owned(), "H:/two".to_owned()];
    let offered = app.picker();
    assert_eq!(
        offered.first().map(String::as_str),
        Some("H:/one"),
        "the folders are not the first thing offered: {offered:?}"
    );
    assert!(
        offered.iter().any(|entry| entry == TYPE_IT),
        "the way to type one by hand went away: {offered:?}"
    );

    // And a row that is not a directory is not offered any, however many the
    // shell happened to have looked up.
    walk_to(&mut app, Setting::Tracker);
    let offered = app.picker();
    assert!(
        !offered.iter().any(|entry| entry == "H:/one"),
        "a row that takes no directory was offered folders: {offered:?}"
    );
}

#[test]
fn the_walk_starts_where_the_row_is_set_and_falls_back_to_home() {
    // comparing a `TempDir` on the left to the real home on the right. One
    // lock around the whole region is what makes every read in it agree.
    crate::test_env::with_environment_read_locked(|| {
        // Choosing a folder sets the row to it, and opening the picker again
        // offers *its* children — which is what makes this a walk rather than
        // one list. Unset, it starts at home, because a picker rooted at
        // nothing has nothing on it.
        let mut app = options();
        walk_to(&mut app, Setting::Worktree);
        let home = crate::paths::home_dir().expect("a home");
        assert_eq!(
            app.folder_root(),
            home,
            "an unset row does not start anywhere somebody can walk from"
        );

        // Set to a directory that is really there, the walk continues from it.
        let root = tempfile::tempdir().expect("a directory");
        app.set(Setting::Worktree, &root.path().display().to_string())
            .expect("an absolute path");
        assert_eq!(
            app.folder_root(),
            root.path(),
            "the walk did not continue from what the row is set to"
        );

        // Set to something that is not there, it falls back rather than
        // offering nothing at all.
        app.set(Setting::Worktree, ABSENT)
            .expect("an absolute path");
        assert_eq!(
            app.folder_root(),
            home,
            "a path that is not there left the picker rooted at nowhere"
        );
    });
}

#[test]
fn the_options_page_can_be_pointed_at_another_checkout() {
    let mut app = options();
    for _ in 0..OPTIONS_SETTINGS.len() {
        app.press(Key::Down);
    }
    while app.screen_at_cursor() != Some(super::app::Screen::Repository) {
        app.press(Key::Down);
    }

    // Fed, because which checkouts answer for themselves is a question about
    // the disk and this machine has none in it.
    app.repositories = vec!["H:/one".to_owned(), "H:/two".to_owned()];
    let offered = app.picker();
    assert_eq!(offered, app.repositories, "the checkouts are not offered");

    // Choosing one names a reload rather than performing it: reading another
    // checkout's answers is the shell's to do.
    let before = app.configs.clone();
    let action = app.press(Key::Char(' '));
    assert_eq!(action, Action::None, "space opened nothing");
    assert_eq!(app.focus, Focus::Picking);
    let shown = app.screen_value(super::app::Screen::Repository);
    let action = app.press(Key::Enter);
    assert_eq!(
        action,
        Action::Reload("H:/one".to_owned()),
        "choosing a checkout did not ask for its answers"
    );
    // And the row has not moved. It names the checkout whose answers are on the
    // page, and nothing has been read yet — a page that renamed itself first
    // would be showing one repository's answers under another's name if the
    // read then failed.
    assert_eq!(
        app.screen_value(super::app::Screen::Repository),
        shown,
        "the page renamed itself before anything had been read"
    );
    assert_eq!(
        app.configs, before,
        "choosing a checkout wrote into the tables, which is the one thing it must not do"
    );
}

#[test]
fn pointing_at_a_checkout_shows_that_checkouts_answers() {
    let installed: std::collections::BTreeMap<&'static str, Config> = crate::setup::AGENTS
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                Config {
                    // As though the checkout the screen was run in had said so.
                    change_size: 400,
                    ..Config::default()
                },
            )
        })
        .collect();
    let mut app = App::with_configs(&installed, &["claude-code"]);
    // What each agent answers with when no checkout has said anything.
    app.bare = crate::setup::AGENTS
        .iter()
        .map(|adapter| (adapter.slug, Config::default()))
        .collect();

    // A checkout that says one thing and is silent about the other.
    let chosen = tempfile::tempdir().expect("a checkout");
    let file = crate::skill::repository_config_path(chosen.path());
    std::fs::create_dir_all(file.parent().expect("that file has a directory"))
        .expect("the checkout's directory is made");
    std::fs::write(
        &file,
        "| Setting | Value here | Skill default |\n|---|---|---|\n\
         | Tracker | github acme/repo | github |\n",
    )
    .expect("the checkout's rows are written");
    let chosen = chosen.path().display().to_string();
    super::reload_repository(&mut app, &chosen).expect("a readable checkout was refused");

    let shown = app
        .configs
        .get("claude-code")
        .expect("that agent has a table");
    // The floor: the row the checkout *does* name arrives. A reload that did
    // nothing at all would satisfy the assertion below.
    assert_eq!(
        shown.tracker,
        crate::config::Tracker::Github {
            repo: Some(crate::config::RepoRef::parse("acme/repo").unwrap())
        },
        "the checkout's own answer did not arrive"
    );
    assert_eq!(
        shown.change_size,
        Config::default().change_size,
        "a row this checkout is silent about is showing the previous checkout's answer"
    );
}

#[test]
fn saving_does_not_write_this_checkouts_answers_into_the_contract() {
    let contract = Config {
        change_size: 1000,
        ..Config::default()
    };
    // As the screen is handed it: the contract with this checkout's row on top.
    let here = Config {
        change_size: 400,
        ..contract.clone()
    };
    let mut app = App::with_configs(
        &crate::setup::AGENTS
            .iter()
            .map(|adapter| (adapter.slug, here.clone()))
            .collect(),
        &["claude-code"],
    );
    app.bare = crate::setup::AGENTS
        .iter()
        .map(|adapter| (adapter.slug, contract.clone()))
        .collect();

    assert_eq!(
        app.configs
            .get("claude-code")
            .expect("that agent has a table")
            .change_size,
        400,
        "the page is not showing what is in force here, so this measures nothing"
    );

    // Nothing touched: the plan carries no contract candidate at all. The
    // writer reads that layer independently from disk.
    let plan = app.plan();
    assert_eq!(plan.opened["claude-code"].change_size, 400);

    // And a row the operator *does* move goes in. Without this the fix would be
    // a screen whose save writes nothing, which passes the assertion above.
    let moved = crate::config::Tracker::Github {
        repo: Some(crate::config::RepoRef::parse("acme/repo").unwrap()),
    };
    let mut edited = app.configs.clone();
    for config in edited.values_mut() {
        config.tracker = moved.clone();
    }
    app.configs = edited;
    let plan = app.plan();
    let theirs = &plan.rows["claude-code"];
    assert_eq!(
        theirs.tracker, moved,
        "a row the operator moved did not reach the plan"
    );
    assert_eq!(
        plan.opened["claude-code"].change_size, 400,
        "the plan lost what the effective screen opened on"
    );
}

#[test]
fn a_save_sends_the_contracts_table_to_the_contract_and_the_merged_one_to_the_checkout() {
    let contract = Config {
        change_size: 1000,
        ..Config::default()
    };
    let here = Config {
        change_size: 400,
        ..contract.clone()
    };
    let mut app = App::with_configs(
        &crate::setup::AGENTS
            .iter()
            .map(|adapter| (adapter.slug, here.clone()))
            .collect(),
        &["claude-code"],
    );
    app.bare = crate::setup::AGENTS
        .iter()
        .map(|adapter| (adapter.slug, contract.clone()))
        .collect();
    app.repository = "H:/somewhere".to_owned();

    let plan = app.plan();
    assert_eq!(
        plan.opened.get("claude-code").expect("a table").change_size,
        400,
        "the plan did not retain the effective value it measures edits against"
    );
    assert_eq!(
        plan.rows.get("claude-code").expect("a table").change_size,
        400,
        "the checkout's own file is about to be written with the contract's answer"
    );
    assert_eq!(
        plan.repository,
        std::path::PathBuf::from("H:/somewhere"),
        "the save is not about the checkout the page names"
    );
}

#[test]
fn the_answer_under_the_cursor_says_what_choosing_it_does() {
    // The screen offered `reading` and `measuring` with a sentence about the
    // row and none about either word, so the arrow key that decides what a
    // reviewer may do was a guess between two spellings.
    let mut app = options();
    walk_to(&mut app, Setting::Evidence);
    answer(&mut app, "measuring");

    let painted = rows(&app, 96, 30).join("\n");
    assert!(
        painted.contains("established by running"),
        "the answer in force is not explained:\n{painted}"
    );

    // And each one as it is reached, which is where somebody is when they are
    // choosing rather than reading.
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");
    while app.picker().get(app.pick).map(String::as_str) != Some("reading") {
        app.press(Key::Down);
    }
    let picking = rows(&app, 96, 40).join("\n");
    assert!(
        picking.contains("established by reading"),
        "arrowing onto an answer does not say what it means:\n{picking}"
    );
    // The floor: it is the answer under the picker that is explained, not
    // whatever happens to be set.
    assert!(
        !picking.contains("established by running"),
        "the explanation did not follow the picker's cursor:\n{picking}"
    );
}

#[test]
fn a_folder_row_is_walked_rather_than_answered_at_every_step() {
    // without deciding was missing.
    let root = tempfile::tempdir().expect("a temporary root");
    let deep = root.path().join("trees").join("issue-7");
    std::fs::create_dir_all(&deep).expect("a folder to walk into");

    let mut app = options();
    while app.setting_at_cursor() != Some(Setting::Worktree) {
        app.press(Key::Down);
    }
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");

    // The shell feeds the listing; this state machine has no filesystem in it.
    app.browsing = Some(root.path().to_path_buf());
    app.folders = vec![root.path().join("trees").display().to_string()];
    app.pick = 0;

    let before = app.shown_value(Setting::Worktree);
    app.press(Key::Right);
    assert_eq!(
        app.folder_root(),
        root.path().join("trees"),
        "the right arrow did not walk into the folder"
    );
    assert_eq!(
        app.shown_value(Setting::Worktree),
        before,
        "walking into a folder answered the row"
    );
    assert_eq!(app.focus, Focus::Picking, "the walk closed the picker");

    // And back out.
    app.press(Key::Left);
    assert_eq!(
        app.folder_root(),
        root.path(),
        "the left arrow did not walk back out"
    );

    // Enter is the one key that answers, and it ends the walk.
    app.folders = vec![deep.display().to_string()];
    app.pick = 0;
    app.press(Key::Enter);
    assert_eq!(
        app.shown_value(Setting::Worktree),
        deep.display().to_string()
    );
    assert_eq!(
        app.browsing, None,
        "the next picker would open where this one ended"
    );
}

#[test]
fn naming_a_folder_asks_the_shell_to_make_it_and_refuses_a_path() {
    let root = tempfile::tempdir().expect("a temporary root");
    let mut app = options();
    while app.setting_at_cursor() != Some(Setting::Worktree) {
        app.press(Key::Down);
    }
    app.press(Key::Char(' '));
    app.browsing = Some(root.path().to_path_buf());

    app.press(Key::Char('n'));
    assert_eq!(app.focus, Focus::Naming, "`n` did not open the name field");
    type_in(&mut app, "trees");
    assert_eq!(
        app.press(Key::Enter),
        Action::MakeFolder(root.path().join("trees")),
        "the screen did not name the folder for the shell to make"
    );

    app.press(Key::Char('n'));
    type_in(&mut app, "a/b");
    assert_eq!(app.press(Key::Enter), Action::None);
    assert!(
        app.message.is_some_and(|why| why.contains("separators")),
        "a path was taken as a folder name"
    );
}

#[test]
fn the_screen_only_asks_the_disk_where_an_answer_could_use_it() {
    let mut app = options();

    // The rows that answer with a path, and only those.
    let mut asked_on = Vec::new();
    for _ in 0..app.row_count() {
        if app.wants_folders() {
            asked_on.push(app.setting_at_cursor());
        }
        app.press(Key::Down);
    }
    asked_on.dedup();
    assert_eq!(
        asked_on,
        vec![Some(Setting::Worktree)],
        "the disk is read for rows whose answer is not a path"
    );

    // And the shell is what asks. A screen that reached for the filesystem
    // itself could not be driven by this test at all.
    let shell = crate::source_text::code_of(include_str!("../tui.rs"));
    let reads = crate::source_text::code_of(include_str!("app.rs"));
    assert!(
        shell.contains("reads.refresh(&mut app, &mut Disk);")
            && reads.contains("if app.wants_folders() {"),
        "the frame reads the disk without asking whether anything wants it"
    );
    assert!(
        !app.wants_folders() || app.setting_at_cursor() == Some(Setting::Worktree),
        "the question answers yes somewhere it should not"
    );
}

#[test]
fn the_row_wearing_the_arrows_mark_is_the_row_the_arrows_change() {
    // Three readers of one offset in one function, and only one had it — which
    let mut app = options();
    // Onto a settings row that offers more than one answer, past the screen
    // preferences at the top.
    walk_to(&mut app, Setting::Integration);
    let setting = app
        .setting_at_cursor()
        .expect("the cursor is on a settings row");

    let rows = rows(&app, 110, 34);
    let label = app.tongue.say(setting.label());
    // The settings panel only. The preferences panel above it draws `‹ ›` on
    // every one of its rows, where the character means *this row is editable*
    // rather than *the cursor is here* — so counting them across the frame
    // measures two different marks.
    let row = rows
        .iter()
        .find(|row| row.contains(label))
        .unwrap_or_else(|| panic!("the row is not drawn at all:\n{}", rows.join("\n")));

    // The floor: this row is the highlighted one. If the highlight were
    // elsewhere the assertion below would be about a row nobody is on.
    assert!(
        row.trim_start_matches(['\u{2502}', ' ']).starts_with('>'),
        "the cursor is on {label:?} and the highlight is elsewhere:\n{}",
        rows.join("\n")
    );
    assert!(
        row.contains('\u{2039}'),
        "the highlighted row carries no arrows mark, so the mark is on another row:\n{}",
        rows.join("\n")
    );
    // And no row below it wears one: the mark used to be drawn as many rows
    // further down as the page draws above this list.
    let below: Vec<&String> = rows
        .iter()
        .skip_while(|other| !other.contains(label))
        .skip(1)
        .filter(|other| other.contains('\u{2039}'))
        .collect();
    assert!(
        below.is_empty(),
        "a row under the cursor also wears the arrows mark: {:?}",
        below.iter().map(|row| row.trim()).collect::<Vec<_>>()
    );
}

#[test]
fn model_rows_never_advertise_setting_cycles() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");
    let model_row = |app: &App| {
        rows(app, 110, 34)
            .into_iter()
            .find(|row| row.contains("orchestrate"))
            .expect("the orchestrate row is drawn")
    };

    let row = model_row(&app);
    assert!(
        !row.contains('\u{2039}') && !row.contains('\u{203a}'),
        "an inherited target wears setting-cycle arrows: {row}"
    );

    for (key, effort) in [(Key::Left, "max"), (Key::Right, "low")] {
        app.set(Setting::Models, "orchestrate=gpt-5.6")
            .expect("a model route");
        let row = model_row(&app);
        assert!(
            !row.contains('\u{2039}') && !row.contains('\u{203a}'),
            "a custom target advertises setting-cycle arrows: {row}"
        );

        app.press(key);
        assert_eq!(
            app.shown_value(Setting::Models),
            format!("orchestrate=gpt-5.6; orchestrate={effort}"),
            "{key:?} did more to a target than step its effort"
        );
    }

    app.set(Setting::Models, "orchestrate=gpt-5.6")
        .expect("a model route");
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    let row = model_row(&app);
    assert!(
        !row.contains('\u{2039}') && !row.contains('\u{203a}'),
        "setting-row arrows stayed visible while the picker owned the keys: {row}"
    );
}

#[test]
fn the_answers_open_under_the_row_they_belong_to() {
    // The fourth reader of one offset, and the one an operator sees most: the
    // list of answers is placed inside the settings panel by the page's cursor,
    // which counts the preferences drawn above that panel. So it opened hanging
    // off a row below the one it was about to change.
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    walk_onto_a_setting(&mut app);
    app.press(Key::Down);
    let setting = app.setting_at_cursor().expect("a settings row");
    let label = app.tongue.say(setting.label());
    app.press(Key::Char(' '));
    assert_eq!(
        app.focus,
        Focus::Picking,
        "the picker did not open, so nothing here is measured"
    );

    let rows = rows(&app, 110, 34);
    let at = rows
        .iter()
        .position(|row| row.contains(label))
        .unwrap_or_else(|| panic!("the row is not drawn:\n{}", rows.join("\n")));
    let answer = app
        .picker()
        .first()
        .cloned()
        .expect("this row offers answers");
    // Below the row itself, which shows that same value in its own cell — the
    // first match otherwise is the row the box is hanging from.
    let opened = at
        + 1
        + rows
            .iter()
            .skip(at + 1)
            .position(|row| row.contains(&answer))
            .unwrap_or_else(|| panic!("the answers are not drawn:\n{}", rows.join("\n")));

    assert_eq!(
        opened,
        at + 2,
        "the answers for {label:?} (row {at}) open at row {opened}, not under it:\n{}",
        rows.join("\n")
    );
}

#[test]
fn a_reload_that_refuses_leaves_the_page_showing_what_it_names() {
    // checkout's name. A save then sends `app.configs` to the checkout
    // `app.repository` names, so the mixture goes to the wrong one.
    let contract = Config::default();
    let mut app = App::with_configs(
        &crate::setup::AGENTS
            .iter()
            .map(|adapter| (adapter.slug, contract.clone()))
            .collect(),
        &["claude-code"],
    );
    // Every agent's contract is the same except one, so the injected layering
    // below can refuse on exactly that one — and `configs` is a `BTreeMap`, so
    // which agents come before it is fixed.
    app.bare = crate::setup::AGENTS
        .iter()
        .map(|adapter| {
            let mut base = contract.clone();
            if adapter.slug == "cursor" {
                base.evidence = crate::config::Evidence::Measuring;
            }
            (adapter.slug, base)
        })
        .collect();
    app.repository = "H:/here".to_owned();
    let before = app.configs.clone();

    // Succeeds until it meets the marked contract, which is not the first.
    fn layering(base: &Config, _repo: &std::path::Path) -> Result<Config, crate::outcome::Refusal> {
        if base.evidence == crate::config::Evidence::Measuring {
            return Err(crate::outcome::Refusal::not_started(
                "config-local-unreadable",
                "the checkout's own file will not open".to_owned(),
                crate::outcome::Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "that file readable",
                ),
            ));
        }
        let mut theirs = base.clone();
        theirs.evidence = crate::config::Evidence::Reading;
        // Flip away from Measuring so the succeeding path is visible.
        if theirs.evidence == crate::config::Evidence::Reading {
            theirs.integration = crate::config::Integration::Trunk;
        }
        Ok(theirs)
    }

    // The floor: this layering really does change a row, so "unchanged after a
    // refusal" is a statement about a reload that had something to write.
    let mut succeeding = app.clone();
    succeeding.bare = crate::setup::AGENTS
        .iter()
        .map(|adapter| (adapter.slug, contract.clone()))
        .collect();
    super::reload_repository_with(&mut succeeding, "H:/there", layering)
        .expect("a reload with nothing to refuse");
    assert_eq!(
        succeeding
            .configs
            .get("claude-code")
            .expect("a table")
            .integration,
        crate::config::Integration::Trunk,
        "the layering changed nothing, so the assertion below poses nothing"
    );

    let refused = super::reload_repository_with(&mut app, "H:/there", layering);
    assert!(refused.is_err(), "the injected refusal did not arrive");
    assert_eq!(
        app.configs, before,
        "a refused reload left some agents carrying the other checkout's rows"
    );
    assert_eq!(
        app.repository, "H:/here",
        "the page renamed itself on a reload that refused"
    );
}

/// While the walk is inside a folder, the box says which folder.
///
/// The picker's title repeated the row's own name — which is already drawn one
/// line above it and one column to its left — so the only line that could say
/// **where these names are** said nothing. Two levels down, `interno` beside
#[test]
fn the_folder_walk_says_which_folder_it_is_showing() {
    let root = tempfile::tempdir().expect("a temporary root");
    let deep = root.path().join("proyectos").join("interno");
    std::fs::create_dir_all(&deep).expect("the folders");

    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    app.press(Key::Char(' '));

    // Before the walk starts, the box still names the row: there is no folder
    // to name yet, and a box titled with a path nobody chose would be worse.
    let label = app.tongue.say(Setting::Worktree.label());
    let closed = rows(&app, 110, 34);
    assert!(
        closed
            .iter()
            .any(|row| row.matches(&label).count() >= 2 || row.contains(&format!("╭ {label}"))),
        "the picker does not name the row it belongs to:\n{}",
        closed.join("\n")
    );

    // Walked in. The shell feeds the folders; this is the state it feeds them
    // into, which is the part a test can hold.
    app.browsing = Some(root.path().join("proyectos"));
    app.folders = vec!["interno".to_owned()];
    let walking = rows(&app, 110, 34);
    let titled = walking
        .iter()
        .find(|row| row.contains("proyectos") && row.contains('\u{256d}'))
        .unwrap_or_else(|| {
            panic!(
                "no box names the folder the walk is in:\n{}",
                walking.join("\n")
            )
        });
    assert!(
        !titled.contains(label),
        "the box still spends its title on the row's own name: {}",
        titled.trim()
    );
    // And the entries are still there, or the title took the box with it.
    assert!(
        walking.iter().any(|row| row.contains("interno")),
        "the folders stopped being drawn:\n{}",
        walking.join("\n")
    );
}

#[test]
fn a_repository_remote_on_github_is_offered_to_the_tracker_and_the_host() {
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    for setting in [Setting::Tracker, Setting::RepositoryHost] {
        assert!(
            app.offered(setting)
                .contains(&"github asanabrial/estigia".to_owned()),
            "{setting:?} does not offer the repository's own slug"
        );
    }
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    assert!(
        app.picker()
            .contains(&"github asanabrial/estigia".to_owned())
    );

    let bare = options();
    for setting in [Setting::Tracker, Setting::RepositoryHost] {
        assert_eq!(bare.offered(setting), vec!["github".to_owned()]);
    }
}

#[test]
fn an_open_row_with_a_single_answer_is_a_text_field_and_not_a_picker() {
    let mut app = options();
    for setting in crate::config::OPTIONS_SETTINGS {
        if setting.answers().closed || setting.takes_a_directory() {
            continue;
        }
        assert!(
            app.offered(*setting).len() >= 2 || app.is_text_field(*setting),
            "{setting:?} would draw a picker with one entry"
        );
    }
    walk_to(&mut app, Setting::Tracker);
    assert!(paint(&app, 110, 40).contains("type to change"));
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Editing);
}

#[test]
fn every_suggestion_a_row_offers_is_accepted_and_reads_back_the_same() {
    let mut app = options();
    app.remote = Some("github asanabrial/estigia".to_owned());
    let mut checked = 0;
    for setting in crate::config::OPTIONS_SETTINGS {
        for offered in app.offered(*setting) {
            let mut config = Config::default();
            setting
                .apply(&mut config, &offered)
                .unwrap_or_else(|refusal| panic!("{setting:?} refuses {offered:?}: {refusal}"));
            assert_eq!(setting.value_of(&config), offered, "{setting:?}");
            checked += 1;
        }
    }
    assert!(checked > 20);
    assert!(
        app.offered(Setting::Boundaries)
            .contains(&"git push --force".to_owned())
    );
    assert!(
        app.offered(Setting::Worktree)
            .iter()
            .any(|entry| entry.contains("<repo>") && entry.contains("<branch>"))
    );
}

fn unsaved() -> App {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    app.message = None;
    assert!(
        app.dirty(),
        "the answer did not land, so nothing is at risk"
    );
    app
}

const WARNING: &str = "unsaved changes";

fn key_event(code: crossterm::event::KeyCode, modifiers: crossterm::event::KeyModifiers) -> Key {
    use crossterm::event::{Event, KeyEvent, KeyEventKind, KeyEventState};
    super::key_of(&Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }))
    .expect("a pressed key is a key")
}

#[test]
fn a_control_chord_is_never_its_bare_letter() {
    use crossterm::event::{KeyCode, KeyModifiers};
    for letter in ['s', 'q', 'r', 'a', 'j', 'n'] {
        assert_eq!(
            key_event(KeyCode::Char(letter), KeyModifiers::CONTROL),
            Key::Other,
            "Ctrl-{letter} was read as a bare {letter}"
        );
    }
    assert_eq!(
        key_event(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Key::Interrupt
    );
    assert_eq!(
        key_event(KeyCode::Char('c'), KeyModifiers::NONE),
        Key::Char('c')
    );
    assert_eq!(
        key_event(KeyCode::Char('S'), KeyModifiers::SHIFT),
        Key::Char('S')
    );
}

#[test]
fn ctrl_c_leaves_from_every_reachable_state_and_asks_once_over_unsaved_answers() {
    let mut editing = per_agent();
    walk_to_model_target(&mut editing, "orchestrate");
    open_the_field(&mut editing);
    let mut picking = per_agent();
    picking.press(Key::Char(' '));
    let mut doctor = App::new(Config::default());
    doctor.page = Page::Doctor;
    let mut help = App::new(Config::default());
    help.page = Page::Help;
    let mut modal = App::new(Config::default());
    modal.install_failed("it failed".to_owned(), "because".to_owned());
    let mut one_table = App::one_table(None, Config::default());
    one_table.press(Key::Char(' '));
    for (name, base) in [
        ("home", App::new(Config::default())),
        ("agents", setup()),
        ("per agent", per_agent()),
        ("options", options()),
        ("editing", editing),
        ("picking", picking),
        ("doctor", doctor),
        ("help", help),
        ("a verdict over the page", modal),
        ("one table", one_table),
    ] {
        let mut app = base;
        assert_eq!(
            app.press(Key::Interrupt),
            Action::Quit,
            "{name}: Ctrl-C did not leave"
        );
    }

    for name in ["agents", "per agent", "options", "editing", "picking"] {
        let mut app = unsaved();
        match name {
            "agents" => {
                app.press(Key::Backspace);
                app.press(Key::Backspace);
            }
            "options" => to_options(&mut app),
            "editing" => {
                walk_to_model_target(&mut app, "orchestrate");
                open_the_field(&mut app);
            }
            "picking" => {
                app.press(Key::Down);
                app.press(Key::Char(' '));
            }
            _ => {}
        }
        assert!(app.dirty(), "{name}: nothing is unsaved");
        assert_eq!(
            app.press(Key::Interrupt),
            Action::None,
            "{name}: no warning"
        );
        assert!(
            app.message.as_deref().is_some_and(|m| m.contains(WARNING)),
            "{name}: said nothing: {:?}",
            app.message
        );
        assert_eq!(
            app.press(Key::Interrupt),
            Action::Quit,
            "{name}: asked twice"
        );
    }
}

#[test]
fn ctrl_s_does_not_install() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = per_agent();
    let key = key_event(KeyCode::Char('s'), KeyModifiers::CONTROL);
    assert_ne!(app.press(key), Action::Save, "Ctrl-S installed");
}

#[test]
fn a_message_already_showing_does_not_disarm_the_unsaved_warning() {
    for source in ["restored", "a refusal", "no agent chosen", "catalog error"] {
        let mut app = unsaved();
        app.message = Some(source.to_owned());
        assert_eq!(
            app.press(Key::Char('q')),
            Action::None,
            "{source}: the first q discarded the answers"
        );
        assert!(
            app.message.as_deref().is_some_and(|m| m.contains(WARNING)),
            "{source}: the warning did not replace the message: {:?}",
            app.message
        );
        assert_ne!(app.press(Key::Char('q')), Action::None, "{source}: stuck");
    }
}

#[test]
fn any_other_key_disarms_the_unsaved_warning() {
    let mut app = unsaved();
    assert_eq!(app.press(Key::Char('q')), Action::None);
    app.press(Key::Down);
    assert_eq!(
        app.press(Key::Char('q')),
        Action::None,
        "a warning two keys ago still let this one discard"
    );
}

#[test]
fn restoring_a_row_then_leaving_with_other_answers_unsaved_still_warns() {
    let mut app = unsaved();
    walk_to(&mut app, Setting::Transitions);
    app.press(Key::Down);
    app.press(Key::Char('r'));
    assert!(app.message.is_some(), "r said nothing");
    assert_eq!(
        app.press(Key::Char('q')),
        Action::None,
        "the message r left disarmed the warning"
    );
}

#[test]
fn the_unsaved_warning_is_drawn_on_the_home_doctor_and_help_pages() {
    for page in [Page::Home, Page::Doctor, Page::Help] {
        let mut app = unsaved();
        app.page = page;
        assert_eq!(app.press(Key::Char('q')), Action::None);
        let drawn = paint(&app, 100, 30);
        assert!(
            drawn.contains(WARNING),
            "{page:?}: the warning is set and not drawn, so q reads as a dead key:\n{drawn}"
        );
    }
}

#[test]
fn an_arrow_leaves_a_typed_value_alone_on_an_open_row() {
    let mut checked = 0;
    for setting in SETTINGS {
        if setting.answers().closed {
            continue;
        }
        let app = App::new(Config::default());
        for delta in [1, -1] {
            assert_eq!(
                app.step_of(*setting, "a value nobody offers", delta),
                None,
                "{setting:?}: stepping replaced a typed value with an offered one"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "no open row, so nothing was measured");
}

#[test]
fn an_arrow_on_a_typed_tracker_keeps_what_was_typed() {
    let mut app = per_agent();
    to_options(&mut app);
    walk_to(&mut app, Setting::Tracker);
    answer(&mut app, "github acme/typed-by-hand");
    let typed = app.shown_value(Setting::Tracker);
    app.press(Key::Right);
    app.press(Key::Left);
    assert_eq!(
        app.shown_value(Setting::Tracker),
        typed,
        "an arrow replaced what was typed"
    );
}

fn two_checkouts_app() -> (App, std::collections::BTreeMap<&'static str, Config>) {
    let installed: std::collections::BTreeMap<&'static str, Config> = crate::setup::AGENTS
        .iter()
        .map(|adapter| (adapter.slug, Config::default()))
        .collect();
    let mut app = App::with_configs(&installed, &["claude-code"]);
    app.bare.clone_from(&installed);
    app.repository = "H:/here".to_owned();
    (app, installed)
}

fn layering_to_trunk(
    base: &Config,
    _repo: &std::path::Path,
) -> Result<Config, crate::outcome::Refusal> {
    let mut theirs = base.clone();
    theirs.integration = crate::config::Integration::Trunk;
    Ok(theirs)
}

#[test]
fn pointing_at_another_checkout_refuses_while_its_rows_carry_unsaved_edits() {
    let (mut app, _) = two_checkouts_app();
    app.set(Setting::Integration, "trunk")
        .expect("an integration answer");
    let before = app.configs.clone();
    let refused = super::reload_repository_with(&mut app, "H:/there", |base, _| {
        let mut theirs = base.clone();
        theirs.integration = crate::config::Integration::Branch;
        Ok(theirs)
    });
    assert!(refused.is_err(), "unsaved repository rows were replaced");
    assert_eq!(app.configs, before, "the refused reload still wrote rows");
}

#[test]
fn pointing_at_another_checkout_makes_its_rows_the_installed_ones() {
    let (mut app, _) = two_checkouts_app();
    super::reload_repository_with(&mut app, "H:/there", layering_to_trunk)
        .expect("a clean page reloads");
    for (slug, installed) in &app.installed {
        assert_eq!(
            installed.integration,
            crate::config::Integration::Trunk,
            "{slug}: installed still holds the first checkout's rows"
        );
    }
    assert!(
        !app.dirty(),
        "a reload that changed nothing the operator did reads as an unsaved edit"
    );
}

fn options_in(home: &std::path::Path) -> crate::setup::SetupOptions {
    crate::setup::SetupOptions {
        home_dir: Some(home.to_path_buf()),
        config_home: Some(home.join(".config")),
        app_data: Some(home.join("AppData").join("Roaming")),
        ..crate::setup::SetupOptions::default()
    }
}

fn guard_line_of(report: &str) -> &str {
    report
        .lines()
        .find(|line| line.contains("push-guard"))
        .unwrap_or_else(|| panic!("no push-guard line: {report}"))
        .trim_start()
}

#[test]
fn the_guard_and_the_checks_act_on_the_repository_the_screen_shows() {
    let repo = crate::test_env::repository_with_commit();
    let skill = tempfile::tempdir().expect("a skill root");
    let home = tempfile::tempdir().expect("a home");
    let (text, refusal) = super::install_guard(repo.path(), Tongue::English);
    assert!(refusal.is_none(), "{text}");
    assert_eq!(
        crate::harness::guard::state(repo.path()),
        crate::harness::guard::State::Installed,
        "the hook went to another directory: {text}"
    );
    let report = super::report_of_checks(Some(skill.path()), repo.path(), &options_in(home.path()));
    let guard_line = guard_line_of(&report);
    assert!(
        guard_line.starts_with("ok"),
        "the checks looked at the working directory, not the page's repository: {guard_line}"
    );
}

#[test]
fn a_stand_down_in_effect_makes_the_pages_push_guard_row_skipped() {
    let repo = crate::test_env::repository_with_commit();
    let skill = tempfile::tempdir().expect("a skill root");
    let home = tempfile::tempdir().expect("a home");
    let (text, refusal) = super::install_guard(repo.path(), Tongue::English);
    assert!(refusal.is_none(), "{text}");
    let now = crate::harness::session::now_seconds().expect("a readable clock");
    let record = crate::harness::standdown::declare("a migration by hand", 5, now, "someone")
        .expect("a stand-down");
    let state_root = crate::harness::session::state_root(Some(home.path())).expect("a state root");
    let file = crate::harness::standdown::path(&state_root);
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("a directory");
    std::fs::write(&file, serde_json::to_string(&record).expect("serialises"))
        .expect("the stand-down is written");
    let report = super::report_of_checks(Some(skill.path()), repo.path(), &options_in(home.path()));
    let guard_line = guard_line_of(&report);
    assert!(
        guard_line.starts_with("skipped") && guard_line.contains("standing down"),
        "the page promises a refusal the stand-down has suspended: {guard_line}"
    );
}

#[test]
fn a_broken_row_on_the_page_is_followed_by_its_resolution() {
    let resolution = crate::outcome::Resolution::run("estigia doctor --fix-me");
    let report = super::render_checks(&[
        crate::harness::doctor::Check {
            name: "gh",
            about: "a tracker CLI",
            health: crate::harness::doctor::Health::Fine {
                detail: "found".to_owned(),
            },
        },
        crate::harness::doctor::Check {
            name: "skill",
            about: "a skill",
            health: crate::harness::doctor::Health::Broken {
                detail: "missing".to_owned(),
                resolution: resolution.clone(),
            },
        },
    ]);
    let lines: Vec<&str> = report.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.trim_start().starts_with("BROKEN"))
        .unwrap_or_else(|| panic!("no broken row: {report}"));
    assert!(
        lines
            .get(at + 1)
            .is_some_and(|next| next.contains(&resolution.to_string())),
        "the broken row has no resolution under it: {report}"
    );
}

#[test]
fn the_help_pages_claim_no_interpreter_check() {
    for tongue in super::words::TONGUES {
        let lowered = tongue.say(super::HELP).to_lowercase();
        assert!(
            !lowered.contains("interpreter") && !lowered.contains("intérprete"),
            "{tongue:?} help still names an interpreter check"
        );
    }
}

#[test]
fn a_folder_walk_opens_folders_and_nothing_else() {
    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    app.press(Key::Char(' '));
    app.folders = vec![ELSEWHERE.to_owned()];
    let entries = app.picker();
    let before = app.browsing.clone();
    for (at, entry) in entries.iter().enumerate() {
        let mut probe = app.clone();
        probe.pick = at;
        probe.press(Key::Right);
        if entry == ELSEWHERE {
            assert_eq!(
                probe.browsing.as_deref(),
                Some(std::path::Path::new(ELSEWHERE))
            );
        } else {
            assert_eq!(
                probe.browsing, before,
                "→ opened {entry:?} as though it were a folder"
            );
        }
    }
}

#[test]
fn a_failed_model_catalog_is_asked_for_again() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    let mut app = App::one_table(Some(opencode), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode)
    );
    app.model_catalog_failed(opencode);
    app.press(Key::Esc);
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode),
        "a failed catalog was cached as an empty one for the session"
    );
}

#[test]
fn a_catalog_that_loaded_is_not_asked_for_again() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    let mut app = App::one_table(Some(opencode), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    app.model_catalog_loaded(opencode, vec!["zai/glm-5".to_owned()]);
    app.press(Key::Esc);
    assert_eq!(app.press(Key::Char(' ')), Action::None);
}

#[test]
fn the_single_table_screen_names_the_key_that_saves_it() {
    let mut app = App::one_table(None, Config::default());
    app.set(Setting::Delivery, "ask").expect("an answer");
    assert_eq!(app.press(Key::Char('q')), Action::None);
    let said = app.message.clone().unwrap_or_default();
    assert!(
        said.contains("s to save") && !said.contains("install"),
        "the warning names a key that installs, where s saves a table: {said}"
    );
}

#[test]
fn saving_a_single_table_nothing_changed_in_writes_nothing() {
    let mut app = App::one_table(None, Config::default());
    assert_eq!(
        app.press(Key::Char('s')),
        Action::None,
        "s asked for a write of what is already there"
    );
    assert!(app.message.is_some(), "and said nothing about it");
    app.set(Setting::Delivery, "ask").expect("an answer");
    assert_eq!(app.press(Key::Char('s')), Action::None);
    assert!(app.reviewing, "s did not ask before writing the table");
    assert_eq!(app.press(Key::Enter), Action::Save);
}

#[test]
fn the_guard_says_which_state_it_read_back_and_a_refusal_is_kept() {
    let chained = crate::test_env::repository_with_commit();
    let hooks = crate::harness::guard::hooks_directory(chained.path()).expect("a hooks directory");
    std::fs::create_dir_all(&hooks).expect("hooks");
    let hook = hooks.join("pre-push");
    std::fs::write(&hook, "#!/bin/sh\nestigia hook pre-push\n").expect("a hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
    }
    let (text, refusal) = super::install_guard(chained.path(), Tongue::English);
    assert!(refusal.is_none(), "{text}");
    assert!(
        text.contains("chained") && !text.contains("guard installed"),
        "a hook that was left alone was reported as installed: {text}"
    );

    let foreign = crate::test_env::repository_with_commit();
    let hooks = crate::harness::guard::hooks_directory(foreign.path()).expect("a hooks directory");
    std::fs::create_dir_all(&hooks).expect("hooks");
    std::fs::write(hooks.join("pre-push"), "#!/bin/sh\necho mine\n").expect("a hook");
    let (text, refusal) = super::install_guard(foreign.path(), Tongue::English);
    assert!(refusal.is_some(), "a refusal stayed in the box: {text}");

    let fresh = crate::test_env::repository_with_commit();
    let (text, refusal) = super::install_guard(fresh.path(), Tongue::English);
    assert!(refusal.is_none(), "{text}");
    assert!(
        text.contains("installed") && text.contains("read back"),
        "{text}"
    );
}

#[test]
fn restoring_a_row_the_installed_value_will_not_go_back_to_says_so() {
    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    for config in app.installed.values_mut() {
        config.worktree = crate::config::WorktreeRoot::Path("relative/dir".into());
    }
    app.press(Key::Char('r'));
    let said = app.message.clone().unwrap_or_default();
    assert!(
        !said.contains("restored"),
        "a refused restore was reported as done: {said}"
    );
    assert!(said.contains("absolute"), "{said}");
}

#[test]
fn every_install_of_a_session_is_reported_and_a_failure_does_not_hide_one() {
    let reports = vec!["first landed".to_owned(), "second landed".to_owned()];
    let text = crate::cli::guided_report(&reports, &Ok(())).expect("something to say");
    assert!(
        text.contains("first landed") && text.contains("second landed"),
        "{text}"
    );

    let failed = Err(super::io_refusal("draw", &std::io::Error::other("gone")));
    let text = crate::cli::guided_report(&reports[..1], &failed).expect("the earlier install");
    assert!(text.contains("first landed"), "{text}");
    assert!(crate::cli::guided_report(&[], &failed).is_none());
    assert_eq!(
        crate::cli::guided_report(&[], &Ok(())).as_deref(),
        Some("nothing installed")
    );
}

#[test]
fn a_new_folder_says_what_was_created() {
    let root = tempfile::tempdir().expect("a root");
    let made = root.path().join("fresh");
    let mut app = options();
    super::make_folder(&mut app, made.clone());
    assert!(made.is_dir());
    let said = app.message.clone().unwrap_or_default();
    assert!(said.contains("fresh") && said.contains("created"), "{said}");
}

#[test]
fn the_help_says_what_writes_before_s() {
    assert!(
        !super::HELP.contains("Nothing is written until `s` \u{2014}")
            && super::HELP.contains("`n` in the folder picker creates the folder on disk"),
        "the help and the lifecycle document disagree about what writes before s"
    );
    assert!(
        !super::HELP.contains("the interpreter, the tracker CLI"),
        "the help says the screen's Doctor checks what it does not"
    );
}

#[test]
fn a_guard_refusal_survives_a_later_successful_install() {
    let guard = super::io_refusal("guard", &std::io::Error::other("x"));
    assert!(super::exit_of(None, Some(guard), Ok(None)).is_err());
    assert!(super::exit_of(None, None, Ok(None)).is_ok());
}

#[test]
fn alt_gr_characters_are_not_a_ctrl_chord() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let alt_gr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    assert_eq!(super::translate(KeyCode::Char('@'), alt_gr), Key::Char('@'));
    assert_eq!(
        super::translate(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Key::Interrupt
    );
}

/// The text of every cell drawn in the selection style, by row, header excluded.
fn highlighted(app: &App, width: u16, height: u16) -> String {
    let mut app = app.clone();
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test terminal");
    terminal
        .draw(|frame| super::draw(frame, &mut app))
        .expect("the frame paints");
    let buffer = terminal.backend().buffer().clone();
    (1..height)
        .flat_map(|row| (0..width).map(move |column| (column, row)))
        .filter(|position| buffer[*position].bg == ratatui::style::Color::Cyan)
        .map(|position| buffer[position].symbol().to_owned())
        .collect()
}

/// What the cursor is on, as the first characters of the name the screen draws for it.
fn name_under_the_cursor(app: &App) -> String {
    let name = if let Some(screen) = app.screen_at_cursor() {
        screen.label(app.tongue).to_owned()
    } else if app.model_profile_at_cursor() {
        "Profile".to_owned()
    } else if let Some(target) = app.model_target_at_cursor() {
        target.name.to_owned()
    } else {
        app.tongue
            .say(app.setting_at_cursor().expect("a row").label())
            .to_owned()
    };
    name.chars().take(6).collect()
}

/// Every setting's value in every table, flattened, so two states can be told apart by what they would write.
fn written_by(app: &App) -> Vec<(&'static str, Setting, String)> {
    app.configs
        .iter()
        .flat_map(|(slug, config)| {
            SETTINGS
                .iter()
                .map(move |setting| (*slug, *setting, setting.value_of(config)))
        })
        .collect()
}

#[test]
fn from_the_menu_one_setting_of_one_installed_agent_is_applied_in_six_keys() {
    let mut app = App::with_agents(Config::default(), &["agents"]);
    let keys = [
        Key::Enter,
        Key::Down,
        Key::Enter,
        Key::Right,
        Key::Char('s'),
        Key::Enter,
    ];
    let mut last = Action::None;
    for key in keys {
        last = app.press(key);
    }
    assert_eq!(last, Action::Save, "six keys did not reach the write");
    assert_eq!(app.current(), "agents");
    assert_ne!(
        written_by(&app),
        written_by(&App::with_agents(Config::default(), &["agents"])),
        "nothing was changed on the way"
    );
    let moved: Vec<&str> = app
        .configs
        .iter()
        .filter(|(slug, config)| config.render_rows() != app.installed[**slug].render_rows())
        .map(|(slug, _)| *slug)
        .collect();
    assert_eq!(moved, ["agents"], "another agent's table moved");
}

#[test]
fn the_enter_key_opens_the_row_under_the_cursor_as_space_does() {
    for (mut app, row) in [
        (per_agent(), Setting::Delivery),
        (options(), Setting::Evidence),
    ] {
        walk_to(&mut app, row);
        let mut by_space = app.clone();
        by_space.press(Key::Char(' '));
        app.press(Key::Enter);
        assert_eq!(app.focus, Focus::Picking, "Enter did not open the row");
        assert_eq!(app.focus, by_space.focus);
        assert_eq!(app.picker(), by_space.picker());
        assert_eq!(app.pick, by_space.pick);
    }
}

#[test]
fn every_row_the_cursor_can_be_on_is_drawn_highlighted_and_is_the_only_one_a_key_changes() {
    let mut pinned = App::one_table(None, Config::default());
    pinned.set(Setting::Delivery, "ask").expect("accepted");
    let mut everyone = per_agent();
    to_everyone(&mut everyone);
    for (name, base) in [
        ("detail", per_agent()),
        ("all chosen agents", everyone),
        ("options", options()),
        ("config edit", pinned),
    ] {
        for row in 0..base.row_count() {
            let mut app = base.clone();
            for _ in 0..row {
                app.press(Key::Down);
            }
            let under = name_under_the_cursor(&app);
            let shown = highlighted(&app, 110, 60);
            assert!(
                shown.contains(&under),
                "{name}: the cursor is on {under:?} and the highlighted row says {shown:?}"
            );

            for key in [Key::Left, Key::Right, Key::Char('r')] {
                let mut pressed = app.clone();
                let before = written_by(&pressed);
                let _ = pressed.press(key);
                let allowed: Vec<Setting> = match pressed.setting_at_cursor() {
                    Some(Setting::Profile) => {
                        vec![Setting::Profile, Setting::Delivery, Setting::Transitions]
                    }
                    Some(setting @ (Setting::Delivery | Setting::Transitions)) => {
                        vec![setting, Setting::Profile]
                    }
                    Some(
                        setting @ (Setting::DeliveryPreset
                        | Setting::Workflow
                        | Setting::SelfReview
                        | Setting::ReadyBy
                        | Setting::CorrectionsAfterReady
                        | Setting::MergeBy),
                    ) => {
                        let mut preset_rows = vec![
                            Setting::DeliveryPreset,
                            Setting::SelfReview,
                            Setting::ReadyBy,
                            Setting::CorrectionsAfterReady,
                            Setting::MergeBy,
                        ];
                        if matches!(setting, Setting::DeliveryPreset | Setting::Workflow) {
                            preset_rows.push(Setting::Workflow);
                        }
                        preset_rows
                    }
                    Some(setting) => vec![setting],
                    None => vec![Setting::Models],
                };
                let after = written_by(&pressed);
                for (was, now) in before.iter().zip(&after) {
                    assert!(
                        was.2 == now.2 || allowed.contains(&was.1),
                        "{name}: {key:?} on {under:?} changed {:?} in {}",
                        was.1,
                        was.0
                    );
                }
            }
        }
    }

    let mut list = per_agent();
    list.press(Key::Esc);
    let before = (list.agents.clone(), written_by(&list));
    for key in [
        Key::Left,
        Key::Right,
        Key::Char('r'),
        Key::Char('h'),
        Key::Char('l'),
    ] {
        let _ = list.press(key);
        assert_eq!(
            (list.agents.clone(), written_by(&list)),
            before,
            "{key:?} changed something on the agents list"
        );
    }
    list.press(Key::Char(' '));
    let flipped: Vec<usize> = (0..list.agents.len())
        .filter(|at| list.agents[*at].1 != before.0[*at].1)
        .collect();
    assert_eq!(
        flipped,
        [list.agent],
        "space ticked an agent that is not highlighted"
    );
}

#[test]
fn s_shows_what_will_be_written_and_escape_writes_nothing() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    let before = app.configs.clone();

    assert_eq!(app.press(Key::Char('s')), Action::None, "s wrote at once");
    assert!(app.reviewing);
    let review = app.review();
    assert_eq!(
        review.installs.iter().map(|a| a.slug).collect::<Vec<_>>(),
        ["agents", "claude-code"],
        "the two newly ticked agents are installed"
    );
    assert_eq!(
        review.changes.len(),
        1,
        "only the edited agent has a change"
    );
    assert_eq!(review.changes[0].0.slug, "claude-code");
    assert!(review.changes[0].1.contains(&Setting::Delivery));

    let painted = paint(&app, 110, 34);
    assert!(painted.contains("install agents"), "{painted}");
    assert!(painted.contains("install claude-code"), "{painted}");
    assert!(
        painted.contains("changed in claude-code") && painted.contains("Delivery"),
        "{painted}"
    );
    assert!(painted.contains("nothing is written until you confirm"));

    assert_eq!(app.press(Key::Char('j')), Action::None);
    assert!(app.reviewing, "a stray key closed the review");
    assert_eq!(app.press(Key::Esc), Action::None, "cancelling wrote");
    assert!(!app.reviewing);
    assert_eq!(app.configs, before, "cancelling changed the tables");
    assert!(app.dirty(), "cancelling threw the answer away");
    assert_eq!(app.page, Page::Detail);

    app.press(Key::Char('s'));
    assert_eq!(app.press(Key::Interrupt), Action::None);
    assert!(!app.reviewing, "Ctrl-C left the review standing");

    app.press(Key::Char('s'));
    assert_eq!(app.press(Key::Enter), Action::Save);
    assert!(!app.reviewing);
}

#[test]
fn an_agent_that_is_installed_and_unchanged_is_not_in_the_review() {
    let mut app = App::with_agents(Config::default(), &["claude-code", "codex"]);
    assert!(app.review().is_empty());
    assert_eq!(app.press(Key::Char('s')), Action::None);
    assert!(!app.reviewing, "an empty review was laid over the screen");
    assert!(
        app.message.is_some(),
        "nothing was said about writing nothing"
    );

    app.set(Setting::Evidence, "measuring").expect("accepted");
    let review = app.review();
    assert!(review.installs.is_empty());
    assert_eq!(
        review.changes.len(),
        2,
        "a repository row reaches both tables"
    );
}

#[test]
fn after_applying_the_page_and_the_cursor_are_the_ones_it_was_asked_from() {
    let mut on_options = options();
    walk_to(&mut on_options, Setting::Evidence);
    let mut on_detail = per_agent();
    walk_to(&mut on_detail, Setting::Transitions);
    let mut on_list = per_agent();
    on_list.press(Key::Esc);
    on_list.press(Key::Down);
    for (name, mut app) in [
        ("options", on_options),
        ("detail", on_detail),
        ("agents", on_list),
    ] {
        let at = (app.page, app.cursor(), app.everyone, app.agent);
        assert_eq!(app.press(Key::Char('s')), Action::None, "{name}");
        assert_eq!(app.press(Key::Enter), Action::Save, "{name}");
        acknowledge(&mut app, "wrote it");
        app.press(Key::Char('x'));
        assert_eq!(app.modal, None, "{name}");
        assert_eq!(
            (app.page, app.cursor(), app.everyone, app.agent),
            at,
            "{name}: applying moved the screen"
        );
    }
}

#[test]
fn all_installed_agents_writes_to_every_chosen_table_and_no_other() {
    let mut app = per_agent();
    let chosen: std::collections::BTreeSet<&str> =
        app.chosen().iter().map(|adapter| adapter.slug).collect();
    to_everyone(&mut app);
    assert_eq!(app.press(Key::Char('?')), Action::None);
    app.press(Key::Char('x'));
    assert_eq!(
        app.page,
        Page::Detail,
        "help did not return to where it was opened"
    );
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "ask");
    let moved: std::collections::BTreeSet<&str> = app
        .configs
        .iter()
        .filter(|(slug, config)| config.delivery != app.installed[**slug].delivery)
        .map(|(slug, _)| *slug)
        .collect();
    assert_eq!(moved, chosen);
}

#[test]
fn escape_quits_only_from_the_menu_and_goes_back_one_level_everywhere_else() {
    let mut app = per_agent();
    assert_eq!(app.press(Key::Esc), Action::None);
    assert_eq!(app.page, Page::Agents);
    assert_eq!(app.press(Key::Esc), Action::None);
    assert_eq!(app.page, Page::Home);
    assert_eq!(
        app.press(Key::Backspace),
        Action::None,
        "Backspace quit from the menu"
    );
    assert_eq!(app.page, Page::Home);
    assert_eq!(app.press(Key::Esc), Action::Quit);

    let mut app = per_agent();
    assert_eq!(app.press(Key::Backspace), Action::None);
    assert_eq!(app.page, Page::Agents);
    to_options(&mut app);
    assert_eq!(app.press(Key::Backspace), Action::None);
    assert_eq!(app.page, Page::Home);
}

#[test]
fn help_opens_from_every_page_and_returns_to_it_with_the_cursor_where_it_was() {
    for mut app in [setup(), per_agent(), options(), App::new(Config::default())] {
        let at = (app.page, app.cursor(), app.everyone, app.agent);
        assert_eq!(app.press(Key::Char('?')), Action::None);
        assert_eq!(app.page, Page::Help);
        assert!(paint(&app, 110, 34).contains("Keys"));
        app.press(Key::Char('x'));
        assert_eq!((app.page, app.cursor(), app.everyone, app.agent), at);
    }

    let mut editing = per_agent();
    walk_to_model_target(&mut editing, "orchestrate");
    open_the_field(&mut editing);
    editing.press(Key::Char('?'));
    assert_eq!(
        editing.page,
        Page::Detail,
        "a question mark typed into a field opened help"
    );
    assert!(editing.draft.ends_with('?'));
}

mod uninstall {
    use super::*;
    use crate::harness::guard::{Removal as Taken, State};
    use crate::tui::app::{Deed, Goes, MENU, Removal};

    fn agents_with_claude_installed() -> App {
        let mut app = App::with_agents(Config::default(), &["claude-code"]);
        app.page = Page::Agents;
        walk_agents_to(&mut app, "claude-code", Key::Down);
        app
    }

    fn refusal() -> crate::outcome::Refusal {
        super::super::io_refusal("uninstall", &std::io::Error::other("x"))
    }

    #[test]
    fn u_on_an_installed_agent_asks_what_would_go_and_writes_nothing() {
        let mut app = agents_with_claude_installed();
        assert_eq!(
            app.press(Key::Char('u')),
            Action::PreviewUninstall(Removal::Agent("claude-code"))
        );
        assert!(app.removing.is_none());
        assert_eq!(app.installed_agents, vec!["claude-code"]);
    }

    #[test]
    fn u_does_nothing_on_an_agent_that_is_not_installed_or_on_all_chosen_agents() {
        let mut app = agents_with_claude_installed();
        walk_agents_to(&mut app, "codex", Key::Down);
        assert_eq!(app.press(Key::Char('u')), Action::None);
        let mut everyone = agents_with_claude_installed();
        everyone.everyone = true;
        assert_eq!(everyone.press(Key::Char('u')), Action::None);
    }

    #[test]
    fn the_footer_offers_u_only_on_an_installed_agent() {
        let mut app = agents_with_claude_installed();
        let offered = |app: &App| {
            super::super::keys_for(app)
                .iter()
                .any(|k| k.starts_with("u "))
        };
        assert!(offered(&app));
        walk_agents_to(&mut app, "codex", Key::Down);
        assert!(!offered(&app));
        assert_eq!(
            super::super::keys_for(&app).last().map(String::as_str),
            Some("s apply")
        );
    }

    #[test]
    fn enter_confirms_and_esc_cancels_without_changing_anything() {
        let mut app = agents_with_claude_installed();
        app.removal_asked(Removal::Agent("claude-code"), vec!["/x".to_owned()]);
        assert_eq!(app.press(Key::Char('j')), Action::None);
        assert!(app.removing.is_some());
        assert_eq!(app.press(Key::Esc), Action::None);
        assert!(app.removing.is_none());
        assert_eq!(app.installed_agents, vec!["claude-code"]);
        app.removal_asked(Removal::Agent("claude-code"), vec!["/x".to_owned()]);
        assert_eq!(
            app.press(Key::Enter),
            Action::Uninstall(Removal::Agent("claude-code"))
        );
        assert!(app.removing.is_none());
    }

    #[test]
    fn the_installed_state_follows_the_read_back_only() {
        let mut app = agents_with_claude_installed();
        app.agent_read_back("claude-code", true);
        assert_eq!(app.installed_agents, vec!["claude-code"]);
        app.agent_read_back("claude-code", false);
        assert!(app.installed_agents.is_empty());
        assert!(app.chosen().is_empty());
    }

    #[test]
    fn an_agent_that_still_reads_as_installed_is_never_reported_removed() {
        let (text, kept) = super::super::agent_removal_text(
            Tongue::English,
            "codex",
            3,
            &Installed::Configured,
            None,
        );
        assert!(kept.is_some() && !text.contains("uninstalled:"), "{text}");
        let (text, kept) = super::super::agent_removal_text(
            Tongue::English,
            "codex",
            1,
            &Installed::Absent,
            Some(refusal()),
        );
        assert!(kept.is_some() && !text.contains("uninstalled:"), "{text}");
        let (text, kept) =
            super::super::agent_removal_text(Tongue::English, "codex", 3, &Installed::Absent, None);
        assert!(
            kept.is_none() && text.contains("read back as not installed"),
            "{text}"
        );
    }

    #[test]
    fn a_read_back_that_could_not_read_is_a_refusal_naming_the_failure() {
        let unknown = Installed::Unknown("/home/x/CLAUDE.md: Permission denied".to_owned());
        let (text, kept) =
            super::super::agent_removal_text(Tongue::English, "codex", 3, &unknown, None);
        let kept = kept.expect("an unread read-back is a refusal");
        assert!(text.contains("Permission denied"), "{text}");
        assert!(!text.contains("read back as not installed"), "{text}");
        assert_eq!(kept.outcome, crate::outcome::MutationOutcome::Unknown);
        let (text, kept) = super::super::agent_removal_text(
            Tongue::English,
            "codex",
            1,
            &unknown,
            Some(refusal()),
        );
        assert!(kept.is_some(), "{text}");
        assert!(text.contains("Permission denied"), "{text}");
        assert!(!text.contains("its block is gone"), "{text}");
    }

    #[test]
    fn an_unread_read_back_leaves_the_agent_listed() {
        let mut app = agents_with_claude_installed();
        let unknown = Installed::Unknown("denied".to_owned());
        let (_, kept) = super::super::agent_removed(&mut app, "claude-code", 2, &unknown, None);
        assert!(kept.is_some());
        assert_eq!(app.installed_agents, vec!["claude-code"]);
        let (_, kept) =
            super::super::agent_removed(&mut app, "claude-code", 2, &Installed::Absent, None);
        assert!(kept.is_none());
        assert!(app.installed_agents.is_empty());
    }

    #[test]
    fn a_failed_removal_survives_a_later_successful_install() {
        let (_, kept) = super::super::agent_removal_text(
            Tongue::English,
            "codex",
            0,
            &Installed::Configured,
            Some(refusal()),
        );
        assert!(super::super::exit_of(None, kept, Ok(None)).is_err());
    }

    #[test]
    fn the_guard_is_removed_only_when_the_hook_reads_back_absent() {
        let repo = std::path::Path::new("/repo");
        let said = |taken, after| {
            super::super::guard_removal_outcome(Tongue::English, repo, taken, &after)
        };
        assert!(said(Ok(Taken::Taken), State::Absent).1.is_none());
        assert!(said(Ok(Taken::Taken), State::Installed).1.is_some());
        assert!(said(Err(refusal()), State::Absent).1.is_some());
        assert!(said(Ok(Taken::NothingThere), State::Absent).1.is_none());
        let (text, refusal) = said(Ok(Taken::LeftAlone(State::Chained)), State::Chained);
        let refusal = refusal.expect("a hook left alone is a refusal");
        assert_ne!(refusal.code, "push-guard-removal-not-read-back");
        assert_eq!(refusal.outcome, crate::outcome::MutationOutcome::NotStarted);
        assert!(text.contains("left alone") && text.contains("nothing was removed"));
        assert!(!text.contains("was removed from"));
    }

    #[test]
    fn the_text_pages_do_what_the_help_says_their_keys_do() {
        for from in [Page::Help, Page::Doctor] {
            let mut app = per_agent();
            app.press(Key::Esc);
            app.page = from;
            assert_eq!(app.press(Key::Char('q')), Action::Quit, "{from:?} q");
            let mut app = per_agent();
            app.page = from;
            app.press(Key::Char('s'));
            assert!(app.reviewing, "{from:?} s");
            let mut app = per_agent();
            app.page = from;
            app.press(Key::Down);
            assert_eq!(app.scroll, 1, "{from:?} down");
            app.press(Key::Up);
            assert_eq!(app.scroll, 0, "{from:?} up");
        }
        let mut app = per_agent();
        app.page = Page::Doctor;
        app.press(Key::Char('?'));
        assert_eq!(app.page, Page::Help);
        app.press(Key::Char('?'));
        assert_eq!(app.page, Page::Doctor);
        app.press(Key::Char('x'));
        assert_eq!(app.page, Page::Home);
    }

    fn on_the_guard_entry(state: crate::harness::guard::State) -> App {
        let mut app = App::with_agents(Config::default(), &[]);
        app.guard_state = state;
        while app.entry().goes != Goes::Doing(Deed::Guard) {
            app.press(Key::Down);
        }
        app
    }

    #[test]
    fn the_guard_footer_hint_shows_only_where_uninstall_would_remove_something() {
        use crate::harness::guard::State;
        for (state, offered) in [
            (State::Installed, true),
            (State::Inert, true),
            (State::Absent, false),
            (State::Foreign, false),
            (State::Chained, false),
            (State::Unreadable, false),
        ] {
            let app = on_the_guard_entry(state.clone());
            assert_eq!(
                paint(&app, 100, 30).contains("u uninstall"),
                offered,
                "{state:?}"
            );
        }
    }

    #[test]
    fn u_on_the_guard_entry_asks_only_where_uninstall_would_remove_something() {
        use crate::harness::guard::State;
        for (state, offered) in [
            (State::Installed, true),
            (State::Inert, true),
            (State::Absent, false),
            (State::Foreign, false),
            (State::Chained, false),
            (State::Unreadable, false),
        ] {
            let mut app = App::with_agents(Config::default(), &[]);
            app.guard_state = state.clone();
            for _ in 0..MENU.len() {
                let guard = app.entry().goes == Goes::Doing(Deed::Guard);
                let pressed = app.press(Key::Char('u'));
                assert_eq!(
                    pressed == Action::PreviewUninstall(Removal::Guard),
                    guard && offered,
                    "{state:?}"
                );
                app.press(Key::Down);
            }
        }
    }

    #[test]
    fn the_menu_marks_a_guard_that_is_inert_or_unreadable() {
        use crate::harness::guard::State;
        for (state, word) in [(State::Inert, "inert"), (State::Unreadable, "unreadable")] {
            let app = on_the_guard_entry(state.clone());
            assert!(paint(&app, 100, 30).contains(word), "{state:?}");
        }
        for state in [
            State::Installed,
            State::Absent,
            State::Foreign,
            State::Chained,
        ] {
            let app = on_the_guard_entry(state.clone());
            let painted = paint(&app, 100, 30);
            assert!(
                !painted.contains("inert") && !painted.contains("unreadable"),
                "{state:?}"
            );
        }
    }

    #[test]
    fn the_guard_marks_are_translated() {
        for text in ["inert", "unreadable"] {
            assert_ne!(Tongue::Spanish.say(text), text, "{text}");
        }
    }

    #[test]
    fn every_uninstall_sentence_is_translated() {
        for text in [
            "u uninstall",
            "⏎ remove it",
            "Uninstall",
            "the push guard",
            "read back: still installed",
        ] {
            assert_ne!(Tongue::Spanish.say(text), text, "{text}");
        }
    }
}

fn cat_rows_shown(app: &App, width: u16, height: u16) -> usize {
    let painted = rows(app, width, height);
    super::art::TANTAN
        .iter()
        .filter(|row| painted.iter().any(|line| line.contains(*row)))
        .count()
}

fn menu_labels_survive(painted: &str) {
    for label in ["Agents", "Options", "Push guard", "Doctor", "Help", "Quit"] {
        assert!(painted.contains(label), "`{label}` is gone from the menu");
    }
}

#[test]
fn a_large_terminal_draws_tantan_beside_the_menu() {
    let app = App::new(Config::default());
    assert_eq!(
        cat_rows_shown(&app, 130, 50),
        super::art::TANTAN.len(),
        "Tantan is not drawn whole"
    );
    menu_labels_survive(&paint(&app, 130, 50));
}

#[test]
fn a_terminal_too_small_for_tantan_keeps_the_wordmark_and_the_menu() {
    let app = App::new(Config::default());
    for (width, height) in [(100, 30), (130, 30), (44, 14)] {
        assert_eq!(
            cat_rows_shown(&app, width, height),
            0,
            "Tantan drawn at {width}x{height}"
        );
        menu_labels_survive(&paint(&app, width, height));
    }
    let painted = paint(&app, 100, 30);
    assert!(
        rows(&app, 100, 30)
            .iter()
            .any(|row| row.contains(" ####  ####")),
        "the wordmark did not stand in: {painted}"
    );
}

#[test]
fn tantan_is_banded_from_top_to_bottom() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    let mut app = App::new(Config::default());
    let mut terminal = Terminal::new(TestBackend::new(130, 50)).expect("a test terminal");
    terminal
        .draw(|frame| super::draw(frame, &mut app))
        .expect("the frame paints");
    let buffer = terminal.backend().buffer().clone();
    let inked: Vec<(u16, Color)> = (0..50)
        .flat_map(|row| (0..130).map(move |column| (column, row)))
        .filter(|(column, row)| {
            let symbol = buffer[(*column, *row)].symbol();
            symbol
                .chars()
                .next()
                .is_some_and(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
        })
        .map(|(column, row)| (row, buffer[(column, row)].fg))
        .collect();
    let colours: std::collections::HashSet<String> = inked
        .iter()
        .map(|(_, colour)| format!("{colour:?}"))
        .collect();
    assert!(colours.len() >= 3, "not banded: {colours:?}");
    assert!(
        inked
            .iter()
            .all(|(_, colour)| matches!(colour, Color::Rgb(..)))
    );
    let top = inked.first().expect("some ink").1;
    let bottom = inked.last().expect("some ink").1;
    assert_ne!(top, bottom, "the top and the bottom share a colour");
}

#[test]
fn tantan_stops_short_of_the_footer_at_the_smallest_size_that_shows_it() {
    let mut app = App::new(Config::default());
    app.message = Some("a message that takes a line of the footer".to_owned());
    let (width, height) = (60..200u16)
        .flat_map(|width| (20..80u16).map(move |height| (width, height)))
        .filter(|(width, height)| cat_rows_shown(&app, *width, *height) > 0)
        .min_by_key(|(width, height)| (u32::from(*height), u32::from(*width)))
        .expect("Tantan is drawn at no size");
    assert_eq!(
        cat_rows_shown(&app, width, height),
        super::art::TANTAN.len(),
        "Tantan is clipped at {width}x{height}"
    );
    let painted = rows(&app, width, height);
    let keys = painted
        .iter()
        .position(|row| row.contains("quit"))
        .expect("the key hints are gone");
    let last_ink = painted
        .iter()
        .rposition(|row| row.chars().any(|c| ('\u{2801}'..='\u{28FF}').contains(&c)))
        .expect("no ink");
    let message = painted
        .iter()
        .position(|row| row.contains("a message that takes"))
        .expect("the message is gone");
    assert!(last_ink < message && message < keys, "{painted:#?}");
    let footer_top = message - 1;
    let stage = footer_top - 1;
    let air = footer_top - last_ink - 1;
    assert_eq!(
        air,
        (stage - super::art::TANTAN.len()).div_ceil(2),
        "the air above the footer is not the drawing centred in its stage at {width}x{height}"
    );
    assert!(air >= 2, "Tantan crowds the footer at {width}x{height}");
}

#[test]
fn tantan_gives_way_to_the_wordmark_one_row_below_the_height_that_shows_it() {
    let app = App::new(Config::default());
    assert_eq!(
        cat_rows_shown(&app, 130, 43),
        super::art::TANTAN.len(),
        "Tantan is not drawn at the measured minimum height"
    );
    assert_eq!(
        cat_rows_shown(&app, 130, 42),
        0,
        "Tantan is drawn below the minimum"
    );
    assert!(
        rows(&app, 130, 42)
            .iter()
            .any(|row| row.contains(" ####  ####")),
        "the wordmark did not stand in"
    );
    menu_labels_survive(&paint(&app, 130, 42));
}

fn waiting_on_the_checks() -> App {
    let mut app = options();
    app.page = Page::Home;
    app.entry = super::app::MENU
        .iter()
        .position(|entry| entry.goes == super::app::Goes::Doing(super::app::Deed::Doctor))
        .expect("a Doctor entry");
    assert_eq!(app.press(Key::Enter), Action::Doctor);
    app
}

#[test]
fn keys_pressed_while_the_checks_run_change_neither_the_page_nor_the_report() {
    let mut app = waiting_on_the_checks();
    assert!(app.is_busy());
    assert_eq!(app.page, Page::Doctor);
    let waiting = app.report.clone();
    assert!(waiting.is_some());
    for key in [
        Key::Enter,
        Key::Enter,
        Key::Esc,
        Key::Backspace,
        Key::Char(' '),
        Key::Char('?'),
        Key::Char('s'),
        Key::Down,
        Key::Other,
    ] {
        assert_eq!(app.press(key), Action::None);
        assert_eq!(app.page, Page::Doctor, "{key:?} left the page");
        assert_eq!(app.report, waiting, "{key:?} changed the report");
    }
    app.doctor_finished("ok skill".to_owned());
    assert!(!app.is_busy());
    assert_eq!(app.page, Page::Doctor);
    assert_eq!(app.report.as_deref(), Some("ok skill"));
    assert_eq!(app.press(Key::Enter), Action::None);
    assert_eq!(
        app.page,
        Page::Home,
        "a finished report no longer returns on a key"
    );
}

#[test]
fn quit_and_interrupt_leave_while_the_checks_run() {
    for key in [Key::Char('q'), Key::Interrupt] {
        let mut app = waiting_on_the_checks();
        assert_eq!(app.press(key), Action::Quit, "{key:?} did not leave");
    }
}

#[test]
fn keys_pressed_while_a_catalog_loads_are_not_kept_for_the_picker() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    let mut app = App::one_table(Some(opencode), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode)
    );
    assert!(app.is_busy());
    let before = (app.page, app.pick, app.focus);
    for key in [Key::Enter, Key::Down, Key::Esc, Key::Char(' ')] {
        assert_eq!(app.press(key), Action::None);
        assert_eq!(
            (app.page, app.pick, app.focus),
            before,
            "{key:?} moved the picker"
        );
    }
    app.model_catalog_loaded(opencode, vec!["zai/glm-5".to_owned()]);
    assert!(!app.is_busy());
}

#[test]
fn the_folder_picker_measures_wide_names_in_columns() {
    let last = "目录目录目录目录目录目录";
    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    app.press(Key::Char(' '));
    app.browsing = Some(std::path::PathBuf::from("/srv/projects").join(last));
    app.folders = vec!["文件夹文件夹".to_owned()];

    let painted: Vec<String> = rows(&app, 40, 34)
        .into_iter()
        .map(|row| row.split_whitespace().collect())
        .collect();
    let top = painted
        .iter()
        .find(|row| row.contains('\u{256d}') && row.contains(last))
        .unwrap_or_else(|| {
            panic!(
                "the cut path lost the directory's own last component:\n{}",
                painted.join("\n")
            )
        });
    assert!(
        top.contains('\u{256e}'),
        "the box overflows its area, its corner is gone: {top}"
    );
    assert!(
        painted.iter().any(|row| row.contains("文件夹文件夹")),
        "the folder entry is not drawn whole:\n{}",
        painted.join("\n")
    );
}

#[test]
fn the_keys_never_exceed_the_window_in_display_columns() {
    let hints = ["回车 确认", "退出 离开", "选择 文件夹", "帮助 说明"];
    for width in 1..=60u16 {
        let shown = super::fit_keys(&hints, width);
        assert!(
            super::display_width(&shown) <= usize::from(width),
            "{shown:?} is wider than {width} columns"
        );
    }
}

#[test]
fn ascii_and_spanish_text_measures_as_it_always_did() {
    assert_eq!(super::display_width("configuración"), 13);
    assert_eq!(super::clipped("configuración", 13), "configuración");
    assert_eq!(super::clipped("configuración", 6), "confi\u{2026}");
    assert_eq!(super::clipped("abc", 1), "a");
    assert_eq!(super::fit_keys(&["a", "b", "c"], 20), "a   b   c");
}

#[derive(Default)]
struct CountingWorld {
    repositories: usize,
    folders: Vec<std::path::PathBuf>,
}

impl super::app::World for CountingWorld {
    fn repositories(&mut self) -> Vec<String> {
        self.repositories += 1;
        vec!["H:/one".to_owned(), "H:/two".to_owned()]
    }

    fn folders(&mut self, root: &std::path::Path) -> Vec<String> {
        self.folders.push(root.to_path_buf());
        vec![format!("{}/child", root.display())]
    }

    fn drives(&mut self) -> Vec<String> {
        Vec::new()
    }
}

#[test]
fn the_checkouts_are_read_once_per_opening_of_the_picker_not_once_per_key() {
    let mut app = options();
    let mut reads = super::app::Reads::default();
    let mut world = CountingWorld::default();

    while app.screen_at_cursor() != Some(super::app::Screen::Repository) {
        reads.refresh(&mut app, &mut world);
        app.press(Key::Down);
    }
    assert_eq!(
        world.repositories, 0,
        "the checkouts were read while no row could offer them"
    );

    reads.refresh(&mut app, &mut world);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    for _ in 0..4 {
        reads.refresh(&mut app, &mut world);
        app.press(Key::Down);
    }
    assert_eq!(
        world.repositories, 1,
        "the checkouts were read again on a key inside one opening"
    );
    assert_eq!(app.picker(), vec!["H:/one".to_owned(), "H:/two".to_owned()]);

    app.press(Key::Esc);
    assert_eq!(app.focus, Focus::List);
    reads.refresh(&mut app, &mut world);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    assert_eq!(
        world.repositories, 2,
        "reopening the picker on the same row did not read the checkouts again"
    );
}

fn repositories_read_after(action: &Action) -> usize {
    let mut app = options();
    let mut reads = super::app::Reads::default();
    let mut world = CountingWorld::default();
    while app.screen_at_cursor() != Some(super::app::Screen::Repository) {
        app.press(Key::Down);
    }
    reads.refresh(&mut app, &mut world);
    reads.refresh(&mut app, &mut world);
    assert_eq!(world.repositories, 1);
    reads.performed(action);
    reads.refresh(&mut app, &mut world);
    world.repositories
}

#[test]
fn a_reload_and_a_made_folder_forget_what_they_made_stale() {
    assert_eq!(
        repositories_read_after(&Action::Reload("H:/two".to_owned())),
        2,
        "a reload did not make the checkouts read again"
    );
    assert_eq!(
        repositories_read_after(&Action::Remember),
        1,
        "remembering the language made the checkouts read again"
    );
    assert_eq!(
        repositories_read_after(&Action::None),
        1,
        "an action that changed nothing made the checkouts read again"
    );

    let mut app = options();
    walk_to(&mut app, Setting::Worktree);
    let mut reads = super::app::Reads::default();
    let mut world = CountingWorld::default();
    reads.refresh(&mut app, &mut world);
    reads.performed(&Action::None);
    reads.refresh(&mut app, &mut world);
    assert_eq!(world.folders.len(), 1);
    reads.performed(&Action::MakeFolder(std::path::PathBuf::from("H:/new")));
    reads.refresh(&mut app, &mut world);
    assert_eq!(world.folders.len(), 2, "a made folder would not show");
}

#[test]
fn the_folders_are_read_only_when_the_browsed_directory_changes() {
    crate::test_env::with_environment_read_locked(|| {
        let mut app = options();
        walk_to(&mut app, Setting::Worktree);
        let mut reads = super::app::Reads::default();
        let mut world = CountingWorld::default();

        for _ in 0..5 {
            reads.refresh(&mut app, &mut world);
        }
        assert_eq!(world.folders.len(), 1, "the same directory was read again");

        let root = tempfile::tempdir().expect("a directory");
        app.set(Setting::Worktree, &root.path().display().to_string())
            .expect("an absolute path");
        reads.refresh(&mut app, &mut world);
        reads.refresh(&mut app, &mut world);
        assert_eq!(
            world.folders.len(),
            2,
            "a new root was not read exactly once"
        );
        assert_eq!(world.folders[1], root.path());
        assert_eq!(
            app.folders,
            vec![format!("{}/child", root.path().display())]
        );

        reads.performed(&Action::MakeFolder(root.path().join("new")));
        reads.refresh(&mut app, &mut world);
        assert_eq!(world.folders.len(), 3, "a made folder would not show");
    });
}

#[test]
fn a_checks_worker_that_died_clears_the_busy_state_with_a_report() {
    for (tongue, expected) in [
        (
            crate::tui::words::Tongue::English,
            "the checks did not finish",
        ),
        (
            crate::tui::words::Tongue::Spanish,
            "las comprobaciones no terminaron",
        ),
    ] {
        let mut app = waiting_on_the_checks();
        app.tongue = tongue;
        app.worker_died();
        assert!(!app.is_busy());
        assert_eq!(app.page, Page::Doctor);
        assert!(
            app.report.as_deref().is_some_and(|r| r.contains(expected)),
            "{:?}",
            app.report
        );
    }
}

#[test]
fn a_checks_worker_that_panics_reaches_the_screen_as_a_report_not_a_stuck_wait() {
    let (sender, answers) = std::sync::mpsc::channel();
    super::spawn_worker(sender, || panic!("the checks blew up"));
    let done = answers
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("a panicking worker still answers");
    let mut app = waiting_on_the_checks();
    assert!(app.is_busy());
    super::deliver(&mut app, done);
    assert!(!app.is_busy());
    assert!(
        app.report
            .as_deref()
            .is_some_and(|r| r.contains("the checks did not finish")),
        "{:?}",
        app.report
    );
}

#[test]
fn a_catalog_worker_that_died_says_so_in_both_tongues() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    for (tongue, expected) in [
        (Tongue::English, "model catalog did not finish loading"),
        (Tongue::Spanish, "no terminó de cargar"),
    ] {
        let mut app = App::one_table(Some(opencode), Config::default());
        app.tongue = tongue;
        walk_to_model_target(&mut app, "orchestrate");
        app.press(Key::Char(' '));
        app.worker_died();
        assert!(
            app.message.as_deref().is_some_and(|m| m.contains(expected)),
            "{:?}",
            app.message
        );
    }
}

#[test]
fn a_catalog_worker_that_died_clears_the_busy_state_and_is_asked_for_again() {
    let opencode = crate::setup::find_agent("opencode").expect("the OpenCode adapter");
    let mut app = App::one_table(Some(opencode), Config::default());
    walk_to_model_target(&mut app, "orchestrate");
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode)
    );
    assert!(app.is_busy());
    app.worker_died();
    assert!(!app.is_busy());
    assert!(
        app.message
            .as_deref()
            .is_some_and(|m| m.contains("did not finish")),
        "{:?}",
        app.message
    );
    app.press(Key::Esc);
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::LoadModelCatalog(opencode)
    );
}

#[test]
fn a_worker_that_died_with_nothing_running_changes_nothing() {
    let mut app = options();
    let before = app.report.clone();
    app.worker_died();
    assert!(!app.is_busy());
    assert_eq!(app.report, before);
}

fn route_of(agent: &str, route: &str) -> App {
    let mut app = per_agent_with(&[agent]);
    app.configs
        .get_mut(agent)
        .expect("the agent's table")
        .models = crate::config::ModelRouting::parse(route).expect("a model route");
    app
}

#[test]
fn a_stored_model_that_belongs_to_another_host_is_advised_on_the_agent_page() {
    let mut app = route_of(
        "codex",
        "developer=sonnet, analyst=gpt-5.5, judge=my-fine-tune",
    );

    walk_to_model_target(&mut app, "developer");
    let screen = paint(&app, 140, 50);
    assert!(
        screen.contains("Claude Code") && screen.contains("does not list it"),
        "a Claude model stored for Codex went unremarked:\n{screen}"
    );
    assert_eq!(
        app.configs["codex"].models.for_target("developer"),
        Some("sonnet"),
        "the advisory changed the stored value"
    );

    for own in ["analyst", "judge"] {
        walk_to_model_target(&mut app, own);
        assert!(
            !paint(&app, 140, 50).contains("does not list it"),
            "{own} was advised although no other host lists it"
        );
    }
}

#[test]
fn the_advisory_is_said_in_both_languages() {
    let mut app = route_of("codex", "developer=sonnet");
    walk_to_model_target(&mut app, "developer");
    app.tongue = crate::tui::words::Tongue::Spanish;
    let screen = paint(&app, 140, 50);
    assert!(
        screen.contains("no lo lista"),
        "the advisory has no Spanish:\n{screen}"
    );
}

#[test]
fn the_every_agent_page_advises_before_one_id_goes_to_agents_with_different_catalogs() {
    let mut app = per_agent_with(&["claude-code", "codex"]);
    app.everyone = true;
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    for _ in 0..app.draft.chars().count() {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "gpt-5.5");
    app.press(Key::Enter);

    for agent in ["claude-code", "codex"] {
        assert_eq!(
            app.configs[agent].models.for_target("orchestrate"),
            Some("gpt-5.5"),
            "{agent} was refused the value"
        );
    }
    let message = app.message.clone().unwrap_or_default();
    assert!(
        message.contains("Claude Code") && message.contains("Codex"),
        "no advisory named the hosts: {message:?}"
    );

    let mut app = per_agent_with(&["claude-code", "codex"]);
    app.everyone = true;
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    choose_picker_entry(&mut app, "type a model ID…");
    for _ in 0..app.draft.chars().count() {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "my-fine-tune");
    app.press(Key::Enter);
    assert_eq!(app.message, None, "an ID in no catalog was advised");
}

#[test]
fn the_delivery_preset_fills_its_rows_on_the_screen_and_a_fixed_row_says_why() {
    let mut app = App::one_table(None, Config::default());
    walk_to(&mut app, Setting::ReadyBy);
    let painted = rows(&app, 110, 60).join("\n");
    assert!(
        painted.contains("fixed while `Claim workflow` is `autonomous`"),
        "a row autonomous fixes is offered as if it could be chosen:\n{painted}"
    );
    let preset_at = painted
        .find("Delivery preset")
        .expect("the preset is drawn");
    let workflow_at = painted
        .find("Claim workflow ")
        .expect("Claim workflow is drawn");
    let merge_at = painted.find("Merge by").expect("Merge by is drawn");
    assert!(
        preset_at < workflow_at && workflow_at < merge_at,
        "{painted}"
    );

    app.set(Setting::DeliveryPreset, "team").expect("team");
    for (setting, value) in [
        (Setting::Workflow, "assisted"),
        (Setting::SelfReview, "on"),
        (Setting::ReadyBy, "agent"),
        (Setting::CorrectionsAfterReady, "push-ready"),
        (Setting::MergeBy, "person"),
    ] {
        assert_eq!(app.shown_value(setting), value, "{}", setting.label());
    }
    let painted = rows(&app, 110, 60).join("\n");
    assert!(!painted.contains("fixed while"), "{painted}");

    app.set(Setting::SelfReview, "off").expect("off");
    assert_eq!(app.shown_value(Setting::DeliveryPreset), "custom");
    assert_eq!(app.shown_value(Setting::MergeBy), "person");
}
