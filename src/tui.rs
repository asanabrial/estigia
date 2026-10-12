//! The screen: a menu, and the work behind it.

pub mod app;
mod art;
mod models;
pub mod words;

use crate::config::{Config, Group, ModelTargetKind, SETTINGS, Scope, Setting};
use crate::harness::guard::{Removal as Removal_, State};
use crate::outcome::Refusal;
use crate::setup::{Installed, SetupOptions};
use app::{Action, App, Focus, Key, MENU, ModelAssignment, Page, Removal, SCREEN_ROWS, Screen};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap};
use words::{Tongue, fill, t};

/// Runs the screen and returns the configuration to write, if any.
pub fn edit(agent: Option<&str>, installed: Config) -> Result<Option<Config>, Refusal> {
    let adapter = agent.and_then(|slug| {
        crate::setup::AGENTS
            .iter()
            .find(|adapter| adapter.slug == slug)
    });
    let mut app = App::one_table(adapter, installed);
    app.tongue = words::remembered(None);
    Ok(run(app, None)?.map(|(_, configs)| configs.values().next().cloned().unwrap_or_default()))
}

/// The setup screen: a list of agents, each opening onto its own settings.
pub fn setup(
    installed: &std::collections::BTreeMap<&'static str, Config>,
    bare: &std::collections::BTreeMap<&'static str, Config>,
    configured: &[&str],
    install: &mut Installer<'_>,
) -> Result<(), Refusal> {
    // The one place the machine's remembered language is read; the state machine never touches a disk.
    let mut app = App::with_configs(installed, configured);
    // What each agent answers with where no checkout has said anything.
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
/// installs: `guided_install` is still the only place the screen writes
/// through, and it is where the lifecycle gate sits.
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

impl Plan {
    /// The settings whose value in this agent's table differs from what the screen opened on.
    pub(crate) fn changed_settings(&self, slug: &'static str) -> Vec<Setting> {
        SETTINGS
            .iter()
            .copied()
            .filter(|setting| {
                self.opened.get(slug).is_none_or(|opened| {
                    setting.value_of(opened) != setting.value_of(&self.rows[slug])
                })
            })
            .collect()
    }

    /// The agents a save of this plan runs setup for: ticked and not yet installed,
    /// or with a setting that differs from what the screen opened on.
    pub(crate) fn to_touch(
        &self,
        is_configured: impl Fn(&'static crate::setup::AgentAdapter) -> bool,
    ) -> Vec<&'static crate::setup::AgentAdapter> {
        self.agents
            .iter()
            .copied()
            .filter(|adapter| {
                !is_configured(adapter) || !self.changed_settings(adapter.slug).is_empty()
            })
            .collect()
    }
}

/// Runs one install attempt and keeps the shell-visible refusal in step with it.
fn install_from_screen(app: &mut App, install: &mut Installer<'_>, refused: &mut Option<Refusal>) {
    let plan = app.plan();
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
    // A refusal is shown in the box *and* kept: it is both a message and an exit code.
    let mut refused: Option<Refusal> = None;
    let mut guard_refused: Option<Refusal> = None;
    let mut removal_refused: Option<Refusal> = None;
    let mut remote_read_for = String::new();
    let mut reads = app::Reads::default();
    let (finished, answers) = std::sync::mpsc::channel::<Finished>();
    let outcome = loop {
        if app.repository.is_empty() {
            app.repository = repo_dir().display().to_string();
        }
        if remote_read_for != app.repository {
            app.remote = github_remote_of(std::path::Path::new(&app.repository));
            remote_read_for.clone_from(&app.repository);
            refresh_guard(&mut app);
        }
        if let Err(error) = terminal.draw(|frame| draw(frame, &mut app)) {
            break Err(io_refusal("draw to the terminal", &error));
        }
        if app.is_busy() {
            if let Ok(done) = answers.try_recv() {
                deliver(&mut app, done);
                continue;
            }
            match crossterm::event::poll(BUSY_TICK) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => break Err(io_refusal("read a key", &error)),
            }
        }
        let event = match crossterm::event::read() {
            Ok(event) => event,
            Err(error) => break Err(io_refusal("read a key", &error)),
        };
        let Some(pressed) = key_of(&event) else {
            continue;
        };
        reads.refresh(&mut app, &mut Disk);
        let action = app.press(pressed);
        reads.performed(&action);
        match action {
            Action::None => {}
            // With an installer the write happens here; without one the decision goes back to the caller.
            Action::Save => {
                let decided = (app.chosen(), app.configs.clone());
                let Some(install) = install.as_mut() else {
                    break Ok(Some(decided));
                };
                install_from_screen(&mut app, *install, &mut refused);
            }
            Action::Guard => {
                let (text, refusal) =
                    install_guard(std::path::Path::new(&app.repository), app.tongue);
                app.report = Some(text);
                guard_refused = refusal;
                refresh_guard(&mut app);
            }
            Action::PreviewUninstall(target) => preview_removal(&mut app, target),
            Action::Uninstall(target) => {
                let refusal = remove(&mut app, target);
                removal_refused = refusal.or(removal_refused);
                refresh_guard(&mut app);
            }
            Action::Remember => {
                if let Err(refusal) = words::remember(None, app.tongue) {
                    app.install_failed(
                        format!("{} ({})", refusal.message, refusal.code),
                        refusal.resolution.to_string(),
                    );
                }
            }
            Action::Doctor => {
                let repository = std::path::PathBuf::from(&app.repository);
                let sender = finished.clone();
                spawn_worker(sender, move || {
                    let root = crate::harness::discover_skill_root().ok();
                    Finished::Doctor(report_of_checks(
                        root.as_deref(),
                        &repository,
                        &crate::setup::SetupOptions::default(),
                    ))
                });
            }
            Action::Reload(chosen) => match reload_repository(&mut app, &chosen) {
                Ok(()) => {
                    app.repository = chosen;
                }
                Err(refusal) => app.install_failed(
                    format!("{} ({})", refusal.message, refusal.code),
                    refusal.resolution.to_string(),
                ),
            },
            // Made here: this state machine has no filesystem in it.
            Action::MakeFolder(path) => {
                make_folder(&mut app, path);
            }
            Action::LoadModelCatalog(adapter) => {
                let sender = finished.clone();
                spawn_worker(sender, move || {
                    let loaded = models::load(adapter).map_err(|error| error.to_string());
                    Finished::Catalog(adapter, loaded)
                });
            }
            Action::Quit => break Ok(None),
        }
    };
    ratatui::restore();
    // The refusal outlives the box it was shown in: whoever ran the command is
    // still waiting for an exit code.
    exit_of(refused, guard_refused.or(removal_refused), outcome)
}

/// How long the screen waits for a key before it looks for the worker's answer.
const BUSY_TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// What a worker off the event thread hands back.
pub(super) enum Finished {
    Doctor(String),
    Catalog(
        &'static crate::setup::AgentAdapter,
        Result<Vec<String>, String>,
    ),
    Died,
}

pub(super) fn spawn_worker(
    sender: std::sync::mpsc::Sender<Finished>,
    work: impl FnOnce() -> Finished + Send + 'static,
) {
    std::thread::spawn(move || {
        let done =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or(Finished::Died);
        let _ = sender.send(done);
    });
}

pub(super) fn deliver(app: &mut App, done: Finished) {
    match done {
        Finished::Doctor(text) => app.doctor_finished(text),
        Finished::Died => app.worker_died(),
        Finished::Catalog(adapter, Ok(models)) => app.model_catalog_loaded(adapter, models),
        Finished::Catalog(adapter, Err(error)) => {
            app.model_catalog_failed(adapter);
            app.message = Some(fill!(
                app.tongue,
                "{agent} model catalog unavailable: {why}. Type a model ID instead.",
                "agent" => adapter.display_name,
                "why" => error
            ));
        }
    }
}

fn exit_of(
    install: Option<Refusal>,
    guard: Option<Refusal>,
    outcome: Result<Option<Decided>, Refusal>,
) -> Result<Option<Decided>, Refusal> {
    match install.or(guard) {
        Some(refusal) => Err(refusal),
        None => outcome,
    }
}

/// Says what removing this would take, or why there is nothing to take.
fn preview_removal(app: &mut App, target: Removal) {
    let tongue = app.tongue;
    let repo = std::path::PathBuf::from(&app.repository);
    match target {
        Removal::Agent(slug) => {
            let Some(adapter) = crate::setup::AGENTS
                .iter()
                .find(|adapter| adapter.slug == slug)
            else {
                return;
            };
            let options = SetupOptions {
                dry_run: true,
                ..SetupOptions::default()
            };
            let (done, refused, _) = crate::cli::uninstall_agents(&[adapter], &options);
            let lines = done
                .iter()
                .flat_map(|(_, result)| result.actions.iter())
                .filter(|action| removes(&action.change))
                .map(|action| action.path.display().to_string())
                .collect::<Vec<_>>();
            match (refused.first(), lines.is_empty()) {
                (Some(refusal), _) => app.message = Some(format!("{refusal}")),
                (None, true) => {
                    app.message = Some(fill!(
                        tongue,
                        "nothing of Estigia's to remove from {agent}",
                        "agent" => slug
                    ));
                }
                (None, false) => app.removal_asked(target, lines),
            }
        }
        Removal::Guard => match crate::harness::guard::uninstall(&repo, true) {
            Ok(Removal_::WouldBeTaken) => {
                let path = crate::harness::guard::hooks_directory(&repo)
                    .map(|hooks| hooks.join(crate::harness::guard::HOOK));
                match path {
                    Ok(path) => app.removal_asked(target, vec![path.display().to_string()]),
                    Err(refusal) => app.message = Some(format!("{refusal}")),
                }
            }
            Ok(other) => app.report = Some(guard_removal_text(tongue, &repo, &other)),
            Err(refusal) => app.message = Some(format!("{refusal}")),
        },
    }
}

fn refresh_guard(app: &mut App) {
    app.guard_state = crate::harness::guard::state(std::path::Path::new(&app.repository));
}

/// Whether an uninstall of this file would take something away.
fn removes(change: &crate::skill::Change) -> bool {
    use crate::skill::Change::{Create, Overwrite, Remove, Replace, Update};
    matches!(change, Create | Update | Replace | Overwrite | Remove)
}

/// Removes what was confirmed and reads the answer back; the refusal, if any, is the exit code.
fn remove(app: &mut App, target: Removal) -> Option<Refusal> {
    let tongue = app.tongue;
    let (text, refusal) = match target {
        Removal::Agent(slug) => {
            let adapter = crate::setup::AGENTS
                .iter()
                .find(|adapter| adapter.slug == slug)?;
            let options = SetupOptions::default();
            let (done, refused, _) = crate::cli::uninstall_agents(&[adapter], &options);
            let refusal = crate::cli::refusal_over(&done, refused);
            let read = crate::setup::read_configured(adapter, &options);
            let changed = done.iter().map(|(_, r)| r.changed_files()).sum();
            agent_removed(app, slug, changed, &read, refusal)
        }
        Removal::Guard => {
            let repo = std::path::PathBuf::from(&app.repository);
            let removal = crate::harness::guard::uninstall(&repo, false);
            let after = crate::harness::guard::state(&repo);
            guard_removal_outcome(tongue, &repo, removal, &after)
        }
    };
    match refusal {
        Some(refusal) => {
            app.install_failed(text, refusal.resolution.to_string());
            Some(refusal)
        }
        None => {
            app.message = Some(text);
            None
        }
    }
}

/// Takes the agent's listing from the read-back, and leaves it alone when nothing was read.
fn agent_removed(
    app: &mut App,
    slug: &'static str,
    changed: usize,
    read: &Installed,
    refusal: Option<Refusal>,
) -> (String, Option<Refusal>) {
    match read {
        Installed::Configured => app.agent_read_back(slug, true),
        Installed::Absent => app.agent_read_back(slug, false),
        Installed::Unknown(_) => {}
    }
    agent_removal_text(app.tongue, slug, changed, read, refusal)
}

/// What an agent's uninstall says, from the read-back and never from the return alone.
fn agent_removal_text(
    tongue: Tongue,
    slug: &str,
    changed: usize,
    read: &Installed,
    refusal: Option<Refusal>,
) -> (String, Option<Refusal>) {
    match (refusal, read) {
        (Some(refusal), _) => {
            let state = match read {
                Installed::Configured => t!(tongue, "read back: still installed").to_owned(),
                Installed::Absent => t!(
                    tongue,
                    "read back: its block is gone, but the uninstall did not finish"
                )
                .to_owned(),
                Installed::Unknown(reason) => {
                    fill!(tongue, "read back: could not be read ({reason})", "reason" => reason)
                }
            };
            (
                format!("{} ({}) — {state}", refusal.message, refusal.code),
                Some(refusal),
            )
        }
        (None, Installed::Unknown(reason)) => {
            let refusal = uninstall_read_back_unreadable(slug, reason);
            (
                format!("{} ({})", refusal.message, refusal.code),
                Some(refusal),
            )
        }
        (None, Installed::Configured) => {
            let refusal = uninstall_not_read_back(slug);
            (
                format!("{} ({})", refusal.message, refusal.code),
                Some(refusal),
            )
        }
        (None, Installed::Absent) => (
            fill!(
                tongue,
                "{agent} uninstalled: {count} file(s) changed, read back as not installed",
                "agent" => slug,
                "count" => changed
            ),
            None,
        ),
    }
}

fn guard_removal_text(tongue: Tongue, repo: &std::path::Path, removal: &Removal_) -> String {
    let at = repo.display();
    match removal {
        Removal_::Taken => fill!(tongue, "push guard removed from {where}", "where" => at),
        Removal_::WouldBeTaken => {
            fill!(tongue, "push guard would be removed from {where}", "where" => at)
        }
        Removal_::NothingThere => {
            fill!(tongue, "no push guard in {where} — nothing to remove", "where" => at)
        }
        Removal_::LeftAlone(_) => fill!(
            tongue,
            "the hook in {where} is not only Estigia's and is left alone — nothing was removed",
            "where" => at
        ),
    }
}

/// What a guard uninstall says, and the refusal when the read-back disagrees with it.
fn guard_removal_outcome(
    tongue: Tongue,
    repo: &std::path::Path,
    removal: Result<Removal_, Refusal>,
    after: &State,
) -> (String, Option<Refusal>) {
    match removal {
        Err(refusal) => (format!("{refusal}"), Some(refusal)),
        Ok(Removal_::Taken) if *after != State::Absent => {
            let refusal = guard_removal_not_read_back(repo, after);
            (format!("{refusal}"), Some(refusal))
        }
        Ok(Removal_::LeftAlone(found)) => {
            let refusal = guard_removal_left_alone(repo, &found);
            (format!("{refusal}"), Some(refusal))
        }
        Ok(other) => (guard_removal_text(tongue, repo, &other), None),
    }
}

/// Makes the folder the operator named and lists it.
fn make_folder(app: &mut App, path: std::path::PathBuf) {
    match std::fs::create_dir_all(&path) {
        Ok(()) => {
            app.message = Some(fill!(
                app.tongue,
                "created folder {path} on disk now; this is not held back until s",
                "path" => path.display()
            ));
            app.browsing = Some(path);
            app.pick = 0;
        }
        Err(error) => {
            app.message = Some(format!("{}: {error}", path.display()));
        }
    }
}

/// Installs the pre-push guard, and says what happened either way.
fn install_guard(repo: &std::path::Path, tongue: Tongue) -> (String, Option<Refusal>) {
    let repo = repo.to_path_buf();
    let Ok(executable) = std::env::current_exe() else {
        let message = t!(
            tongue,
            "could not work out where this executable is, so the hook would name nothing"
        );
        let refusal = Refusal::not_started(
            "executable-not-resolvable",
            message,
            crate::outcome::Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a resolvable path to the Estigia executable",
            ),
        );
        return (message.to_owned(), Some(refusal));
    };
    let executable = crate::paths::remove_windows_verbatim_prefix(
        executable.canonicalize().unwrap_or(executable),
    );
    let refuse = |refusal: Refusal| (format!("{refusal}"), Some(refusal));
    if let Err(refusal) = crate::harness::guard::install(&repo, &executable, false) {
        return refuse(refusal);
    }
    match crate::harness::guard::state(&repo) {
        State::Installed => (
            fill!(
                tongue,
                "push guard installed in {where}, and read back as installed",
                "where" => repo.display()
            ),
            None,
        ),
        State::Chained => (
            fill!(
                tongue,
                "push guard chained in {where}: a hook of yours already hands the push to Estigia, so nothing was written",
                "where" => repo.display()
            ),
            None,
        ),
        other => refuse(guard_not_read_back(&repo, &other)),
    }
}

/// An uninstall returned, and the agent still reads back as installed.
pub(crate) fn uninstall_not_read_back(slug: &str) -> Refusal {
    Refusal {
        code: "uninstall-not-read-back",
        message: format!("{slug} was uninstalled and still reads back as installed"),
        outcome: crate::outcome::MutationOutcome::Unknown,
        class: crate::harness::refusal_class::class_of_code("uninstall-not-read-back"),
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: crate::outcome::Resolution::run("estigia status"),
        observed: None,
    }
}

/// The agent was uninstalled and its instruction file could not be read to say whether it still holds the directive.
pub(crate) fn uninstall_read_back_unreadable(slug: &str, reason: &str) -> Refusal {
    Refusal {
        message: format!(
            "{slug} was uninstalled and the read-back could not read its instruction file: {reason}"
        ),
        ..uninstall_not_read_back(slug)
    }
}

/// The hook was removed and does not read back as absent.
pub(crate) fn guard_removal_not_read_back(repo: &std::path::Path, found: &State) -> Refusal {
    Refusal {
        code: "push-guard-removal-not-read-back",
        message: format!(
            "the push guard was removed from {} and reads back as {found:?}, not as absent",
            repo.display()
        ),
        outcome: crate::outcome::MutationOutcome::Unknown,
        class: crate::harness::refusal_class::class_of_code("push-guard-removal-not-read-back"),
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: crate::outcome::Resolution::run("estigia doctor"),
        observed: None,
    }
}

/// The hook is not one Estigia wrote, so the uninstall left it and removed nothing.
pub(crate) fn guard_removal_left_alone(repo: &std::path::Path, found: &State) -> Refusal {
    Refusal::not_started(
        "push-guard-left-alone",
        format!(
            "the push guard in {} was left alone and nothing was removed: the hook reads back as {found:?}",
            repo.display()
        ),
        crate::outcome::Resolution::run("estigia doctor"),
    )
}

/// The hook was written and does not read back as installed.
pub(crate) fn guard_not_read_back(repo: &std::path::Path, found: &State) -> Refusal {
    Refusal {
        code: "push-guard-not-read-back",
        message: format!(
            "the push guard was written in {} and reads back as {found:?}, not as installed",
            repo.display()
        ),
        outcome: crate::outcome::MutationOutcome::Unknown,
        class: crate::harness::refusal_class::class_of_code("push-guard-not-read-back"),
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: crate::outcome::Resolution::run("estigia doctor"),
        observed: None,
    }
}

/// Another checkout was chosen while this one's rows carry answers nobody installed.
pub(crate) fn repository_rows_unsaved() -> Refusal {
    Refusal::not_started(
        "repository-rows-unsaved",
        "this checkout's rows carry answers that are not installed, and showing another \
         checkout would replace them",
        crate::outcome::Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "install them with s, or put each row back with r, then choose the repository again",
        ),
    )
}

/// The frame around a panel, coloured by whether it has the keys.
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
fn bordered(title: &str, colour: Color) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colour))
        .title(Span::styled(title.to_owned(), Style::default().fg(colour)))
}

/// A panel title, with the breathing room every one of them has.
fn padded(title: &str) -> String {
    format!(" {title} ")
}

/// The selected row, in the one style this screen uses for it.
fn selected() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
}

/// A bar down the right edge of a panel whose list is taller than it.
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

/// The columns a text occupies on screen.
fn display_width(text: &str) -> usize {
    Line::raw(text).width()
}

/// The text padded with spaces to `width` columns.
fn pad_to(text: &str, width: usize) -> String {
    let gap = width.saturating_sub(display_width(text));
    format!("{text}{}", " ".repeat(gap))
}

/// The longest start of `text` that fits `columns`.
fn head(text: &str, columns: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|c| {
            used += display_width(c.encode_utf8(&mut [0; 4]));
            used <= columns
        })
        .collect()
}

/// Text cut to a width, with a mark saying it was.
fn clipped(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    // Under two columns there is no room for a character *and* the mark, so the mark goes.
    if width < 2 {
        return head(text, width);
    }
    let mut out = head(text, width - 1);
    out.push('\u{2026}');
    out
}

/// The longest end of `text` that fits `columns`.
fn tail(text: &str, columns: usize) -> String {
    let mut used = 0;
    let mut kept: Vec<char> = text
        .chars()
        .rev()
        .take_while(|c| {
            used += display_width(c.encode_utf8(&mut [0; 4]));
            used <= columns
        })
        .collect();
    kept.reverse();
    kept.into_iter().collect()
}

/// The keys, as many of them as the window holds.
fn fit_keys<Hint: AsRef<str>>(hints: &[Hint], width: u16) -> String {
    const GAP: &str = "   ";
    let width = usize::from(width);
    let mut kept: Vec<&str> = hints.iter().map(AsRef::as_ref).collect();
    while kept.len() > 1 && display_width(&kept.join(GAP)) > width {
        // From the middle, except at two — where the middle *is* the last; the first goes instead.
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

/// The checks on one repository, rendered for the page that shows them.
fn report_of_checks(
    root: Option<&std::path::Path>,
    repo: &std::path::Path,
    options: &crate::setup::SetupOptions,
) -> String {
    let tracker = crate::harness::doctor::tracker_in_force(root, repo);
    render_checks(&crate::harness::doctor::full(root, repo, &tracker, options))
}

fn render_checks(checks: &[crate::harness::doctor::Check]) -> String {
    checks
        .iter()
        .flat_map(crate::harness::doctor::Check::report_lines)
        .collect::<Vec<_>>()
        .join("\n")
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

/// `github <owner>/<name>` for the checkout's `origin` — or its first remote — when that is on GitHub.
fn github_remote_of(checkout: &std::path::Path) -> Option<String> {
    let git = |args: &[&str]| -> Option<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    };
    let remotes = git(&["remote"])?;
    let name = remotes
        .lines()
        .find(|name| *name == "origin")
        .or_else(|| remotes.lines().next())?;
    let url = git(&["remote", "get-url", name])?;
    let repo = crate::config::RepoRef::from_github_remote(&url)?;
    Some(format!("github {}/{}", repo.owner, repo.name))
}

/// The machine itself, as the page reads it.
struct Disk;

impl app::World for Disk {
    fn repositories(&mut self) -> Vec<String> {
        known_repositories_for()
    }

    fn folders(&mut self, root: &std::path::Path) -> Vec<String> {
        folders_under(root)
    }

    fn drives(&mut self) -> Vec<String> {
        drives_here()
    }
}

/// The checkouts this page may be pointed at.
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
fn reload_repository(app: &mut App, chosen: &str) -> Result<(), Refusal> {
    reload_repository_with(app, chosen, crate::skill::layer_repository)
}

/// The layering, as a seam.
type Layering = fn(&Config, &std::path::Path) -> Result<Config, Refusal>;

fn reload_repository_with(app: &mut App, chosen: &str, layer: Layering) -> Result<(), Refusal> {
    let chosen = std::path::Path::new(chosen);
    let shared = || {
        SETTINGS
            .iter()
            .filter(|setting| matches!(setting.scope(), Scope::Everywhere | Scope::Repository))
    };
    let carrying = shared().any(|setting| {
        app.configs.iter().any(|(slug, config)| {
            app.installed
                .get(slug)
                .is_some_and(|opened| setting.value_of(config) != setting.value_of(opened))
        })
    });
    if carrying {
        return Err(repository_rows_unsaved());
    }
    // Built beside the page's tables and swapped in only once every agent has been read:
    // a refusal mid-way must not leave the page naming one checkout and showing another's rows.
    let mut next = app.configs.clone();
    let mut opened = app.installed.clone();
    for (slug, config) in next.iter_mut() {
        // What that agent answers with where no checkout has said anything.
        let Some(base) = app.bare.get(slug) else {
            continue;
        };
        let theirs = layer(base, chosen)?;
        for setting in shared() {
            let value = setting.value_of(&theirs);
            setting.apply(config, &value)?;
            if let Some(table) = opened.get_mut(slug) {
                setting.apply(table, &value)?;
            }
        }
    }
    app.configs = next;
    app.installed = opened;
    Ok(())
}

/// The roots a walk can reach that are not under this one.
fn drives_here() -> Vec<String> {
    if !cfg!(windows) {
        return Vec::new();
    }
    (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|drive| std::path::Path::new(drive).is_dir())
        .collect()
}

/// The directories directly under one, named rather than pathed.
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
/// Pure and fed: the interesting inputs cannot be produced by a test that drives the
/// state machine, so every test presses keys directly. Windows reports a key twice;
/// acting on both types types every character twice and moves the cursor two rows per arrow.
fn key_of(event: &crossterm::event::Event) -> Option<Key> {
    let crossterm::event::Event::Key(key) = event else {
        return None;
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return None;
    }
    Some(translate(key.code, key.modifiers))
}

fn translate(code: crossterm::event::KeyCode, modifiers: crossterm::event::KeyModifiers) -> Key {
    use crossterm::event::{KeyCode, KeyModifiers};
    if modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT) {
        return match code {
            KeyCode::Char('c' | 'C') => Key::Interrupt,
            _ => Key::Other,
        };
    }
    match code {
        KeyCode::Char(character) => Key::Char(character),
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
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
            &if app.is_busy() {
                vec![t!(app.tongue, "q quit")]
            } else {
                vec![t!(app.tongue, "any key returns"), t!(app.tongue, "q quit")]
            },
        ),
        Page::Help => draw_text(
            frame,
            t!(app.tongue, "Help"),
            app,
            &[t!(app.tongue, "any key returns"), t!(app.tongue, "q quit")],
        ),
        Page::Options => draw_options(frame, app),
        Page::Agents => draw_agents(frame, app),
        Page::Detail => draw_detail_page(frame, app),
    }
    if app.reviewing {
        draw_review(frame, app);
    }
    if let Some((target, lines)) = &app.removing {
        draw_removal(frame, app.tongue, *target, lines);
    }
    // Last, and over everything: a verdict about what just happened has to be
    // read before the screen it happened on.
    if let Some(modal) = &app.modal {
        draw_modal(frame, modal, app.tongue);
    }
}

/// The verdict box: centred, over whatever is behind it.
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
    draw_box(frame, lines, title, colour);
}

/// What a save would write, over the page it was asked from.
fn draw_review(frame: &mut ratatui::Frame<'_>, app: &App) {
    let tongue = app.tongue;
    let review = app.review();
    let mut lines = vec![Line::from("")];
    for adapter in &review.installs {
        lines.push(Line::styled(
            format!(
                "  {}",
                fill!(tongue, "install {agent}", "agent" => adapter.slug)
            ),
            Style::default().fg(Color::Cyan),
        ));
    }
    for (adapter, settings) in &review.changes {
        let agent = match app.purpose {
            app::Purpose::OneTable(None) => t!(tongue, "the shared contract"),
            _ => adapter.slug,
        };
        let named = settings
            .iter()
            .map(|setting| tongue.say(setting.label()))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(Line::raw(format!(
            "  {}",
            fill!(tongue, "changed in {agent}: {named}", "agent" => agent, "named" => named)
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        format!("  {}", t!(tongue, "nothing is written until you confirm")),
        muted(),
    ));
    lines.push(Line::styled(
        format!(
            "  {}",
            fit_keys(
                &[t!(tongue, "⏎ write it"), t!(tongue, "Esc cancel")],
                frame.area().width.saturating_sub(12)
            )
        ),
        muted(),
    ));
    draw_box(frame, lines, t!(tongue, "Review"), Color::Cyan);
}

/// What a removal would take, over the page it was asked from.
fn draw_removal(frame: &mut ratatui::Frame<'_>, tongue: Tongue, target: Removal, paths: &[String]) {
    let what = match target {
        Removal::Agent(slug) => slug,
        Removal::Guard => t!(tongue, "the push guard"),
    };
    let mut lines = vec![Line::from("")];
    lines.push(Line::styled(
        format!("  {}", fill!(tongue, "uninstall {what}", "what" => what)),
        Style::default().fg(Color::Yellow),
    ));
    lines.extend(paths.iter().map(|path| Line::raw(format!("    {path}"))));
    lines.push(Line::from(""));
    lines.push(Line::styled(
        format!("  {}", t!(tongue, "nothing is removed until you confirm")),
        muted(),
    ));
    lines.push(Line::styled(
        format!(
            "  {}",
            fit_keys(
                &[t!(tongue, "⏎ remove it"), t!(tongue, "Esc cancel")],
                frame.area().width.saturating_sub(12)
            )
        ),
        muted(),
    ));
    draw_box(frame, lines, t!(tongue, "Uninstall"), Color::Yellow);
}

/// The rows one line takes at `room` columns under a word wrap that keeps its spaces: a word that no
/// longer fits starts the next row, and one wider than a row is broken across rows.
fn wrapped_rows(line: &Line<'_>, room: usize) -> usize {
    let room = room.max(1);
    let text: String = line.iter().map(|span| span.content.as_ref()).collect();
    let mut rows = 1;
    let mut column = 0;
    let mut rest = text.as_str();
    while let Some(first) = rest.chars().next() {
        let space = first.is_whitespace();
        let end = rest
            .find(|c: char| c.is_whitespace() != space)
            .unwrap_or(rest.len());
        let (token, tail) = rest.split_at(end);
        rest = tail;
        let width = Line::raw(token).width();
        if space {
            column = (column + width).min(room);
        } else if column + width <= room {
            column += width;
        } else if width <= room {
            rows += 1;
            column = width;
        } else {
            let filled = column + width - 1;
            rows += filled / room;
            column = filled % room + 1;
        }
    }
    rows
}

/// A bordered box, centred over whatever is behind it.
fn draw_box(frame: &mut ratatui::Frame<'_>, lines: Vec<Line<'static>>, title: &str, colour: Color) {
    let area = frame.area();
    // Room for the border, the blank rows and whatever the detail wraps to.
    let width = area.width.saturating_sub(8).clamp(1, 72);
    let room = usize::from(width.saturating_sub(4)).max(1);
    let wrapped: usize = lines.iter().map(|line| wrapped_rows(line, room)).sum();
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

/// The landing screen: Tantan or the wordmark beside the menu, centred; the menu alone when neither fits.
fn draw_menu(frame: &mut ratatui::Frame<'_>, app: &App) {
    let [header, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(4)]).areas(frame.area());
    draw_header(frame, app, header);

    // A heading above and the version below: both go in the stacked arrangement, where every row is contested.
    let mut menu = vec![
        Line::styled(format!("  {}", t!(app.tongue, "ACTIONS")), muted()),
        Line::raw(""),
    ];
    menu.extend(menu_entries(app));
    menu.push(Line::raw(""));
    menu.push(Line::styled(
        format!("  estigia {}", crate::build_info::VERSION),
        muted(),
    ));
    let menu_width = width_of(&menu);
    let menu_height = u16::try_from(menu.len()).unwrap_or(u16::MAX);

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
    if let Some(message) = &app.message {
        foot.push(Line::styled(
            format!("  {message}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let mut home_keys = vec![
        t!(app.tongue, "up/down move"),
        t!(app.tongue, "enter choose"),
    ];
    if app.guard_removable() && app.entry().goes == app::Goes::Doing(app::Deed::Guard) {
        home_keys.push(t!(app.tongue, "u uninstall"));
    }
    home_keys.extend([t!(app.tongue, "? help"), t!(app.tongue, "q quit")]);
    foot.push(Line::styled(
        format!("  {}", fit_keys(&home_keys, body.width.saturating_sub(2))),
        muted(),
    ));
    let foot_height = u16::try_from(foot.len()).unwrap_or(3).min(body.height);

    // Wide enough that the mark and the menu read as two things.
    const GAP: u16 = 10;
    const AIR: u16 = 4;
    let cat_width = art::width(art::TANTAN);
    let cat_height = art::height(art::TANTAN);
    let stage_height = body.height.saturating_sub(foot_height);
    let cat_fits = body.width >= cat_width + GAP + menu_width && stage_height >= cat_height + AIR;
    let (mark, stage_height): (Vec<Line<'static>>, u16) = if cat_fits {
        let rows = art::TANTAN.len();
        let lines = art::TANTAN
            .iter()
            .enumerate()
            .map(|(row, text)| {
                Line::styled(
                    (*text).to_owned(),
                    Style::default().fg(art::band(row, rows)),
                )
            })
            .collect();
        (lines, stage_height)
    } else {
        let lines = WORDMARK
            .lines()
            .map(|row| Line::styled(row.to_owned(), Style::default().fg(Color::Cyan)))
            .collect();
        (lines, body.height)
    };
    let mark_width = width_of(&mark);
    let mark_height = u16::try_from(mark.len()).unwrap_or(u16::MAX);
    let paired = mark_width + GAP + menu_width;
    let (mark_at, menu_at) = if body.width >= paired && stage_height >= mark_height + AIR {
        let left = body.x + (body.width - paired) / 2;
        let top = body.y + (stage_height.saturating_sub(mark_height)) / 2;
        (
            Some(ratatui::layout::Rect {
                x: left,
                y: top,
                width: mark_width,
                height: mark_height,
            }),
            // Centred against the mark rather than against the screen.
            ratatui::layout::Rect {
                x: left + mark_width + GAP,
                y: top + mark_height.saturating_sub(menu_height) / 2,
                width: menu_width,
                height: menu_height.min(body.height),
            },
        )
    } else {
        // No room beside it: the entries are why the screen exists, so the mark goes.
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

    let height = foot_height;
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

/// The mark for terminals too small for Tantan: a wordmark stands in for the drawing.
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
                    .map(|span| display_width(&span.content))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0),
    )
    .unwrap_or(u16::MAX)
}

/// One row per menu entry, with the cursor on the selected one.
fn menu_entries(app: &App) -> Vec<Line<'static>> {
    let width = MENU
        .iter()
        .map(|entry| display_width(t!(app.tongue, entry.label)))
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
                // Dimmer than the cursor line by a whole step.
                Style::default().fg(Color::Gray)
            };
            let mut spans = vec![Span::styled(
                format!("{marker} {}", pad_to(t!(app.tongue, entry.label), width)),
                style,
            )];
            // An answer was given and not yet written.
            let touched = match entry.goes {
                app::Goes::To(Page::Agents) => app.agents_touched(),
                app::Goes::To(Page::Options) => app.options_touched(),
                _ => false,
            };
            if touched {
                spans.push(Span::styled(
                    " \u{2022}",
                    Style::default().fg(Color::Yellow),
                ));
            }
            let flaw = match (&entry.goes, &app.guard_state) {
                (app::Goes::Doing(app::Deed::Guard), State::Inert) => Some(t!(app.tongue, "inert")),
                (app::Goes::Doing(app::Deed::Guard), State::Unreadable) => {
                    Some(t!(app.tongue, "unreadable"))
                }
                _ => None,
            };
            if let Some(flaw) = flaw {
                spans.push(Span::styled(
                    format!(" ({flaw})"),
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
    // On a screen handed its table, the count is of a choice nobody made.
    let context = match app.purpose {
        app::Purpose::OneTable(Some(adapter)) => {
            fill!(app.tongue, "editing {agent}'s own table", "agent" => adapter.slug)
        }
        app::Purpose::OneTable(None) => t!(app.tongue, "editing the shared contract").to_owned(),
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
                    Page::Detail if app.pinned() => t!(app.tongue, "Edit"),
                    Page::Agents | Page::Detail => t!(app.tongue, "Agents"),
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
    let room = usize::from(body.width.saturating_sub(2));
    let rows: usize = text
        .lines()
        .map(|line| wrapped_rows(&Line::raw(line), room))
        .sum();
    let lines = u16::try_from(rows).unwrap_or(u16::MAX);
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
    let line = match &app.message {
        Some(message) => Span::styled(
            clipped(message, usize::from(footer.width)),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        None => Span::styled(fit_keys(&hints, footer.width), muted()),
    };
    frame.render_widget(Paragraph::new(line), footer);
}

/// What the screen is, and the keys — written once, here, because a footer can
const HELP: &str = "Estigia holds the tools. It gates every repository write,
and every irreversible boundary, against a claim adjudicated on the issue tracker.

  Agents      every agent Estigia can hold the tools for, with its state.
              The first row, all chosen agents, answers for every chosen
              agent at once. ⏎ opens the agent under the cursor: its own
              settings, and the models beneath Model routing. Space ticks or
              unticks the agent under the cursor.
  Options     what is true of this repository whichever agent asks — where
              the issues live, what history the base branch must end up
              with, how long a routine write may ride on the last check.
              Under them, in their own panel, are the preferences of this
              screen: what it looks like on this machine, written into no
              agent's contract because no agent reads them.
  Push guard  the pre-push hook. The one boundary no agent can go around,
              because it runs in git rather than in the agent.
  Doctor      what a run needs before it swears, for the repository this
              screen shows: the same checks, in the same words, as
              `estigia doctor`, with the way out under each one that fails.

Keys
  ↑↓ or jk    move the cursor
  ⏎           open the agent, or the list of answers a row takes
  ⌫ / Esc     back one level; Esc leaves only from the menu
  ←→ or hl    change the highlighted row, on the row itself
  space       tick an agent, or show every answer a row takes
  r           restore the highlighted row to what is installed
  s           review what will be written, from anywhere; ⏎ confirms and
              Esc cancels without writing anything
  u           uninstall the installed agent under the cursor, or the push
              guard from the menu; it lists what would go and waits for ⏎
  ?           this page, from anywhere; any key goes back
  q           quit
  Ctrl-C      quit, asking once first when something is unsaved

Marks
  *           this session moved it and has not written it yet
  ·           this repository already sets it to something other than
              what the skill ships
  •           beside Agents or Options on the menu: it is carrying one of
              those
  (not held)  written into the contract, with no gate behind it here

Nearly every setting has two or three answers and no others, so ←→ is
usually the whole thing: no field, nothing to type, nothing to spell
right. ⏎ or space opens the list when you would rather see them all, and a
path or board row has a field for what no list can hold. The model targets are
a separate block beneath the settings, and they open their advisory catalog
directly; ⏎ or space chooses, custom IDs are always available, and
inherit removes only that target. The same list also offers the effort a
target runs at, and once one is set, an entry to hand it back to the host.
Shared answers borrow no one agent's catalog. The CLI still edits the
complete persisted cell: the models in the first list, the efforts in the
second after a semicolon.
Only Claude Code currently receives host-routable reviewer
definitions; Claude Code and OpenCode both receive whichever roles
`Model routing` names — naming a role is what installs its definition, and
the model beside it says only what it runs on.
Every other host keeps these values as routing declarations;
orchestrate, apply, judge, and a visible route are not execution proof.

Some rows are worth less for some agents. Estigia can gate the tool calls
of an agent whose host lets it and no others; the rest get the contract,
and the pre-push guard, and nothing standing between them and an edit.
An agent's own page marks those rows, on the row.

The agents list shows the same question: `gated` is an agent Estigia
holds, `contract only` is one it can only ask.

A setting's display label is translated. Persisted setting names, accepted
values, and printed CLI commands stay canonical: they are the keys and
cells in markdown tables and the arguments to `estigia config set`.

Nothing is written to an agent until `s` has shown what it will write and
you have confirmed. Three keys act at once: the push guard writes a hook
into the repository shown, a language change is remembered, and
`n` in the folder picker creates the folder on disk. Leaving without `s`
changes nothing else.";

/// The repository's own rows: one list, reached from the menu.
fn draw_options(frame: &mut ratatui::Frame<'_>, app: &App) {
    let said = detail_lines(app);
    let [header, body, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(if said.is_empty() { 0 } else { 7 }),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);

    let screens = u16::try_from(SCREEN_ROWS.len()).unwrap_or(1) + 2;
    // The screen rows above the settings: `Repository shown` decides which checkout's
    // answers the rows below are.
    let [preferences, rows] =
        Layout::vertical([Constraint::Length(screens), Constraint::Min(3)]).areas(body);
    let here = app.screen_at_cursor();
    draw_settings(
        frame,
        app,
        rows,
        // What the cursor walks, not the full list.
        app.rows(),
        &padded(t!(app.tongue, "REPOSITORY OPTIONS")),
        here.is_none(),
    );
    draw_screen_rows(frame, app, preferences, here);

    draw_detail(frame, said, detail);
    draw_footer(frame, app, footer);
    if app.focus == Focus::Picking {
        draw_picker(frame, app, if here.is_some() { preferences } else { rows });
    }
}

/// The two rows that decide what the page is showing, not what it enforces.
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
                // No changed mark and no default mark: there is nothing unsaved to mark.
                Span::raw(format!("  {}", pad_to(screen.label(app.tongue), 24))),
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

/// The agents, with **all chosen agents** first.
fn draw_agents(frame: &mut ratatui::Frame<'_>, app: &App) {
    let said = detail_lines(app);
    let [header, body, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(if said.is_empty() { 0 } else { 7 }),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);
    draw_agent_choice(frame, app, body);
    draw_detail(frame, said, detail);
    draw_footer(frame, app, footer);
}

/// One agent's rows, or everyone's: the page an agent opens onto.
fn draw_detail_page(frame: &mut ratatui::Frame<'_>, app: &App) {
    let said = detail_lines(app);
    let [header, body, detail, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(if said.is_empty() { 0 } else { 7 }),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    draw_header(frame, app, header);
    let title = match app.purpose {
        app::Purpose::OneTable(Some(adapter)) => padded(adapter.display_name),
        app::Purpose::OneTable(None) => padded(t!(app.tongue, "CONTRACT")),
        app::Purpose::Setup if app.everyone => padded(t!(app.tongue, "ALL INSTALLED AGENTS")),
        app::Purpose::Setup => padded(&app.adapter().display_name.to_uppercase()),
    };
    let picker_over = draw_configuration_rows(frame, app, body, &title, true);
    draw_detail(frame, said, detail);
    draw_footer(frame, app, footer);
    if app.focus == Focus::Picking {
        draw_picker(frame, app, picker_over);
    }
}

fn draw_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    frame.render_widget(
        Paragraph::new(Span::styled(fit_keys(&keys_for(app), area.width), muted())),
        area,
    );
}

fn draw_agent_choice(frame: &mut ratatui::Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let everyone = ListItem::new(Line::from(vec![
        Span::raw(format!(
            "    {}",
            pad_to(&clipped(t!(app.tongue, "All chosen agents"), 28), 28)
        )),
        Span::styled(
            fill!(
                app.tongue,
                "{chosen} of {known} agents chosen",
                "chosen" => app.chosen().len(),
                "known" => app.agents.len()
            ),
            muted(),
        ),
    ]));
    let rows: Vec<ListItem<'_>> = std::iter::once(everyone)
        .chain(app.agents.iter().map(|(adapter, on)| {
            // A box drawn in characters, not colour alone.
            let mark = if *on { "[x]" } else { "[ ]" };
            let held = if adapter.can_gate_tools() {
                Span::styled(
                    format!("  {}", pad_to(t!(app.tongue, "gated"), 14)),
                    Style::default().fg(Color::Green),
                )
            } else {
                Span::styled(
                    format!("  {}", pad_to(t!(app.tongue, "contract only"), 14)),
                    muted(),
                )
            };
            // Already installed, or ticked just now.
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
                Span::raw(format!(
                    "{mark} {}",
                    pad_to(&clipped(adapter.display_name, 28), 28)
                )),
                held,
                state,
            ]))
        }))
        .collect();
    let at = if app.everyone { 0 } else { app.agent + 1 };
    let mut state = ListState::default();
    state.select(Some(at));
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(&padded(t!(app.tongue, "AGENTS")), true))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
    scrollbar(frame, area, app.agents.len() + 1, at);
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
    let sections = {
        let mut seen: Vec<crate::config::ModelSection> = Vec::new();
        for target in &models {
            let section = target.kind.section();
            if !seen.contains(&section) {
                seen.push(section);
            }
        }
        u16::try_from(seen.len()).unwrap_or(0)
    };
    let minimum_models = (if app.has_model_profiles() { 4 } else { 3 }) + sections;
    let settings_height = if area.height < 6 && !on_model {
        area.height.min(3)
    } else {
        preferred
            .min(area.height.saturating_sub(minimum_models))
            .max(area.height.min(3))
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
    let model_title = padded(t!(app.tongue, "MODELS"));
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
        .map(|target| display_width(&app.model_value(target.name)))
        .chain(has_profile.then(|| display_width(&app.model_profile_value())))
        .max()
        .unwrap_or(0);
    let room = usize::from(area.width.saturating_sub(2));
    let column = room
        .saturating_sub(4 + widest)
        .clamp(6, 26)
        .min((room / 2).max(1));
    let selected_target = app.model_target_at_cursor();
    let mut rows: Vec<ListItem<'_>> = Vec::new();
    let mut selected_at = None;
    if has_profile {
        if app.model_profile_at_cursor() {
            selected_at = Some(rows.len());
        }
        let value = app.model_profile_value();
        let mark = if app.model_profile_changed() {
            "*"
        } else {
            " "
        };
        rows.push(ListItem::new(Line::from(vec![
            Span::raw(format!(
                "{mark} {}",
                pad_to(
                    &clipped(t!(app.tongue, "Profile"), column.saturating_sub(1)),
                    column
                )
            )),
            Span::styled(value, Style::default().add_modifier(Modifier::BOLD)),
        ])));
    }
    let mut last_section: Option<crate::config::ModelSection> = None;
    for target in targets {
        let section = target.kind.section();
        if last_section != Some(section) {
            last_section = Some(section);
            let heading = match section {
                crate::config::ModelSection::Phases => t!(app.tongue, "PHASES"),
                crate::config::ModelSection::Roles => t!(app.tongue, "ROLES"),
            };
            rows.push(ListItem::new(Line::from(Span::styled(
                heading,
                Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
            ))));
        }
        if selected_target.is_some_and(|selected| selected.name == target.name) {
            selected_at = Some(rows.len());
        }
        let changed = app.model_changed(target.name);
        let value = app.model_value(target.name);
        let mark = if changed {
            "*"
        } else if app.installed_model_assignment(target.name) != ModelAssignment::Shared(None) {
            "\u{00b7}"
        } else {
            " "
        };
        rows.push(ListItem::new(Line::from(vec![
            Span::raw(format!(
                "{mark} {}",
                pad_to(&clipped(target.name, column.saturating_sub(1)), column)
            )),
            Span::styled(
                value,
                if changed {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ])));
    }
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

/// The list position of a setting once the group headings above it are counted.
///
/// The cursor walks settings, not drawn rows: a heading is shown but never
/// selected, so a setting's row is its index plus one for every group that has
/// begun at or before it.
fn settings_list_index(settings: &[Setting], setting_index: usize) -> usize {
    let mut headings = 0usize;
    let mut last: Option<Group> = None;
    for (at, setting) in settings.iter().enumerate() {
        if last != Some(setting.group()) {
            headings += 1;
            last = Some(setting.group());
        }
        if at == setting_index {
            break;
        }
    }
    setting_index + headings
}

/// How many drawn rows a list of settings takes, headings included.
fn settings_list_len(settings: &[Setting]) -> usize {
    if settings.is_empty() {
        0
    } else {
        settings_list_index(settings, settings.len() - 1) + 1
    }
}

/// The heading a group is shown under, uppercased the way every panel title is.
fn group_heading(tongue: Tongue, group: Group) -> String {
    t!(tongue, group.label()).to_uppercase()
}

/// A list of settings, with what each is set to, under one heading per group.
///
/// A heading is a drawn row the cursor never lands on: the same shape the model
/// panel gives `PHASES` and `ROLES`, and [`settings_list_index`] is the one
/// place that says where a setting sits among them.
fn draw_settings(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
    settings: &[Setting],
    title: &str,
    focused: bool,
) {
    // The label column gives way before the value does: the value is what this screen reports.
    let widest = settings
        .iter()
        .map(|setting| display_width(&app.shown_value(*setting)))
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
    // Where this list starts inside the page's one cursor, computed once for all readers.
    let cursor = if app.page == Page::Options {
        app.options_at.saturating_sub(SCREEN_ROWS.len())
    } else {
        app.selected
    };
    let mut rows: Vec<ListItem<'_>> = Vec::new();
    let mut last_group: Option<Group> = None;
    for (index, setting) in settings.iter().enumerate() {
        if last_group != Some(setting.group()) {
            last_group = Some(setting.group());
            rows.push(ListItem::new(Line::from(Span::styled(
                group_heading(app.tongue, setting.group()),
                Style::default().add_modifier(Modifier::BOLD | Modifier::DIM),
            ))));
        }
        // A changed row is marked with a character as well as a colour.
        let changed = app.changed(*setting);
        let applies = app.applies(*setting);
        let shown = app.shown_value(*setting);
        // Three states, not two: `*` moved this session, `·` the repository already sets.
        let mark = if changed {
            "*"
        } else if shown != setting.default_value() {
            "\u{00b7}"
        } else {
            " "
        };
        let mut spans = vec![Span::raw(format!(
            "{mark} {}",
            // One short of the column, so the padding always leaves a gap.
            pad_to(
                &clipped(app.tongue.say(setting.label()), column.saturating_sub(1)),
                column
            )
        ))];
        // Brackets on the row under the cursor, and only there — against the offset cursor.
        let here = index == cursor && app.focus == Focus::List;
        let can_cycle = [-1, 1].into_iter().any(|delta| {
            app.step_of(*setting, &shown, delta)
                .is_some_and(|next| next != shown)
        });
        if here && can_cycle {
            spans.push(Span::styled("\u{2039} ", Style::default().fg(Color::Cyan)));
        }
        spans.push(Span::styled(
            shown,
            if changed {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ));
        if here && can_cycle {
            spans.push(Span::styled(" \u{203a}", Style::default().fg(Color::Cyan)));
        }
        // Scope, on every row: a group can hold more than one, so the heading
        // alone would not say whose answer this is.
        spans.push(Span::styled(
            format!("  {}", t!(app.tongue, setting.scope_label())),
            muted(),
        ));
        if app.page == Page::Detail
            && let Some(tag) = applies.tag()
        {
            spans.push(Span::styled(format!("  {}", t!(app.tongue, tag)), muted()));
        }
        if app.page == Page::Detail && setting.repository_may_tighten() {
            spans.push(Span::styled(
                format!("  {}", t!(app.tongue, "a repository may tighten")),
                muted(),
            ));
        }
        if app.disagrees(*setting) {
            spans.push(Span::styled(
                format!("  {}", t!(app.tongue, "(differs by agent)")),
                muted(),
            ));
        }
        rows.push(ListItem::new(Line::from(spans)));
    }
    let total = settings_list_len(settings);
    let selected_at = focused
        .then(|| settings_list_index(settings, cursor.min(settings.len().saturating_sub(1))));
    let mut state = ListState::default();
    // Unselected when the cursor has moved past this list; offset by the rows drawn above.
    state.select(selected_at);
    frame.render_stateful_widget(
        List::new(rows)
            .block(panel(title, focused))
            .highlight_style(selected())
            .highlight_symbol("> "),
        area,
        &mut state,
    );
    if let Some(at) = selected_at {
        scrollbar(frame, area, total, at);
    }
}

/// The panel under the body: what the thing under the cursor is, and anything
/// the screen has to say about it.
fn draw_detail(
    frame: &mut ratatui::Frame<'_>,
    lines: Vec<Line<'static>>,
    area: ratatui::layout::Rect,
) {
    // An empty bordered box with nothing to say is furniture.
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
    let mut lines: Vec<Line<'static>> = if let Some(screen) = app.screen_at_cursor() {
        detail_of_screen(app, screen)
    } else if app.model_profile_at_cursor() {
        detail_of_model_profile(app)
    } else if app.model_target_at_cursor().is_some() {
        detail_of_model_target(app)
    } else if matches!(app.page, Page::Options | Page::Detail) {
        detail_of_setting(app)
    } else if app.everyone {
        vec![
            Line::styled(
                t!(app.tongue, "All chosen agents").to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::raw(t!(
                app.tongue,
                "answers here are written to every chosen agent at once"
            )),
        ]
    } else {
        let (adapter, on) = app.agents[app.agent];
        vec![
            Line::styled(
                adapter.display_name.to_owned(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Line::raw(if on {
                t!(app.tongue, "chosen — enter opens its own settings")
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
    };
    // Whatever the screen last had to say, always.
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
            "profiles do not prove model availability or make Estigia run a model"
        )
        .to_owned(),
        muted(),
    ));
    lines
}

/// The same three things, for a preference of this screen.
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
                "accepts: any model ID that fits one key=model entry; no comma, semicolon, pipe, or line break; catalogs are advisory"
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

    if let Some(note) = app.model_foreign_note(target.name) {
        lines.push(Line::styled(note, Style::default().fg(Color::Yellow)));
    }

    if app.everyone && app.disagrees(Setting::Models) {
        lines.push(Line::styled(
            t!(
                app.tongue,
                "Model routing differs across selected agents; unify it or edit each agent"
            )
            .to_owned(),
            Style::default().fg(Color::Yellow),
        ));
    }
    lines.push(Line::styled(
        if crate::skill::DELEGATED_AGENTS
            .iter()
            .any(|(key, _)| *key == target.name)
        {
            // Naming this role in the routing is what installs its definition.
            t!(
                app.tongue,
                "naming this role in `Model routing` is what installs its definition; removing the entry retracts it, and the model beside it says only what it runs on"
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
        if app.everyone {
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
    // Directly under the label, before anything explanatory: everything else wraps.
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

    // The answers, laid out with the one in force marked.
    lines.push(answers_line(app, setting));

    // What choosing *that one* does — the answer under the picker's cursor while one is
    // open, the one in force otherwise.
    let looking_at = if app.focus == Focus::Picking {
        app.picker().get(app.pick).cloned()
    } else {
        Some(app.shown_value(setting))
    };
    // Unless there is a warning to show: a transient message yields to nothing.
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

    // Where the answer lands, said on the row.
    let installed = setting.value_of(&app.installed[app.current()]);
    lines.push(Line::styled(
        match setting.scope() {
            Scope::Agent if app.everyone => fill!(
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
            Scope::Everywhere | Scope::Repository => fill!(
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
    if app.page == Page::Detail
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
fn answers_line(app: &App, setting: Setting) -> Line<'static> {
    let answers = setting.answers();
    let current = app.shown_value(setting);
    let mut spans = Vec::new();
    if app.is_text_field(setting) {
        spans.push(Span::styled(
            format!(" \u{25cf}{current} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(
                "  {} — {}",
                t!(app.tongue, "type to change"),
                t!(app.tongue, setting.accepted())
            ),
            muted(),
        ));
        return Line::from(spans);
    }
    let offered = app.offered(setting);
    for choice in &offered {
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
    // A value not on the list is shown where the list would have shown it.
    if !offered.contains(&current) && !current.is_empty() {
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
fn draw_picker(frame: &mut ratatui::Frame<'_>, app: &App, over: ratatui::layout::Rect) {
    let entries = app.picker();
    if entries.is_empty() || over.width < 12 || over.height < 4 {
        return;
    }
    // While the walk is inside a folder, the folder is the title, clipped from the left:
    // the end of a path is what tells two folders apart.
    let title = match app.browsing.as_deref() {
        Some(here) => {
            let shown = here.display().to_string();
            let room = usize::from(over.width.saturating_sub(10)).max(12);
            if display_width(&shown) > room {
                format!("…{}", tail(&shown, room))
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
        .map(|entry| display_width(entry))
        .max()
        .unwrap_or(20)
        .max(display_width(&title) + 2) as u16
        + 6;
    let indent = 4.min(inner.width.saturating_sub(width));
    let width = width.min(inner.width.saturating_sub(indent));

    // Under the row, unless that runs off the bottom — then above it.
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
        // configuration panels already count their own settings from zero. A
        // group heading is a drawn row, so the setting's list position counts
        // the headings above it.
        None if app.page == Page::Options => {
            settings_list_index(app.rows(), app.options_at.saturating_sub(SCREEN_ROWS.len()))
        }
        None => settings_list_index(app.rows(), app.selected),
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

fn change_key(app: &App) -> String {
    if app.model_target_at_cursor().is_some() {
        t!(app.tongue, "←→ effort").to_owned()
    } else {
        t!(app.tongue, "←→ change").to_owned()
    }
}

/// The keys of the step somebody is on, and only those.
fn keys_for(app: &App) -> Vec<String> {
    if app.removing.is_some() {
        return vec![
            t!(app.tongue, "⏎ remove it").to_owned(),
            t!(app.tongue, "Esc cancel").to_owned(),
        ];
    }
    if app.reviewing {
        return vec![
            t!(app.tongue, "⏎ write it").to_owned(),
            t!(app.tongue, "Esc cancel").to_owned(),
        ];
    }
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
            // Named only where they do something.
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
    if app.page == Page::Agents {
        let mut keys = vec![
            t!(app.tongue, "↑↓ move").to_owned(),
            t!(app.tongue, "space tick").to_owned(),
            t!(app.tongue, "enter open").to_owned(),
        ];
        if app.can_uninstall() {
            keys.push(t!(app.tongue, "u uninstall").to_owned());
        }
        keys.extend([
            t!(app.tongue, "? help").to_owned(),
            t!(app.tongue, "Esc back").to_owned(),
            t!(app.tongue, "s apply").to_owned(),
        ]);
        return keys;
    }
    vec![
        change_key(app),
        t!(app.tongue, "⏎ / space all answers").to_owned(),
        t!(app.tongue, "↑↓ move").to_owned(),
        t!(app.tongue, "r restore").to_owned(),
        t!(app.tongue, "? help").to_owned(),
        if app.pinned() {
            t!(app.tongue, "q quit").to_owned()
        } else {
            t!(app.tongue, "Esc back").to_owned()
        },
        t!(app.tongue, "s apply").to_owned(),
    ]
}

#[cfg(test)]
mod tests;
