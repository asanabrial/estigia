//! The state a key press moves, with no terminal in it.
//!
//! Everything here is a plain value: a key goes in, the state moves, and an
//! [`Action`] comes out for the caller to perform. That seam is borrowed from
//! Leteo's TUI, and it is the only reason a TUI can be tested at all — a state
//! machine that reaches for the screen has to be driven through one.

use crate::config::{
    AGENT_SETTINGS, Config, ModelRouting, ModelTarget, OPTIONS_SETTINGS, Scope, Setting,
};
use crate::outcome::Refusal;
use crate::setup::{AGENTS, AgentAdapter, Applies, ModelCatalogSource};
use crate::tui::InstallReceipt;
use crate::tui::words::{TONGUES, Tongue, fill, t};

/// What the shell around the state machine has to do next.
///
/// Every one of these leaves the screen to touch the world. The state machine
/// names them and performs none of them, which is what keeps it testable.
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
    ///
    /// The change itself needs no world: the next frame is drawn in it. What
    /// needs one is *tomorrow* — and somebody whose screen quietly reverts,
    /// having been told nothing, has no way to find out why.
    Remember,
    /// Leave.
    Quit,
    /// Read the chosen repository's own answers and hand them back.
    ///
    /// The page shows one checkout's rows at a time and the operator may pick
    /// another, which is a question for the disk — so the screen names it and
    /// the shell performs it, the way it already performs the install.
    ///
    /// It carries the checkout **because the row has not moved yet**. The page
    /// names the checkout whose answers are on it, and until they have been
    /// read that is still the previous one. A page that renamed itself first
    /// and then failed to read would be showing one repository's answers under
    /// another's name, which is the state this tool exists to refuse.
    Reload(String),
    /// Make a directory the operator named, and list it.
    ///
    /// The same shape as [`Self::Reload`] and for the same reason: choosing
    /// where isolated checkouts go often means choosing a folder that does not
    /// exist yet, and a picker that can only walk into what is already there
    /// sends somebody out to a shell to make it and back. The screen names the
    /// path and the shell performs it, because this state machine has no
    /// filesystem in it — which is the only reason it can be driven by a test.
    MakeFolder(std::path::PathBuf),
    /// Ask the tracker which project boards this owner has.
    ///
    /// `Project board` is configured as `owner/number`, and the number is the
    /// one part of that pair nobody knows without opening a browser. The screen
    /// names the question and the shell asks the transport — the same shape as
    /// [`Self::Reload`], and for the same reason: no world in here.
    ListBoards,
    /// Load one host's advisory model catalog and feed it back into the App.
    ///
    /// Curated catalogs need no world and never produce this. OpenCode's list is
    /// host-owned and potentially slow, so it is requested only when one of
    /// OpenCode's derived model rows is first opened.
    LoadModelCatalog(&'static AgentAdapter),
}

/// A preference of this screen, on this machine.
///
/// Not a [`Setting`], and it must never become one. A setting is a row of an
/// agent's contract, and no agent reads which language a person's terminal is
/// in — so a row for this there would be one `config set` writes, `config list`
/// reads back, and no decision consults, which is the defect
/// [`Setting::Window`]'s note records.
///
/// The difference is visible on the page rather than only true in the types:
/// these sit in their own panel, are applied the moment the key is pressed, and
/// are never written by `s`. One list holding both would make the unsaved mark
/// mean two things and `s` write some of its rows and not others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// The language this screen speaks.
    Language,
    /// Which checkout's own answers this page is showing.
    ///
    /// A preference of this screen for the same reason the language is: it
    /// belongs to the person sitting here and to no agent's contract. What it
    /// decides is *which* repository the rows above answer for — the one this
    /// was run in, or one it has answered for before.
    ///
    /// Without it, managing a checkout's rows meant going to that checkout
    /// first. The registry already knew where they all were; nothing offered
    /// them.
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
            // Fed rather than known: which checkouts answer for themselves is a
            // question about the disk, and this type has none. `App::picker`
            // fills it from what the shell looked up.
            Self::Repository => Vec::new(),
        }
    }
}

/// Which screen is showing.
///
/// A menu first, and the work behind it — the shape Leteo's TUI has. A tool
/// whose first screen is one of its jobs teaches an operator that job is the
/// tool, and the rest goes unfound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The menu.
    Home,
    /// Agents and their own settings.
    Setup,
    /// The settings that are the same whichever agent asks.
    ///
    /// A page of its own rather than a step of setup, because it answers a
    /// different question. Setup is about **who is holding the tools** and is
    /// walked once per machine; these are facts about the repository — where
    /// the issues live, what history the base branch must end up with — and
    /// they are what somebody comes back to change. Buried on step three of a
    /// wizard, changing one meant answering "which agents?" first.
    Options,
    /// What the checks said.
    Doctor,
    /// The keys.
    Help,
}

/// A verdict on something that already happened, laid over the screen.
///
/// Deliberately **not** the log. Installing into eleven agents writes thirty-odd
/// lines, and a screen that hands those back has told an operator everything
/// except the one thing they asked: did it work. The lines still reach them —
/// the caller prints them on the way out, where they can be scrolled — and what
/// stays here is the answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modal {
    /// Whether the thing it reports on succeeded.
    pub ok: bool,
    /// The verdict, in a few words.
    pub title: String,
    /// What to do about it, when there is anything to do.
    pub detail: Option<String>,
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
    Doing(&'static str),
}

/// The menu, in the order somebody meets them.
///
/// Setup first because it is what a new install needs, and the guard second
/// because it is the one door no agent can go around — an operator who never
/// finds it has a gate with a hole in it and no way to know.
pub const MENU: &[Entry] = &[
    Entry {
        label: "Setup",
        about: "choose agents, and configure each one",
        goes: Goes::To(Page::Setup),
    },
    // Directly under setup, because it holds the other half of the same
    // question and the two are told apart by *what the answer is about*: setup
    // is who is holding the tools, and this is what is true of the repository
    // whichever of them asks.
    Entry {
        label: "Options",
        about: "the settings that are the same whichever agent asks",
        goes: Goes::To(Page::Options),
    },
    Entry {
        label: "Push guard",
        about: "the pre-push hook: the one boundary no agent can go around",
        goes: Goes::Doing("guard"),
    },
    Entry {
        label: "Doctor",
        about: "check that everything a run needs before it swears actually works",
        goes: Goes::Doing("doctor"),
    },
    Entry {
        label: "Help",
        about: "the keys, and what this screen is",
        goes: Goes::To(Page::Help),
    },
    // Listed even though `q` also works: somebody who arrived at a full-screen
    // program has no way to know that without being told.
    Entry {
        label: "Quit",
        about: "leave — nothing is written that was not already installed",
        goes: Goes::Doing("quit"),
    },
];

/// One stage of setting Estigia up.
///
/// Sixteen settings shown at once, once per agent, is a screen that answers
/// every question and asks nothing — an operator meeting it has to work out on
/// their own which rows are about *this agent* and which are about the
/// repository, and there was nothing on the screen that told them.
///
/// So the screen asks one question at a time, in the order the answers depend
/// on each other: who is holding the tools, then what each of them may do, then
/// what that adds up to.
///
/// The repository's own rows were a fourth step here and are [`Page::Options`]
/// now. They are not about the agents, they are the ones somebody comes back
/// to change, and reaching them meant walking past two questions that had
/// already been answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Which agents Estigia holds the tools for.
    Agents,
    /// Each chosen agent's own settings.
    PerAgent,
    /// What all of it adds up to, and the key that writes it.
    Install,
}

/// The steps, in order.
pub const STEPS: &[Step] = &[Step::Agents, Step::PerAgent, Step::Install];

/// The question the options page asks, in the shape a step asks one.
pub const OPTIONS_QUESTION: &str =
    "what is true of this repository, and of this machine, whichever agent asks?";

impl Step {
    /// What the stepper calls it.
    pub fn title(self) -> &'static str {
        match self {
            Self::Agents => "Agents",
            // Not "per agent" any more: the step answers for one agent *or*
            // for all of them, and a title naming half of what it does sends
            // somebody looking elsewhere for the other half.
            Self::PerAgent => "Configuration",
            Self::Install => "Install",
        }
    }

    /// The question this step asks, in one line.
    pub fn question(self) -> &'static str {
        match self {
            Self::Agents => "which agents should Estigia hold the tools for?",
            Self::PerAgent => "what may they do — all of them, or each on its own?",
            Self::Install => "this is what will be written",
        }
    }

    /// The settings this step edits, if it edits any.
    pub fn settings(self) -> &'static [Setting] {
        match self {
            Self::Agents | Self::Install => &[],
            Self::PerAgent => AGENT_SETTINGS,
        }
    }

    /// Its place in the stepper, counting from one.
    pub fn number(self) -> usize {
        STEPS.iter().position(|step| *step == self).unwrap_or(0) + 1
    }
}

/// What this screen was opened to do.
///
/// The steps are right for `estigia setup`, which is a decision about a machine:
/// which agents, what each may do, what the repository is. They are wrong for
/// `estigia config edit`, which was handed exactly one table on the command
/// line and has nothing to choose and nothing to install — walking somebody
/// through "which agents?" when the answer already arrived as an argument is a
/// question whose answer will be thrown away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The three questions, and the options page beside them.
    Setup,
    /// One table, every row of it, in table order.
    ///
    /// The adapter when the table is one adapter's own, and `None` for the
    /// shared contract — which every agent reads, so no single one of them can
    /// say what a row is worth.
    OneTable(Option<&'static AgentAdapter>),
}

/// The picker entry that opens somewhere to type.
///
/// A row whose answers are useful rather than exhaustive still has to reach the
/// ones nobody listed — an absolute path, a board name, a language.
pub const TYPE_IT: &str = "type a value…";

/// Per-agent settings in interactive order.
///
/// Persistence retains [`AGENT_SETTINGS`] in table order. The TUI moves
/// `Planning` to the end and projects its model targets beneath it, so the
/// dependent rows never precede the control that decides which phases exist.
const AGENT_TUI_SETTINGS: &[Setting] = &[
    Setting::Delivery,
    Setting::Review,
    Setting::Transitions,
    Setting::Judges,
    Setting::Planning,
];

/// A whole table in interactive order, without the synthetic model section.
const TUI_SETTINGS: &[Setting] = &[
    Setting::Delivery,
    Setting::Route,
    Setting::Review,
    Setting::Transitions,
    Setting::Merge,
    Setting::Worktree,
    Setting::Tracker,
    Setting::Integration,
    Setting::Window,
    Setting::ReviewProtocol,
    Setting::Judges,
    Setting::ChangeSize,
    Setting::Boundaries,
    Setting::Board,
    Setting::Summary,
    Setting::Body,
    Setting::Planning,
];

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
    /// Every destination has this exact assignment, including shared inherit.
    Shared(Option<String>),
    /// At least two selected destinations have different installed values.
    Divergent,
}

/// Which pane has the keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Who the answers are for: every agent, or one of them.
    Who,
    /// The rows themselves.
    Rows,
}

/// Where the keys go, within a step that has two panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Moving through the settings.
    List,
    /// Choosing from the answers a setting offers.
    ///
    /// Most rows have a handful of answers and no others. Making somebody type
    /// one of three words, exactly, into a field that refuses a typo is work
    /// the screen already knows how to do for them.
    Picking,
    /// Typing a new value into the selected one.
    Editing,
    /// Naming a folder to make, inside the one the picker is looking at.
    ///
    /// Its own focus rather than [`Self::Editing`] with a flag, because the two
    /// answer different questions and Enter does different things: one sets the
    /// row to what was typed, and this one asks the shell to make a directory
    /// and carries on walking. A flag on a shared state is how the same key
    /// comes to mean two things depending on something the screen does not show.
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
    /// Which stage of setup is showing.
    pub step: Step,
    /// Every adapter, and whether it is chosen.
    ///
    /// All of them are listed rather than only the installed ones: a setup
    /// screen that shows what you already have cannot answer the question
    /// people arrive with, which is *what else could I install this into*.
    pub agents: Vec<(&'static AgentAdapter, bool)>,
    /// Which adapter the cursor is on.
    pub agent: usize,
    /// The adapters that were already configured when the screen opened.
    ///
    /// A tick means two different things — *this is installed* and *install
    /// this* — and a screen that draws them identically cannot answer the
    /// question somebody reruns setup to ask, which is what they already have.
    pub installed_agents: Vec<&'static str>,
    /// Each adapter's configuration, as edited so far, by slug.
    ///
    /// One per adapter rather than one shared table, because that is the
    /// question this screen is actually asked: *what does **this** agent do*.
    /// A single table forced anybody who wanted two agents to differ to leave
    /// the screen and learn `config set`, which is the thing a setup screen
    /// exists not to make necessary.
    pub configs: std::collections::BTreeMap<&'static str, Config>,
    /// The same, as they were when the screen opened.
    pub installed: std::collections::BTreeMap<&'static str, Config>,
    /// The same again, with **no checkout's** rows on them.
    ///
    /// What each agent answers with where nothing has been said about the
    /// repository. It is the only base the chosen checkout's own rows can
    /// honestly be laid over: laying them over what is showing keeps the
    /// previous checkout's answer for every row this one is silent about.
    pub bare: std::collections::BTreeMap<&'static str, Config>,
    /// The checkout whose own answers this page is showing.
    ///
    /// Fed with the one this was run in, and changed by choosing another.
    pub repository: String,
    /// The checkouts that answer for themselves, as the shell last read them.
    pub repositories: Vec<String>,
    /// Where the folder picker is looking, while it is open.
    ///
    /// Independent of what the row is **set to**, and that is the whole of it:
    /// walking used to mean choosing — every step down set the value and closed
    /// the list, so reaching a folder three deep wrote three answers nobody
    /// wanted and reopened the picker three times. Now the right arrow walks in,
    /// the left arrow walks out, and Enter is the one key that answers.
    ///
    /// `None` while nothing is open, so the next picker starts from whatever the
    /// row says rather than from where the last one was left.
    pub browsing: Option<std::path::PathBuf>,
    /// The boards this owner has, as `owner/number` and what each is called.
    ///
    /// Fed like the folders and the drives. The value is the pair the row takes;
    /// the title is what makes it choosable, because `acme/7` and `acme/9` are
    /// the same to anybody who has not opened them.
    pub boards: Vec<(String, String)>,
    /// The drives this machine has, for a walk that has to leave one.
    ///
    /// Fed like the folders. Walking up from a home reaches that drive's root
    /// and stops — the parent of a root is nothing — so a worktree on another
    /// drive was unreachable by walking and could only be typed.
    pub drives: Vec<String>,
    /// The directories the shell last looked up, for a row that takes one.
    ///
    /// Fed rather than fetched: this state machine has no terminal in it and no
    /// filesystem either, which is the only reason it can be driven by a test.
    /// The shell refreshes this before it draws, the way it hands over the
    /// installed tables — and an empty list is simply a picker with no folders
    /// on it, which is what a machine that could not read the directory should
    /// show.
    pub folders: Vec<String>,
    /// Dynamic advisory model IDs the shell has loaded, by their owning adapter.
    ///
    /// A present empty list means the host probe was unavailable or returned no
    /// configured models; custom input remains available either way.
    model_catalogs: std::collections::BTreeMap<&'static str, Vec<String>>,
    /// The model target whose custom ID is being edited.
    model_edit: Option<&'static str>,
    /// Which of the per-agent step's two panels the keys are going to.
    ///
    /// The step has two lists and one set of arrow keys, and until this existed
    /// the only way to move between them was a letter — `a` walked the agents
    /// while the arrows moved the rows. Two lists sharing one cursor is a
    /// screen that has to be explained, and the one key nobody has to be taught
    /// is the one that already means *forward*.
    ///
    /// So `Tab` advances the focus and then the step: left panel, right panel,
    /// next step. That keeps the rule the step walk already states — *the one
    /// key that moves the screen forward means the same thing everywhere* —
    /// rather than making `Tab` mean two different things depending on where it
    /// is pressed. `a` still works, as a shortcut rather than as the only door.
    pub panel: Panel,
    /// Whether one set of answers stands for every agent.
    ///
    /// The per-agent step exists because two agents genuinely differ — Claude
    /// Code runs Opus where OpenCode runs Kimi, in the same checkout. But that
    /// is the *interesting* case, not the common one, and answering six rows
    /// once per agent is six chances to set the same thing four times and get
    /// it wrong on the third.
    ///
    /// So the step asks which of the two it is, and the answer changes only
    /// where a write lands: with this set, an agent row spreads to every table
    /// the way a repository row already does. Nothing else about the rows
    /// changes — a setting that differs by agent still differs by agent, and
    /// turning this off leaves each table holding what it was last given.
    ///
    /// **Off until the screen offers the switch.** On by default is the right
    /// end state — the operator who wants them all the same should not have to
    /// find a control, and the one who wants them different is already looking
    /// for one — but defaulting to it before the control exists takes away the
    /// case the per-agent step was built for: Claude Code on Opus while
    /// OpenCode runs Kimi, in the same checkout, with no way back. A capability
    /// removed is worse than one not yet offered.
    pub uniform: bool,
    /// Which setting the cursor is on, within the current step's list.
    pub selected: usize,
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
    /// Whether anything has been saved this session.
    pub saved: bool,
    /// The language this screen speaks.
    ///
    /// English in every constructor, and set from the machine's remembered
    /// preference by the caller that owns a terminal. Reading a file here would
    /// put the world back inside the state machine, which is the one thing this
    /// module exists not to do — and it would make every test in this crate
    /// depend on whatever language the person running it last chose.
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
            // Until a caller that knows better says otherwise. A screen handed
            // one checkout's tables and nothing else is a screen whose base is
            // those tables.
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
            step: Step::Agents,
            agent: 0,
            installed: configs.clone(),
            configs,
            repository: String::new(),
            repositories: Vec::new(),
            boards: Vec::new(),
            browsing: None,
            drives: Vec::new(),
            folders: Vec::new(),
            model_catalogs: std::collections::BTreeMap::new(),
            model_edit: None,
            panel: Panel::Who,
            uniform: false,
            selected: 0,
            focus: Focus::List,
            pick: 0,
            scroll: 0,
            draft: String::new(),
            message: None,
            saved: false,
            tongue: Tongue::English,
        }
    }

    /// Opens on one table, edited whole: `estigia config edit`.
    ///
    /// `adapter` is `None` for the shared contract. Some adapter still carries
    /// the table — the screen has to key it under something — but the purpose
    /// remembers that nobody in particular owns it, so no adapter's limits are
    /// applied to rows every agent reads.
    pub fn one_table(adapter: Option<&'static AgentAdapter>, installed: Config) -> Self {
        let carrier = adapter.unwrap_or(&AGENTS[0]);
        let mut configs = std::collections::BTreeMap::new();
        configs.insert(carrier.slug, installed);
        Self {
            bare: configs.clone(),
            page: Page::Setup,
            entry: 0,
            report: None,
            modal: None,
            purpose: Purpose::OneTable(adapter),
            // Unused while pinned — the rows are the whole table — but it has to
            // be something, and the settings step is what this screen is.
            step: Step::PerAgent,
            agents: vec![(carrier, true)],
            installed_agents: vec![carrier.slug],
            agent: 0,
            installed: configs.clone(),
            configs,
            repository: String::new(),
            repositories: Vec::new(),
            boards: Vec::new(),
            browsing: None,
            drives: Vec::new(),
            folders: Vec::new(),
            model_catalogs: std::collections::BTreeMap::new(),
            model_edit: None,
            panel: Panel::Who,
            uniform: false,
            selected: 0,
            focus: Focus::List,
            pick: 0,
            scroll: 0,
            draft: String::new(),
            message: None,
            saved: false,
            tongue: Tongue::English,
        }
    }

    /// Records an install that landed, and says so over the screen.
    ///
    /// Only tables in the writer's read-back receipt become installed, which is
    /// not bookkeeping for its own sake: `dirty`, `touched` and the tick beside
    /// each agent all answer "does this differ from what is on disk". Treating
    /// every in-memory table as written hides an edit on an agent unticked just
    /// before save and lets quit discard it without warning.
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
        self.saved = !acknowledged.is_empty() || repository.is_some();
    }

    /// Records an install that did not land, and says so over the screen.
    ///
    /// Nothing else moves: the tables are still unsaved, because they are.
    pub fn install_failed(&mut self, title: String, detail: String) {
        self.message = None;
        self.modal = Some(Modal {
            ok: false,
            title,
            detail: Some(detail),
        });
    }

    /// Whether this screen edits one table rather than walking the steps.
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
        // Every adapter is seeded in the constructor, so this cannot be absent
        // — and if it ever were, editing a table nobody will read is worse than
        // stopping: the operator would type answers into nothing.
        let Some(config) = self.configs.get_mut(slug) else {
            unreachable!("{slug} has no table, so its answers would go nowhere")
        };
        config
    }

    /// The rows this screen offers.
    ///
    /// Every persisted setting except `Models` when the screen was handed a
    /// table. `Planning` moves to the end; [`Self::model_targets`] projects the
    /// one hidden cell beneath it without manufacturing persisted settings.
    pub fn rows(&self) -> &'static [Setting] {
        match self.purpose {
            // An adapter that shares a skill root is edited through a file of
            // its **own**, and that file holds only the rows that differ by
            // agent — the repository's are answered by the contract underneath
            // it. Offering all seventeen here took an answer the write then
            // dropped: `config edit --agent qwen`, change the tracker, save, and
            // the tracker is untouched with nothing on screen saying so.
            //
            // The same rule this screen already applies one page over, to a step
            // with no agents ticked: a row whose answer this run will not write
            // is a row it must not ask about.
            Purpose::OneTable(Some(adapter)) if !adapter.discovers_skills() => AGENT_TUI_SETTINGS,
            Purpose::OneTable(_) => TUI_SETTINGS,
            // The repository's own rows, wherever the steps happen to be left:
            // this page is entered from the menu and owes nothing to them.
            // Without the board when the chosen tracker has no board. Linear
            // and Trello declare no mirror and the transport asks for one only
            // under GitHub, so on those the row is a question with no answer —
            // and a setting nothing can read is the defect this table has
            // already produced three times.
            Purpose::Setup if self.page == Page::Options => {
                if Setting::Board
                    .applies_to(&self.configs[self.shown_from(Setting::Tracker)].tracker)
                {
                    OPTIONS_SETTINGS
                } else {
                    crate::config::OPTIONS_SETTINGS_WITHOUT_BOARD
                }
            }
            // Nothing ticked means every one of these rows would be answered
            // into a table this run will not write. Offering them anyway is the
            // screen taking answers it has already decided to throw away.
            Purpose::Setup if self.step == Step::PerAgent && self.chosen().is_empty() => &[],
            Purpose::Setup if self.step == Step::PerAgent => AGENT_TUI_SETTINGS,
            Purpose::Setup => self.step.settings(),
        }
    }

    /// The model rows projected beneath `Planning` for the active answer.
    ///
    /// When selected agents disagree about Planning, only targets that exist
    /// under every protocol remain. Showing one agent's phase list under an
    /// `EVERY AGENT` heading would report a shared state that was never read.
    pub fn model_targets(&self) -> Vec<ModelTarget> {
        if !self.rows().contains(&Setting::Planning) {
            return Vec::new();
        }
        let planning = if self.uniform && self.disagrees(Setting::Planning) {
            crate::config::Planning::Direct
        } else {
            self.shown_config(Setting::Planning).planning
        };
        ModelRouting::visible_targets(planning)
    }

    /// Whether the concrete adapter has reviewed presets to offer.
    pub fn has_model_profiles(&self) -> bool {
        self.rows().contains(&Setting::Planning)
            && self
                .model_adapter()
                .is_some_and(|adapter| !adapter.model_profiles().is_empty())
    }

    /// Whether the synthetic profile row is under the cursor.
    pub fn model_profile_at_cursor(&self) -> bool {
        self.has_model_profiles() && self.selected == self.rows().len()
    }

    /// How many rows the list under the cursor has.
    ///
    /// The options page's list is the repository's rows **and** this screen's
    /// own preferences under them: one cursor over two kinds of row, because a
    /// second focusable list would have to teach which one the arrows belong to
    /// before anything on the page could be answered.
    pub fn row_count(&self) -> usize {
        match self.page {
            // `self.rows()`, not the full list. It was `OPTIONS_SETTINGS.len()`
            // — a second answer to how many rows there are — and the day the
            // page stopped offering all of them the cursor would have walked
            // one row past the end of the list it was drawing.
            Page::Options => self.rows().len() + SCREEN_ROWS.len(),
            _ => {
                self.rows().len()
                    + usize::from(self.has_model_profiles())
                    + self.model_targets().len()
            }
        }
    }

    /// The line this page is really showing, remembered.
    ///
    /// The draw knows how far the text can go — it depends on the window — and
    /// it used to clamp the position for itself and leave the state free. So
    /// scrolling past the bottom kept counting: after twenty presses of `j` at
    /// the end, twenty presses of `k` moved nothing at all, because the first
    /// nineteen only spent an overshoot the screen had never shown.
    ///
    /// **Fed the limit** rather than working it out: how tall the window is is
    /// not something this file can know, and a screen state that guesses at
    /// layout is one nothing can test.
    pub fn showing(&mut self, furthest: u16) -> u16 {
        self.scroll = self.scroll.min(furthest);
        self.scroll
    }

    /// The screen preference under the cursor, when the cursor is on one.
    pub fn screen_at_cursor(&self) -> Option<Screen> {
        if self.page != Page::Options {
            return None;
        }
        // First, not last. These two decide what the rows under them *mean* —
        // `Repository shown` chooses which checkout's answers the page is
        // showing — and a control that changes the meaning of everything below
        // it belongs above it. The cursor order moves with the drawing order,
        // because a panel that is drawn first and walked last is a screen whose
        // arrow keys have to be explained.
        SCREEN_ROWS.get(self.selected).copied()
    }

    /// The setting under the cursor, if this step has any.
    ///
    /// `None` on a screen preference as well as on a step with no rows: a
    /// preference is not a row of any contract, and answering it as though it
    /// were would write a language into eleven agents' tables.
    pub fn setting_at_cursor(&self) -> Option<Setting> {
        if self.screen_at_cursor().is_some() {
            return None;
        }
        let rows = self.rows();
        // Offset on the options page, where the screen rows come first.
        let at = if self.page == Page::Options {
            self.selected.saturating_sub(SCREEN_ROWS.len())
        } else {
            self.selected
        };
        rows.get(at).copied()
    }

    /// The derived model target under the cursor, if it is on one.
    pub fn model_target_at_cursor(&self) -> Option<ModelTarget> {
        let model_start = self.rows().len() + usize::from(self.has_model_profiles());
        if self.page == Page::Options || self.selected < model_start {
            return None;
        }
        self.model_targets()
            .get(self.selected.saturating_sub(model_start))
            .copied()
    }

    /// The setting under the cursor.
    ///
    /// Steps with no settings have no cursor to be under; callers on those
    /// steps ask [`Self::setting_at_cursor`] instead.
    pub fn setting(&self) -> Setting {
        self.setting_at_cursor().unwrap_or(AGENT_TUI_SETTINGS[0])
    }

    /// What one setting is worth for the adapter under the cursor.
    pub fn applies(&self, setting: Setting) -> Applies {
        match self.purpose {
            Purpose::Setup => self.adapter().applies(setting),
            Purpose::OneTable(Some(adapter)) => adapter.applies(setting),
            // The shared contract is read by every agent, so no one of them
            // gets to say a row in it is worth nothing.
            Purpose::OneTable(None) => Applies::Held,
        }
    }

    /// The value shown for one row on the current step.
    ///
    /// A repository-wide row is one answer written into every agent's table, so
    /// what it shows is what they agree on. They can still disagree — somebody
    /// may have hand-edited one file — and [`Self::disagrees`] is how the screen
    /// says so rather than quietly showing the first one it found.
    pub fn shown_value(&self, setting: Setting) -> String {
        setting.value_of(self.shown_config(setting))
    }

    /// The whole table [`Self::shown_value`] reads that row out of.
    ///
    /// A row sometimes has to be explained in terms of a *different* row —
    /// `Model routing`'s phase keys mean nothing without the `Planning` in
    /// force — and reading that neighbour out of some other agent's table would
    /// explain the value on screen with a setting that is not beside it. Sharing
    /// Sharing the private `shown_from` with [`Self::shown_value`] is what keeps
    /// the two answers from drifting apart. Named rather than linked, because a
    /// public page cannot link a private item and `cargo doc` is on the list of
    /// commands this repository runs precisely because it once was not.
    pub fn shown_config(&self, setting: Setting) -> &Config {
        &self.configs[self.shown_from(setting)]
    }

    /// The table one row's value is read out of.
    ///
    /// One place, because it is one rule and it was starting to be held in two:
    /// the restore key had its own copy, and the copy did not learn about the
    /// shared answer when this did. Its comment already said what that costs —
    /// *"restoring from a different one would put back a value that was never
    /// on screen"* — so the two disagreeing was the exact fault it names.
    ///
    /// On the shared answer an agent row is written everywhere, so it is read
    /// the same way a repository row is: from the first table that carries it,
    /// with [`Self::disagrees`] saying so when they do not agree.
    fn shown_from(&self, setting: Setting) -> &'static str {
        match setting.scope() {
            Scope::Agent if !self.uniform => self.current(),
            Scope::Agent | Scope::Everywhere | Scope::Machine => {
                self.interested().first().copied().unwrap_or(self.current())
            }
        }
    }

    /// Whether a row is not the same everywhere it is written.
    ///
    /// Agent rows are exempt only while each agent answers for itself. Under
    /// the shared answer they are written everywhere, so they can disagree
    /// everywhere — and the panel says `EVERY AGENT` above them.
    ///
    /// Without this the screen showed one agent's value under that heading and
    /// nothing said the others held something else: switching to the shared
    /// answer with two agents already differing displayed `sdd lite` for both
    /// while one of them was on `direct`, and only the rows somebody happened
    /// to touch ever became true. A screen that reports a state it has not read
    /// is the fault this crate exists to refuse, and it was one this screen had
    /// already solved one scope over.
    pub fn disagrees(&self, setting: Setting) -> bool {
        if setting.scope() == Scope::Agent && !self.uniform {
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

    /// The adapters a repository-wide answer is written into.
    ///
    /// The ticked ones, because those are the tables this run will write. With
    /// nothing ticked yet the answer is still worth showing, so every adapter
    /// stands in — a blank row on the options page would look like a missing
    /// setting rather than an unanswered question.
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
    ///
    /// Uniform means the agents chosen for this run, not every config the TUI
    /// loaded and not whichever chosen agent happens to supply the shown value.
    fn model_destinations(&self) -> Vec<&'static str> {
        if self.uniform {
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
        let shared = configs[first].models.for_target(target).map(str::to_owned);
        if destinations.all(|slug| configs[slug].models.for_target(target) == shared.as_deref()) {
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
            ModelAssignment::Shared(Some(model)) => model,
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
        self.model_destinations().into_iter().any(|slug| {
            self.configs[slug].models.for_target(target)
                != self.installed[slug].models.for_target(target)
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
    ///
    /// A repository-wide answer goes into **every** table, which is what makes
    /// it repository-wide. Model routing is narrower: its uniform answer goes
    /// only to the agents chosen for this run, through `model_destinations`.
    ///
    /// Checked once against a copy before anything is written, so a value half
    /// the tables accept cannot leave the rest holding the old one.
    pub fn set(&mut self, setting: Setting, value: &str) -> Result<(), Refusal> {
        let selected_target = self.model_target_at_cursor();
        let mut probe = self.config().clone();
        setting.apply(&mut probe, value)?;
        if setting == Setting::Models {
            let _ = self.replace_model_route(value)?;
            return Ok(());
        }
        // A shared agent answer has the same destination boundary as a shared
        // model answer: the agents selected for this run. Applying it to every
        // loaded table lets an unticked agent carry an edit the writer will not
        // persist. Stage every destination before replacing the live map so a
        // rejection can never leave a partially shared answer behind.
        let destinations = match setting.scope() {
            Scope::Agent if !self.uniform => vec![self.current()],
            Scope::Agent => self.model_destinations(),
            Scope::Everywhere | Scope::Machine => self.configs.keys().copied().collect(),
        };
        let mut staged = self.configs.clone();
        for slug in destinations {
            setting.apply(staged.entry(slug).or_default(), value)?;
        }
        self.configs = staged;
        let result = Ok(());
        if result.is_ok() && setting == Setting::Planning {
            self.restore_model_cursor(selected_target);
        }
        result
    }

    /// Whether this row differs from what is installed.
    ///
    /// Shown per row rather than as one "unsaved changes" flag, because the
    /// question an operator actually has is *which* of the sixteen they
    /// touched — and a single flag answers a question nobody asked.
    pub fn changed(&self, setting: Setting) -> bool {
        // The third door of the same family, and the one that would have been
        // left open: `shown_value` and `disagrees` both had to learn that an
        // agent row under the shared answer is written everywhere, and so does
        // this. A row spread to four tables, unchanged in the one the cursor is
        // on, is still an unsaved edit in the other three — and a mark that says
        // otherwise is the screen answering about a table nobody asked about.
        match setting.scope() {
            Scope::Agent if !self.uniform => {
                setting.value_of(self.config()) != setting.value_of(&self.installed[self.current()])
            }
            Scope::Agent | Scope::Everywhere | Scope::Machine => {
                self.interested().into_iter().any(|slug| {
                    setting.value_of(&self.configs[slug]) != setting.value_of(&self.installed[slug])
                })
            }
        }
    }

    /// Whether this step is carrying an answer that is not installed yet.
    ///
    /// Marked on the stepper, because a screen that asks one question at a time
    /// has to say where the answers already given are — otherwise the only way
    /// to find an edit made two steps ago is to walk back and look.
    pub fn touched(&self, step: Step) -> bool {
        if step == Step::Agents {
            let mut now: Vec<&str> = self.chosen().iter().map(|a| a.slug).collect();
            let mut before = self.installed_agents.clone();
            now.sort_unstable();
            before.sort_unstable();
            return now != before;
        }
        self.any_unsaved(step.settings())
    }

    /// Whether the repository's own rows are carrying an unsaved answer.
    ///
    /// The stepper marks a step that is; the repository's rows left the stepper
    /// when they became [`Page::Options`], and an edit with nothing on screen
    /// saying it is unsaved is an edit somebody walks away from. So the menu
    /// carries the same mark, in the same vocabulary, beside the entry.
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
    ///
    /// Across every table rather than the one under the cursor: an operator who
    /// edited one agent, walked to another and pressed `q` was told there was
    /// nothing to lose.
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
        let next = (self.selected as isize + delta).rem_euclid(count);
        self.selected = next as usize;
        self.message = None;
    }

    fn restore_model_cursor(&mut self, target: Option<ModelTarget>) {
        let Some(target) = target else {
            self.selected = self.selected.min(self.row_count().saturating_sub(1));
            return;
        };
        if let Some(at) = self
            .model_targets()
            .iter()
            .position(|candidate| candidate.name == target.name)
        {
            self.selected = self.rows().len() + usize::from(self.has_model_profiles()) + at;
        } else if let Some(at) = self
            .rows()
            .iter()
            .position(|setting| *setting == Setting::Planning)
        {
            self.selected = at;
        } else {
            self.selected = self.selected.min(self.row_count().saturating_sub(1));
        }
    }

    /// The adapters that are ticked.
    pub fn chosen(&self) -> Vec<&'static AgentAdapter> {
        self.agents
            .iter()
            .filter(|(_, on)| *on)
            .map(|(adapter, _)| *adapter)
            .collect()
    }

    /// The menu entry under the cursor.
    pub fn entry(&self) -> Entry {
        MENU[self.entry.min(MENU.len() - 1)]
    }

    /// Moves to another step, leaving the cursor somewhere that exists.
    fn go(&mut self, step: Step) {
        self.step = step;
        self.selected = 0;
        self.focus = Focus::List;
        self.model_edit = None;
        self.draft.clear();
        self.message = None;
        // The per-agent step is about the agents somebody chose, so it opens on
        // one of them. Landing on an unticked adapter would invite edits to a
        // table this run is not going to write.
        if step == Step::PerAgent
            && !self.agents[self.agent].1
            && let Some(first) = self.agents.iter().position(|(_, on)| *on)
        {
            self.agent = first;
        }
    }

    /// One step along, stopping at the ends rather than wrapping.
    ///
    /// A stepper that wraps takes somebody from the last step back to the first
    /// on the key they were using to go forward, which reads as the screen
    /// having thrown their answers away.
    fn step_by(&mut self, delta: isize) {
        let at = STEPS
            .iter()
            .position(|step| *step == self.step)
            .unwrap_or(0) as isize;
        let next = (at + delta).clamp(0, STEPS.len() as isize - 1) as usize;
        if STEPS[next] != self.step {
            self.go(STEPS[next]);
        }
    }

    /// Applies one key and says what the caller must do.
    pub fn press(&mut self, key: Key) -> Action {
        // The verdict is laid over whatever is showing, so it answers first: the
        // key that dismisses it is spent on that and never also reaches the step
        // behind it, which would be one keystroke doing two things with only one
        // of them visible.
        //
        // Where it leaves somebody depends on what it said, because those are
        // two different situations rather than one with two colours. An install
        // that landed is **finished** — the next thing is the guard, the checks,
        // or leaving, and all three are on the menu. One that refused is not:
        // the answers are still unsaved and the thing to fix is on the step they
        // were standing on, so sending them to the menu would cost them the walk
        // back through it.
        if let Some(verdict) = self.modal.take() {
            if verdict.ok {
                self.page = Page::Home;
                self.report = None;
                self.scroll = 0;
            }
            return Action::None;
        }
        match self.page {
            Page::Home => return self.in_menu(key),
            // Both are read-only: anything but leaving goes back to the menu,
            // so nobody is stuck on a page whose keys they have to guess.
            // Read-only, and taller than most windows: the help runs to thirty
            // lines and the doctor's report grows with the checks. Without the
            // arrow keys the bottom of either is simply unreachable, which is
            // the same as not having written it.
            Page::Doctor | Page::Help => {
                match key {
                    Key::Char('q') => {
                        if self.warn_unsaved() {
                            return Action::None;
                        }
                        return Action::Quit;
                    }
                    Key::Down | Key::Char('j') => self.scroll = self.scroll.saturating_add(1),
                    Key::Up | Key::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                    // Anything else returns, so nobody is stuck on a page whose
                    // keys they have to guess.
                    _ => {
                        self.page = Page::Home;
                        self.scroll = 0;
                        // The text belonged to the page being left, and goes
                        // with it. The menu draws `report` as a notice along
                        // its foot — one line, from the guard, which never
                        // leaves the menu to say it. A page's whole body poured
                        // into that slot covers the menu it just returned to.
                        self.report = None;
                    }
                }
                return Action::None;
            }
            Page::Options => return self.in_options(key),
            Page::Setup => {}
        }
        self.in_setup(key)
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
                        self.page = page;
                        // Setup always opens on its first question. Resuming
                        // somebody's third step after they walked out of it is
                        // a screen that starts in the middle of a sentence.
                        if page == Page::Setup {
                            self.go(Step::Agents);
                        }
                        // The options page opens at the top of its list for the
                        // same reason, and with nothing half-open behind it: a
                        // picker left showing from the last visit would be a
                        // list of answers to a row nobody is looking at.
                        if page == Page::Options {
                            self.selected = 0;
                            self.focus = Focus::List;
                            self.model_edit = None;
                            self.draft.clear();
                            self.message = None;
                        }
                        Action::None
                    }
                    Goes::Doing("guard") => Action::Guard,
                    Goes::Doing("doctor") => Action::Doctor,
                    Goes::Doing("quit") => {
                        if self.warn_unsaved() {
                            return Action::None;
                        }
                        Action::Quit
                    }
                    // A menu entry naming an action nothing performs would be a
                    // key that quietly does nothing, which reads as broken.
                    Goes::Doing(_) => Action::None,
                };
            }
            Key::Char('q') | Key::Esc => {
                if self.warn_unsaved() {
                    return Action::None;
                }
                return Action::Quit;
            }
            _ => {}
        }
        Action::None
    }

    /// The pane that is open, if one is: while it is, every key belongs to it.
    ///
    /// Shared by every page that shows rows, because a field that swallows keys
    /// on one screen and not on another is a field that eats an answer.
    fn in_open_pane(&mut self, key: Key) -> Option<Action> {
        match self.focus {
            Focus::Editing => Some(self.in_editor(key)),
            Focus::Picking => Some(self.in_picker(key)),
            Focus::Naming => Some(self.in_namer(key)),
            Focus::List => None,
        }
    }

    /// Keys on the options page: one list, and the keys that write it.
    ///
    /// No steps and no agents to walk — every row here is a fact about the
    /// repository, so there is nothing to move between.
    fn in_options(&mut self, key: Key) -> Action {
        if let Some(action) = self.in_open_pane(key) {
            return action;
        }
        self.in_list(key)
    }

    /// Keys on the setup page.
    fn in_setup(&mut self, key: Key) -> Action {
        if let Some(action) = self.in_open_pane(key) {
            return action;
        }
        // One table has no steps to walk, and a Tab that silently did nothing
        // would read as broken — so it is not offered in the footer either.
        if self.pinned() {
            return self.in_list(key);
        }
        // A number goes straight to its step. Walking back two steps to change
        // one answer is four keys for a screen that already numbers them.
        if let Key::Char(digit @ '1'..='9') = key {
            let at = digit as usize - '1' as usize;
            if let Some(step) = STEPS.get(at) {
                let step = *step;
                if step != self.step {
                    self.go(step);
                }
                return Action::None;
            }
        }
        // Tab walks the steps wherever it is pressed, so the one key that moves
        // the screen forward means the same thing on every one of them.
        match key {
            // `Tab` belongs to the one step that has two panels, and to
            // nothing else. Moving between steps is `Enter` forward and
            // `Backspace` back — accept and undo, the two words a person
            // already has for a form — so `Tab` is left meaning the one thing
            // it means everywhere else: *the other field*.
            Key::Tab | Key::BackTab if self.walks_agents() => {
                self.panel = match self.panel {
                    Panel::Who => Panel::Rows,
                    Panel::Rows => Panel::Who,
                };
                return Action::None;
            }
            Key::Tab | Key::BackTab => return Action::None,
            // Accept and undo, on every step. `Enter` means *this one is
            // answered* and `Backspace` means *take me back* — the two words
            // anybody filling in a form already has. Neither is ambiguous here:
            // a picker or a text field is handled above and swallows `Enter`
            // while it is open.
            // On the last step there is nothing further to accept, and what
            // `Enter` confirms there is the install itself — which is what the
            // step is showing. Falling through to the step's own keys, where
            // `s` already means this.
            Key::Enter if self.page == Page::Setup && self.step == Step::Install => {}
            Key::Enter => {
                self.step_by(1);
                self.panel = Panel::Who;
                return Action::None;
            }
            Key::Backspace => {
                self.step_by(-1);
                self.panel = Panel::Who;
                return Action::None;
            }
            _ => {}
        }
        match self.step {
            Step::Agents => self.in_agents(key),
            Step::PerAgent => self.in_list(key),
            Step::Install => self.in_install(key),
        }
    }

    /// The keys every step of setup answers the same way.
    ///
    /// `None` means this key was not one of them and the step should read it.
    fn shared_key(&mut self, key: Key) -> Option<Action> {
        match key {
            Key::Char('s') => Some(self.install()),
            Key::Char('q') | Key::Esc => Some(self.leave(key)),
            _ => None,
        }
    }

    /// Installing, or saying why not.
    fn install(&mut self) -> Action {
        // Nothing ticked is not a plan. Installing into no agent writes nothing
        // and would report success, which is the shape of lie this whole tool
        // exists to refuse.
        if self.chosen().is_empty() {
            // The page as well as the step. Pressed on the options page this
            // put the cursor on a step nobody was looking at and left the
            // operator reading "choose an agent" with no agents on screen.
            self.page = Page::Setup;
            self.step = Step::Agents;
            self.focus = Focus::List;
            self.message = Some(
                t!(
                    self.tongue,
                    "choose at least one agent — space ticks the one under the cursor"
                )
                .to_owned(),
            );
            return Action::None;
        }
        Action::Save
    }

    /// Says once that there is something to lose, and whether it said it.
    ///
    /// Refusing outright is how a TUI traps somebody; going silently is worse
    /// than one extra key. Extracted because the menu needs the same sentence:
    /// it *shows* the unsaved mark beside `Options` — that is what
    /// [`Self::options_touched`] is for — and then let `q` discard the edit
    /// without a word. A screen that knows enough to draw the mark knows
    /// enough to ask.
    fn warn_unsaved(&mut self) -> bool {
        if !self.dirty() || self.message.is_some() {
            return false;
        }
        self.message = Some(
            t!(
                self.tongue,
                "unsaved changes — press again to discard, or s to install"
            )
            .to_owned(),
        );
        true
    }

    /// Leaving, warning once if there is anything to lose.
    fn leave(&mut self, key: Key) -> Action {
        // `Esc` goes back to the menu and `q` leaves the program, but both can
        // lose unsaved answers, so both ask once.
        if self.warn_unsaved() {
            return Action::None;
        }
        // `Esc` goes back to the menu — but a pinned screen was opened straight
        // onto its table and has no menu behind it, so there is nowhere for Esc
        // to go except out.
        if key == Key::Esc && !self.pinned() {
            self.page = Page::Home;
            self.message = None;
            return Action::None;
        }
        Action::Quit
    }

    /// Keys while choosing agents.
    fn in_agents(&mut self, key: Key) -> Action {
        if let Some(action) = self.shared_key(key) {
            return action;
        }
        let count = self.agents.len();
        match key {
            Key::Up | Key::Char('k') => {
                self.agent = (self.agent + count - 1) % count;
                self.message = None;
            }
            Key::Down | Key::Char('j') => {
                self.agent = (self.agent + 1) % count;
                self.message = None;
            }
            // Space marks, `Enter` accepts. They were one key here, which made
            // `Enter` mean *tick this one* on the first step and *go on* on
            // every other — the ambiguity the keymap was straightened out to
            // remove.
            Key::Char(' ') => {
                if let Some((_, on)) = self.agents.get_mut(self.agent) {
                    *on = !*on;
                }
                self.message = None;
            }
            _ => {}
        }
        Action::None
    }

    /// Whether `a` has anything to move here.
    ///
    /// Only the per-agent step of setup: one table was handed its adapter as an
    /// argument, and the options page's rows are the same whichever agent asks.
    pub fn walks_agents(&self) -> bool {
        self.page == Page::Setup && self.step == Step::PerAgent && !self.pinned()
    }

    /// Moves to the next ticked adapter, wrapping.
    ///
    /// This used to be reachable **only** through `a`, and the reason written
    /// here was that a screen with two lists has to teach which one the arrows
    /// belong to. The objection was right and the answer was wrong: what
    /// teaches it is `Tab`, which already means *forward* on every step, and
    /// which now walks the panels before it walks the steps. `a` stays as the
    /// shortcut it always was.
    fn walk_chosen(&mut self, delta: isize) {
        let selected_target = self.model_target_at_cursor();
        // One more stop than there are agents, and it comes first: **every
        // agent**, where an agent row is answered once and lands in all of
        // them. The other reading — this agent and no other — is the rest of
        // the cycle, and the two are the same key because they are the same
        // question: *who is this answer for*.
        //
        // Written as positions rather than by threading a flag through the hop
        // loop below: `uniform` is not one of the adapters, and a loop that
        // pretended it was would have to skip it exactly once and get the
        // wrapping right in both directions.
        let chosen: Vec<usize> = (0..self.agents.len())
            .filter(|at| self.agents[*at].1)
            .collect();
        if chosen.is_empty() {
            return;
        }
        let here = if self.uniform {
            0
        } else {
            // `map_or(1)`, not `map_or(0)`: a cursor sitting on an adapter
            // nobody ticked is shown as the first chosen one, so that is the
            // stop it is on. Reading it as *every agent* made the first press
            // land back where it already was, and the key looked dead.
            chosen
                .iter()
                .position(|at| *at == self.agent)
                .map_or(1, |at| at + 1)
        };
        let stops = chosen.len() as isize + 1;
        let there = (here as isize + delta).rem_euclid(stops);
        self.message = None;
        if there == 0 {
            self.uniform = true;
        } else {
            self.uniform = false;
            self.agent = chosen[(there - 1) as usize];
        }
        self.restore_model_cursor(selected_target);
    }

    /// Where this agent stands among the ones chosen, counting from one.
    pub fn agent_place(&self) -> (usize, usize) {
        let chosen = self.chosen();
        let at = chosen
            .iter()
            .position(|adapter| adapter.slug == self.current())
            .map_or(1, |at| at + 1);
        (at, chosen.len())
    }

    /// The answers the picker is offering for the row under the cursor.
    ///
    /// The setting's own list, and — where that list is not the whole
    /// vocabulary — one more entry that opens somewhere to type. A picker over
    /// three of the possible answers with no way to reach the fourth is worse
    /// than no picker at all.
    ///
    /// For a row whose answer is a directory it also carries the folders the
    /// shell last looked up, so a path is walked rather than typed.
    /// Derived model rows instead render typed model actions, so a translated
    /// control label can never become persistence or control flow.
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
        self.picker_entries()
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
        self.model_catalogs.insert(adapter.slug, models);
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
            ModelCatalogSource::OpenCode if self.model_catalogs.contains_key(adapter.slug) => t!(
                self.tongue,
                "OpenCode model catalog unavailable or empty; type a model ID"
            )
            .to_owned(),
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

    fn model_adapter(&self) -> Option<&'static AgentAdapter> {
        if self.uniform {
            return None;
        }
        match self.purpose {
            Purpose::OneTable(adapter) => adapter,
            Purpose::Setup => Some(self.adapter()),
        }
    }

    fn model_current(&self, target: &str) -> Option<String> {
        match self.model_assignment(target) {
            ModelAssignment::Shared(model) => model,
            ModelAssignment::Divergent => None,
        }
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
    ///
    /// Named here rather than decided by the shell, because it is a question
    /// about the *state*: what this row is set to, and where that leaves the
    /// walk. The shell only performs it.
    pub fn folder_root(&self) -> std::path::PathBuf {
        // Where the walk is, when one is under way. Reading the row instead
        // meant every step down had to *answer* the row to move, so walking
        // three folders deep wrote three values nobody wanted.
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
        // A preference offers exactly the languages this screen has words for,
        // and nowhere to type: one it does not carry would render in English
        // with nothing on screen saying why.
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
        let mut entries: Vec<String> = answers
            .choices
            .iter()
            .map(|choice| (*choice).to_owned())
            .collect();
        // A value somebody typed goes on the list too, so the picker opens on
        // what is actually set rather than jumping to an answer nobody chose.
        let current = self.shown_value(setting);
        if answers.at(&current).is_none() && !current.is_empty() {
            entries.insert(0, current);
        }
        // The folders under whatever it is set to, so a path is walked rather
        // than typed. Choosing one sets the row to it, and opening the picker
        // again offers *its* children — which is a directory browser made of
        // the list this screen already has, with no new keys to learn.
        // The boards this owner has, above `none`: the row takes `owner/number`
        // and the number is the part nobody knows without opening a browser.
        for (value, _) in self.boards.iter().rev() {
            if setting == Setting::Board && !entries.iter().any(|entry| entry == value) {
                entries.insert(0, value.clone());
            }
        }
        if setting.takes_a_directory() {
            for folder in self.folders.iter().rev() {
                entries.insert(0, folder.clone());
            }
            // The drives last, so a walk that has to leave this one has
            // somewhere to go. Below the folders because leaving the drive is
            // the rarer move, and above nothing: a list whose last entry is a
            // drive root reads as the end of the walk, which is what it is.
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
    ///
    /// It looked before **every** frame: a `read_dir` of wherever the walk was,
    /// and — since the drives were added — twenty-six `is_dir` probes, one per
    /// letter. Both on every keypress, on every row, whether or not anything on
    /// screen could use them.
    ///
    /// Measured here: half a millisecond a frame for the letters, with only
    /// local drives. The cost is not the point. A mapped network drive that is
    /// no longer reachable answers `is_dir` when the share times out, and that
    /// is seconds — so the screen would stop answering keys on a row that has
    /// nothing to do with folders.
    ///
    /// Pure and fed, so the shell asks rather than deciding: the one thing this
    /// state machine must never have is a filesystem.
    pub fn wants_folders(&self) -> bool {
        self.setting_at_cursor()
            .is_some_and(Setting::takes_a_directory)
    }

    /// Whether the open picker is a folder walk rather than a list of answers.
    ///
    /// The navigation keys belong to that walk and to nothing else: `l` and `h`
    /// are letters on every other row, and a picker of three words that moved
    /// when they were pressed would be a list with keys nobody could see.
    fn browsing_a_folder(&self) -> bool {
        // The same question as `wants_folders`, and it was the same expression
        // written out again twelve lines away. One of the two answering
        // differently is a screen whose navigation keys and whose picker
        // disagree about what the row is.
        self.wants_folders()
    }

    /// Whether this row decides nothing here, saying so if it does not.
    fn refuse_if_inert(&mut self, setting: Setting) -> bool {
        let applies = self.applies(setting);
        if applies.editable() {
            return false;
        }
        // Taking the answer and writing it would tell the operator they had
        // configured something they had not.
        self.message = applies.because().map(|why| {
            fill!(
                self.tongue,
                "{label} has no effect here: {why}",
                "label" => setting.label(),
                "why" => t!(self.tongue, why)
            )
        });
        true
    }

    /// Steps the row under the cursor to its next or previous answer.
    fn cycle(&mut self, delta: isize) -> Action {
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
        if self.refuse_if_inert(setting) {
            return Action::None;
        }
        let current = self.shown_value(setting);
        let Some(next) = setting.answers().step(&current, delta) else {
            return Action::None;
        };
        match self.set(setting, next) {
            Ok(()) => self.message = None,
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
    /// It takes effect here rather than on a later `s`: the next frame is drawn
    /// in the new language, which is the only way somebody can tell whether the
    /// one they picked is the one they wanted. What the caller still owes is
    /// remembering it — see [`Action::Remember`].
    pub(crate) fn choose_screen(&mut self, screen: Screen, value: &str) -> Action {
        match screen {
            // Nothing is written here. Choosing a checkout changes what the
            // page is *showing*, and the answers come back through the shell —
            // a screen that quietly wrote one repository's rows into another
            // would be the worst thing this page could do.
            Screen::Repository => {
                self.message = None;
                // The boards belong to the checkout that was showing. Kept, they
                // would offer another repository's projects on a page that had
                // already renamed itself — and a board is chosen once and
                // mirrored to for the life of the configuration.
                //
                // Cleared here rather than by the shell after the read: this is
                // an invariant of the state, and a state whose invariants live
                // in the caller is one no test can hold to them.
                self.boards.clear();
                Action::Reload(value.to_owned())
            }
            Screen::Language => match Tongue::from_name(value) {
                Some(tongue) => {
                    self.tongue = tongue;
                    self.message = None;
                    Action::Remember
                }
                // Unreachable through the picker, which only offers the names
                // above — and silently doing nothing is what it must not do if
                // it ever is reached.
                None => Action::None,
            },
        }
    }

    /// Keys while moving through a list of settings.
    fn in_list(&mut self, key: Key) -> Action {
        if let Some(action) = self.shared_key(key) {
            return action;
        }
        match key {
            // The arrows go to whichever panel `Tab` left them in. `a` and `A`
            // below still walk the agents from either side, because a shortcut
            // that keeps working costs nothing and the habit is already there.
            Key::Up | Key::Char('k') if self.walks_agents() && self.panel == Panel::Who => {
                self.walk_chosen(-1);
            }
            Key::Down | Key::Char('j') if self.walks_agents() && self.panel == Panel::Who => {
                self.walk_chosen(1);
            }
            Key::Up | Key::Char('k') => self.move_by(-1),
            Key::Down | Key::Char('j') => self.move_by(1),
            // The answer changes under the cursor, with no field to open and
            // nothing to type. Most rows have two or three answers and this is
            // the whole interaction for them.
            Key::Left | Key::Char('h') => return self.cycle(-1),
            Key::Right | Key::Char('l') => return self.cycle(1),
            // Which agent is being configured, on the step that has more than
            // one. Elsewhere there is a single table — or, on the options page,
            // rows that are the same whichever agent asks — and the key would
            // move nothing, so it is not offered there either.
            Key::Char('a') if self.walks_agents() => self.walk_chosen(1),
            Key::Char('A') if self.walks_agents() => self.walk_chosen(-1),
            // Space acts on the row under the cursor: it ticks an agent one
            // step over, and here it opens what that row offers.
            Key::Char(' ') => {
                // A preference opens the same list, on what it is set to. It
                // has no scope to check and nothing to refuse: the answers are
                // the languages this screen has words for, and it is set to one
                // of them by construction.
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
                    self.panel = Panel::Rows;
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
                    if self.refuse_if_inert(Setting::Models) {
                        return Action::None;
                    }
                    self.panel = Panel::Rows;
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
                if self.refuse_if_inert(setting) {
                    return Action::None;
                }
                // The boards this owner has, asked for once and only for the row
                // that takes one. A number nobody can look up is a value nobody
                // can supply, which is the ratchet this crate applies to every
                // refusal — and `acme/7` is exactly that kind of value.
                let ask_for_boards = setting == Setting::Board && self.boards.is_empty();
                // Opened on what is set, so the answer already chosen is the
                // one under the cursor rather than somewhere down the list.
                let current = self.shown_value(setting);
                self.pick = self
                    .picker()
                    .iter()
                    .position(|entry| *entry == current)
                    .unwrap_or(0);
                self.panel = Panel::Rows;
                self.focus = Focus::Picking;
                self.message = None;
                if ask_for_boards {
                    return Action::ListBoards;
                }
            }
            Key::Char('r') => {
                // A preference has nothing to restore *to*: it took effect when
                // the key was pressed and was remembered then. Saying so beats
                // a key that quietly does nothing, which reads as broken.
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
                // Undo this row, not the screen. A single "revert everything"
                // is a key that can lose fifteen good edits to fix one.
                let Some(setting) = self.setting_at_cursor() else {
                    return Action::None;
                };
                // The same table the row is *shown* from, asked rather than
                // restated: a repository-wide row displays one agent's answer,
                // and restoring from a different one would put back a value
                // that was never on screen.
                let from = self.shown_from(setting);
                let installed = setting.value_of(&self.installed[from]);
                let _ = self.set(setting, &installed);
                self.message = Some(fill!(
                    self.tongue,
                    "{label} restored to what is installed",
                    "label" => setting.label()
                ));
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
            match installed[slug].models.for_target(target) {
                Some(model) => {
                    let _ = routing.assign(target, model);
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
                        match self.update_model_route(|routing| routing.assign(target, &model)) {
                            Ok(true) => {
                                self.close_model_picker();
                            }
                            Ok(false) => {
                                self.message = Some(
                                    t!(
                                        self.tongue,
                                        "a model ID must fit one key=model entry: no comma, pipe, or line break"
                                    )
                                    .to_owned(),
                                );
                            }
                            Err(refusal) => self.message = Some(format!("{refusal}")),
                        }
                    }
                    ModelEntry::TypeModel => {
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
        let entries = self.picker();
        if entries.is_empty() {
            self.focus = Focus::List;
            return Action::None;
        }
        let count = entries.len();
        match key {
            Key::Up | Key::Char('k') => self.pick = (self.pick + count - 1) % count,
            Key::Down | Key::Char('j') => self.pick = (self.pick + 1) % count,
            // A list that is open closes rather than quitting the program: `q`
            // here is somebody backing out of it, not leaving.
            Key::Esc | Key::Char('q') => {
                self.focus = Focus::List;
                self.browsing = None;
                self.message = None;
            }
            // Into a folder, without answering the row. Walking and choosing
            // were one key: every step down set the value and closed the list,
            // so reaching a folder three deep wrote three answers nobody wanted
            // and reopened the picker three times. That is what a picker with no
            // navigation feels like, and it is the same list either way — what
            // was missing was a key that moves without deciding.
            Key::Right | Key::Char('l') if self.browsing_a_folder() => {
                let into = entries[self.pick.min(count - 1)].clone();
                self.browsing = Some(std::path::PathBuf::from(into));
                self.pick = 0;
                self.message = None;
            }
            // And back out. The parent is offered as an entry too, so this is
            // the shortcut rather than the only way — but a walk with no way
            // back is one somebody leaves by pressing Escape and starting again.
            Key::Left | Key::Char('h') if self.browsing_a_folder() => {
                let here = self.folder_root();
                if let Some(parent) = here.parent() {
                    self.browsing = Some(parent.to_path_buf());
                    self.pick = 0;
                    self.message = None;
                }
            }
            // A folder that does not exist yet. Choosing where isolated
            // checkouts go usually means naming one, and a picker that can only
            // walk into what is already there sends somebody out to a shell and
            // back.
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
                // Against the *translated* entry, because that is what the
                // list is showing. Compared to the English constant, the way
                // out of a picker on a Spanish screen was an entry that chose
                // itself as a literal value.
                if chosen == t!(self.tongue, TYPE_IT) {
                    // The field opens on the current value rather than empty:
                    // most edits are a tweak, and clearing it first makes the
                    // operator retype what was already right. A canned answer
                    // with no meaning is a placeholder, not editable content.
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
                        // The walk ends with the answer. Left standing, the next
                        // picker would open where the last one was abandoned
                        // rather than on what the row says.
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

    /// Keys on the last step, which only reads and installs.
    fn in_install(&mut self, key: Key) -> Action {
        if let Some(action) = self.shared_key(key) {
            return action;
        }
        if key == Key::Enter {
            return self.install();
        }
        Action::None
    }

    /// Keys while naming a folder to make.
    ///
    /// Enter names the path and hands it to the shell; the walk stays where it
    /// is until the shell says the directory is there, because a picker that
    /// moved into a folder nobody had made yet would be listing nothing and
    /// calling it empty.
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
                // Refused here rather than by the filesystem, because the
                // message a person needs is about the name they typed and not
                // about a path they never saw. A separator would make one folder
                // out of two, or reach out of the directory they are in.
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
                if !ModelRouting::accepts_model_id(&value) {
                    self.message = Some(
                        t!(
                            self.tongue,
                            "a model ID must fit one key=model entry: no comma, pipe, or line break"
                        )
                        .to_owned(),
                    );
                    return Action::None;
                }
                match self.update_model_route(|routing| routing.assign(target, &value)) {
                    Ok(true) => {
                        self.close_model_picker();
                    }
                    Ok(false) => {
                        self.message = Some(
                            t!(
                                self.tongue,
                                "a model ID must fit one key=model entry: no comma, pipe, or line break"
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
        match key {
            Key::Char(character) => self.draft.push(character),
            Key::Backspace => {
                self.draft.pop();
            }
            Key::Esc => {
                // Out of the editor, and the value is left as it was: walking
                // away from an edit is not making it.
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
                    // The refusal already says what the setting accepts, so it
                    // is shown and the editor stays open with what was typed.
                    // Closing it would lose the answer to punish a typo.
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
    /// Tab.
    Tab,
    /// Shift-Tab.
    BackTab,
    /// Backspace.
    Backspace,
    /// Anything else, which this screen ignores.
    Other,
}
