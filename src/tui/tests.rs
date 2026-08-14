use super::app::{Action, App, Focus, Key, Page, STEPS, Step, TYPE_IT};
use crate::config::{
    AGENT_SETTINGS, Config, EVERYWHERE_SETTINGS, OPTIONS_SETTINGS, SETTINGS, Setting,
};

/// Absolute paths, spelled for the platform running the test.
///
/// `H:/somewhere/else` is absolute on Windows and a **relative** path
/// everywhere else. The `Worktree` row requires an absolute path and refused
/// it, so two tests panicked in `expect` on Linux and macOS while passing here.
/// The row was right; the fixtures were Windows-shaped, and a fixture that only
/// spells one platform's paths measures one platform.
///
/// Only values that go through the row's validation need these. A path that is
/// merely an item in a list never reaches it and stays as it was written.
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
        .apply(&mut claude_config, "linear")
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
    Setting::Merge
        .apply(&mut repository, "squash")
        .expect("the repository merge strategy is accepted");

    let mut receipt = super::InstallReceipt::empty("repository saved".to_owned());
    receipt.repository = Some(repository);
    receipt.repository_settings = vec![Setting::Merge];
    app.installed_partially(receipt);

    assert_eq!(
        Setting::Merge.value_of(&app.installed[claude.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Merge.value_of(&app.installed[codex.slug]),
        "squash"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[claude.slug]),
        "linear"
    );
    assert_eq!(
        Setting::Tracker.value_of(&app.installed[codex.slug]),
        "github acme/issues",
        "the repository receipt copied Claude's inherited Tracker onto Codex"
    );
}

/// The setup page, on its first step.
///
/// The screen opens on the **menu** — a tool whose first screen is one of its
/// jobs teaches an operator that job is the tool, and the rest goes unfound.
fn setup() -> App {
    let mut app = App::new(Config::default());
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app
}

/// The per-agent step, with two agents ticked.
fn per_agent() -> App {
    let mut app = setup();
    app.press(Key::Char(' '));
    app.press(Key::Down);
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    // And on into the rows. The step opens on the panel that asks *who this
    // answers for*, because that is the first decision — but almost every test
    // below is about answering a row, so they start where they mean to be.
    app.press(Key::Tab);
    assert_eq!(app.panel, super::app::Panel::Rows);
    app
}

/// The per-agent step with an explicit set of already configured adapters.
fn per_agent_with(configured: &[&str]) -> App {
    let mut app = App::with_agents(Config::default(), configured);
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    app.press(Key::Tab);
    assert_eq!(app.panel, super::app::Panel::Rows);
    app
}

/// The options page, with the same two agents ticked.
///
/// Reached the way an operator reaches it — out to the menu and down to the
/// entry — rather than by assigning `page`, because "the entry is on the menu
/// and it opens the rows" is half of what this page had to become.
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

/// Walks back to the setup page, on its first question.
fn to_setup(app: &mut App) {
    to_menu(app);
    walk_the_menu_to(app, Page::Setup);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Setup);
}

/// Out to the menu, pressing Esc twice where there is something unsaved.
///
/// The first press warns rather than discarding, which is the screen doing its
/// job — not the walk failing.
fn to_menu(app: &mut App) {
    for _ in 0..2 {
        if app.page == Page::Home {
            return;
        }
        app.press(Key::Esc);
    }
    assert_eq!(app.page, Page::Home, "the walk never reached the menu");
}

/// Presses `Enter` until the named step is showing.
///
/// `Enter` is *accept*, and accepting a step is what moves to the next one.
/// Written as "until" rather than as a count so a step that gains a question
/// does not break every test that walks past it.
fn to_step(app: &mut App, wanted: Step) {
    for _ in 0..=STEPS.len() * 3 {
        if app.step == wanted {
            return;
        }
        app.press(Key::Enter);
    }
    panic!("`Enter` never reached {wanted:?}");
}

/// Presses `a` until the cursor is on a different agent.
///
/// `a` walks one stop, and the stops are *every agent* and then each chosen one
/// — so from the last agent it lands on the shared answer rather than wrapping
/// straight round. Written as "until it changes" so these tests say what they
/// mean, which is *another agent*, and stop depending on how many stops the
/// cycle happens to have.
fn to_another_agent(app: &mut App) {
    let from = app.current();
    for _ in 0..=crate::setup::AGENTS.len() + 1 {
        app.press(Key::Char('a'));
        if app.current() != from {
            return;
        }
    }
    panic!("`a` never reached an agent other than {from}");
}

/// Presses `A` until the cursor is back on a named agent.
///
/// The mirror of [`to_another_agent`], and it exists for the same reason: one
/// stop back from an agent is the shared answer, not the agent before it.
fn back_to_agent(app: &mut App, wanted: &str) {
    for _ in 0..=crate::setup::AGENTS.len() + 1 {
        if app.current() == wanted {
            return;
        }
        app.press(Key::Char('A'));
    }
    panic!("`A` never came back to {wanted}");
}

/// Moves the cursor onto one setting, or says which one it could not reach.
///
/// Bounded. Written as `while app.setting() != wanted { app.press(Down) }`,
/// every one of these walks turned a wrong assumption into a **hang**: the day
/// `Renewal window` moved from the per-agent step to the repository rows, two
/// tests span forever instead of failing, and a suite that hangs costs far more
/// to read than one that fails. There are only ever `SETTINGS.len()` rows.
/// Presses `Down` on the menu until the entry that opens `wanted` is under the
/// cursor.
///
/// Bounded, like every walk here. `walk_to` learned it first and said why;
/// these five did not, and the cost was measured: mutating `pinned` to `true`
/// made the menu never reach the page these look for, and the run **hung** for
/// as long as it was left. A hang is the one answer a suite cannot give — in CI
/// it is a timeout with no failing test named, and locally it is a machine that
/// looks busy.
fn walk_the_menu_to(app: &mut App, wanted: Page) {
    for _ in 0..=super::app::MENU.len() {
        if app.entry().goes == super::app::Goes::To(wanted) {
            return;
        }
        app.press(Key::Down);
    }
    panic!("`Down` never reached the entry that opens {wanted:?}");
}

/// Presses `key` until the cursor is on `wanted`.
fn walk_agents_to(app: &mut App, wanted: &str, key: Key) {
    for _ in 0..=crate::setup::AGENTS.len() * 2 {
        if app.current() == wanted {
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
        "{:?} is not on the {:?} step, so this walk would never have ended",
        wanted, app.step
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

/// Answers the row under the cursor, through the keys somebody would press.
///
/// Enter opens the answers. If the one wanted is on that list it is chosen
/// there; if it is not, the last entry opens a field and it is typed.
fn answer(app: &mut App, value: &str) {
    // Space opens what a row offers; `Enter` accepts the step. The two were
    // one key until the keymap was straightened out, and every test that
    // answers a row goes through here, so they all learned it at once.
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

/// A value this setting accepts that is not the one it already has, worked out
/// from what the setting itself declares — so these tests do not go stale when
/// a vocabulary moves.
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
fn every_setting_is_reachable_from_the_per_agent_step_or_the_options_page() {
    // The screen exists so an operator can see all of them. Splitting them
    // between the per-agent step and the options page is only an improvement if
    // none went missing in the split — a setting in neither is invisible here,
    // which is the same failure as leaving it out of the table, in a place
    // nobody would look.
    let mut seen = Vec::new();
    for mut app in [per_agent(), options()] {
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
        }
        assert_eq!(app.selected, 0, "the cursor did not wrap");
        app.press(Key::Up);
        assert_eq!(app.selected, app.row_count() - 1, "it did not wrap back");
    }
    for setting in SETTINGS {
        assert!(seen.contains(setting), "{setting:?} cannot be reached");
    }
    assert_eq!(
        seen.len(),
        SETTINGS.len(),
        "a setting is missing or duplicated"
    );
}

#[test]
fn the_per_agent_step_asks_only_what_differs_by_agent() {
    // The whole point of the split. A repository has one tracker whichever
    // agent looks at it, and asking eleven times invites eleven answers.
    let app = per_agent();
    assert_eq!(
        app.rows(),
        &[
            Setting::Delivery,
            Setting::Review,
            Setting::Transitions,
            Setting::Judges,
            Setting::Planning,
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
            "implementer",
            "reviewer",
            "judge",
            "strategist",
            "analyst",
            "builder",
            "refactorer",
            "validator",
            "auditor",
        ]
    );

    let app = options();
    assert_eq!(app.rows(), OPTIONS_SETTINGS);
    assert!(app.rows().contains(&Setting::Tracker));
    assert!(!app.rows().contains(&Setting::Models));
}

#[test]
fn an_answer_on_the_options_page_lands_in_every_agents_table() {
    // There is no separate shared document in play: the answer is written into
    // each agent's own table. One that reached only the agent under the cursor
    // would be a setting the other ten silently disagree with.
    let mut app = options();
    walk_to(&mut app, Setting::Merge);
    answer(&mut app, "squash");

    for config in app.configs.values() {
        assert_eq!(
            config.merge,
            crate::config::MergeStrategy::Squash,
            "a repository-wide answer reached only some of the tables"
        );
    }
    assert!(app.changed(Setting::Merge));
    assert!(!app.disagrees(Setting::Merge), "they were left disagreeing");
    assert!(app.dirty());
}

#[test]
fn the_per_agent_step_edits_the_agent_under_the_cursor_and_nobody_else() {
    // "What does *this* agent do" is the question this step is asked. One
    // shared table forced anybody who wanted two agents to differ to leave the
    // screen and learn `config set`.
    let mut app = per_agent();
    let first = app.current();
    // `a` moves to the next chosen agent. There is no second list to move focus
    // into: one list means the arrow keys never have to be explained.
    to_another_agent(&mut app);
    let second = app.current();
    assert_ne!(first, second, "`a` did not change agent");

    walk_to(&mut app, Setting::Planning);
    answer(&mut app, "sdd lite");

    assert_eq!(
        app.configs[second].planning,
        crate::config::Planning::Sdd {
            openspec: false,
            lite: true
        }
    );
    assert_eq!(
        app.configs[first].planning,
        Config::default().planning,
        "editing one agent changed another"
    );

    // Walking back shows the first agent's value again, not the edit just made.
    back_to_agent(&mut app, first);
    assert_eq!(app.current(), first);
    walk_to(&mut app, Setting::Planning);
    assert_eq!(app.setting().value_of(app.config()), "direct");
    assert!(!app.changed(Setting::Planning));
}

#[test]
fn the_per_agent_step_walks_only_the_agents_that_were_ticked() {
    // Landing on an unticked adapter invites edits to a table this run is not
    // going to write, which is an answer the operator will believe they gave.
    let mut app = per_agent();
    let ticked: Vec<&str> = app.chosen().iter().map(|adapter| adapter.slug).collect();
    assert_eq!(ticked.len(), 2);

    let mut walked = vec![app.current()];
    for _ in 0..4 {
        app.press(Key::Char('a'));
        walked.push(app.current());
    }
    for _ in 0..4 {
        app.press(Key::Char('A'));
        walked.push(app.current());
    }
    for slug in &walked {
        assert!(
            ticked.contains(slug),
            "{slug} is not ticked and the cursor reached it"
        );
    }
}

#[test]
fn an_agent_estigia_cannot_gate_says_so_on_every_row_that_would_need_one() {
    // Three of the per-agent rows name who authorises something, and the gate
    // is what stops and asks. Without one the contract still says it and the
    // agent may still honour it — but nothing checks, and a screen that took
    // the answer in silence would tell somebody they had configured an
    // enforcement they had not.
    let ungated = crate::setup::AGENTS
        .iter()
        .position(|adapter| !adapter.can_gate_tools())
        .expect("at least one adapter is contract-only");

    let mut app = setup();
    app.agent = ungated;
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.current(), crate::setup::AGENTS[ungated].slug);

    // Editable, because the answer still reaches the contract, and captioned,
    // because what happens to it afterwards is different.
    for setting in [Setting::Delivery, Setting::Review, Setting::Transitions] {
        let applies = app.adapter().applies(setting);
        assert!(applies.editable(), "{setting:?} was refused");
        assert!(
            applies.because().is_some(),
            "{setting:?} is unenforced here and the screen does not say so"
        );
    }

    // A gated agent holds every row except review: the gate can enforce the
    // handoff, but no gate can manufacture its independent context.
    let gated = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.can_gate_tools())
        .expect("at least one adapter is gated");
    for setting in SETTINGS {
        let expected = if *setting == Setting::Review {
            crate::setup::Applies::Asked(
                "Estigia records and releases the review handoff, but this runtime must still \
                 provide a distinct reviewer context",
            )
        } else {
            crate::setup::Applies::Held
        };
        assert_eq!(
            gated.applies(*setting),
            expected,
            "{setting:?} has the wrong capability caveat for an agent Estigia gates"
        );
    }
}

#[test]
fn the_steps_walk_forward_and_back_and_stop_at_the_ends() {
    // A stepper that wraps takes somebody from the last step back to the first
    // on the key they were using to go forward, which reads as the screen
    // having thrown their answers away.
    // `Tab` walks the panels before it walks the steps, so a step with two of
    // them is two stops. Written as "press until the step moves" rather than as
    // one press per step: what this test is about is the *order* and the ends,
    // and counting presses would make it fail the day a step gains a panel
    // without anything being wrong.
    let mut app = setup();
    assert_eq!(app.step, STEPS[0]);
    let forward = |app: &mut App| {
        let from = app.step;
        for _ in 0..4 {
            app.press(Key::Enter);
            if app.step != from {
                return;
            }
        }
        panic!("`Tab` never left {from:?}");
    };
    for expected in &STEPS[1..] {
        forward(&mut app);
        assert_eq!(app.step, *expected);
    }
    // At the end, pressed directly: the point is that it does **not** move, so
    // a helper that presses until it does would loop over the thing being
    // measured.
    for _ in 0..3 {
        app.press(Key::Enter);
    }
    assert_eq!(app.step, Step::Install, "the last step wrapped");

    let back = |app: &mut App| {
        let from = app.step;
        for _ in 0..4 {
            app.press(Key::Backspace);
            if app.step != from {
                return;
            }
        }
        panic!("`BackTab` never left {from:?}");
    };
    for expected in STEPS[..STEPS.len() - 1].iter().rev() {
        back(&mut app);
        assert_eq!(app.step, *expected);
    }
    for _ in 0..3 {
        app.press(Key::Backspace);
    }
    assert_eq!(app.step, Step::Agents, "the first step wrapped");
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
    walk_to(&mut app, Setting::Planning);
    app.press(Key::Char(' '));
    let at = app
        .picker()
        .iter()
        .position(|entry| entry == "sdd lite")
        .expect("the planning answer");
    walk_picker_to(&mut app, at);
    app.press(Key::Char(' '));
    assert_eq!(app.shown_value(Setting::Planning), "sdd lite");

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
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Enter);
    app.press(Key::Tab);
    app.uniform = true;

    assert_eq!(app.model_value("orchestrate"), "different values");
    assert_eq!(app.model_value("reviewer"), "shared");
    walk_to_model_target(&mut app, "orchestrate");
    app.press(Key::Char(' '));
    assert_eq!(app.picker(), vec!["type a model ID…", "inherit"]);
}

#[test]
fn planning_disagreement_suppresses_only_planning_phase_rows() {
    let installed = std::collections::BTreeMap::from([
        ("claude-code", Config::default()),
        (
            "codex",
            Config {
                planning: crate::config::Planning::Sdd {
                    openspec: false,
                    lite: false,
                },
                ..Config::default()
            },
        ),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Enter);
    app.press(Key::Tab);
    app.uniform = true;

    let names = app
        .model_targets()
        .into_iter()
        .map(|target| target.name)
        .collect::<Vec<_>>();
    for fixed in ["orchestrate", "apply", "implementer", "auditor"] {
        assert!(names.contains(&fixed), "missing fixed target {fixed:?}");
    }
    for phase in ["explore", "propose", "spec", "design", "tasks"] {
        assert!(!names.contains(&phase), "leaked disputed phase {phase:?}");
    }
    walk_to_model_target(&mut app, "orchestrate");
    assert!(
        paint(&app, 100, 30).contains("Planning differs across selected agents"),
        "the fixed rows do not explain why planning phases are absent"
    );
}

#[test]
fn tui_model_projection_matches_all_five_planning_modes_exactly() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    for planning in crate::config::Planning::all() {
        let app = App::one_table(
            Some(claude),
            Config {
                planning,
                ..Config::default()
            },
        );
        let mut expected = vec!["orchestrate"];
        expected.extend(planning.phases());
        expected.extend([
            "apply",
            "implementer",
            "reviewer",
            "judge",
            "strategist",
            "analyst",
            "builder",
            "refactorer",
            "validator",
            "auditor",
        ]);
        assert_eq!(
            app.model_targets()
                .into_iter()
                .map(|target| target.name)
                .collect::<Vec<_>>(),
            expected,
            "wrong rows under {planning:?}"
        );
    }
}

#[test]
fn uniform_planning_changes_only_selected_agents_across_all_five_modes() {
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
    Setting::Planning
        .apply(app.configs.get_mut(unselected).expect("its table"), "sdd")
        .expect("the conflicting Planning value is valid");
    let untouched = app.configs[unselected].clone();
    app.uniform = true;

    let modes = crate::config::Planning::all();
    assert_eq!(
        modes.len(),
        5,
        "the test no longer covers every Planning mode"
    );
    for planning in modes {
        let value = Setting::Planning.value_of(&Config {
            planning,
            ..Config::default()
        });
        app.set(Setting::Planning, &value)
            .unwrap_or_else(|refusal| panic!("{value:?} is accepted: {refusal}"));

        for slug in &selected {
            assert_eq!(
                Setting::Planning.value_of(&app.configs[*slug]),
                value,
                "uniform Planning did not reach selected agent {slug}"
            );
        }
        assert_eq!(
            app.configs[unselected].render_rows(),
            untouched.render_rows(),
            "uniform Planning changed unselected agent {unselected} under {planning:?}"
        );
        let mut expected = vec!["orchestrate"];
        expected.extend(planning.phases());
        expected.extend([
            "apply",
            "implementer",
            "reviewer",
            "judge",
            "strategist",
            "analyst",
            "builder",
            "refactorer",
            "validator",
            "auditor",
        ]);
        assert_eq!(
            app.model_targets()
                .into_iter()
                .map(|target| target.name)
                .collect::<Vec<_>>(),
            expected,
            "the visible model rows borrowed unselected agent {unselected}'s Planning"
        );
    }

    let before = app.configs.clone();
    assert!(app.set(Setting::Planning, "not-a-planning-mode").is_err());
    assert_eq!(
        app.configs, before,
        "a rejected uniform Planning value partially changed the tables"
    );
}

#[test]
fn planning_and_phase_models_render_as_separate_nonselectable_blocks() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            planning: crate::config::Planning::Sdd {
                openspec: false,
                lite: true,
            },
            ..Config::default()
        },
    );
    walk_to(&mut app, Setting::Planning);
    let screen = paint(&app, 80, 24);

    assert!(screen.contains("Planning"));
    assert!(screen.contains("PHASE MODELS"));
    assert!(screen.contains("orchestrate"));
    assert!(!screen.contains("Model routing"));
    assert_eq!(
        app.row_count(),
        app.rows().len() + 1 + app.model_targets().len()
    );
}

#[test]
fn changing_to_an_agent_without_the_selected_phase_returns_to_planning() {
    let installed = std::collections::BTreeMap::from([
        (
            "claude-code",
            Config {
                planning: crate::config::Planning::Sdd {
                    openspec: false,
                    lite: false,
                },
                ..Config::default()
            },
        ),
        ("codex", Config::default()),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Enter);
    app.press(Key::Tab);
    walk_to_model_target(&mut app, "design");

    app.press(Key::Char('a'));

    assert_eq!(app.current(), "codex");
    assert_eq!(app.setting_at_cursor(), Some(Setting::Planning));
}

#[test]
fn changing_planning_when_the_selected_phase_disappears_returns_to_planning() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    let mut app = App::one_table(
        Some(claude),
        Config {
            planning: crate::config::Planning::Sdd {
                openspec: false,
                lite: false,
            },
            ..Config::default()
        },
    );
    walk_to_model_target(&mut app, "design");

    app.set(Setting::Planning, "sdd lite")
        .expect("a shorter planning protocol");

    assert_eq!(app.setting_at_cursor(), Some(Setting::Planning));
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
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Char(' '));
    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    app.press(Key::Tab);
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
    assert!(!claude.iter().any(|entry| entry == "gpt-5.6-sol"));

    app.press(Key::Esc);
    walk_agents_to(&mut app, "codex", Key::Char('a'));
    app.press(Key::Char(' '));
    let codex = app.picker();
    assert!(codex.iter().any(|entry| entry == "gpt-5.6-sol"));
    assert!(!codex.iter().any(|entry| entry == "fable"));
    assert_ne!(claude, codex, "two hosts were given one universal catalog");

    app.press(Key::Esc);
    walk_agents_to(&mut app, "opencode", Key::Char('a'));
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
    for foreign in ["fable", "opus", "sonnet", "haiku", "gpt-5.6-sol"] {
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
    assert_eq!(app.picker(), vec!["type a model ID…", "inherit"]);

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
        app.config().planning,
        Config::default().planning,
        "choosing a catalog model also chose SDD"
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
            models: crate::config::ModelRouting::parse("auditor=private/model")
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
fn profile_restore_and_dirty_marker_cover_hidden_route_changes() {
    let claude = crate::setup::find_agent("claude-code").expect("the Claude adapter");
    let mut installed = claude.model_profiles()[0]
        .routing()
        .expect("the balanced profile is valid");
    assert!(installed.assign("done", "haiku"));
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
        "the hidden route removal was unmarked:\n{screen}"
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
fn custom_model_editor_rejects_persisted_entry_delimiters() {
    let claude = crate::setup::AGENTS
        .iter()
        .find(|adapter| adapter.slug == "claude-code")
        .expect("the Claude Code adapter");
    for model in [
        "provider,model",
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
            app.message
                .as_deref()
                .is_some_and(|message| { message.contains("no comma, pipe, or line break") }),
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
    uniform.uniform = true;
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
            vec!["type a model ID…", "inherit"],
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
    let unchosen = "orchestrate=unchosen-current, auditor=unchosen-current";

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
        "implementer=codex-current, orchestrate=shared/new"
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
        "implementer=codex-current"
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
        "implementer=codex-current, orchestrate=codex-installed"
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
            config("implementer=codex-installed, orchestrate=codex-installed"),
        ),
        (
            "opencode",
            config("auditor=unchosen-installed, orchestrate=unchosen-installed"),
        ),
    ]);
    let mut app = App::with_configs(&installed, &["claude-code", "codex"]);
    walk_the_menu_to(&mut app, Page::Setup);
    app.press(Key::Enter);
    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    app.press(Key::Tab);
    assert_eq!(app.panel, super::app::Panel::Rows);

    app.configs
        .get_mut("claude-code")
        .expect("Claude table")
        .models =
        crate::config::ModelRouting::parse("reviewer=claude-current, orchestrate=claude-current")
            .expect("Claude routing");
    app.configs.get_mut("codex").expect("Codex table").models =
        crate::config::ModelRouting::parse("implementer=codex-current, orchestrate=codex-current")
            .expect("Codex routing");
    app.configs
        .get_mut("opencode")
        .expect("unchosen OpenCode table")
        .models = crate::config::ModelRouting::parse(
        "auditor=unchosen-current, orchestrate=unchosen-current",
    )
    .expect("unchosen routing");
    app.uniform = true;
    walk_to_model_target(&mut app, "orchestrate");
    app
}

#[test]
fn uniform_model_detail_names_every_selected_agent() {
    let mut app = per_agent_with(&["claude-code", "codex"]);
    app.uniform = true;
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
    let editing = paint(&app, 120, 34);
    assert!(editing.contains("sin coma, barra vertical ni salto de línea"));
    assert!(editing.contains("Esc cancela"));
}

#[test]
fn spanish_help_explains_the_structured_model_selector() {
    let mut app = App::new(Config::default());
    app.page = Page::Help;
    app.tongue = crate::tui::words::Tongue::Spanish;
    let help = paint(&app, 120, 90);

    for fact in [
        "Planning es el último ajuste principal",
        "sección separada con orchestrate",
        "abre directamente el catálogo orientativo",
        "heredar elimina solo ese destino",
        "CLI conserva la edición de la ruta key=model completa",
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
                "Setup",
                "Push guard",
                "Marks",
                "Planning is the last",
                "Only Claude Code currently receives planned phase definitions",
                "display label is translated",
                "printed CLI commands stay canonical",
            ],
        ),
        (
            &spanish,
            [
                "Instalación",
                "Guardia",
                "Marcas",
                "Planning es el último ajuste principal",
                "Solo Claude Code recibe actualmente definiciones de fases planificadas",
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
    // It goes into every table, so a value half of them accept would leave the
    // rest holding the old one — a repository that disagrees with itself, made
    // by one keystroke.
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
    let first = app.rows()[0];
    let second = app.rows()[1];
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

    app.selected = 0;
    app.press(Key::Char('r'));
    assert!(!app.changed(first), "the row was not restored");
    assert!(
        app.changed(second),
        "restoring one row threw away an edit on another"
    );
}

#[test]
fn installing_needs_an_agent_and_says_so_where_one_is_chosen() {
    // Installing into no agent writes nothing and would report success, which
    // is the shape of lie this whole tool exists to refuse. The refusal puts
    // the cursor on the step that fixes it rather than only complaining.
    let mut app = setup();
    for step in STEPS {
        let mut app = app.clone();
        app.step = *step;
        assert_eq!(app.press(Key::Char('s')), Action::None);
        assert_eq!(app.step, Step::Agents, "the refusal left nowhere to go");
        assert!(app.message.is_some(), "an empty install went unremarked");
    }
    // And from the options page, which is a *different screen* — sending
    // somebody to step one without also sending them to setup left them
    // reading "choose an agent" with no agents anywhere on the page.
    let mut empty = app.clone();
    empty.page = Page::Options;
    assert_eq!(empty.press(Key::Char('s')), Action::None);
    assert_eq!(
        empty.page,
        Page::Setup,
        "the refusal named a page it did not open"
    );
    assert_eq!(empty.step, Step::Agents);
    assert!(empty.message.is_some());

    // Space ticks, and ticks off again.
    app.press(Key::Char(' '));
    assert_eq!(app.chosen().len(), 1);
    app.press(Key::Char(' '));
    assert!(app.chosen().is_empty(), "space did not untick");
    app.press(Key::Char(' '));

    // And now `s` is a plan, from every step.
    for step in STEPS {
        let mut app = app.clone();
        app.step = *step;
        assert_eq!(app.press(Key::Char('s')), Action::Save);
    }
    // As is Enter on the last step, which is what it is for.
    let mut last = app.clone();
    last.step = Step::Install;
    assert_eq!(last.press(Key::Enter), Action::Save);
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
    assert_eq!(again.press(Key::Char('s')), Action::Save);
}

#[test]
fn an_edit_on_one_agent_is_not_lost_by_walking_to_another() {
    // `dirty` used to ask only the agent under the cursor, so somebody who
    // edited one, walked to the next and pressed `q` was told there was nothing
    // to lose.
    let mut app = per_agent();
    let edited = app.current();
    app.set(Setting::Planning, "sdd").expect("accepted");
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

#[test]
fn the_commands_the_screen_prints_are_commands_that_run() {
    // The screen ends by offering "the same without this screen", which is the
    // ratchet applied to a person leaving the TUI: a way out that is not this.
    // It printed `config set <row> <value> --agent <slug>` for every row that
    // moved — including the rows that are facts about the repository, which
    // `--agent` refuses outright as `setting-not-per-agent`. So the one screen
    // whose job is to hand back a runnable command handed back a refusal, for
    // exactly the settings a person is likeliest to have changed.
    let mut app = options();
    let mut moved = Vec::new();
    for setting in crate::config::OPTIONS_SETTINGS {
        // Which rows this page offers depends on the answers already given:
        // moving `Tracker` off GitHub takes the board row away, because those
        // bindings declare no board. A walk to a row the page no longer draws
        // would never end.
        if !app.rows().contains(setting) {
            continue;
        }
        walk_to(&mut app, *setting);
        let Some(other) = another_value(&app, *setting) else {
            continue;
        };
        answer(&mut app, &other);
        moved.push(*setting);
    }
    assert!(!moved.is_empty(), "no repository-wide row could be moved");

    to_setup(&mut app);
    app.step = Step::Install;
    let width = 200;
    let painted = paint(&app, width, 60);
    assert!(
        painted.contains("estigia setup"),
        "the screen does not offer the way out at all"
    );
    for setting in moved {
        // Offered at all, first: dropping these rows would satisfy "no `--agent`
        // on them" and lose the value instead, which is the same promise broken
        // the other way round.
        let named = format!("estigia config set {:?}", setting.label());
        let at = painted
            .find(&named)
            .unwrap_or_else(|| panic!("{} was moved and is not offered", setting.label()));
        // The rest of that row only. The buffer is a grid with no newlines, so
        // a longer window reads the next line's `--agent` as this one's.
        let row = &painted[at..at + (usize::from(width) - at % usize::from(width))];
        assert!(
            !row.contains("--agent"),
            "{} is offered with `--agent`, which refuses it: {row}",
            setting.label()
        );
    }
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
fn every_step_paints_and_says_what_it_is_asking() {
    // The state machine is tested without a terminal, which leaves the drawing
    // untested — and a screen that panics on the first frame is a feature that
    // exists only in its own tests. `TestBackend` is a real render into a
    // buffer, so this fails the way the terminal would.
    let base = per_agent();
    for step in STEPS {
        let mut app = base.clone();
        app.step = *step;
        let painted = paint(&app, 110, 34);

        // The stepper: every step's name, on every step, so somebody one
        // question in can see how many are left.
        for other in STEPS {
            assert!(
                painted.contains(other.title()),
                "{step:?} does not show {other:?} in the stepper"
            );
        }
        // And the question this one is asking, in words.
        let question = step.question();
        let head: String = question.chars().take(20).collect();
        assert!(
            painted.contains(&head),
            "{step:?} does not say what it is asking: {painted}"
        );
    }

    // On a settings step, the row's own explanation and every answer it takes —
    // so nobody has to remember a vocabulary to change a setting.
    let painted = paint(&base, 110, 34);
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

    // The last step names the commands that would do the same thing without
    // this screen, per agent.
    let mut install = base;
    install.step = Step::Install;
    let painted = paint(&install, 110, 34);
    assert!(
        painted.contains("estigia setup"),
        "the summary does not say what will be written: {painted}"
    );
}

#[test]
fn the_screen_says_which_agents_it_can_actually_hold() {
    // An operator ticking an agent Estigia cannot gate is choosing a contract
    // and a push guard, not a gate. A screen that did not say so would have
    // sold them the wrong thing.
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
            merge: crate::config::MergeStrategy::Squash,
            ..Config::default()
        },
    );
    installed.insert("codex", Config::default());

    let app = App::with_configs(&installed, &["claude-code"]);
    assert_eq!(
        app.configs["claude-code"].merge,
        crate::config::MergeStrategy::Squash
    );
    assert_eq!(app.configs["codex"].merge, Config::default().merge);
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
                assert_eq!(app.page, Page::Home, "an action left the menu");
            }
        }
    }

    // The push guard is on it. An operator who never finds it has a gate with
    // a hole in it and no way to know.
    assert!(
        MENU.iter().any(|entry| entry.goes == Goes::Doing("guard")),
        "the one boundary no agent can go around is not on the menu"
    );
}

#[test]
fn setup_always_opens_on_its_first_question() {
    // Resuming somebody's third step after they walked out of it is a screen
    // that starts in the middle of a sentence.
    let mut app = setup();
    to_step(&mut app, Step::Install);
    assert_eq!(app.step, Step::Install);
    app.press(Key::Esc);
    assert_eq!(app.page, Page::Home);
    app.press(Key::Enter);
    assert_eq!(app.page, Page::Setup);
    assert_eq!(app.step, Step::Agents);
}

#[test]
fn esc_walks_back_to_the_menu_rather_than_out() {
    // Somebody who opened the wrong page should not have to quit the tool.
    let mut app = setup();
    assert_eq!(app.page, Page::Setup);
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
    // Borrowed whole from Leteo: rounded borders, cyan where the keys are and
    // dark grey everywhere else. Checked because a panel built by hand
    // somewhere would differ in a way nobody notices until it is on screen —
    // and the seam that makes the rest of this file testable does not cover
    // how it looks.
    let source = include_str!("../tui.rs");
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
    assert!(wide.contains("Setup") && wide.contains("Push guard"));
    assert!(
        wide.contains(env!("CARGO_PKG_VERSION")),
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
        narrow.contains("Setup"),
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
    assert_eq!(app.page, Page::Setup);
    assert!(app.pinned());
    assert_eq!(app.current(), "claude-code");

    // Persisted Model routing is projected as target rows, and Planning is the
    // final primary row that decides which phase targets follow it.
    assert!(!app.rows().contains(&Setting::Models));
    assert_eq!(app.rows().last(), Some(&Setting::Planning));
    assert!(!app.model_targets().is_empty());
    let rows = app.rows().to_vec();
    app.press(Key::Enter);
    assert_eq!(app.rows(), rows, "Enter moved a screen with no steps");

    // An answer lands in the one table, whichever scope the row has.
    walk_to(&mut app, Setting::Tracker);
    answer(&mut app, "linear");
    assert_eq!(app.configs.len(), 1, "a second table appeared from nowhere");
    assert_eq!(app.config().tracker, crate::config::Tracker::Linear);

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
    // The whole point of the change. Most rows have two or three answers and no
    // others, and typing one of three words exactly, from memory, into a field
    // that refuses a typo is work the screen can do instead.
    let mut app = options();
    walk_to(&mut app, Setting::Merge);
    let answers = Setting::Merge.answers();
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

/// The screen's refusal for an inert row is a branch nothing can reach today.
///
/// `Applies::Inert` says so itself — *nothing produces this today* — and the
/// consequence is that `refuse_if_inert` can be made to answer **either** way
/// with the whole suite green. `true` was closed by asserting the other side
/// (`a_row_that_decides_something_here_is_not_refused_as_inert`); `false` cannot
/// be, because no state this screen can be in reaches the refusal at all.
///
/// So this is a tripwire rather than a crossing: it fails on the day an adapter
/// starts answering `Inert`, and it names what has to be crossed then. The
/// screen's own translation guard already carries the same shape in its own
/// words — *the day something returns it, the screen would have shown one
/// English word and no guard would have moved*.
#[test]
fn nothing_answers_inert_yet_and_the_day_it_does_this_screen_needs_crossing() {
    use crate::setup::{AGENTS, Applies};

    let mut inert = Vec::new();
    for adapter in AGENTS {
        for setting in SETTINGS {
            if matches!(adapter.applies(*setting), Applies::Inert(_)) {
                inert.push((adapter.slug, *setting));
            }
        }
    }
    assert!(
        inert.is_empty(),
        "{inert:?} answer `Inert` now. `refuse_if_inert` refuses those rows and no test drives \
         that path — write one that opens such a row on the screen, presses an answer, and \
         requires the row to say it has no effect here rather than take it."
    );
}

/// A row that decides something here is taken, not refused.
///
/// `refuse_if_inert` guards the two places an answer is accepted, and it could
/// answer `true` for every row with the whole suite green — a screen that says
/// *X has no effect here* to everything and writes nothing. It survived because
/// the state it exists to catch, `Applies::Inert`, is produced by no adapter
/// today, so the refusal never fired and nothing asserted the other side of it.
#[test]
fn a_row_that_decides_something_here_is_not_refused_as_inert() {
    let mut app = options();
    walk_to(&mut app, Setting::Merge);
    let before = app.setting().value_of(app.config());

    // Through the picker, which is one of the two places the guard sits.
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

    // And through cycling, which is the other.
    let stepped = app.setting().value_of(app.config());
    app.press(Key::Right);
    assert_ne!(
        app.setting().value_of(app.config()),
        stepped,
        "stepping a row this screen answers was refused"
    );
}

/// The folder keys belong to the folder walk and to nothing else.
///
/// `l`, `h` and `n` move, step out and name a directory while a picker is
/// walking one — and they are **letters on every other row**, which is what the
/// guard on those three arms says in its own words. Measured by mutation:
/// making `browsing_a_folder` answer `true` for every row left the whole suite
/// green, so a picker of three words would have moved when they were pressed
/// and `n` would have replaced it with a prompt asking for a folder name.
///
/// The predicate that decides it also had a twin: `wants_folders` carried the
/// same expression byte for byte, twelve lines away. One question, one answer.
#[test]
fn the_folder_keys_are_letters_on_a_row_that_takes_no_folder() {
    let mut app = options();
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
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);

    // Every answer, and — because a tracker also takes `github owner/name`,
    // which no list can hold — somewhere to type. A picker over three of the
    // possible answers with no way to reach the fourth is worse than none.
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

    // Choosing one takes it.
    answer(&mut app, "linear");
    assert_eq!(app.config().tracker, crate::config::Tracker::Linear);
    assert_eq!(app.focus, Focus::List);

    // And the way past the list reaches a value the list never had.
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
fn bottom_model_picker_tracks_the_rendered_auditor_row_in_each_layout() {
    for planning in ["direct", "sdd", "sdd lite"] {
        let mut app = per_agent();
        app.set(Setting::Planning, planning)
            .unwrap_or_else(|refusal| panic!("{planning:?} is valid: {refusal}"));
        walk_to_model_target(&mut app, "auditor");
        app.press(Key::Char(' '));
        assert_eq!(app.focus, Focus::Picking, "the model picker did not open");
        assert_auditor_picker_geometry(&app, planning);

        for (width, height) in [(40, 10), (20, 6), (4, 3), (1, 1)] {
            let _ = paint(&app, width, height);
        }
    }
}

#[test]
fn opening_a_model_picker_from_who_returns_focus_and_viewport_to_rows() {
    for planning in ["direct", "sdd", "sdd lite"] {
        let mut app = per_agent();
        app.set(Setting::Planning, planning)
            .unwrap_or_else(|refusal| panic!("{planning:?} is valid: {refusal}"));
        walk_to_model_target(&mut app, "auditor");
        app.press(Key::Tab);
        assert_eq!(app.panel, super::app::Panel::Who);

        app.press(Key::Char(' '));
        assert_eq!(app.focus, Focus::Picking, "the model picker did not open");
        assert_eq!(
            app.panel,
            super::app::Panel::Rows,
            "the open pane does not own the row viewport under {planning:?}"
        );
        assert_auditor_picker_geometry(&app, planning);

        choose_picker_entry(&mut app, "fable");
        assert_eq!(app.model_value("auditor"), "fable");
        let lines = rows(&app, 80, 24);
        let title = if planning == "direct" {
            " MODELS "
        } else {
            " PHASE MODELS "
        };
        let (top, bottom, left, right) = panel_bounds(&lines, title);
        let (_, assignment) = assignment_row(&lines, top, bottom, left, right, "auditor");
        assert!(
            assignment.contains("fable") && assignment.contains('*'),
            "the auditor mutation is not visible in its assignment row under {planning:?}:\n{}",
            lines.join("\n")
        );
    }
}

#[test]
fn unfocused_model_rows_and_scrollbar_both_return_to_the_top_before_space() {
    let mut app = per_agent();
    walk_to_model_target(&mut app, "auditor");
    app.press(Key::Tab);
    assert_eq!(app.panel, super::app::Panel::Who);

    let lines = rows(&app, 80, 24);
    let (top, bottom, _, right) = panel_bounds(&lines, " MODELS ");
    assert_eq!(
        lines[top + 1].chars().nth(right - 1),
        Some('\u{2588}'),
        "the unfocused rows reset to the first model but the thumb did not:\n{}",
        lines.join("\n")
    );
    assert_ne!(
        lines[bottom - 1].chars().nth(right - 1),
        Some('\u{2588}'),
        "the unfocused thumb still points at auditor while the rows start at zero:\n{}",
        lines.join("\n")
    );
}

#[test]
fn space_on_the_agent_choice_still_toggles_the_agent_instead_of_opening_a_row() {
    let mut app = setup();
    let before = app.agents[app.agent].1;
    app.press(Key::Char(' '));
    assert_eq!(app.agents[app.agent].1, !before);
    assert_eq!(app.step, Step::Agents);
    assert_eq!(app.focus, Focus::List);
    assert!(app.modal.is_none());
}

#[test]
fn a_truncated_model_panel_shows_its_scrollbar_while_settings_are_focused() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Planning);
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
    walk_to(&mut app, Setting::Merge);
    app.press(Key::Char(' '));
    assert!(
        !app.picker().iter().any(|entry| entry == TYPE_IT),
        "a closed row offers somewhere to type a value it will refuse"
    );

    // An open one does, because its answers are examples rather than the whole
    // set — and without the field there would be no way to say the rest.
    let mut app = options();
    walk_to(&mut app, Setting::Tracker);
    app.press(Key::Char(' '));
    assert!(
        app.picker().iter().any(|entry| entry == TYPE_IT),
        "an open row offers only the answers it happens to list"
    );
}

#[test]
fn the_read_only_pages_scroll_rather_than_hiding_their_own_bottom_half() {
    // The help runs to forty lines and the doctor's report grows with the
    // checks. On a twenty-four-row window a page that cannot scroll has a
    // section nobody can reach, which is the same as not having written it.
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

#[test]
fn the_per_agent_step_with_nothing_ticked_asks_nothing_and_says_why() {
    // Every row here would be answered into a table this run will not write.
    // Offering them is the screen taking answers it has already decided to
    // throw away.
    let mut app = setup();
    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    assert!(app.rows().is_empty(), "it offered rows for nobody");
    assert_eq!(app.setting_at_cursor(), None);

    // The keys that would edit one do nothing, and nothing is left half-open.
    // Space is the key that would open a row now; `Enter` accepts the step.
    for key in [Key::Char(' '), Key::Left, Key::Right, Key::Char('r')] {
        assert_eq!(app.press(key), Action::None);
        assert_eq!(app.focus, Focus::List, "{key:?} opened something");
    }
    assert!(!app.dirty());

    let painted = paint(&app, 80, 24);
    assert!(
        painted.contains("No agent is ticked"),
        "it shows an empty screen with no explanation: {painted}"
    );
    // And the footer offers only the keys that step has.
    assert!(painted.contains("back to the agents"));
    assert!(
        !painted.contains("r restore"),
        "the footer offers a key that does nothing here"
    );
}

#[test]
fn a_warning_is_visible_on_whichever_step_raised_it() {
    // The panel that carries messages is skipped when a step has nothing to
    // say. A warning raised on such a step would then be a key that, as far as
    // the operator can tell, did nothing at all.
    let mut app = per_agent();
    app.set(Setting::Planning, "sdd").expect("accepted");
    for step in STEPS {
        let mut app = app.clone();
        app.step = *step;
        assert_eq!(app.press(Key::Char('q')), Action::None, "it left at once");
        let message = app.message.clone().expect("something was raised");
        let painted = paint(&app, 80, 24);
        assert!(
            painted.contains(&message),
            "{step:?} raised {message:?} and did not show it: {painted}"
        );
    }

    // The options page has the same detail panel and the same warning.
    let mut app = options();
    app.set(Setting::Merge, "squash").expect("accepted");
    assert_eq!(app.press(Key::Char('q')), Action::None, "it left at once");
    let message = app.message.clone().expect("something was raised");
    let painted = paint(&app, 80, 24);
    assert!(
        painted.contains(&message),
        "the options page raised {message:?} and did not show it: {painted}"
    );
}

/// Options is a page off the menu, holding the rows that are not about agents.
///
/// Sixteen settings split by *what the answer is about*: setup asks who is
/// holding the tools and what each may do, and these are facts about the
/// repository — one tracker, one merge strategy, whichever agent looks. As step
/// three of setup they were behind two questions already answered, which is the
/// wrong shape for the rows somebody comes back to change.
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
    // And setup no longer offers them, or the same row could be answered twice
    // with two answers, of which only the later write survives.
    for step in STEPS {
        for setting in step.settings() {
            assert!(
                !OPTIONS_SETTINGS.contains(setting),
                "{setting:?} is on {step:?} and on the options page"
            );
        }
    }

    // It paints, says what it is asking, and names the row under the cursor.
    let painted = paint(&app, 96, 30);
    assert!(
        painted.contains("Options"),
        "the header does not say where you are"
    );
    assert!(
        painted.contains(
            &super::app::OPTIONS_QUESTION
                .chars()
                .take(24)
                .collect::<String>()
        ),
        "the page does not say what it is asking: {painted}"
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
    assert!(
        !painted.contains("Per agent"),
        "the page carries a stepper for steps it is not one of"
    );
    // The keys it actually has, and not the ones it does not.
    assert!(painted.contains("s install"));
    assert!(
        !painted.contains("next agent"),
        "a key that moves nothing here is offered anyway"
    );
    assert!(
        !app.walks_agents(),
        "`a` would move an agent on rows that are the same for all of them"
    );

    // `s` writes, from here, without walking back through setup.
    assert_eq!(app.clone().press(Key::Char('s')), Action::Save);

    // Esc goes back to the menu it was opened from rather than out.
    assert_eq!(app.press(Key::Esc), Action::None);
    assert_eq!(app.page, Page::Home);

    // And it reopens at the top of its list, with nothing half-open behind it.
    let mut app = options();
    app.press(Key::Down);
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking);
    to_menu(&mut app);
    to_options(&mut app);
    assert_eq!(app.selected, 0, "it resumed in the middle of a list");
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
        app.agents.len() > 10,
        "this test needs more adapters than a short window holds"
    );
    let short = paint(&app, 80, 18);
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
    app.page = Page::Setup;
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
fn the_stepper_says_which_steps_are_carrying_an_unsaved_answer() {
    // A screen that asks one question at a time has to say where the answers
    // already given are, or the only way to find an edit made two steps ago is
    // to walk back and look at every row.
    let mut app = App::with_agents(Config::default(), &["claude-code"]);
    app.page = Page::Setup;
    for step in STEPS {
        assert!(
            !app.touched(*step),
            "{step:?} is dirty before anything moved"
        );
    }

    // An agent ticked is a change to step one.
    app.press(Key::Down);
    app.press(Key::Char(' '));
    assert!(app.touched(Step::Agents));
    assert!(!app.touched(Step::PerAgent));

    // A per-agent row moved is a change to step two.
    app.press(Key::Char('2'));
    app.set(Setting::Planning, "sdd").expect("accepted");
    assert!(app.touched(Step::PerAgent));

    let painted = paint(&app, 96, 26);
    assert!(painted.contains('\u{2022}'), "no mark on the stepper");

    // A repository row is not on the stepper at all — it is on the options
    // page — and moving one must not mark a step that does not hold it.
    assert!(!app.options_touched(), "nothing repository-wide has moved");
    app.set(Setting::Merge, "squash").expect("accepted");
    assert!(app.options_touched());
    for step in STEPS {
        assert_eq!(
            app.touched(*step),
            *step == Step::Agents || *step == Step::PerAgent,
            "{step:?} claims a repository row it does not hold"
        );
    }
}

/// The mark that says an answer is given and not written, where the rows are.
///
/// The stepper carried it while the repository's rows were step three. They are
/// a page off the menu now, and an edit with nothing on screen calling it
/// unsaved is an edit somebody walks away from — so the entry carries the same
/// mark, in the same colour, in the one place that entry is ever seen from.
#[test]
fn the_menu_says_when_the_options_page_is_carrying_an_unsaved_answer() {
    let mut app = options();
    assert!(!app.options_touched());

    walk_to(&mut app, Setting::Merge);
    answer(&mut app, "squash");
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
fn a_number_goes_straight_to_its_step() {
    // Walking back two steps to change one answer is four keys on a screen
    // that already numbers them.
    let mut app = per_agent();
    for (digit, step) in ['1', '2', '3'].into_iter().zip(STEPS) {
        assert_eq!(app.press(Key::Char(digit)), Action::None);
        assert_eq!(app.step, *step, "{digit} went to the wrong step");
    }
    // A number with no step behind it does nothing rather than something
    // surprising.
    app.press(Key::Char('9'));
    assert_eq!(app.step, Step::Install);

    // And a digit typed into a field is a digit, not a jump.
    let mut app = per_agent();
    walk_to_model_target(&mut app, "orchestrate");
    open_the_field(&mut app);
    for _ in 0..64 {
        app.press(Key::Backspace);
    }
    type_in(&mut app, "gpt-4");
    assert_eq!(app.draft, "gpt-4", "a digit escaped the field");
    assert_eq!(app.step, Step::PerAgent);
}

#[test]
fn a_row_this_repository_already_customises_is_marked_apart_from_one_just_moved() {
    // Two different facts. Without the second, a rerun cannot tell a row this
    // repository deliberately set from one nobody has ever touched.
    let mut installed = std::collections::BTreeMap::new();
    installed.insert(
        "claude-code",
        Config {
            merge: crate::config::MergeStrategy::Squash,
            ..Config::default()
        },
    );
    let mut app = App::with_configs(&installed, &["claude-code"]);
    app.page = Page::Options;
    assert!(!app.changed(Setting::Merge), "nothing has moved yet");

    let painted = paint(&app, 96, 26);
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
    let painted = paint(&app, 96, 26);
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
            merge: crate::config::MergeStrategy::Squash,
            ..Config::default()
        },
    );
    installed.insert("codex", Config::default());
    let mut app = App::with_configs(&installed, &["claude-code"]);
    app.page = Page::Options;
    walk_to(&mut app, Setting::Merge);
    let shown = app.shown_value(Setting::Merge);
    assert_eq!(shown, "squash", "the chosen agent's answer is what shows");

    answer(&mut app, "rebase");
    assert!(app.changed(Setting::Merge));
    app.press(Key::Char('r'));
    assert_eq!(
        app.shown_value(Setting::Merge),
        shown,
        "restore put back a value that was never on screen"
    );
    assert!(!app.changed(Setting::Merge));
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

fn assert_auditor_picker_geometry(app: &App, planning: &str) {
    let title = if planning == "direct" {
        " MODELS "
    } else {
        " PHASE MODELS "
    };
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
    let (auditor, assignment) = assignment_row(
        &lines,
        model_top,
        model_bottom,
        model_left,
        model_right,
        "auditor",
    );
    assert!(
        assignment.contains("inherit") || assignment.contains("fable"),
        "auditor was found outside its assignment row under {planning:?}:\n{painted}"
    );
    assert_eq!(
        lines[auditor].chars().nth(model_right - 1),
        Some('\u{2588}'),
        "the scrollbar thumb does not point at bottom-row auditor under {planning:?}:\n{painted}"
    );

    let (picker_top, picker_bottom, picker_left, picker_right) =
        panel_bounds(&lines, " Models for auditor ");
    assert!(
        picker_left >= model_left
            && picker_right <= model_right
            && picker_top >= model_top
            && picker_bottom <= model_bottom,
        "the auditor picker is not clamped inside the model panel under {planning:?}:\n{painted}"
    );
    assert!(
        picker_top == auditor + 1 || picker_bottom + 1 == auditor,
        "the auditor picker is not adjacent to its assignment row under {planning:?}:\n{painted}"
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
        // Onto a setting first. The options page opens on the two rows that say
        // what it is showing, and `setting()` falls back to the first per-agent
        // row there — so the label being looked for was not the label on screen.
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
    // The layout subtracts borders, centres panels and floats a dropdown over a
    // row. Every one of those is a size somebody chooses by dragging a window
    // edge, and the small ones are the sizes nobody paints while building. A
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

    let mut summary = per_agent();
    summary.press(Key::Enter);
    assert_eq!(summary.step, Step::Install);
    screens.push(("summary", summary));

    for page in [Page::Doctor, Page::Help] {
        let mut app = App::new(Config::default());
        app.page = page;
        screens.push(("text", app));
    }

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
    // The hints were one line handed to the window edge, so a narrow window ate
    // them from the right — and what is written last is `s install`, the key
    // the step exists to reach. It also cut words: `r restore` read as
    // `r rest`, which is a key nobody has.
    for width in [44u16, 60, 80, 110] {
        let app = per_agent();
        let painted = rows(&app, width, 16);
        let footer = painted.last().expect("a footer row").trim().to_owned();
        assert!(
            footer.contains("s install"),
            "at {width} columns the footer never says how to install: {footer}"
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

#[test]
fn a_narrow_window_keeps_the_count_of_steps_even_when_it_cannot_keep_the_names() {
    // The stepper is not there to say where you are — the header does that.
    // It is there to say how many questions there are, so somebody one step in
    // knows the screen ends. Handed to the window edge it lost the last step
    // entirely and cut the one before it in half.
    let app = per_agent();
    for width in [30u16, 44, 60, 80, 110] {
        let painted = rows(&app, width, 16);
        let stepper = painted[1].trim_end().to_owned();
        for step in STEPS {
            assert!(
                stepper.contains(&step.number().to_string()),
                "at {width} columns step {} is not on the stepper: {stepper}",
                step.number()
            );
        }
        // Whichever step somebody is on is named, at every width.
        assert!(
            stepper.contains(app.step.title()),
            "at {width} columns the step being answered is unnamed: {stepper}"
        );
        assert!(
            stepper.chars().count() <= usize::from(width),
            "the stepper is wider than the window: {stepper}"
        );
    }
}

#[test]
fn the_last_step_keeps_its_table_a_table_on_a_window_that_wraps() {
    // The panel wraps, because the sentence above the table has to. A row wider
    // than the panel therefore wrapped into the next line and carried its
    // columns with it, and the list of what will be installed — the last thing
    // read before pressing `s` — stopped lining up at sixty columns.
    let mut app = per_agent();
    app.press(Key::Enter);
    assert_eq!(app.step, Step::Install);
    let names: Vec<&str> = app
        .chosen()
        .iter()
        .map(|adapter| adapter.display_name)
        .collect();
    assert!(names.len() > 1, "one row cannot show a table falling apart");
    for width in [50u16, 60, 80, 110] {
        let painted = rows(&app, width, 22);
        let at: Vec<usize> = names
            .iter()
            .map(|name| {
                painted
                    .iter()
                    // A prefix, not the whole name: a narrow window cuts the
                    // name column, which is the point.
                    .position(|row| row.contains(&name.chars().take(6).collect::<String>()))
                    .unwrap_or_else(|| panic!("at {width} columns no row names {name}"))
            })
            .collect();
        // One row per agent, so the rows are consecutive. A row that wrapped
        // pushed the next agent down by however many lines it spilled onto.
        for pair in at.windows(2) {
            assert_eq!(
                pair[1],
                pair[0] + 1,
                "at {width} columns a row wrapped and split the table: {:?}",
                &painted[pair[0]..=pair[1]]
            );
        }
    }
}

/// Prints frames for a human to look at, at the sizes that break things.
///
/// Not run by the suite. `cargo test --lib look -- --ignored --nocapture`.
/// Every layout fault found so far was found by painting the screen small and
/// reading it, not by reading the code that paints it.
#[test]
#[ignore = "a human reads the output; nothing is asserted"]
fn look() {
    let mut summary = per_agent();
    summary.press(Key::Enter);
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
        ("summary", summary, 60, 20),
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
    // The tests around this one check that `s` *offers* to save. None checked
    // what it would save — and that is the whole question an operator has: the
    // row I set for one agent went to that agent, the row I set for the
    // repository went to all of them, and nothing I did not touch moved.
    //
    // Driven through the keys rather than by calling `set`, because a journey
    // that answers correctly and a screen that routes the keys correctly are
    // two different claims.
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

    // A row that differs by agent, set on the agent under the cursor.
    let mine = *AGENT_SETTINGS
        .iter()
        .find(|setting| another_value(&app, **setting).is_some())
        .expect("some per-agent row offers a second answer");
    let mine_was = app.shown_value(mine);
    let mine_now = another_value(&app, mine).expect("a second answer");
    walk_to(&mut app, mine);
    answer(&mut app, &mine_now);

    // And a row that belongs to the repository, set on the page that owns it —
    // out to the menu, into Options, and back, which is the whole walk an
    // operator now makes to answer both halves.
    to_options(&mut app);
    let ours = *OPTIONS_SETTINGS
        .iter()
        .find(|setting| another_value(&app, **setting).is_some())
        .expect("some repository row offers a second answer");
    let ours_now = another_value(&app, ours).expect("a second answer");
    walk_to(&mut app, ours);
    answer(&mut app, &ours_now);

    // Offered for saving from the options page it was answered on, without
    // walking back through setup for a row setup no longer holds.
    assert_eq!(
        app.clone().press(Key::Char('s')),
        Action::Save,
        "the options page cannot write what it took"
    );

    // And from the last step of setup, which writes the same tables.
    to_setup(&mut app);
    to_step(&mut app, Step::Install);
    assert_eq!(app.step, Step::Install);
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

    // And nothing else moved, on either agent.
    for setting in SETTINGS {
        if *setting == mine || *setting == ours {
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

/// Installing answers the one question, over the screen, and nothing else.
///
/// This was a full page of the install log, and it was wrong twice. The log
/// says what changed and never says whether it worked, which is the thing
/// somebody presses the key to find out; and the menu draws a report in full
/// along its foot, so thirty rows of paths came back with the operator and
/// buried the menu they had returned to.
///
/// So the verdict is a box over the screen, dismissed by any key — and that key
/// is spent closing it, because one keystroke that both closed the box and moved
/// what was behind it would be two things happening with only one of them seen.
#[test]
fn installing_says_whether_it_worked_and_says_it_over_the_screen() {
    let mut app = per_agent();
    let step = app.step;
    let selected = app.selected;
    acknowledge(&mut app, "installed into 2 agents, 34 files");

    let modal = app.modal.clone().expect("no verdict was shown");
    assert!(modal.ok, "a run that landed was reported as a failure");
    assert_eq!(modal.title, "installed into 2 agents, 34 files");
    assert_eq!(modal.detail, None, "a run that worked needs no way out");
    assert_eq!(
        app.page,
        Page::Setup,
        "the verdict moved the screen under it"
    );

    // Any key closes it and lands on the menu: the run is finished, and what
    // comes next — the guard, the checks, leaving — is all there.
    assert_eq!(app.press(Key::Down), Action::None);
    assert_eq!(
        app.modal, None,
        "the verdict outlived the key that closed it"
    );
    assert_eq!(
        app.page,
        Page::Home,
        "a finished install did not reach the menu"
    );
    assert_eq!(
        app.report, None,
        "the menu was handed a page body to draw at its foot"
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

    // And it leaves them where the thing to fix is. Sent to the menu, the walk
    // back to this step is four keys the refusal did not have to cost them.
    assert_eq!(app.press(Key::Char('x')), Action::None);
    assert_eq!(app.modal, None, "the verdict outlived the key");
    assert_eq!(
        app.page,
        Page::Setup,
        "a refusal sent them away from the fix"
    );
    assert_eq!(app.step, step, "a refusal moved the step under it");
    assert_eq!(
        app.selected, selected,
        "a refusal moved the cursor under it"
    );
}

#[test]
fn a_successful_retry_replaces_the_pending_refusal() {
    let mut app = per_agent();
    app.uniform = true;
    app.set(Setting::Planning, "sdd lite")
        .expect("Planning is accepted");
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
    app.uniform = true;
    app.set(Setting::Planning, "sdd lite")
        .expect("Planning is accepted");
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
        .insert(selected, vec![Setting::Planning]);

    app.installed_partially(receipt);

    assert_eq!(
        Setting::Planning.value_of(&app.installed[selected]),
        "sdd lite",
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
        let second = crate::setup::find_agent("gemini-cli").expect("Gemini is an adapter");
        let mut selected = Config::default();
        Setting::Planning
            .apply(&mut selected, "sdd lite")
            .expect("Planning is accepted");
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

/// What was just written is what is installed, and the screen has to know.
///
/// `dirty` warns about losing answers, the stepper marks a step as carrying an
/// unsaved one, and the agent list ticks what was configured. All three ask the
/// same question — *does this differ from the disk* — and installing changed the
/// answer. Left alone the screen warned about discarding what it had just
/// written, which teaches an operator to ignore the warning that matters.
///
/// A refusal moves none of it: nothing was written, so the tables are unsaved
/// because they *are*.
#[test]
fn a_finished_install_leaves_nothing_the_screen_still_calls_unsaved() {
    let mut app = per_agent();
    walk_to(&mut app, Setting::Delivery);
    answer(&mut app, "auto");
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
    for step in STEPS {
        assert!(
            !app.touched(*step),
            "{step:?} is marked unsaved after the install"
        );
    }
    assert!(
        !app.options_touched(),
        "the menu still marks the options page unsaved after the install"
    );
}

/// A successful write acknowledges only the tables that crossed the write and
/// read-back boundary.
///
/// This is deliberately a disk-backed lifecycle rather than an `App`-only
/// assertion. A uniform edit can reach two selected agents and one can then be
/// unticked before save. The plan writes only the remaining agent; treating the
/// whole in-memory map as installed hides the other edit and lets quit discard
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
    app.uniform = true;
    app.set(Setting::Planning, "sdd lite")
        .expect("the shared planning answer is accepted");
    app.set(Setting::Models, "orchestrate=provider/planner")
        .expect("the shared model answer is accepted");

    let codex = app
        .agents
        .iter_mut()
        .find(|(adapter, _)| adapter.slug == "codex")
        .expect("codex is on the agent page");
    codex.1 = false;
    let plan = super::plan_of(&app);
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
        Setting::Planning.value_of(&read("claude-code")),
        "sdd lite",
        "the selected agent did not persist"
    );
    assert_eq!(
        Setting::Planning.value_of(&read("codex")),
        "direct",
        "the deselected agent reached disk"
    );
    assert_eq!(
        Setting::Planning.value_of(&app.configs["codex"]),
        "sdd lite",
        "deselecting discarded the in-memory edit"
    );
    assert_eq!(
        Setting::Planning.value_of(&app.installed["codex"]),
        "direct",
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
fn two_saves_keep_planning_when_a_model_is_saved_second() {
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
        models: crate::config::ModelRouting::parse("analysis=hidden/model")
            .expect("a hidden assignment"),
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
    app.uniform = true;

    app.set(Setting::Planning, "sdd lite")
        .expect("Planning is accepted");
    let plan = super::plan_of(&app);
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the first save persists Planning");
    app.installed_now(receipt);

    app.set(
        Setting::Models,
        "analysis=hidden/model, orchestrate=provider/planner",
    )
    .expect("the visible model is accepted");
    let plan = super::plan_of(&app);
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the second save persists the model");
    app.installed_now(receipt);

    let final_config = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the final contract reads");
    assert_eq!(
        Setting::Planning.value_of(&final_config),
        "sdd lite",
        "the second save reconstructed the contract from the pre-session bare table"
    );
    assert_eq!(
        final_config.models.for_target("orchestrate"),
        Some("provider/planner")
    );
    assert_eq!(
        final_config.models.for_target("analysis"),
        Some("hidden/model"),
        "a hidden assignment was lost while composing the saves"
    );
}

#[test]
fn two_saves_keep_the_model_when_planning_is_saved_second() {
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
        models: crate::config::ModelRouting::parse("analysis=hidden/model")
            .expect("a hidden assignment"),
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
    app.uniform = true;

    app.set(
        Setting::Models,
        "analysis=hidden/model, orchestrate=provider/planner",
    )
    .expect("the visible model is accepted");
    let plan = super::plan_of(&app);
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the first save persists the model");
    app.installed_now(receipt);

    app.set(Setting::Planning, "sdd lite")
        .expect("Planning is accepted");
    let plan = super::plan_of(&app);
    let (receipt, _) = crate::cli::install_planned(&plan, &options, false)
        .expect("the second save persists Planning");
    app.installed_now(receipt);

    let final_config = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the final contract reads");
    assert_eq!(
        final_config.models.for_target("orchestrate"),
        Some("provider/planner"),
        "the second save reconstructed the contract from the pre-session bare table"
    );
    assert_eq!(Setting::Planning.value_of(&final_config), "sdd lite");
    assert_eq!(
        final_config.models.for_target("analysis"),
        Some("hidden/model"),
        "a hidden assignment was lost while composing the saves"
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
    app.uniform = true;
    app.set(Setting::Planning, "sdd lite")
        .expect("Planning is accepted");
    app.set(Setting::Models, "orchestrate=provider/preview")
        .expect("the model is accepted");
    app.set(Setting::Merge, "squash")
        .expect("the repository answer is accepted");
    assert!(app.dirty(), "the preview has no change to protect");
    let installed_before = app.installed.clone();
    let bare_before = app.bare.clone();

    let dry_run = crate::setup::SetupOptions {
        dry_run: true,
        home_dir: Some(home.path().to_path_buf()),
        ..crate::setup::SetupOptions::default()
    };
    let plan = super::plan_of(&app);
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
    assert!(!app.saved, "preview was recorded as a completed save");
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
             | Planning | sdd lite | direct |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the local override is written");
    let contract = root.join(crate::skill::CONTRACT);
    let before = std::fs::read(&contract).expect("the shared contract reads");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the local layer reads");
    assert_eq!(Setting::Planning.value_of(&effective), "sdd lite");
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
        "the local Planning answer was promoted into shared SKILL.md"
    );
    assert!(
        !crate::skill::agent_override(&root, adapter.slug).exists(),
        "a no-edit local answer was duplicated into the per-agent override"
    );
}

#[test]
fn a_local_override_materializes_and_retracts_claude_phase_definitions_without_promotion() {
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
    let contract_before = std::fs::read(&contract).expect("the portable contract reads");
    let local = root.join(crate::config::LOCAL_FILE);
    std::fs::write(
        &local,
        format!(
            "{}\n| Setting | Value here | Skill default |\n|---|---|---|\n\
             | Planning | sdd lite | direct |\n\
             | Model routing | spec=opus | unset |\n{}\n",
            crate::config::BLOCK_BEGIN,
            crate::config::BLOCK_END
        ),
    )
    .expect("the local override is written");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the local layer reads");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, effective)]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("the effective host installs");

    let phase = home
        .path()
        .join(".claude")
        .join("agents")
        .join("sdd-spec.md");
    let rendered = std::fs::read_to_string(&phase).expect("the effective phase is materialized");
    assert!(rendered.contains("model: opus"), "{rendered}");
    assert_eq!(
        std::fs::read(&contract).expect("the portable contract still reads"),
        contract_before,
        "the local host configuration was promoted into SKILL.md"
    );

    std::fs::remove_file(local).expect("the effective override is removed");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the portable contract is effective again");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, effective)]),
        repository: std::path::PathBuf::new(),
    };
    crate::cli::install_planned(&plan, &options, false).expect("the host artifacts retract");
    assert!(
        !phase.exists(),
        "removing the effective Planning override left its phase definition behind"
    );
}

#[test]
fn a_per_agent_override_materializes_claude_phase_models_without_rewriting_its_contract() {
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
    let contract_before = std::fs::read(&contract).expect("the portable contract reads");
    let mut override_config = Config::default();
    Setting::Planning
        .apply(&mut override_config, "sdd lite")
        .expect("Planning is accepted");
    Setting::Models
        .apply(&mut override_config, "tasks=sonnet")
        .expect("the phase model is accepted");
    crate::setup::write_agent_configuration(
        &crate::skill::agent_override(&root, adapter.slug),
        adapter.slug,
        &override_config,
        &[Setting::Planning, Setting::Models],
    )
    .expect("the per-agent override is written");
    let effective = crate::skill::installed_config_for(&root, Some(adapter.slug))
        .expect("the per-agent layer reads");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, effective.clone())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, effective)]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("the effective host installs");

    let rendered = std::fs::read_to_string(
        home.path()
            .join(".claude")
            .join("agents")
            .join("sdd-tasks.md"),
    )
    .expect("the effective phase is materialized");
    assert!(rendered.contains("model: sonnet"), "{rendered}");
    assert_eq!(
        std::fs::read(contract).expect("the portable contract still reads"),
        contract_before,
        "the per-agent host configuration was promoted into SKILL.md"
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
    let gemini = crate::setup::find_agent("gemini-cli").expect("Gemini is an adapter");
    let cursor = crate::setup::find_agent("cursor").expect("Cursor is an adapter");
    for adapter in [opencode, gemini, cursor] {
        crate::setup::setup(adapter, &Config::default(), &options)
            .unwrap_or_else(|refusal| panic!("{} installs: {refusal}", adapter.slug));
    }
    let root = crate::setup::resolve_paths(opencode, &options)
        .expect("OpenCode resolves")
        .skill_root;
    let contract = root.join(crate::skill::CONTRACT);
    let before_contract = std::fs::read(&contract).expect("the shared contract reads");
    let before_gemini = crate::skill::installed_config_for(&root, Some(gemini.slug))
        .expect("Gemini reads the shared root");
    let before_cursor = crate::skill::installed_config_for(&root, Some(cursor.slug))
        .expect("Cursor reads the shared root");
    let mut chosen = Config::default();
    Setting::Planning
        .apply(&mut chosen, "sdd lite")
        .expect("Planning is accepted");
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
        crate::skill::installed_config_for(&root, Some(gemini.slug)).expect("Gemini still reads"),
        before_gemini,
        "unselected Gemini inherited OpenCode's edit"
    );
    assert_eq!(
        crate::skill::installed_config_for(&root, Some(cursor.slug)).expect("Cursor still reads"),
        before_cursor,
        "unselected Cursor inherited OpenCode's edit"
    );
    let own = crate::skill::installed_config_for(&root, Some(opencode.slug))
        .expect("OpenCode reads its override");
    assert_eq!(Setting::Planning.value_of(&own), "sdd lite");
    assert_eq!(
        own.models.for_target("orchestrate"),
        Some("provider/opencode")
    );
    let override_text = std::fs::read_to_string(crate::skill::agent_override(&root, opencode.slug))
        .expect("OpenCode's override was written");
    assert!(override_text.contains("| Planning | sdd lite |"));
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
    Setting::Planning
        .apply(&mut chosen, "sdd lite")
        .expect("Planning is accepted");
    let plan = super::Plan {
        agents: vec![adapter],
        opened: std::collections::BTreeMap::from([(adapter.slug, Config::default())]),
        rows: std::collections::BTreeMap::from([(adapter.slug, chosen)]),
        repository: std::path::PathBuf::new(),
    };

    crate::cli::install_planned(&plan, &options, false).expect("Claude Code installs");

    assert_eq!(
        Setting::Planning
            .value_of(&crate::skill::contract_config(&root).expect("the private contract reads")),
        "sdd lite"
    );
    assert!(
        !crate::skill::agent_override(&root, adapter.slug).exists(),
        "a private-root answer was duplicated into a redundant override"
    );
}

/// A page's report does not follow the operator back to the menu.
///
/// The menu draws `report` in full along its foot, which is right for the one
/// line the guard leaves there — the guard never leaves the menu to say it. The
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

/// The whole screen, in the language somebody chose, and nothing left behind.
///
/// A page at a time, because the failure this catches is never the page
/// somebody was translating — it is the neighbour. `keys_for` alone has five
/// arms, and four of them are only ever seen on one step.
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

        // Nothing English survives on a Spanish screen — except the words that
        // are deliberately not prose. A setting's label is the key of a row in
        // the operator's own markdown table, and its values are that row's
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

/// The language is a preference of this screen, not a row of any contract.
///
/// No agent reads which language a person's terminal is in, so a row for it in
/// the contract would be one `config set` writes, `config list` reads back, and
/// no decision consults. It lives on the options page all the same — under its
/// own border, applied on the key rather than by `s`.
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

/// The key that finishes survives every width, on every page, in every language.
///
/// `fit_keys` drops hints from the middle so the first and last survive — the
/// first says how to move, the last says how to finish. Two things broke that,
/// and each hid the other.
///
/// Down to **two** hints, the middle is index 1, which is the last: the final
/// removal took the very key the rule exists to keep. At twenty columns the
/// footer read `up down move` and said nothing about installing.
///
/// And three pages did not put the finishing key last at all — the agents step,
/// the options page, and `config edit`. At forty-four columns those footers
/// offered a way *out* and no way to save. The one test that covered this looked
/// only at the per-agent step, which is the single page that happened to be
/// ordered right.
///
/// Both are only visible across the whole grid, so the grid is what is walked.
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

/// The finishing key is the **last** one every page lists.
///
/// Said as an ordering rather than only checked through the rendering, because
/// `fit_keys` protects a position and nothing made the pages agree to use it.
/// Three of five did not, and the rendering test above is the expensive way to
/// find that out.
#[test]
fn every_page_lists_the_key_that_writes_last() {
    let mut agents = setup();
    agents.press(Key::Char(' '));
    let mut install = per_agent();
    install.press(Key::Enter);
    install.press(Key::Enter);

    for (name, app) in [
        ("agents", agents),
        ("per agent", per_agent()),
        ("install", install),
        ("options", options()),
        ("config edit", App::one_table(None, Config::default())),
    ] {
        let hints = super::keys_for(&app);
        let writes: Vec<&String> = hints
            .iter()
            .filter(|hint| hint.contains("s install") || hint.contains("s save"))
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

/// `config edit` never asks for a row the file it writes cannot hold.
///
/// The screen's own rule, one page over, is that a row whose answer this run
/// will not write is a row it must not ask about — a step with no agents ticked
/// offers nothing for exactly that reason. The pinned screen broke it the day
/// an adapter's own file stopped carrying the repository's rows: it went on
/// offering all seventeen while the write kept six, so
/// `config edit --agent qwen`, change the tracker, save, and the tracker was
/// untouched with nothing on screen saying so.
///
/// Crossed against the write rather than restated, because the two live in
/// different modules and one of them moved without the other.
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
        assert!(!asked.iter().any(|label| label == "Model routing"));
        assert!(kept.iter().any(|label| label == "Model routing"));
        assert!(!app.model_targets().is_empty());
        assert_eq!(asked.len() + 1, kept.len());
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
    // Walked rather than listed. The screen is pure — `press(Key) -> Action` —
    // so every state a person can put it in is reachable from here, and the
    // question worth asking of a whole state machine is not "does this page
    // work" but "is there one a person cannot get out of".
    //
    // A trap needs no bug to appear: a page added without an `Esc` arm, or a
    // modal that eats the key it is dismissed with, is enough. Nothing else
    // here would notice, because every existing test drives a route somebody
    // already thought of.
    use std::collections::BTreeSet;

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Tab,
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
    // The draw clamped the position for itself and left the state free, so
    // pressing `j` at the end kept counting. Twenty presses past the bottom
    // meant twenty presses of `k` that moved nothing — the screen looked frozen
    // while every key was being spent on an overshoot it had never shown.
    //
    // Found by walking the whole state space for something else: 3987 of 4000
    // states were one page's scroll counter, which is what an unbounded field
    // in a small screen looks like from the outside.
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
    // Walked, and **breadth-first on purpose**: depth-first spent 3987 of 4000
    // states on one page's scroll counter, so the regions that hold the edits
    // were visited three times each. A walk has to be measured by where it
    // went, not by how many states it counted.
    //
    // What it found: the menu draws the unsaved mark beside `Options` — that is
    // what `options_touched` exists for — and then `q`, `Esc` and its own
    // `Quit` entry all returned `Action::Quit` without a word. The one place
    // that asked was `leave`, which the settings pages use and the menu did
    // not. 1323 reachable states discarded an edit silently.
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Tab,
        Key::Char('q'),
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
    for page in ["Home", "Setup", "Options"] {
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
        // A message already showing is the warning itself: the second press is
        // meant to discard.
        if app.message.is_some() {
            continue;
        }
        let mut walk = app.clone();
        if matches!(walk.press(Key::Char('q')), Action::Quit) {
            *silent.entry(format!("{:?}", app.page)).or_default() += 1;
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
    // The other question a walk can answer: not "is this state right" but "does
    // any of them fall over". A panic here is the tool dying in somebody's
    // terminal, and the arithmetic that would do it — the footer's key list,
    // the clipping, the scrollbar — is exactly the arithmetic that has nothing
    // left to work with in a small window.
    //
    // Walked from a screen that has been **answered**, not a blank one: which
    // rows a pane shows depends on the answers already in it, so a walk from
    // `Config::default()` covers one shape of the screen.
    use std::collections::{BTreeSet, VecDeque};

    let keys = [
        Key::Up,
        Key::Down,
        Key::Left,
        Key::Right,
        Key::Enter,
        Key::Esc,
        Key::Char(' '),
        Key::Tab,
        Key::Char('?'),
    ];
    let mut config = Config::default();
    Setting::Planning
        .apply(&mut config, "sdd")
        .expect("a planning protocol");
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
    app.uniform = true;
    app.set(Setting::Models, "implementer=opus")
        .expect("a model routing");
    let wrong: Vec<&'static str> = app
        .configs
        .iter()
        .filter(|(slug, config)| {
            let changed = Setting::Models.value_of(config) == "implementer=opus";
            changed != chosen.contains(**slug)
        })
        .map(|(slug, _)| *slug)
        .collect();
    assert!(
        wrong.is_empty(),
        "uniform Model routing changed a table outside the chosen set or missed one: {wrong:?}"
    );

    // And turning it off puts the answer back where the row says it belongs.
    // Asserted rather than assumed, because a screen that spreads every answer
    // whatever the mode is a screen with one mode and a switch that lies.
    let mut app = setup();
    app.uniform = false;
    app.set(Setting::Models, "implementer=kimi")
        .expect("a model routing");
    let here = app.current();
    let spread: Vec<&'static str> = app
        .configs
        .iter()
        .filter(|(slug, config)| {
            **slug != here && Setting::Models.value_of(config) == "implementer=kimi"
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
        app.uniform = uniform;
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
fn the_agent_walk_stops_at_every_agent_before_the_first_one() {
    // The control the uniform mode is reached through. It is the same key that
    // already walks the agents, because it is the same question — *who is this
    // answer for* — and a second key would have to be found and explained.
    //
    // Ordered with the shared answer first: it is what somebody arriving wants,
    // and the per-agent readings are the exception the step exists for.
    let mut app = per_agent();
    assert!(
        !app.uniform,
        "the screen opened on the shared answer, so the walk below measures nothing"
    );

    // Round the whole cycle and back, counting where it lands. Two agents are
    // ticked, so there are three stops.
    let mut seen_uniform = 0;
    let mut agents = std::collections::BTreeSet::new();
    for _ in 0..3 {
        app.press(Key::Char('a'));
        if app.uniform {
            seen_uniform += 1;
        } else {
            agents.insert(app.current());
        }
    }
    assert_eq!(
        seen_uniform, 1,
        "the shared answer is not one stop of the cycle"
    );
    assert_eq!(agents.len(), 2, "the two ticked agents are not both stops");

    // And it walks the other way, which is what `A` is for. Measured from the
    // **first** chosen agent, because that is the only stop the shared answer
    // is directly behind — and the helper leaves the cursor on the second.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    back_to_agent(&mut app, first);
    app.press(Key::Char('A'));
    assert!(
        app.uniform,
        "walking back from the first agent skipped the shared answer"
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
    app.set(Setting::Planning, "sdd lite")
        .expect("a planning protocol");
    assert_ne!(
        Setting::Planning.value_of(&app.configs[first]),
        Setting::Planning.value_of(&app.configs[second]),
        "the two agents agree, so this test measures nothing"
    );
    assert!(
        !app.disagrees(Setting::Planning),
        "each agent answering for itself cannot disagree with anybody"
    );

    app.uniform = true;
    assert!(
        app.disagrees(Setting::Planning),
        "the screen offered one agent's answer as everyone's"
    );

    // And once they do agree, it stops saying so — or the mark is noise that
    // teaches people to ignore it.
    app.set(Setting::Planning, "direct")
        .expect("a planning protocol");
    assert!(
        !app.disagrees(Setting::Planning),
        "a row every agent now shares is still reported as split"
    );

    // The third door of the same family, and it needs a shape a shared write
    // cannot make: one table edited, the other still as installed, and the
    // cursor on the untouched one. Reached the way an operator reaches it —
    // answer for one agent, walk to the next, then switch to the shared answer.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    back_to_agent(&mut app, first);
    app.set(Setting::Planning, "sdd lite")
        .expect("a planning protocol");
    to_another_agent(&mut app);
    app.uniform = true;
    assert_eq!(
        Setting::Planning.value_of(app.config()),
        Setting::Planning.value_of(&app.installed[app.current()]),
        "the cursor's own table already differs, so this half measures nothing"
    );
    assert!(
        app.changed(Setting::Planning),
        "an unsaved edit in another agent's table is reported as saved"
    );
}

#[test]
fn restoring_under_the_shared_answer_puts_back_what_was_on_screen() {
    // The fifth door of the family, and the one a search found rather than
    // memory. `r` restores a row from the table it is *shown* from — its own
    // comment says why — and that rule was written down twice. When the shared
    // answer taught one copy that an agent row is written everywhere, the other
    // copy went on reading the cursor's table, so `r` could put back a value
    // that had never been on screen.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    let second = app.chosen()[1].slug;

    // Make the two tables differ *as installed*, which is the only way the two
    // readings can point at different values.
    back_to_agent(&mut app, first);
    app.set(Setting::Planning, "sdd lite")
        .expect("a planning protocol");
    app.installed = app.configs.clone();
    to_another_agent(&mut app);
    assert_eq!(
        app.current(),
        second,
        "the walk did not reach the other agent"
    );

    app.uniform = true;
    let on_screen = app.shown_value(Setting::Planning);
    assert_ne!(
        on_screen,
        Setting::Planning.value_of(&app.installed[second]),
        "both tables read the same, so this test measures nothing"
    );

    // Change it, then restore.
    app.set(Setting::Planning, "direct")
        .expect("a planning protocol");
    walk_to(&mut app, Setting::Planning);
    app.press(Key::Char('r'));
    assert_eq!(
        app.shown_value(Setting::Planning),
        on_screen,
        "`r` put back a value that was never on screen"
    );
}

#[test]
fn tab_belongs_to_the_step_with_two_panels_and_enter_walks_the_steps() {
    // The keymap a person already has for a form: `Enter` accepts and moves on,
    // `Backspace` goes back, space marks or opens what is under the cursor, and
    // `Tab` means *the other field* — which only exists on the one step that
    // has two.
    let mut app = setup();
    assert_eq!(app.step, Step::Agents);
    app.press(Key::Char(' '));
    app.press(Key::Tab);
    assert_eq!(
        app.step,
        Step::Agents,
        "`Tab` moved a step that has one panel"
    );

    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    assert_eq!(
        app.panel,
        super::app::Panel::Who,
        "the step opens somewhere other than the first question it asks"
    );
    app.press(Key::Tab);
    assert_eq!(app.panel, super::app::Panel::Rows);
    assert_eq!(app.step, Step::PerAgent, "`Tab` left the step");
    app.press(Key::Tab);
    assert_eq!(
        app.panel,
        super::app::Panel::Who,
        "`Tab` does not come back"
    );

    app.press(Key::Enter);
    assert_eq!(app.step, Step::Install);
    app.press(Key::Backspace);
    assert_eq!(app.step, Step::PerAgent, "`Backspace` did not go back");

    // The arrows follow the focus: on the left they answer *who*, and the row
    // under the cursor does not move.
    let mut app = per_agent();
    app.press(Key::Enter);
    assert_eq!(app.panel, super::app::Panel::Who);
    let row = app.setting();
    app.press(Key::Down);
    assert_eq!(app.setting(), row, "an arrow on the left moved the rows");
    // That it *does* move the who is measured by
    // `the_agent_walk_stops_at_every_agent_before_the_first_one`, which walks
    // the whole cycle. What this half is about is the other panel staying put.
}

#[test]
fn every_key_the_footer_names_does_something_where_it_is_named() {
    // The footer is where somebody learns the keys, so a hint for a key that
    // does nothing on that page is worse than no hint: they press it, nothing
    // moves, and the screen reads as broken.
    //
    // It happened. When the keymap was rewritten, six hints went on naming
    // keys that had stopped doing what they said — two footers still offered
    // `⇥` for the next step, three offered `⏎` for the answers a row takes.
    // Two were caught, by tests that happened to assert on those strings; the
    // other four were found by reading, which is not a guard.
    //
    // What makes this checkable is that a hint begins with its key. Pressing it
    // has to move *something* — the page, the step, the focus, the cursor, the
    // tables — or return an action. Doing nothing is the failure.
    let key_of = |hint: &str| -> Option<Key> {
        let head = hint.split_whitespace().next()?;
        Some(match head {
            "←→" => Key::Left,
            "↑↓" => Key::Up,
            "⇥" => Key::Tab,
            "⏎" => Key::Enter,
            "Esc" => Key::Esc,
            "space" | "espacio" => Key::Char(' '),
            "enter" | "intro" => Key::Enter,
            "backspace" | "retroceso" => Key::Backspace,
            "1-3" => Key::Char('1'),
            other => Key::Char(other.chars().next()?),
        })
    };
    // What "it moved" means, in the fields somebody can see. Rendered rather
    // than compared field by field: a tuple runs out of `PartialEq` past twelve
    // elements, and a rendering says *what* moved when the guard fires.
    let shape = |app: &App| {
        format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{}|{:?}|{:?}|{:?}",
            app.page,
            app.step,
            app.panel,
            app.focus,
            app.selected,
            app.agent,
            app.uniform,
            app.pick,
            app.draft,
            app.dirty(),
            app.message,
            app.configs,
            // Which agents are ticked: space on the first step moves this and
            // nothing else, so a shape without it reports the one key that
            // step is for as doing nothing.
            app.agents.iter().map(|(_, on)| *on).collect::<Vec<_>>(),
        )
    };

    let mut agents = setup();
    agents.press(Key::Char(' '));
    let mut install = per_agent();
    install.press(Key::Enter);
    let mut picking = per_agent();
    picking.press(Key::Char(' '));
    let mut editing = per_agent();
    walk_to_model_target(&mut editing, "orchestrate");
    open_the_field(&mut editing);

    for (name, base) in [
        ("agents", agents),
        ("per agent", per_agent()),
        ("install", install),
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
            // Tried on every row, not only the first. A footer names what the
            // *page* offers, and a page can hold a row with one legal answer —
            // `Route` takes `direct` and nothing else, so cycling it is a
            // no-op that says nothing about the key. What would be a defect is
            // a key that does nothing **anywhere** on the page.
            let mut answered = false;
            for row in 0..base.row_count().max(1) {
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
    // `screen_at_cursor` finds the preference rows by counting past the
    // repository's, and the page check in front of it is what keeps that
    // arithmetic from applying where it means nothing. Nothing measured that
    // check — turning it off left the whole suite green.
    //
    // It is reachable, and the confusion is not cosmetic. `config edit` with no
    // agent shows all seventeen rows, so its twelfth is past
    // `EVERYWHERE_SETTINGS.len()`: without the guard the screen would read that
    // row as the interface language, `setting_at_cursor` would answer `None`
    // for a row that is a setting, and space on it would change what language
    // the screen speaks instead of what the row says.
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
    // Row zero, because these two are drawn and walked **first** now:
    // `Repository shown` decides which checkout's answers the settings under it
    // are, and a control that changes the meaning of everything below it
    // belongs above it.
    let mut options = options();
    options.selected = 0;
    assert!(
        options.screen_at_cursor().is_some(),
        "the options page no longer offers the two rows that say what it is showing"
    );
    // And the settings still start where those rows end, so the cursor covers
    // both panels without a gap or an overlap.
    options.selected = crate::tui::app::SCREEN_ROWS.len();
    assert!(
        options.screen_at_cursor().is_none() && options.setting_at_cursor().is_some(),
        "the first row under the two is not a setting"
    );
}

#[test]
fn leaving_with_answers_nobody_wrote_asks_once_from_every_door() {
    // Four places call `warn_unsaved` — `q` on a settings page, `q` on doctor
    // or help, the menu's own Quit entry, and `Esc` back to the menu — and no
    // test named it. A mutation sweep turned two of them off and the whole
    // suite stayed green, which is the same thing said with a measurement.
    //
    // What it protects is the answer somebody typed and has not installed. The
    // rule is *ask once*: the first press warns and does not leave, the second
    // leaves, because a warning that cannot be got past is a screen nobody can
    // quit.
    // Dirty first, then walk to the door: the menu has no rows to answer, so
    // an answer has to exist before getting there.
    let dirty = || {
        let mut app = per_agent();
        walk_to(&mut app, Setting::Planning);
        answer(&mut app, "sdd lite");
        // Nothing already on screen, because a message already showing is what
        // the warning checks for — a stale one would make this pass with the
        // guard turned off.
        app.message = None;
        app
    };
    /// A way to the door, and the key pressed there.
    type Door = (&'static str, fn(App) -> App, Key);
    let doors: [Door; 4] = [
        ("a settings page, q", |app| app, Key::Char('q')),
        ("a settings page, esc", |app| app, Key::Esc),
        (
            "the menu's own quit",
            |mut app| {
                app.page = Page::Home;
                while app.entry().goes != super::app::Goes::Doing("quit") {
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
        assert!(
            app.message.is_none(),
            "{name}: something was already on screen, and the warning checks for that"
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
        // And it can be got past. Not always by returning an action: `Esc`
        // goes back to the menu, which is a page change and no action at all —
        // so what is asserted is that the second press *does something*, which
        // is the claim rather than one of its two spellings.
        let page = app.page;
        let action = app.press(key);
        assert!(
            action != Action::None || app.page != page,
            "{name}: the warning cannot be got past"
        );
    }
}

#[test]
fn the_configuration_step_lands_on_an_agent_that_will_be_written() {
    // Arriving at the step with the cursor on an agent nobody ticked means
    // every answer typed there goes into a table this run is not going to
    // write. The screen would look like it was working.
    //
    // Reached the way an operator reaches it: walk down to the second agent,
    // tick that one, walk back up to the first — which is **not** ticked — and
    // accept. Nothing measured the correction; a mutation sweep turned it off
    // and the whole suite stayed green.
    let mut app = setup();
    app.press(Key::Down);
    app.press(Key::Char(' '));
    let ticked = app.current();
    app.press(Key::Up);
    let under_cursor = app.current();
    assert_ne!(
        under_cursor, ticked,
        "the cursor is on the ticked agent, so this test measures nothing"
    );
    assert!(
        !app.agents[app.agent].1,
        "the agent under the cursor is ticked, so there is nothing to correct"
    );

    app.press(Key::Enter);
    assert_eq!(app.step, Step::PerAgent);
    assert_eq!(
        app.current(),
        ticked,
        "the step opened on an agent nobody ticked, so its answers would go nowhere"
    );
}

#[test]
fn the_picker_offers_what_the_row_is_set_to_even_when_it_is_not_on_the_list() {
    // Some rows take more than they can list — a path, a board, a model — so
    // their answers are useful ones rather than every one. A picker for such a
    // row that omits what it is currently set to shows the operator a list
    // their own value is not in, and choosing anything is then a change they
    // did not mean to make.
    //
    // Nothing measured it: turning the line off left the suite green.
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
    // Windows reports a key twice — once pressed, once released — and the
    // screen acts on one of them. Acting on both types every character twice
    // and moves the cursor two rows per arrow, which on this platform makes
    // the screen unusable.
    //
    // The filter lived inside the event loop, where no test could reach it:
    // every test here presses keys straight into the state machine. A mutation
    // sweep turned it off and the whole suite stayed green. So it moved out to
    // a function that takes an event, which is the only shape a test can hold.
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
    // written; drawing it was not — a mutation sweep turned the mark off and
    // the whole suite stayed green, which would leave the shared answer
    // showing a value under `EVERY AGENT` with nothing saying the other agent
    // holds something else.
    let mut app = per_agent();
    let first = app.chosen()[0].slug;
    back_to_agent(&mut app, first);
    walk_to(&mut app, Setting::Planning);
    answer(&mut app, "sdd lite");
    app.uniform = true;
    assert!(
        app.disagrees(Setting::Planning),
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
    // The third scope's turn at a family this screen has already paid for
    // twice. `set`, `shown_from` and `changed` each decide whether a row is one
    // agent's or everybody's, and a machine row is everybody's: the language
    // somebody writes in does not change because another agent asked.
    //
    // Two of the three could be put on the wrong side with the whole suite
    // green. Found by mutation rather than by reading, because reading them is
    // what put them there.
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

    // `shown_from`: read from what they agree on rather than from whichever
    // table the cursor happens to be over. Only ever visible when they do
    // **not** agree, which a spread answer cannot produce — so the tables are
    // made to differ the way a hand-edited contract makes them differ.
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
    app.uniform = false;
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
fn the_screen_shows_the_name_and_the_commands_it_prints_carry_the_key() {
    // Both halves of one rule. A dropdown shows a label and sends an id, and
    // this screen does the same: the row says `Estrategia de fusión` and the
    // contract keeps `Merge strategy`.
    //
    // The half that makes it safe is the one the old rule was protecting: the
    // install step prints the commands that reproduce the same result without
    // the questions, and those name the setting the way `config set` takes it.
    // Translate those and the operator has a line that cannot be run.
    let mut app = per_agent();
    app.tongue = crate::tui::words::Tongue::Spanish;
    walk_to(&mut app, Setting::Planning);
    answer(&mut app, "sdd lite");

    let painted = paint(&app, 120, 34);
    assert!(
        painted.contains(app.tongue.say(Setting::Planning.label())),
        "the row is not shown by its name: {painted}"
    );

    to_step(&mut app, Step::Install);
    let printed = paint(&app, 120, 34);
    assert!(
        printed.contains("config set"),
        "the step that prints the commands printed none: {printed}"
    );
    assert!(
        printed.contains(Setting::Planning.label()),
        "a printed command names the setting in a language `config set` will not take: {printed}"
    );
}

#[test]
fn a_row_that_takes_a_directory_offers_the_ones_that_are_there() {
    // Typing an absolute path into a field is the worst way to answer a row
    // whose answer is a directory. The picker already had arrows, `Enter` and
    // somewhere to type — so the folders go on it, and choosing one walks in.
    //
    // Fed rather than fetched: this machine has no filesystem in it, which is
    // the only reason a test can hold it. The shell refreshes the list before
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
    // Choosing a folder sets the row to it, and opening the picker again offers
    // *its* children — which is what makes this a walk rather than one list.
    // Unset, it starts at home, because a picker rooted at nothing has nothing
    // on it.
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

    // Set to something that is not there, it falls back rather than offering
    // nothing at all.
    app.set(Setting::Worktree, ABSENT)
        .expect("an absolute path");
    assert_eq!(
        app.folder_root(),
        home,
        "a path that is not there left the picker rooted at nowhere"
    );
}

#[test]
fn the_options_page_can_be_pointed_at_another_checkout() {
    // Managing a checkout's own rows meant going to that checkout first. The
    // registry already knew where they all were and nothing offered them.
    //
    // It is a preference of this screen, beside the language, for the same
    // reason: it belongs to the person sitting here and to no agent's contract.
    // What it decides is which checkout the rows above answer for.
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

/// Pointing the page at a checkout shows **that** checkout's answers.
///
/// The reload laid the chosen checkout's rows over whatever the page was
/// already showing — which, since the screen opens with the rows of the
/// checkout it was run in, meant every row the chosen one is silent about kept
/// the *previous* repository's value and was drawn as though it were this
/// one's. Somebody comparing two checkouts through the dropdown was reading the
/// first one twice.
///
/// The base is what that agent answers with when no checkout has said anything,
/// which is the only thing the layer can honestly be put on top of.
#[test]
fn pointing_at_a_checkout_shows_that_checkouts_answers() {
    let installed: std::collections::BTreeMap<&'static str, Config> = crate::setup::AGENTS
        .iter()
        .map(|adapter| {
            (
                adapter.slug,
                Config {
                    // As though the checkout the screen was run in had said so.
                    merge: crate::config::MergeStrategy::Squash,
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
         | Tracker | linear | github |\n",
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
        crate::config::Tracker::Linear,
        "the checkout's own answer did not arrive"
    );
    assert_eq!(
        shown.merge,
        Config::default().merge,
        "a row this checkout is silent about is showing the previous checkout's answer"
    );
}

/// Saving does not push this checkout's answers into the machine's contract.
///
/// The screen opens on one merged view — the contract with this checkout's own
/// rows on it — and handed that whole view back to the writer. So opening
/// `estigia setup` in a checkout that overrides a row and pressing save wrote
/// that override into the *contract*, and every other checkout that had said
/// nothing quietly moved to it. The operator changed nothing.
///
/// What goes into the contract is what it already held, with the rows the
/// operator actually moved on top. What goes into the checkout's file is the
/// merged view, which is what that file already is.
#[test]
fn saving_does_not_write_this_checkouts_answers_into_the_contract() {
    let contract = Config {
        merge: crate::config::MergeStrategy::Rebase,
        ..Config::default()
    };
    // As the screen is handed it: the contract with this checkout's row on top.
    let here = Config {
        merge: crate::config::MergeStrategy::Squash,
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

    // The page shows this checkout's answer, which is right and is the whole
    // reason the merged view exists. What the plan used to carry was this same
    // value, straight into the contract.
    assert_eq!(
        app.configs
            .get("claude-code")
            .expect("that agent has a table")
            .merge,
        crate::config::MergeStrategy::Squash,
        "the page is not showing what is in force here, so this measures nothing"
    );

    // Nothing touched: the plan carries no contract candidate at all. The
    // writer reads that layer independently from disk.
    let plan = super::plan_of(&app);
    assert_eq!(
        plan.opened["claude-code"].merge,
        crate::config::MergeStrategy::Squash
    );

    // And a row the operator *does* move goes in. Without this the fix would be
    // a screen whose save writes nothing, which passes the assertion above.
    let mut edited = app.configs.clone();
    for config in edited.values_mut() {
        config.tracker = crate::config::Tracker::Linear;
    }
    app.configs = edited;
    let plan = super::plan_of(&app);
    let theirs = &plan.rows["claude-code"];
    assert_eq!(
        theirs.tracker,
        crate::config::Tracker::Linear,
        "a row the operator moved did not reach the plan"
    );
    assert_eq!(
        plan.opened["claude-code"].merge,
        crate::config::MergeStrategy::Squash,
        "the plan lost what the effective screen opened on"
    );
}

/// The two tables a save carries go to the two files they belong in.
///
/// `plan_of` exists because this was four lines inside a loop that needs a
/// terminal, where putting the merged view into the contract's slot was
/// something no test could see.
#[test]
fn a_save_sends_the_contracts_table_to_the_contract_and_the_merged_one_to_the_checkout() {
    let contract = Config {
        merge: crate::config::MergeStrategy::Rebase,
        ..Config::default()
    };
    let here = Config {
        merge: crate::config::MergeStrategy::Squash,
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

    let plan = super::plan_of(&app);
    assert_eq!(
        plan.opened.get("claude-code").expect("a table").merge,
        crate::config::MergeStrategy::Squash,
        "the plan did not retain the effective value it measures edits against"
    );
    assert_eq!(
        plan.rows.get("claude-code").expect("a table").merge,
        crate::config::MergeStrategy::Squash,
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
    // The screen offered `standard` and `receipt-driven` with a sentence about
    // the row and none about either word, so the arrow key that decides what a
    // verdict is bound to was a guess between two spellings.
    let mut app = options();
    while app.setting_at_cursor() != Some(Setting::ReviewProtocol) {
        app.press(Key::Down);
    }

    let painted = rows(&app, 96, 30).join("\n");
    assert!(
        painted.contains("a verdict is a review of the head it was published against"),
        "the answer in force is not explained:\n{painted}"
    );

    // And each one as it is reached, which is where somebody is when they are
    // choosing rather than reading.
    app.press(Key::Char(' '));
    assert_eq!(app.focus, Focus::Picking, "the picker did not open");
    while app.picker().get(app.pick).map(String::as_str) != Some("receipt-driven") {
        app.press(Key::Down);
    }
    let picking = rows(&app, 96, 30).join("\n");
    assert!(
        picking.contains("must carry a receipt"),
        "arrowing onto an answer does not say what it means:\n{picking}"
    );
    // The floor: it is the answer under the picker that is explained, not
    // whatever happens to be set.
    assert!(
        !picking.contains("a verdict is a review of the head it was published against"),
        "the explanation did not follow the picker's cursor:\n{picking}"
    );
}

#[test]
fn the_board_row_is_offered_only_by_a_tracker_that_has_one() {
    // A board is a GitHub Projects thing. `bindings/linear.md` and
    // `bindings/trello.md` declare no board mirror, and the transport asks
    // `cfg(config, "project board")` only under GitHub — so on those trackers
    // the row was a question with no answer, offered beside the ones that have
    // them.
    let mut app = options();

    // The floor: on GitHub it is there, or the assertion below holds for the
    // trivial reason that the row never appears at all.
    assert!(
        app.rows().contains(&Setting::Board),
        "the board row is not offered on GitHub, so this measures nothing"
    );
    let before = app.row_count();

    while app.setting_at_cursor() != Some(Setting::Tracker) {
        app.press(Key::Down);
    }
    while app.shown_value(Setting::Tracker) != "linear" {
        app.press(Key::Right);
    }

    assert!(
        !app.rows().contains(&Setting::Board),
        "a tracker with no board still offers the row that names one"
    );
    // The cursor walks what is drawn. `row_count` held its own copy of how many
    // rows there are, so a page that stopped offering one would have had a
    // cursor position with nothing under it.
    assert_eq!(
        app.row_count(),
        before - 1,
        "the cursor still counts a row the list no longer draws"
    );
    let painted = rows(&app, 96, 30).join("\n");
    assert!(
        !painted.contains("Project board"),
        "the row is gone from the list and still on the screen:\n{painted}"
    );

    // And back, because a tracker is a row somebody changes their mind about.
    while app.shown_value(Setting::Tracker) != "github" {
        app.press(Key::Right);
    }
    assert!(
        app.rows().contains(&Setting::Board),
        "choosing GitHub again did not bring the board row back"
    );
}

#[test]
fn a_folder_row_is_walked_rather_than_answered_at_every_step() {
    // Walking and choosing were one key: every step down set the row and closed
    // the list, so reaching a folder three deep wrote three answers nobody
    // wanted and reopened the picker three times. That is what a picker with no
    // navigation feels like — the list was always right, and the key that moves
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

    // A separator would make one folder out of two, or reach out of the
    // directory the walk is in — refused where the message can be about the
    // name that was typed.
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
    // The shell looked at the filesystem before every frame: a `read_dir` of
    // wherever the walk was, and twenty-six `is_dir` probes for the drive
    // letters — on every keypress, on every row, whether or not anything on
    // screen could use them.
    //
    // The cost is not the point. A mapped network drive that has gone away
    // answers `is_dir` when its share times out, which is seconds of a screen
    // that has stopped taking keys, on a row that has nothing to do with
    // folders.
    let mut app = options();

    // The rows that answer with a path, and only those.
    let mut asked_on = Vec::new();
    for _ in 0..app.row_count() {
        if app.wants_folders() {
            asked_on.push(app.setting_at_cursor());
        }
        app.press(Key::Down);
    }
    assert_eq!(
        asked_on,
        vec![Some(Setting::Worktree)],
        "the disk is read for rows whose answer is not a path"
    );

    // And the shell is what asks. A screen that reached for the filesystem
    // itself could not be driven by this test at all.
    let shell = include_str!("../tui.rs");
    assert!(
        shell.contains("if app.wants_folders() {"),
        "the frame reads the disk without asking whether anything wants it"
    );
    assert!(
        !app.wants_folders() || app.setting_at_cursor() == Some(Setting::Worktree),
        "the question answers yes somewhere it should not"
    );
}

#[test]
fn the_boards_on_offer_belong_to_the_checkout_the_page_is_showing() {
    // They are asked for once, when the picker on that row first opens. Nothing
    // cleared them, so pointing the page at another checkout left the previous
    // repository's projects on offer under this one's name — and a board is
    // chosen once and mirrored to for the life of the configuration.
    let mut app = options();
    app.repository = "C:/work/first".to_owned();
    app.boards = vec![("acme/7".to_owned(), "Sprint".to_owned())];

    // The row offers them while the page is still showing that checkout.
    while app.setting_at_cursor() != Some(Setting::Board) {
        app.press(Key::Down);
    }
    assert!(
        app.picker().iter().any(|entry| entry == "acme/7"),
        "the boards that were read are not on offer, so this measures nothing"
    );

    // Choosing another checkout drops them. Held here rather than by reading
    // the shell's source: an assertion that the text `app.boards.clear()`
    // appears is one a commented-out line satisfies, which is what the first
    // version of this test did — it stayed green through a mutation that turned
    // the clearing off.
    let dropped = app.choose_screen(crate::tui::app::Screen::Repository, "C:/work/second");
    assert_eq!(dropped, Action::Reload("C:/work/second".to_owned()));
    assert!(
        app.boards.is_empty(),
        "changing the checkout leaves another repository's boards on offer"
    );
    // And the question is asked about the checkout on the page rather than the
    // directory this process happens to have been started in.
    assert!(
        include_str!("../tui.rs").contains("std::path::PathBuf::from(&app.repository)"),
        "the boards are read from wherever the process was run, not from what \
         the page is showing"
    );

    // With them gone, opening the picker asks again rather than offering
    // nothing: an empty list is what makes the next open a question.
    app.boards.clear();
    app.focus = Focus::List;
    assert_eq!(
        app.press(Key::Char(' ')),
        Action::ListBoards,
        "an emptied list does not make the picker ask again"
    );
}

#[test]
fn the_row_wearing_the_arrows_mark_is_the_row_the_arrows_change() {
    // Reported from a real terminal: on the options page the selection appeared
    // to be on the row *after* the one being changed.
    //
    // The page draws its own preferences above the settings and walks both with
    // one cursor, so a row's place in the settings list is not its place under
    // that cursor. The highlight subtracted the rows above; the `‹` that says
    // *the arrows move this row* did not, and neither did the scrollbar thumb.
    // Three readers of one offset in one function, and only one had it — which
    // the highlight's own comment says was the bug it had already been fixed
    // for: *`Delivery route` was selected and `Worktree location` was the one
    // wearing the mark*.
    let mut app = options();
    // Onto a settings row, past the screen preferences at the top.
    walk_onto_a_setting(&mut app);
    // And one further, so the offset cannot be zero by luck.
    app.press(Key::Down);
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

    for key in [Key::Left, Key::Right] {
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
            "orchestrate=gpt-5.6",
            "{key:?} changed a target without an advertised operation"
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
    // The first answer, one border below the row: the box opens directly under
    // it, so the entry after the border sits at `at + 2`. Anything further down
    // is a box hanging off a row that is not the one being changed.
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
    // The highest-stakes path on this page, in its own words: *a screen that
    // quietly wrote one repository's rows into another would be the worst thing
    // this page could do*.
    //
    // The reload wrote into the page's tables agent by agent. The caller moves
    // the row that says which checkout is showing **only** on success — which is
    // right, and was not enough: a refusal on the second agent left the first
    // one already carrying the chosen checkout's answers, under the old
    // checkout's name. A save then sends `app.configs` to the checkout
    // `app.repository` names, so the mixture goes to the wrong one.
    let contract = Config {
        merge: crate::config::MergeStrategy::MergeCommit,
        ..Config::default()
    };
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
                base.merge = crate::config::MergeStrategy::Squash;
            }
            (adapter.slug, base)
        })
        .collect();
    app.repository = "H:/here".to_owned();
    let before = app.configs.clone();

    // Succeeds until it meets the marked contract, which is not the first.
    fn layering(base: &Config, _repo: &std::path::Path) -> Result<Config, crate::outcome::Refusal> {
        if base.merge == crate::config::MergeStrategy::Squash {
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
        theirs.merge = crate::config::MergeStrategy::Rebase;
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
            .merge,
        crate::config::MergeStrategy::Rebase,
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
/// `unset` is a list of words with nothing to place them, and `→` walks in while
/// `←` walks up: neither key is usable without knowing where you are.
///
/// This is the row an operator reported as *muy limitado, no permite
/// exploración entre carpetas ni discos*. The walking was there; being able to
/// tell where it had got to was not.
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
