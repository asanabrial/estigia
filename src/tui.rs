//! The screen: a menu, and the work behind it.
//!
//! Setup asks three questions rather than showing every setting at once, and
//! the order is the order the answers depend on each other: which agents, what
//! each of them may do, and what all of it adds up to. Every row on one
//! screen answers everything and asks nothing — whoever meets it has to work out
//! unaided which rows are about one agent and which are about the repository,
//! and nothing on the screen said.
//!
//! The rows that are about the repository are `Options`, off the menu. They
//! were a fourth step and are a page because they answer a different question —
//! not who is holding the tools, but what is true here whichever of them asks —
//! and they are the ones somebody comes back to change. As a step, changing
//! where the issues live meant walking past two questions already answered.
//!
//! Answers are **chosen, not typed**. Nearly every setting has two or three
//! answers and no others, and making somebody type one of three words exactly,
//! from memory, into a field that punishes a typo is work the screen already
//! knows how to do for them. Left and right change the row under the cursor;
//! Space opens the whole list; a row that takes something no list can hold — a
//! path or a board — offers a field at the bottom of that list. Model targets
//! are derived beneath `Planning`; each opens that concrete agent's advisory
//! suggestions directly and always offers custom-ID and target-local inherit.
//!
//! `estigia config edit` opens the same screen with no steps at all: it was told
//! which table on the command line, so there is nothing to choose and nothing to
//! install. See [`app::Purpose`].
//!
//! The split follows Leteo's TUI: [`app`] holds the state a key press moves and
//! returns an [`app::Action`] for the caller, and everything that touches a
//! terminal lives here. That is the only seam that makes a TUI testable — a
//! state machine that reaches for the screen has to be driven through one.

pub mod app;
mod models;
pub mod words;

use crate::config::{
    AGENT_SETTINGS, Config, ModelTargetKind, OPTIONS_SETTINGS, SETTINGS, Scope, Setting,
};
use crate::outcome::Refusal;
use app::{
    Action, App, Focus, Key, MENU, ModelAssignment, Page, Panel, SCREEN_ROWS, STEPS, Screen, Step,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use words::{Tongue, fill, t};

/// Runs the screen and returns the configuration to write, if any.
///
/// A map from adapter slug to table: this screen edits **one table per agent**,
/// because "what does this agent do" is the question it is asked.
///
/// `None` means the operator left without saving, which is not a failure and
/// must not be reported as one.
pub fn edit(agent: Option<&str>, installed: Config) -> Result<Option<Config>, Refusal> {
    // One table, edited whole. The steps belong to `setup`, which is a decision
    // about a machine; this was handed its table as an argument and has nothing
    // to choose and nothing to install.
    let adapter = agent.and_then(|slug| {
        crate::setup::AGENTS
            .iter()
            .find(|adapter| adapter.slug == slug)
    });
    let mut app = App::one_table(adapter, installed);
    app.tongue = words::remembered(None);
    Ok(run(app, None)?.map(|(_, configs)| configs.values().next().cloned().unwrap_or_default()))
}

/// The setup screen: agents on the left, settings on the right.
///
/// This is where installing happens. There is no separate wizard, because two
/// ways to answer the same questions is two things to keep in step — and the
/// one that walks an operator through every prompt in a fixed order is the
/// worse of the two for the thing they do most often, which is *change one row*.
///
/// `install` performs the write and hands back a **one-line verdict**, which is
/// what the box over the screen says. It runs inside the event loop rather than
/// after it, for the reason `Action::Guard` gives: ending the screen to report
/// one thing makes somebody start again. It is a callback rather than code
/// living here so there is still exactly one piece of code that installs
/// anything — and the caller keeps the long report, which belongs in the shell
/// where it can be scrolled, not in a panel that vanishes with the screen.
pub fn setup(
    installed: &std::collections::BTreeMap<&'static str, Config>,
    bare: &std::collections::BTreeMap<&'static str, Config>,
    configured: &[&str],
    install: &mut Installer<'_>,
) -> Result<(), Refusal> {
    // The one place the machine's remembered language is read. The state
    // machine never touches a disk — that seam is what makes it testable, and a
    // constructor that read a file would make every test in this crate depend
    // on whatever language the person running it last chose.
    let mut app = App::with_configs(installed, configured);
    // What each agent answers with where no checkout has said anything. The
    // tables above already have this checkout's rows on them, which is right
    // for what the page opens showing and wrong for anything laid over it.
    app.bare = bare.clone();
    app.tongue = words::remembered(None);
    run(app, Some(install)).map(|_| ())
}

/// What a completed install proved by reading the written configuration back.
#[derive(Debug, Clone)]
pub struct InstallReceipt {
    /// The short verdict shown over the screen.
    pub summary: String,
    /// Effective tables read back for the agents the writer persisted.
    pub read_back: std::collections::BTreeMap<&'static str, Config>,
    /// The same agents read back before this repository's rows are layered.
    pub unlayered_read_back: std::collections::BTreeMap<&'static str, Config>,
    /// The portable contract read back without adapter or local overrides.
    pub contract_read_back: std::collections::BTreeMap<&'static str, Config>,
    /// Cumulative read-back immediately after an adapter's own file.
    pub agent_read_back: std::collections::BTreeMap<&'static str, Config>,
    /// Cumulative read-back immediately after the operator's local file.
    pub local_read_back: std::collections::BTreeMap<&'static str, Config>,
    /// Settings whose intended values were proved by effective read-back.
    pub acknowledged: std::collections::BTreeMap<&'static str, Vec<crate::config::Setting>>,
    /// Adapters whose complete setup lifecycle returned successfully.
    pub completed: std::collections::BTreeSet<&'static str>,
    /// The repository-scoped values read back after its file was written.
    pub repository: Option<Config>,
    /// The exact repository-scoped rows that document explicitly owns.
    pub repository_settings: Vec<crate::config::Setting>,
}

/// A failed install together with every scope read back before it failed.
#[derive(Debug, Clone)]
pub struct InstallFailure {
    /// The classified failure shown to the operator and returned to the shell.
    pub refusal: Refusal,
    /// Only proven state; unproven settings remain dirty in the screen.
    pub receipt: Box<InstallReceipt>,
}

impl std::fmt::Display for InstallFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.refusal.fmt(formatter)
    }
}

impl std::ops::Deref for InstallFailure {
    type Target = Refusal;

    fn deref(&self) -> &Self::Target {
        &self.refusal
    }
}

impl From<Refusal> for InstallFailure {
    fn from(refusal: Refusal) -> Self {
        Self {
            refusal,
            receipt: Box::new(InstallReceipt::empty(String::new())),
        }
    }
}

impl InstallReceipt {
    /// A receipt carrying no acknowledgement evidence.
    pub fn empty(summary: String) -> Self {
        Self {
            summary,
            read_back: std::collections::BTreeMap::new(),
            unlayered_read_back: std::collections::BTreeMap::new(),
            contract_read_back: std::collections::BTreeMap::new(),
            agent_read_back: std::collections::BTreeMap::new(),
            local_read_back: std::collections::BTreeMap::new(),
            acknowledged: std::collections::BTreeMap::new(),
            completed: std::collections::BTreeSet::new(),
            repository: None,
            repository_settings: Vec::new(),
        }
    }
}

/// What performs the write, and says what it did.
///
/// Held as a callback so the screen can install without owning the code that
/// installs: `guided` is still the only place that writes an agent's files.
pub type Installer<'a> = dyn FnMut(&Plan) -> Result<InstallReceipt, InstallFailure> + 'a;

/// What the screen decided.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The adapters to install into.
    pub agents: Vec<&'static crate::setup::AgentAdapter>,
    /// The effective configurations the screen opened on.
    pub opened: std::collections::BTreeMap<&'static str, Config>,
    /// The merged view, which is what this checkout's own file is written from.
    pub rows: std::collections::BTreeMap<&'static str, Config>,
    /// The checkout these answers are **about**.
    ///
    /// Carried rather than asked of the process, because since the page can be
    /// pointed at another checkout the two are not the same question. A write
    /// that asks where it is standing writes one repository's rows into
    /// another the moment somebody uses the dropdown.
    pub repository: std::path::PathBuf,
}

/// What a save asks for, read off the screen.
///
/// A function rather than four lines inside the loop, because the loop needs a
/// terminal and nothing that needs a terminal can be measured. What it decides
/// is which of the two tables goes where — and putting the merged view in the
/// contract's slot is a defect no test could see while this was written inline.
fn plan_of(app: &App) -> Plan {
    Plan {
        agents: app.chosen(),
        opened: app.installed.clone(),
        rows: app.configs.clone(),
        repository: std::path::PathBuf::from(&app.repository),
    }
}

/// Runs one install attempt and keeps the shell-visible refusal in step with it.
fn install_from_screen(app: &mut App, install: &mut Installer<'_>, refused: &mut Option<Refusal>) {
    let plan = plan_of(app);
    match install(&plan) {
        Ok(receipt) => {
            *refused = None;
            app.installed_now(receipt);
        }
        Err(failure) => {
            app.installed_partially(*failure.receipt);
            let refusal = failure.refusal;
            app.install_failed(
                format!("{} ({})", refusal.message, refusal.code),
                refusal.resolution.to_string(),
            );
            *refused = Some(refusal);
        }
    }
}

/// Drives one screen and returns what it decided.
type Decided = (
    Vec<&'static crate::setup::AgentAdapter>,
    std::collections::BTreeMap<&'static str, Config>,
);

fn run(mut app: App, mut install: Option<&mut Installer<'_>>) -> Result<Option<Decided>, Refusal> {
    let mut terminal = ratatui::init();
    // A refusal is shown in the box and *also* kept, because it is two things at
    // once: something the operator has to read, and an exit code the shell that
    // ran this is waiting for. Ending the screen on it would cost the first;
    // showing it and returning `Ok` would cost the second, and a command that
    // failed while reporting success is the shape of lie this tool refuses.
    let mut refused: Option<Refusal> = None;
    let outcome = loop {
        if let Err(error) = terminal.draw(|frame| draw(frame, &mut app)) {
            break Err(io_refusal("draw to the terminal", &error));
        }
        let event = match crossterm::event::read() {
            Ok(event) => event,
            Err(error) => break Err(io_refusal("read a key", &error)),
        };
        let Some(pressed) = key_of(&event) else {
            continue;
        };
        // The world, before the state machine is asked anything. A row that
        // takes a directory offers the ones that are there, and *there* is a
        // question only the shell may ask — the machine has no filesystem in it
        // for the same reason it has no terminal.
        // Only where they can be used. Asked on every frame, this was a
        // `read_dir` and twenty-six drive probes per keypress on every row of
        // the screen — and a mapped drive that has gone away answers `is_dir`
        // when its share times out, which is seconds of a screen that has
        // stopped taking keys.
        if app.wants_folders() {
            app.folders = folders_under(&app.folder_root());
            if app.drives.is_empty() {
                app.drives = drives_here();
            }
        }
        // The checkouts that answer for themselves, pruned by the reader, plus
        // the one this was run in — which is always offered even when it has
        // never answered for itself, because it is the one somebody is in.
        app.repositories = known_repositories_for();
        // The one this was run in, until somebody points the page elsewhere.
        if app.repository.is_empty() {
            app.repository = repo_dir().display().to_string();
        }
        match app.press(pressed) {
            Action::None => {}
            // With an installer, the write happens here and the screen stays.
            // Without one — `config edit`, which was handed a single table and
            // has nothing to install — the decision goes back to the caller.
            Action::Save => {
                let decided = (app.chosen(), app.configs.clone());
                let Some(install) = install.as_mut() else {
                    break Ok(Some(decided));
                };
                install_from_screen(&mut app, *install, &mut refused);
            }
            // Performed here and reported back into the screen rather than
            // ending it: an operator who installed the guard is usually about
            // to do the next thing, and dropping them to the shell to read one
            // line makes them start again.
            Action::Guard => {
                app.report = Some(install_guard(app.tongue));
            }
            // The screen already changed language — `press` did that, and the
            // frame after this one is drawn in it. What is left is tomorrow.
            //
            // A failure here is shown and **not** returned: the thing the
            // operator asked for happened, so ending the run with an error
            // would be reporting a failure about a screen that is visibly in
            // the language they picked. What they have to be told is the part
            // that did not happen, which is that it will not still be in it
            // next time.
            Action::Remember => {
                if let Err(refusal) = words::remember(None, app.tongue) {
                    app.install_failed(
                        format!("{} ({})", refusal.message, refusal.code),
                        refusal.resolution.to_string(),
                    );
                }
            }
            Action::Doctor => {
                app.page = app::Page::Doctor;
                app.scroll = 0;
                // Drawn before the checks run, not after. They shell out to
                // git, the tracker CLI and the interpreter, and a screen that
                // goes still for two seconds with nothing on it reads as a
                // hang — which is the point at which people press keys.
                app.report = Some(
                    t!(
                        app.tongue,
                        "running the checks — git, the tracker CLI and the interpreter…"
                    )
                    .to_owned(),
                );
                let _ = terminal.draw(|frame| draw(frame, &mut app));
                app.report = Some(report_of_checks());
            }
            // The chosen checkout's own answers, read and handed back. Nothing
            // is written: this only changes what the page is showing.
            //
            // The row moves **after** the read, and only if it worked. Shown
            // and not returned, for the reason `Action::Remember` gives: the
            // page is still usable and still true — it names the checkout it is
            // actually showing, which is the one it was showing before.
            Action::Reload(chosen) => match reload_repository(&mut app, &chosen) {
                Ok(()) => app.repository = chosen,
                Err(refusal) => app.install_failed(
                    format!("{} ({})", refusal.message, refusal.code),
                    refusal.resolution.to_string(),
                ),
            },
            // Made here, because this state machine has no filesystem in it.
            // The walk moves into the folder only once it exists: a picker that
            // stepped in first would be listing nothing and calling it empty.
            Action::MakeFolder(path) => match std::fs::create_dir_all(&path) {
                Ok(()) => {
                    app.browsing = Some(path);
                    app.pick = 0;
                }
                Err(error) => {
                    app.message = Some(format!("{}: {error}", path.display()));
                }
            },
            // Asked of the transport the binding already names, not of `gh`
            // directly: a second way to reach the tracker is a second answer to
            // what it says. A read that fails leaves the list empty and says so
            // — the row still takes a typed pair, so nothing is lost but the
            // convenience.
            Action::ListBoards => match boards_for(&app) {
                Ok(boards) => app.boards = boards,
                Err(why) => app.message = Some(why),
            },
            Action::LoadModelCatalog(adapter) => {
                // Show the stage and its loading note before a bounded host read.
                // Without this, the screen appears to ignore Enter for up to the
                // timeout even though the state has already moved.
                let _ = terminal.draw(|frame| draw(frame, &mut app));
                match models::load(adapter) {
                    Ok(models) => app.model_catalog_loaded(adapter, models),
                    Err(error) => {
                        app.model_catalog_loaded(adapter, Vec::new());
                        app.message = Some(fill!(
                            app.tongue,
                            "{agent} model catalog unavailable: {why}. Type a model ID instead.",
                            "agent" => adapter.display_name,
                            "why" => error
                        ));
                    }
                }
            }
            Action::Quit => break Ok(None),
        }
    };
    ratatui::restore();
    // The refusal outlives the box it was shown in: whoever ran the command is
    // still waiting for an exit code.
    match refused {
        Some(refusal) => Err(refusal),
        None => outcome,
    }
}

/// Installs the pre-push guard, and says what happened either way.
///
/// The executable is resolved from this process rather than assumed to be on
/// the path: the hook runs later, from git, in an environment that may not have
/// the same `PATH` this screen does.
fn install_guard(tongue: Tongue) -> String {
    let repo = repo_dir();
    let Ok(executable) = std::env::current_exe() else {
        return t!(
            tongue,
            "could not work out where this executable is, so the hook would name nothing"
        )
        .to_owned();
    };
    let executable = crate::paths::remove_windows_verbatim_prefix(
        executable.canonicalize().unwrap_or(executable),
    );
    match crate::harness::guard::install(&repo, &executable, false) {
        Ok(_) => fill!(tongue, "push guard installed in {where}", "where" => repo.display()),
        Err(refusal) => format!("{refusal}"),
    }
}

/// The frame around a panel, coloured by whether it has the keys.
///
/// Borrowed whole from Leteo's `focus_panel`: rounded borders, cyan where the
/// keys are and dark grey everywhere else, and the heading in the same colour
/// as its border. No padding on the focused one, so a panel that has the keys
/// differs from its neighbours in **colour alone** rather than also shifting
/// its heading along by a character — a title that moves when focus does reads
/// as the layout twitching.
fn panel(title: &str, focused: bool) -> Block<'_> {
    bordered(
        title,
        if focused {
            Color::Cyan
        } else {
            Color::DarkGray
        },
    )
}

/// The one bordered box this screen draws, in whatever colour it is drawn in.
///
/// Focus is what nearly every panel is coloured by, and [`panel`] says so. The
/// verdict box is coloured by its answer instead — a third meaning for the same
/// border — so the colour is the argument here and the shape stays in one place.
/// Built once because a box assembled by hand elsewhere differs in a way nobody
/// notices until it is on screen; held by
/// `every_panel_is_drawn_in_the_one_style_this_screen_has`.
fn bordered(title: &str, colour: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colour))
        .title(Span::styled(title.to_owned(), Style::default().fg(colour)))
}

/// A panel title, with the breathing room every one of them has.
///
/// Written into the string at each call site the padding was part of the text,
/// so a translation had to remember to carry two spaces it could not see. One
/// that forgot put the word against the border.
fn padded(title: &str) -> String {
    format!(" {title} ")
}

/// The selected row, in the one style this screen uses for it.
fn selected() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
}

/// A bar down the right edge of a panel whose list is taller than it.
///
/// Without one, a list that runs past the bottom looks like a list that ends
/// there — and the eleventh agent, or the last three settings, are then rows
/// nobody knows to go looking for.
fn scrollbar(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, total: usize, at: usize) {
    let visible = usize::from(area.height.saturating_sub(2));
    if total <= visible || visible == 0 || area.width < 3 {
        return;
    }
    // Inside the border. Drawn over it, the panel's own frame disappears into a
    // column of blocks and the panel stops looking like a panel.
    let inside = ratatui::layout::Rect {
        x: area.x + area.width - 2,
        y: area.y + 1,
        width: 1,
        height: area.height - 2,
    };
    let mut state = ratatui::widgets::ScrollbarState::new(total).position(at);
    frame.render_stateful_widget(
        ratatui::widgets::Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("\u{250a}"))
            .track_style(muted())
            .thumb_style(Style::default().fg(Color::Cyan)),
        inside,
        &mut state,
    );
}

/// The first one-line item Ratatui shows for a freshly rendered list.
///
/// Model rows and the picker hanging from one must use the same viewport. The
/// list state is rebuilt on every frame, so its offset is solely the smallest
/// one that keeps the selected row inside the bordered panel.
fn model_viewport_offset(
    area: ratatui::layout::Rect,
    total: usize,
    selected: Option<usize>,
) -> usize {
    let visible = usize::from(area.height.saturating_sub(2));
    if visible == 0 || total <= visible {
        return 0;
    }
    selected
        .unwrap_or(0)
        .min(total.saturating_sub(1))
        .saturating_add(1)
        .saturating_sub(visible)
}

/// Text cut to a width, with a mark saying it was.
///
/// A name that simply stops at the panel edge reads as the name. Two adapters
/// sharing a prefix would then look like the same one, twice.
fn clipped(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    // Under two columns there is no room for a character *and* the mark, so the
    // mark is what goes. It used to be the clipping that went: `width < 2`
    // returned the text whole, which made this function's one promise — that
    // what comes back fits — false at exactly the widths nobody paints while
    // building, and it is the footer's own guarantee that rested on it.
    if width < 2 {
        return text.chars().take(width).collect();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('\u{2026}');
    out
}

/// The keys, as many of them as the window holds.
///
/// One long line left to the window edge loses whichever key is written last,
/// and last is `s install` — the key the whole screen is for. It also cut
/// words in half: at 44 columns `r restore` read as `r rest`. So hints are
/// dropped whole, and from the middle, where the least is lost: the first says
/// how to move and the last says how to finish.
///
/// **The last is never dropped**, which the middle rule alone did not give.
/// Down to two hints the middle *is* index 1 — the last — so the final removal
/// took exactly the key this function was written to protect. At twenty columns
/// the footer read `up down move` and said nothing about installing.
fn fit_keys<Hint: AsRef<str>>(hints: &[Hint], width: u16) -> String {
    const GAP: &str = "   ";
    let width = usize::from(width);
    let mut kept: Vec<&str> = hints.iter().map(AsRef::as_ref).collect();
    while kept.len() > 1 && kept.join(GAP).chars().count() > width {
        // From the middle, except at two — where the middle *is* the last, and
        // the first is what goes instead. Somebody who has room for one key
        // needs the one that finishes, not the one that moves the cursor.
        let at = if kept.len() == 2 { 0 } else { kept.len() / 2 };
        kept.remove(at);
    }
    clipped(&kept.join(GAP), width)
}

/// Secondary text: descriptions, keys, anything that is not the answer.
fn muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// The repository the screen is looking at.
fn repo_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// The checks, rendered for the page that shows them.
///
/// Run when the entry is chosen rather than when the screen opens: they touch
/// `git`, `gh` and the interpreter, and paying for that on every launch would
/// make the menu slow for somebody who only came to tick an agent.
fn report_of_checks() -> String {
    let repo = repo_dir();
    let root = crate::harness::discover_skill_root().ok();
    let tracker = crate::harness::doctor::tracker_in_force(root.as_deref(), &repo);
    crate::harness::doctor::examine(root.as_deref(), &repo, &tracker)
        .into_iter()
        .map(|check| match check.health {
            crate::harness::doctor::Health::Fine { detail } => {
                format!("ok       {:<11}{detail}", check.name)
            }
            crate::harness::doctor::Health::Skipped { detail } => {
                format!("skipped  {:<11}{detail}", check.name)
            }
            crate::harness::doctor::Health::Broken { detail, .. } => {
                format!("BROKEN   {:<11}{detail}", check.name)
            }
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// A terminal that stopped answering.
pub(crate) fn io_refusal(what: &str, error: &std::io::Error) -> Refusal {
    use crate::outcome::{NoCommandReason, Resolution};
    Refusal::not_started(
        "terminal-unusable",
        format!("could not {what}: {error}"),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "a terminal this process can read and write — or use `estigia config set`, which \
             needs none",
        ),
    )
}

/// The terminal's key, as this screen's key.
/// The checkouts this page may be pointed at.
///
/// The one it was run in first, whether or not it has ever answered for itself
/// — somebody sitting in a checkout expects to see that one — and then the ones
/// the registry knows, pruned by its reader so a repository that has had its
/// answers taken away is not offered.
fn known_repositories_for() -> Vec<String> {
    let mut found = vec![repo_dir().display().to_string()];
    if let Ok(home) = crate::paths::home_dir() {
        for path in crate::skill::known_repositories(&home) {
            let path = path.display().to_string();
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    found
}

/// Reads the chosen checkout's own answers into the page.
///
/// Only the rows that are the repository's. An agent's stay as they are, and so
/// do this machine's: pointing the page at another checkout changes what that
/// checkout says about itself, and nothing else — the same asymmetry the
/// layering keeps everywhere else.
fn reload_repository(app: &mut App, chosen: &str) -> Result<(), Refusal> {
    reload_repository_with(app, chosen, crate::skill::layer_repository)
}

/// The layering, as a seam.
///
/// A function pointer for the reason [`crate::skill::installed_config_with`] has
/// one: the property below is *what a failure leaves behind*, and a failure that
/// cannot be posed is a property nothing holds.
type Layering = fn(&Config, &std::path::Path) -> Result<Config, Refusal>;

fn reload_repository_with(app: &mut App, chosen: &str, layer: Layering) -> Result<(), Refusal> {
    // The layering itself is `skill::layer_repository`, which is where the rule
    // about *which rows a repository may speak for* is kept — and which refuses
    // an unreadable file rather than passing it off as an empty one.
    let chosen = std::path::Path::new(chosen);
    // Built beside the page's tables and swapped in only once every agent has
    // been read. It wrote into them as it went, so a refusal on the second agent
    // left the first one already carrying the chosen checkout's answers — and
    // the caller, correctly, does not move the row that says which checkout is
    // showing. The page then **named one checkout and showed another's rows**,
    // which this module calls the worst thing it can do, and a save would have
    // written that mixture against the checkout it was still naming.
    //
    // Whether a refusal can land on the second agent rather than the first is
    // not something to reason about: this is the one page where being wrong is
    // silent, and all-or-nothing costs one clone of a table.
    let mut next = app.configs.clone();
    for (slug, config) in next.iter_mut() {
        // What that agent answers with where no checkout has said anything.
        // Laying the chosen rows over what is *showing* keeps the previous
        // checkout's answer for every row this one is silent about.
        let Some(base) = app.bare.get(slug) else {
            continue;
        };
        let theirs = layer(base, chosen)?;
        for setting in SETTINGS
            .iter()
            .filter(|setting| setting.scope() == Scope::Everywhere)
        {
            // Only these. An agent's rows and this machine's stay as the
            // operator left them: pointing the page at another checkout changes
            // what that checkout says about itself, and nothing else.
            setting.apply(config, &setting.value_of(&theirs))?;
        }
    }
    app.configs = next;
    Ok(())
}

/// The directories directly under one, named rather than pathed.
///
/// Sorted, because a listing in whatever order the filesystem answers in is a
/// list somebody has to read twice. Hidden ones are left out: `.git` and its
/// neighbours are not where anybody puts a worktree, and a picker that opens on
/// eleven dotted directories has buried the three that matter.
///
/// Silent about failure. A directory that cannot be read is a picker with no
/// folders on it, which is the truth — and refusing to draw the screen over it
/// would cost somebody the tool for a convenience.
/// The roots a walk can reach that are not under this one.
///
/// Walking up ends at the drive the walk started on — the parent of a root is
/// nothing — so a folder on another drive could only be typed. Probed rather
/// than asked: there is no portable listing, and a letter that answers `is_dir`
/// is one somebody can walk into, which is the only question here.
///
/// Empty everywhere else, where `/` is already reachable by walking up.
/// The project boards this checkout's owner has, as the row's own pairs.
///
/// Through `list-boards`, which pages the owner's projects to exhaustion and
/// raises rather than returning a short list — a truncated set of boards offered
/// as *the* set is the partial read this crate refuses everywhere else.
fn boards_for(app: &App) -> Result<Vec<(String, String)>, String> {
    let skill_root = crate::harness::control_surface();
    // The checkout the page is **showing**, not the one this process was run
    // in. They are the same until somebody points the page elsewhere, and after
    // that the owner is a different one: the boards offered would be the other
    // repository's, under this one's name.
    let here = if app.repository.is_empty() {
        repo_dir()
    } else {
        std::path::PathBuf::from(&app.repository)
    };
    // Answered here rather than by spawning the transport. It is the same walk,
    // crossed against it by `listing_the_boards_answers_the_same_on_both_sides`,
    // and it is the first caller moved off Python — the operator's own screen no
    // longer needs an interpreter to fill this row.
    // Through the constructor, like the other two live callers. This built the
    // context by hand with an empty table â€” the shape that turned the board
    // mirror off for every run on the machine when the gate and the tool server
    // did it, and the guard written to stop it from happening again named those
    // two files and not this one.
    //
    // Harmless here **today**: `list_boards` takes the owner it needs and
    // derives the rest from the checkout, so nothing on this screen reads a row.
    // That is a fact about `list_boards` this week, not about the context, and
    // it is not what the next person will check.
    let context = crate::transport::Context::live(skill_root, here, None);
    let body = crate::transport::board::list_boards(&context, None).map_err(|failure| {
        failure
            .envelope()
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("the tracker did not list its boards")
            .to_owned()
    })?;
    Ok(body
        .get("boards")
        .and_then(serde_json::Value::as_array)
        .map(|boards| {
            boards
                .iter()
                .filter_map(|board| {
                    Some((
                        board.get("board")?.as_str()?.to_owned(),
                        board
                            .get("title")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default())
}

fn drives_here() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|drive| std::path::Path::new(drive).is_dir())
        .collect()
}

fn folders_under(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
        })
        .map(|path| path.display().to_string())
        .collect();
    found.sort();
    // The way up, first, so walking back out is one key rather than typing.
    if let Some(parent) = root.parent() {
        found.insert(0, parent.display().to_string());
    }
    found
}

/// The key an event carries, when it carries one this screen acts on.
///
/// **Pure and fed**, because the interesting inputs cannot be produced by a
/// test that drives the state machine: every test here presses keys directly,
/// so the event loop's own filtering was reachable by nothing. A mutation
/// sweep turned it off and the whole suite stayed green.
///
/// What it filters is not cosmetic. Windows reports a key **twice** — once
/// pressed and once released — and acting on both types every character twice
/// and moves the cursor two rows per arrow. On the platform this crate is
/// developed on, without this line the screen is unusable.
fn key_of(event: &crossterm::event::Event) -> Option<Key> {
    let crossterm::event::Event::Key(key) = event else {
        return None;
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return None;
    }
    Some(translate(key.code))
}

fn translate(code: crossterm::event::KeyCode) -> Key {
    use crossterm::event::KeyCode;
    match code {
        KeyCode::Char(character) => Key::Char(character),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Backspace => Key::Backspace,
        _ => Key::Other,
    }
}

/// Paints one frame.
fn draw(frame: &mut ratatui::Frame<'_>, app: &mut App) {
    match app.page {
        Page::Home => draw_menu(frame, app),
        Page::Doctor => draw_text(
            frame,
            t!(app.tongue, "Doctor"),
            app,
            &[t!(app.tongue, "any key returns"), t!(app.tongue, "q quit")],
        ),
        Page::Help => draw_text(
            frame,
            t!(app.tongue, "Help"),
            app,
            &[t!(app.tongue, "any key returns"), t!(app.tongue, "q quit")],
        ),
        Page::Options => draw_options(frame, app),
        Page::Setup => draw_setup(frame, app),
    }
    // Last, and over everything: a verdict about what just happened has to be
    // read before the screen it happened on.
    if let Some(modal) = &app.modal {
        draw_modal(frame, modal, app.tongue);
    }
}

/// The verdict box: centred, over whatever is behind it.
///
/// One question answered — did it work — and, when it did not, what to do. The
/// log that used to be here said everything except that, and thirty lines of it
/// buried the menu it handed back to.
fn draw_modal(frame: &mut ratatui::Frame<'_>, modal: &app::Modal, tongue: Tongue) {
    let (colour, title) = if modal.ok {
        (Color::Green, t!(tongue, "Done"))
    } else {
        (Color::Red, t!(tongue, "Not done"))
    };
    let mut lines = vec![
        Line::from(""),
        Line::styled(
            format!("  {}", modal.title),
            Style::default().fg(colour).add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(detail) = &modal.detail {
        lines.push(Line::from(""));
        lines.push(Line::styled(format!("  {detail}"), muted()));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        format!("  {}", t!(tongue, "any key closes this")),
        muted(),
    ));

    let area = frame.area();
    // Room for the border, the blank rows and whatever the detail wraps to,
    // measured rather than assumed: a refusal's way out is a sentence, and a box
    // sized for the title alone would cut off the half that says what to do.
    let width = area.width.saturating_sub(8).clamp(1, 72);
    let wrapped: usize = lines
        .iter()
        .map(|line| {
            let room = usize::from(width.saturating_sub(4)).max(1);
            line.width().div_ceil(room).max(1)
        })
        .sum();
    let height = u16::try_from(wrapped + 2).unwrap_or(8).min(area.height);
    let box_at = ratatui::layout::Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    // Cleared first: without it the page underneath shows through the gaps.
    frame.render_widget(ratatui::widgets::Clear, box_at);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(bordered(&padded(title), colour)),
        box_at,
    );
}

/// The landing screen: the wordmark and the menu, centred.
///
/// **No frame around it.** Every other page is a bordered panel because it
/// holds a list and the title says which; this one is a mark and a menu, and a
/// box around that is a second border directly under the header's for no gain.
///
/// Two arrangements, chosen by the room available rather than by preference.
/// Side by side the pair costs only as many rows as the mark; stacked it costs
/// the mark plus everything under it, which would keep it off a short window
/// entirely.
fn draw_menu(frame: &mut ratatui::Frame<'_>, app: &App) {
    let [header, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(4)]).areas(frame.area());
    draw_header(frame, app, header);

    let mark: Vec<Line<'static>> = WORDMARK
        .lines()
        .map(|row| Line::styled(row.to_owned(), Style::default().fg(Color::Cyan)))
        .collect();
    let mark_width = width_of(&mark);
    let mark_height = u16::try_from(mark.len()).unwrap_or(u16::MAX);

    // A heading above and the version below: neither is load-bearing, and both
    // go in the stacked arrangement where every row is contested.
    let mut menu = vec![
        Line::styled(format!("  {}", t!(app.tongue, "ACTIONS")), muted()),
        Line::raw(""),
    ];
    menu.extend(menu_entries(app));
    menu.push(Line::raw(""));
    menu.push(Line::styled(
        format!("  estigia {}", env!("CARGO_PKG_VERSION")),
        muted(),
    ));
    let menu_width = width_of(&menu);
    let menu_height = u16::try_from(menu.len()).unwrap_or(u16::MAX);

    // Wide enough that the mark and the menu read as two things rather than as
    // one block with a seam down it.
    const GAP: u16 = 10;
    let paired = mark_width + GAP + menu_width;
    let (mark_at, menu_at) = if body.width >= paired && body.height >= mark_height + 4 {
        let left = body.x + (body.width - paired) / 2;
        let top = body.y + (body.height.saturating_sub(mark_height)) / 2;
        (
            Some(ratatui::layout::Rect {
                x: left,
                y: top,
                width: mark_width,
                height: mark_height,
            }),
            // Centred against the mark rather than against the screen, so the
            // two sit on one axis whatever the window does.
            ratatui::layout::Rect {
                x: left + mark_width + GAP,
                y: top + mark_height.saturating_sub(menu_height) / 2,
                width: menu_width,
                height: menu_height.min(body.height),
            },
        )
    } else {
        // No room beside it: the entries are why the screen exists and are
        // always drawn, so the mark is what goes.
        (
            None,
            ratatui::layout::Rect {
                x: body.x + (body.width.saturating_sub(menu_width)) / 2,
                y: body.y + (body.height.saturating_sub(menu_height)) / 2,
                width: menu_width.min(body.width),
                height: menu_height.min(body.height),
            },
        )
    };

    if let Some(area) = mark_at {
        frame.render_widget(Paragraph::new(mark), area);
    }
    frame.render_widget(Paragraph::new(menu), menu_at);

    // The description of the entry under the cursor, and whatever the last
    // action reported, along the bottom where nothing else is competing.
    let mut foot = vec![Line::styled(
        format!("  {}", t!(app.tongue, app.entry().about)),
        muted(),
    )];
    if let Some(report) = &app.report {
        for line in report.lines() {
            foot.push(Line::styled(
                format!("  {line}"),
                Style::default().fg(Color::Yellow),
            ));
        }
    }
    foot.push(Line::styled(
        format!(
            "  {}",
            fit_keys(
                &[
                    t!(app.tongue, "up/down move"),
                    t!(app.tongue, "enter choose"),
                    t!(app.tongue, "q quit"),
                ],
                body.width.saturating_sub(2)
            )
        ),
        muted(),
    ));
    let height = u16::try_from(foot.len()).unwrap_or(3).min(body.height);
    frame.render_widget(
        Paragraph::new(foot),
        ratatui::layout::Rect {
            x: body.x,
            y: body.y + body.height.saturating_sub(height),
            width: body.width,
            height,
        },
    );
}

/// The mark. Estigia has no drawing, and a wordmark is what stands in for one.
const WORDMARK: &str = "\
 ####  #### ##### #  #### #  ###
 #      #     #   # #     # #   #
 ####   #     #   # #  ## # #####
    #   #     #   # #   # # #   #
 ####  ####   #   #  ###  # #   #";

/// The widest line in a block, for sizing what holds it.
fn width_of(lines: &[Line<'static>]) -> u16 {
    u16::try_from(
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.chars().count())
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0),
    )
    .unwrap_or(u16::MAX)
}

/// One row per menu entry, with the cursor on the selected one.
///
/// Every entry padded to one width so the cursor sits in a column rather than
/// stepping in and out with the length of each label. The cursor is **part of
/// the line** rather than a highlight: a reversed row reads as a bar across the
/// screen, and beside a mark it reads as a fault in it.
fn menu_entries(app: &App) -> Vec<Line<'static>> {
    let width = MENU
        .iter()
        .map(|entry| t!(app.tongue, entry.label).chars().count())
        .max()
        .unwrap_or(0);
    MENU.iter()
        .enumerate()
        .map(|(index, entry)| {
            let chosen = index == app.entry;
            let marker = if chosen { '\u{25b8}' } else { ' ' };
            let style = if chosen {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                // Dimmer than the cursor line by a whole step, so which entry
                // is selected is obvious at a glance rather than on inspection.
                Style::default().fg(Color::Gray)
            };
            let mut spans = vec![Span::styled(
                format!("{marker} {:<width$}", t!(app.tongue, entry.label)),
                style,
            )];
            // The repository's rows left the stepper when they became a page of
            // their own, and the stepper's mark was the only thing saying an
            // answer was given and not yet written. Same mark, same colour, in
            // the one place that entry is ever seen from.
            if entry.goes == app::Goes::To(Page::Options) && app.options_touched() {
                spans.push(Span::styled(
                    " \u{2022}",
                    Style::default().fg(Color::Yellow),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

/// The bar every page carries: what this is, where you are, and how much of it
/// is chosen — said once, where it is always in view.
fn draw_header(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let known = app.agents.len();
    let chosen = app.chosen().len();
    // "1 of 1 agents chosen" on a screen that was handed its table is a count of
    // a choice nobody made. What that screen needs said is which table it holds.
    let context = match app.purpose {
        app::Purpose::OneTable(Some(adapter)) => {
            fill!(app.tongue, "editing {agent}'s own table", "agent" => adapter.slug)
        }
        app::Purpose::OneTable(None) => t!(app.tongue, "editing the shared contract").to_owned(),
        // On the options page the count is not what somebody is deciding — it
        // is **where the answer lands**, which is the whole reason these rows
        // are apart from the per-agent ones. There is no separate shared
        // document: one answer is written into every chosen agent's own table,
        // and with none chosen it is written nowhere.
        app::Purpose::Setup if app.page == Page::Options => match chosen {
            0 => t!(
                app.tongue,
                "no agent is chosen, so there is nowhere to write these"
            )
            .to_owned(),
            1 => t!(app.tongue, "one answer, into the one chosen agent").to_owned(),
            count => fill!(
                app.tongue,
                "one answer, into each of the {count} chosen agents",
                "count" => count
            ),
        },
        app::Purpose::Setup => fill!(
            app.tongue,
            "{chosen} of {known} agents chosen",
            "chosen" => chosen,
            "known" => known
        ),
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " ESTIGIA ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                match app.page {
                    Page::Home => t!(app.tongue, "Menu"),
                    Page::Setup if app.pinned() => t!(app.tongue, "Edit"),
                    Page::Setup => t!(app.tongue, "Setup"),
                    Page::Options => t!(app.tongue, "Options"),
                    Page::Doctor => t!(app.tongue, "Doctor"),
                    Page::Help => t!(app.tongue, "Help"),
                },
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(context, muted()),
        ])),
        area,
    );
}

/// A page that only reads — and scrolls, because it is taller than the window.
///
/// The help runs to thirty lines and the doctor's report grows with the checks.
/// A page that cannot scroll past its own bottom edge has a section nobody can
/// read, which is the same as not having written it.
fn draw_text(frame: &mut ratatui::Frame<'_>, title: &str, app: &mut App, keys: &[&str]) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);
    let text = match app.page {
        Page::Help => t!(app.tongue, HELP).to_owned(),
        _ => app
            .report
            .clone()
            .unwrap_or_else(|| t!(app.tongue, "nothing to show").to_owned()),
    };
    // Held one short of the end rather than free, so `j` on the last line does
    // not scroll the text out of the window entirely.
    let visible = body.height.saturating_sub(2);
    let lines = u16::try_from(text.lines().count()).unwrap_or(u16::MAX);
    let furthest = lines.saturating_sub(visible);
    let at = app.showing(furthest);
    let title = if furthest == 0 {
        padded(title)
    } else if at >= furthest {
        fill!(app.tongue, "{title} — the end", "title" => title)
    } else {
        fill!(
            app.tongue,
            "{title} — {more} more below",
            "title" => title,
            "more" => furthest - at
        )
    };
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((at, 0))
            .block(panel(&padded(title.trim()), true)),
        body,
    );
    let mut hints: Vec<&str> = keys.to_vec();
    if furthest > 0 {
        hints.insert(0, t!(app.tongue, "↑↓ scroll"));
    }
    frame.render_widget(
        Paragraph::new(Span::styled(fit_keys(&hints, footer.width), muted())),
        footer,
    );
}

/// What the screen is, and the keys — written once, here, because a footer can
/// only carry the keys of the step somebody is on.
///
/// Every line is kept inside seventy-six columns. The panel wraps what does not
/// fit, and a wrapped line in a hanging-indent block breaks the column the block
/// exists to make — one word alone at the left margin under a key name reads as
/// another key.
const HELP: &str = "Estigia holds the tools. It gates every repository write, and
every irreversible boundary, against a claim adjudicated on the issue tracker.

  Setup       three questions, in the order their answers depend on each
              other: which agents, what each one may do, and what that
              adds up to.
  Options     what is true of this repository whichever agent asks — where
              the issues live, what history the base branch must end up
              with, how long a routine write may ride on the last check.
              A page rather than a step of setup, because these are the
              rows somebody comes back to change, and reaching them
              should not mean answering \"which agents?\" again first.
              Under them, in their own panel, are the preferences of this
              screen: what it looks like on this machine, written into no
              agent's contract because no agent reads them.
  Push guard  the pre-push hook. The one boundary no agent can go around,
              because it runs in git rather than in the agent.
  Doctor      what a run needs before it swears: the skill, the transport,
              the interpreter, the tracker CLI, the guard, and where each
              configured agent reads its contract.

Keys
  ←→ or hl    change the setting under the cursor, on the row itself
  space       tick an agent, or show every answer a row takes
  ↑↓ or jk    move            ⏎ / backspace  accept and go on / back
  ⇥           the other panel, on the step that has two
  a / A       next / previous agent, on the configuration step
  1 2 3       straight to that step
  r           restore one row to what is installed
  s           install into the ticked agents, from any step or from Options
  Esc         back            q  quit

Marks
  *           this session moved it and has not written it yet
  ·           this repository already sets it to something other than
              what the skill ships
  •           on a step in the bar, or beside Options on the menu: it is
              carrying one of those
  (not held)  written into the contract, with no gate behind it here
  (no effect) decides nothing for this agent, and will not open

Nearly every setting has two or three answers and no others, so ←→ is
usually the whole thing: no field, nothing to type, nothing to spell
right. Space opens the list when you would rather see them all, and a path
or board row has a field for what no list can hold. Planning is the last
primary row. The model targets beneath it open their advisory catalog
directly; Enter or Space chooses, custom IDs are always available, and
inherit removes only that target. Where a model is named, the same list
also offers the effort it runs at, and one entry to hand that back to the
host. Shared answers borrow no one agent's catalog. The CLI still edits
the complete persisted key=model route, where an effort follows its
model after a slash.
Only Claude Code currently receives host-routable definitions: selected
planning phases, one inert blind reviewer in every mode, and the
implementer and analyst workers, which exist because this row names them.
OpenCode and every other host keep these values as routing declarations;
orchestrate, apply, judge, and a visible route are not execution proof.

Some rows are worth less for some agents. Estigia can gate the tool calls
of an agent whose host lets it and no others; the rest get the contract,
and the pre-push guard, and nothing standing between them and an edit.
The per-agent step marks those rows, on the row.

The first step's rows are the same question: `gated` is an agent Estigia
holds, `contract only` is one it can only ask.

A setting's display label is translated. Persisted setting names, accepted
values, and printed CLI commands stay canonical: they are the keys and
cells in markdown tables and the arguments to `estigia config set`.

Nothing is written until `s` — except the preferences of this screen,
which take effect on the key and are remembered at once. Leaving without
`s` changes nothing else.";

/// The repository's own rows: one list, no steps, reached from the menu.
///
/// Deliberately the *same* list panel, detail panel and footer as a settings
/// step of setup. It is one page rather than a step because it answers a
/// different question — not who is holding the tools, but what is true of the
/// repository whichever of them asks — and the rows behave identically, so
/// nothing here should look or key differently.
fn draw_options(frame: &mut ratatui::Frame<'_>, app: &App) {
    let said = detail_lines(app);
    let [header, question, body, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(if said.is_empty() { 0 } else { 7 }),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("  {}", t!(app.tongue, app::OPTIONS_QUESTION)),
            Style::default().fg(Color::White),
        )),
        question,
    );

    // Two panels, one cursor. The contract rows are written into every chosen
    // agent's table by `s`; the ones below take effect the moment a key is
    // pressed and are never written by anything. Drawn in one list they would
    // make the unsaved mark mean two things and `s` write some of its rows and
    // not others — so the difference is a border, and the arrows cross it.
    let screens = u16::try_from(SCREEN_ROWS.len()).unwrap_or(1) + 2;
    // The screen rows above the settings: `Repository shown` decides which
    // checkout's answers the rows below it are, so it was a control that changed
    // the meaning of everything drawn before it.
    let [preferences, rows] =
        Layout::vertical([Constraint::Length(screens), Constraint::Min(3)]).areas(body);
    let here = app.screen_at_cursor();
    draw_settings(
        frame,
        app,
        rows,
        // What the cursor walks, not the full list. Passing `OPTIONS_SETTINGS`
        // here was a second answer to which rows this page has: the day one of
        // them stopped being offered — a board on a tracker that has none — the
        // list drew a row the cursor could not reach.
        app.rows(),
        &padded(t!(app.tongue, "REPOSITORY OPTIONS")),
        here.is_none(),
    );
    draw_screen_rows(frame, app, preferences, here);

    draw_detail(frame, said, detail);
    frame.render_widget(
        Paragraph::new(Span::styled(
            fit_keys(&keys_for(app), footer.width),
            muted(),
        )),
        footer,
    );
    if app.focus == Focus::Picking {
        draw_picker(frame, app, if here.is_some() { preferences } else { rows });
    }
}

/// The two rows that decide what the page is showing, not what it enforces.
///
/// Titled `THIS SCREEN`, which was wrong about both of them: the interface
/// language belongs to the person reading it, and `Repository shown` chooses
/// **which checkout's answers** the rows below are. Neither is a property of a
/// screen, and the second changes what everything else on the page means — so
/// it is drawn first, and the cursor walks it first.
///
/// It carries the page's own name, `OPTIONS`, and the panel under it is
/// `REPOSITORY OPTIONS`: these two are the options of the tool, and those are
/// the answers a checkout gives.
///
/// A panel of its own directly under the contract rows, sharing their cursor.
/// The panel is the sentence: everything above this border is written into an
/// agent's file when `s` is pressed, and everything below it happened already.
fn draw_screen_rows(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
    here: Option<Screen>,
) {
    if area.height < 3 {
        return;
    }
    let rows: Vec<ListItem<'_>> = SCREEN_ROWS
        .iter()
        .map(|screen| {
            ListItem::new(Line::from(vec![
                // No changed mark and no default mark: there is nothing unsaved
                // to mark. The leading space keeps the column with the rows
                // above, which is the whole point of drawing them alike.
                Span::raw(format!("  {:<24}", screen.label(app.tongue))),
                Span::styled("‹ ", Style::default().fg(Color::Cyan)),
                Span::raw(app.screen_value(*screen)),
                Span::styled(" ›", Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(here.and_then(|screen| SCREEN_ROWS.iter().position(|row| *row == screen)));
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(&padded(t!(app.tongue, "OPTIONS")), here.is_some()))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

/// The three questions, and where the answers land.
fn draw_setup(frame: &mut ratatui::Frame<'_>, app: &App) {
    // Worked out before the layout, because a panel with nothing in it should
    // not be given rows. Seven blank rows under a step that has said everything
    // it has to say is furniture, and the step above it wants the room.
    let said = detail_lines(app);
    let [header, stepper, question, body, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(if said.is_empty() { 0 } else { 7 }),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);

    // One table has no steps: `config edit` was told which table on the command
    // line, so its settings and derived model targets are drawn at once.
    if app.pinned() {
        let title = match app.purpose {
            app::Purpose::OneTable(Some(adapter)) => padded(adapter.display_name),
            _ => padded(t!(app.tongue, "CONTRACT")),
        };
        let [list, detail, footer] = Layout::vertical([
            Constraint::Min(5),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .areas(ratatui::layout::Rect {
            height: frame.area().height.saturating_sub(1),
            y: frame.area().y + 1,
            ..frame.area()
        });
        let picker_over = draw_configuration_rows(frame, app, list, &title, true);
        draw_detail(frame, detail_lines(app), detail);
        frame.render_widget(
            Paragraph::new(Span::styled(
                fit_keys(&keys_for(app), footer.width),
                muted(),
            )),
            footer,
        );
        if app.focus == Focus::Picking {
            draw_picker(frame, app, picker_over);
        }
        return;
    }

    draw_stepper(frame, app, stepper);
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("  {}", t!(app.tongue, app.step.question())),
            Style::default().fg(Color::White),
        )),
        question,
    );

    // Where a dropdown would hang from: the panel holding the rows, which is
    // not the whole body on the step that has a sidebar beside it.
    let rows = match app.step {
        Step::Agents => {
            draw_agent_choice(frame, app, body);
            None
        }
        Step::PerAgent => Some(draw_per_agent(frame, app, body)),
        Step::Install => {
            draw_summary(frame, app, body);
            None
        }
    };
    draw_detail(frame, said, detail);
    frame.render_widget(
        Paragraph::new(Span::styled(
            fit_keys(&keys_for(app), footer.width),
            muted(),
        )),
        footer,
    );
    if app.focus == Focus::Picking
        && let Some(rows) = rows
    {
        draw_picker(frame, app, rows);
    }
}

/// `1 Agents ▸ 2 Per agent ▸ 3 Install`.
///
/// Drawn on every step rather than only where somebody might be lost, because
/// its job is not to say where you are — the header already does — but to say
/// **how many questions there are**. A screen that asks one at a time without
/// showing the end of the list is a screen that could go on forever.
fn draw_stepper(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    // Three arrangements, and the fullest that fits is the one drawn. Written
    // one way and handed to the window edge, a narrow window cut it after the
    // third name — so a stepper whose whole job is to say *how many
    // questions there are* said three, and misspelled the third.
    for named in [Named::All, Named::Current, Named::Number] {
        let spans = stepper_spans(app, named);
        let width: usize = spans.iter().map(|span| span.content.chars().count()).sum();
        if width <= usize::from(area.width) || named == Named::Number {
            frame.render_widget(Paragraph::new(Line::from(spans)), area);
            return;
        }
    }
}

/// How much of the stepper is spelled out, when the window is short of room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    /// Every step, by name.
    All,
    /// Only the step somebody is on. The others keep their numbers, which is
    /// what says how many there are.
    Current,
    /// Numbers alone.
    Number,
}

fn stepper_spans(app: &App, named: Named) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw("  ")];
    for (index, step) in STEPS.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(
                match named {
                    // The compact arrangements pull the separators in as well:
                    // five columns a side of breathing room is what pushed the
                    // named form off a thirty-column window.
                    Named::All => "  \u{25b8}  ",
                    _ => " \u{25b8} ",
                },
                muted(),
            ));
        }
        // Steps already answered are a shade apart from those still ahead: an
        // operator on step three wants to know at a glance that one and two are
        // behind them, not read four labels in the same colour.
        let style = if *step == app.step {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if step.number() < app.step.number() {
            Style::default().fg(Color::Gray)
        } else {
            muted()
        };
        let label = match named {
            Named::All => format!("{} {}", step.number(), t!(app.tongue, step.title())),
            Named::Current if *step == app.step => {
                format!("{} {}", step.number(), t!(app.tongue, step.title()))
            }
            _ => step.number().to_string(),
        };
        spans.push(Span::styled(label, style));
        // A screen that asks one question at a time has to say where the
        // answers already given are, or the only way to find an edit made two
        // steps ago is to walk back and look.
        if app.touched(*step) {
            spans.push(Span::styled(
                " \u{2022}",
                Style::default().fg(Color::Yellow),
            ));
        }
    }
    spans
}

/// Step one: which agents.
fn draw_agent_choice(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let rows: Vec<ListItem<'_>> = app
        .agents
        .iter()
        .map(|(adapter, on)| {
            // A box drawn in characters, not colour alone: a terminal without
            // it, or somebody who cannot see it, still has to be able to tell
            // what is chosen.
            let mark = if *on { "[x]" } else { "[ ]" };
            let held = if adapter.can_gate_tools() {
                Span::styled(
                    format!("  {:<14}", t!(app.tongue, "gated")),
                    Style::default().fg(Color::Green),
                )
            } else {
                Span::styled(
                    format!("  {:<14}", t!(app.tongue, "contract only")),
                    muted(),
                )
            };
            // Already installed, or ticked just now. A screen that draws the
            // two identically cannot answer the question people rerun setup to
            // ask, which is what they already have.
            let was = app.installed_agents.contains(&adapter.slug);
            let state = match (was, *on) {
                (true, true) => Span::styled(t!(app.tongue, "installed"), muted()),
                (true, false) => Span::styled(
                    t!(app.tongue, "installed — will be left alone"),
                    Style::default().fg(Color::Yellow),
                ),
                (false, true) => Span::styled(
                    t!(app.tongue, "will be installed"),
                    Style::default().fg(Color::Cyan),
                ),
                (false, false) => Span::raw(""),
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{mark} {:<28}", clipped(adapter.display_name, 28))),
                held,
                state,
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.agent));
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(&padded(t!(app.tongue, "AGENTS")), true))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
    scrollbar(frame, area, app.agents.len(), app.agent);
}

/// Step two: the chosen agents down the side, and the rows of the one being
/// configured.
///
/// The side list is a **map, not a menu**: `a` moves through it. A second
/// focusable list would mean teaching which one the arrow keys belong to before
/// anything on the screen could be answered.
fn draw_per_agent(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    // Every row here would be answered into a table this run will not write.
    // Drawing them anyway is the screen taking answers it has already decided
    // to throw away.
    if app.chosen().is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!(
                        "  {}",
                        t!(
                            app.tongue,
                            "No agent is ticked, so there is nobody to configure."
                        )
                    ),
                    Style::default().fg(Color::Yellow),
                ),
                Line::raw(""),
                Line::styled(
                    format!(
                        "  {}",
                        t!(
                            app.tongue,
                            "Backspace goes back to step 1, where space ticks one."
                        )
                    ),
                    muted(),
                ),
            ])
            .block(panel(&padded(t!(app.tongue, "CONFIGURATION")), true)),
            area,
        );
        return area;
    }

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(32), Constraint::Percentage(68)]).areas(area);

    // Only the ticked ones. This step is about what the agents somebody chose
    // may do, and listing the other eight would offer edits to tables this run
    // is not going to write.
    let here = app.current();
    // The first stop, and the one somebody arriving wants: answer the rows once
    // and let every agent hear it. The agents under it are the other reading —
    // this one and no other — and they are drawn the same way because the same
    // key walks through both.
    let mut rows: Vec<Line<'static>> = vec![Line::styled(
        format!(
            "{} {}",
            if app.uniform { '\u{25b8}' } else { ' ' },
            t!(app.tongue, "Every agent")
        ),
        if app.uniform {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            muted()
        },
    )];
    rows.extend(app.chosen().iter().map(|adapter| {
        let on = !app.uniform && adapter.slug == here;
        Line::styled(
            format!(
                "{} {}",
                if on { '\u{25b8}' } else { ' ' },
                // Cut with a mark rather than by the panel edge: a name that
                // simply stops looks like the name, and two adapters can
                // share a prefix.
                clipped(adapter.display_name, left.width.saturating_sub(4) as usize)
            ),
            if on {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                muted()
            },
        )
    }));
    let (at, of) = app.agent_place();
    // The heading answers the question the panel is for, and the two readings
    // are different questions: with one agent picked it is *which of them is
    // this*, and with none it is *this is for all of them*. A count shown while
    // the answer goes everywhere would say `1 of 4` about a write that reaches
    // four.
    let heading = if app.uniform {
        t!(app.tongue, "ANSWERS FOR ALL — a moves").to_owned()
    } else {
        fill!(
            app.tongue,
            "AGENT {at} OF {of} — a moves",
            "at" => at,
            "of" => of
        )
    };
    frame.render_widget(
        // Bordered when the keys are going here, plain when they are not: with
        // two lists and one set of arrows, which one is lit is the whole of how
        // somebody knows where a press will land.
        Paragraph::new(rows).block(panel(&padded(&heading), app.panel == Panel::Who)),
        left,
    );

    let title = if app.uniform {
        padded(t!(app.tongue, "EVERY AGENT"))
    } else {
        padded(&app.adapter().display_name.to_uppercase())
    };
    draw_configuration_rows(frame, app, right, &title, app.panel == Panel::Rows)
}

/// The persisted settings and the derived model targets, in separate panels.
fn draw_configuration_rows(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
    title: &str,
    focused: bool,
) -> ratatui::layout::Rect {
    let models = app.model_targets();
    if models.is_empty() {
        draw_settings(frame, app, area, app.rows(), title, focused);
        return area;
    }

    let on_model = app.model_profile_at_cursor() || app.model_target_at_cursor().is_some();
    let preferred = u16::try_from(app.rows().len() + 2).unwrap_or(area.height);
    let minimum_models = if app.has_model_profiles() { 4 } else { 3 };
    let settings_height = if area.height < 6 && !on_model {
        area.height.min(3)
    } else {
        preferred.min(area.height.saturating_sub(minimum_models))
    };
    let [settings, model_rows] = Layout::vertical([
        Constraint::Length(settings_height),
        Constraint::Length(area.height.saturating_sub(settings_height)),
    ])
    .areas(area);
    draw_settings(
        frame,
        app,
        settings,
        app.rows(),
        title,
        focused && !on_model,
    );
    let model_title = if models
        .iter()
        .any(|target| target.kind == ModelTargetKind::PlanningPhase)
    {
        padded(t!(app.tongue, "PHASE MODELS"))
    } else {
        padded(t!(app.tongue, "MODELS"))
    };
    draw_model_rows(
        frame,
        app,
        model_rows,
        &models,
        &model_title,
        focused && on_model,
    );
    if on_model { model_rows } else { settings }
}

/// Derived model targets with their exact assignment or `inherit`.
fn draw_model_rows(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
    targets: &[crate::config::ModelTarget],
    title: &str,
    focused: bool,
) {
    let has_profile = app.has_model_profiles();
    let widest = targets
        .iter()
        .map(|target| app.model_value(target.name).chars().count())
        .chain(has_profile.then(|| app.model_profile_value().chars().count()))
        .max()
        .unwrap_or(0);
    let room = usize::from(area.width.saturating_sub(2));
    let column = room
        .saturating_sub(4 + widest)
        .clamp(6, 26)
        .min((room / 2).max(1));
    let selected_target = app.model_target_at_cursor();
    let rows: Vec<ListItem<'_>> = has_profile
        .then(|| {
            let value = app.model_profile_value();
            let mark = if app.model_profile_changed() {
                "*"
            } else {
                " "
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!(
                    "{mark} {:<column$}",
                    clipped(t!(app.tongue, "Profile"), column.saturating_sub(1)),
                    column = column
                )),
                Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
            ]))
        })
        .into_iter()
        .chain(targets.iter().map(|target| {
            let changed = app.model_changed(target.name);
            let value = app.model_value(target.name);
            let mark = if changed {
                "*"
            } else if app.installed_model_assignment(target.name) != ModelAssignment::Shared(None) {
                "\u{00b7}"
            } else {
                " "
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!(
                    "{mark} {:<column$}",
                    clipped(target.name, column.saturating_sub(1)),
                    column = column
                )),
                Span::styled(
                    value,
                    if changed {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ]))
        }))
        .collect();
    let selected_at = if app.model_profile_at_cursor() {
        Some(0)
    } else {
        selected_target.and_then(|selected| {
            targets
                .iter()
                .position(|target| target.name == selected.name)
                .map(|at| at + usize::from(has_profile))
        })
    };
    let row_count = rows.len();
    let viewport_target = if focused { selected_at } else { None };
    let mut state = ListState::default();
    state.select(viewport_target);
    *state.offset_mut() = model_viewport_offset(area, row_count, viewport_target);
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(title, focused))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
    scrollbar(frame, area, row_count, viewport_target.unwrap_or(0));
}

/// A list of settings, with what each is set to.
fn draw_settings(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
    settings: &[Setting],
    title: &str,
    focused: bool,
) {
    // The label column gives way before the value does. A row with room for
    // only one of the two has to show the answer: the label is the same every
    // run and is spelled out again in the detail panel below, while the value
    // is the thing this screen exists to report. Held to a fixed 26 it took the
    // whole row on a narrow window, and the setup step then listed the settings
    // with nothing beside any of them.
    let widest = settings
        .iter()
        .map(|setting| app.shown_value(*setting).chars().count())
        .max()
        .unwrap_or(0);
    // Inside the border: the cursor's `> `, the changed mark and its space,
    // then the value with the brackets that say the arrows move it.
    let room = usize::from(area.width.saturating_sub(2));
    let column = room
        .saturating_sub(4 + widest + 4)
        .clamp(6, 26)
        // Never more than half the row, however short the values are.
        .min((room / 2).max(1));
    // Where this list starts inside the page's one cursor. The options page
    // draws its own preferences above the settings, so a row's place in
    // `settings` is not its place under the cursor — and this was worked out
    // three times in this function, correctly once. Computed here, used by all
    // three, so a fourth reader cannot get its own answer.
    let cursor = if app.page == Page::Options {
        app.selected.saturating_sub(SCREEN_ROWS.len())
    } else {
        app.selected
    };
    let rows: Vec<ListItem<'_>> = settings
        .iter()
        .enumerate()
        .map(|(index, setting)| {
            // A changed row is marked with a character as well as a colour:
            // a terminal without colour, or an operator who cannot see it,
            // still has to be able to tell which rows they touched.
            let changed = app.changed(*setting);
            let applies = app.applies(*setting);
            let shown = app.shown_value(*setting);
            // Three states, not two. `*` is an answer this session moved and
            // has not written; `·` is one the repository already sets to
            // something other than what the skill ships. Without the second, a
            // rerun cannot tell a customised row from an untouched one.
            let mark = if changed {
                "*"
            } else if shown != setting.default_value() {
                "\u{00b7}"
            } else {
                " "
            };
            // Padded so the values line up, and cut with a mark when the
            // window will not hold the whole label. At the width of the longest
            // label exactly, `Transition authorisation` and its value ran
            // together into one word, so the column is one wider than it needs.
            let mut spans = vec![Span::raw(format!(
                "{mark} {:<column$}",
                // One short of the column, so the padding always leaves a
                // gap: a label cut to the column exactly touched the value and
                // the two read as one word.
                clipped(app.tongue.say(setting.label()), column.saturating_sub(1)),
                column = column
            ))];
            // Brackets on the row under the cursor, and only there: they are
            // what says the arrow keys do something here. Drawn on every row
            // they would be decoration nobody reads.
            // Against the offset cursor, not the raw one. It was `app.selected`,
            // so on the options page the `‹` that says *the arrows move this
            // row* was drawn two rows below the row wearing the highlight —
            // pointing at the setting after the one an arrow key would change.
            // The highlight below had this offset and this did not.
            let here = index == cursor && applies.editable() && app.focus == Focus::List;
            let answers = setting.answers();
            let can_cycle = [-1, 1].into_iter().any(|delta| {
                answers
                    .step(&shown, delta)
                    .is_some_and(|next| next != shown.as_str())
            });
            if here && can_cycle {
                spans.push(Span::styled("\u{2039} ", Style::default().fg(Color::Cyan)));
            }
            spans.push(if applies.editable() {
                Span::styled(
                    shown,
                    if changed {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                )
            } else {
                // A row that decides nothing here is drawn in the muted colour
                // and says so in words beside it. Hiding it would leave an
                // operator looking for a setting they read about and cannot
                // find.
                Span::styled(shown, muted())
            });
            if here && can_cycle {
                spans.push(Span::styled(" \u{203a}", Style::default().fg(Color::Cyan)));
            }
            if app.step == Step::PerAgent
                && let Some(tag) = applies.tag()
            {
                spans.push(Span::styled(format!("  {}", t!(app.tongue, tag)), muted()));
            }
            if app.disagrees(*setting) {
                spans.push(Span::styled(
                    format!("  {}", t!(app.tongue, "(differs by agent)")),
                    muted(),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut state = ListState::default();
    // Unselected when the cursor has moved past this list. Held to the last row
    // instead, the options page drew a cursor here *and* on the preference
    // below, and neither of them was wrong-looking enough to be obviously the
    // stale one.
    // Offset by the rows drawn above this list, which on the options page are
    // the two that say what the page is showing. Without it the highlight sat
    // two rows below the cursor: `Delivery route` was selected and `Worktree
    // location` was the one wearing the mark, so every arrow key moved a cursor
    // the operator could not see and changed a row they had not chosen.
    state.select(focused.then(|| cursor.min(settings.len().saturating_sub(1))));
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(title, focused))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
    // The same offset again: the thumb was placed by the raw cursor, so on the
    // options page it sat two rows below the row it was meant to point at, and
    // reached the bottom before the list did.
    if focused {
        scrollbar(frame, area, settings.len(), cursor);
    }
}

/// Step four: what pressing `s` will do.
///
/// In sentences first, and in commands second. An operator on this step is
/// deciding whether to go ahead, and a list of `estigia config set` lines is an
/// answer to a different question — *how would I do this again* — which only
/// helps once they have already decided.
fn draw_summary(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let chosen = app.chosen();
    let mut lines: Vec<Line<'static>> = Vec::new();
    if chosen.is_empty() {
        lines.push(Line::styled(
            t!(
                app.tongue,
                "No agent is ticked, so there is nothing to install."
            ),
            Style::default().fg(Color::Yellow),
        ));
        lines.push(Line::styled(
            t!(
                app.tongue,
                "Backspace goes back to step 1, where space ticks one."
            ),
            muted(),
        ));
    }

    let gated = chosen
        .iter()
        .filter(|adapter| adapter.can_gate_tools())
        .count();
    if !chosen.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                if chosen.len() == 1 {
                    fill!(app.tongue, "{count} agent", "count" => chosen.len())
                } else {
                    fill!(app.tongue, "{count} agents", "count" => chosen.len())
                },
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                fill!(
                    app.tongue,
                    " — Estigia will gate the tool calls of {gated} of them, and give all {count} the contract and the push guard.",
                    "gated" => gated,
                    "count" => chosen.len()
                ),
                Style::default().fg(Color::White),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    // The rows that follow are a table, and this panel wraps. A row wider than
    // the panel wrapped into the next line and carried the columns with it, so
    // on a sixty-column window the list of agents stopped being a table at all.
    // Each column is therefore cut to the room rather than left to the wrapper.
    const STATUS: usize = "contract only".len();
    let room = usize::from(area.width.saturating_sub(2));
    let names = room
        .saturating_sub(2 + STATUS + 4)
        .clamp(6, 26)
        .min((room / 2).max(1));
    // What is left after the indent, the name and the status. Under a handful
    // of columns there is no room to say anything, and three spaces followed by
    // one letter is not a column.
    let tail = room.saturating_sub(2 + names + STATUS + 3);

    let defaults = Config::default();
    for adapter in &chosen {
        let config = &app.configs[adapter.slug];
        let moved: Vec<&'static crate::config::Setting> = SETTINGS
            .iter()
            .filter(|setting| setting.value_of(config) != setting.value_of(&defaults))
            .collect();
        let mut row = vec![
            Span::styled(
                format!(
                    "  {:<names$}",
                    clipped(adapter.display_name, names.saturating_sub(1)),
                    names = names
                ),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            if adapter.can_gate_tools() {
                Span::styled(
                    format!("{:<STATUS$}", t!(app.tongue, "gated")),
                    Style::default().fg(Color::Green),
                )
            } else {
                Span::styled(
                    t!(app.tongue, "contract only"),
                    Style::default().fg(Color::Yellow),
                )
            },
        ];
        if tail >= 8 {
            let said = if moved.is_empty() {
                t!(app.tongue, "everything at its default").to_owned()
            } else {
                let named = moved
                    .iter()
                    .map(|setting| setting.label())
                    .collect::<Vec<_>>()
                    .join(", ");
                if moved.len() == 1 {
                    fill!(
                        app.tongue,
                        "{count} setting away from the default: {named}",
                        "count" => moved.len(),
                        "named" => named
                    )
                } else {
                    fill!(
                        app.tongue,
                        "{count} settings away from the default: {named}",
                        "count" => moved.len(),
                        "named" => named
                    )
                }
            };
            row.push(Span::styled(
                format!("   {}", clipped(&said, tail)),
                muted(),
            ));
        }
        lines.push(Line::from(row));
    }

    if !chosen.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            t!(app.tongue, "The same without this screen:"),
            Style::default().fg(Color::White),
        ));
        for adapter in &chosen {
            let config = &app.configs[adapter.slug];
            lines.push(Line::styled(
                format!("  estigia setup {}", adapter.slug),
                muted(),
            ));
            for setting in AGENT_SETTINGS {
                let value = setting.value_of(config);
                if value == setting.value_of(&defaults) {
                    continue;
                }
                lines.push(Line::styled(
                    format!(
                        "  estigia config set {:?} {value:?} --agent {}",
                        setting.label(),
                        adapter.slug
                    ),
                    muted(),
                ));
            }
        }
        // A repository-wide row is the same whichever agent asks, so it is one
        // command and not one per agent — and `--agent` refuses it outright.
        // Printed with the flag, the way out of this screen was a refusal.
        let config = &app.configs[chosen[0].slug];
        for setting in OPTIONS_SETTINGS {
            let value = setting.value_of(config);
            if value == setting.value_of(&defaults) {
                continue;
            }
            lines.push(Line::styled(
                format!("  estigia config set {:?} {value:?}", setting.label()),
                muted(),
            ));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(panel(&padded(t!(app.tongue, "WHAT S WILL DO")), true)),
        area,
    );
}

/// The panel under the body: what the thing under the cursor is, and anything
/// the screen has to say about it.
fn draw_detail(
    frame: &mut ratatui::Frame<'_>,
    lines: Vec<Line<'static>>,
    area: ratatui::layout::Rect,
) {
    // An empty bordered box under a step with nothing to say is furniture. The
    // step above it already said everything there was.
    if lines.is_empty() || area.height < 3 {
        return;
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel("", false)),
        area,
    );
}

/// What the panel under the body has to say, if anything.
fn detail_lines(app: &App) -> Vec<Line<'static>> {
    // The options page has rows and no steps, so what it has to say is what the
    // row under the cursor is — whatever the steps behind it were left on.
    let mut lines: Vec<Line<'static>> = if let Some(screen) = app.screen_at_cursor() {
        detail_of_screen(app, screen)
    } else if app.model_profile_at_cursor() {
        detail_of_model_profile(app)
    } else if app.model_target_at_cursor().is_some() {
        detail_of_model_target(app)
    } else if app.page == Page::Options {
        detail_of_setting(app)
    } else {
        match app.step {
            Step::Agents => {
                let (adapter, on) = app.agents[app.agent];
                vec![
                    Line::styled(
                        adapter.display_name.to_owned(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(if on {
                        t!(app.tongue, "chosen — its own settings are on step 2")
                    } else {
                        t!(app.tongue, "not chosen — space ticks it")
                    }),
                    Line::raw(if adapter.can_gate_tools() {
                        t!(
                            app.tongue,
                            "Estigia gates this agent's tool calls: every write goes through the claim"
                        )
                    } else {
                        t!(
                            app.tongue,
                            "Estigia cannot gate this agent's tool calls: it gets the contract and the pre-push guard, and its authorisations are asked for rather than held"
                        )
                    }),
                ]
            }
            Step::Install => Vec::new(),
            Step::PerAgent => detail_of_setting(app),
        }
    };
    // Whatever the screen last had to say, always — including on the steps that
    // otherwise show nothing here. A warning nobody can see is a key that did
    // nothing as far as the operator can tell.
    if let Some(message) = &app.message {
        lines.push(Line::styled(
            message.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    lines
}

fn detail_of_model_profile(app: &App) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            t!(app.tongue, "Model profile").to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            t!(
                app.tongue,
                "a reviewed preset that replaces the complete model route"
            )
            .to_owned(),
            Style::default().fg(Color::White),
        ),
    ])];
    lines.push(Line::styled(
        fill!(app.tongue, "profile: {profile}", "profile" => app.model_profile_value()),
        muted(),
    ));
    lines.push(Line::styled(
        t!(
            app.tongue,
            "custom keeps the current route; edit the targets below to customize it"
        )
        .to_owned(),
        muted(),
    ));
    lines.push(Line::styled(
        t!(
            app.tongue,
            "profiles do not select Planning, prove model availability, or make Estigia run a model"
        )
        .to_owned(),
        muted(),
    ));
    lines
}

/// The same three things, for a preference of this screen.
///
/// Deliberately the same shape as [`detail_of_setting`] — what it is, what it
/// takes, where the answer goes — because it is the same question. What differs
/// is the last line, and that is the difference this page exists to show: the
/// rows above land in agents' files when `s` is pressed, and this one landed
/// when the key was.
fn detail_of_screen(app: &App, screen: Screen) -> Vec<Line<'static>> {
    let here = app.screen_value(screen);
    let mut spans = Vec::new();
    for answer in screen.answers() {
        let on = answer == here;
        spans.push(Span::styled(
            format!(" {}{answer} ", if on { '\u{25cf}' } else { ' ' }),
            if on {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                muted()
            },
        ));
    }
    vec![
        Line::from(vec![
            Span::styled(
                screen.label(app.tongue).to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                screen.about(app.tongue).to_owned(),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(spans),
        Line::styled(screen.lands(app.tongue).to_owned(), muted()),
    ]
}

/// What one derived model target declares and where that declaration lands.
fn detail_of_model_target(app: &App) -> Vec<Line<'static>> {
    let Some(target) = app.model_target_at_cursor() else {
        return Vec::new();
    };
    let about = match target.kind {
        ModelTargetKind::Orchestration => {
            t!(app.tongue, "model declared for orchestration")
        }
        ModelTargetKind::PlanningPhase => {
            t!(app.tongue, "model declared for this planning phase")
        }
        ModelTargetKind::Application => {
            t!(app.tongue, "model declared for applying changes")
        }
        ModelTargetKind::DelegatedAgent => {
            t!(app.tongue, "model declared for this delegated agent")
        }
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(
            target.name.to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(about.to_owned(), Style::default().fg(Color::White)),
    ])];

    if app.focus == Focus::Editing {
        lines.push(Line::styled(
            format!("  {}_", app.draft),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(
            t!(
                app.tongue,
                "accepts: any model ID that fits one key=model entry, optionally followed by /low, /medium, /high, /xhigh or /max; no comma, pipe, or line break; catalogs are advisory"
            )
            .to_owned(),
            muted(),
        ));
        return lines;
    }

    if app.focus == Focus::Picking
        && let Some(note) = app.model_picker_note()
    {
        lines.push(Line::styled(note, muted()));
    }
    lines.push(Line::styled(
        fill!(
            app.tongue,
            "assignment: {model}",
            "model" => app.model_value(target.name)
        ),
        muted(),
    ));

    if app.uniform && app.disagrees(Setting::Planning) {
        lines.push(Line::styled(
            t!(
                app.tongue,
                "Planning differs across selected agents; unify it or edit each agent to route planning phases"
            )
            .to_owned(),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines.push(Line::styled(
        if target.kind == ModelTargetKind::PlanningPhase {
            t!(
                app.tongue,
                "only Claude Code currently emits planned phase definitions; other hosts keep this as a routing declaration"
            )
            .to_owned()
        } else if crate::skill::DELEGATED_AGENTS
            .iter()
            .any(|(key, _)| *key == target.name)
        {
            // Not the sentence below, and the difference is which row
            // decides: this one says what the worker runs on, and
            // `Delegated workers` says whether it exists at all. An operator
            // told only that this is a declaration would set it and find
            // nothing installed.
            t!(
                app.tongue,
                "this reaches a definition only where `Delegated workers` names this worker; that row decides whether it exists, this one what it runs on"
            )
            .to_owned()
        } else {
            fill!(
                app.tongue,
                "{target} is a routing declaration, not proof that a host executes it",
                "target" => target.name
            )
        },
        muted(),
    ));

    let installed = app.installed_model_value(target.name);
    lines.push(Line::styled(
        if app.uniform {
            fill!(
                app.tongue,
                "every selected agent — installed: {installed}",
                "installed" => installed
            )
        } else {
            fill!(
                app.tongue,
                "this agent only ({agent}) — installed: {installed}",
                "agent" => app.current(),
                "installed" => installed
            )
        },
        muted(),
    ));
    lines
}

/// What the row under the cursor is, what it takes, and where its answer goes.
fn detail_of_setting(app: &App) -> Vec<Line<'static>> {
    let Some(setting) = app.setting_at_cursor() else {
        return Vec::new();
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(
            app.tongue.say(setting.label()).to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            t!(app.tongue, setting.about()).to_owned(),
            Style::default().fg(Color::White),
        ),
    ])];
    // Directly under the label, before anything explanatory. Everything else
    // here wraps, so a line further down is a line that disappears on a narrow
    // window — and the one it disappears on is what the operator is typing.
    if app.focus == Focus::Editing {
        lines.push(Line::styled(
            format!("  {}_", app.draft),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::styled(
            fill!(app.tongue, "accepts: {accepted}", "accepted" => t!(app.tongue, setting.accepted())),
            muted(),
        ));
        return lines;
    }

    // The answers, laid out with the one in force marked. This is the same
    // information `accepted()` carries in prose, in the shape somebody can act
    // on — and it means the arrow keys never have to be guessed at.
    lines.push(answers_line(app, setting));

    // What choosing *that one* does. The line above says which words are
    // accepted and the line above that says what the row is for, and between
    // them nothing said what either word means: `standard` and `receipt-driven`
    // were two spellings and an arrow key, on the row that decides what a
    // verdict is bound to.
    //
    // The answer under the **picker's** cursor while one is open, and the one in
    // force otherwise — so arrowing through the list explains each as it is
    // reached, which is where somebody is when they are choosing.
    let looking_at = if app.focus == Focus::Picking {
        app.picker().get(app.pick).cloned()
    } else {
        Some(app.shown_value(setting))
    };
    // Unless there is a warning to show. The panel has the height it has, and
    // this line is two of them on a narrow window — enough to push the message
    // off the bottom, which `detail_lines` appends last for the reason it gives
    // there: *a warning nobody can see is a key that did nothing as far as the
    // operator can tell*. A standing explanation yields to a transient one.
    if let Some(meaning) = looking_at
        .as_deref()
        .filter(|_| app.message.is_none())
        .and_then(|value| setting.means(value))
    {
        lines.push(Line::styled(
            format!("  {}", t!(app.tongue, meaning)),
            Style::default().fg(Color::White),
        ));
    }

    // Where the answer lands, said on the row rather than only in the step's
    // title: the difference between "this agent" and "all of them" is the whole
    // reason the rows are split across two screens, and it is the thing
    // somebody will get wrong.
    let installed = setting.value_of(&app.installed[app.current()]);
    lines.push(Line::styled(
        match setting.scope() {
            Scope::Agent if app.uniform => fill!(
                app.tongue,
                "every agent — installed: {installed}",
                "installed" => installed
            ),
            Scope::Agent => fill!(
                app.tongue,
                "this agent only ({agent}) — installed: {installed}",
                "agent" => app.current(),
                "installed" => installed
            ),
            // The two that are not about an agent are not about the same thing
            // either, and the page cannot say so in its title because it holds
            // both. A tracker belongs to one checkout; the language somebody
            // writes in belongs to them, across every checkout they have.
            Scope::Everywhere => fill!(
                app.tongue,
                "this repository — installed: {installed}",
                "installed" => installed
            ),
            Scope::Machine => fill!(
                app.tongue,
                "this machine, every repository — installed: {installed}",
                "installed" => installed
            ),
        },
        muted(),
    ));
    if app.step == Step::PerAgent
        && let Some(why) = app.applies(setting).because()
    {
        lines.push(Line::styled(
            t!(app.tongue, why).to_owned(),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines
}

/// The answers a setting offers, on one line, with the one in force marked.
///
/// While the picker is open this is the picker: the same answers, one per line,
/// with a cursor. The two are one thing seen at two sizes, so nobody has to
/// learn a second vocabulary to use the open one.
fn answers_line(app: &App, setting: Setting) -> Line<'static> {
    let answers = setting.answers();
    let current = app.shown_value(setting);
    let mut spans = Vec::new();
    for choice in answers.choices {
        let on = *choice == current;
        spans.push(Span::styled(
            format!(" {}{choice} ", if on { '\u{25cf}' } else { ' ' }),
            if on {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                muted()
            },
        ));
    }
    // A value that is not on the list is shown where the list would have shown
    // it — otherwise the line reads as though nothing is set at all. Not only
    // where the vocabulary is open: this used to sit inside `!closed`, so a
    // closed list holding a value nobody added here drew a row with no answer
    // marked, while the picker — which inserts it either way — opened on it.
    // One rule, and the row and the list it opens are the same rule.
    if answers.at(&current).is_none() && !current.is_empty() {
        spans.push(Span::styled(
            format!(" \u{25cf}{current} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    } else if !answers.closed {
        // Somewhere to type, on the rows that have somewhere to type.
        spans.push(Span::styled("  \u{2026}".to_owned(), muted()));
    }
    Line::from(spans)
}

/// The open picker: the answers one per line, hanging off the row they belong
/// to.
///
/// Anchored under the cursor rather than centred, and indented, so it reads as
/// *that row, opened* rather than as a dialog that arrived from somewhere. A
/// list floating in the middle of a panel leaves the row it came from looking
/// untouched, and there is nothing on screen tying the two together.
fn draw_picker(frame: &mut ratatui::Frame<'_>, app: &App, over: ratatui::layout::Rect) {
    let entries = app.picker();
    if entries.is_empty() || over.width < 12 || over.height < 4 {
        return;
    }
    // While the walk is inside a folder, the folder is the title. The row's own
    // name is drawn one line above and one column left of this box, so repeating
    // it here spent the only line that could say **where these names are** —
    // and two levels down, `interno` beside `unset` is a list of words with
    // nothing to place them. `→` walks in and `←` walks up, which the footer
    // says; neither is usable without knowing where you are.
    //
    // Clipped from the left, keeping the tail: the end of a path is what tells
    // two folders apart, and the beginning is the same for all of them.
    let title = match app.browsing.as_deref() {
        Some(here) => {
            let shown = here.display().to_string();
            let room = usize::from(over.width.saturating_sub(10)).max(12);
            if shown.chars().count() > room {
                format!(
                    "…{}",
                    shown
                        .chars()
                        .skip(shown.chars().count().saturating_sub(room))
                        .collect::<String>()
                )
            } else {
                shown
            }
        }
        None => app.picker_title(),
    };
    // Inside the panel's border.
    let inner = ratatui::layout::Rect {
        x: over.x + 1,
        y: over.y + 1,
        width: over.width.saturating_sub(2),
        height: over.height.saturating_sub(2),
    };
    let height = (u16::try_from(entries.len()).unwrap_or(4) + 2).min(inner.height);
    let width = entries
        .iter()
        .map(|entry| entry.chars().count())
        .max()
        .unwrap_or(20)
        .max(title.chars().count() + 2) as u16
        + 6;
    let indent = 4.min(inner.width.saturating_sub(width));
    let width = width.min(inner.width.saturating_sub(indent));

    // Under the row, unless that runs off the bottom — then above it. A list
    // clipped by the panel edge is a list whose last answers do not exist.
    let at_in_panel = match app.screen_at_cursor() {
        Some(screen) => SCREEN_ROWS
            .iter()
            .position(|row| *row == screen)
            .unwrap_or(0),
        None if app.model_profile_at_cursor() || app.model_target_at_cursor().is_some() => {
            let targets = app.model_targets();
            let has_profile = app.has_model_profiles();
            let selected = if app.model_profile_at_cursor() {
                Some(0)
            } else {
                app.model_target_at_cursor().and_then(|selected| {
                    targets
                        .iter()
                        .position(|target| target.name == selected.name)
                        .map(|at| at + usize::from(has_profile))
                })
            };
            selected.unwrap_or(0).saturating_sub(model_viewport_offset(
                over,
                targets.len() + usize::from(has_profile),
                selected,
            ))
        }
        // The options page's screen preferences precede its settings panel;
        // configuration panels already count their own settings from zero.
        None if app.page == Page::Options => app.selected.saturating_sub(SCREEN_ROWS.len()),
        None => app.selected,
    };
    let row = inner.y + u16::try_from(at_in_panel).unwrap_or(0);
    let panel_bottom = over.y.saturating_add(over.height);
    let top = if row >= inner.y && row + 1 + height <= panel_bottom {
        row + 1
    } else if row >= over.y.saturating_add(height) {
        row - height
    } else {
        over.y + (over.height.saturating_sub(height)) / 2
    };

    let area = ratatui::layout::Rect {
        x: inner.x + indent,
        y: top,
        width,
        height,
    };
    frame.render_widget(ratatui::widgets::Clear, area);
    let rows: Vec<ListItem<'_>> = entries
        .iter()
        .map(|entry| ListItem::new(Line::raw(entry.clone())))
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.pick.min(entries.len() - 1)));
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(&padded(&title), true))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
}

/// The keys of the step somebody is on, and only those.
///
/// Built rather than borrowed from a `const`, because a translated hint is a
/// `String` and a slice of `&'static str` cannot hold one. The lists are short
/// and rebuilt once per frame, which is cheaper than the alternative shape:
/// a static table per language, which would be the same list written twice and
/// the second copy is the one that goes stale.
fn keys_for(app: &App) -> Vec<String> {
    match app.focus {
        Focus::Editing => {
            return vec![
                t!(app.tongue, "⏎ apply").to_owned(),
                t!(app.tongue, "Esc cancel").to_owned(),
                t!(app.tongue, "⌫ delete").to_owned(),
            ];
        }
        Focus::Naming => {
            return vec![
                t!(app.tongue, "⏎ make it").to_owned(),
                t!(app.tongue, "Esc cancel").to_owned(),
                t!(app.tongue, "⌫ delete").to_owned(),
            ];
        }
        Focus::Picking => {
            let mut keys = vec![
                t!(app.tongue, "↑↓ move").to_owned(),
                t!(app.tongue, "⏎ / space choose").to_owned(),
            ];
            // Named only where they do something. A walk has keys a list of
            // three words does not, and a footer offering them everywhere would
            // teach a key that answers on one row and nothing on the next.
            if app
                .setting_at_cursor()
                .is_some_and(crate::config::Setting::takes_a_directory)
            {
                keys.push(t!(app.tongue, "→ open").to_owned());
                keys.push(t!(app.tongue, "← up").to_owned());
                keys.push(t!(app.tongue, "n new folder").to_owned());
            }
            keys.push(t!(app.tongue, "Esc back").to_owned());
            return keys;
        }
        Focus::List => {}
    }
    // The same keys a settings step has, minus the two it does not: there are
    // no steps to number and no agents to walk between. `Esc` is named because
    // this page was reached from the menu and going back is what somebody does
    // after changing one row.
    if app.page == Page::Options {
        return vec![
            t!(app.tongue, "←→ change").to_owned(),
            t!(app.tongue, "space all answers").to_owned(),
            t!(app.tongue, "↑↓ move").to_owned(),
            t!(app.tongue, "r restore").to_owned(),
            t!(app.tongue, "Esc menu").to_owned(),
            t!(app.tongue, "s install").to_owned(),
        ];
    }
    // A footer that offered the step keys on a screen with no steps would be
    // two keys that quietly do nothing, which reads as broken.
    if app.pinned() {
        return vec![
            t!(app.tongue, "←→ change").to_owned(),
            t!(app.tongue, "space all answers").to_owned(),
            t!(app.tongue, "↑↓ move").to_owned(),
            t!(app.tongue, "r restore").to_owned(),
            t!(app.tongue, "q quit").to_owned(),
            t!(app.tongue, "s save").to_owned(),
        ];
    }
    // A step with no rows has no row keys. Offering them would be five keys that
    // quietly do nothing, which reads as the screen being broken.
    if app.rows().is_empty() && app.step == Step::PerAgent {
        return vec![
            t!(app.tongue, "backspace back to the agents").to_owned(),
            t!(app.tongue, "enter next step").to_owned(),
            t!(app.tongue, "q quit").to_owned(),
        ];
    }
    // Every one of these ends on the key that finishes, because that is the one
    // `fit_keys` protects. Three of them did not, and the footer then offered a
    // way out and no way to save at forty-four columns — on the page whose
    // whole purpose is the key it dropped.
    match app.step {
        Step::Agents => vec![
            t!(app.tongue, "↑↓ move").to_owned(),
            t!(app.tongue, "space tick").to_owned(),
            t!(app.tongue, "enter next, 1-3 step").to_owned(),
            t!(app.tongue, "q quit").to_owned(),
            t!(app.tongue, "s install").to_owned(),
        ],
        Step::PerAgent => vec![
            t!(app.tongue, "←→ change").to_owned(),
            t!(app.tongue, "space all answers").to_owned(),
            t!(app.tongue, "⇥ who answers").to_owned(),
            t!(app.tongue, "1-3 step").to_owned(),
            t!(app.tongue, "r restore").to_owned(),
            t!(app.tongue, "s install").to_owned(),
        ],
        Step::Install => vec![
            t!(app.tongue, "1-3 step").to_owned(),
            t!(app.tongue, "Esc menu").to_owned(),
            t!(app.tongue, "q quit").to_owned(),
            t!(app.tongue, "⏎ or s install").to_owned(),
        ],
    }
}

#[cfg(test)]
mod tests;
