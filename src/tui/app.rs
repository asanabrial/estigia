//! The state a key press moves, with no terminal in it.

use crate::config::{
    AGENT_SETTINGS, Config, ModelRouting, ModelTarget, OPTIONS_SETTINGS, Scope, Setting,
};
use crate::outcome::Refusal;
use crate::setup::{AGENTS, AgentAdapter, Applies, ModelCatalogSource};
use crate::tui::words::{TONGUES, Tongue, fill, t};
use crate::tui::{InstallReceipt, Plan};

/// What the shell around the state machine has to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Nothing; redraw and wait.
    None,
    /// Install into the ticked agents, with the tables as they now stand.
    Save,
    /// Install the pre-push guard into this repository.
    Guard,
    /// Run the checks and show them.
    Doctor,
    /// Remember the screen's language, which has already changed.
    Remember,
    /// Leave.
    Quit,
    /// Read the chosen repository's own answers and hand them back.
    ///
    /// It carries the checkout **because the row has not moved yet**.
    Reload(String),
    /// Make a directory the operator named, and list it.
    MakeFolder(std::path::PathBuf),
    /// Load one host's advisory model catalog and feed it back into the App.
    LoadModelCatalog(&'static AgentAdapter),
    /// Work out what removing this would take, without removing it.
    PreviewUninstall(Removal),
    /// Remove this, once it has been confirmed.
    Uninstall(Removal),
}

/// The blocking work the shell has handed off while the screen waits for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    /// The checks the Doctor page shows.
    Doctor,
    /// One host's advisory model catalog.
    ModelCatalog(&'static AgentAdapter),
}

/// What the screen can take out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// One agent, by slug.
    Agent(&'static str),
    /// The pre-push guard of the repository shown.
    Guard,
}

/// A preference of this screen, on this machine.
///
/// Not a [`Setting`], and it must never become one: no agent reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The language this screen speaks.
    Language,
    /// Which checkout's own answers this page is showing.
    Repository,
}

/// The preferences of this screen, in the order they are shown.
pub const SCREEN_ROWS: &[Screen] = &[Screen::Language, Screen::Repository];

impl Screen {
    /// The label written beside it.
    pub fn label(self, tongue: Tongue) -> &'static str {
        match self {
            Self::Language => t!(tongue, "Interface language"),
            Self::Repository => t!(tongue, "Repository shown"),
        }
    }

    /// One line saying what it decides.
    pub fn about(self, tongue: Tongue) -> &'static str {
        match self {
            Self::Language => t!(
                tongue,
                "the language this screen speaks: this machine only, never a contract"
            ),
            Self::Repository => t!(
                tongue,
                "which checkout the rows above answer for, out of the ones that answer for themselves"
            ),
        }
    }

    /// Where the answer goes, said the way a contract row says it.
    pub fn lands(self, tongue: Tongue) -> &'static str {
        match self {
            Self::Language => t!(
                tongue,
                "applied at once, and remembered in ~/.estigia/screen"
            ),
            Self::Repository => t!(tongue, "this screen only — it changes what is shown"),
        }
    }

    /// What it accepts, in prose.
    pub fn accepted(self, tongue: Tongue) -> &'static str {
        match self {
            Self::Language => t!(tongue, "one of the languages this screen has words for"),
            Self::Repository => t!(tongue, "a checkout that answers for itself"),
        }
    }

    /// The answers it offers.
    pub fn answers(self) -> Vec<String> {
        match self {
            Self::Language => TONGUES
                .iter()
                .map(|tongue| tongue.name().to_owned())
                .collect(),
            // Fed rather than known: which checkouts answer is a question about the disk.
            Self::Repository => Vec::new(),
        }
    }
}

/// Which screen is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The menu.
    Home,
    /// Every adapter, and whether it is chosen.
    Agents,
    /// One agent's own settings, or everyone's at once.
    Detail,
    /// The settings that are the same whichever agent asks.
    Options,
    /// What the checks said.
    Doctor,
    /// The keys.
    Help,
}

/// A verdict on something that already happened, laid over the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modal {
    /// Whether the thing it reports on succeeded.
    pub ok: bool,
    /// The verdict, in a few words.
    pub title: String,
    /// What to do about it, when there is anything to do.
    pub detail: Option<String>,
}

/// What a save would write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    /// The agents it would install.
    pub installs: Vec<&'static AgentAdapter>,
    /// The settings it would move, by the agent whose table they are in.
    pub changes: Vec<(&'static AgentAdapter, Vec<Setting>)>,
}

impl Review {
    /// Whether a save would write nothing at all.
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty() && self.changes.is_empty()
    }
}

/// One entry on the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// What it says.
    pub label: &'static str,
    /// What it is for.
    pub about: &'static str,
    /// Where it goes, or what it does.
    pub goes: Goes,
}

/// What choosing a menu entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goes {
    /// To another page.
    To(Page),
    /// Straight out to the caller.
    Doing(Deed),
}

/// What a menu entry does without opening a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deed {
    /// Install the pre-push guard.
    Guard,
    /// Run the checks.
    Doctor,
    /// Leave.
    Quit,
}

/// The menu, in the order somebody meets them.
pub const MENU: &[Entry] = &[
    Entry {
        label: "Agents",
        about: "choose agents, and open each one's own settings",
        goes: Goes::To(Page::Agents),
    },
    Entry {
        label: "Options",
        about: "the settings that are the same whichever agent asks",
        goes: Goes::To(Page::Options),
    },
    Entry {
        label: "Push guard",
        about: "the pre-push hook: the one boundary no agent can go around",
        goes: Goes::Doing(Deed::Guard),
    },
    Entry {
        label: "Doctor",
        about: "check that everything a run needs before it swears actually works",
        goes: Goes::Doing(Deed::Doctor),
    },
    Entry {
        label: "Help",
        about: "the keys, and what this screen is",
        goes: Goes::To(Page::Help),
    },
    // Listed even though `q` also works.
    Entry {
        label: "Quit",
        about: "leave — nothing is written that was not already installed",
        goes: Goes::Doing(Deed::Quit),
    },
];

/// What this screen was opened to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The agents, each opening onto its own settings, and the options page beside them.
    Setup,
    /// One table, every row of it, in table order.
    OneTable(Option<&'static AgentAdapter>),
}

/// The picker entry that opens somewhere to type.
pub const TYPE_IT: &str = "type a value…";

/// Per-agent settings in interactive order.
pub(super) const AGENT_TUI_SETTINGS: &[Setting] = &[
    Setting::Profile,
    Setting::Delivery,
    Setting::Transitions,
    Setting::LoopPlugin,
    Setting::GoalLoopPlugin,
];

/// The same, for an adapter that has the Estigia loop and no generic goal and
/// loop plugin to switch.
pub(super) const AGENT_TUI_SETTINGS_WITH_LOOP_ONLY: &[Setting] = &[
    Setting::Profile,
    Setting::Delivery,
    Setting::Transitions,
    Setting::LoopPlugin,
];

/// The same, for an adapter with no loop to switch.
///
/// A second slice rather than a filter: `rows` hands back `&'static [Setting]`
/// and it is the **one** list both the cursor and the drawing walk.
/// [`Setting::applies_to_agent`] decides which of the two an adapter is offered,
/// and the writer ([`crate::setup::write_agent_configuration_wholly`]) is
/// narrowed the same way, so the screen and the file agree.
pub(super) const AGENT_TUI_SETTINGS_WITHOUT_LOOP_PLUGIN: &[Setting] =
    &[Setting::Profile, Setting::Delivery, Setting::Transitions];

/// A whole table in interactive order, without the synthetic model section.
///
/// [`SETTINGS`] filtered to everything but `Model routing`, which the screen
/// draws as its own projected block rather than as one row. Repository-only
/// rows are here on purpose: the shared contract screen is where an operator
/// looks for a row no single agent answers, and offering it is what makes it
/// reachable.
pub(super) const TUI_SETTINGS: &[Setting] = &[
    Setting::Profile,
    Setting::Delivery,
    Setting::Transitions,
    Setting::Tracker,
    Setting::RepositoryHost,
    Setting::IssueRequirement,
    Setting::Integration,
    Setting::Worktree,
    Setting::DeliveryPreset,
    Setting::Workflow,
    Setting::StateNames,
    Setting::SelfReview,
    Setting::ReadyBy,
    Setting::ExternalReviewers,
    Setting::CorrectionsAfterReady,
    Setting::MergeBy,
    Setting::Boundaries,
    Setting::Window,
    Setting::GitOperationWindow,
    Setting::Evidence,
    Setting::PublicationLane,
    Setting::ReviewBudget,
    Setting::ChangeSize,
    Setting::CorrectionsPerLineage,
    Setting::LoopPlugin,
    Setting::GoalLoopPlugin,
    Setting::Summary,
    Setting::Body,
];

/// The per-agent rows `adapter` is offered: the rows that can act on it.
///
/// Two rows narrow — the loop and the generic goal and loop plugin — and
/// [`Setting::applies_to_agent`] answers which list. The same predicate decides
/// what the adapter's own file carries, so a row the screen does not draw is
/// not written either.
fn agent_rows(adapter: &AgentAdapter) -> &'static [Setting] {
    rows_offered(
        Setting::LoopPlugin.applies_to_agent(adapter),
        Setting::GoalLoopPlugin.applies_to_agent(adapter),
    )
}

fn rows_offered(loop_row: bool, goal_loop_row: bool) -> &'static [Setting] {
    match (loop_row, goal_loop_row) {
        (true, true) => AGENT_TUI_SETTINGS,
        (true, false) => AGENT_TUI_SETTINGS_WITH_LOOP_ONLY,
        (false, _) => AGENT_TUI_SETTINGS_WITHOUT_LOOP_PLUGIN,
    }
}

/// One typed action in a derived model row's picker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelEntry {
    Model(String),
    TypeModel,
    Inherit,
}

/// What one model target holds across the canonical destinations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModelAssignment {
    /// Every destination has this exact route, or none of them names the target.
    Shared(Option<crate::config::Route>),
    /// At least two selected destinations have different installed values.
    Divergent,
}

/// Where the keys go, within a page of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Moving through the settings.
    List,
    /// Choosing from the answers a setting offers.
    Picking,
    /// Typing a new value into the selected one.
    Editing,
    /// Naming a folder to make, inside the one the picker is looking at.
    Naming,
}

/// The whole screen's state.
#[derive(Debug, Clone)]
pub struct App {
    /// Which screen is showing.
    pub page: Page,
    /// Which menu entry the cursor is on.
    pub entry: usize,
    /// The last thing an action reported, shown until something else happens.
    pub report: Option<String>,
    /// A verdict laid over whatever is showing, until a key dismisses it.
    pub modal: Option<Modal>,
    /// What this screen was opened to do.
    pub purpose: Purpose,
    /// Every adapter, and whether it is chosen.
    pub agents: Vec<(&'static AgentAdapter, bool)>,
    /// Which adapter the cursor is on, unless it is on [`Self::everyone`].
    pub agent: usize,
    /// Whether the cursor is on the row that stands for every chosen agent.
    pub everyone: bool,
    /// The adapters that were already configured when the screen opened.
    pub installed_agents: Vec<&'static str>,
    /// Each adapter's configuration, as edited so far, by slug.
    pub configs: std::collections::BTreeMap<&'static str, Config>,
    /// The same, as they were when the screen opened.
    pub installed: std::collections::BTreeMap<&'static str, Config>,
    /// The same again, with **no checkout's** rows on them.
    pub bare: std::collections::BTreeMap<&'static str, Config>,
    /// The checkout whose own answers this page is showing.
    pub repository: String,
    /// How Estigia's push guard reads in that checkout.
    pub guard_state: crate::harness::guard::State,
    /// `github <owner>/<name>`, when that checkout's remote names a GitHub repository.
    pub remote: Option<String>,
    /// The checkouts that answer for themselves, as the shell last read them.
    pub repositories: Vec<String>,
    /// Where the folder picker is looking, while it is open.
    pub browsing: Option<std::path::PathBuf>,
    /// The drives this machine has, for a walk that has to leave one.
    pub drives: Vec<String>,
    /// The directories the shell last looked up, for a row that takes one.
    pub folders: Vec<String>,
    /// Dynamic advisory model IDs the shell has loaded, by their owning adapter.
    model_catalogs: std::collections::BTreeMap<&'static str, Vec<String>>,
    catalogs_failed: std::collections::BTreeSet<&'static str>,
    /// The model target whose custom ID is being edited.
    model_edit: Option<&'static str>,
    /// The `External reviewers` stage being typed: its place in the list, which
    /// is one past the end for a new one.
    stage_edit: Option<usize>,
    /// Which row the cursor is on, on the detail page.
    pub selected: usize,
    /// Which row the cursor is on, on the options page.
    pub options_at: usize,
    /// Whether what a save would write is laid over the screen, waiting to be confirmed.
    pub reviewing: bool,
    /// What a removal would take, laid over the screen, waiting to be confirmed.
    pub removing: Option<(Removal, Vec<String>)>,
    /// The page the help was opened from, which is where it goes back to.
    help_from: Page,
    /// Where the keys go.
    pub focus: Focus,
    /// Which answer the picker is on, while one is open.
    pub pick: usize,
    /// How far down a read-only page has been scrolled.
    pub scroll: u16,
    /// What is being typed, while editing.
    pub draft: String,
    /// The last refusal, shown until something else happens.
    pub message: Option<String>,
    /// Whether the last key put up the unsaved-changes warning, so the next may discard.
    pub warned: bool,
    armed: bool,
    busy: Option<Busy>,
    /// The language this screen speaks.
    pub tongue: Tongue,
}

impl App {
    /// Opens on the configuration that is installed.
    pub fn new(installed: Config) -> Self {
        Self::with_agents(installed, &[])
    }

    /// Opens with the adapters that are already configured ticked.
    pub fn with_agents(installed: Config, configured: &[&str]) -> Self {
        Self::with_configs(
            &AGENTS
                .iter()
                .map(|adapter| (adapter.slug, installed.clone()))
                .collect(),
            configured,
        )
    }

    /// Opens with each adapter's own table.
    pub fn with_configs(
        installed: &std::collections::BTreeMap<&'static str, Config>,
        configured: &[&str],
    ) -> Self {
        let configs: std::collections::BTreeMap<&'static str, Config> = AGENTS
            .iter()
            .map(|adapter| {
                (
                    adapter.slug,
                    installed.get(adapter.slug).cloned().unwrap_or_default(),
                )
            })
            .collect();
        Self {
            // Until a caller that knows better says otherwise.
            bare: configs.clone(),
            agents: AGENTS
                .iter()
                .map(|adapter| (adapter, configured.contains(&adapter.slug)))
                .collect(),
            installed_agents: AGENTS
                .iter()
                .map(|adapter| adapter.slug)
                .filter(|slug| configured.contains(slug))
                .collect(),
            page: Page::Home,
            entry: 0,
            report: None,
            modal: None,
            purpose: Purpose::Setup,
            agent: 0,
            everyone: true,
            installed: configs.clone(),
            configs,
            repository: String::new(),
            guard_state: crate::harness::guard::State::Absent,
            remote: None,
            repositories: Vec::new(),
            browsing: None,
            drives: Vec::new(),
            folders: Vec::new(),
            model_catalogs: std::collections::BTreeMap::new(),
            catalogs_failed: std::collections::BTreeSet::new(),
            model_edit: None,
            stage_edit: None,
            selected: 0,
            options_at: 0,
            reviewing: false,
            removing: None,
            help_from: Page::Home,
            focus: Focus::List,
            pick: 0,
            scroll: 0,
            draft: String::new(),
            message: None,
            armed: false,
            busy: None,
            warned: false,
            tongue: Tongue::English,
        }
    }

    /// Opens on one table, edited whole: `estigia config edit`.
    pub fn one_table(adapter: Option<&'static AgentAdapter>, installed: Config) -> Self {
        let carrier = adapter.unwrap_or(&AGENTS[0]);
        let mut configs = std::collections::BTreeMap::new();
        configs.insert(carrier.slug, installed);
        Self {
            bare: configs.clone(),
            page: Page::Detail,
            entry: 0,
            report: None,
            modal: None,
            purpose: Purpose::OneTable(adapter),
            agents: vec![(carrier, true)],
            installed_agents: vec![carrier.slug],
            agent: 0,
            everyone: false,
            installed: configs.clone(),
            configs,
            repository: String::new(),
            guard_state: crate::harness::guard::State::Absent,
            remote: None,
            repositories: Vec::new(),
            browsing: None,
            drives: Vec::new(),
            folders: Vec::new(),
            model_catalogs: std::collections::BTreeMap::new(),
            catalogs_failed: std::collections::BTreeSet::new(),
            model_edit: None,
            stage_edit: None,
            selected: 0,
            options_at: 0,
            reviewing: false,
            removing: None,
            help_from: Page::Home,
            focus: Focus::List,
            pick: 0,
            scroll: 0,
            draft: String::new(),
            message: None,
            armed: false,
            busy: None,
            warned: false,
            tongue: Tongue::English,
        }
    }

    /// Records an install that landed, and says so over the screen.
    pub fn installed_now(&mut self, receipt: InstallReceipt) {
        let summary = receipt.summary.clone();
        self.acknowledge(receipt);
        self.modal = Some(Modal {
            ok: true,
            title: summary,
            detail: None,
        });
    }

    /// Applies only read-back evidence from an install that later failed.
    pub fn installed_partially(&mut self, receipt: InstallReceipt) {
        self.acknowledge(receipt);
    }

    fn acknowledge(&mut self, receipt: InstallReceipt) {
        let InstallReceipt {
            read_back,
            unlayered_read_back,
            contract_read_back: _,
            agent_read_back: _,
            local_read_back: _,
            acknowledged,
            completed,
            repository,
            repository_settings,
            ..
        } = receipt;
        if let Some(repository) = &repository {
            for installed in self.installed.values_mut() {
                for setting in &repository_settings {
                    let _ = setting.apply(installed, &setting.value_of(repository));
                }
            }
        }
        for (slug, settings) in &acknowledged {
            if let Some(config) = unlayered_read_back.get(slug) {
                let bare = self.bare.entry(slug).or_default();
                for setting in settings {
                    let _ = setting.apply(bare, &setting.value_of(config));
                }
            }
            if let Some(config) = read_back.get(slug) {
                let installed = self.installed.entry(slug).or_default();
                for setting in settings {
                    let _ = setting.apply(installed, &setting.value_of(config));
                }
            }
        }
        self.installed_agents = AGENTS
            .iter()
            .map(|adapter| adapter.slug)
            .filter(|slug| self.installed_agents.contains(slug) || completed.contains(slug))
            .collect();
        self.message = None;
    }

    /// Records an install that did not land, and says so over the screen.
    pub fn install_failed(&mut self, title: String, detail: String) {
        self.message = None;
        self.modal = Some(Modal {
            ok: false,
            title,
            detail: Some(detail),
        });
    }

    /// Whether this screen edits one table rather than choosing among agents.
    pub fn pinned(&self) -> bool {
        matches!(self.purpose, Purpose::OneTable(_))
    }

    /// The adapter the cursor is on.
    pub fn current(&self) -> &'static str {
        self.agents[self.agent.min(self.agents.len() - 1)].0.slug
    }

    /// The adapter the cursor is on, whole.
    pub fn adapter(&self) -> &'static AgentAdapter {
        self.agents[self.agent.min(self.agents.len() - 1)].0
    }

    /// That adapter's configuration.
    pub fn config(&self) -> &Config {
        &self.configs[self.current()]
    }

    /// That adapter's configuration, to write into.
    pub fn config_mut(&mut self) -> &mut Config {
        let slug = self.current();
        // Every adapter is seeded in the constructor, so this cannot be absent.
        let Some(config) = self.configs.get_mut(slug) else {
            unreachable!("{slug} has no table, so its answers would go nowhere")
        };
        config
    }

    /// The rows this screen offers.
    pub fn rows(&self) -> &'static [Setting] {
        match self.purpose {
            // A named adapter is edited through the rows that differ by agent; a
            // machine-wide or repository row is `config set`'s, not one agent's.
            Purpose::OneTable(Some(adapter)) => agent_rows(adapter),
            Purpose::OneTable(None) => TUI_SETTINGS,
            Purpose::Setup => match self.page {
                Page::Options => OPTIONS_SETTINGS,
                Page::Detail => self.per_agent_rows(),
                Page::Home | Page::Agents | Page::Doctor | Page::Help => &[],
            },
        }
    }

    /// The detail page's rows for whoever it is open on.
    ///
    /// On **all installed agents** the answer is written to every chosen
    /// adapter, so a row only some of them act on is not offered there: it is
    /// [`Setting::applies_to_agent`] for the one adapter, or for every chosen
    /// one when the page is open on all of them.
    fn per_agent_rows(&self) -> &'static [Setting] {
        if self.everyone {
            let every = |setting: Setting| {
                self.chosen()
                    .into_iter()
                    .all(|adapter| setting.applies_to_agent(adapter))
            };
            rows_offered(every(Setting::LoopPlugin), every(Setting::GoalLoopPlugin))
        } else {
            agent_rows(self.adapter())
        }
    }

    /// The model rows projected beneath the settings on an agent's page.
    pub fn model_targets(&self) -> Vec<ModelTarget> {
        if self.page != Page::Detail {
            return Vec::new();
        }
        ModelRouting::visible_targets()
    }

    /// Whether the concrete adapter has reviewed presets to offer.
    pub fn has_model_profiles(&self) -> bool {
        self.page == Page::Detail
            && self
                .model_adapter()
                .is_some_and(|adapter| !adapter.model_profiles().is_empty())
    }

    /// Whether the synthetic profile row is under the cursor.
    pub fn model_profile_at_cursor(&self) -> bool {
        self.has_model_profiles() && self.cursor() == self.rows().len()
    }

    /// How many rows the list under the cursor has.
    pub fn row_count(&self) -> usize {
        match self.page {
            // `self.rows()`, not the full list.
            Page::Options => self.rows().len() + SCREEN_ROWS.len(),
            _ => {
                self.rows().len()
                    + usize::from(self.has_model_profiles())
                    + self.model_targets().len()
            }
        }
    }

    /// The line this page is really showing, clamped to what exists.
    pub fn showing(&mut self, furthest: u16) -> u16 {
        self.scroll = self.scroll.min(furthest);
        self.scroll
    }

    /// The screen preference under the cursor, when the cursor is on one.
    pub fn screen_at_cursor(&self) -> Option<Screen> {
        if self.page != Page::Options {
            return None;
        }
        // First, not last: these two decide what the rows under them *mean*.
        SCREEN_ROWS.get(self.options_at).copied()
    }

    /// Where the cursor is on the page of rows that is showing.
    pub fn cursor(&self) -> usize {
        if self.page == Page::Options {
            self.options_at
        } else {
            self.selected
        }
    }

    fn cursor_mut(&mut self) -> &mut usize {
        if self.page == Page::Options {
            &mut self.options_at
        } else {
            &mut self.selected
        }
    }

    /// The setting under the cursor, if the page has any.
    pub fn setting_at_cursor(&self) -> Option<Setting> {
        self.setting_at(self.cursor())
    }

    fn setting_at(&self, index: usize) -> Option<Setting> {
        let rows = self.rows();
        if self.page == Page::Options {
            // The screen rows come first.
            return rows.get(index.checked_sub(SCREEN_ROWS.len())?).copied();
        }
        rows.get(index).copied()
    }

    /// The answers offered beyond the row's own vocabulary: what this checkout suggests.
    fn suggestions(&self, setting: Setting) -> Vec<String> {
        match setting {
            Setting::Tracker | Setting::RepositoryHost => self.remote.iter().cloned().collect(),
            Setting::Worktree => crate::paths::home_dir()
                .ok()
                .map(|home| home.join("worktrees").join("<repo>").join("<branch>"))
                .filter(|template| template.is_absolute())
                .map(|template| template.display().to_string())
                .into_iter()
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Every answer the row offers to pick: its vocabulary, then what is suggested.
    pub fn offered(&self, setting: Setting) -> Vec<String> {
        let mut entries: Vec<String> = setting
            .answers()
            .choices
            .iter()
            .map(|choice| (*choice).to_owned())
            .collect();
        for suggestion in self.suggestions(setting) {
            if !entries.contains(&suggestion) {
                entries.push(suggestion);
            }
        }
        entries
    }

    /// The offered answer `delta` places from `from`, wrapping; a value not offered steps onto the first, unless the row is open and holds something typed.
    pub fn step_of(&self, setting: Setting, from: &str, delta: isize) -> Option<String> {
        let entries = self.offered(setting);
        let count = entries.len() as isize;
        if count == 0 {
            return None;
        }
        let next = match entries.iter().position(|entry| entry == from) {
            Some(at) => (at as isize + delta).rem_euclid(count) as usize,
            None if !setting.answers().closed && !from.is_empty() => return None,
            None => 0,
        };
        entries.into_iter().nth(next)
    }

    /// Whether an open row has too few answers to be worth a picker, and is typed into instead.
    pub fn is_text_field(&self, setting: Setting) -> bool {
        !setting.answers().closed && !setting.takes_a_directory() && self.offered(setting).len() < 2
    }

    /// The derived model target under the cursor, if it is on one.
    pub fn model_target_at_cursor(&self) -> Option<ModelTarget> {
        let model_start = self.rows().len() + usize::from(self.has_model_profiles());
        if self.page == Page::Options || self.cursor() < model_start {
            return None;
        }
        self.model_targets()
            .get(self.cursor().saturating_sub(model_start))
            .copied()
    }

    /// The setting under the cursor.
    pub fn setting(&self) -> Setting {
        self.setting_at_cursor().unwrap_or(AGENT_TUI_SETTINGS[0])
    }

    /// What one setting is worth here: the one concrete adapter it is about
    /// when there is one — the detail page's, or the pinned table's — and,
    /// on **all installed agents**, the unanimous answer of every
    /// adapter that page's write actually reaches, because no single one of
    /// them can speak for it.
    pub fn applies(&self, setting: Setting) -> Applies {
        match self.purpose {
            // No adapter at all to ask: the write has nothing to disagree over.
            Purpose::OneTable(None) => Applies::Held,
            // All chosen agents writes to every ticked adapter, while
            // `adapter()` still names whichever was opened last — so this
            // asks the adapters the write reaches, not that one.
            Purpose::Setup if self.everyone => self.applies_across(setting),
            Purpose::Setup => self.adapter().applies(setting),
            Purpose::OneTable(Some(adapter)) => adapter.applies(setting),
        }
    }

    /// The unanimous answer of every adapter [`Self::chosen`] reaches, or
    /// [`Applies::Held`] when they disagree: the shared contract is read by
    /// every one of them, so if any reached adapter needs a row edited the
    /// row must stay editable for the whole stop — `Held` never claims less
    /// than that. On a mixed answer this does overstate for the adapters
    /// that only answered `Asked`; see `docs/honesty.md`.
    fn applies_across(&self, setting: Setting) -> Applies {
        let mut answers = self
            .chosen()
            .into_iter()
            .map(|adapter| adapter.applies(setting));
        let Some(first) = answers.next() else {
            return Applies::Held;
        };
        if answers.all(|next| next == first) {
            first
        } else {
            Applies::Held
        }
    }

    /// The value shown for one row on the current page.
    pub fn shown_value(&self, setting: Setting) -> String {
        setting.value_of(self.shown_config(setting))
    }

    /// The whole table [`Self::shown_value`] reads that row out of.
    pub fn shown_config(&self, setting: Setting) -> &Config {
        &self.configs[self.shown_from(setting)]
    }

    /// The table one row's value is read out of.
    fn shown_from(&self, setting: Setting) -> &'static str {
        match setting.scope() {
            Scope::Agent if !self.everyone => self.current(),
            Scope::Agent | Scope::Everywhere | Scope::Repository | Scope::Machine => {
                self.interested().first().copied().unwrap_or(self.current())
            }
        }
    }

    /// Whether a row is not the same everywhere it is written.
    pub fn disagrees(&self, setting: Setting) -> bool {
        if setting.scope() == Scope::Agent && !self.everyone {
            return false;
        }
        let mut values = self
            .interested()
            .into_iter()
            .map(|slug| setting.value_of(&self.configs[slug]));
        let Some(first) = values.next() else {
            return false;
        };
        values.any(|value| value != first)
    }

    fn interested(&self) -> Vec<&'static str> {
        let chosen: Vec<&'static str> = self
            .agents
            .iter()
            .filter(|(_, on)| *on)
            .map(|(adapter, _)| adapter.slug)
            .collect();
        if chosen.is_empty() {
            self.agents
                .iter()
                .map(|(adapter, _)| adapter.slug)
                .collect()
        } else {
            chosen
        }
    }

    /// The exact tables one Model routing operation may change.
    fn model_destinations(&self) -> Vec<&'static str> {
        if self.everyone {
            self.agents
                .iter()
                .filter(|(_, on)| *on)
                .map(|(adapter, _)| adapter.slug)
                .collect()
        } else {
            vec![self.current()]
        }
    }

    fn model_assignment_in(
        &self,
        configs: &std::collections::BTreeMap<&'static str, Config>,
        target: &str,
    ) -> ModelAssignment {
        let mut destinations = self.model_destinations().into_iter();
        let Some(first) = destinations.next() else {
            return ModelAssignment::Divergent;
        };
        // The whole route, not the model half.
        let shared = configs[first].models.route(target).cloned();
        if destinations.all(|slug| configs[slug].models.route(target).cloned() == shared) {
            ModelAssignment::Shared(shared)
        } else {
            ModelAssignment::Divergent
        }
    }

    /// The exact assignment for one visible target across its write scope.
    pub(super) fn model_assignment(&self, target: &str) -> ModelAssignment {
        self.model_assignment_in(&self.configs, target)
    }

    /// The exact installed assignment for one visible target across its scope.
    pub(super) fn installed_model_assignment(&self, target: &str) -> ModelAssignment {
        self.model_assignment_in(&self.installed, target)
    }

    /// Renders one current or installed assignment in the screen language.
    fn rendered_model_assignment(&self, assignment: ModelAssignment) -> String {
        match assignment {
            ModelAssignment::Shared(Some(route)) => route.as_value(),
            ModelAssignment::Shared(None) => t!(self.tongue, "inherit").to_owned(),
            ModelAssignment::Divergent => t!(self.tongue, "different values").to_owned(),
        }
    }

    /// The value one derived model row displays.
    pub fn model_value(&self, target: &str) -> String {
        self.rendered_model_assignment(self.model_assignment(target))
    }

    /// The installed value one derived model row displays in its detail.
    pub fn installed_model_value(&self, target: &str) -> String {
        self.rendered_model_assignment(self.installed_model_assignment(target))
    }

    /// Whether one derived target differs from what is installed.
    pub fn model_changed(&self, target: &str) -> bool {
        // Routes, not the model half: an effort that moved is a row that changed.
        self.model_destinations().into_iter().any(|slug| {
            self.configs[slug].models.route(target) != self.installed[slug].models.route(target)
        })
    }

    /// Whether the complete route differs from what is installed.
    pub fn model_profile_changed(&self) -> bool {
        self.model_destinations()
            .into_iter()
            .any(|slug| self.configs[slug].models != self.installed[slug].models)
    }

    /// The preset matching every destination's complete route, or `custom`.
    pub fn model_profile_value(&self) -> String {
        let Some(adapter) = self.model_adapter() else {
            return t!(self.tongue, "custom").to_owned();
        };
        let mut destinations = self.model_destinations().into_iter();
        let Some(first) = destinations.next() else {
            return t!(self.tongue, "different values").to_owned();
        };
        let route = &self.configs[first].models;
        if destinations.any(|slug| self.configs[slug].models != *route) {
            return t!(self.tongue, "different values").to_owned();
        }
        adapter
            .model_profiles()
            .iter()
            .find(|profile| profile.routing().as_ref() == Some(route))
            .map_or_else(
                || t!(self.tongue, "custom").to_owned(),
                |profile| profile.name.to_owned(),
            )
    }

    /// Writes one value where this setting's scope says it belongs.
    pub fn set(&mut self, setting: Setting, value: &str) -> Result<(), Refusal> {
        let mut probe = self.config().clone();
        setting.apply(&mut probe, value)?;
        if setting == Setting::Models {
            let _ = self.replace_model_route(value)?;
            return Ok(());
        }
        // Stage every destination before replacing the live map, so a rejection
        // never leaves a partially shared answer behind.
        let destinations = match setting.scope() {
            Scope::Agent if !self.everyone => vec![self.current()],
            Scope::Agent => self.model_destinations(),
            Scope::Everywhere | Scope::Repository | Scope::Machine => {
                self.configs.keys().copied().collect()
            }
        };
        let mut staged = self.configs.clone();
        for slug in destinations {
            setting.apply(staged.entry(slug).or_default(), value)?;
        }
        self.configs = staged;
        Ok(())
    }

    /// Whether this row differs from what is installed.
    pub fn changed(&self, setting: Setting) -> bool {
        // The third door of the same family: an agent row under the shared answer is
        // written everywhere, so this reads it everywhere too.
        match setting.scope() {
            Scope::Agent if !self.everyone => {
                setting.value_of(self.config()) != setting.value_of(&self.installed[self.current()])
            }
            Scope::Agent | Scope::Everywhere | Scope::Repository | Scope::Machine => {
                self.interested().into_iter().any(|slug| {
                    setting.value_of(&self.configs[slug]) != setting.value_of(&self.installed[slug])
                })
            }
        }
    }

    /// Whether the agents are carrying an answer that is not installed yet.
    pub fn agents_touched(&self) -> bool {
        let mut now: Vec<&str> = self.chosen().iter().map(|a| a.slug).collect();
        let mut before = self.installed_agents.clone();
        now.sort_unstable();
        before.sort_unstable();
        now != before || self.any_unsaved(AGENT_SETTINGS)
    }

    /// Whether the repository's own rows are carrying an unsaved answer.
    pub fn options_touched(&self) -> bool {
        self.any_unsaved(OPTIONS_SETTINGS)
    }

    /// Whether any of these rows differs from what is installed.
    fn any_unsaved(&self, settings: &[Setting]) -> bool {
        settings.iter().any(|setting| {
            self.interested().iter().any(|slug| {
                setting.value_of(&self.configs[slug]) != setting.value_of(&self.installed[slug])
            })
        })
    }

    /// Whether anything at all is unsaved, anywhere.
    pub fn dirty(&self) -> bool {
        self.configs
            .iter()
            .any(|(slug, config)| config.render_rows() != self.installed[slug].render_rows())
    }

    /// Moves the cursor, wrapping at both ends.
    fn move_by(&mut self, delta: isize) {
        let count = self.row_count() as isize;
        if count == 0 {
            return;
        }
        let next = (self.cursor() as isize + delta).rem_euclid(count);
        *self.cursor_mut() = next as usize;
        self.message = None;
    }

    /// The adapters that are ticked.
    pub fn chosen(&self) -> Vec<&'static AgentAdapter> {
        self.agents
            .iter()
            .filter(|(_, on)| *on)
            .map(|(adapter, _)| *adapter)
            .collect()
    }

    /// Whether `guard::uninstall` would take the guard away: Estigia's hook, executable or not.
    pub fn guard_removable(&self) -> bool {
        matches!(
            self.guard_state,
            crate::harness::guard::State::Installed | crate::harness::guard::State::Inert
        )
    }

    /// The menu entry under the cursor.
    pub fn entry(&self) -> Entry {
        MENU[self.entry.min(MENU.len() - 1)]
    }

    /// Shows another page, with nothing of the last one left open.
    fn open(&mut self, page: Page) {
        if page == Page::Help {
            self.help_from = self.page;
        }
        self.page = page;
        self.focus = Focus::List;
        self.model_edit = None;
        self.draft.clear();
        self.message = None;
        self.scroll = 0;
        if matches!(page, Page::Detail | Page::Options) {
            let last = self.row_count().saturating_sub(1);
            let at = self.cursor_mut();
            *at = (*at).min(last);
        }
    }

    /// Whether a blocking action is running off the event thread.
    pub fn is_busy(&self) -> bool {
        self.busy.is_some()
    }

    /// The work the screen is waiting on, if any.
    pub fn busy(&self) -> Option<Busy> {
        self.busy
    }

    /// Hands the finished checks back to the page waiting on them.
    pub fn doctor_finished(&mut self, text: String) {
        self.busy = None;
        self.report = Some(text);
    }

    /// Clears the busy state after a worker died without answering.
    pub fn worker_died(&mut self) {
        match self.busy {
            Some(Busy::Doctor) => {
                self.busy = None;
                self.report = Some(t!(self.tongue, "the checks did not finish").to_owned());
            }
            Some(Busy::ModelCatalog(adapter)) => {
                self.model_catalog_failed(adapter);
                self.message = Some(fill!(
                    self.tongue,
                    "{agent} model catalog did not finish loading. Type a model ID instead.",
                    "agent" => adapter.display_name
                ));
            }
            None => {}
        }
    }

    fn wait_on(&mut self, action: Action) -> Action {
        match action {
            Action::Doctor => {
                self.page = Page::Doctor;
                self.scroll = 0;
                self.report = Some(
                    t!(
                        self.tongue,
                        "running the checks — the skill, the tracker CLI, the guard and the remote…"
                    )
                    .to_owned(),
                );
                self.busy = Some(Busy::Doctor);
            }
            Action::LoadModelCatalog(adapter) => {
                self.message = Some(fill!(
                    self.tongue,
                    "loading the {agent} model catalog…",
                    "agent" => adapter.display_name
                ));
                self.busy = Some(Busy::ModelCatalog(adapter));
            }
            _ => {}
        }
        action
    }

    /// Applies one key and says what the caller must do.
    pub fn press(&mut self, key: Key) -> Action {
        if self.busy.is_some() {
            return match key {
                Key::Char('q') | Key::Interrupt => self.leave(),
                _ => Action::None,
            };
        }
        self.armed = std::mem::take(&mut self.warned);
        let action = if key == Key::Interrupt {
            self.modal = None;
            self.reviewing = false;
            self.removing = None;
            self.leave()
        } else {
            self.dispatch(key)
        };
        let action = self.wait_on(action);
        if self.armed && !self.warned && self.message.as_deref() == Some(self.unsaved_warning()) {
            self.message = None;
        }
        action
    }

    fn unsaved_warning(&self) -> &'static str {
        if self.pinned() {
            t!(
                self.tongue,
                "unsaved changes — press again to discard, or s to save"
            )
        } else {
            t!(
                self.tongue,
                "unsaved changes — press again to discard, or s to install"
            )
        }
    }

    fn dispatch(&mut self, key: Key) -> Action {
        // The verdict is laid over whatever is showing, so the key that dismisses it
        // is spent on that and never also reaches the page behind it.
        if self.modal.take().is_some() {
            return Action::None;
        }
        if self.removing.is_some() {
            return self.in_removal(key);
        }
        if self.reviewing {
            return self.in_review(key);
        }
        match self.page {
            Page::Home => self.in_menu(key),
            Page::Doctor | Page::Help => self.in_text(key),
            Page::Agents => self.in_agents(key),
            Page::Detail | Page::Options => self.in_rows(key),
        }
    }

    /// Keys on the pages that only read.
    fn in_text(&mut self, key: Key) -> Action {
        match key {
            Key::Char('q') => return self.leave(),
            Key::Char('s') => return self.ask_to_save(),
            Key::Char('?') if self.page != Page::Help => self.open(Page::Help),
            Key::Down | Key::Char('j') => self.scroll = self.scroll.saturating_add(1),
            Key::Up | Key::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            // Anything else returns, so nobody is stuck on a page whose
            // keys they have to guess.
            _ => {
                let back = if self.page == Page::Help {
                    self.help_from
                } else {
                    Page::Home
                };
                self.open(back);
                // The text belonged to the page being left, and goes with it.
                self.report = None;
            }
        }
        Action::None
    }

    /// Keys on the menu.
    fn in_menu(&mut self, key: Key) -> Action {
        let count = MENU.len();
        match key {
            Key::Up | Key::Char('k') => {
                self.entry = (self.entry + count - 1) % count;
                self.report = None;
            }
            Key::Down | Key::Char('j') => {
                self.entry = (self.entry + 1) % count;
                self.report = None;
            }
            Key::Enter => {
                self.report = None;
                return match self.entry().goes {
                    Goes::To(page) => {
                        self.open(page);
                        Action::None
                    }
                    Goes::Doing(Deed::Guard) => Action::Guard,
                    Goes::Doing(Deed::Doctor) => Action::Doctor,
                    Goes::Doing(Deed::Quit) => self.leave(),
                };
            }
            Key::Char('u')
                if self.guard_removable() && self.entry().goes == Goes::Doing(Deed::Guard) =>
            {
                self.report = None;
                return Action::PreviewUninstall(Removal::Guard);
            }
            _ => return self.shared_key(key).unwrap_or(Action::None),
        }
        Action::None
    }

    /// The pane that is open, if one is: while it is, every key belongs to it.
    fn in_open_pane(&mut self, key: Key) -> Option<Action> {
        match self.focus {
            Focus::Editing => Some(self.in_editor(key)),
            Focus::Picking => Some(self.in_picker(key)),
            Focus::Naming => Some(self.in_namer(key)),
            Focus::List => None,
        }
    }

    /// Keys on a page of rows: the detail of one agent, or the repository's options.
    fn in_rows(&mut self, key: Key) -> Action {
        if let Some(action) = self.in_open_pane(key) {
            return action;
        }
        if let Some(action) = self.shared_key(key) {
            return action;
        }
        self.in_list(key)
    }

    /// The keys every page of the screen answers the same way.
    ///
    /// `None` means this key was not one of them and the page should read it.
    fn shared_key(&mut self, key: Key) -> Option<Action> {
        Some(match key {
            Key::Char('s') => self.ask_to_save(),
            Key::Char('q') => self.leave(),
            Key::Char('?') => {
                self.open(Page::Help);
                Action::None
            }
            Key::Esc => self.back(),
            Key::Backspace if self.page != Page::Home => self.back(),
            _ => return None,
        })
    }

    /// One level up; from the top, out.
    fn back(&mut self) -> Action {
        match self.page {
            Page::Home => return self.leave(),
            Page::Detail if self.pinned() => return self.leave(),
            Page::Detail => self.open(Page::Agents),
            Page::Agents | Page::Options | Page::Doctor | Page::Help => self.open(Page::Home),
        }
        Action::None
    }

    /// Laying what a save would write over the screen, or saying why there is none.
    fn ask_to_save(&mut self) -> Action {
        // Nothing ticked is not a plan.
        if self.chosen().is_empty() {
            self.open(Page::Agents);
            self.message = Some(
                t!(
                    self.tongue,
                    "choose at least one agent — space ticks the one under the cursor"
                )
                .to_owned(),
            );
            return Action::None;
        }
        if self.review().is_empty() {
            self.message =
                Some(t!(self.tongue, "nothing changed — there is nothing to save").to_owned());
            return Action::None;
        }
        self.reviewing = true;
        Action::None
    }

    /// What a save asks for, read off the screen.
    pub fn plan(&self) -> Plan {
        Plan {
            agents: self.chosen(),
            opened: self.installed.clone(),
            rows: self.configs.clone(),
            repository: std::path::PathBuf::from(&self.repository),
        }
    }

    /// What a save would write: the agents it installs, and the settings it moves in each.
    pub fn review(&self) -> Review {
        let plan = self.plan();
        let touched = plan.to_touch(|adapter| self.installed_agents.contains(&adapter.slug));
        Review {
            installs: touched
                .iter()
                .copied()
                .filter(|adapter| !self.installed_agents.contains(&adapter.slug))
                .collect(),
            changes: touched
                .into_iter()
                .map(|adapter| (adapter, plan.changed_settings(adapter.slug)))
                .filter(|(_, settings)| !settings.is_empty())
                .collect(),
        }
    }

    /// Keys while the review is laid over the screen.
    fn in_review(&mut self, key: Key) -> Action {
        match key {
            Key::Enter => {
                self.reviewing = false;
                Action::Save
            }
            Key::Esc | Key::Backspace => {
                self.reviewing = false;
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Says once that there is something to lose, and whether it said it.
    fn warn_unsaved(&mut self) -> bool {
        if !self.dirty() || self.armed {
            return false;
        }
        self.message = Some(self.unsaved_warning().to_owned());
        self.warned = true;
        true
    }

    /// Quitting, warning once if there is anything to lose.
    fn leave(&mut self) -> Action {
        if self.warn_unsaved() {
            Action::None
        } else {
            Action::Quit
        }
    }

    /// Keys while choosing agents.
    fn in_agents(&mut self, key: Key) -> Action {
        if let Some(action) = self.shared_key(key) {
            return action;
        }
        match key {
            Key::Up | Key::Char('k') => self.walk_agents(-1),
            Key::Down | Key::Char('j') => self.walk_agents(1),
            Key::Char(' ') if !self.everyone => {
                if let Some((_, on)) = self.agents.get_mut(self.agent) {
                    *on = !*on;
                }
                self.message = None;
            }
            Key::Enter => self.open_detail(),
            Key::Char('u') if self.can_uninstall() => {
                return Action::PreviewUninstall(Removal::Agent(self.current()));
            }
            _ => {}
        }
        Action::None
    }

    /// Whether the cursor is on an agent that is installed, which is the only one `u` is for.
    pub fn can_uninstall(&self) -> bool {
        !self.everyone && self.installed_agents.contains(&self.current())
    }

    /// Lays what removing this would take over the screen.
    pub fn removal_asked(&mut self, target: Removal, lines: Vec<String>) {
        self.message = None;
        self.removing = Some((target, lines));
    }

    /// Takes an agent's state from a read-back, and from nothing else.
    pub fn agent_read_back(&mut self, slug: &'static str, configured: bool) {
        self.installed_agents.retain(|installed| *installed != slug);
        if configured {
            self.installed_agents.push(slug);
        } else if let Some((_, on)) = self.agents.iter_mut().find(|(a, _)| a.slug == slug) {
            *on = false;
        }
    }

    /// Keys while a removal is laid over the screen.
    fn in_removal(&mut self, key: Key) -> Action {
        match key {
            Key::Enter => match self.removing.take() {
                Some((target, _)) => Action::Uninstall(target),
                None => Action::None,
            },
            Key::Esc | Key::Backspace => {
                self.removing = None;
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Moves down the list: **all installed agents** first, then each adapter.
    fn walk_agents(&mut self, delta: isize) {
        let stops = self.agents.len() as isize + 1;
        let here = if self.everyone {
            0
        } else {
            self.agent as isize + 1
        };
        let there = (here + delta).rem_euclid(stops);
        self.everyone = there == 0;
        if there > 0 {
            self.agent = (there - 1) as usize;
        }
        self.message = None;
    }

    /// Opening what the list is on, unless it would be answered into a table this run will not write.
    fn open_detail(&mut self) {
        if self.everyone && self.chosen().is_empty() {
            self.message = Some(
                t!(
                    self.tongue,
                    "choose at least one agent — space ticks the one under the cursor"
                )
                .to_owned(),
            );
        } else if !self.everyone && !self.agents[self.agent].1 {
            self.message = Some(t!(self.tongue, "not chosen — space ticks it").to_owned());
        } else {
            self.open(Page::Detail);
        }
    }

    /// The answers the picker is offering for the row under the cursor.
    pub fn picker(&self) -> Vec<String> {
        if self.model_profile_at_cursor() {
            let mut profiles = self
                .model_adapter()
                .into_iter()
                .flat_map(AgentAdapter::model_profiles)
                .map(|profile| profile.name.to_owned())
                .collect::<Vec<_>>();
            profiles.push(t!(self.tongue, "custom").to_owned());
            return profiles;
        }
        if let Some(target) = self.model_target_at_cursor() {
            return self
                .model_entries(target.name)
                .iter()
                .map(|entry| self.model_entry_label(entry))
                .collect();
        }
        if self.stage_list_at_cursor() {
            let mut entries: Vec<String> = self
                .stages()
                .iter()
                .map(|stage| crate::config::render_stages(std::slice::from_ref(stage)))
                .collect();
            entries.push(t!(self.tongue, "+ add a stage").to_owned());
            return entries;
        }
        self.picker_entries()
    }

    /// Whether the row under the cursor is edited as a list of stages.
    pub fn stage_list_at_cursor(&self) -> bool {
        self.setting_at_cursor() == Some(Setting::ExternalReviewers)
    }

    fn stages(&self) -> Vec<crate::config::ReviewStage> {
        self.shown_config(Setting::ExternalReviewers)
            .external_reviewers
            .clone()
    }

    fn write_stages(&mut self, stages: &[crate::config::ReviewStage]) {
        let value = crate::config::render_stages(stages);
        match self.set(Setting::ExternalReviewers, &value) {
            Ok(()) => self.message = None,
            Err(refusal) => self.message = Some(format!("{refusal}")),
        }
    }

    fn in_stage_list(&mut self, key: Key) -> Action {
        let mut stages = self.stages();
        let count = stages.len() + 1;
        let at = self.pick.min(count - 1);
        match key {
            Key::Up | Key::Char('k') => self.pick = (at + count - 1) % count,
            Key::Down | Key::Char('j') => self.pick = (at + 1) % count,
            Key::Esc | Key::Char('q') => self.focus = Focus::List,
            Key::Enter => {
                self.draft = stages
                    .get(at)
                    .map(|stage| crate::config::render_stages(std::slice::from_ref(stage)))
                    .unwrap_or_default();
                self.stage_edit = Some(at);
                self.focus = Focus::Editing;
            }
            Key::Char(' ') if at < stages.len() => {
                stages[at].blocking = !stages[at].blocking;
                self.write_stages(&stages);
            }
            Key::Char('d') | Key::Backspace if at < stages.len() => {
                stages.remove(at);
                self.write_stages(&stages);
                self.pick = at.min(stages.len());
            }
            Key::Char('K') if at > 0 && at < stages.len() => {
                stages.swap(at, at - 1);
                self.write_stages(&stages);
                self.pick = at - 1;
            }
            Key::Char('J') if at + 1 < stages.len() => {
                stages.swap(at, at + 1);
                self.write_stages(&stages);
                self.pick = at + 1;
            }
            _ => {}
        }
        Action::None
    }

    fn in_stage_editor(&mut self, key: Key, at: usize) -> Action {
        match key {
            Key::Char(character) => self.draft.push(character),
            Key::Backspace => {
                self.draft.pop();
            }
            Key::Esc => {
                self.stage_edit = None;
                self.focus = Focus::Picking;
                self.draft.clear();
                self.message = None;
            }
            Key::Enter => {
                let parsed = crate::config::parse_stages(&self.draft)
                    .filter(|parsed| parsed.len() == 1)
                    .and_then(|mut parsed| parsed.pop());
                let Some(stage) = parsed else {
                    self.message = Some(
                        t!(
                            self.tongue,
                            "one stage: check:<check name>, check:<check name>@<login>, review:<login>, review:<org>/<team> or review:any, optionally ending in :advisory"
                        )
                        .to_owned(),
                    );
                    return Action::None;
                };
                let mut stages = self.stages();
                if at < stages.len() {
                    stages[at] = stage;
                } else {
                    stages.push(stage);
                }
                self.write_stages(&stages);
                self.stage_edit = None;
                self.focus = Focus::Picking;
                self.draft.clear();
                self.pick = at;
            }
            _ => {}
        }
        Action::None
    }

    /// Feeds a host-derived advisory model list back into the pure state.
    pub fn model_catalog_loaded(
        &mut self,
        adapter: &'static AgentAdapter,
        mut models: Vec<String>,
    ) {
        models.retain(|model| ModelRouting::accepts_model_id(model));
        models.sort();
        models.dedup();
        self.catalog_settled();
        self.model_catalogs.insert(adapter.slug, models);
        self.catalogs_failed.remove(adapter.slug);
        self.align_model_pick();
    }

    fn catalog_settled(&mut self) {
        if matches!(self.busy.take(), Some(Busy::ModelCatalog(_))) {
            self.message = None;
        }
    }

    /// Records that a host's catalog could not be read, without caching it as empty.
    pub fn model_catalog_failed(&mut self, adapter: &'static AgentAdapter) {
        self.catalog_settled();
        self.catalogs_failed.insert(adapter.slug);
        self.align_model_pick();
    }

    /// The title of the open picker.
    pub fn picker_title(&self) -> String {
        if self.model_profile_at_cursor() {
            return t!(self.tongue, "Model profile").to_owned();
        }
        match self.model_target_at_cursor() {
            Some(target) => fill!(self.tongue, "Models for {target}", "target" => target.name),
            None => self
                .setting_at_cursor()
                .map(|setting| self.tongue.say(setting.label()).to_owned())
                .or_else(|| {
                    self.screen_at_cursor()
                        .map(|screen| screen.label(self.tongue).to_owned())
                })
                .unwrap_or_default(),
        }
    }

    /// Truthful provenance text for an open model picker.
    pub fn model_picker_note(&self) -> Option<String> {
        self.model_target_at_cursor()?;
        let Some(adapter) = self.model_adapter() else {
            return Some(
                t!(
                    self.tongue,
                    "shared answers have no single agent model catalog; type a model ID"
                )
                .to_owned(),
            );
        };
        Some(match adapter.model_catalog() {
            ModelCatalogSource::Curated(_) => fill!(
                self.tongue,
                "{agent} model suggestions are advisory; Estigia neither validates nor runs models",
                "agent" => adapter.display_name
            ),
            ModelCatalogSource::OpenCode
                if self
                    .model_catalogs
                    .get(adapter.slug)
                    .is_some_and(|models| !models.is_empty()) =>
            {
                t!(
                    self.tongue,
                    "loaded from `opencode models` without refresh; advisory only"
                )
                .to_owned()
            }
            ModelCatalogSource::OpenCode
                if self.model_catalogs.contains_key(adapter.slug)
                    || self.catalogs_failed.contains(adapter.slug) =>
            {
                t!(
                    self.tongue,
                    "OpenCode model catalog unavailable or empty; type a model ID"
                )
                .to_owned()
            }
            ModelCatalogSource::OpenCode => {
                t!(self.tongue, "loading OpenCode's model catalog…").to_owned()
            }
            ModelCatalogSource::None => fill!(
                self.tongue,
                "no verified model catalog for {agent}; type a model ID",
                "agent" => adapter.display_name
            ),
        })
    }

    /// The advisory for the model one target currently names, when another host owns it.
    pub fn model_foreign_note(&self, target: &str) -> Option<String> {
        let model = self.model_current(target)?;
        self.foreign_model_note(&model)
    }

    fn foreign_model_note(&self, model: &str) -> Option<String> {
        let mut owners: Vec<&'static str> = Vec::new();
        let mut strangers: Vec<&'static str> = Vec::new();
        for slug in self.model_destinations() {
            let Some(adapter) = AGENTS.iter().find(|adapter| adapter.slug == slug) else {
                continue;
            };
            let hosts = adapter.foreign_model_hosts(model);
            if hosts.is_empty() {
                continue;
            }
            if !strangers.contains(&adapter.display_name) {
                strangers.push(adapter.display_name);
            }
            for host in hosts {
                if !owners.contains(&host) {
                    owners.push(host);
                }
            }
        }
        (!strangers.is_empty()).then(|| {
            fill!(
                self.tongue,
                "{model} is a {hosts} model; {agents} does not list it, and it is stored as given",
                "model" => model,
                "hosts" => owners.join(", "),
                "agents" => strangers.join(", ")
            )
        })
    }

    fn close_model_picker_advising(&mut self, model: &str) {
        self.close_model_picker();
        self.message = self.foreign_model_note(model);
    }

    fn model_adapter(&self) -> Option<&'static AgentAdapter> {
        if self.everyone {
            return None;
        }
        match self.purpose {
            Purpose::OneTable(adapter) => adapter,
            Purpose::Setup => Some(self.adapter()),
        }
    }

    /// The whole route on this row, when every destination agrees.
    fn model_route(&self, target: &str) -> Option<crate::config::Route> {
        match self.model_assignment(target) {
            ModelAssignment::Shared(route) => route,
            ModelAssignment::Divergent => None,
        }
    }

    /// Its model half, which is what the catalog and the cursor are about.
    fn model_current(&self, target: &str) -> Option<String> {
        self.model_route(target).and_then(|route| route.model)
    }

    /// Its effort half, when one is named.
    fn model_effort(&self, target: &str) -> Option<crate::config::Effort> {
        self.model_route(target).and_then(|route| route.effort)
    }

    fn model_suggestions(&self, target: &str) -> Vec<String> {
        let mut suggestions = match self.model_adapter().map(AgentAdapter::model_catalog) {
            Some(ModelCatalogSource::Curated(models)) => {
                models.iter().map(|model| (*model).to_owned()).collect()
            }
            Some(ModelCatalogSource::OpenCode) => self
                .model_adapter()
                .and_then(|adapter| self.model_catalogs.get(adapter.slug))
                .cloned()
                .unwrap_or_default(),
            Some(ModelCatalogSource::None) | None => Vec::new(),
        };
        if let Some(current) = self.model_current(target)
            && !suggestions.iter().any(|model| model == &current)
        {
            suggestions.insert(0, current);
        }
        suggestions
    }

    fn model_entries(&self, target: &str) -> Vec<ModelEntry> {
        self.model_suggestions(target)
            .into_iter()
            .map(ModelEntry::Model)
            .chain([ModelEntry::TypeModel, ModelEntry::Inherit])
            .collect()
    }

    fn model_entry_label(&self, entry: &ModelEntry) -> String {
        match entry {
            ModelEntry::Model(model) => model.clone(),
            ModelEntry::TypeModel => t!(self.tongue, "type a model ID…").to_owned(),
            ModelEntry::Inherit => t!(self.tongue, "inherit").to_owned(),
        }
    }

    fn align_model_pick(&mut self) {
        let Some(target) = self.model_target_at_cursor() else {
            return;
        };
        let current = self.model_current(target.name);
        self.pick = current
            .and_then(|current| {
                self.model_entries(target.name).iter().position(
                    |entry| matches!(entry, ModelEntry::Model(model) if *model == current),
                )
            })
            .unwrap_or(0);
    }

    /// Where the folder listing should start, for the row under the cursor.
    pub fn folder_root(&self) -> std::path::PathBuf {
        if let Some(browsing) = &self.browsing {
            return browsing.clone();
        }
        let here = self
            .setting_at_cursor()
            .filter(|setting| setting.takes_a_directory())
            .map(|setting| self.shown_value(setting))
            .unwrap_or_default();
        let path = std::path::PathBuf::from(&here);
        if here.is_empty() || here == "unset" || !path.is_dir() {
            return crate::paths::home_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        }
        path
    }

    /// The list itself, once the caller has been told what it is.
    fn picker_entries(&self) -> Vec<String> {
        // A preference offers exactly the languages this screen has words for.
        if let Some(screen) = self.screen_at_cursor() {
            if screen == Screen::Repository {
                return self.repositories.clone();
            }
            return screen.answers();
        }
        let Some(setting) = self.setting_at_cursor() else {
            return Vec::new();
        };
        let answers = setting.answers();
        let mut entries = self.offered(setting);
        // A value somebody typed goes on the list too.
        let current = self.shown_value(setting);
        if !entries.contains(&current) && !current.is_empty() {
            entries.insert(0, current);
        }
        // The folders under whatever it is set to, so a path is walked rather than typed.
        if setting.takes_a_directory() {
            for folder in self.folders.iter().rev() {
                entries.insert(0, folder.clone());
            }
            // The drives last, so a walk that has to leave this one has somewhere to go.
            for drive in &self.drives {
                if !entries.iter().any(|entry| entry == drive) {
                    entries.push(drive.clone());
                }
            }
        }
        if !answers.closed {
            entries.push(t!(self.tongue, TYPE_IT).to_owned());
        }
        entries
    }

    /// Whether the shell should look at the filesystem before this frame.
    pub fn wants_folders(&self) -> bool {
        self.setting_at_cursor()
            .is_some_and(Setting::takes_a_directory)
    }

    /// Whether the open picker is a folder walk rather than a list of answers.
    fn browsing_a_folder(&self) -> bool {
        // The same question as `wants_folders`, written once.
        self.wants_folders()
    }

    /// Whether a picker entry is a folder the walk can open, as the shell listed it.
    fn is_a_folder(&self, entry: &str) -> bool {
        self.folders
            .iter()
            .chain(&self.drives)
            .any(|known| known == entry)
    }

    /// Steps the row under the cursor to its next or previous answer.
    fn cycle(&mut self, delta: isize) -> Action {
        if let Some(target) = self.model_target_at_cursor() {
            return self.cycle_effort(target.name, delta);
        }
        if let Some(screen) = self.screen_at_cursor() {
            let answers = screen.answers();
            if answers.is_empty() {
                return Action::None;
            }
            let here = self.screen_value(screen);
            let count = answers.len() as isize;
            let at = answers
                .iter()
                .position(|answer| *answer == here)
                .map_or(0, |at| at as isize);
            let next = answers[(at + delta).rem_euclid(count) as usize].clone();
            return self.choose_screen(screen, &next);
        }
        let Some(setting) = self.setting_at_cursor() else {
            return Action::None;
        };
        let current = self.shown_value(setting);
        let Some(next) = self.step_of(setting, &current, delta) else {
            return Action::None;
        };
        match self.set(setting, &next) {
            Ok(()) => self.message = None,
            Err(refusal) => self.message = Some(format!("{refusal}")),
        }
        Action::None
    }

    fn cycle_effort(&mut self, target: &'static str, delta: isize) -> Action {
        let ring: Vec<Option<crate::config::Effort>> = std::iter::once(None)
            .chain(crate::config::Effort::all().into_iter().map(Some))
            .collect();
        let at = ring
            .iter()
            .position(|effort| *effort == self.model_effort(target))
            .unwrap_or(0) as isize;
        let next = ring[(at + delta).rem_euclid(ring.len() as isize) as usize];
        match self.update_model_route(|routing| routing.assign_effort(target, next)) {
            Ok(_) => self.message = None,
            Err(refusal) => self.message = Some(format!("{refusal}")),
        }
        Action::None
    }

    /// What a preference of this screen is set to.
    pub fn screen_value(&self, screen: Screen) -> String {
        match screen {
            Screen::Language => self.tongue.name().to_owned(),
            Screen::Repository => self.repository.clone(),
        }
    }

    /// Takes an answer to a preference of this screen.
    ///
    pub(crate) fn choose_screen(&mut self, screen: Screen, value: &str) -> Action {
        match screen {
            // Nothing is written here: the answers come back through the shell.
            Screen::Repository => {
                self.message = None;
                Action::Reload(value.to_owned())
            }
            Screen::Language => match Tongue::from_name(value) {
                Some(tongue) => {
                    self.tongue = tongue;
                    self.message = None;
                    Action::Remember
                }
                // Unreachable through the picker, and silently doing nothing is the last resort.
                None => Action::None,
            },
        }
    }

    /// Keys while moving through a list of settings.
    fn in_list(&mut self, key: Key) -> Action {
        match key {
            Key::Up | Key::Char('k') => self.move_by(-1),
            Key::Down | Key::Char('j') => self.move_by(1),
            // The answer changes under the cursor, with no field to open.
            Key::Left | Key::Char('h') => return self.cycle(-1),
            Key::Right | Key::Char('l') => return self.cycle(1),
            // Enter and space act on the row under the cursor.
            Key::Enter | Key::Char(' ') => {
                // A preference opens the same list, on what it is set to.
                if let Some(screen) = self.screen_at_cursor() {
                    let here = self.screen_value(screen);
                    self.pick = self
                        .picker()
                        .iter()
                        .position(|entry| *entry == here)
                        .unwrap_or(0);
                    self.focus = Focus::Picking;
                    self.message = None;
                    return Action::None;
                }
                if self.model_profile_at_cursor() {
                    self.pick = self
                        .picker()
                        .iter()
                        .position(|entry| *entry == self.model_profile_value())
                        .unwrap_or(0);
                    self.focus = Focus::Picking;
                    self.message = None;
                    return Action::None;
                }
                if self.model_target_at_cursor().is_some() {
                    self.model_edit = None;
                    self.focus = Focus::Picking;
                    self.message = None;
                    self.align_model_pick();
                    if let Some(adapter) = self.model_adapter()
                        && adapter.model_catalog() == ModelCatalogSource::OpenCode
                        && !self.model_catalogs.contains_key(adapter.slug)
                    {
                        return Action::LoadModelCatalog(adapter);
                    }
                    return Action::None;
                }
                let Some(setting) = self.setting_at_cursor() else {
                    return Action::None;
                };
                let current = self.shown_value(setting);
                if self.is_text_field(setting) {
                    self.draft = if setting.answers().at(&current).is_some()
                        && setting.means(&current).is_none()
                    {
                        String::new()
                    } else {
                        current
                    };
                    self.focus = Focus::Editing;
                    self.message = None;
                    return Action::None;
                }
                // Opened on what is set, so the chosen answer is under the cursor.
                self.pick = self
                    .picker()
                    .iter()
                    .position(|entry| *entry == current)
                    .unwrap_or(0);
                self.focus = Focus::Picking;
                self.message = None;
            }
            Key::Char('r') => {
                // A preference has nothing to restore *to*.
                if let Some(screen) = self.screen_at_cursor() {
                    self.message = Some(fill!(
                        self.tongue,
                        "{label} is already remembered — there is nothing unsaved to restore",
                        "label" => screen.label(self.tongue)
                    ));
                    return Action::None;
                }
                if self.model_profile_at_cursor() {
                    let installed = self.installed.clone();
                    match self.apply_to_model_destinations(|slug, config| {
                        Setting::Models.apply(config, &installed[slug].models.as_value())?;
                        Ok(true)
                    }) {
                        Ok(true) => {
                            self.message = Some(
                                t!(self.tongue, "Model profile restored to what is installed")
                                    .to_owned(),
                            );
                        }
                        Ok(false) => {}
                        Err(refusal) => self.message = Some(format!("{refusal}")),
                    }
                    return Action::None;
                }
                if let Some(target) = self.model_target_at_cursor() {
                    match self.restore_model_target(target.name) {
                        Ok(true) => {
                            self.message = Some(fill!(
                                self.tongue,
                                "{target} restored to what is installed",
                                "target" => target.name
                            ));
                        }
                        Ok(false) => {}
                        Err(refusal) => self.message = Some(format!("{refusal}")),
                    }
                    return Action::None;
                }
                // Undo this row, not the screen.
                let Some(setting) = self.setting_at_cursor() else {
                    return Action::None;
                };
                // The same table the row is *shown* from.
                let from = self.shown_from(setting);
                let installed = setting.value_of(&self.installed[from]);
                self.message = Some(match self.set(setting, &installed) {
                    Ok(()) => fill!(
                        self.tongue,
                        "{label} restored to what is installed",
                        "label" => setting.label()
                    ),
                    Err(refusal) => format!("{refusal}"),
                });
            }
            _ => {}
        }
        Action::None
    }

    fn update_model_route(
        &mut self,
        update: impl Fn(&mut ModelRouting) -> bool,
    ) -> Result<bool, Refusal> {
        self.apply_to_model_destinations(|_, config| {
            let mut routing = config.models.clone();
            if !update(&mut routing) {
                return Ok(false);
            }
            Setting::Models.apply(config, &routing.as_value())?;
            Ok(true)
        })
    }

    fn replace_model_route(&mut self, value: &str) -> Result<bool, Refusal> {
        self.apply_to_model_destinations(|_, config| {
            Setting::Models.apply(config, value)?;
            Ok(true)
        })
    }

    fn restore_model_target(&mut self, target: &'static str) -> Result<bool, Refusal> {
        let installed = self.installed.clone();
        self.apply_to_model_destinations(|slug, config| {
            let mut routing = config.models.clone();
            match installed[slug].models.route(target).cloned() {
                Some(route) => {
                    let _ = routing.remove(target);
                    if let Some(model) = route.model.as_deref() {
                        let _ = routing.assign(target, model);
                    }
                    let _ = routing.assign_effort(target, route.effort);
                }
                None => {
                    let _ = routing.remove(target);
                }
            }
            Setting::Models.apply(config, &routing.as_value())?;
            Ok(true)
        })
    }

    fn apply_to_model_destinations(
        &mut self,
        apply: impl Fn(&'static str, &mut Config) -> Result<bool, Refusal>,
    ) -> Result<bool, Refusal> {
        let destinations = self.model_destinations();
        if destinations.is_empty() {
            return Ok(false);
        }

        // Every route is validated in a staged map before any live table moves.
        let mut staged = self.configs.clone();
        for slug in destinations {
            let Some(config) = staged.get_mut(slug) else {
                return Ok(false);
            };
            if !apply(slug, config)? {
                return Ok(false);
            }
        }
        self.configs = staged;
        Ok(true)
    }

    fn close_model_picker(&mut self) {
        self.model_edit = None;
        self.focus = Focus::List;
        self.draft.clear();
        self.message = None;
    }

    fn in_model_picker(&mut self, target: &'static str, key: Key) -> Action {
        let entries = self.model_entries(target);
        if entries.is_empty() {
            self.close_model_picker();
            return Action::None;
        }
        let count = entries.len();
        match key {
            Key::Up | Key::Char('k') => self.pick = (self.pick + count - 1) % count,
            Key::Down | Key::Char('j') => self.pick = (self.pick + 1) % count,
            Key::Esc | Key::Char('q') => self.close_model_picker(),
            Key::Enter | Key::Char(' ') => {
                let chosen = entries[self.pick.min(count - 1)].clone();
                match chosen {
                    ModelEntry::Model(model) => {
                        // The effort is a separate list, so choosing a model
                        // leaves whatever effort is already named alone.
                        match self.update_model_route(|routing| routing.assign(target, &model)) {
                            Ok(true) => {
                                self.close_model_picker_advising(&model);
                            }
                            Ok(false) => {
                                self.message = Some(
                                    t!(
                                        self.tongue,
                                        "a model ID must fit one key=model entry: no comma, semicolon, pipe, or line break"
                                    )
                                    .to_owned(),
                                );
                            }
                            Err(refusal) => self.message = Some(format!("{refusal}")),
                        }
                    }
                    ModelEntry::TypeModel => {
                        // The model value, so an effort already set stays where it is.
                        self.draft = self.model_current(target).unwrap_or_default();
                        self.model_edit = Some(target);
                        self.focus = Focus::Editing;
                        self.message = None;
                    }
                    ModelEntry::Inherit => {
                        match self.update_model_route(|routing| {
                            let _ = routing.remove(target);
                            true
                        }) {
                            Ok(true) => {
                                self.close_model_picker();
                            }
                            Ok(false) => {}
                            Err(refusal) => self.message = Some(format!("{refusal}")),
                        }
                    }
                }
            }
            _ => {}
        }
        Action::None
    }

    /// Keys while the answers are showing.
    fn in_picker(&mut self, key: Key) -> Action {
        if self.model_profile_at_cursor() {
            let entries = self.picker();
            let count = entries.len();
            match key {
                Key::Up | Key::Char('k') => self.pick = (self.pick + count - 1) % count,
                Key::Down | Key::Char('j') => self.pick = (self.pick + 1) % count,
                Key::Esc | Key::Char('q') => self.focus = Focus::List,
                Key::Enter | Key::Char(' ') => {
                    let chosen = &entries[self.pick.min(count - 1)];
                    if chosen != t!(self.tongue, "custom") {
                        let routing = self
                            .model_adapter()
                            .and_then(|adapter| {
                                adapter
                                    .model_profiles()
                                    .iter()
                                    .find(|profile| profile.name == chosen)
                            })
                            .and_then(|profile| profile.routing());
                        if let Some(routing) = routing {
                            match self.replace_model_route(&routing.as_value()) {
                                Ok(true) => self.message = None,
                                Ok(false) => {}
                                Err(refusal) => self.message = Some(format!("{refusal}")),
                            }
                        }
                    }
                    self.focus = Focus::List;
                }
                _ => {}
            }
            return Action::None;
        }
        if let Some(target) = self.model_target_at_cursor() {
            return self.in_model_picker(target.name, key);
        }
        if self.stage_list_at_cursor() {
            return self.in_stage_list(key);
        }
        let entries = self.picker();
        if entries.is_empty() {
            self.focus = Focus::List;
            return Action::None;
        }
        let count = entries.len();
        match key {
            Key::Up | Key::Char('k') => self.pick = (self.pick + count - 1) % count,
            Key::Down | Key::Char('j') => self.pick = (self.pick + 1) % count,
            // A list that is open closes rather than quitting the program.
            Key::Esc | Key::Char('q') => {
                self.focus = Focus::List;
                self.browsing = None;
                self.message = None;
            }
            // Into a folder, without answering the row.
            Key::Right | Key::Char('l')
                if self.browsing_a_folder()
                    && self.is_a_folder(&entries[self.pick.min(count - 1)]) =>
            {
                let into = entries[self.pick.min(count - 1)].clone();
                self.browsing = Some(std::path::PathBuf::from(into));
                self.pick = 0;
                self.message = None;
            }
            // And back out. The parent is offered as an entry too.
            Key::Left | Key::Char('h') if self.browsing_a_folder() => {
                let here = self.folder_root();
                if let Some(parent) = here.parent() {
                    self.browsing = Some(parent.to_path_buf());
                    self.pick = 0;
                    self.message = None;
                }
            }
            Key::Char('n') if self.browsing_a_folder() => {
                self.draft.clear();
                self.focus = Focus::Naming;
                self.message = None;
            }
            Key::Enter | Key::Char(' ') => {
                let chosen = entries[self.pick.min(count - 1)].clone();
                if let Some(screen) = self.screen_at_cursor() {
                    self.focus = Focus::List;
                    return self.choose_screen(screen, &chosen);
                }
                let Some(setting) = self.setting_at_cursor() else {
                    self.focus = Focus::List;
                    return Action::None;
                };
                // Against the *translated* entry, because that is what the list is showing.
                if chosen == t!(self.tongue, TYPE_IT) {
                    // The field opens on the current value rather than empty.
                    let current = self.shown_value(setting);
                    self.draft = if setting.answers().at(&current).is_some()
                        && setting.means(&current).is_none()
                    {
                        String::new()
                    } else {
                        current
                    };
                    self.focus = Focus::Editing;
                    return Action::None;
                }
                match self.set(setting, &chosen) {
                    Ok(()) => {
                        self.focus = Focus::List;
                        // The walk ends with the answer.
                        self.browsing = None;
                        self.message = None;
                    }
                    Err(refusal) => self.message = Some(format!("{refusal}")),
                }
            }
            _ => {}
        }
        Action::None
    }

    /// Keys while naming a folder to make.
    fn in_namer(&mut self, key: Key) -> Action {
        match key {
            Key::Char(character) => {
                self.draft.push(character);
                Action::None
            }
            Key::Backspace => {
                self.draft.pop();
                Action::None
            }
            Key::Esc => {
                self.draft.clear();
                self.focus = Focus::Picking;
                Action::None
            }
            Key::Enter => {
                let named = self.draft.trim().to_owned();
                self.draft.clear();
                self.focus = Focus::Picking;
                if named.is_empty() {
                    return Action::None;
                }
                // Refused here rather than by the filesystem: the message is about the name typed.
                if named.contains(['/', '\\']) || named == "." || named == ".." {
                    self.message = Some(
                        t!(
                            self.tongue,
                            "a folder name, not a path: no separators, and not `.` or `..`"
                        )
                        .to_owned(),
                    );
                    return Action::None;
                }
                Action::MakeFolder(self.folder_root().join(named))
            }
            _ => Action::None,
        }
    }

    fn in_model_editor(&mut self, key: Key, target: &'static str) -> Action {
        match key {
            Key::Char(character) => self.draft.push(character),
            Key::Backspace => {
                self.draft.pop();
            }
            Key::Esc => {
                self.model_edit = None;
                self.focus = Focus::Picking;
                self.draft.clear();
                self.message = None;
            }
            Key::Enter => {
                let value = self.draft.trim().to_owned();
                // The route's own parser, not the model half's.
                if crate::config::Route::parse(&value).is_none() {
                    self.message = Some(
                        t!(
                            self.tongue,
                            "a model ID must fit one key=model entry: no comma, semicolon, pipe, or line break"
                        )
                        .to_owned(),
                    );
                    return Action::None;
                }
                match self.update_model_route(|routing| routing.assign(target, &value)) {
                    Ok(true) => {
                        self.close_model_picker_advising(&value);
                    }
                    Ok(false) => {
                        self.message = Some(
                            t!(
                                self.tongue,
                                "a model ID must fit one key=model entry: no comma, semicolon, pipe, or line break"
                            )
                            .to_owned(),
                        );
                    }
                    Err(refusal) => self.message = Some(format!("{refusal}")),
                }
            }
            _ => {}
        }
        Action::None
    }

    /// Keys while typing a value.
    fn in_editor(&mut self, key: Key) -> Action {
        if let Some(edit) = self.model_edit {
            return self.in_model_editor(key, edit);
        }
        if let Some(at) = self.stage_edit {
            return self.in_stage_editor(key, at);
        }
        match key {
            Key::Char(character) => self.draft.push(character),
            Key::Backspace => {
                self.draft.pop();
            }
            Key::Esc => {
                // Out of the editor, and the value is left as it was.
                self.focus = Focus::List;
                self.draft.clear();
            }
            Key::Enter => {
                let Some(setting) = self.setting_at_cursor() else {
                    self.focus = Focus::List;
                    return Action::None;
                };
                let value = self.draft.trim().to_owned();
                match self.set(setting, &value) {
                    Ok(()) => {
                        self.focus = Focus::List;
                        self.draft.clear();
                        self.message = None;
                    }
                    // The refusal already says what the setting accepts; the editor stays open.
                    Err(refusal) => self.message = Some(format!("{refusal}")),
                }
            }
            _ => {}
        }
        Action::None
    }
}

/// The keys this screen understands, named rather than borrowed from the
/// terminal library so the state machine can be driven without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable character.
    Char(char),
    /// Up.
    Up,
    /// Down.
    Down,
    /// Left.
    Left,
    /// Right.
    Right,
    /// Enter.
    Enter,
    /// Escape.
    Esc,
    /// Backspace.
    Backspace,
    /// Ctrl-C, which is asked to leave rather than typed.
    Interrupt,
    /// Anything else, which this screen ignores.
    Other,
}

/// What the shell can read off the machine for the page.
pub trait World {
    /// The checkouts the Repository picker offers.
    fn repositories(&mut self) -> Vec<String>;
    /// The folders directly under one, for a row that takes a directory.
    fn folders(&mut self, root: &std::path::Path) -> Vec<String>;
    /// The roots a walk can reach that are not under the current one.
    fn drives(&mut self) -> Vec<String>;
}

/// What the shell has already read, so a key press asks the disk only when the answer can differ.
#[derive(Default)]
pub struct Reads {
    repositories: bool,
    folders_of: Option<std::path::PathBuf>,
    was_picking: bool,
}

impl Reads {
    /// Brings the page's read-only tables up to date ahead of a key.
    pub fn refresh(&mut self, app: &mut App, world: &mut impl World) {
        if app.wants_folders() {
            let root = app.folder_root();
            if self.folders_of.as_ref() != Some(&root) {
                app.folders = world.folders(&root);
                self.folders_of = Some(root);
            }
            if app.drives.is_empty() {
                app.drives = world.drives();
            }
        } else {
            self.folders_of = None;
        }
        let picking = app.focus == Focus::Picking;
        if self.was_picking && !picking {
            self.repositories = false;
        }
        self.was_picking = picking;
        if app.screen_at_cursor() == Some(Screen::Repository) {
            if !self.repositories {
                app.repositories = world.repositories();
                self.repositories = true;
            }
        } else {
            self.repositories = false;
        }
    }

    /// Forgets what the action just performed has made stale.
    pub fn performed(&mut self, action: &Action) {
        match action {
            Action::Reload(_) => self.forget_repositories(),
            Action::MakeFolder(_) => self.forget_folders(),
            _ => {}
        }
    }

    /// Makes the next [`Reads::refresh`] read the checkouts again.
    fn forget_repositories(&mut self) {
        self.repositories = false;
    }

    /// Makes the next [`Reads::refresh`] read the folders again.
    fn forget_folders(&mut self) {
        self.folders_of = None;
    }
}
