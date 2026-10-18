//! Dispatch, and the only place a [`Refusal`] becomes an exit code.

use std::io::Write;
use std::process::ExitCode as ProcessExitCode;

use anyhow::Result;
use clap::Parser;

use crate::config::{Config, Group, SETTINGS, Scope, Setting};
use crate::harness;
use crate::harness::population::{Disposition, Population};
use crate::lifecycle;
use crate::outcome::{ExitCode, Refusal, Resolution, exit_code_for};
use crate::setup::{
    self, AGENTS, AgentAdapter, COMPANIONS, Change, InstallScope, SetupOptions, SetupResult,
    find_agent,
};
use crate::skill;

use args::ScopeArg;

// Crate-visible so the harness asks clap which verbs exist rather than greps the source.
pub(crate) mod args;
mod loop_stop;

pub use args::{
    Cli, Command, ConfigAction, GoalAction, ReviewAction, SessionAction, SessionsAction,
    StoreAction,
};

/// A note for whoever is watching, on standard error.
macro_rules! note {
    ($($argument:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($argument)*);
    }};
}

macro_rules! say {
    ($($argument:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($argument)*).is_err() {
            // The reader is gone. There is nothing left to report and nobody to
            // report it to.
            std::process::exit(0);
        }
    }};
}

pub(crate) mod sessions;

/// The same line, into a string rather than onto the terminal.
macro_rules! line {
    ($text:expr, $($argument:tt)*) => {{
        use std::fmt::Write as _;
        let _ = writeln!($text, $($argument)*);
    }};
}

/// Parses the command line and runs it.
pub fn run() -> ProcessExitCode {
    // `try_parse`, not `parse`: clap exits 2 on a usage error, and 2 must only ever
    // mean *interrupted* here — every hook reads 1 and 2 as decisions.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::Unreadable.into()
            } else {
                ExitCode::Success.into()
            };
        }
    };
    match dispatch(&cli) {
        Ok(()) => ExitCode::Success.into(),
        Err(refusal) => {
            report(&refusal, cli.json);
            gate_exit_code(&refusal).into()
        }
    }
}

/// Writes a refusal where a person or a script will see it.
fn report(refusal: &Refusal, json: bool) {
    let mut stderr = std::io::stderr();
    let written = if json {
        serde_json::to_string_pretty(refusal)
            .map(|text| writeln!(stderr, "{text}"))
            .unwrap_or_else(|error| writeln!(stderr, "{error}"))
    } else {
        writeln!(stderr, "estigia: {refusal}")
    };
    // Nothing useful is left to do about a failed write to stderr, and
    // panicking here would replace a precise refusal with a backtrace.
    let _ = written;
}

/// Refuses before reading anything when the screen has no terminal to draw on.
fn require_terminal(instead: Resolution) -> Result<(), Refusal> {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(());
    }
    let mut refusal = setup::no_terminal();
    refusal.resolution = instead;
    Err(refusal)
}

/// A workspace named under the global scope, which would install nothing there.
/// `verb` is the command being run, so the way out names that command.
pub(crate) fn workspace_without_scope(verb: &str) -> Refusal {
    Refusal::not_started(
        "workspace-without-scope",
        "--workspace named a workspace but the scope is global, so nothing would be installed there",
        Resolution::run(format!(
            "estigia {verb} --scope=workspace --workspace <path>"
        )),
    )
}

/// The workspace scope named with no workspace to install into.
pub(crate) fn workspace_without_a_path(verb: &str) -> Refusal {
    Refusal::not_started(
        "workspace-without-a-path",
        "--scope=workspace was given with no --workspace, so there is nowhere to install",
        Resolution::run(format!(
            "estigia {verb} --scope=workspace --workspace <path>"
        )),
    )
}

/// A companion named together with a workspace, which a companion install has no
/// use for.
pub(crate) fn companion_with_a_workspace(verb: &str) -> Refusal {
    Refusal::not_started(
        "companion-with-a-workspace",
        "--companion installs nothing into a checkout, so the workspace beside it would be read by nothing",
        Resolution::run(format!("estigia {verb} --companion <name>")),
    )
}

/// The install options a command's scope and workspace name.
///
/// `verb` is the command being run, so a refusal names that command rather than
/// this module's first caller.
fn install_options(
    dry_run: bool,
    scope: ScopeArg,
    workspace: Option<std::path::PathBuf>,
    verb: &str,
) -> Result<SetupOptions, Refusal> {
    let scope = match (scope, workspace.is_some()) {
        (ScopeArg::Workspace, true) => InstallScope::Workspace,
        (ScopeArg::Workspace, false) => return Err(workspace_without_a_path(verb)),
        (ScopeArg::Global, true) => return Err(workspace_without_scope(verb)),
        (ScopeArg::Global, false) => InstallScope::Global,
    };
    Ok(SetupOptions {
        dry_run,
        scope,
        workspace,
        ..SetupOptions::default()
    })
}

fn with_installed_pi(options: SetupOptions) -> SetupOptions {
    SetupOptions {
        pi_version: setup::installed_pi_version(),
        ..options
    }
}

fn dispatch(cli: &Cli) -> Result<(), Refusal> {
    match &cli.command {
        Command::RecordInstall => record_install(cli.json),

        Command::ReviewJob { dir } => {
            crate::reviewer::job::run(dir, &crate::reviewer::select_adapter)
        }

        Command::Tui => {
            require_terminal(Resolution::run(
                "estigia setup --all   # or estigia setup <agent>",
            ))?;
            guided(&with_installed_pi(SetupOptions::default()), cli.json)
        }

        Command::Setup {
            agent,
            all,
            dry_run,
            uninstall,
            skill_only,
            companion,
            allow_source_build,
            scope,
            workspace,
        } => {
            // Built before the companion branch: a scope flag that would install
            // nothing is refused on every setup path, including the one that
            // installs nothing at all.
            //
            // The `--uninstall` flag routes to removal below, so the refusal's
            // way out has to be that same command rather than a bare `setup`.
            let verb = if *uninstall {
                "setup --uninstall"
            } else {
                "setup"
            };
            let mut options = install_options(*dry_run, *scope, workspace.clone(), verb)?;
            options.skip_directive = *skill_only;
            if let Some(slug) = companion {
                if options.scope == InstallScope::Workspace {
                    // A bare `setup`, not `verb`: the companion branch shows the
                    // companion whatever `--uninstall` says, so a way out
                    // carrying it would name a command that does not remove.
                    return Err(companion_with_a_workspace("setup"));
                }
                return show_companion(slug);
            }
            let targets = select(agent.as_deref(), *all, !*uninstall)?;
            if *uninstall {
                take_out(&targets, &options, cli.json)
            } else {
                let options = with_installed_pi(options);
                lifecycle_preflight(
                    &options,
                    *allow_source_build,
                    Resolution::run("estigia setup --allow-source-build"),
                )?;
                let shared = installed_configs(&targets, &options);
                report_fresh_roots(&shared, &targets, &options);
                run_setup_over(&targets, &shared, &options, cli.json)
            }
        }

        Command::Uninstall {
            agent,
            all,
            dry_run,
            scope,
            workspace,
        } => {
            let options = install_options(*dry_run, *scope, workspace.clone(), "uninstall")?;
            let targets = select(agent.as_deref(), *all, false)?;
            take_out(&targets, &options, cli.json)
        }

        Command::Sync {
            agent,
            all: _,
            dry_run,
            allow_source_build,
            scope,
            workspace,
        } => {
            let options = with_installed_pi(install_options(
                *dry_run,
                *scope,
                workspace.clone(),
                "sync",
            )?);
            let (targets, notes) = match agent {
                Some(slug) => (vec![setup::find_installable_agent(slug)?], Vec::new()),
                None => (configured(&options), setup::substitution_notes(&options)),
            };
            if targets.is_empty() {
                return Err(nothing_configured(&notes));
            }
            // A note `describe` already prints for one of these targets is
            // printed there; the rest would otherwise be lost with an adapter
            // `is_present` could not find under the substituted root.
            let attached: Vec<String> = targets
                .iter()
                .filter_map(|adapter| setup::resolve_paths(adapter, &options).ok())
                .flat_map(|paths| paths.notes)
                .collect();
            for note in notes.iter().filter(|note| !attached.contains(note)) {
                note!("{note}");
            }
            lifecycle_preflight(
                &options,
                *allow_source_build,
                Resolution::run("estigia sync --allow-source-build"),
            )?;
            let shared = installed_configs(&targets, &options);
            report_fresh_roots(&shared, &targets, &options);
            let mut pending = setup::Pending::new();
            let recorded = setup::install_root_records(&options);
            let synced = run_setup_evidenced_over(&targets, cli.json, |adapter| {
                let theirs = SetupOptions {
                    skip_directive: !setup::is_configured(adapter, &options),
                    ..options.clone()
                };
                setup::setup_into_evidenced(
                    adapter,
                    &config_for(adapter, &shared, &options).map_err(|refusal| {
                        setup::SetupFailure::before_mutation(
                            adapter.slug,
                            options.dry_run,
                            refusal.into(),
                        )
                    })?,
                    &theirs,
                    &mut pending,
                )
            });
            name_new_install_roots(&options, &recorded, cli.json);
            synced
        }

        Command::Status => status(cli.json),

        Command::Sessions { action } => sessions_command(action.as_ref(), cli.json),

        Command::Update => update(cli.json),

        Command::Doctor => doctor(cli.json),

        Command::Prune { confirm, base } => prune(*confirm, base, cli.json),

        Command::Changes => changes(cli.json),

        Command::Store { action } => match action {
            StoreAction::Import => store_import(cli.json),
        },

        Command::Flush { detach, status } => flush(*detach, *status, cli.json),

        Command::Roles {
            run_id,
            issue,
            json,
        } => roles(run_id.as_deref(), issue.as_ref(), *json || cli.json),

        Command::Mcp => mcp(),

        Command::Hook {
            event,
            dialect,
            agent,
            wait_mail,
        } => hook(event, dialect, agent.as_deref(), *wait_mail),

        Command::Guard { uninstall, dry_run } => guard(*uninstall, *dry_run, cli.json),

        Command::Claim {
            issue,
            run_id,
            horizon,
            state,
            transition_to,
        } => claim(
            issue,
            run_id,
            horizon,
            state,
            transition_to.as_deref(),
            cli.json,
        ),

        Command::Recover {
            issue,
            epoch,
            authorized_by,
            reason,
        } => recover(
            issue,
            epoch.as_deref(),
            authorized_by.as_deref(),
            reason,
            cli.json,
        ),

        Command::Release { run_id } => release(run_id, cli.json),

        Command::Gate {
            tool,
            input,
            run_id,
            session_id,
            runtime,
            parent_session_id,
            can_ask,
        } => {
            let ambient = std::env::var("ESTIGIA_RUN_ID").ok();
            let identity = match parent_session_id.as_deref() {
                Some(parent) => parent_identity(run_id.as_deref(), runtime.as_deref(), parent),
                None => gate_identity(
                    "estigia gate <tool>",
                    run_id.as_deref(),
                    session_id.as_deref(),
                    runtime.as_deref(),
                    ambient.as_deref(),
                ),
            }?;
            let input = gate_input(input)?;
            show_gate(
                tool,
                &input,
                identity.as_deref(),
                cli.json,
                asks_through(*can_ask, runtime.as_deref()),
            )
        }

        Command::Goal {
            action,
            run_id,
            session_id,
            runtime,
            completed,
        } => goal(
            action.as_ref(),
            session_id.as_deref(),
            runtime.as_deref(),
            run_id.as_deref(),
            *completed,
            cli.json,
        ),

        Command::Session { action } => session_lifecycle(action, cli.json),

        Command::Review {
            action,
            run_id,
            session_id,
            runtime,
        } => review(
            action,
            run_id.as_deref(),
            session_id.as_deref(),
            runtime.as_deref(),
            cli.json,
        ),

        Command::StandDown {
            reason,
            minutes,
            lift,
        } => stand_down(reason, *minutes, *lift, cli.json),

        Command::Config { action } => match action {
            ConfigAction::List { agent } => {
                config_list(agent.as_deref(), &SetupOptions::default(), cli.json)
            }
            ConfigAction::Edit { agent } => {
                require_terminal(Resolution::run("estigia config set <setting> <value>"))?;
                config_edit(agent.as_deref(), &SetupOptions::default())
            }
            ConfigAction::Set {
                setting,
                value,
                agent,
                repo,
                contract,
            } => config_set(
                setting,
                value,
                agent.as_deref(),
                *repo,
                *contract,
                &SetupOptions::default(),
                cli.json,
            ),
            ConfigAction::Repos => config_repos(cli.json),
            ConfigAction::Forget { repo } => config_forget(repo.as_deref(), cli.json),
        },
    }
}

fn listed_at_startup(read: &setup::Installed) -> bool {
    *read != setup::Installed::Absent
}

/// Asks the questions, then applies the answers through the ordinary path.
fn guided(options: &SetupOptions, json: bool) -> Result<(), Refusal> {
    let targets: Vec<&'static AgentAdapter> = AGENTS.iter().collect();
    let mut installed: std::collections::BTreeMap<&'static str, Config> =
        std::collections::BTreeMap::new();
    let mut configured: Vec<&str> = Vec::new();
    // The same tables twice: as the checkout answers, and with no checkout's rows —
    // the screen opens on the first; another repository's rows are laid over the second.
    let mut bare: std::collections::BTreeMap<&'static str, Config> =
        std::collections::BTreeMap::new();
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for adapter in &targets {
        if listed_at_startup(&setup::read_configured(adapter, options)) {
            configured.push(adapter.slug);
        }
        let layers = setup::resolve_paths(adapter, options)
            .ok()
            .and_then(|paths| {
                skill::config_layers(&paths.skill_root, Some(adapter.slug), Some(&here)).ok()
            });
        let own = layers
            .as_ref()
            .map(|layers| layers.unlayered().clone())
            .unwrap_or_default();
        // Tolerated here and refused at the write; `install_planned` asks again.
        installed.insert(
            adapter.slug,
            layers
                .as_ref()
                .map(|layers| layers.effective().clone())
                .unwrap_or_else(|| own.clone()),
        );
        bare.insert(adapter.slug, own);
    }

    let mut written: Vec<String> = Vec::new();
    let outcome = crate::tui::setup(&installed, &bare, &configured, &mut |plan| {
        let (receipt, report) = guided_install(plan, options, json)?;
        written.push(report);
        Ok(receipt)
    });

    if let Some(report) = guided_report(&written, &outcome) {
        say!("{report}");
    }
    outcome
}

/// What the screen's session leaves on standard output: every install it made, in order.
///
/// A later refusal does not take an earlier install out of it, and leaving without
/// installing is a choice only when nothing failed.
pub(crate) fn guided_report(written: &[String], outcome: &Result<(), Refusal>) -> Option<String> {
    if !written.is_empty() {
        return Some(written.join("\n"));
    }
    outcome.is_ok().then(|| "nothing installed".to_owned())
}

/// The screen's write, gated the way the shell's write is.
fn guided_install(
    plan: &crate::tui::Plan,
    options: &SetupOptions,
    json: bool,
) -> Result<(crate::tui::InstallReceipt, String), crate::tui::InstallFailure> {
    lifecycle_preflight(
        options,
        true,
        Resolution::run("estigia setup --allow-source-build"),
    )?;
    install_planned(plan, options, json)
}

/// The agents a save of this plan runs setup for: ticked and not yet installed,
/// or with a setting that differs from what the screen opened on.
pub(crate) fn agents_to_touch(
    plan: &crate::tui::Plan,
    options: &SetupOptions,
) -> Vec<&'static AgentAdapter> {
    plan.to_touch(|adapter| setup::is_configured(adapter, options))
}

/// The population and boundary the guard over `install_planned`'s call sites
/// enforces.
///
/// `install_planned` deploys adapter assets and is itself ungated; what makes
/// `guided_install` safe is that it calls the lifecycle preflight first. This
/// declares the set of production call sites that must be preceded by that
/// preflight, so a second one cannot arrive without the rule being read.
pub const INSTALL_PLANNED_CALLERS_POPULATION: Population = Population {
    family: "install-planned-callers",
    disposition: Disposition::TooTight,
    subject: "install_planned",
    text: "every production call site of `install_planned`, the function that deploys adapter \
           assets. Legitimate population: every call in `src/` outside a `*tests.rs` file, where \
           this crate keeps its test modules. Boundary: each call site is preceded, inside its own \
           function, by a `lifecycle_preflight` call \u{2014} the gate `guided_install` puts in \
           front of it today. Read from stripped source text, so a caller copied or moved without \
           the preflight in front of it is caught; a call the line filter does not see \u{2014} one \
           written inside a `#[cfg(test)]` block in a production file, or reached through a macro, \
           a function pointer or a `use` alias \u{2014} is not, and deliberate circumvention is \
           not in reach either.",
};

/// Performs one screenful of decisions: the verdict, then everything it did.
pub(crate) fn install_planned(
    plan: &crate::tui::Plan,
    options: &SetupOptions,
    json: bool,
) -> Result<(crate::tui::InstallReceipt, String), crate::tui::InstallFailure> {
    let chosen: Vec<&'static AgentAdapter> = plan.agents.clone();
    let here = &plan.repository;
    let mut paths_by_slug = std::collections::BTreeMap::new();
    let mut layers_before = std::collections::BTreeMap::new();
    let mut folded_before: std::collections::BTreeMap<_, Option<String>> =
        std::collections::BTreeMap::new();
    for adapter in &chosen {
        let paths = setup::resolve_paths(adapter, options).map_err(|_| skill::no_skill_root())?;
        let repository = (!here.as_os_str().is_empty()).then_some(here.as_path());
        let (layers, folded) =
            skill::config_layers_for_install(&paths.skill_root, Some(adapter.slug), repository)?;
        folded_before.insert(adapter.slug, folded.map(|report| report.message));
        paths_by_slug.insert(adapter.slug, paths);
        layers_before.insert(adapter.slug, layers);
    }

    let changed_by_slug: std::collections::BTreeMap<_, _> = chosen
        .iter()
        .map(|adapter| (adapter.slug, plan.changed_settings(adapter.slug)))
        .collect();

    let mut contract_by_root = std::collections::BTreeMap::new();
    for adapter in &chosen {
        let paths = &paths_by_slug[adapter.slug];
        let contract = contract_by_root
            .entry(paths.skill_root.clone())
            .or_insert_with(|| layers_before[adapter.slug].contract.clone());
        for setting in &changed_by_slug[adapter.slug] {
            let belongs_in_contract = match setting.scope() {
                crate::config::Scope::Agent => adapter.discovers_skills(),
                crate::config::Scope::Everywhere => {
                    layers_before[adapter.slug].repository.is_none()
                }
                crate::config::Scope::Machine => true,
                crate::config::Scope::Repository => false,
            };
            if belongs_in_contract {
                setting.apply(contract, &setting.value_of(&plan.rows[adapter.slug]))?;
            }
        }
    }

    let effective_by_slug: std::collections::BTreeMap<_, _> = chosen
        .iter()
        .map(|adapter| {
            let paths = &paths_by_slug[adapter.slug];
            let portable = contract_by_root
                .get(&paths.skill_root)
                .unwrap_or(&layers_before[adapter.slug].contract);
            let layers = &layers_before[adapter.slug];
            let mut effective = portable.clone();
            if let Some(agent) = &layers.agent {
                skill::apply_settings(&mut effective, agent, &layers.agent_settings)?;
            }
            if !adapter.discovers_skills() {
                skill::apply_settings(
                    &mut effective,
                    &plan.rows[adapter.slug],
                    &changed_by_slug[adapter.slug]
                        .iter()
                        .copied()
                        .filter(|setting| setting.scope() == crate::config::Scope::Agent)
                        .collect::<Vec<_>>(),
                )?;
            }
            if let Some(local) = &layers.local {
                skill::apply_settings(&mut effective, local, &layers.local_settings)?;
            }
            if let Some(repository) = &layers.repository {
                skill::apply_settings(&mut effective, repository, &layers.repository_settings)?;
                skill::apply_settings(
                    &mut effective,
                    &plan.rows[adapter.slug],
                    &changed_by_slug[adapter.slug]
                        .iter()
                        .copied()
                        .filter(|setting| {
                            matches!(
                                setting.scope(),
                                crate::config::Scope::Everywhere | crate::config::Scope::Repository
                            )
                        })
                        .collect::<Vec<_>>(),
                )?;
            }
            Ok((adapter.slug, effective))
        })
        .collect::<Result<_, Refusal>>()?;

    let touched = agents_to_touch(plan, options);
    if touched.is_empty() {
        let receipt =
            crate::tui::InstallReceipt::empty("nothing changed, nothing written".to_owned());
        let text = receipt.summary.clone();
        return Ok((receipt, if json { "[]".to_owned() } else { text }));
    }

    let mut pending = setup::Pending::new();
    let mut done: Vec<(&'static AgentAdapter, SetupResult)> = Vec::new();
    let mut completed = std::collections::BTreeSet::new();
    let mut roots_written = std::collections::BTreeSet::new();
    for adapter in &touched {
        let paths = &paths_by_slug[adapter.slug];
        let owns_skill = roots_written.insert(paths.skill_root.clone());
        let setup_config = if owns_skill {
            &contract_by_root[&paths.skill_root]
        } else {
            &layers_before[adapter.slug].contract
        };
        match setup::setup_adapter_into(
            adapter,
            setup_config,
            &effective_by_slug[adapter.slug],
            options,
            &mut pending,
            owns_skill,
        ) {
            Ok(mut result) => {
                if let Some(note) = folded_before
                    .get(adapter.slug)
                    .and_then(|note| note.clone())
                {
                    result.notes.push(note);
                }
                completed.insert(adapter.slug);
                done.push((*adapter, result));
            }
            Err(mut failure) => {
                if let Some(note) = folded_before
                    .get(adapter.slug)
                    .and_then(|note| note.clone())
                {
                    failure.result.notes.push(note);
                }
                let refusal = setup_failure_refusal(adapter, &failure);
                let write_attempted = failure.write_attempted;
                done.push((*adapter, failure.result));
                return Err(install_failure(
                    refusal,
                    plan,
                    options,
                    &done,
                    write_attempted,
                    false,
                    &completed,
                ));
            }
        }
    }

    if let Some(refusal) = injected_install_failure(InstallFailurePoint::AfterSetup) {
        return Err(install_failure(
            refusal, plan, options, &done, false, false, &completed,
        ));
    }

    let mut overrides_written = 0;
    let mut override_failure = None;
    for (adapter, result) in &mut done {
        if adapter.discovers_skills() {
            continue;
        }
        let agent_settings = changed_by_slug[adapter.slug]
            .iter()
            .copied()
            .filter(|setting| setting.scope() == crate::config::Scope::Agent)
            .collect::<Vec<_>>();
        if agent_settings.is_empty() {
            continue;
        }
        let paths = &paths_by_slug[adapter.slug];
        let mut desired = layers_before[adapter.slug]
            .agent
            .clone()
            .unwrap_or_else(|| layers_before[adapter.slug].contract.clone());
        for setting in &agent_settings {
            setting.apply(&mut desired, &setting.value_of(&plan.rows[adapter.slug]))?;
        }
        let file = skill::agent_override(&paths.skill_root, adapter.slug);
        let snapshot = setup::AgentConfigurationSnapshot::from_document(
            file,
            layers_before[adapter.slug].agent_document.clone(),
        );
        let mut persisted_settings = snapshot.settings();
        persisted_settings.extend(agent_settings.iter().copied());
        persisted_settings.sort_by_key(|setting| {
            crate::config::AGENT_SETTINGS
                .iter()
                .position(|candidate| candidate == setting)
                .unwrap_or(usize::MAX)
        });
        persisted_settings.dedup();
        match setup::write_agent_configuration_snapshot_into(
            &snapshot,
            adapter.slug,
            &desired,
            &persisted_settings,
            options.dry_run,
            &mut pending,
        ) {
            Ok(action) => result.actions.push(action),
            Err(refusal) => {
                override_failure = Some(refusal);
                break;
            }
        }
        overrides_written += 1;
        if let Some(refusal) =
            injected_install_failure(InstallFailurePoint::AfterOverride(overrides_written))
        {
            override_failure = Some(refusal);
            break;
        }
    }
    if let Some(refusal) = override_failure {
        return Err(install_failure(
            refusal, plan, options, &done, true, false, &completed,
        ));
    }

    let mut repository_failure = None;
    let mut repository_proven = false;
    let mut repository_settings = chosen
        .first()
        .map(|adapter| layers_before[adapter.slug].repository_settings.clone())
        .unwrap_or_default();
    if let Some(adapter) = chosen.first() {
        repository_settings.extend(changed_by_slug[adapter.slug].iter().copied().filter(
            |setting| {
                matches!(
                    setting.scope(),
                    crate::config::Scope::Everywhere | crate::config::Scope::Repository
                )
            },
        ));
        repository_settings.sort_by_key(|setting| {
            crate::config::EVERYWHERE_SETTINGS
                .iter()
                .chain(crate::config::REPOSITORY_SETTINGS)
                .position(|candidate| candidate == setting)
                .unwrap_or(usize::MAX)
        });
        repository_settings.dedup();
    }
    let asks_for_a_file = !here.as_os_str().is_empty()
        && skill::checkout_root(here).is_some()
        && chosen.first().is_some_and(|adapter| {
            changed_by_slug[adapter.slug]
                .iter()
                .any(|setting| setting.scope() == crate::config::Scope::Repository)
        });
    let repository_action = if !here.as_os_str().is_empty()
        && let Some(path) = match skill::repository_rows(here)? {
            Some((path, _)) => Some(path),
            None if asks_for_a_file => Some(skill::repository_config_path(here)),
            None => None,
        }
        && let Some((adapter, result)) = done.first_mut()
        && let Some(any) = plan.rows.get(adapter.slug)
    {
        if let Some(refusal) = injected_install_failure(InstallFailurePoint::AtRepository) {
            repository_failure = Some(refusal);
            false
        } else {
            let mut repository_config = any.clone();
            if let Some(held) = layers_before[adapter.slug].repository.as_ref() {
                let tightened: Vec<_> = repository_settings
                    .iter()
                    .copied()
                    .filter(|setting| setting.repository_may_tighten())
                    .collect();
                skill::apply_settings(&mut repository_config, held, &tightened)?;
            }
            match setup::write_repository_configuration_into(
                &path,
                &repository_config,
                &repository_settings,
                options.dry_run,
                &mut pending,
            ) {
                Ok(action) => {
                    result.actions.push(action);
                    repository_proven = !options.dry_run;
                    if injected_install_failure(InstallFailurePoint::RemoveRepositoryAfterWrite)
                        .is_some()
                    {
                        #[cfg(test)]
                        inject_repository_readback_removal();
                    }
                    if injected_install_failure(InstallFailurePoint::CorruptRepositoryAfterWrite)
                        .is_some()
                    {
                        #[cfg(test)]
                        inject_repository_readback_corruption();
                    }
                    if let Some(refusal) =
                        injected_install_failure(InstallFailurePoint::AfterRepository)
                    {
                        repository_failure = Some(refusal);
                        false
                    } else {
                        true
                    }
                }
                Err(refusal) => {
                    repository_failure = Some(refusal);
                    false
                }
            }
        }
    } else {
        false
    };
    if let Some(refusal) = repository_failure {
        return Err(install_failure(
            refusal,
            plan,
            options,
            &done,
            true,
            repository_proven,
            &completed,
        ));
    }

    let results: Vec<SetupResult> = done.iter().map(|(_, result)| result.clone()).collect();

    // Counted from what the run reported, not from what it was asked for.
    let files = results
        .iter()
        .flat_map(|result| &result.actions)
        .filter(|action| {
            !matches!(
                action.change,
                Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
            )
        })
        .map(|action| &action.path)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let verdict = format!(
        "{} into {} agent{}, {files} file{}",
        if options.dry_run {
            "would install"
        } else {
            "installed"
        },
        touched.len(),
        if touched.len() == 1 { "" } else { "s" },
        if files == 1 { "" } else { "s" },
    );
    let receipt = if options.dry_run {
        crate::tui::InstallReceipt::empty(verdict)
    } else {
        receipt_from_disk(
            plan,
            options,
            verdict,
            repository_action,
            touched.iter().map(|adapter| adapter.slug),
        )?
    };

    let unacknowledged = receipt
        .acknowledged
        .iter()
        .flat_map(|(slug, acknowledged)| {
            changed_by_slug[slug]
                .iter()
                .filter(|setting| !acknowledged.contains(setting))
                .map(|setting| setting.label())
        })
        .collect::<Vec<_>>();
    if !options.dry_run && !unacknowledged.is_empty() {
        let root = chosen
            .first()
            .map(|adapter| paths_by_slug[adapter.slug].skill_root.as_path())
            .unwrap_or_else(|| std::path::Path::new("."));
        return Err(crate::tui::InstallFailure {
            refusal: shadowed_table(root, &unacknowledged),
            receipt: Box::new(receipt),
        });
    }

    if json {
        return Ok((receipt, rendered_json(&results)));
    }

    let text = &mut describe_all(&done, false);
    line!(text, "");
    line!(text, "what this did, without the screen:");
    let defaults = Config::default();
    for adapter in &chosen {
        line!(
            text,
            "  estigia setup {} --allow-source-build",
            adapter.slug
        );
        let Some(theirs) = plan.rows.get(adapter.slug) else {
            continue;
        };
        for setting in crate::config::AGENT_SETTINGS {
            let chosen = setting.value_of(theirs);
            if chosen != setting.value_of(&defaults) {
                line!(
                    text,
                    "  estigia config set {:?} {chosen:?} --agent {}",
                    setting.label(),
                    adapter.slug
                );
            }
        }
    }
    // Once, and without the flag: a repository-wide row is a fact about the repository.
    if let Some(theirs) = chosen.first().and_then(|a| plan.rows.get(a.slug)) {
        for setting in crate::config::EVERYWHERE_SETTINGS {
            let value = setting.value_of(theirs);
            if value != setting.value_of(&defaults) {
                line!(text, "  estigia config set {:?} {value:?}", setting.label());
            }
        }
    }
    Ok((receipt, text.trim_end().to_owned()))
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallFailurePoint {
    AfterSetup,
    AfterOverride(usize),
    AtRepository,
    AfterRepository,
    RemoveRepositoryAfterWrite,
    CorruptRepositoryAfterWrite,
}

#[cfg(not(test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallFailurePoint {
    AfterSetup,
    AfterOverride(usize),
    AtRepository,
    AfterRepository,
    RemoveRepositoryAfterWrite,
    CorruptRepositoryAfterWrite,
}

#[cfg(test)]
std::thread_local! {
    static INSTALL_FAILURE: std::cell::Cell<Option<InstallFailurePoint>> = const { std::cell::Cell::new(None) };
    static REMOVE_REPOSITORY_READBACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CORRUPT_REPOSITORY_READBACK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
fn inject_install_failure(point: InstallFailurePoint) {
    INSTALL_FAILURE.with(|injected| injected.set(Some(point)));
}

#[cfg(test)]
fn inject_repository_readback_removal() {
    REMOVE_REPOSITORY_READBACK.with(|remove| remove.set(true));
}

#[cfg(test)]
fn inject_repository_readback_corruption() {
    CORRUPT_REPOSITORY_READBACK.with(|corrupt| corrupt.set(true));
}

#[cfg(test)]
fn alter_repository_before_readback(repository: &std::path::Path) {
    REMOVE_REPOSITORY_READBACK.with(|remove| {
        if remove.replace(false) {
            std::fs::remove_file(skill::repository_config_path(repository))
                .expect("the injected repository readback is removable");
        }
    });
    CORRUPT_REPOSITORY_READBACK.with(|corrupt| {
        if corrupt.replace(false) {
            std::fs::write(
                skill::repository_config_path(repository),
                [0xff, 0xfe, 0xfd],
            )
            .expect("the injected repository readback is corruptible");
        }
    });
}

#[cfg(not(test))]
fn alter_repository_before_readback(_: &std::path::Path) {}

#[cfg(test)]
fn injected_install_failure(point: InstallFailurePoint) -> Option<Refusal> {
    INSTALL_FAILURE.with(|injected| {
        (injected.get() == Some(point)).then(|| {
            injected.set(None);
            Refusal::not_started(
                "setup-write-failed",
                format!("injected failure at {point:?}"),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "the test removes this injected obstacle",
                ),
            )
        })
    })
}

#[cfg(not(test))]
fn injected_install_failure(_: InstallFailurePoint) -> Option<Refusal> {
    None
}

fn receipt_from_disk(
    plan: &crate::tui::Plan,
    options: &SetupOptions,
    summary: String,
    repository_proven: bool,
    completed: impl IntoIterator<Item = &'static str>,
) -> Result<crate::tui::InstallReceipt, crate::tui::InstallFailure> {
    let mut receipt = crate::tui::InstallReceipt::empty(summary);
    receipt.completed.extend(completed);
    let repository_snapshot = if repository_proven && !plan.repository.as_os_str().is_empty() {
        alter_repository_before_readback(&plan.repository);
        skill::repository_snapshot(&plan.repository)
            .and_then(|snapshot| {
                snapshot.ok_or_else(|| {
                    Refusal {
                        code: "repository-readback-missing",
                        message: format!(
                            "{} is missing after its repository write was proved",
                            skill::repository_config_path(&plan.repository).display()
                        ),
                        outcome: crate::outcome::MutationOutcome::Unknown,
                        class: crate::harness::refusal_class::class_of_code("repository-readback-missing"),
                        replay: crate::outcome::Replayability::StatusRequired,
                        resolution: Resolution::no_command(
                            crate::outcome::NoCommandReason::OperatorKnowledge,
                            "read the repository configuration path named above before deciding whether to retry",
                        ),
                        observed: None,
                    }
                })
            })
            .map(Some)
            .map_err(|mut refusal| {
            refusal.message = format!(
                "written, and the repository layer does not read back: {}",
                refusal.message
            );
            refusal.outcome = crate::outcome::MutationOutcome::Unknown;
            refusal.replay = crate::outcome::Replayability::StatusRequired;
            crate::tui::InstallFailure {
                refusal,
                receipt: Box::new(receipt.clone()),
            }
        })?
    } else {
        None
    };
    for adapter in &plan.agents {
        let paths =
            setup::resolve_paths(adapter, options).map_err(|_| crate::tui::InstallFailure {
                refusal: skill::no_skill_root(),
                receipt: Box::new(receipt.clone()),
            })?;
        let contract = skill::contract_config(&paths.skill_root).map_err(|mut refusal| {
            refusal.message = format!(
                "written, and the contract does not read back: {}",
                refusal.message
            );
            refusal.outcome = crate::outcome::MutationOutcome::Committed;
            refusal.replay = crate::outcome::Replayability::NotReplayable;
            crate::tui::InstallFailure {
                refusal,
                receipt: Box::new(receipt.clone()),
            }
        })?;
        receipt.contract_read_back.insert(adapter.slug, contract);
        let layers = skill::config_layers(&paths.skill_root, Some(adapter.slug), None).map_err(
            |mut refusal| {
                refusal.message = format!(
                    "written, and the table does not read back: {}",
                    refusal.message
                );
                refusal.outcome = crate::outcome::MutationOutcome::Committed;
                refusal.replay = crate::outcome::Replayability::NotReplayable;
                crate::tui::InstallFailure {
                    refusal,
                    receipt: Box::new(receipt.clone()),
                }
            },
        )?;
        if let Some(agent) = &layers.agent {
            receipt.agent_read_back.insert(adapter.slug, agent.clone());
        }
        if let Some(local) = &layers.local {
            receipt.local_read_back.insert(adapter.slug, local.clone());
        }
        receipt
            .unlayered_read_back
            .insert(adapter.slug, layers.unlayered().clone());
        let repository_layer = repository_snapshot
            .as_ref()
            .map(|snapshot| snapshot.layer_over(layers.unlayered()))
            .transpose()
            .map_err(|mut refusal| {
                refusal.message = format!(
                    "written, and the repository layer does not read back: {}",
                    refusal.message
                );
                refusal.outcome = crate::outcome::MutationOutcome::Unknown;
                refusal.replay = crate::outcome::Replayability::StatusRequired;
                crate::tui::InstallFailure {
                    refusal,
                    receipt: Box::new(receipt.clone()),
                }
            })?;
        let effective = repository_layer
            .as_ref()
            .map_or_else(|| layers.unlayered().clone(), |layer| layer.config.clone());
        receipt.read_back.insert(adapter.slug, effective.clone());
        let acknowledged = crate::config::SETTINGS
            .iter()
            .copied()
            .filter(|setting| {
                plan.opened.get(adapter.slug).is_none_or(|opened| {
                    setting.value_of(opened) != setting.value_of(&plan.rows[adapter.slug])
                }) && setting.value_of(&effective) == setting.value_of(&plan.rows[adapter.slug])
            })
            .collect::<Vec<_>>();
        receipt.acknowledged.insert(adapter.slug, acknowledged);
        if let Some(repository_layer) = repository_layer
            && receipt.repository.is_none()
        {
            receipt.repository = Some(repository_layer.config);
            receipt.repository_settings = repository_layer.settings;
        }
    }
    Ok(receipt)
}

fn install_failure(
    mut refusal: Refusal,
    plan: &crate::tui::Plan,
    options: &SetupOptions,
    done: &[(&'static AgentAdapter, SetupResult)],
    write_attempted: bool,
    repository_proven: bool,
    completed: &std::collections::BTreeSet<&'static str>,
) -> crate::tui::InstallFailure {
    let crossed = !options.dry_run
        && done.iter().any(|(_, result)| {
            result.actions.iter().any(|action| {
                !matches!(
                    action.change,
                    Change::Unchanged | Change::Kept | Change::Shared | Change::Unrecorded
                )
            })
        });
    if crossed {
        refusal.outcome = crate::outcome::MutationOutcome::Committed;
        refusal.replay = crate::outcome::Replayability::ExactReplaySafe;
    } else if !options.dry_run
        && write_attempted
        && refusal.outcome == crate::outcome::MutationOutcome::NotStarted
    {
        refusal.outcome = crate::outcome::MutationOutcome::Unknown;
        refusal.replay = crate::outcome::Replayability::StatusRequired;
    }
    let mut partial_plan = plan.clone();
    partial_plan.agents = done.iter().map(|(adapter, _)| *adapter).collect();
    let receipt = if options.dry_run {
        crate::tui::InstallReceipt::empty(String::new())
    } else {
        match receipt_from_disk(
            &partial_plan,
            options,
            String::new(),
            repository_proven,
            completed.iter().copied(),
        ) {
            Ok(receipt) => receipt,
            Err(mut readback_failure) => {
                if repository_proven
                    && readback_failure.refusal.outcome == crate::outcome::MutationOutcome::Unknown
                {
                    readback_failure.refusal.message = format!(
                        "{}; and {}",
                        refusal.message, readback_failure.refusal.message
                    );
                    return readback_failure;
                }
                *readback_failure.receipt
            }
        }
    };
    crate::tui::InstallFailure {
        refusal,
        receipt: Box::new(receipt),
    }
}

/// Which adapters a command applies to.
///
/// `installing` refuses a retired adapter — `setup` and `sync` must not write
/// into a host Estigia has dropped — while `uninstall` still reaches it, which is
/// the whole reason its entry is kept.
fn select(
    agent: Option<&str>,
    all: bool,
    installing: bool,
) -> Result<Vec<&'static AgentAdapter>, Refusal> {
    match (agent, all, installing) {
        (Some(slug), _, true) => Ok(vec![setup::find_installable_agent(slug)?]),
        (Some(slug), _, false) => Ok(vec![find_agent(slug)?]),
        (None, true, true) => Ok(AGENTS.iter().collect()),
        (None, true, false) => Ok(setup::every_adapter().collect()),
        (None, false, _) => Err(setup::no_agent_named()),
    }
}

/// The adapters Estigia is installed in, by any of the three ways it can be.
fn configured(options: &SetupOptions) -> Vec<&'static AgentAdapter> {
    AGENTS
        .iter()
        .filter(|adapter| setup::is_present(adapter, options))
        .collect()
}

/// The distinct contracts already written on this machine, whatever this
/// run's own targets are.
///
/// The population is `targets` **unioned with** [`configured`], not `targets`
/// alone. A named agent's own target list is one adapter, and a one-element
/// list can never disagree with itself — read that way, a fresh root's only
/// neighbour was never consulted, and `estigia setup <slug>` for an adapter
/// with no contract yet wrote `Config::default()` in silence rather than the
/// machine's already-installed answers. `configured` is a union and not a
/// replacement: it filters on [`setup::is_present`], and a partially-installed
/// adapter that still holds a readable contract must keep contributing it.
///
/// Read as the **contract**, through [`skill::contract_config_folding_retired`],
/// and not as what each adapter actually reads. A retired `Delegated workers`
/// row folds rather than refusing, so a contract that still carries one keeps
/// contributing its answer instead of dropping out of the population.
/// `skill::installed_config_for` remains
/// the readability gate: a root whose contract exists but will not parse
/// contributes nothing here, skipped outright — which is not what
/// `existing_config` does with the same condition. `existing_config` returns
/// that case as `Err` rather than as an absence, because writing over the
/// root that holds an unreadable override is the one thing nothing may do.
/// Skipping it here instead is a deliberate difference, not an oversight:
/// this is gathering a population, and one neighbour's unreadable override
/// must not stop every other adapter from contributing an answer to the
/// fold.
///
/// Once a root does read, its value is `unlayered()`, which prefers
/// `estigia.local.md`, read with no scope filter, so it can carry an
/// `Everywhere`- or `Machine`-scoped row. `existing_config` already refuses
/// to promote that value into a versioned block for the adapter's own
/// write; this population feeds the very fold that writes one, for a
/// *different* adapter, so it has to make the same refusal — reading
/// [`skill::contract_config`] instead. Dropping the per-agent layer
/// alongside it costs nothing here either — that layer can only carry
/// `Agent`-scoped rows, which the fold below never inherits — and it is the
/// more accurate de-duplicated count for a note whose subject is contracts,
/// not overrides.
///
/// De-duplicated, because most adapters share the neutral root and reading one
/// file through two adapters is not two answers. A root with no contract
/// contributes nothing: it has no answer yet.
fn installed_configs(targets: &[&'static AgentAdapter], options: &SetupOptions) -> Vec<Config> {
    let mut found: Vec<Config> = Vec::new();
    for adapter in targets.iter().copied().chain(configured(options)) {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        if skill::installed_config_for(&paths.skill_root, Some(adapter.slug)).is_err() {
            continue;
        }
        let Ok(config) = skill::contract_config_folding_retired(&paths.skill_root) else {
            continue;
        };
        if !found.contains(&config) {
            found.push(config);
        }
    }
    found
}

/// The configuration to write for one adapter.
///
/// Reading it back is what makes `setup` re-runnable and `sync` safe: an
/// operator who configured `squash` a month ago does not get `merge commit`
/// because the tool was upgraded.
///
/// It is read from **that adapter's own skill root**. It used to be read once,
/// from the first root that answered, and handed to every target — so two agents
/// deliberately configured differently ended the run agreeing, the second
/// silently rewritten to the first one's table. Nobody was asked and nobody was
/// told, which is the quiet overwrite this whole tool exists to refuse.
///
/// A root with no contract has no answer of its own, so it inherits **per
/// row**, through [`inherited_from_shared`]: an `Everywhere`- or
/// `Machine`-scoped setting every installed contract agrees on is folded in,
/// and a setting they disagree on — or every setting, when nothing else is
/// installed at all — gets the portable default rather than a guess.
/// [`report_fresh_roots`] says which rows, if any, fell back.
/// The contract already installed for this adapter, when there is one to keep.
/// Three answers where the code had two. `Ok(Some)` is an answer to preserve.
/// `Ok(None)` is no contract yet, which inherits. `Err` is a contract that is
/// *there* and will not parse — not an absence, and the one case nothing may
/// write over: the rows exist and cannot be read, so nothing can say what
/// writing would cost.
fn existing_config(
    adapter: &'static AgentAdapter,
    options: &SetupOptions,
) -> Result<Option<Config>, Refusal> {
    let Ok(paths) = setup::resolve_paths(adapter, options) else {
        return Ok(None);
    };
    match skill::installed_config_for(&paths.skill_root, Some(adapter.slug)) {
        // Read layered, returned unlayered: the read must layer, so a bad row in
        // `estigia.local.md` refuses here; what is written must not, because this
        // value renders the versioned block. A *retired* row is not that case:
        // its meaning is defined, and the write below is what takes it out of
        // the table, so it folds rather than refusing.
        Ok(_) => skill::contract_config_folding_retired(&paths.skill_root).map(Some),
        Err(refusal) if refusal.code != "skill-not-installed" => Err(refusal),
        Err(_) => Ok(None),
    }
}

fn config_for(
    adapter: &'static AgentAdapter,
    shared: &[Config],
    options: &SetupOptions,
) -> Result<Config, Refusal> {
    // Its own answer wins over anything inherited: `existing_config` returns
    // `Some` only when this adapter's own root already carries a contract.
    // A contract that will not parse travels out as the refusal it is — one bad
    // row in the operator's own `estigia.local.md`, a file this never edits,
    // cost them every choice in a file it does: `squash` and `Loop plugin on`
    // came back `merge commit` and `off`, under one line reading `update`.
    if let Some(own) = existing_config(adapter, options)? {
        return Ok(own);
    }
    Ok(inherited_from_shared(shared).0)
}

/// Folds the installed contracts' agreed answers into a fresh [`Config`], row
/// by row, and names the rows that did not fold — and why.
///
/// A whole-`Config` equality check — the fold this replaced — could hold on a
/// real machine only by coincidence: it requires every `Agent`-scoped row
/// (delegated workers, delegated models, who plans) to match too, and those are meant to
/// differ per agent. So only `Everywhere`- and `Machine`-scoped rows are
/// eligible here; an `Agent`-scoped row stays at the portable default no
/// matter what the installed contracts hold, because inheriting one from a
/// neighbour would hand a fresh adapter another agent's answer to a question
/// this adapter has to answer for itself.
///
/// For each eligible row, in [`SETTINGS`] order: if every config in `shared`
/// renders the same value, that value is applied onto the result. Two
/// different reasons a row can still end up at the default, kept apart rather
/// than folded into one list: the configs actually **disagreed**, or they
/// agreed and the agreed value still failed to `apply` — read back through
/// the same `Setting` that rendered it, which
/// `the_settings_fold_round_trips_every_everywhere_and_machine_row` pins as not
/// happening for the one sample it checks. Reporting the second as the first
/// would have a message assert a disagreement that was never observed.
fn inherited_from_shared(shared: &[Config]) -> (Config, Vec<Setting>, Vec<Setting>) {
    let mut result = Config::default();
    let mut disagreed = Vec::new();
    let mut rejected = Vec::new();
    for &setting in SETTINGS {
        if !matches!(setting.scope(), Scope::Everywhere | Scope::Machine) {
            continue;
        }
        let mut values = shared.iter().map(|config| setting.value_of(config));
        let agreed = match values.next() {
            Some(first) if values.all(|value| value == first) => Some(first),
            _ => None,
        };
        match agreed {
            None => disagreed.push(setting),
            Some(value) => {
                if setting.apply(&mut result, &value).is_err() {
                    rejected.push(setting);
                }
            }
        }
    }
    (result, disagreed, rejected)
}

/// The lines [`report_fresh_roots`] prints, built without touching the world.
///
/// Split out so a test can read the exact wording a fresh root gets rather
/// than only trusting that `note!` fired: a message nothing asserts is a
/// message that drifts, and the silence this function replaces was half of
/// the defect named. Same lines, same order, whichever caller
/// reads them.
///
/// Not a refusal: every adapter that has an answer keeps it, so the run is
/// correct either way. What an operator cannot be left to discover on their
/// own is which rows a root with no contract did **not** inherit — either
/// because nothing else is installed at all, or because the installed
/// contracts disagree on those specific rows.
///
/// Every line stands on its own. The aggregate "N different configurations"
/// line used to be the only header a per-root line's leading two spaces made
/// sense under, so a fresh root on an otherwise-empty or fully-agreeing
/// machine — the very first command a new user runs — printed an indented
/// line with no header above it. Each line here opens with `note:` itself,
/// whether or not the aggregate line also printed.
///
/// Counted by skill **root**, not by adapter, and read with the run's own
/// options. Most adapters share the neutral root, so naming adapters printed
/// "agents, opencode, gemini-cli, cursor, continue, cline, crush, windsurf, qwen
/// had none" for a *single file* — a line that reads like nine decisions and
/// describes one. It also asked `is_configured` with `SetupOptions::default()`
/// rather than the options in hand, which is the wrong home and the wrong
/// question: a directive is not a contract.
fn fresh_root_notes(
    shared: &[Config],
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
) -> Vec<String> {
    let mut fresh: Vec<String> = Vec::new();
    for adapter in targets {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        if skill::installed_config(&paths.skill_root).is_ok() {
            continue;
        }
        let root = paths.skill_root.display().to_string();
        if !fresh.contains(&root) {
            fresh.push(root);
        }
    }
    if fresh.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    if shared.len() >= 2 {
        lines.push(format!(
            "note: {} different configurations are installed on this machine, and each keeps its \
             own.",
            shared.len()
        ));
    }
    let (_, disagreed, rejected) = inherited_from_shared(shared);
    for root in &fresh {
        if shared.is_empty() {
            lines.push(format!(
                "note: {root} had none, and nothing else is installed, so every row that is not \
                 this agent's own took the default — `estigia config set` names one."
            ));
        } else if disagreed.is_empty() && rejected.is_empty() {
            lines.push(format!(
                "note: {root} had none, so it took the machine's answers for every row that is \
                 not this agent's own."
            ));
        } else {
            let disagreed_names = disagreed
                .iter()
                .map(|setting| setting.label())
                .collect::<Vec<_>>()
                .join(", ");
            let rejected_names = rejected
                .iter()
                .map(|setting| setting.label())
                .collect::<Vec<_>>()
                .join(", ");
            let reason = match (disagreed.is_empty(), rejected.is_empty()) {
                (false, true) => {
                    format!(
                        "except: {disagreed_names} — these disagree, so `estigia config set` names one."
                    )
                }
                (true, false) => format!(
                    "except: {rejected_names} — those failed to read back, which is not a \
                     disagreement and should not happen; `estigia config set` names one."
                ),
                _ => format!(
                    "except: {disagreed_names} — these disagree; and {rejected_names} — those \
                     failed to read back, which is not a disagreement and should not happen; \
                     `estigia config set` names one."
                ),
            };
            lines.push(format!(
                "note: {root} had none, so it took the machine's answers for every row that is \
                 not this agent's own, {reason}"
            ));
        }
    }
    lines
}

/// Says what a fresh skill root took, before anything is written.
///
/// The thin half of [`fresh_root_notes`]: every line that function built,
/// `note!`d in order.
fn report_fresh_roots(
    shared: &[Config],
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
) {
    for line in fresh_root_notes(shared, targets, options) {
        note!("{line}");
    }
}

/// Runs setup across every target without discarding a failed adapter's proof.
fn run_setup_over(
    targets: &[&'static AgentAdapter],
    shared: &[Config],
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
    let recorded = setup::install_root_records(options);
    let mut pending = setup::Pending::new();
    let mut done = Vec::with_capacity(targets.len());
    let mut refused = Vec::new();
    for adapter in targets {
        let config = match config_for(adapter, shared, options) {
            Ok(config) => config,
            Err(error) => {
                refused.push(Refusal {
                    message: format!("{}: {}", adapter.display_name, error.message),
                    ..error
                });
                continue;
            }
        };
        match setup::setup_into_evidenced(adapter, &config, options, &mut pending) {
            Ok(result) => done.push((*adapter, result)),
            Err(failure) => {
                let refusal = classified_setup_failure(adapter, &failure);
                done.push((*adapter, failure.result));
                refused.push(refusal);
            }
        }
    }
    let reported = report_over(&done, refused, json, false);
    name_new_install_roots(options, &recorded, json);
    reported
}

fn name_new_install_roots(options: &SetupOptions, before: &[std::path::PathBuf], json: bool) {
    if json || options.dry_run {
        return;
    }
    for path in setup::install_root_records(options) {
        if !before.contains(&path) {
            say!("  create  {}", path.display());
        }
    }
}

fn run_setup_evidenced_over(
    targets: &[&'static AgentAdapter],
    json: bool,
    mut operation: impl FnMut(
        &'static AgentAdapter,
    ) -> std::result::Result<SetupResult, setup::SetupFailure>,
) -> Result<(), Refusal> {
    let mut done = Vec::with_capacity(targets.len());
    let mut refused = Vec::new();
    for adapter in targets {
        match operation(adapter) {
            Ok(result) => done.push((*adapter, result)),
            Err(failure) => {
                refused.push(classified_setup_failure(adapter, &failure));
                done.push((*adapter, failure.result));
            }
        }
    }
    report_over(&done, refused, json, false)
}

fn classified_setup_failure(adapter: &AgentAdapter, failure: &setup::SetupFailure) -> Refusal {
    let mut refusal = setup_failure_refusal(adapter, failure);
    if failure.phase == setup::SetupFailurePhase::Mutation
        && !failure.result.dry_run
        && failure.result.changed_files() > 0
    {
        refusal.outcome = crate::outcome::MutationOutcome::Committed;
        refusal.replay = crate::outcome::Replayability::ExactReplaySafe;
    }
    refusal
}

/// Prints one batch report and returns its aggregate refusal, when any.
fn report_over(
    done: &[(&'static AgentAdapter, SetupResult)],
    refused: Vec<Refusal>,
    json: bool,
    taking_out: bool,
) -> Result<(), Refusal> {
    let results: Vec<SetupResult> = done.iter().map(|(_, result)| result.clone()).collect();
    if json {
        print_json(&results);
    } else {
        say!("{}", describe_all(done, taking_out).trim_end());
    }
    match refusal_over(done, refused) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// What an uninstall did, per agent, and which refusals stopped it.
pub(crate) type TakenOut = (
    Vec<(&'static AgentAdapter, SetupResult)>,
    Vec<Refusal>,
    Vec<std::path::PathBuf>,
);

/// The uninstall `uninstall`, `setup --uninstall` and the screen all run.
pub(crate) fn uninstall_agents(
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
) -> TakenOut {
    let mut pending = setup::Pending::new();
    let recorded = setup::install_root_records(options);
    let (done, refused) = collect_over(targets, |adapter| {
        setup::uninstall_from(adapter, options, &mut pending)
    });
    let dropped: Vec<std::path::PathBuf> = if options.dry_run {
        Vec::new()
    } else {
        let left = setup::install_root_records(options);
        recorded
            .into_iter()
            .filter(|path| !left.contains(path))
            .collect()
    };
    // Estigia's own state, removed once no agent is left to read it.
    let mut state = dropped;
    state.extend(setup::forget_state(options));
    (done, refused, state)
}

/// Takes Estigia out of every named agent, and its own state with it.
fn take_out(
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
    let (done, refused, state) = uninstall_agents(targets, options);
    let results: Vec<SetupResult> = done.iter().map(|(_, result)| result.clone()).collect();
    let left = kept_by_the_operator(targets, options, &results);
    if json {
        // One document, not two: prose after the array is not parseable.
        print_json(&serde_json::json!({
            "agents": results,
            "state": state
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
            "kept": left
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        }));
    } else {
        say!("{}", describe_all(&done, true).trim_end());
        for path in &state {
            say!(
                "  {}  {}",
                if options.dry_run {
                    "would remove"
                } else {
                    "remove"
                },
                path.display()
            );
        }
        let unreadable = setup::unreadable_install_roots(options);
        if !unreadable.is_empty() {
            say!(
                "  note: Estigia's state was kept because {} install record(s) could not be \
                 read, so another configuration root may still use it: {}",
                unreadable.len(),
                unreadable
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !left.is_empty() {
            say!(
                "  note: {} file(s) in that directory are not Estigia's and {}, \
                 which is why it is still on disk: {}",
                left.len(),
                if options.dry_run {
                    "would be left where they are"
                } else {
                    "were left there"
                },
                left.iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        say!(
            "  note: `estigia guard` writes a pre-push hook into a repository, and no record \
             is kept of which \u{2014} so any it wrote {}. Run `estigia guard --uninstall` in \
             each repository you installed it in; it only removes a hook Estigia wrote.",
            if options.dry_run {
                "would stay where they are"
            } else {
                "are still there"
            }
        );
    }
    match refusal_over(&done, refused) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// The files still in the skill directories that Estigia did not install.
fn kept_by_the_operator(
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
    results: &[SetupResult],
) -> Vec<std::path::PathBuf> {
    let spoken: std::collections::BTreeSet<&std::path::Path> = results
        .iter()
        .flat_map(|result| result.actions.iter())
        .map(|action| action.path.as_path())
        .collect();
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    for adapter in targets {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        // Most adapters share one root, so walking it per adapter would name
        // the operator's file more than once.
        if roots.contains(&paths.skill_root) {
            continue;
        }
        roots.push(paths.skill_root.clone());
        let mut here = Vec::new();
        collect_files(&paths.skill_root, &mut here);
        // The install record is Estigia's, and surviving agents still read it.
        let ledger = skill::record::path(&paths.skill_root);
        here.retain(|path| {
            // And not a staged write left behind by a killed process.
            *path != ledger
                && !spoken.contains(path.as_path())
                && !crate::paths::is_staged_write(path)
        });
        if !options.dry_run {
            for staged in std::fs::read_dir(&paths.skill_root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| crate::paths::is_staged_write(path))
            {
                let _ = std::fs::remove_file(staged);
            }
        }
        found.append(&mut here);
    }
    found.sort();
    found
}

/// Every file under `root`, however deep.
fn collect_files(root: &std::path::Path, into: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, into);
        } else {
            into.push(path);
        }
    }
}

/// Runs the operation over every target and hands back what each one did.
fn collect_over(
    targets: &[&'static AgentAdapter],
    mut operation: impl FnMut(&'static AgentAdapter) -> Result<SetupResult>,
) -> (Vec<(&'static AgentAdapter, SetupResult)>, Vec<Refusal>) {
    let mut done = Vec::with_capacity(targets.len());
    let mut refused = Vec::new();
    for adapter in targets {
        match operation(adapter) {
            Ok(result) => done.push((*adapter, result)),
            Err(error) => refused.push(setup_failed(adapter, &error)),
        }
    }
    (done, refused)
}

/// The one refusal a partly-done batch answers with.
pub(crate) fn refusal_over(
    done: &[(&'static AgentAdapter, SetupResult)],
    refused: Vec<Refusal>,
) -> Option<Refusal> {
    let first = refused.first()?;
    let controlling = refused
        .iter()
        .find(|refusal| refusal.outcome == crate::outcome::MutationOutcome::Unknown)
        .unwrap_or(first);
    let unknown = controlling.outcome == crate::outcome::MutationOutcome::Unknown;
    let wrote = done
        .iter()
        .any(|(_, result)| !result.dry_run && result.changed_files() > 0);
    let mut message = refused
        .iter()
        .map(|refusal| refusal.message.as_str())
        .collect::<Vec<_>>()
        .join("; and ");
    // What happened to the rest of the batch, for the reader handed one refusal.
    let done = done
        .iter()
        .filter(|(_, result)| result.completed && !result.dry_run && result.changed_files() > 0)
        .count();
    if done > 0 {
        message.push_str(&format!(
            " \u{2014} the other {done} agent(s) were done, and what each changed is above"
        ));
    }
    Some(Refusal {
        message,
        outcome: if unknown {
            crate::outcome::MutationOutcome::Unknown
        } else if wrote {
            crate::outcome::MutationOutcome::Committed
        } else {
            controlling.outcome
        },
        replay: if unknown {
            crate::outcome::Replayability::StatusRequired
        } else {
            controlling.replay
        },
        ..controlling.clone()
    })
}

/// Every adapter's report, in the order they were run.
fn describe_all(done: &[(&'static AgentAdapter, SetupResult)], taking_out: bool) -> String {
    let mut moved: std::collections::BTreeSet<&std::path::Path> = std::collections::BTreeSet::new();
    let mut text = String::new();
    for (adapter, result) in done {
        text.push_str(&describe(adapter, result, &moved, taking_out));
        for action in &result.actions {
            if !matches!(action.change, Change::Unchanged) {
                moved.insert(action.path.as_path());
            }
        }
    }
    text
}

fn describe(
    adapter: &AgentAdapter,
    result: &SetupResult,
    moved: &std::collections::BTreeSet<&std::path::Path>,
    taking_out: bool,
) -> String {
    let text = &mut String::new();
    let changed = result.changed_files();
    if !result.dry_run && !result.completed && changed == 0 {
        line!(
            text,
            "{} — setup stopped before changing any files",
            adapter.display_name
        );
        for note in &result.notes {
            line!(text, "  note: {note}");
        }
        return text.clone();
    }
    let verb = if result.dry_run {
        "would change"
    } else if !result.completed {
        "changed before setup stopped"
    } else {
        "changed"
    };
    line!(text, "{} — {verb} {changed} file(s)", adapter.display_name);
    for note in &result.notes {
        line!(text, "  note: {note}");
    }
    for action in &result.actions {
        let Some(mark) = word(action.change, result.dry_run) else {
            continue;
        };
        line!(text, "  {mark:<13} {}", action.path.display());
    }
    if changed == 0 {
        if result
            .actions
            .iter()
            .any(|action| moved.contains(action.path.as_path()))
        {
            line!(
                text,
                "  no file of its own; it shares the ones listed above"
            );
        } else if taking_out {
            line!(text, "  nothing of Estigia's is here");
        } else {
            line!(text, "  already current");
        }
    }
    let count = |wanted| {
        result
            .actions
            .iter()
            .filter(|action| action.change == wanted)
            .count()
    };
    let what = if result.dry_run {
        "would be written over. Nothing has been yet"
    } else {
        "were written over"
    };
    if count(Change::Replace) > 0 {
        line!(
            text,
            "  note: {} file(s) were already here and are not Estigia's, and {what}. Estigia \
             installs its own copy of the skill, and taking it back out leaves that copy rather \
             than what was there \u{2014} nothing keeps the old one.",
            count(Change::Replace)
        );
    }
    if count(Change::Overwrite) > 0 {
        line!(
            text,
            "  note: {} file(s) Estigia wrote had been edited since, and {what}. The edit is \
             gone; keep changes in `{}` beside the contract, which Estigia never writes.",
            count(Change::Overwrite),
            crate::config::LOCAL_FILE
        );
    }
    if count(Change::Kept) > 0 {
        line!(
            text,
            "  note: {} file(s) were here before Estigia installed over them, so they stay. \
             Estigia removes what it created.",
            count(Change::Kept)
        );
    }
    if count(Change::Unrecorded) > 0 {
        line!(
            text,
            "  note: {} file(s) stay because Estigia has no record of installing here. An \
             install writes one, so this directory was filled by something else, by a build \
             from before the record, or after somebody removed it. Estigia will not guess \
             on the way out: delete the directory yourself if you know it is its.",
            count(Change::Unrecorded)
        );
    }
    let emptied: Vec<&setup::SetupAction> = result
        .actions
        .iter()
        .filter(|action| {
            action.kind == setup::ActionKind::Directive
                && action.change == Change::Update
                && std::fs::metadata(&action.path).is_ok_and(|found| found.len() == 0)
        })
        .collect();
    if !result.dry_run && !emptied.is_empty() {
        line!(
            text,
            "  note: {} instruction file(s) are empty and still on disk, because nothing records \
             that Estigia created them \u{2014} an install from a build before the record says \
             nothing either way, and a file that may be yours is not Estigia's to delete: {}",
            emptied.len(),
            emptied
                .iter()
                .map(|action| action.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if count(Change::Shared) > 0 {
        line!(
            text,
            "  note: {} file(s) are the skill, and another configured agent reads the same \
             copy — it goes out with the last one. Run: estigia status",
            count(Change::Shared)
        );
    }
    if !adapter.discovers_skills() {
        line!(
            text,
            "  note: {} is not known to read a skill directory of its own, so the skill was \
             installed in the shared location and the directive names its path.",
            adapter.display_name
        );
    }
    std::mem::take(text)
}

/// The same JSON, as text, for a caller that has somewhere else to put it.
fn rendered_json<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(error) => format!("could not serialize the result: {error}"),
    }
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => say!("{text}"),
        Err(error) => eprintln!("estigia: could not serialize the result: {error}"),
    }
}

/// Detects a companion and says what to do about it.
///
/// Detection only — nothing is downloaded and nothing is installed. That is
/// what keeps the trust surface of the default path at zero, and why a
/// companion's own installer is never reimplemented here.
fn show_companion(slug: &str) -> Result<(), Refusal> {
    let Some(companion) = setup::find_companion(slug) else {
        let known = COMPANIONS
            .iter()
            .map(|companion| companion.slug)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Refusal::not_started(
            "companion-unknown",
            format!("{slug:?} is not a companion Estigia knows"),
            Resolution::run(format!("estigia setup --companion {known}")),
        ));
    };
    say!("{}", setup::probe_companion(companion).describe(companion));
    Ok(())
}

fn lifecycle_status(options: &SetupOptions) -> Result<lifecycle::Status, Refusal> {
    let state = options
        .home_dir
        .as_deref()
        .map_or_else(lifecycle::StateRoot::current, |home| {
            Ok(lifecycle::StateRoot::under(home))
        })?;
    let executable = std::env::current_exe().map_err(|error| {
        Refusal::not_started(
            "executable-not-resolvable",
            format!("the running executable could not be resolved: {error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a readable path to the running Estigia executable",
            ),
        )
    })?;
    Ok(lifecycle::Status::inspect_executable(&state, executable))
}

fn record_install(json: bool) -> Result<(), Refusal> {
    let state = lifecycle::StateRoot::current()?;
    let candidate = std::env::current_exe().map_err(lifecycle::StateError::Executable)?;
    match state.record_installer_install(&candidate) {
        Ok(()) => {
            if json {
                print_json(&serde_json::json!({ "recorded": true }));
            } else {
                say!("installer lifecycle recorded");
            }
            Ok(())
        }
        Err(lifecycle::StateError::Downgrade {
            candidate,
            high_water,
        }) => Err(Refusal::not_started(
            "installer-downgrade-blocked",
            format!(
                "installer candidate {candidate} is below this machine's recorded high-water {high_water}"
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "an official installer candidate at or above the recorded high-water",
            ),
        )),
        Err(lifecycle::StateError::ReleaseAfterProvenance(error)) => Err(Refusal {
            code: "lifecycle-release-publication-failed",
            message: error.to_string(),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code(
                "lifecycle-release-publication-failed",
            ),
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "inspect the lifecycle evidence under ~/.estigia/lifecycle before retrying the installer",
            ),
            observed: None,
        }),
        Err(error) => Err(error.into()),
    }
}

impl From<lifecycle::StateError> for Refusal {
    fn from(error: lifecycle::StateError) -> Self {
        Refusal::not_started(
            "lifecycle-state-unreadable",
            error.to_string(),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the lifecycle evidence under ~/.estigia/lifecycle readable and valid; do not delete or overwrite malformed evidence",
            ),
        )
    }
}

fn lifecycle_preflight(
    options: &SetupOptions,
    allow_source_build: bool,
    resolution: Resolution,
) -> Result<(), Refusal> {
    let status = lifecycle_status(options)?;
    match status.relation {
        lifecycle::Relation::SourceOrUnrecorded if !allow_source_build => {
            Err(Refusal::not_started(
                "source-build-not-allowed",
                "this build has no matching observed-path installer record, so it may not rewrite installed assets by default",
                resolution,
            ))
        }
        lifecycle::Relation::SourceOrUnrecorded => Ok(()),
        lifecycle::Relation::DowngradeBlocked => Err(Refusal::not_started(
            "recorded-downgrade-blocked",
            format!(
                "installer-recorded release {} is below this machine's recorded high-water {}",
                match status.provenance {
                    lifecycle::Provenance::InstallerRecorded { version } => version.to_string(),
                    _ => "unknown".to_owned(),
                },
                status
                    .high_water
                    .map_or_else(|| "unknown".to_owned(), |version| version.to_string())
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "an installer-recorded Estigia release at or above the recorded high-water",
            ),
        )),
        lifecycle::Relation::Unknown => Err(Refusal::not_started(
            "lifecycle-state-unreadable",
            status
                .state_error
                .unwrap_or_else(|| "lifecycle state could not be read safely".to_owned()),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the lifecycle evidence under ~/.estigia/lifecycle readable and valid; do not delete or overwrite malformed evidence",
            ),
        )),
        lifecycle::Relation::Current
        | lifecycle::Relation::AheadOfRecorded
        | lifecycle::Relation::RecordedNoHistory => Ok(()),
    }
}

fn update(json: bool) -> Result<(), Refusal> {
    let status = lifecycle_status(&SetupOptions::default())?;
    if json {
        print_json(&status);
        return Ok(());
    }
    say!("estigia update — read-only binary lifecycle status");
    say!("  executable: {}", status.executable.path.display());
    say!(
        "  observed executable path bytes sha256: {}",
        status
            .executable
            .observed_path_sha256
            .as_deref()
            .unwrap_or("unreadable")
    );
    match &status.provenance {
        lifecycle::Provenance::SourceOrUnrecorded => {
            say!("  provenance: source or unrecorded build")
        }
        lifecycle::Provenance::InstallerRecorded { version } => {
            say!("  provenance: installer-recorded release {version}")
        }
        lifecycle::Provenance::Unknown => say!("  provenance: UNKNOWN"),
    }
    say!(
        "  relation: {}",
        match status.relation {
            lifecycle::Relation::SourceOrUnrecorded => "source or unrecorded",
            lifecycle::Relation::Current => "current against recorded machine history",
            lifecycle::Relation::DowngradeBlocked => "downgrade blocked",
            lifecycle::Relation::AheadOfRecorded => "ahead of recorded machine history",
            lifecycle::Relation::RecordedNoHistory => "recorded; no earlier release recorded",
            lifecycle::Relation::Unknown => "UNKNOWN",
        }
    );
    say!(
        "  high-water: {}",
        status
            .high_water
            .as_ref()
            .map_or_else(|| "none".to_owned(), ToString::to_string)
    );
    if let Some(error) = &status.state_error {
        say!("  lifecycle state: unreadable — {error}");
    }
    let lifecycle::PublicRelease::Unavailable { reason, .. } = &status.public_release;
    say!("  latest public release: not verifiable — {reason}");
    Ok(())
}

fn status(json: bool) -> Result<(), Refusal> {
    let options = SetupOptions::default();
    #[derive(serde::Serialize)]
    struct Row {
        agent: &'static str,
        configured: bool,
        current: bool,
        /// Which of the three, for a reader that is a program.
        presence: skill::Presence,
        /// Whether the lifecycle gate is registered for this agent.
        gated: bool,
        /// Whether the workflow tools are registered for this agent.
        tools: bool,
        /// Why there is no gate, when there is none to be had.
        gate_gap: Option<&'static str>,
        /// Why the workflow tools can never be registered, when they cannot.
        tools_gap: Option<&'static str>,
        /// Whether a hook response can start or continue a turn for this agent.
        continuation: &'static str,
        /// Whether the skill under this root belongs to another agent too.
        #[serde(skip)]
        shared: bool,
        /// What is wrong with the gate that is registered, if anything.
        gate_fault: Option<String>,
        tools_fault: Option<String>,
        skill_root: String,
        instructions: String,
    }

    let mut rows = Vec::with_capacity(AGENTS.len());
    for adapter in AGENTS {
        let Ok(paths) = setup::resolve_paths(adapter, &options) else {
            continue;
        };
        // Through `presence_of`, which reads the configuration rather than defaulting it.
        let presence = skill::presence_of(&paths.skill_root);
        rows.push(Row {
            agent: adapter.slug,
            configured: setup::is_configured(adapter, &options),
            current: presence == skill::Presence::Current,
            presence,
            gated: setup::is_gated(adapter, &options),
            // A registered entry this build cannot read as its own is an unknown, not `on`.
            gate_fault: setup::wiring::registered(adapter, &options)
                .iter()
                .flat_map(|(_, wires)| wires)
                .find_map(super::setup::wiring::Wire::fault)
                .or_else(|| {
                    (setup::is_gated(adapter, &options)
                        && adapter.supports_hooks()
                        && setup::wiring::registered(adapter, &options).is_empty())
                    .then(|| {
                        "an entry is registered here and this build cannot read it as one of \
                         its own, so whether the gate would run is unknown"
                            .to_owned()
                    })
                }),
            tools: setup::exposes_tools(adapter, &options),
            tools_fault: tools_fault(adapter, &options),
            gate_gap: adapter.gate_gap(),
            tools_gap: adapter.tools_gap(),
            continuation: adapter.continuation(),
            shared: setup::skill_shared_with(adapter, &options, &setup::Pending::new())
                .ok()
                .flatten()
                .is_some(),
            skill_root: paths.skill_root.display().to_string(),
            instructions: paths.instructions.display().to_string(),
        });
    }

    if json {
        let holdings = harness::session::state_root(None)
            .map(|root| {
                let held: Vec<serde_json::Value> = harness::session::holdings(&root)
                    .into_iter()
                    .map(|run| {
                        serde_json::json!({
                            "run_id": run.run_id,
                            "issue": run.issue,
                            "state": run.state,
                            "silent_for": harness::session::silence(&run).seconds(),
                            "last_answer": harness::session::silence(&run).said(),
                            "repo_dir": run.repo_dir,
                            "worktree": run.worktree,
                        })
                    })
                    .collect();
                (held, harness::session::unreadable_holdings(&root))
            })
            .unwrap_or_default();
        print_json(&serde_json::json!({
            "estigia": crate::build_info::VERSION,
            "skill": skill::version(),
            "agents": rows,
            "holdings": holdings.0,
            "unreadable_pointers": holdings.1,
            "standing_down": standing_down_line(),
        }));
        return Ok(());
    }

    say!(
        "estigia {} — skill {} ({})",
        crate::build_info::VERSION,
        skill::version().unwrap_or("unknown"),
        skill::DIRECTORY
    );
    // Before the agents: `gate on` while standing down means the hook decides nothing.
    if let Some(line) = standing_down_line() {
        say!();
        say!("  {line}");
    }
    say!();
    for row in &rows {
        let state = standing(
            row.configured,
            row.presence,
            row.shared,
            row.gated || row.tools,
        );
        say!("  {:<14} {state}", row.agent);
        if row.configured || row.presence != skill::Presence::Absent {
            say!("                 {}", row.skill_root);
            say!(
                "                 harness: gate {}, tools {}",
                match (row.gated, row.gate_fault.is_some()) {
                    (true, true) => "REGISTERED BUT DEAD",
                    (true, false) => "on",
                    (false, _) => "off",
                },
                match (row.tools, row.tools_fault.is_some()) {
                    (true, true) => "REGISTERED BUT DEAD",
                    (true, false) => "on",
                    (false, _) => "off",
                },
            );
            say!("                 continuation: {}", row.continuation);
            if let Some(fault) = &row.tools_fault {
                for line in wrapped(fault, 68) {
                    say!("                   {line}");
                }
                say!("                   run: estigia setup {}", row.agent);
            }
            if let Some(fault) = &row.gate_fault {
                for line in wrapped(fault, 68) {
                    say!("                   {line}");
                }
                say!("                   run: estigia setup {}", row.agent);
            }
            if let Some(gap) = row.gate_gap {
                for line in wrapped(gap, 68) {
                    say!("                   {line}");
                }
            }
            if let Some(gap) = row.tools_gap {
                for line in wrapped(gap, 68) {
                    say!("                   {line}");
                }
            }
        }
    }
    // The push guard is per-repository, so it is reported for this checkout only.
    if let Ok(repo_dir) = std::env::current_dir() {
        let state = harness::guard::state(&repo_dir);
        if state != harness::guard::State::Absent
            || harness::guard::hooks_directory(&repo_dir).is_ok()
        {
            println_guard(&repo_dir, state);
        }
    }

    let holdings = harness::session::state_root(None)
        .map(|root| harness::session::holdings(&root))
        .unwrap_or_default();
    if !holdings.is_empty() {
        say!();
        say!("held by runs on this machine:");
        for run in &holdings {
            let silence = harness::session::silence(run).said();
            say!(
                "  {:<18} #{:<6} {:<12} last answer {silence}",
                run.run_id,
                run.issue
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                run.state.as_deref().unwrap_or("unknown"),
            );
        }
        say!("  a claim is live only while the tracker says so — `estigia release --run-id <id>`");
        say!("  puts one down, and a run that ended without releasing still holds its issue.");
    }

    let unreadable = harness::session::state_root(None)
        .map(|root| harness::session::unreadable_holdings(&root))
        .unwrap_or_default();
    if !unreadable.is_empty() {
        say!();
        say!("run pointers that cannot be read:");
        for path in &unreadable {
            say!("  {path}");
        }
        say!("  what these hold is unknown, so a push from a checkout nothing else holds is");
        say!("  refused rather than let by. Make them readable, or take them away.");
    }

    say!();
    for companion in COMPANIONS {
        say!("{}", setup::probe_companion(companion).describe(companion));
    }
    Ok(())
}

/// Breaks a sentence into lines that fit, without splitting a word.
fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// One line about this repository's push guard.
fn println_guard(repo_dir: &std::path::Path, state: harness::guard::State) {
    say!();
    say!("this repository ({}):", repo_dir.display());
    match state {
        harness::guard::State::Installed => {
            say!("  push guard on — a push no live claim authorises is refused");
        }
        harness::guard::State::Absent => {
            say!("  push guard off — `estigia guard` installs it");
        }
        harness::guard::State::Chained => {
            say!(
                "  push guard on, from a hook Estigia did not write \u{2014} a push no live claim \
                 authorises is refused"
            );
        }
        harness::guard::State::Foreign => {
            say!("  a pre-push hook is here and Estigia did not write it; it is left alone");
        }
        harness::guard::State::Unreadable => {
            say!(
                "  a pre-push hook is here and cannot be read, so whether the push boundary is \
                 gated is unknown; it is left alone"
            );
        }
        harness::guard::State::Inert => {
            say!(
                "  push guard present and NOT running \u{2014} the hook has no execute bit, so git \
                 skips it; `estigia guard` puts it back"
            );
        }
    }
}

fn config_list(agent: Option<&str>, options: &SetupOptions, json: bool) -> Result<(), Refusal> {
    // With this checkout's own rows layered on, as a run here reads them, and
    // every layer kept apart so each row can name the one that answers it.
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let (_, layers, _) = layers_of(agent, options, Some(&here))?;
    let effective = layers.effective();
    if json {
        #[derive(serde::Serialize)]
        struct Shadowed {
            value: String,
            source: &'static str,
        }
        #[derive(serde::Serialize)]
        struct Row {
            setting: &'static str,
            group: &'static str,
            scope: &'static str,
            value: String,
            accepts: &'static str,
            source: &'static str,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            shadowed: Vec<Shadowed>,
        }
        let rows = SETTINGS
            .iter()
            .map(|setting| {
                let provenance = provenance_of(*setting, &layers);
                Row {
                    setting: setting.label(),
                    group: setting.group().label(),
                    scope: setting.scope_label(),
                    value: setting.value_of(effective),
                    accepts: setting.accepted(),
                    source: provenance.source.name(),
                    shadowed: provenance
                        .shadowed
                        .iter()
                        .map(|(value, source)| Shadowed {
                            value: value.clone(),
                            source: source.name(),
                        })
                        .collect(),
                }
            })
            .collect::<Vec<_>>();
        print_json(&rows);
        return Ok(());
    }
    let mut group: Option<Group> = None;
    for setting in SETTINGS {
        if group != Some(setting.group()) {
            group = Some(setting.group());
            say!();
            say!("{}", setting.group().label().to_uppercase());
        }
        let provenance = provenance_of(*setting, &layers);
        let value = setting.value_of(effective);
        say!(
            "{:<26} {:<24} accepts: {}  [scope: {}]  ({})",
            setting.label(),
            value,
            setting.accepted(),
            setting.scope_label(),
            provenance.source.name()
        );
        if !provenance.shadowed.is_empty() {
            let listed = provenance
                .shadowed
                .iter()
                .map(|(shadowed, source)| format!("{shadowed} ({})", source.name()))
                .collect::<Vec<_>>()
                .join(", ");
            say!("  shadows {listed}");
        }
        if layers.repository.is_some()
            && setting.value_of(effective) != setting.value_of(layers.unlayered())
        {
            say!("{}", checkout_answers_instead(&value));
        }
    }
    Ok(())
}

/// Where one row's winning value came from, and what it shadows.
struct Provenance {
    source: RowSource,
    shadowed: Vec<(String, RowSource)>,
}

/// The document a row's value is read from, nearest scope first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowSource {
    Repository,
    Local,
    Agent,
    Contract,
    Default,
}

impl RowSource {
    fn name(self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Local => "user local",
            Self::Agent => "agent file",
            Self::Contract => "versioned block",
            Self::Default => "skill default",
        }
    }
}

/// The layer each row's winning value comes from, and every farther layer it
/// disagrees with.
fn provenance_of(setting: Setting, layers: &skill::ConfigLayers) -> Provenance {
    let mut present: Vec<(RowSource, &Config, bool)> = Vec::new();
    if let Some(config) = layers.repository.as_ref() {
        present.push((
            RowSource::Repository,
            config,
            layers.repository_settings.contains(&setting),
        ));
    }
    if let Some(config) = layers.local.as_ref() {
        present.push((
            RowSource::Local,
            config,
            layers.local_settings.contains(&setting),
        ));
    }
    if let Some(config) = layers.agent.as_ref() {
        present.push((
            RowSource::Agent,
            config,
            layers.agent_settings.contains(&setting),
        ));
    }
    present.push((
        RowSource::Contract,
        &layers.contract,
        layers.contract_settings.contains(&setting),
    ));

    let below = |at: usize| -> String {
        present.get(at + 1).map_or_else(
            || setting.default_value(),
            |(_, config, _)| setting.value_of(config),
        )
    };
    let winner = present
        .iter()
        .enumerate()
        .position(|(at, (_, config, owns))| *owns || setting.value_of(config) != below(at));
    let (source, winning) = match winner {
        Some(at) => (present[at].0, setting.value_of(present[at].1)),
        None => (RowSource::Default, setting.default_value()),
    };
    let shadowed = present
        .iter()
        .enumerate()
        .filter(|(at, _)| winner.is_none_or(|won| *at > won))
        .filter_map(|(_, (source, config, owns))| {
            let value = setting.value_of(config);
            (*owns && value != winning).then_some((value, *source))
        })
        .collect();
    Provenance { source, shadowed }
}

/// Declares — or lifts — a stand-down.
fn stand_down(reason: &str, minutes: u64, lift: bool, json: bool) -> Result<(), Refusal> {
    let root = harness::session::state_root(None)?;
    let file = harness::standdown::path(&root);
    // An unreadable clock lands as the epoch; `doctor` reports a broken clock.
    let now = harness::session::now_seconds().unwrap_or_default();
    let who = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "an operator".to_owned());
    // Through `standing`, so a record that will not open is not read as no record.
    let existing = harness::standdown::standing(&root);
    let standing = match &existing {
        harness::standdown::Standing::Declared(record) if record.covers(now) => {
            Some(record.clone())
        }
        _ => None,
    };
    let unreadable = matches!(existing, harness::standdown::Standing::Unreadable(_));

    if lift {
        // Read back rather than trusted: a removal that failed because the file was
        // already gone is a lift; only a file still on disk refuses.
        lift_record(&file)?;
        if json {
            print_json(&serde_json::json!({
                "standing_down": false,
                "lifted": standing.as_ref().map(|was| serde_json::json!({
                    "reason": was.reason,
                    "by": was.declared_by,
                    "seconds_left": was.remaining(now),
                })),
            }));
            return Ok(());
        }
        say!("stand-down lifted; the gate decides on its own again");
        match &standing {
            Some(was) => {
                say!(
                    "  it had {} left, declared by {} for: {}",
                    minutes_left(was.remaining(now)),
                    was.declared_by,
                    was.reason
                );
                trail(
                    &root,
                    &serde_json::json!({
                        "at": now,
                        "verdict": "stand-down-lifted",
                        "by": who,
                        "detail": reason,
                        "lifted": { "reason": was.reason, "by": was.declared_by },
                        "seconds_unused": was.remaining(now),
                    }),
                );
            }
            None if unreadable => say!(
                "  a stand-down file was here and could not be read — it has been taken \
                 away, and whatever it declared was never being honoured"
            ),
            None => say!("  nothing was in force — the gate was already deciding on its own"),
        }
        return Ok(());
    }

    let declared = harness::standdown::declare(reason, minutes, now, &who).map_err(|why| {
        Refusal::not_started(
            why.code(),
            format!("this is not a stand-down: {}", why.action()),
            Resolution::run("estigia stand-down --reason \"...\" --minutes 30"),
        )
    })?;

    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| record_unwritable(&file, &error))?;
    }
    let body = serde_json::to_string_pretty(&declared).map_err(|error| {
        Refusal::not_started(
            "stand-down-unserialisable",
            format!("the record did not serialise: {error}"),
            // No command: a serialiser failure is a defect in this build.
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a build that can write its own record \u{2014} nothing on this machine is \
                 wrong, and the gate is deciding on its own in the meantime",
            ),
        )
    })?;
    crate::paths::replace_atomically(
        &file,
        &format!(
            "{body}
"
        ),
    )
    .map_err(|error| record_unwritable(&file, &error))?;

    // Into the ledger, which is append-only.
    let superseded = standing.as_ref().map_or_else(
        || unreadable.then(|| serde_json::json!({ "unreadable": true })),
        |was| {
            Some(serde_json::json!({
                "reason": was.reason,
                "by": was.declared_by,
                "seconds_left": was.remaining(now),
            }))
        },
    );
    trail(
        &root,
        &serde_json::json!({
            "at": now,
            "verdict": "stand-down-declared",
            "by": who,
            "detail": reason,
            "minutes": minutes,
            "until": declared.until,
            "superseded": superseded,
        }),
    );

    if json {
        print_json(&serde_json::json!({
            "standing_down": true,
            "reason": reason,
            "minutes": minutes,
            "until": declared.until,
            "by": who,
            "superseded": superseded,
        }));
        return Ok(());
    }
    let note = [
        "gate stood down for {window} — every write it lets through is",
        "recorded as having gone through this, and it expires on its own",
    ]
    .join(" ");
    say!("{}", note.replace("{window}", &minutes_left(minutes * 60)));
    say!("  reason: {reason}");
    // Named, because the reason and the declarer of a replaced stand-down are
    // otherwise gone from the file, and somebody re-declaring over a colleague's
    // window should be told they did.
    if let Some(was) = &standing {
        say!(
            "  this replaces one declared by {} with {} left, for: {}",
            was.declared_by,
            minutes_left(was.remaining(now)),
            was.reason
        );
    }
    say!("  lift early: estigia stand-down --lift --reason \"done\"");
    Ok(())
}

/// A duration, in the words a person uses for it.
fn minutes_left(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match minutes {
        0 => "less than a minute".to_owned(),
        1 => "1 minute".to_owned(),
        _ => format!("{minutes} minutes"),
    }
}

/// Appends one line to the decision ledger, best effort.
fn trail(state_root: &std::path::Path, entry: &serde_json::Value) {
    harness::session::record(state_root, entry);
}

/// Takes the stand-down record away, or says it is still there.
fn lift_record(file: &std::path::Path) -> Result<(), Refusal> {
    if let Err(error) = std::fs::remove_file(file)
        && file.exists()
    {
        return Err(Refusal {
            code: "stand-down-not-lifted",
            message: format!(
                "{} is still there ({error}), so the gate is still standing down",
                file.display()
            ),
            outcome: crate::outcome::MutationOutcome::NotStarted,
            class: crate::harness::refusal_class::class_of_code("stand-down-not-lifted"),
            replay: crate::outcome::Replayability::ExactReplaySafe,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                format!(
                    concat!(
                        "whatever is holding {} released, or the file removed by hand — ",
                        "until then writes keep going through unadjudicated",
                    ),
                    file.display()
                ),
            ),
            observed: None,
        });
    }
    Ok(())
}

/// The record could not be written, and only a person can change that.
fn record_unwritable(file: &std::path::Path, error: &std::io::Error) -> Refusal {
    Refusal::not_started(
        "stand-down-unwritable",
        format!("could not write {}: {error}", file.display()),
        Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "write access to the file named above \u{2014} it is read-only, held open, or on a \
             filesystem that refused; free it and declare the stand-down again",
        ),
    )
}

/// Opens the screen, and writes only what comes back from it.
fn config_edit(agent: Option<&str>, options: &SetupOptions) -> Result<(), Refusal> {
    let writable = writable_config(agent, options)?;
    let Some(config) = crate::tui::edit(agent, writable.config.clone())? else {
        println!("nothing changed");
        return Ok(());
    };
    commit_edit(agent, writable, &config, options)
}

fn commit_edit(
    agent: Option<&str>,
    writable: WritableConfig,
    config: &Config,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    let report = writable.report;
    let target = writable.target.clone();
    let installed = writable.config;
    // The editor writes the machine-wide contract, and a machine-wide Linear
    // tracker is refused for the same reason `config set` refuses it: the row that admits it is answered in a repository's own file.
    if config.tracker.is_read_only() {
        return Err(Refusal::not_started(
            "linear-is-per-repository",
            "`Tracker linear` is read-only and admitted only under `Workflow: assisted`, which a \
             repository answers in its own file; the editor writes the machine-wide contract, so \
             set it in the repository instead",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the repository-scoped form, naming the team key: `estigia config set Tracker \
                 \"linear <team-key>\" --repo`",
            ),
        ));
    }
    if agent.is_none() {
        refuse_moved_agent_rows(&installed, config, options)?;
    }
    let agent_write = writable
        .agent_snapshot
        .as_ref()
        .map(|snapshot| (agent.unwrap_or_default(), snapshot));
    write_edited_table(&target, agent_write, &installed, config, options)?;
    if let Some(refusal) = report.as_ref() {
        say!("{}", refusal.message);
    }
    match agent_write {
        Some((slug, _)) => println!("configuration for {slug} written to {}", target.display()),
        None => println!("configuration written to {}", target.display()),
    }
    Ok(())
}

/// Writes an edited table, and refuses when a run will not read back what it
fn write_edited_table(
    target: &std::path::Path,
    agent: Option<(&str, &setup::AgentConfigurationSnapshot)>,
    installed: &Config,
    config: &Config,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    let root = target.parent().unwrap_or(target);
    match agent {
        Some((slug, snapshot)) => setup::write_agent_configuration_from_snapshot(
            snapshot,
            slug,
            config,
            crate::config::AGENT_SETTINGS,
        )?,
        None => {
            let _ = setup::rewrite_configuration(target, config)?;
            spread_edited_rows(target, installed, config, options)?;
            // The repository's own rows cannot be held by the contract, so they
            // go to the checkout's own file. `SETTINGS` filtered by scope, the
            // same predicate every other list uses.
            spread_repository_rows(installed, config, options)?;
        }
    }
    let effective = read_back(match agent {
        Some((slug, _)) => skill::installed_config_for(root, Some(slug)),
        None => skill::installed_config(root),
    })?;
    // The repository's rows are read from its own file, which this read does
    // not layer, so they are written above and checked there rather than here.
    let rows: Vec<Setting> = match agent {
        Some(_) => crate::config::AGENT_SETTINGS.to_vec(),
        None => crate::config::SETTINGS
            .iter()
            .copied()
            .filter(|setting| setting.scope() != crate::config::Scope::Repository)
            .collect(),
    };
    let shadowed = shadowed_rows(&rows, config, &effective);
    if shadowed.is_empty() {
        return Ok(());
    }
    Err(shadowed_table(root, &shadowed))
}

/// Carries a shared-screen save's changed rows that are not per-agent into every
/// installed contract but the one already written.
fn spread_edited_rows(
    written: &std::path::Path,
    installed: &Config,
    config: &Config,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    for setting in crate::config::SETTINGS {
        if matches!(
            setting.scope(),
            crate::config::Scope::Agent | crate::config::Scope::Repository
        ) {
            continue;
        }
        let value = setting.value_of(config);
        if value == setting.value_of(installed) {
            continue;
        }
        let (_, refused, _) = elsewhere(written, *setting, &value, options)?;
        if let Some(root) = refused.first() {
            return Err(shadowed_elsewhere(root, *setting, refused.len()));
        }
    }
    Ok(())
}

/// Writes a shared-screen save's changed repository rows into this checkout's
/// own file, where only a repository may answer them.
///
/// The contract holds no row for either, so writing one there would be a row
/// nobody reads; the checkout is the only place it can mean anything. Outside a
/// checkout, `repository_set` refuses by name rather than dropping the answer.
fn spread_repository_rows(
    installed: &Config,
    config: &Config,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for setting in crate::config::SETTINGS {
        if setting.scope() != crate::config::Scope::Repository {
            continue;
        }
        let value = setting.value_of(config);
        if value == setting.value_of(installed) {
            continue;
        }
        repository_set(*setting, &value, &here, options)?;
    }
    Ok(())
}

/// Sets one repository-wide row in every installed contract but `written`.
fn elsewhere(
    written: &std::path::Path,
    setting: Setting,
    value: &str,
    options: &SetupOptions,
) -> Result<(usize, Vec<std::path::PathBuf>, Vec<String>), Refusal> {
    let mut done: Vec<std::path::PathBuf> = vec![written.to_owned()];
    let mut refused: Vec<std::path::PathBuf> = Vec::new();
    let mut folded: Vec<String> = Vec::new();
    for adapter in configured(options) {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        let contract = paths.skill_root.join(skill::CONTRACT);
        // Most adapters share the neutral root, so this comes round
        // repeatedly for one file.
        if done.contains(&contract) {
            continue;
        }
        // A contract that will not read back is left alone. Read through the
        // reporting fold, so a retired `Delegated workers` row this root still
        // carries is named rather than dropped: the write below is what takes
        // it out of the table, and the note is the same one `setup` prints.
        let Ok((mut held, note)) = skill::installed_config_for_reporting(&paths.skill_root, None)
        else {
            continue;
        };
        if let Some(note) = note {
            folded.push(note.message);
        }
        setting.apply(&mut held, value)?;
        setup::rewrite_configuration(&contract, &held)?;
        let took = skill::installed_config(&paths.skill_root)
            .is_ok_and(|effective| setting.value_of(&effective) == setting.value_of(&held));
        if !took {
            refused.push(paths.skill_root.clone());
        }
        done.push(contract);
    }
    Ok((done.len() - 1, refused, folded))
}

/// Writes one row into this repository's own file, creating it if need be.
fn repository_set(
    setting: Setting,
    value: &str,
    here: &std::path::Path,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    let rows = repository_rows_for(setting, value)?;
    for (row, row_value) in &rows {
        refuse_a_row_looser_than_an_agent_holds(*row, row_value, options)?;
    }
    // A repository first: creating `.git` here would not make a checkout.
    if skill::checkout_root(here).is_none() {
        return Err(Refusal::not_started(
            "not-a-repository",
            format!("{} is not a git checkout", here.display()),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "run it inside a checkout, or set the row without `--repo`",
            ),
        ));
    }
    // Started from what this repository already answers, so a second `--repo`
    // does not quietly undo the first one's rows.
    let path = skill::repository_config_path(here);
    let mut spoken_for = Vec::new();
    let mut config = match skill::repository_rows(here)? {
        // A row this repository carries and cannot parse is a stop, not an empty table.
        Some((_, text)) => crate::config::Config::read_scope_over(
            &crate::config::Config::default(),
            &text,
            crate::config::Scope::Everywhere,
        )
        .and_then(|(mut read, settings)| {
            let (tightened, refused) = crate::config::Config::tighten_over(
                &crate::config::Config::default(),
                &mut read,
                &text,
                false,
            );
            match refused {
                Some(refusal) => Err(refusal),
                None => {
                    spoken_for = settings;
                    spoken_for.extend(tightened);
                    Ok(read)
                }
            }
        })
        .map_err(|error| {
            Refusal::not_started(
                "repository-configuration-unreadable",
                format!("{}: {error}", path.display()),
                Resolution::run("estigia config list"),
            )
        })?,
        None => crate::config::Config::default(),
    };
    let mut speaks_for = spoken_for;
    for (row, row_value) in &rows {
        row.apply(&mut config, row_value)?;
        if !speaks_for.contains(row) {
            speaks_for.push(*row);
        }
    }
    // A read-only Linear tracker is admitted only under `Workflow: assisted`:
    // the row and the row that admits it are both answered in this
    // repository's own file, so the check reads that file rather than a machine
    // default. Setting `Workflow assisted` first is the way through.
    if rows.iter().any(|(row, value)| {
        *row == crate::config::Setting::Tracker
            && value.trim().to_ascii_lowercase().starts_with("linear")
    }) {
        let workflow = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| {
                crate::config::Config::read_scope_over(
                    &crate::config::Config::default(),
                    &text,
                    crate::config::Scope::Repository,
                )
                .ok()
            })
            .map(|(read, _)| read.workflow)
            .unwrap_or(crate::config::Workflow::Autonomous);
        if workflow != crate::config::Workflow::Assisted {
            return Err(Refusal::not_started(
                "linear-needs-assisted",
                format!(
                    "`Tracker linear` is read-only and supported only under `Workflow: assisted`, \
                     and this repository's `Workflow` is `{}`",
                    workflow.as_value()
                ),
                Resolution::run("estigia config set Workflow assisted --repo"),
            ));
        }
    }
    // The other order reaches the same refused state: setting
    // `Workflow autonomous` where the repository's `Tracker` is already read-only
    // Linear would leave the two rows disagreeing, so it is refused here too.
    if rows
        .iter()
        .any(|(row, _)| *row == crate::config::Setting::Workflow)
        && config.workflow != crate::config::Workflow::Assisted
        && config.tracker.is_read_only()
    {
        return Err(Refusal::not_started(
            "autonomous-needs-github-tracker",
            "`Workflow: autonomous` and a read-only `Tracker linear` cannot both hold in one \
             repository; set `Tracker` to a GitHub tracker first",
            Resolution::run("estigia config set Tracker github --repo"),
        ));
    }
    setup::write_repository_configuration(&path, &config, &speaks_for)?;
    // The caller's home when one was named, so tests never write the real profile.
    if let Some(home) = options
        .home_dir
        .clone()
        .or_else(|| crate::paths::home_dir().ok())
    {
        skill::remember_repository(&home, here);
    }
    say!("{} = {} in {}", setting.label(), value, path.display());
    Ok(())
}

/// The rows one `config set --repo` writes: the row itself, or the two a
/// `cautious` profile stands for.
///
/// Refuses what a repository may not say before anything else is asked: an
/// agent row, and `balanced`, which would be a grant rather than a tightening.
fn repository_rows_for(setting: Setting, value: &str) -> Result<Vec<(Setting, String)>, Refusal> {
    if setting == Setting::Profile {
        setting.apply(&mut Config::default(), value)?;
        return match value.trim().to_lowercase().as_str() {
            "cautious" => Ok([Setting::Delivery, Setting::Transitions]
                .into_iter()
                .map(|row| (row, "ask".to_owned()))
                .collect()),
            _ => Err(Refusal::not_started(
                "repository-cannot-loosen",
                format!(
                    "a repository may only tighten authorisations, and `Profile` {value:?} \
                     would grant them"
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    "`cautious`, or the two authorisation rows set to `ask`; \
                     `estigia config forget` takes this repository's rows away",
                ),
            )),
        };
    }
    if !matches!(
        setting.scope(),
        crate::config::Scope::Everywhere | crate::config::Scope::Repository
    ) && !setting.repository_may_tighten()
    {
        let (mine, way_out) = match setting.scope() {
            crate::config::Scope::Machine => (
                "a fact about this machine, not what this repository is",
                "set it with neither `--repo` nor `--agent` \u{2014} `estigia config set` writes a \
                 row about this machine into every installed contract",
            ),
            _ => (
                "what one agent does, not what this repository is",
                "set it with `--agent <agent>` instead, naming the setting the same way",
            ),
        };
        return Err(Refusal::not_started(
            "setting-not-the-repositorys",
            format!("`{}` is {mine}", setting.label()),
            Resolution::no_command(crate::outcome::NoCommandReason::OperatorKnowledge, way_out),
        ));
    }
    Ok(vec![(setting, value.to_owned())])
}

/// Refuses an `auto` row over an `ask` any layer holds.
///
/// The layer is resolved the way [`ConfigLayers::unlayered`] reads — local, then
/// agent, then contract — so the refusal names the file that holds the stricter
/// answer and, where a command clears it, only a command that does. Every other
/// value is a tightening or a no-op and passes, whatever the durations of two
/// `ask` rows.
fn refuse_a_row_looser_than_an_agent_holds(
    setting: Setting,
    value: &str,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    if !setting.repository_may_tighten() {
        return Ok(());
    }
    let mut candidate = Config::default();
    setting.apply(&mut candidate, value)?;
    for adapter in configured(options) {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        let Ok(layers) = skill::config_layers(&paths.skill_root, Some(adapter.slug), None) else {
            continue;
        };
        let held = layers.unlayered();
        if setting.tightens(held, &candidate) {
            continue;
        }
        let held_value = setting.value_of(held);
        if layers.local_settings.contains(&setting) {
            let file = skill::local_override(&paths.skill_root)
                .unwrap_or_else(|| paths.skill_root.join(crate::config::LOCAL_FILE));
            return Err(Refusal::not_started(
                "repository-cannot-loosen",
                format!(
                    "`{}` is {held_value} in {}, and a repository may only tighten it, not answer \
                     {value}",
                    setting.label(),
                    file.display()
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::OperatorKnowledge,
                    format!(
                        "that row changed or removed in {} \u{2014} it is the operator's file, and \
                         Estigia will not edit it",
                        file.display()
                    ),
                ),
            ));
        }
        if layers.agent_settings.contains(&setting) {
            let file = skill::agent_override(&paths.skill_root, adapter.slug);
            return Err(Refusal::not_started(
                "repository-cannot-loosen",
                format!(
                    "`{}` is {held_value} for {} in {}, and a repository may only tighten it, not \
                     answer {value}",
                    setting.label(),
                    adapter.slug,
                    file.display()
                ),
                Resolution::run(format!(
                    "estigia config set {:?} {value} --agent {}",
                    setting.label(),
                    adapter.slug
                )),
            ));
        }
        let file = paths.skill_root.join(skill::CONTRACT);
        let resolution = Resolution::run(format!(
            "estigia config set {:?} {value} --agent {}",
            setting.label(),
            adapter.slug,
        ));
        return Err(Refusal::not_started(
            "repository-cannot-loosen",
            format!(
                "`{}` is {held_value} in {}, and a repository may only tighten it, not answer \
                 {value}",
                setting.label(),
                file.display()
            ),
            resolution,
        ));
    }
    Ok(())
}

/// Lists the checkouts that answer for themselves.
fn config_repos(json: bool) -> Result<(), Refusal> {
    let home = crate::paths::home_dir().map_err(|error| {
        Refusal::not_started(
            "home-unknown",
            format!("{error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a home directory this process can read",
            ),
        )
    })?;
    let known = skill::known_repositories(&home);
    if json {
        print_json(&serde_json::json!({
            "repositories": known
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    if known.is_empty() {
        say!("no repository answers for itself yet");
        return Ok(());
    }
    for path in known {
        say!("{}", path.display());
    }
    Ok(())
}

/// Takes a repository's own answers away, leaving the contract underneath.
fn config_forget(repo: Option<&str>, json: bool) -> Result<(), Refusal> {
    let here = match repo {
        Some(path) => std::path::PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| {
            Refusal::not_started(
                "working-directory-unknown",
                format!("{error}"),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::WorldAction,
                    "a working directory the process can read",
                ),
            )
        })?,
    };
    let path = skill::repository_config_path(&here);
    if !path.is_file() {
        if json {
            print_json(&serde_json::json!({
                "repository": here.display().to_string(),
                "forgotten": false,
                "reason": "answers for nothing of its own",
            }));
        } else {
            say!("{} answers for nothing of its own", here.display());
        }
        return Ok(());
    }
    std::fs::remove_file(&path).map_err(|error| {
        Refusal::not_started(
            "repository-configuration-unwritable",
            format!("could not remove {}: {error}", path.display()),
            Resolution::run("estigia doctor"),
        )
    })?;
    // The registry prunes itself on the next read.
    if json {
        print_json(&serde_json::json!({
            "repository": here.display().to_string(),
            "forgotten": true,
            "removed": path.display().to_string(),
        }));
    } else {
        say!(
            "{} answers with the installed contract again",
            here.display()
        );
    }
    Ok(())
}

fn config_set(
    label: &str,
    value: &str,
    agent: Option<&str>,
    repo: bool,
    contract: bool,
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
    if let Some(retired) = crate::config::retired_setting_report(label, value) {
        return Err(retired);
    }
    let Some(setting) = Setting::from_label(label) else {
        let known = crate::config::SETTINGS
            .iter()
            .map(|setting| setting.label())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Refusal::not_started(
            "setting-unknown",
            format!("{label:?} is not a setting"),
            Resolution::run(format!("estigia config list   # settings: {known}")),
        ));
    };
    if contract && agent.is_none() {
        return Err(Refusal::not_started(
            "contract-needs-an-agent",
            "`--contract` writes one agent's contract table, so it needs `--agent <slug>`",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the same command with `--agent <slug>` naming the agent whose table it is",
            ),
        ));
    }
    // The value is checked before the machine is: parsing never depends on the other rows.
    if repo {
        let here = std::env::current_dir().map_err(|error| {
            Refusal::not_started(
                "working-directory-unknown",
                format!("{error}"),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::WorldAction,
                    "a working directory the process can read",
                ),
            )
        })?;
        return repository_set(setting, value, &here, options);
    }
    // Linear is read-only and admitted only under `Workflow: assisted`, which a
    // repository answers in its own file. A machine-wide value
    // could not be checked against that, and would reach every autonomous
    // checkout on the machine, so it is set in the repository instead.
    if setting == crate::config::Setting::Tracker
        && value.trim().to_ascii_lowercase().starts_with("linear")
    {
        return Err(Refusal::not_started(
            "linear-is-per-repository",
            "`Tracker linear` is read-only and admitted only under `Workflow: assisted`, which a \
             repository answers in its own file; set it there so one machine can work a \
             Linear-tracked repository and a GitHub-tracked one side by side",
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the repository-scoped form, naming the team key: `estigia config set Tracker \
                 \"linear <team-key>\" --repo`",
            ),
        ));
    }
    if let Some(slug) = agent {
        let adapter = setup::find_installable_agent(slug)?;
        // A repository-scoped row cannot be per agent: the gate reads it without
        // asking which agent is holding the tools.
        if setting.scope() != crate::config::Scope::Agent {
            // Both arms named, so a new `Scope` stops the build rather than falling through.
            let about = match setting.scope() {
                crate::config::Scope::Everywhere | crate::config::Scope::Repository => {
                    "this repository"
                }
                crate::config::Scope::Machine => "this machine",
                crate::config::Scope::Agent => unreachable!("the guard above excluded it"),
            };
            return Err(Refusal::not_started(
                "setting-not-per-agent",
                format!(
                    "{} is a fact about {about}, not about one agent",
                    setting.label()
                ),
                Resolution::run(format!(
                    "estigia config set {:?} {value:?}",
                    setting.label()
                )),
            ));
        }
        // And a row this adapter acts on nothing for is refused before the
        // machine is read, rather than written and then read back as somebody
        // else's answer.
        if !setting.applies_to_agent(adapter) {
            return Err(Refusal::not_started(
                "setting-not-for-this-agent",
                format!(
                    "{} has no meaning for {}: it reads no plugin Estigia writes",
                    setting.label(),
                    adapter.display_name
                ),
                Resolution::run("estigia config edit --agent opencode"),
            ));
        }
    }
    if setting.scope() == crate::config::Scope::Repository {
        return Err(Refusal::not_started(
            "setting-only-the-repositorys",
            format!(
                "`{}` is answered by a repository's own file and has no machine-wide value",
                setting.label()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the same `estigia config set` with `--repo`, run inside the checkout that should \
                 answer for it",
            ),
        ));
    }
    setting.apply(&mut Config::default(), value)?;
    if agent.is_none() && setting.scope() == crate::config::Scope::Agent {
        return Err(agent_row_needs_an_agent(setting, options));
    }

    let writable = if contract {
        let slug = agent.unwrap_or_default();
        if setting.scope() == crate::config::Scope::Agent
            && !setup::find_installable_agent(slug)?.discovers_skills()
        {
            return Err(Refusal::not_started(
                "contract-is-shared",
                format!(
                    "{} is answered per agent, and {slug} reads the contract on the shared skill \
                     root, which every agent there without a file of its own reads too; \
                     `--contract` would write it for all of them",
                    setting.label()
                ),
                Resolution::run(format!(
                    "estigia config set {:?} {value:?} --agent {slug}",
                    setting.label()
                )),
            ));
        }
        contract_writable_config(slug, options)?
    } else {
        writable_config(agent, options)?
    };
    // What the tolerant read had to drop to give a table it could write into.
    // Never swallowed: a key that no longer parses is reported here, because
    // the write about to happen is what rewrites the cell without it.
    let report = writable.report;
    let target = writable.target;
    let mut config = writable.config;
    setting.apply(&mut config, value)?;
    if let Some(snapshot) = writable.agent_snapshot.as_ref() {
        // A shared skill root: this adapter's own file, so `--agent opencode`
        // cannot write the table `claude-code` reads.
        let slug = agent.unwrap_or_default();
        // The rows this file already answers for, plus the one asked now.
        let mut speaks_for = snapshot.settings();
        if !speaks_for.contains(&setting) {
            speaks_for.push(setting);
        }
        setup::write_agent_configuration_from_snapshot(snapshot, slug, &config, &speaks_for)?;
        let root = target.parent().unwrap_or(&target);
        let effective = read_back(skill::installed_config_for(root, Some(slug)))?;
        if setting.value_of(&effective) != setting.value_of(&config) {
            return Err(shadowed(root, setting, &config, &effective));
        }
        if !json && let Some(refusal) = report.as_ref() {
            say!("{}", refusal.message);
        }
        say!(
            "{} is now {:?} for {slug} ({})",
            setting.label(),
            setting.value_of(&config),
            target.display()
        );
        say_model_advisories(setting, agent, &config);
        return Ok(());
    }
    let file = target;
    // Passed through whole: the refusal already carries its code and resolution.
    let change = setup::rewrite_configuration(&file, &config)?;

    // Read back what the operator will actually read.
    let root = file.parent().unwrap_or(&file);
    let effective = read_back(skill::installed_config(root))?;
    if setting.value_of(&effective) != setting.value_of(&config) {
        return Err(shadowed(root, setting, &config, &effective));
    }
    // And through each agent's own file, which is what a run actually reads.
    let unread: Vec<&'static str> = configured(options)
        .into_iter()
        .filter(|_| !contract)
        .filter(|adapter| {
            setup::resolve_paths(adapter, options).is_ok_and(|paths| paths.skill_root == root)
        })
        .filter(|adapter| {
            skill::installed_config_for(root, Some(adapter.slug))
                .is_ok_and(|theirs| setting.value_of(&theirs) != setting.value_of(&config))
        })
        .map(|adapter| adapter.slug)
        .collect();
    if !unread.is_empty() {
        return Err(shadowed_per_agent(root, setting, &config, &unread));
    }

    // Into every installed contract, not just the first one that answered.
    let (spread, unread, folded) = if setting.scope() != crate::config::Scope::Agent {
        elsewhere(&file, setting, value, options)?
    } else {
        (0, Vec::new(), Vec::new())
    };
    if let Some(root) = unread.first() {
        return Err(shadowed_elsewhere(root, setting, unread.len()));
    }

    // Both branches report the value the table holds, not the one typed.
    let held = setting.value_of(&config);
    if json {
        let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let elsewhere = skill::layer_repository(&config, &here)
            .ok()
            .map(|layered| setting.value_of(&layered))
            .filter(|value| value != &held);
        print_json(&serde_json::json!({
            "setting": setting.label(),
            "value": held,
            "changed": change != Change::Unchanged,
            "written_into_other_contracts": spread,
            "this_checkout_answers": elsewhere,
            "dropped": report.as_ref().map(|refusal| refusal.message.clone()),
            "folded_elsewhere": folded,
            "file": file.display().to_string(),
            "advisories": advisories_of(setting, agent, &config),
        }));
        return Ok(());
    }
    if let Some(refusal) = report.as_ref() {
        say!("{}", refusal.message);
    }
    say!("{}", set_answer(setting.label(), &held, change, &file));
    say_model_advisories(setting, agent, &config);
    // And what this checkout answers when it answers differently — said, not refused.
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Ok(layered) = skill::layer_repository(&config, &here)
        && setting.value_of(&layered) != held
    {
        say!("{}", checkout_answers_instead(&setting.value_of(&layered)));
    }
    if spread > 0 {
        // Which kind of fact it is, rather than "this repository" for both.
        let about = match setting.scope() {
            crate::config::Scope::Machine => "this machine",
            _ => "this repository",
        };
        say!(
            "  written into {spread} other installed contract(s): it is a fact about {about}, so \
             every agent's copy says it"
        );
    }
    for note in &folded {
        say!("  note: {note}");
    }
    Ok(())
}

/// What this checkout answers when a nearer scope shadows the value asked for.
///
/// One sentence, written by both `config set` and `config list`: a fact written
/// twice is a fact that will disagree with itself.
fn checkout_answers_instead(value: &str) -> String {
    format!(
        "  this checkout answers {value} instead \u{2014} it says so in its own file, and \
         `estigia config set --repo` is what changes that"
    )
}

/// The row was written, and still does not read back.
fn shadowed_rows(rows: &[Setting], written: &Config, effective: &Config) -> Vec<&'static str> {
    rows.iter()
        .filter(|setting| setting.value_of(effective) != setting.value_of(written))
        .map(|setting| setting.label())
        .collect()
}

/// The refusal for a table whose rows an operator's own file overrides.
fn shadowed_table(root: &std::path::Path, rows: &[&str]) -> Refusal {
    let rows = rows.join(", ");
    let Some(file) = overriding_file(root) else {
        return Refusal {
            code: "setting-not-read-back",
            message: format!(
                "written, and {} still reads {rows} as something else \u{2014} with no local \
                 override there to account for it",
                root.display()
            ),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code("setting-not-read-back"),
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what answers those rows instead \u{2014} `estigia config list --agent <agent>` \
                 reports what each one reads",
            ),
            observed: None,
        };
    };
    // Committed, not `not_started`: the table was written.
    Refusal {
        code: "setting-shadowed-by-local-file",
        message: format!("written, and {file} still overrides {rows}"),
        outcome: crate::outcome::MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-local-file"),
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!("those rows changed or removed in {file}, which Estigia will not edit"),
        ),
        observed: None,
    }
}

/// The refusal for a contract the row was propagated into and does not reach.
fn shadowed_elsewhere(
    root: &std::path::Path,
    setting: crate::config::Setting,
    roots: usize,
) -> Refusal {
    let Some(shadow) = overriding_file(root) else {
        return Refusal {
            code: "setting-not-read-back",
            message: format!(
                "{} was written into {roots} other installed contract(s), and {} answers that row \
                 as something else \u{2014} with no local override there to account for it",
                setting.label(),
                root.display()
            ),
            outcome: crate::outcome::MutationOutcome::Committed,
            class: crate::harness::refusal_class::class_of_code("setting-not-read-back"),
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what answers that row in that root instead \u{2014} `estigia config list --agent \
                 <agent>` reports what each one reads",
            ),
            observed: None,
        };
    };
    Refusal {
        code: "setting-shadowed-by-local-file",
        message: format!(
            "{} was written into {roots} other installed contract(s), and {shadow} answers \
             that row there \u{2014} so the agents reading it were not told",
            setting.label()
        ),
        outcome: crate::outcome::MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-local-file"),
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!(
                "that row changed or removed in {shadow} \u{2014} it is the operator's file, \
                 and Estigia will not edit it"
            ),
        ),
        observed: None,
    }
}

/// The refusal for a row an agent's **own** file answers instead of the table.
fn shadowed_per_agent(
    root: &std::path::Path,
    setting: crate::config::Setting,
    written: &Config,
    agents: &[&'static str],
) -> Refusal {
    let value = setting.value_of(written);
    let named = agents.join(", ");
    Refusal {
        code: "setting-shadowed-by-agent-file",
        message: format!(
            "{} was written as {value:?} into the table under {}, and {} answer(s) that row from \
             their own file \u{2014} so {named} still read something else",
            setting.label(),
            root.display(),
            agents.len()
        ),
        outcome: crate::outcome::MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-agent-file"),
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::run(format!(
            "estigia config set {:?} {value:?} --agent {}",
            setting.label(),
            agents[0]
        )),
        observed: None,
    }
}

fn shadowed(
    root: &std::path::Path,
    setting: crate::config::Setting,
    written: &Config,
    effective: &Config,
) -> Refusal {
    // Only when there is one — never blame "the local override" when none was found.
    let Some(shadow) = overriding_file(root) else {
        return unexplained_readback(root, setting, written, effective);
    };
    Refusal {
        code: "setting-shadowed-by-local-file",
        message: format!(
            "{} was written as {} and still reads {}: {shadow} overrides that row",
            setting.label(),
            setting.value_of(written),
            setting.value_of(effective)
        ),
        outcome: crate::outcome::MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("setting-shadowed-by-local-file"),
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!(
                "that row changed or removed in {shadow} — it is the operator's file, and Estigia will not edit it"
            ),
        ),
        observed: None,
    }
}

/// The operator's own file overriding rows in this root, when there is one.
fn overriding_file(root: &std::path::Path) -> Option<String> {
    skill::local_override(root).map(|path| path.display().to_string())
}

/// The refusal for a row that read back wrong with nothing there to explain it.
fn unexplained_readback(
    root: &std::path::Path,
    setting: crate::config::Setting,
    written: &Config,
    effective: &Config,
) -> Refusal {
    Refusal {
        code: "setting-not-read-back",
        message: format!(
            "{} was written as {} into the table under {}, and reads {} \u{2014} and no local \
             override is there to account for it",
            setting.label(),
            setting.value_of(written),
            root.display(),
            setting.value_of(effective)
        ),
        outcome: crate::outcome::MutationOutcome::Committed,
        class: crate::harness::refusal_class::class_of_code("setting-not-read-back"),
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "what answers that row instead — `estigia config list --agent <agent>` reports what \
             each one reads",
        ),
        observed: None,
    }
}

/// The configuration a write can be checked against, or why it cannot be.
/// Reaching here means a table was written that cannot be read back — never swallow it.
fn read_back(read: Result<Config, Refusal>) -> Result<Config, Refusal> {
    read.map_err(|refusal| Refusal {
        message: format!(
            "written, and the table does not read back: {}",
            refusal.message
        ),
        outcome: crate::outcome::MutationOutcome::Committed,
        replay: crate::outcome::Replayability::NotReplayable,
        ..refusal
    })
}

fn writable_config(agent: Option<&str>, options: &SetupOptions) -> Result<WritableConfig, Refusal> {
    let Some(slug) = agent else {
        // A table with one bad row is still a table to write into — readers must
        // report an unreadable contract; only a write may keep what parses.
        let (contract, config, report) = match contract_of_reporting(None, options) {
            Ok(pair) => pair,
            Err(refusal) if refusal.code == "skill-not-installed" => return Err(refusal),
            Err(refusal) => {
                let repaired = configured(options).into_iter().find_map(|adapter| {
                    let paths = setup::resolve_paths(adapter, options).ok()?;
                    let contract = paths.skill_root.join(skill::CONTRACT);
                    contract.is_file().then(|| {
                        let (config, report) = skill::installed_config_in_keeping_what_parses(
                            &paths.skill_root,
                            options
                                .home_dir
                                .as_deref()
                                .unwrap_or_else(|| std::path::Path::new("")),
                        );
                        (contract, config, report)
                    })
                });
                repaired.ok_or(refusal)?
            }
        };
        return Ok(WritableConfig {
            target: contract,
            config,
            agent_snapshot: None,
            report,
        });
    };
    let adapter = setup::find_installable_agent(slug)?;
    // The caller's machine, not a fresh default.
    let paths = setup::resolve_paths(adapter, options).map_err(|_| skill::no_skill_root())?;
    if !adapter.discovers_skills() {
        let target = skill::agent_override(&paths.skill_root, adapter.slug);
        let agent_snapshot = setup::agent_configuration_snapshot(&target)?;
        let (config, report) = skill::installed_config_for_agent_write(
            &paths.skill_root,
            adapter.slug,
            agent_snapshot.document(),
        )?;
        return Ok(WritableConfig {
            target,
            config,
            agent_snapshot: Some(agent_snapshot),
            report,
        });
    }
    // Strict first, so an intact table is read exactly as before. The reporting
    // read, because a fold is a row leaving the table this call is about to
    // rewrite, and the operator is told rather than left to notice.
    let (config, report) =
        match skill::installed_config_for_reporting(&paths.skill_root, Some(adapter.slug)) {
            Ok(pair) => pair,
            Err(refusal) if refusal.code == "skill-not-installed" => return Err(refusal),
            Err(_) => skill::installed_config_in_keeping_what_parses(
                &paths.skill_root,
                options
                    .home_dir
                    .as_deref()
                    .unwrap_or_else(|| std::path::Path::new("")),
            ),
        };
    Ok(WritableConfig {
        target: paths.skill_root.join(skill::CONTRACT),
        config,
        agent_snapshot: None,
        report,
    })
}

struct WritableConfig {
    target: std::path::PathBuf,
    config: Config,
    agent_snapshot: Option<setup::AgentConfigurationSnapshot>,
    /// What the tolerant read dropped to give a writable table, if anything.
    report: Option<Refusal>,
}

/// The installed table and the row it folded, for a caller about to write it.
fn contract_of_reporting(
    agent: Option<&str>,
    options: &SetupOptions,
) -> Result<(std::path::PathBuf, Config, Option<Refusal>), Refusal> {
    let (root, layers, report) = layers_of(agent, options, None)?;
    Ok((
        root.join(skill::CONTRACT),
        layers.unlayered().clone(),
        report,
    ))
}

/// The contract root the adapter choice resolves to, with every layer kept apart.
fn layers_of(
    agent: Option<&str>,
    options: &SetupOptions,
    repo_dir: Option<&std::path::Path>,
) -> Result<(std::path::PathBuf, skill::ConfigLayers, Option<Refusal>), Refusal> {
    let targets = match agent {
        Some(slug) => vec![setup::find_installable_agent(slug)?],
        // With no agent named, the canonical root governs; a stable sort keeps the
        // declared order behind it.
        None => {
            let mut targets = configured(options);
            if let Ok(canonical) = harness::discover_skill_root_in(options) {
                targets.sort_by_key(|adapter| {
                    setup::resolve_paths(adapter, options)
                        .map_or(true, |paths| paths.skill_root != canonical)
                });
            }
            targets
        }
    };
    // A contract that is there and does not parse is not "nothing installed".
    let mut unreadable: Option<Refusal> = None;
    for adapter in targets {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        // Narrowed to the adapter: most share one root, and the answers live in the
        // file beside the table.
        match skill::config_layers_for_reporting(&paths.skill_root, Some(adapter.slug), repo_dir) {
            Ok((layers, report)) => return Ok((paths.skill_root, layers, report)),
            // Absent is walked past only when no agent was named.
            Err(refusal) if refusal.code == "skill-not-installed" && agent.is_none() => {}
            // Kept, not raised at once: an unreadable adapter must not hide a readable one.
            Err(refusal) => {
                unreadable.get_or_insert(refusal);
            }
        }
    }
    Err(unreadable.unwrap_or_else(|| nothing_configured(&[])))
}

/// Reports whether everything a run needs before it swears actually works.
///
/// Exits non-zero when something is broken, so a script can gate on it — and
/// the refusal it exits with names the first thing to fix rather than the last
/// thing checked.
fn doctor(json: bool) -> Result<(), Refusal> {
    let skill_root = harness::discover_skill_root().ok();
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let tracker = harness::doctor::tracker_in_force(skill_root.as_deref(), &repo_dir);
    // A read-only tracker under autonomous work is refused here:
    // it is admitted only under assisted work, and `doctor` is where a machine
    // finds that out before a run tries to swear against it.
    if tracker.is_read_only() {
        let workflow = harness::doctor::workflow_in_force(skill_root.as_deref(), &repo_dir);
        if workflow != crate::config::Workflow::Assisted {
            return Err(Refusal::not_started(
                "linear-needs-assisted",
                format!(
                    "`{}` is read-only and supported only under `Workflow: assisted`, and this \
                     repository's `Workflow` is `{}`",
                    tracker.as_value(),
                    workflow.as_value()
                ),
                Resolution::run("estigia config set Workflow assisted --repo"),
            ));
        }
    }
    let checks = harness::doctor::full(
        skill_root.as_deref(),
        &repo_dir,
        &tracker,
        &SetupOptions::default(),
    );

    if json {
        print_json(&checks);
    } else {
        for line in checks.iter().flat_map(harness::doctor::Check::report_lines) {
            say!("{line}");
        }
    }

    let broken = harness::doctor::first_blocking(&checks).and_then(|check| match &check.health {
        harness::doctor::Health::Broken { resolution, .. } => Some((check, resolution)),
        _ => None,
    });
    match broken {
        // Said rather than refused: choosing a transportless tracker is a choice,
        // not a fault — what was missing is the sentence, not the failure.
        None => {
            if !json && tracker.transport().is_none() {
                say!("{}", transportless_sentence(&tracker));
            }
            Ok(())
        }
        Some((first, resolution)) => Err(Refusal::not_started(
            "environment-not-ready",
            format!(
                "{} is not usable, so a run cannot swear yet: {}",
                first.name, first.about
            ),
            resolution.clone(),
        )),
    }
}

/// Sweeps branches and checkouts no delivery owns.
///
/// Read-only until `--confirm`. The listing and the deletion read the same
/// decision, so what a run confirms is exactly what it was shown.
fn prune(confirm: bool, base: &str, json: bool) -> Result<(), Refusal> {
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let skill_root = harness::control_surface();
    let context = crate::transport::Context::live(skill_root, repo_dir, None);
    let root = harness::prune::root_prefix(context.get("Worktree location"));
    let holdings = harness::session::state_root(None)
        .map(|root| harness::session::holdings(&root))
        .unwrap_or_default();
    let view = harness::prune::View {
        context: &context,
        base,
        holdings: &holdings,
        root: root.as_deref(),
        ownership: harness::session::state_root(None)
            .map_err(|failure| failure.message)
            .and_then(|root| harness::prune::ownership(&root)),
        ownership_root: harness::session::state_root(None).ok(),
    };
    let swept = harness::prune::sweep(&view).map_err(|failure| {
        Refusal::not_started(
            "prune-read-failed",
            format!(
                "the sweep could not read the machine, so what accumulated is unknown: {}",
                failure.detail()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "a git checkout with a remote, and an authenticated `gh`",
            ),
        )
    })?;
    let removed = if confirm {
        harness::prune::confirm(&view, &swept)
    } else {
        Vec::new()
    };
    if json {
        print_json(&serde_json::json!({
            "confirmed": confirm,
            "swept": swept.iter().map(swept_json).collect::<Vec<_>>(),
            "removed": removed,
        }));
        return Ok(());
    }
    if swept.is_empty() {
        say!("nothing to sweep: no eligible branch or checkout is unowned");
        return Ok(());
    }
    for entry in &swept {
        let mark = if entry.delete { "sweep" } else { "keep " };
        say!(
            "  {mark}  {:<11} {:<48} {}",
            entry.kind.word(),
            entry.name,
            entry.reason
        );
    }
    let deletable = swept.iter().filter(|entry| entry.delete).count();
    if !confirm && deletable > 0 {
        say!("{deletable} would be removed; re-run with --confirm");
    }
    Ok(())
}

/// Lists the uncommitted work in every checkout. Writes nothing.
fn changes(json: bool) -> Result<(), Refusal> {
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = crate::transport::Context::live(harness::control_surface(), repo_dir, None);
    let holdings = harness::session::state_root(None)
        .map(|root| harness::session::holdings(&root))
        .unwrap_or_default();
    let trees = harness::changes::survey(&context, &holdings).map_err(|failure| {
        Refusal::not_started(
            "read-failed",
            format!(
                "the checkouts could not be listed, so what they hold is unknown: {}",
                failure.detail()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "a git checkout",
            ),
        )
    })?;
    if json {
        print_json(&serde_json::json!({
            "worktrees": trees.iter().map(tree_json).collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    for tree in &trees {
        let head = match (&tree.branch, tree.head) {
            (Some(branch), Some(harness::changes::Head::NoCommits)) => {
                format!("{branch} (no commits yet)")
            }
            (Some(branch), _) => branch.clone(),
            (None, Some(head)) => head.word().to_owned(),
            (None, None) => "unknown".to_owned(),
        };
        let held = match &tree.holder {
            Some(holder) => format!("held by {}", holder.run_id),
            None => "orphan".to_owned(),
        };
        let ahead = tree
            .ahead
            .filter(|ahead| *ahead > 0)
            .map(|ahead| format!(", {ahead} ahead"))
            .unwrap_or_default();
        say!("{}  [{head}]  {held}{ahead}", tree.path.display());
        match &tree.unreadable {
            Some(why) => say!("  unreadable: {why}"),
            None => {
                for change in &tree.files {
                    let counts = match (change.added, change.removed) {
                        (Some(added), Some(removed)) => format!("  +{added} -{removed}"),
                        (Some(added), None) => format!("  +{added}"),
                        _ => String::new(),
                    };
                    say!("  {}  {}{counts}", change.marker, change.path);
                }
            }
        }
    }
    Ok(())
}

/// One checkout, as JSON.
fn tree_json(tree: &harness::changes::Tree) -> serde_json::Value {
    serde_json::json!({
        "path": tree.path.display().to_string(),
        "branch": tree.branch,
        "head": tree.head.map(harness::changes::Head::word),
        "unreadable": tree.unreadable,
        "files": tree.files.iter().map(|change| serde_json::json!({
            "marker": change.marker.to_string(),
            "path": change.path,
            "added": change.added,
            "removed": change.removed,
        })).collect::<Vec<_>>(),
        "ahead": tree.ahead,
        "modified": tree.modified,
        "run": tree.holder.as_ref().map(|holder| holder.run_id.clone()),
        "issue": tree.holder.as_ref().and_then(|holder| holder.issue),
        "orphan": tree.orphan,
    })
}

/// One swept entry, as JSON.
fn swept_json(entry: &harness::prune::Swept) -> serde_json::Value {
    serde_json::json!({
        "kind": match entry.kind {
            harness::prune::Kind::RemoteHead => "remote-head",
            harness::prune::Kind::Worktree => "worktree",
        },
        "name": entry.name,
        "branch": entry.branch,
        "issue": entry.issue,
        "delete": entry.delete,
        "reason": entry.reason,
    })
}

/// Sends, lists or starts the flush of the queue the closed window held back.
fn flush(detach: bool, status: bool, json: bool) -> Result<(), Refusal> {
    if detach {
        return flush_detached(json);
    }
    let answer = tool("flush", &serde_json::json!({ "read_only": status }))?;
    if json {
        print_json(&serde_json::from_str::<serde_json::Value>(&answer).unwrap_or_default());
    } else {
        say!("{}", indented_for_a_person(answer));
    }
    Ok(())
}

/// Starts `estigia flush` in its own process group with no terminal, and
/// returns without waiting for it. The lease in the store keeps two apart.
fn flush_detached(json: bool) -> Result<(), Refusal> {
    let refuse = |why: String| {
        Refusal::unavailable(
            "flush-detach-failed",
            format!("the flush could not be started in the background: {why}"),
            Resolution::run("estigia flush"),
        )
    };
    let executable = std::env::current_exe().map_err(|error| refuse(error.to_string()))?;
    let pid = crate::transport::outbox::flush::detach(&executable)
        .map_err(|error| refuse(error.to_string()))?;
    if json {
        print_json(&serde_json::json!({ "started": true, "pid": pid }));
    } else {
        say!("flush started in the background (process {pid})");
    }
    Ok(())
}

/// What one `store import` admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
struct StoreImport {
    /// Open issues the store did not hold and the import admitted.
    admitted: usize,
    /// Open issues the store already held, left untouched.
    already_held: usize,
}

/// Admits every open issue the local store has never touched.
fn store_import(json: bool) -> Result<(), Refusal> {
    let repo_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = crate::transport::Context::live(harness::control_surface(), repo_dir, None);
    let _gh_pin = harness::gh_repo_pin();
    harness::apply_gh_repo_locked(None);
    let admitted = store_import_in(&context)?;
    if json {
        print_json(&admitted);
        return Ok(());
    }
    say!(
        "admitted {} open issue(s) the store had never touched; {} already held",
        admitted.admitted,
        admitted.already_held
    );
    Ok(())
}

/// The import behind [`store_import`], in a context a test can hand it.
///
/// The store is opened once and each unheld issue is read and imported in turn.
/// A diverged import stops the loop and names its issue: the store keeps every
/// issue admitted before it and nothing of the one that diverged, because
/// [`crate::transport::store::Store::import`] writes in its own transaction.
fn store_import_in(context: &crate::transport::Context) -> Result<StoreImport, Refusal> {
    let repo = crate::transport::claim::repo_of(context).map_err(store_import_unreadable)?;
    let path = crate::transport::store::store_path().map_err(store_import_unreadable)?;
    let mut store = crate::transport::store::Store::open(&path).map_err(store_import_unreadable)?;
    let held: std::collections::BTreeSet<u64> = store
        .issue_rows(&repo)
        .map_err(store_import_unreadable)?
        .into_iter()
        .map(|row| row.number)
        .collect();
    let numbers =
        crate::transport::commands::open_issue_numbers(context).map_err(store_import_unreadable)?;
    let now = harness::session::stamp_of(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
    );
    let mut admitted = 0usize;
    let mut already_held = 0usize;
    for number in numbers {
        if held.contains(&number) {
            already_held += 1;
            continue;
        }
        let github = crate::transport::gh_json(
            &[
                "issue",
                "view",
                &number.to_string(),
                "--json",
                "body,labels,comments",
            ],
            Some(&context.repo_dir),
        )
        .map_err(store_import_unreadable)?
        .ok_or_else(|| {
            store_import_unreadable(crate::transport::Failure::Read(format!(
                "gh issue view {number} returned nothing"
            )))
        })?;
        match store.import(&repo, number, &github, &now) {
            Ok(crate::transport::store::Imported::Comments(_)) => admitted += 1,
            Ok(crate::transport::store::Imported::AlreadyHeld) => already_held += 1,
            Err(failure) => return Err(store_import_diverged(number, failure)),
        }
    }
    Ok(StoreImport {
        admitted,
        already_held,
    })
}

/// A read the import could not make: nothing was written and nothing decided.
fn store_import_unreadable(failure: crate::transport::Failure) -> Refusal {
    Refusal::unavailable(
        "store-import-read-failed",
        format!(
            "the open issues and the local store could not be read: {}",
            failure.detail()
        ),
        Resolution::no_command(
            crate::outcome::NoCommandReason::WorldAction,
            "read the open issues and the local store again; write nothing meanwhile",
        ),
    )
}

/// An import that diverged, as a refusal naming the issue and what to do.
fn store_import_diverged(issue: u64, failure: crate::transport::Failure) -> Refusal {
    let answer = harness::tracker::Answer {
        code: failure.code(),
        body: Some(failure.envelope()),
    };
    let mut refusal = harness::tracker::translate(&answer, "store import")
        .unwrap_or_else(|| unreachable!("a failed call is never exit code 0"));
    refusal.message = format!("#{issue}: {}", refusal.message);
    refusal
}

/// Reads the roles delegated contexts were observed running as, across runs.
///
/// The durable half of the role record: `SessionStart` announces the roles one
/// run saw, and this reads the log that outlives it. Filtered by `--run-id` and
/// `--issue` when given, and read-only either way.
fn roles(
    run_id: Option<&str>,
    issue: Option<&crate::identity::Identity>,
    json: bool,
) -> Result<(), Refusal> {
    let root = harness::session::state_root(None)?;
    let records: Vec<serde_json::Value> = harness::session::role_records(&root)
        .into_iter()
        .filter(|record| {
            let matches_run = run_id.is_none_or(|id| {
                record.get("run_id").and_then(serde_json::Value::as_str) == Some(id)
            });
            matches_run && role_issue_matches(record, issue)
        })
        .collect();
    if json {
        print_json(&records);
        return Ok(());
    }
    if records.is_empty() {
        say!("no role observations recorded");
        return Ok(());
    }
    for record in &records {
        say!("{}", role_line(record));
    }
    Ok(())
}

/// One role observation, as a line for a person.
fn role_line(record: &serde_json::Value) -> String {
    let at = record
        .get("at")
        .and_then(serde_json::Value::as_u64)
        .map(harness::session::stamp_of)
        .unwrap_or_else(|| "unknown time".to_owned());
    let run = record
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("(unnamed)");
    let role = record
        .get("agent_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("(unnamed)");
    let repo = record
        .get("repo_dir")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let declared = match (
        record.get("model").and_then(serde_json::Value::as_str),
        record.get("effort").and_then(serde_json::Value::as_str),
    ) {
        (Some(model), Some(effort)) => format!("  model={model}  effort={effort}"),
        (Some(model), None) => format!("  model={model}"),
        (None, Some(effort)) => format!("  effort={effort}"),
        (None, None) => String::new(),
    };
    match role_issue(record) {
        Some(crate::identity::Identity::Number(issue)) => {
            format!("{at}  {run}  issue #{issue}  {role}{declared}  {repo}")
        }
        Some(crate::identity::Identity::Key(key)) => {
            format!("{at}  {run}  issue {key}  {role}{declared}  {repo}")
        }
        None => format!("{at}  {run}  {role}{declared}  {repo}"),
    }
}

fn role_issue(record: &serde_json::Value) -> Option<crate::identity::Identity> {
    record
        .get("issue")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn role_issue_matches(
    record: &serde_json::Value,
    wanted: Option<&crate::identity::Identity>,
) -> bool {
    wanted.is_none_or(|wanted| role_issue(record).as_ref() == Some(wanted))
}

/// Serves MCP over stdio until the client closes it.
///
/// A harness that is not installed still serves; every call comes back with the
/// refusal that names what to run.
fn mcp() -> Result<(), Refusal> {
    let context = gate_context("");
    let stdin = std::io::BufReader::new(std::io::stdin());
    harness::mcp::serve(stdin, std::io::stdout(), context).map_err(|error| {
        Refusal::not_started(
            "mcp-stream-failed",
            format!("the MCP stream ended badly: {error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a client that keeps standard input and output open",
            ),
        )
    })
}

/// Runs one lifecycle event. Never fails the agent's tool call.
fn hook(event: &str, dialect: &str, agent: Option<&str>, wait_mail: bool) -> Result<(), Refusal> {
    let Some(event) = harness::hook::Event::from_slug(event) else {
        let known = harness::hook::EVENTS
            .iter()
            .map(|(event, _, _)| event.slug())
            .collect::<Vec<_>>()
            .join(", ");
        // This one *is* a refusal: it can only happen if a settings file names
        // an event this build does not have, which is a defect worth seeing.
        return Err(Refusal::not_started(
            "hook-event-unknown",
            format!("{event:?} is not a lifecycle event"),
            Resolution::run(format!("estigia setup --all   # re-registers: {known}")),
        ));
    };
    // Git's hooks answer with an exit code, not JSON.
    if harness::hook::is_git(event) {
        return git_hook(event);
    }

    let input = harness::hook::read_input(std::io::stdin());
    let context = gate_context(&input.cwd).ok();
    let named = dialect;
    let dialect = harness::hook::Dialect::from_slug(named);
    // Standard error, because standard output is the answer — and only on a slug
    // that is not one, so an ordinary run stays quiet.
    if dialect.slug() != named {
        note!(
            "estigia: `{named}` is not a dialect this build knows, so this answer is written in \
             `{}` — if the agent ignores it, that is why",
            dialect.slug()
        );
    }
    if wait_mail {
        wait_for_mail(dialect, agent, &input, context.as_ref());
        return Ok(());
    }
    let answer = loop_stop::answer(dialect, agent, event, &input, context.as_ref());

    // One dialect reads a process status rather than a document.
    if dialect.answers_with_status() {
        if let Some(status) = answer.get("status").and_then(serde_json::Value::as_u64) {
            let reason = answer
                .get("stderr")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("estigia refused this action");
            eprintln!("{reason}");
            std::process::exit(status.try_into().unwrap_or(2));
        }
        return Ok(());
    }

    say!("{answer}");
    Ok(())
}

/// The `Stop` hook that outlives the turn: exits 2 with the mail on standard
/// error, or returns having written nothing.
fn wait_for_mail(
    dialect: harness::hook::Dialect,
    agent: Option<&str>,
    input: &harness::hook::Input,
    context: Option<&harness::GateContext>,
) {
    let (Some(context), Some(run_id)) = (
        context,
        harness::hook::mail_recipient(dialect, agent, input),
    ) else {
        return;
    };
    let deadline = std::time::Duration::from_secs(
        harness::mailbox::WAIT_TIMEOUT_SECONDS - harness::mailbox::WAIT_MARGIN_SECONDS,
    );
    if let Some(text) = harness::mailbox::wait(
        &context.state_root,
        &run_id,
        harness::mailbox::WAIT_INTERVAL,
        deadline,
    ) {
        eprintln!("{text}");
        std::process::exit(2);
    }
}

/// Runs a git hook, whose contract is the exit code.
fn git_hook(event: harness::hook::Event) -> Result<(), Refusal> {
    // Not `.`: coverage compares the absolute checkouts a claim records.
    let Ok(repo_dir) = std::env::current_dir() else {
        note!(
            "estigia: the working directory could not be read, so this push was not checked \
             against any claim"
        );
        return Ok(());
    };
    let Ok(context) = gate_context(&repo_dir.to_string_lossy()) else {
        return Ok(());
    };
    match event {
        harness::hook::Event::PrePush => {
            // Read before the decision, so the local ref — the one that names a
            // branch, and therefore a worktree — can select among several
            // holders rather than only being written to the ledger afterwards.
            let pushed = refs_being_pushed();
            let local_refs: Vec<String> = pushed.iter().map(|r| r.local.clone()).collect();
            let remote_refs: Vec<String> = pushed.iter().map(|r| r.remote.clone()).collect();
            // "No claim covers this checkout" is ordinary; "no skill installed" is
            // not — and both are said rather than passed over in silence.
            let adjudication =
                harness::guard::decide_pushing(&context, &repo_dir, &local_refs, &remote_refs);
            let harness::guard::Adjudication { decision, holder } = adjudication;
            // Asked after the decision: a holder may exist without a skill directory.
            if matches!(decision, harness::Decision::Outside(_)) {
                if let Err(refusal) = crate::skill::installed_config(&context.skill_root) {
                    note!(
                        "estigia: this push was not checked against any claim \u{2014} {}",
                        refusal.message
                    );
                    note!("  run: estigia setup --all");
                    return Ok(());
                }
                if let Some(tracker) = inert_tracker(&repo_dir) {
                    note!(
                        "estigia: this push was not checked against any claim \u{2014} `{}` {}, \
                         so this guard adjudicates nothing here",
                        tracker.value,
                        tracker.why
                    );
                    note!("  run: {}", tracker.command);
                    return Ok(());
                }
            }
            // Recorded through the same function the tool path uses.
            let run_id = holder.unwrap_or_default();
            harness::hook::note(
                &context,
                &run_id,
                "git push",
                Some("git push"),
                &decision,
                &remote_refs,
            );
            match decision {
                harness::Decision::Outside(_) => Ok(()),
                harness::Decision::Allow(reason) => {
                    say!("estigia: {reason}");
                    Ok(())
                }
                harness::Decision::Deny(refusal) | harness::Decision::Ask(refusal) => Err(*refusal),
            }
        }
        _ => Ok(()),
    }
}

/// Installs or removes the push guard, and says which of the ten things it did.
fn guard(uninstall: bool, dry_run: bool, json: bool) -> Result<(), Refusal> {
    let repo_dir = std::env::current_dir().map_err(|error| {
        Refusal::not_started(
            "working-directory-unknown",
            format!("{error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a working directory the process can read",
            ),
        )
    })?;
    let hooks = harness::guard::hooks_directory(&repo_dir)?;
    let path = hooks.join(harness::guard::HOOK);
    let shown = path.display().to_string();

    // `(outcome, prose, whether the caveats belong under it)`. The outcome is
    // the stable word a program branches on; the prose is what a person reads.
    let (outcome, prose, caveats): (&str, String, bool) = if uninstall {
        match harness::guard::uninstall(&repo_dir, dry_run)? {
            harness::guard::Removal::Taken => ("removed", format!("{shown} removed"), false),
            harness::guard::Removal::WouldBeTaken => (
                "would-be-removed",
                format!("{shown} would be removed"),
                false,
            ),
            harness::guard::Removal::NothingThere => (
                "nothing-there",
                format!("{shown} is not there \u{2014} nothing to remove"),
                false,
            ),
            harness::guard::Removal::LeftAlone(
                harness::guard::State::Chained | harness::guard::State::Foreign,
            ) => (
                "left-alone-theirs",
                format!("{shown} was written by somebody else and is left alone"),
                false,
            ),
            harness::guard::Removal::LeftAlone(harness::guard::State::Unreadable) => (
                "left-alone-unreadable",
                format!("{shown} cannot be read and is left alone \u{2014} nothing was removed"),
                false,
            ),
            harness::guard::Removal::LeftAlone(
                harness::guard::State::Installed | harness::guard::State::Inert,
            ) => (
                "left-alone-edited",
                format!(
                    "{shown} carries lines Estigia did not write and is left alone \u{2014} \
                     nothing was removed; take the file out yourself if you want it gone"
                ),
                false,
            ),
            // The file vanished between the read and the removal.
            harness::guard::Removal::LeftAlone(harness::guard::State::Absent) => (
                "nothing-there",
                format!("{shown} is not there \u{2014} nothing to remove"),
                false,
            ),
        }
    } else {
        let executable = std::env::current_exe().map_err(|error| {
            Refusal::not_started(
                "executable-not-resolvable",
                format!("{error}"),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::WorldAction,
                    "a resolvable path to the Estigia executable",
                ),
            )
        })?;
        let executable = crate::paths::remove_windows_verbatim_prefix(
            executable.canonicalize().unwrap_or(executable),
        );

        let before = harness::guard::state(&repo_dir);
        harness::guard::install(&repo_dir, &executable, dry_run)?;
        match before {
            harness::guard::State::Installed => {
                ("already-current", format!("{shown} already current"), false)
            }
            harness::guard::State::Chained => (
                "already-chained",
                format!(
                    "{shown} already hands the push to Estigia, and is left alone\n  it was \
                     written by somebody else, so the checks around it are theirs."
                ),
                true,
            ),
            _ if dry_run => ("would-write", format!("would write {shown}"), true),
            _ => ("wrote", format!("wrote {shown}"), true),
        }
    };

    let inert = if caveats {
        inert_tracker(&repo_dir)
    } else {
        None
    };
    let caveats_shown: Vec<&str> = if caveats {
        guard_caveats(inert.is_none())
    } else {
        Vec::new()
    };
    let inert_note = inert.as_ref().map(|tracker| {
        format!(
            "`{}` {}, so this guard adjudicates nothing until `{}`.",
            tracker.value, tracker.why, tracker.command
        )
    });

    if json {
        let mut answer = serde_json::json!({
            "action": if uninstall { "uninstall" } else { "install" },
            "outcome": outcome,
            "path": shown,
            "dry_run": dry_run,
            "message": prose,
            "caveats": caveats_shown,
        });
        if let Some(note) = &inert_note {
            answer["note"] = serde_json::json!(note);
        }
        print_json(&answer);
        return Ok(());
    }
    for line in prose.lines() {
        say!("{line}");
    }
    if caveats {
        for caveat in &caveats_shown {
            say!("  {caveat}");
        }
        if let Some(note) = &inert_note {
            say!("  {note}");
        }
    }
    Ok(())
}

fn transportless_sentence(tracker: &crate::config::Tracker) -> String {
    if tracker.is_read_only() {
        format!(
            "estigia: `{}` is read-only: an assisted run reads its issue and writes nothing to \
             it, and no run can swear a claim against it",
            tracker.as_value()
        )
    } else {
        format!(
            "estigia: `{}` has no executable transport, so no run can swear here \u{2014} \
             the contract is installed and the gate adjudicates nothing",
            tracker.as_value()
        )
    }
}

/// The configured tracker when it cannot adjudicate an unsworn write here, with why and the
/// command that changes that.
struct InertTracker {
    value: String,
    why: &'static str,
    command: &'static str,
}

/// The configured tracker, when it cannot adjudicate an unsworn write here.
///
/// Asked of `Tracker::adjudicates_unclaimed_writes`: a read-only tracker is inert only without
/// `Workflow: assisted`, where a run does swear against it.
fn inert_tracker(repo_dir: &std::path::Path) -> Option<InertTracker> {
    let skill_root = harness::control_surface();
    let (config, complaint) = skill::installed_config_in_keeping_what_parses(&skill_root, repo_dir);
    let workflow = effective_workflow(complaint.is_some(), config.workflow);
    if config.tracker.adjudicates_unclaimed_writes(workflow) {
        return None;
    }
    Some(InertTracker {
        value: crate::config::Setting::Tracker.value_of(&config),
        why: if config.tracker.is_read_only() {
            "is read-only and this repository is not under `Workflow: assisted`"
        } else {
            "has no executable transport"
        },
        command: if config.tracker.is_read_only() {
            "estigia config set Workflow assisted --repo"
        } else {
            "estigia config set Tracker github"
        },
    })
}

/// What the push guard actually does, which is narrower than having one.
///
/// The line promising the refusal is dropped when the configured tracker cannot adjudicate an
/// unsworn write here.
fn guard_caveats(adjudicates: bool) -> Vec<&'static str> {
    let mut caveats = vec![
        "a push from a checkout a live claim holds is refused unless that claim",
        "justifies it \u{2014} whoever types it, agent or person.",
    ];
    if adjudicates {
        caveats.extend([
            "with no claim on a checkout, this refuses every push and the agent gate refuses",
            "every commit there \u{2014} claim an issue, then work from `start_branch`'s checkout",
            "(under `Workflow: assisted` the way in is `assist` in a worktree).",
        ]);
    } else {
        caveats.extend([
            "with no claim on a checkout, this refuses nothing: the configured tracker cannot",
            "adjudicate an unsworn write here.",
        ]);
    }
    caveats.push(
        "`git push --no-verify` bypasses the hook, not the agent gate \u{2014} a guard rail.",
    );
    caveats
}

/// The evidence standard the gate acts on, given whether the contract could be read.
///
/// A contract nobody could read narrows the grant. Widening on a fault is how a
/// delegated context gets a capability out of a broken file.
fn effective_evidence(
    unreadable: bool,
    installed: crate::config::Evidence,
) -> crate::config::Evidence {
    if unreadable {
        crate::config::Evidence::Reading
    } else {
        installed
    }
}

fn effective_workflow(
    unreadable: bool,
    installed: crate::config::Workflow,
) -> Option<crate::config::Workflow> {
    (!unreadable).then_some(installed)
}

/// Everything the gate needs, or nothing if the harness is not installed.
fn gate_context(cwd: &str) -> Result<harness::GateContext, Refusal> {
    let repo_dir = if cwd.trim().is_empty() {
        launch_directory()?
    } else {
        std::path::PathBuf::from(cwd)
    };
    // Never a refusal: a missing transport is a decision `gate` makes, not a
    // reason to have no context to decide in. See `harness::control_surface`.
    let skill_root = harness::control_surface();
    // Defaults rather than refusing, with this repository's rows layered on top.
    let (installed, complaint) =
        skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir);
    let unreadable = complaint.is_some();
    let tracker = installed.tracker;
    let repository_host = installed.repository_host;
    let boundaries = installed.boundaries;
    // Read here: an unparsable record is **not** a stand-down in force.
    let state_root = harness::session::state_root(None)?;
    // An unreadable stand-down is discarded here; `doctor` reports the file.
    let stand_down = match harness::standdown::standing(&state_root) {
        harness::standdown::Standing::Declared(record) => Some(record),
        harness::standdown::Standing::Away | harness::standdown::Standing::Unreadable(_) => None,
    };

    Ok(harness::GateContext {
        integration: installed.integration,
        issue_requirement: installed.issue_requirement,
        workflow: effective_workflow(unreadable, installed.workflow),
        // The repository's own schedule over the contract, failing closed on
        // an enabled value that does not parse. Read off `installed` so the
        // crossing in `tests/honesty.rs` sees which row this field answers.
        operation_window: harness::operation_window::effective_in(
            &skill_root,
            &repo_dir,
            &installed.git_operation_window,
        ),
        // Empty is not a flag: a variable set to nothing is how a shell says
        // "unset", and treating it as a declaration would open the gate for
        // anybody who exported it once and forgot.
        flag: std::env::var("ESTIGIA_FLAG")
            .ok()
            .filter(|name| !name.trim().is_empty()),
        stand_down,
        skill_root,
        repo_dir,
        state_root,
        // The operator's window or shorter; an unreadable contract grants none.
        window: if unreadable {
            std::time::Duration::ZERO
        } else {
            installed.window.min(harness::hook::default_window())
        },
        evidence: effective_evidence(unreadable, installed.evidence),
        tracker,
        repository_host,
        boundaries,
    })
}

/// A transport answer on its way to a terminal rather than to a model.
///
/// The MCP path renders a tool answer compact, because the model reading it
/// pays for every byte. A person reading the same string in a terminal is the
/// case that trade does not cover, so it is re-indented here. Anything that
/// does not parse is passed through untouched: the answer may carry a prose
/// note appended after the JSON, and a person is better served by seeing it
/// than by a renderer that swallows what it cannot parse.
fn indented_for_a_person(answer: String) -> String {
    serde_json::from_str::<serde_json::Value>(&answer)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or(answer)
}

/// Claims an issue, and binds this run's writes to it.
fn claim(
    issue: &str,
    run_id: &str,
    horizon: &str,
    state: &str,
    transition_to: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let number: u64 = issue
        .trim()
        .parse()
        .map_err(|_| harness::issue_not_a_number(issue))?;
    let mut arguments = serde_json::json!({
        "issue": number,
        "run_id": run_id,
        "horizon": horizon,
        "state": state,
    });
    if let Some(to) = transition_to {
        arguments["transition_to"] = serde_json::json!(to);
    }
    let result = tool("claim", &arguments)?;
    if json {
        print_json(&serde_json::json!({
            "sworn": true,
            "issue": number,
            "run_id": run_id,
            "state": state,
            "transport": serde_json::from_str::<serde_json::Value>(&result)
                .unwrap_or(serde_json::Value::String(result.clone())),
        }));
        return Ok(());
    }
    // Re-indented on the way to a terminal. The transport answer arrives in the
    // compact spelling the MCP path renders for a model, and this is the one
    // road that hands that same string to a person.
    say!("{}", indented_for_a_person(result));
    say!("sworn: {run_id} holds #{number} in {state}");
    say!("  a repository write the gate sees is now measured against that claim, and");
    say!("  every irreversible boundary re-reads the timeline.");
    say!("  writes from an agent Estigia does not gate are not seen — `estigia status`");
    say!("  says which agents it holds the tools for, and the push guard holds the push");
    say!("  whoever types it.");
    Ok(())
}

/// Relays a sealed reviewer the host launched: the prompt out, or the answer in.
///
/// The OpenCode and Pi review relays run this on a machine with no reviewer CLI.
/// `prompt` writes the exact bytes `launch_review_input` recorded, `model` the model its
/// routing row named; `admit` reads
/// the reviewer's raw answer on standard input and binds it to the same receipt,
/// refusing a stale one with the class the taxonomy already carries.
fn review(
    action: &ReviewAction,
    run_id: Option<&str>,
    session_id: Option<&str>,
    runtime: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let identity = goal_identity("estigia review", run_id, session_id, runtime)?;
    let gate = gate_context("")?;
    let named = gate.tracker.named_repo();
    let context =
        crate::transport::Context::live(gate.skill_root.clone(), gate.repo_dir, named.clone());
    let _gh_pin = harness::gh_repo_pin();
    let host_repo = match gate.repository_host.named_repo() {
        Some(host) => Some(host),
        None => crate::transport::claim::host_repo_of(&context).ok(),
    };
    harness::apply_gh_repo_locked(named.as_deref().or(host_repo.as_deref()));
    let stamp = harness::session::stamp_of(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
    );
    match action {
        ReviewAction::Prompt { name } => {
            let prompt = crate::transport::claim::relay_prompt(&context, &identity, name)
                .map_err(|failure| crate::transport::claim::failure_refusal(&failure))?;
            let mut out = std::io::stdout();
            out.write_all(&prompt).map_err(|error| {
                Refusal::not_started(
                    "write-failed",
                    format!(
                        "the reviewer's prompt could not be written to standard output: {error}"
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::WorldAction,
                        "a writable standard output for the relay",
                    ),
                )
                .classed(crate::outcome::RefusalClass::Unknown)
            })?;
            out.flush().ok();
            Ok(())
        }
        ReviewAction::Model { name } => {
            let model = crate::transport::claim::relay_model(&context, &identity, name)
                .map_err(|failure| crate::transport::claim::failure_refusal(&failure))?;
            if json {
                print_json(&serde_json::json!({ "model": model }));
            } else if let Some(model) = model {
                say!("{model}");
            }
            Ok(())
        }
        ReviewAction::Admit { name } => {
            use std::io::Read as _;
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes).map_err(|error| {
                Refusal::not_started(
                    "read-failed",
                    format!("the reviewer's answer could not be read from standard input: {error}"),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::WorldAction,
                        "a readable standard input carrying the reviewer's answer",
                    ),
                )
            })?;
            let answer =
                crate::transport::claim::relay_admit(&context, &identity, name, &bytes, &stamp)
                    .map_err(|failure| crate::transport::claim::failure_refusal(&failure))?;
            if json {
                print_json(&answer);
            } else {
                say!(
                    "admitted the answer of `{name}` for {identity} (job {})",
                    answer
                        .get("job")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                );
            }
            Ok(())
        }
    }
}

/// Recovers an escalated review on a named maintainer's word, and says what that did.
fn recover(
    issue: &str,
    epoch: Option<&str>,
    authorized_by: Option<&str>,
    reason: &str,
    json: bool,
) -> Result<(), Refusal> {
    let number: u64 = issue
        .trim()
        .parse()
        .map_err(|_| harness::issue_not_a_number(issue))?;
    let gate = gate_context("")?;
    let named = gate.tracker.named_repo();
    let context =
        crate::transport::Context::live(gate.skill_root.clone(), gate.repo_dir, named.clone());
    let _gh_pin = harness::gh_repo_pin();
    let host_repo = match gate.repository_host.named_repo() {
        Some(host) => host,
        None => {
            harness::apply_gh_repo_locked(None);
            crate::transport::claim::host_repo_of(&context).map_err(|failure| {
                harness::tracker::translate(
                    &harness::tracker::Answer {
                        code: failure.code(),
                        body: Some(failure.envelope()),
                    },
                    "recover",
                )
                .unwrap_or_else(|| unreachable!("a failed call is never exit code 0"))
            })?
        }
    };
    harness::apply_gh_repo_locked(named.as_deref());
    let stamp = harness::session::stamp_of(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
    );
    match crate::transport::claim::recover_review(
        &context,
        &crate::transport::claim::RecoverReview {
            issue: number,
            epoch,
            authorized_by,
            reason,
            host_repo: &host_repo,
            now: &stamp,
        },
    ) {
        Ok(answer) => {
            if json {
                print_json(&answer);
            } else {
                say!(
                    "recovered: #{number} epoch {}, authorised by {} ({})",
                    answer["epoch"].as_str().unwrap_or_default(),
                    answer["authorized_by"].as_str().unwrap_or_default(),
                    answer["role"].as_str().unwrap_or_default()
                );
                say!("  nothing is released: the next publication on the pull request is a fresh");
                say!("  candidate in a successor lineage, reviewed again under the same tier.");
                say!(
                    "  pinned: tier {}, lens focus {} \u{2014} acquire the panel that tier licenses (four \
                     lenses at high, the focus's lens at medium); `review_tier` does not size it",
                    answer["level"].as_str().unwrap_or_default(),
                    answer["focus"].as_str().unwrap_or_default()
                );
            }
            Ok(())
        }
        Err(failure) => {
            let answer = harness::tracker::Answer {
                code: failure.code(),
                body: Some(failure.envelope()),
            };
            Err(harness::tracker::translate(&answer, "recover")
                .unwrap_or_else(|| unreachable!("a failed call is never exit code 0")))
        }
    }
}

/// Puts the issue down and forgets the run.
fn release(run_id: &str, json: bool) -> Result<(), Refusal> {
    let context = gate_context("")?;
    let run = harness::session::load(&context.state_root, run_id);
    // Unreadable is not absent: an unknown is never reported as nothing held.
    if run.unreadable {
        let pointer = harness::session::pointer_path(&context.state_root, run_id);
        return Err(Refusal::not_started(
            "run-pointer-unreadable",
            format!(
                "{run_id} wrote a pointer Estigia can no longer read, so what it holds is \
                 unknown: {}",
                pointer.display()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what that run holds, read from the tracker \u{2014} then that file removed, \
                 which is Estigia's own state and no claim of anybody's",
            ),
        ));
    }
    let Some(issue) = run.issue.clone() else {
        return Err(Refusal::not_started(
            "nothing-held",
            format!("{run_id} holds no issue"),
            Resolution::run("estigia status"),
        ));
    };
    let answer = match tool(
        "release",
        &serde_json::json!({ "issue": issue, "run_id": run_id }),
    ) {
        Ok(answer) => answer,
        // A phantom pointer arrives as this refusal; forgetting the pointer here is
        // what makes the command clear the block it names.
        Err(refusal) if refusal.code == "nothing-to-unassign" => {
            return forget_since_nothing_to_release(&context, &run, run_id, &issue, json);
        }
        Err(refusal) => return Err(refusal),
    };
    let found: serde_json::Value = serde_json::from_str(&answer).unwrap_or_default();
    let mut written = found.clone();
    if found.get("write_performed") == Some(&serde_json::json!(false)) {
        let Some(target) = found
            .get("target_operation")
            .and_then(|value| value.as_str())
        else {
            // The tracker read succeeded; this is its answer.
            return forget_since_nothing_to_release(&context, &run, run_id, &issue, json);
        };
        let released = tool(
            "release",
            &serde_json::json!({
                "issue": issue,
                "run_id": run_id,
                "target_operation": target,
            }),
        )?;
        written = serde_json::from_str(&released).unwrap_or_default();
    }
    let checkout = written.get("checkout");
    if json {
        let mut answer = serde_json::json!({
            "released": true,
            "issue": issue,
            "run_id": run_id,
        });
        if let Some(checkout) = checkout {
            answer["checkout"] = checkout.clone();
        }
        print_json(&answer);
        return Ok(());
    }
    say!("released: {run_id} no longer holds #{issue}");
    if let Some(checkout) = checkout {
        let worktree = checkout
            .get("worktree")
            .and_then(|value| value.as_str())
            .unwrap_or("its checkout");
        // The partial case the `removed` conjunction cannot name: the directory
        // went and the branch stayed. Saying "kept" there reports a state the run
        // did not read back — the directory is gone.
        if checkout.get("outcome").and_then(|value| value.as_str())
            == Some("worktree-removed-branch-kept")
        {
            let branch = checkout
                .get("branch")
                .and_then(|value| value.as_str())
                .unwrap_or("its branch");
            let reason = checkout
                .get("detail")
                .and_then(|value| value.as_str())
                .unwrap_or("no reason recorded");
            say!("removed {worktree}; kept branch {branch}: {reason}");
        } else if checkout.get("removed") == Some(&serde_json::json!(true)) {
            say!("removed {worktree}");
        } else if let Some(reason) = checkout.get("detail").and_then(|value| value.as_str()) {
            say!("kept {worktree}: {reason}");
        }
    }
    Ok(())
}

/// Puts down a pointer the tracker has just said holds no epoch of its own.
fn forget_since_nothing_to_release(
    context: &harness::GateContext,
    run: &harness::session::Run,
    run_id: &str,
    issue: &crate::identity::Identity,
    json: bool,
) -> Result<(), Refusal> {
    let same_repository = run
        .repo_dir
        .as_deref()
        .is_some_and(|recorded| harness::same_git_repository(recorded, &context.repo_dir));
    if !same_repository {
        // The recorded checkout may be gone; the resolution says so without
        // presuming which case this is.
        let pointer = harness::session::pointer_path(&context.state_root, run_id);
        return Err(Refusal::not_started(
            "release-repository-mismatch",
            format!(
                "{run_id} recorded its claim against a different repository than the one this \
                 release was run against, so a successful read here says nothing about that \
                 pointer's own timeline \u{2014} its local record is left exactly as it was"
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                format!(
                    "run `estigia release --run-id {run_id}` from the checkout that pointer's \
                     own claim covers, if it still exists \u{2014} and if it does not, its \
                     pointer is Estigia's own local state and no claim of anybody's, removable \
                     by hand: {}",
                    pointer.display()
                ),
            ),
        ));
    }
    harness::session::forget(&context.state_root, run_id);
    if json {
        print_json(&serde_json::json!({
            "released": false,
            "forgotten": true,
            "issue": issue,
            "run_id": run_id,
        }));
        return Ok(());
    }
    say!("{run_id} holds no epoch of #{issue} on the tracker; its local pointer is forgotten");
    Ok(())
}

/// Runs one workflow tool and returns its text, or the refusal it carried.
///
/// The refusal is passed through **whole**.
fn tool(name: &str, arguments: &serde_json::Value) -> Result<String, Refusal> {
    let context = gate_context("")?;
    match harness::mcp::run_tool(name, arguments, Ok(&context)) {
        Ok(result) => Ok(result["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned()),
        Err(harness::mcp::ToolFailure::Refused(refusal)) => Err(*refusal),
        Err(harness::mcp::ToolFailure::Malformed(message)) => Err(Refusal::not_started(
            "tool-arguments-invalid",
            message,
            Resolution::run(format!("estigia {name} --help")),
        )),
    }
}

/// Which sub-agent a gate payload names, if it names one.
fn payload_agent(parsed: &serde_json::Value) -> Option<&str> {
    ["agent_type", "agent_name"]
        .into_iter()
        .find_map(|key| parsed.get(key).and_then(serde_json::Value::as_str))
        .filter(|agent| !agent.trim().is_empty())
}

/// The directory this process was launched in.
fn launch_directory() -> Result<std::path::PathBuf, Refusal> {
    std::env::current_dir().map_err(|error| {
        Refusal::not_started(
            "working-directory-unknown",
            format!("{error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "a working directory the process can read",
            ),
        )
    })
}

/// The checkout the payload named, or nothing.
///
/// Deliberately **not** where OpenCode's `workdir` is read; see
/// [`narrowed_by_the_call`].
fn payload_cwd(parsed: &serde_json::Value) -> &str {
    parsed
        .get("cwd")
        .or_else(|| parsed.get("tool_input").and_then(|inner| inner.get("cwd")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

/// The directory the **call itself** named, clamped to somewhere the gate was
/// already going to look: inside the launch checkout, or a checkout a
/// pointer already records — see [`held_by_a_pointer`].
///
/// Bash names where the command runs; a write tool names where the write lands.
/// Either may only narrow to one of those two — read as freely as a host's
/// `cwd`, a model-written argument stops being evidence and becomes a lever.
///
/// The second value is which alias was read, when one was — the one fact
/// this function alone knows, so the caller reports it rather than
/// re-deriving it from `tool` a second time. `None` when this call named
/// nothing at all, the one case there was no alias to read.
///
/// The third value says why the first is `None`, when it is worth saying:
/// `None` when this call named nothing at all, and a reason for a value that
/// *was* named but discarded — clamped outside the launch directory and held
/// by no pointer, or unplaceable — because those are different facts for a
/// reader from having nothing to check at all.
fn narrowed_by_the_call(
    parsed: &serde_json::Value,
    tool: &str,
    launched: &std::path::Path,
    state_root: &std::path::Path,
    run_id: Option<&str>,
) -> (
    Option<std::path::PathBuf>,
    Option<crate::outcome::CheckoutSource>,
    Option<&'static str>,
) {
    // OpenCode 1 names its shell tool `bash`; OpenCode 2 names it `shell`.
    let is_shell = crate::harness::SHELL_TOOLS
        .iter()
        .any(|shell| tool.eq_ignore_ascii_case(shell));
    let is_write_tool = crate::harness::WRITE_TOOLS
        .iter()
        .any(|write| tool.eq_ignore_ascii_case(write));
    // Which alias this call carries evidence in — known unconditionally,
    // since it depends only on `tool`, and needed whether reading that
    // evidence succeeds or is discarded partway through.
    let source = if is_shell {
        crate::outcome::CheckoutSource::PayloadWorkdir
    } else {
        crate::outcome::CheckoutSource::WriteTarget
    };
    // `Ok(None)` and `Err` are two different facts, not one collapsed by a
    // `?`: `Ok(None)` is *this call carries no such evidence at all* (wrong
    // tool, or the key that would carry it is absent or blank), and `Err` is
    // *evidence was named, and this is why it never became a candidate to
    // clamp*. A `?` inside this closure once let the write-target road's own
    // placement failure fall through as the first fact when it was the
    // second — the exact confusion `ShellCommandArgument.literal` was
    // reshaped to end last round, here for the same reason.
    let named: Result<Option<String>, &'static str> = (|| {
        // Bash names where the command runs; a write tool names where the
        // write lands. Each is the only evidence that tool carries about
        // where the call happens, and each is clamped the same way below.
        if is_shell {
            Ok(parsed
                .get("workdir")
                .or_else(|| {
                    parsed
                        .get("tool_input")
                        .and_then(|inner| inner.get("workdir"))
                })
                .and_then(serde_json::Value::as_str)
                .filter(|named| !named.trim().is_empty())
                .map(str::to_owned))
        } else if is_write_tool {
            // The target the write lands on, read through the one owner of the
            // aliases. `path_argument` answers with the file path; the directory
            // containing it is the evidence handed back — not the directory
            // finally adjudicated, which [`checkout_root_covering`] resolves back
            // to the covering checkout's own root before this candidate ever
            // becomes `context.repo_dir`.
            // Blank is filtered here for the same reason `workdir` is filtered
            // above, and it has to be filtered *before* the join: a target of
            // spaces is a relative path on Unix, so `launched.join("   ")` is a
            // real directory whose parent is the launch directory — evidence
            // conjured out of a value that named nothing. Windows collapses the
            // same spelling differently and answered `None` already, so the two
            // platforms disagreed until this line existed.
            let Some(target) = parsed
                .get("tool_input")
                .and_then(crate::harness::path_argument)
                .or_else(|| crate::harness::path_argument(parsed))
                .filter(|target| !target.trim().is_empty())
            else {
                // No path argument at all — nothing was named, the same fact
                // as a shell call carrying no `workdir`.
                return Ok(None);
            };
            let target = std::path::PathBuf::from(target);
            let resolved = if target.is_absolute() {
                target
            } else {
                launched.join(target)
            };
            // Placed before the parent, in the order the clamp needs: `placed`
            // collapses the spelling the way the platform does, so a bare `..` (or
            // one that climbs through a link) is seen for where it lands rather
            // than for the component it hangs off. Its parent is then the
            // directory the write lands in.
            //
            // A target was named — `path_argument` answered — so a path that
            // cannot be placed, or that places with no parent (root itself),
            // is *this evidence, discarded*, not *no evidence at all*.
            let Some(placed) = crate::paths::placed(&resolved) else {
                return Err("the named directory could not be placed");
            };
            let Some(directory) = placed.parent() else {
                return Err("the named directory could not be placed");
            };
            let Some(directory) = directory.to_str() else {
                return Err("the named directory could not be placed");
            };
            Ok(Some(directory.to_owned()))
        } else {
            Ok(None)
        }
    })();
    let named = match named {
        Ok(Some(named)) => named,
        Ok(None) => return (None, None, None),
        // Something was named, and reading it already established why it
        // never becomes a candidate — the clamp below never runs.
        Err(reason) => return (None, Some(source), Some(reason)),
    };
    let named = std::path::PathBuf::from(named);
    let resolved = if named.is_absolute() {
        named
    } else {
        launched.join(named)
    };
    // `placed`, not `covers`: `covers` was written for directories that exist and
    // resolves an unresolvable path literally, answering *inside* for a path that is not.
    //
    // A second admissible target sits beside `launched`: a checkout a pointer
    // already records — see [`held_by_a_pointer`]. `launched` alone is a
    // sibling-blind clamp, and `Worktree location` on a real install is
    // `<repo-parent>\worktree\<repo>\<branch>`, a **sibling** of the
    // repository it was minted from, never a child of it.
    match crate::paths::placed(&resolved) {
        Some(placed)
            if crate::paths::covers(launched, &placed)
                || held_by_a_pointer(state_root, run_id, &placed) =>
        {
            (Some(placed), Some(source), None)
        }
        // Named, placed, and neither inside the launch directory nor a
        // checkout a pointer already records: the clamp refused it rather
        // than trusting a model- or host-written value to reach outside
        // where this process was launched or outside every claim it already
        // knows about. Distinct from `checkout_root_covering`'s own "clamped
        // outside the launch directory" — that one is about a *holder's
        // recorded root* escaping `launched`, not the call's own named
        // candidate.
        Some(_) => (
            None,
            Some(source),
            Some("the named directory is outside the launch directory and held by no pointer"),
        ),
        // Named, but not even placeable — the same shape `context_the_payload_names`
        // already distinguishes for the payload `cwd`; this is its sibling for
        // `workdir`/the write target.
        None => (
            None,
            Some(source),
            Some("the named directory could not be placed"),
        ),
    }
}

/// The checkout root a narrowed candidate directory actually belongs to,
/// when exactly one currently-known holding covers it — `None` otherwise,
/// including when nothing covers it at all.
///
/// A write's target is almost always **inside** a subdirectory of whatever
/// checkout it belongs to — `<worktree>/src/main.rs`, never
/// `<worktree>/f.txt` — and role-definition lookup and this checkout's own
/// configuration are both read from `context.repo_dir` **directly**, with no
/// upward search: `roles::definition_for` joins `.claude/agents` onto
/// exactly the directory it is given, and so does the repository's own
/// configuration layer. Handing either of them `<worktree>/src` instead of
/// `<worktree>` does not merely mis-adjudicate a claim, it makes a
/// sub-agent's own declared tool list invisible — measured, driving `Edit`
/// with a `file_path` one directory below a checkout root and a `reader`
/// sub-agent definition sitting at that root: the role refusal that
/// definition earns disappeared, and the call fell through to an ordinary
/// claim check instead.
///
/// So the candidate `narrowed_by_the_call` clamped is only *evidence of
/// which checkout the call belongs to*, never the directory adjudicated
/// directly. Resolved back to the covering holder's own recorded root — the
/// same value [`harness::guard::holders_of`] already matched it against —
/// this is a no-op for the ordinary case the existing `workdir` clamp
/// exercises, where the value named a checkout root exactly to begin with,
/// and it is the fix for the case a write target is one directory further
/// in: two or more overlapping candidates, or none, both fall back to
/// `launched` in [`launched_context`], which is the checkout root the call
/// was already going to be judged against before this alias was read.
///
/// Every value this filters comes from a holder's own `covered()`, so the
/// one property still worth enforcing is the direction the clamp exists to
/// refuse: a covered root is dropped only when it is a **strict ancestor**
/// of `launched` (`covers(covered, launched)` true, and not the same
/// directory). `launched` can itself be a subdirectory of a wider checkout —
/// a package directory in a monorepo, or anywhere a person stands when they
/// type `estigia gate`, both named in [`crate::paths::covers`]'s own doc —
/// and a covered root above it climbs *above* `launched`, which
/// `checkout_root_covering_never_climbs_above_the_launch_directory` pins.
/// A covered root equal to `launched`, or disjoint from it (a sibling
/// worktree, including one whose git directory is a different repository),
/// both survive the drop. That is what this filter **gives up**: sideways
/// roots are admitted on purpose under the one-task-per-worktree invariant,
/// and `context.repo_dir` is then that root — so configuration
/// (`gate_context` → `installed_config_in_keeping_what_parses`) and role
/// lookup (`roles::definition_for`) follow it, not only the verdict binding
/// `stale_verdict` already discloses. `docs/honesty.md` names all three
/// readers. When nothing survives the drop, the call falls back to
/// `launched` in [`launched_context`], exactly as it does for the "two or
/// more holders" and "no holder" cases above. The equal-to-`launched` case
/// *is* observable, through the reason rather than the directory: dropping
/// this covered root falls back to the same directory keeping it resolves
/// to, but [`launched_context`] reports this function's own reason and
/// `CheckoutSource::LaunchDirectory` for the drop, while keeping it reports
/// the alias `narrowed_by_the_call` read and an empty `dropped`, so
/// `Refusal.observed` still tells them apart even though `context.repo_dir`
/// cannot. The drop condition is shaped to cover only a **strict ancestor**
/// of `launched` for symmetry with the sibling case; the equal case is
/// exempt because dropping it would state, of the launch directory itself,
/// that it was clamped outside the launch directory.
fn checkout_root_covering(
    state_root: &std::path::Path,
    candidate: &std::path::Path,
    launched: &std::path::Path,
) -> (Option<std::path::PathBuf>, Option<&'static str>) {
    let holders = harness::guard::holders_of(state_root, candidate);
    let [holder] = holders.as_slice() else {
        return (
            None,
            Some(if holders.is_empty() {
                "uncovered by any holder"
            } else {
                // `holders_of`'s own doc: two or more runs tied at the
                // closest depth are still two holders, and still ambiguous —
                // not the same fact as nothing covering it at all.
                "covered by more than one holder"
            }),
        );
    };
    let resolved = holder
        .covered()
        .filter(|covered| {
            !crate::paths::covers(covered, launched)
                || crate::paths::same_directory(covered, launched)
        })
        .filter_map(|covered| {
            crate::paths::coverage_depth(covered, candidate).map(|depth| (depth, covered.clone()))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, covered)| covered);
    if resolved.is_none() {
        return (None, Some("clamped outside the launch directory"));
    }
    (resolved, None)
}

/// Whether `named` is a checkout a pointer already records — this run's own
/// when an identity was given, or whoever holds `named` when it was not.
///
/// A worktree holds exactly one task, so the worktree is the unit of task
/// identity and `repo_dir` is not: several runs sharing one launch checkout
/// is the designed steady state. That is what lets a call carrying no
/// identity still resolve a named worktree to the run that minted it: the
/// closest cover wins, and no other run's pointer records that worktree. Not
/// because a worktree sits beside the repository — `coverage_depth`'s own doc
/// records that an operator may nest one inside it, where another run's
/// `repo_dir` does reach it — but because it loses on depth there. That is
/// also why [`harness::guard::holders_of`] answering with anything but exactly one
/// holder, whose own `worktree` covers `named`, widens nothing: `repo_dir`
/// carries no task identity of its own, so neither arm may resolve through
/// it — the identity-less arm for the reason above, and the identity-
/// carrying arm below because a worktree nested inside the base checkout
/// (`coverage_depth`'s own doc contemplates the layout) would otherwise let
/// one run's `repo_dir` cover another run's worktree, admitting it under the
/// wrong identity.
///
/// Both arms read only `worktree`, never `repo_dir`: the difference between
/// them is whose pointer is consulted, not which field. With an identity,
/// only that run's own `worktree` is read; without one, `holders_of`
/// restricts to a single holder and that holder's `worktree` is read
/// instead. How the call's own argument is read against the pointer-recorded
/// set is in `docs/honesty.md`. An unreadable
/// pointer named by an identity answers `false`: an unreadable control
/// surface widens nothing on that road. What the identity-less road does
/// with an unreadable pointer instead — and what an identity-less call
/// still cannot refuse, and the `cwd` road this is narrower than — are in
/// `docs/honesty.md`.
fn held_by_a_pointer(
    state_root: &std::path::Path,
    run_id: Option<&str>,
    named: &std::path::Path,
) -> bool {
    match run_id {
        Some(id) => {
            let run = harness::session::load(state_root, id);
            !run.unreadable
                && run
                    .worktree
                    .as_deref()
                    .is_some_and(|worktree| crate::paths::covers(worktree, named))
        }
        None => matches!(
            harness::guard::holders_of(state_root, named).as_slice(),
            [holder] if holder
                .worktree
                .as_deref()
                .is_some_and(|worktree| crate::paths::covers(worktree, named))
        ),
    }
}

/// The checkout this process was already going to look at: the launch
/// directory, optionally narrowed by a clamped workdir or write target and
/// then resolved to the checkout root that narrowing actually named — see
/// [`checkout_root_covering`] for why the raw candidate is never
/// used as `context.repo_dir` directly.
fn launched_context(
    parsed: &serde_json::Value,
    tool: &str,
    run_id: Option<&str>,
) -> Result<(harness::GateContext, crate::outcome::CheckoutObserved), Refusal> {
    let launched = launch_directory()?;
    let state_root = harness::session::state_root(None)?;
    let (candidate, named_source, named_dropped) =
        narrowed_by_the_call(parsed, tool, &launched, &state_root, run_id);
    let (looking_at, source, dropped): (_, _, Vec<&'static str>) = match candidate {
        // `named_dropped` carries forward here: `None` when nothing was
        // named at all, `Some(reason)` when something was and the clamp
        // inside `narrowed_by_the_call` refused it — the confusion this
        // change exists to close, since both used to arrive here as one
        // bare `None`.
        None => (
            launched.clone(),
            crate::outcome::CheckoutSource::LaunchDirectory,
            named_dropped.into_iter().collect(),
        ),
        Some(candidate) => {
            // `narrowed_by_the_call` hands back the alias it read alongside
            // the candidate it read from — a `Some` candidate is always
            // paired with a `Some` source, since both come from the same
            // branch there.
            let source = named_source.unwrap_or(crate::outcome::CheckoutSource::LaunchDirectory);
            match checkout_root_covering(&state_root, &candidate, &launched) {
                (Some(root), _) => (root, source, Vec::new()),
                (None, dropped) => (
                    launched.clone(),
                    crate::outcome::CheckoutSource::LaunchDirectory,
                    dropped.into_iter().collect(),
                ),
            }
        }
    };
    let context = gate_context(&looking_at.to_string_lossy())?;
    let observed = crate::outcome::CheckoutObserved {
        adjudicated: looking_at.to_string_lossy().into_owned(),
        source,
        dropped,
    };
    Ok((context, observed))
}

/// The checkout the payload named, unless naming it would take the write
/// out of the gate.
///
/// An uncovered name is dropped rather than believed.
fn context_the_payload_names(
    parsed: &serde_json::Value,
    tool: &str,
    run_id: Option<&str>,
) -> Result<(harness::GateContext, crate::outcome::CheckoutObserved), Refusal> {
    let stated = payload_cwd(parsed);
    if stated.trim().is_empty() {
        return launched_context(parsed, tool, run_id);
    }
    // Placed before asking who covers it: `coverage_depth` keeps an unresolvable
    // spelling; a path that cannot be placed is dropped, not believed.
    let named = std::path::PathBuf::from(stated);
    let named = if named.is_absolute() {
        named
    } else {
        launch_directory()?.join(named)
    };
    let Some(placed) = crate::paths::placed(&named) else {
        // Prepended, not appended: the payload `cwd` was the first candidate
        // tried, and `launched_context`'s own reason — when it has one, for
        // its own `workdir`/target candidate — is a second, later fact, not
        // a replacement for this one.
        let (context, mut observed) = launched_context(parsed, tool, run_id)?;
        observed
            .dropped
            .insert(0, "the payload cwd could not be placed");
        return Ok((context, observed));
    };
    let named_context = gate_context(&placed.to_string_lossy())?;
    if harness::guard::holders_of(&named_context.state_root, &named_context.repo_dir).is_empty() {
        let (context, mut observed) = launched_context(parsed, tool, run_id)?;
        observed
            .dropped
            .insert(0, "the payload cwd names a checkout no holder covers");
        return Ok((context, observed));
    }
    let adjudicated = named_context.repo_dir.to_string_lossy().into_owned();
    Ok((
        named_context,
        crate::outcome::CheckoutObserved {
            adjudicated,
            source: crate::outcome::CheckoutSource::PayloadCwd,
            dropped: Vec::new(),
        },
    ))
}

/// The one run id this call carries, however the caller named it.
///
/// `--run-id` is the id itself; `--session-id` with `--runtime` derives it here,
/// so a host never composes a run id of its own; `ESTIGIA_RUN_ID` is the
/// ambient fallback and a derived id supersedes it, so an operator's exported
/// variable cannot collide with the identity a host forwards. Both explicit
/// sources at once are refused rather than ordered: a precedence would pick an
/// identity nobody chose.
///
/// `command` is the verb the refusal belongs to — `estigia gate <tool>` or
/// `estigia goal` — so a `goal` caller is never told to run `estigia gate`.
fn gate_identity(
    command: &str,
    run_id: Option<&str>,
    session_id: Option<&str>,
    runtime: Option<&str>,
    ambient_run_id: Option<&str>,
) -> Result<Option<String>, Refusal> {
    let resolution = || {
        Resolution::run(format!(
            "{command} --session-id <session-id> --runtime <runtime>"
        ))
    };
    let ambient_run_id = ambient_run_id.filter(|value| !value.trim().is_empty());
    match (run_id, session_id, runtime) {
        (Some(_), Some(_), _) => Err(Refusal::not_started(
            "gate-identity-over-specified",
            "--run-id and --session-id both name this call's run",
            resolution(),
        )),
        (Some(run_id), None, None) => Ok(Some(run_id.to_owned())),
        (None, Some(session), Some(runtime)) => {
            Ok(Some(harness::session::run_id(runtime, session)))
        }
        (None, Some(_), None) => Err(Refusal::not_started(
            "gate-identity-incomplete",
            "--session-id was given without the --runtime that derives its run id",
            resolution(),
        )),
        (Some(_), None, Some(_)) | (None, None, Some(_)) => Err(Refusal::not_started(
            "gate-identity-incomplete",
            "--runtime is read only together with --session-id",
            resolution(),
        )),
        (None, None, None) => Ok(ambient_run_id.map(str::to_owned)),
    }
}

/// The run a delegated context is decided as: its root session's.
///
/// No fallback to the child's own session: a parent that names no live run
/// leaves the child with exactly the parent's answer, not a run of its own that
/// holds nothing and so lets the write through.
fn parent_identity(
    run_id: Option<&str>,
    runtime: Option<&str>,
    parent: &str,
) -> Result<Option<String>, Refusal> {
    let resolution = || {
        Resolution::run("estigia gate <tool> --parent-session-id <session-id> --runtime <runtime>")
    };
    match (run_id, runtime) {
        (Some(_), _) => Err(Refusal::not_started(
            "gate-identity-over-specified",
            "--run-id and --parent-session-id both name this call's run",
            resolution(),
        )),
        (None, None) => Err(Refusal::not_started(
            "gate-identity-incomplete",
            "--parent-session-id was given without the --runtime that derives its run id",
            resolution(),
        )),
        (None, Some(runtime)) => Ok(Some(harness::session::run_id(runtime, parent))),
    }
}

/// What `estigia goal` read about one run.
///
/// The five standings are the answer's whole vocabulary; `live` is true only
/// for `"passed"`, so a caller that cannot read the tracker never continues.
/// `claimable` is read only when the run is not live: the loop it answers
/// continues on a live claim, and on a queue read otherwise.
#[derive(Debug, Clone)]
pub(crate) struct GoalReading {
    /// The issue the pointer holds, when it holds one, as its tracker spells it.
    pub(crate) issue: Option<crate::identity::Identity>,
    /// The state the run believes the issue is in.
    pub(crate) state: Option<String>,
    /// `passed`, `stopped`, `unread`, `nothing-sworn` or `unreadable`.
    pub(crate) standing: &'static str,
    /// Whether the run still holds its issue, on a readable answer.
    pub(crate) live: bool,
    /// Why the tracker stopped it, when it did.
    pub(crate) reason: Option<String>,
    /// How many issues this run could claim, summed over the `review` and
    /// `ready` queues, when the run is not live and both queues could be read.
    ///
    /// `None` when the run is live (the queue is not read) or when a queue
    /// could not be read — never zero, because a failed read is not an empty
    /// queue and a loop must stop on it rather than early.
    pub(crate) claimable: Option<u64>,
    /// How many `blocked` items this run's own recorded exit conditions already
    /// release, summed over the `blocked` queue, when the run is not live and
    /// that queue could be read.
    ///
    /// `None` for the reason `claimable` is `None`, and never zero for a failed
    /// read. A condition this read cannot settle — a human adjudication, or a
    /// clause that is not well formed — counts nothing here rather than counting
    /// as discharged.
    pub(crate) dischargeable: Option<u64>,
    /// How many open issues in `in-progress` or `review` this run could not
    /// claim, when the run is not live and those queues could be read.
    ///
    /// `None` for the reason `claimable` is `None`.
    pub(crate) held: Option<u64>,
    /// How many `blocked` items whose clauses are all well formed, at least one
    /// recorded, and not all holding yet. `None` for the reason `claimable` is `None`.
    pub(crate) pending: Option<u64>,
    /// How many `blocked` items recorded no clause, or any clause that is not
    /// well formed even beside a well-formed one: a person's to settle. `None`
    /// for the reason `claimable` is `None`.
    pub(crate) unconditioned: Option<u64>,
    /// How many open issues the local store has never admitted, as `list_state`
    /// reports them, when the run is not live and that read landed.
    ///
    /// `None` for the reason `claimable` is `None`, and never zero for a read
    /// that did not land. They are counted in no state: only an explicit
    /// `transition` admits one.
    pub(crate) unadmitted: Option<u64>,
    /// How many issues this run's own delivery has closed, when the caller
    /// asked for the count and the tracker could be read.
    ///
    /// `None` when it was not asked for or could not be read — never zero for a
    /// failed read, for the same reason `claimable` is not.
    pub(crate) completed: Option<u64>,
    /// The reason this run asked its loop to stop after the current task, or
    /// `None` when no request was made or it cannot be read.
    ///
    /// Read whether or not the run is live: a live claim keeps the loop going
    /// regardless, but the value is the same read.
    pub(crate) stop_after: Option<String>,
}

/// `estigia sessions`: the list, or `send`.
fn sessions_command(action: Option<&SessionsAction>, json: bool) -> Result<(), Refusal> {
    match action {
        None => sessions::sessions(json),
        Some(SessionsAction::Send {
            session,
            text,
            file,
            run_id,
            session_id,
            runtime,
        }) => {
            let ambient = std::env::var("ESTIGIA_RUN_ID").ok();
            let sender = gate_identity(
                "estigia sessions send <run-id> \"<text>\"",
                run_id.as_deref(),
                session_id.as_deref(),
                runtime.as_deref(),
                ambient.as_deref(),
            )?;
            sessions::send(
                session,
                text.as_deref(),
                file.as_deref(),
                sender.as_deref(),
                json,
            )
        }
    }
}

/// `estigia session`: the context a host's session starts with, and the end of it.
fn session_lifecycle(action: &SessionAction, json: bool) -> Result<(), Refusal> {
    match action {
        SessionAction::Start {
            session_id,
            runtime,
            reason,
            previous_session_id,
        } => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let context = gate_context(&cwd.to_string_lossy())?;
            harness::session::maintain(&context.state_root);
            harness::registry::record_start(&context.state_root, runtime, session_id, &cwd);
            let run = harness::session::load(
                &context.state_root,
                &harness::session::run_id(runtime, session_id),
            );
            let from =
                previous_session_id
                    .as_deref()
                    .zip(reason.as_deref())
                    .map(|(previous, reason)| harness::hook::StartedFrom {
                        reason,
                        run_id: harness::session::run_id(runtime, previous),
                    });
            let text = harness::hook::session_start_text(&context, &run, from.as_ref());
            if json {
                print_json(&serde_json::json!({ "context": text }));
            } else {
                say!("{text}");
            }
            Ok(())
        }
        SessionAction::Deliver {
            session_id,
            runtime,
            peek,
            through,
        } => sessions::deliver(runtime, session_id, *peek, *through, json),
        SessionAction::End {
            session_id,
            runtime,
            reason,
        } => {
            if matches!(
                reason.as_deref(),
                Some("new" | "resume" | "fork" | "reload")
            ) {
                return Ok(());
            }
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let context = gate_context(&cwd.to_string_lossy())?;
            harness::registry::record_end(
                &context.state_root,
                runtime,
                session_id,
                reason.as_deref(),
            );
            harness::session::forget(
                &context.state_root,
                &harness::session::run_id(runtime, session_id),
            );
            Ok(())
        }
    }
}

/// `estigia goal`: the bare read, or one of its four subcommands.
fn goal(
    action: Option<&GoalAction>,
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    completed: bool,
    json: bool,
) -> Result<(), Refusal> {
    match action {
        None => goal_status(session_id, runtime, run_id, completed, json),
        Some(GoalAction::Progress { note }) => {
            goal_activity("progress", note, session_id, runtime, run_id, json)
        }
        Some(GoalAction::Blocked { reason }) => {
            goal_activity("blocked", reason, session_id, runtime, run_id, json)
        }
        Some(GoalAction::Complete) => goal_complete(session_id, runtime, run_id, json),
        Some(GoalAction::StopAfter { reason, clear }) => {
            goal_stop_after(reason.as_deref(), *clear, session_id, runtime, run_id, json)
        }
        Some(GoalAction::Start { text }) => goal_start(text, session_id, runtime, run_id, json),
    }
}

/// `estigia goal`: whether this session's run still holds a live claim, and
/// how much work it could claim if not.
///
/// Read-only, and the answer an OpenCode loop plugin asks for: one JSON object
/// under `--json`, one human line otherwise, and exit 0 either way. An identity
/// that does not name exactly one run refuses.
fn goal_status(
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    completed: bool,
    json: bool,
) -> Result<(), Refusal> {
    let identity = goal_identity("estigia goal", run_id, session_id, runtime)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = gate_context(&cwd.to_string_lossy())?;
    let reading = goal_reading(&context, &identity, completed);
    let goal = harness::session::loop_goal(&context.state_root, &identity);
    if json {
        let mut object = goal_json(&identity, &reading);
        if let Some(goal) = goal.as_ref()
            && let Some(fields) = object.as_object_mut()
        {
            fields.insert(
                "objective".to_owned(),
                serde_json::Value::String(goal.objective.clone()),
            );
            fields.insert(
                "max".to_owned(),
                goal.max
                    .map_or(serde_json::Value::Null, serde_json::Value::from),
            );
            fields.insert("loop_last".to_owned(), moment_json(goal.last.as_ref()));
            fields.insert(
                "loop_stopped".to_owned(),
                moment_json(goal.stopped.as_ref()),
            );
        }
        print_json(&object);
    } else {
        say!("{}", goal_human_line(&context, &identity, reading));
        match goal.as_ref() {
            Some(goal) => {
                say!("loop objective: {}", goal.objective);
                if let Some(max) = goal.max {
                    say!("loop completion cap: {max}");
                }
                say!(
                    "loop last outcome: {}",
                    match &goal.last {
                        Some(last) => format!("{} (at {})", last.reason, last.at),
                        None => "none recorded yet".to_owned(),
                    }
                );
                say!(
                    "loop stop reason: {}",
                    goal.stopped
                        .as_ref()
                        .map_or("none \u{2014} the loop is running", |stop| stop
                            .reason
                            .as_str())
                );
            }
            None => say!("no Estigia loop is stored for this run"),
        }
    }
    Ok(())
}

fn moment_json(moment: Option<&harness::session::LoopEnd>) -> serde_json::Value {
    match moment {
        Some(moment) => serde_json::json!({ "reason": moment.reason, "at": moment.at }),
        None => serde_json::Value::Null,
    }
}

/// `estigia goal start`: store this session's loop, or record the operator's
/// stop.
///
/// `/flow-loop` is a Claude Code slash command; this is what its body runs. Its
/// text is one argument — the objective and an optional `--max <n>` — and
/// `stop` records the portable stop-after request the loop honours at its next
/// turn end, so a stop needs no Claude-only mechanism.
fn goal_start(
    text: &str,
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let identity = goal_identity("estigia goal start", run_id, session_id, runtime)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = gate_context(&cwd.to_string_lossy())?;
    match start_loop(&context, &identity, text)? {
        StartOutcome::StopRequested => {
            if json {
                print_json(&serde_json::json!({
                    "run_id": identity,
                    "loop": "stopping",
                }));
            } else {
                say!(
                    "the loop for {identity} will stop at its next turn end; the current task is \
                     finished first"
                );
            }
        }
        StartOutcome::Stored(goal) => {
            if json {
                print_json(&serde_json::json!({
                    "run_id": identity,
                    "objective": goal.objective,
                    "max": goal.max,
                }));
            } else {
                say!("stored the Estigia loop for {identity}: {}", goal.objective);
                if let Some(max) = goal.max {
                    say!("completion cap: {max}");
                }
            }
        }
    }
    Ok(())
}

/// What `estigia goal start` did with the text it was given.
enum StartOutcome {
    /// The objective was stored as this run's loop.
    Stored(harness::session::LoopGoal),
    /// `stop` was given, so the current loop is asked to end.
    StopRequested,
}

/// Stores a loop for `identity`, or records `stop`, against a context a test can
/// build.
///
/// Starting a loop clears any stop-after request left by a previous one: that
/// request was made against work now over, and honouring it would end the new
/// loop before it opened anything. The reference host clears it at start for the
/// same reason.
fn start_loop(
    context: &harness::GateContext,
    identity: &str,
    text: &str,
) -> Result<StartOutcome, Refusal> {
    if text.trim() == "stop" {
        harness::session::set_stop_after(
            &context.state_root,
            identity,
            Some("the operator stopped the loop"),
        )
        .map_err(|error| {
            Refusal::not_started(
                "stop-after-not-writable",
                format!(
                    "the stop-after request could not be written: {}: {error}",
                    harness::session::stop_after_path(&context.state_root, identity).display()
                ),
                Resolution::no_command(
                    crate::outcome::NoCommandReason::WorldAction,
                    "write permission on the Estigia state directory",
                ),
            )
        })?;
        return Ok(StartOutcome::StopRequested);
    }
    let (rest, max) = crate::harness::loop_text::without_cap(text);
    let objective =
        crate::harness::loop_text::objective(crate::harness::loop_text::Invocation::Slash, &rest);
    let goal = harness::session::LoopGoal {
        objective,
        max,
        stopped: None,
        last: None,
    };
    harness::session::set_loop_goal(&context.state_root, identity, &goal).map_err(|error| {
        Refusal::not_started(
            "loop-not-writable",
            format!(
                "the loop record could not be written: {}: {error}",
                harness::session::loop_path(&context.state_root, identity).display()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::WorldAction,
                "write permission on the Estigia state directory",
            ),
        )
    })?;
    harness::session::clear_stop_after(&context.state_root, identity);
    Ok(StartOutcome::Stored(goal))
}

/// The line a person reads from `estigia goal`: [`goal_line`], except where
/// the repository is `assisted` and the queues were never read.
///
/// The zeros in the JSON stay, because a loop plugin stops on them. A person
/// reading them as a measurement of an empty queue would be misled.
pub(crate) fn goal_human_line(
    context: &harness::GateContext,
    run_id: &str,
    reading: GoalReading,
) -> String {
    if context.workflow != Some(crate::config::Workflow::Assisted) {
        return goal_line(run_id, &reading);
    }
    let unmeasured = GoalReading {
        claimable: None,
        dischargeable: None,
        held: None,
        pending: None,
        unconditioned: None,
        unadmitted: None,
        ..reading
    };
    format!(
        "{} \u{2014} the queues were not read, because this repository's `Workflow` is \
         `assisted`: Estigia claims nothing in it, and no work is counted as claimable",
        goal_line(run_id, &unmeasured)
    )
}

/// Resolve the one run a `goal` call is about, or refuse naming the verb.
///
/// `command` is the full verb the refusal belongs to — `estigia goal`,
/// `estigia goal progress` — so a refusal never sends the caller to a different
/// road than the one they took.
fn goal_identity(
    command: &str,
    run_id: Option<&str>,
    session_id: Option<&str>,
    runtime: Option<&str>,
) -> Result<String, Refusal> {
    let ambient = std::env::var("ESTIGIA_RUN_ID").ok();
    let identity = gate_identity(command, run_id, session_id, runtime, ambient.as_deref())?;
    identity.ok_or_else(|| {
        Refusal::not_started(
            "goal-identity-missing",
            "no run was named: a goal is asked about one run, and neither --run-id nor \
             --session-id was given",
            Resolution::run(format!(
                "{command} --session-id <session-id> --runtime <runtime>"
            )),
        )
    })
}

/// `estigia goal progress` and `estigia goal blocked`: record activity that
/// renews the run's claim.
///
/// A write the gate sees, so the run pointer has to name this run's issue and
/// the transport's `heartbeat` renews before it posts. `kind` is one of
/// [`crate::transport::markers::RENEWAL_KINDS`]; the text is staged exactly as
/// [`claim`] stages its body.
fn goal_activity(
    kind: &str,
    text: &str,
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let placeholder = if kind == "progress" {
        "<note>"
    } else {
        "<reason>"
    };
    let command = format!("estigia goal {kind} {placeholder}");
    let identity = goal_identity(&command, run_id, session_id, runtime)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = gate_context(&cwd.to_string_lossy())?;
    let run = harness::session::load(&context.state_root, &identity);
    // Unreadable is not absent: a pointer that is there and will not parse says
    // a run existed and not what it swore.
    if run.unreadable {
        let pointer = harness::session::pointer_path(&context.state_root, &identity);
        return Err(Refusal::not_started(
            "run-pointer-unreadable",
            format!(
                "{identity} wrote a pointer Estigia can no longer read, so what it holds is \
                 unknown: {}",
                pointer.display()
            ),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what that run holds, read from the tracker \u{2014} then that file removed, \
                 which is Estigia's own state and no claim of anybody's",
            ),
        ));
    }
    let Some(issue) = run.issue else {
        return Err(Refusal::not_started(
            "nothing-held",
            format!("{identity} holds no issue"),
            Resolution::run("estigia status"),
        ));
    };
    let state = run.state.unwrap_or_else(|| "in-progress".to_owned());
    let staged = crate::paths::scratch_file(&format!("goal-{kind}-{issue}.md"));
    crate::paths::replace_atomically(&staged, text).map_err(|error| {
        Refusal::not_started(
            "goal-activity-body-unwritable",
            format!("the {kind} text could not be staged: {error}"),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "write access to the temporary directory the note is staged in",
            ),
        )
    })?;
    let path = staged.display().to_string();
    let answer = tool(
        "heartbeat",
        &serde_json::json!({
            "issue": issue,
            "run_id": identity,
            "expect_state": state,
            "body_file": path,
            "kind": kind,
        }),
    );
    let _ = std::fs::remove_file(&staged);
    let answer = answer?;
    if json {
        print_json(&serde_json::json!({
            "run_id": identity,
            "issue": issue,
            "kind": kind,
            "recorded": true,
            "transport": serde_json::from_str::<serde_json::Value>(&answer)
                .unwrap_or(serde_json::Value::String(answer.clone())),
        }));
        return Ok(());
    }
    say!("{}", indented_for_a_person(answer));
    say!("recorded {kind} on #{issue} for {identity}");
    Ok(())
}

/// `estigia goal stop-after`: record, or take back, this run's request that its
/// loop end once the claim is released.
///
/// A file of its own beside `runs/` under `~/.estigia/` rather than a field on
/// the run pointer, because `release` forgets the pointer and the loop reads the request
/// only after that. The request outlives the claim it was made against, and is
/// consumed when the loop stops on it.
fn goal_stop_after(
    reason: Option<&str>,
    clear: bool,
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let identity = goal_identity("estigia goal stop-after", run_id, session_id, runtime)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = gate_context(&cwd.to_string_lossy())?;
    // The request is keyed by run id alone, so neither branch reads the pointer;
    // an unreadable pointer must not block the clear the loop needs.
    if clear {
        harness::session::clear_stop_after(&context.state_root, &identity);
    } else {
        harness::session::set_stop_after(&context.state_root, &identity, reason).map_err(
            |error| {
                Refusal::not_started(
                    "stop-after-not-writable",
                    format!(
                        "the stop-after request could not be written: {}: {error}",
                        harness::session::stop_after_path(&context.state_root, &identity).display()
                    ),
                    Resolution::no_command(
                        crate::outcome::NoCommandReason::WorldAction,
                        "write permission on the Estigia state directory",
                    ),
                )
            },
        )?;
    }
    let recorded = if clear {
        None
    } else {
        harness::session::stop_after(&context.state_root, &identity)
    };
    if json {
        print_json(&serde_json::json!({
            "run_id": identity,
            "stop_after": recorded,
            "cleared": clear,
        }));
        return Ok(());
    }
    if clear {
        say!("cleared the stop-after request for {identity}");
    } else {
        say!(
            "recorded a stop-after request for {identity}: {}",
            recorded.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

/// `estigia goal complete`: whether delivery evidence proves the goal complete.
///
/// Read-only, and exit 0 for a read answer like the bare verb. It re-reads the
/// tracker and, for an open issue, the publication lane at call time; nothing
/// here proves the work itself or that any claim was honest.
fn goal_complete(
    session_id: Option<&str>,
    runtime: Option<&str>,
    run_id: Option<&str>,
    json: bool,
) -> Result<(), Refusal> {
    let identity = goal_identity("estigia goal complete", run_id, session_id, runtime)?;
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let context = gate_context(&cwd.to_string_lossy())?;
    let run = harness::session::load(&context.state_root, &identity);
    let reading = harness::goal_completion_reading(
        &context.skill_root,
        &context.repo_dir,
        &context.tracker,
        &run,
    );
    if json {
        print_json(&goal_completion_json(&identity, &reading));
    } else {
        say!("{}", goal_completion_line(&identity, &reading));
    }
    Ok(())
}

/// The one JSON object `estigia goal complete --json` prints.
pub(crate) fn goal_completion_json(
    run_id: &str,
    reading: &harness::GoalCompletion,
) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "run_id".to_owned(),
        serde_json::Value::String(run_id.to_owned()),
    );
    object.insert(
        "issue".to_owned(),
        reading
            .issue
            .as_ref()
            .and_then(|issue| serde_json::to_value(issue).ok())
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "complete".to_owned(),
        serde_json::Value::Bool(reading.complete),
    );
    object.insert(
        "standing".to_owned(),
        serde_json::Value::String(reading.standing.to_owned()),
    );
    // Only the keys that are there, the way `goal_json` omits `reason`.
    for (key, value) in [("reason", &reading.reason), ("head", &reading.head)] {
        if let Some(value) = value {
            object.insert(key.to_owned(), serde_json::Value::String(value.clone()));
        }
    }
    serde_json::Value::Object(object)
}

/// The one human line `estigia goal complete` prints, and nothing else.
pub(crate) fn goal_completion_line(run_id: &str, reading: &harness::GoalCompletion) -> String {
    if reading.standing == "unreadable" || reading.standing == "unread" {
        return format!("estigia: {run_id}'s claim could not be read");
    }
    let Some(issue) = reading.issue.clone() else {
        return format!("estigia: {run_id} holds no issue");
    };
    if reading.complete {
        return format!(
            "estigia: {run_id} holds #{issue} \u{2014} goal complete ({})",
            reading.standing,
        );
    }
    format!(
        "estigia: {run_id} holds #{issue} \u{2014} goal not complete: {}",
        reading
            .reason
            .as_deref()
            .unwrap_or("the delivery evidence is not complete"),
    )
}

/// The reading behind [`goal_status`], split out so a test can hand it a context
/// the way the hook's own tests do.
pub(crate) fn goal_reading(
    context: &harness::GateContext,
    run_id: &str,
    with_completed: bool,
) -> GoalReading {
    let completed = if with_completed {
        harness::completed_work(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            run_id,
        )
    } else {
        None
    };
    let run = harness::session::load(&context.state_root, run_id);
    let stop_after = harness::session::stop_after(&context.state_root, run_id);
    // A pointer that is there and will not parse says a run existed and not
    // what it swore: that is not a run holding nothing.
    if run.unreadable {
        return GoalReading {
            issue: None,
            state: None,
            standing: "unreadable",
            live: false,
            reason: None,
            claimable: None,
            dischargeable: None,
            held: None,
            pending: None,
            unconditioned: None,
            unadmitted: None,
            completed,
            stop_after,
        };
    }
    if !pointer_is_for_this_repository(&run, &context.repo_dir) {
        return GoalReading {
            issue: run.issue,
            state: run.state,
            standing: "unreadable",
            live: false,
            reason: None,
            claimable: None,
            dischargeable: None,
            held: None,
            pending: None,
            unconditioned: None,
            unadmitted: None,
            completed,
            stop_after,
        };
    }
    let state = run.state.clone();
    let Some(issue) = run.issue.clone() else {
        let queues = queue_reading(context, &run.run_id);
        return GoalReading {
            issue: None,
            state,
            standing: "nothing-sworn",
            live: false,
            reason: None,
            claimable: queues.claimable,
            dischargeable: queues.dischargeable,
            held: queues.held,
            pending: queues.pending,
            unconditioned: queues.unconditioned,
            unadmitted: queues.unadmitted,
            completed,
            stop_after,
        };
    };
    match harness::tracker_answer_for_pointer(
        &context.skill_root,
        &context.repo_dir,
        &context.tracker,
        &run,
    ) {
        // No transport, no readable clock, no issue: nothing was asked, which
        // is not the same as an answer that says no.
        None => {
            let queues = queue_reading(context, &run.run_id);
            GoalReading {
                issue: Some(issue),
                state,
                standing: "unreadable",
                live: false,
                reason: None,
                claimable: queues.claimable,
                dischargeable: queues.dischargeable,
                held: queues.held,
                pending: queues.pending,
                unconditioned: queues.unconditioned,
                unadmitted: queues.unadmitted,
                completed,
                stop_after,
            }
        }
        Some(answer) => match answer.standing() {
            harness::tracker::Standing::Passed => GoalReading {
                issue: Some(issue),
                state,
                standing: "passed",
                live: true,
                reason: None,
                claimable: None,
                dischargeable: None,
                held: None,
                pending: None,
                unconditioned: None,
                unadmitted: None,
                completed,
                stop_after,
            },
            harness::tracker::Standing::Stopped(reason) => {
                let queues = queue_reading(context, &run.run_id);
                GoalReading {
                    issue: Some(issue),
                    state,
                    standing: "stopped",
                    live: false,
                    reason: Some(reason.to_owned()),
                    claimable: queues.claimable,
                    dischargeable: queues.dischargeable,
                    held: queues.held,
                    pending: queues.pending,
                    unconditioned: queues.unconditioned,
                    unadmitted: queues.unadmitted,
                    completed,
                    stop_after,
                }
            }
            harness::tracker::Standing::Unread => {
                let queues = queue_reading(context, &run.run_id);
                GoalReading {
                    issue: Some(issue),
                    state,
                    standing: "unread",
                    live: false,
                    reason: None,
                    claimable: queues.claimable,
                    dischargeable: queues.dischargeable,
                    held: queues.held,
                    pending: queues.pending,
                    unconditioned: queues.unconditioned,
                    unadmitted: queues.unadmitted,
                    completed,
                    stop_after,
                }
            }
        },
    }
}

/// Whether a run's pointer may be answered from `repo_dir`: a pointer that
/// recorded no repository is answered as it always was, one that did is only
/// answered from a checkout of that repository.
fn pointer_is_for_this_repository(run: &harness::session::Run, repo_dir: &std::path::Path) -> bool {
    run.covered().next().is_none()
        || run.covered().any(|covered| {
            crate::paths::covers(covered, repo_dir)
                || harness::same_git_repository(covered, repo_dir)
        })
}

/// What the tracker's queues hold for one run, each count `None` when the read
/// behind it did not land.
struct Queues {
    claimable: Option<u64>,
    dischargeable: Option<u64>,
    held: Option<u64>,
    pending: Option<u64>,
    unconditioned: Option<u64>,
    unadmitted: Option<u64>,
}

/// Every count `estigia goal` reports about the queues, from one read of each
/// queue: `review`, `ready`, `blocked` and `in-progress`.
///
/// `None` for a count whose queue could not be read, never zero — a queue nobody
/// could read is not a queue with nothing in it. `claimable` is the `review` and
/// `ready` queues summed, and `held` is every open issue in `in-progress` or
/// `review` that is not one this run could claim: the run holds nothing when
/// these are read, so an item in either state is somebody else's, or a claim
/// gone stale that nobody has reclaimed.
fn queue_reading(context: &harness::GateContext, run_id: &str) -> Queues {
    if context.workflow == Some(crate::config::Workflow::Assisted) {
        return Queues {
            claimable: Some(0),
            dischargeable: Some(0),
            held: Some(0),
            pending: Some(0),
            unconditioned: Some(0),
            unadmitted: Some(0),
        };
    }
    let read = |state: &str| {
        harness::queue_read(
            &context.skill_root,
            &context.repo_dir,
            &context.tracker,
            run_id,
            state,
        )
    };
    let number = |answer: &Option<serde_json::Value>, key: &str| {
        answer
            .as_ref()
            .and_then(|answer| answer.get(key))
            .and_then(serde_json::Value::as_u64)
    };
    let review = read("review");
    let ready = read("ready");
    let blocked = read("blocked");
    let in_progress = read("in-progress");
    let claimable = number(&review, "count")
        .zip(number(&ready, "count"))
        .map(|(review, ready)| review + ready);
    let held = number(&review, "open")
        .zip(number(&review, "count"))
        .zip(number(&in_progress, "open"))
        .map(|((open, claimable), in_progress)| in_progress + open.saturating_sub(claimable));
    Queues {
        claimable,
        dischargeable: number(&blocked, "dischargeable"),
        held,
        pending: number(&blocked, "pending"),
        unconditioned: number(&blocked, "unconditioned"),
        unadmitted: ready.as_ref().map(|answer| {
            answer
                .get("unadmitted")
                .and_then(serde_json::Value::as_array)
                .map_or(0, |list| list.len() as u64)
        }),
    }
}

/// The one JSON object `estigia goal --json` prints.
pub(crate) fn goal_json(run_id: &str, reading: &GoalReading) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "run_id".to_owned(),
        serde_json::Value::String(run_id.to_owned()),
    );
    object.insert(
        "issue".to_owned(),
        reading
            .issue
            .as_ref()
            .and_then(|issue| serde_json::to_value(issue).ok())
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "state".to_owned(),
        reading
            .state
            .clone()
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    object.insert(
        "standing".to_owned(),
        serde_json::Value::String(reading.standing.to_owned()),
    );
    object.insert("live".to_owned(), serde_json::Value::Bool(reading.live));
    object.insert(
        "claimable".to_owned(),
        reading
            .claimable
            .map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    object.insert(
        "dischargeable".to_owned(),
        reading
            .dischargeable
            .map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    for (key, count) in [
        ("held", reading.held),
        ("pending", reading.pending),
        ("unconditioned", reading.unconditioned),
        ("unadmitted", reading.unadmitted),
    ] {
        object.insert(
            key.to_owned(),
            count.map_or(serde_json::Value::Null, serde_json::Value::from),
        );
    }
    object.insert(
        "completed".to_owned(),
        reading
            .completed
            .map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    object.insert(
        "stop_after".to_owned(),
        reading
            .stop_after
            .clone()
            .map_or(serde_json::Value::Null, serde_json::Value::String),
    );
    // Only when stopped, so the key's presence says the tracker named a reason.
    if let Some(reason) = &reading.reason {
        object.insert(
            "reason".to_owned(),
            serde_json::Value::String(reason.clone()),
        );
    }
    serde_json::Value::Object(object)
}

/// The one human line `estigia goal` prints, and nothing else.
pub(crate) fn goal_line(run_id: &str, reading: &GoalReading) -> String {
    let claimable = match reading.claimable {
        Some(count) => format!(" ({count} claimable)"),
        None => String::new(),
    };
    let dischargeable = match reading.dischargeable {
        Some(count) => format!(" ({count} dischargeable)"),
        None => String::new(),
    };
    let waiting = [
        ("held", reading.held),
        ("pending", reading.pending),
        ("unconditioned", reading.unconditioned),
        ("unadmitted", reading.unadmitted),
    ]
    .into_iter()
    .filter_map(|(name, count)| count.map(|count| format!(" ({count} {name})")))
    .collect::<String>();
    let dischargeable = format!("{dischargeable}{waiting}");
    let completed = match reading.completed {
        Some(count) => format!(" ({count} completed)"),
        None => String::new(),
    };
    let line = match reading.standing {
        "passed" => {
            let state = reading
                .state
                .as_deref()
                .map_or_else(String::new, |state| format!(" ({state})"));
            format!(
                "estigia: {run_id} holds #{} live{state}{completed}",
                reading
                    .issue
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default()
            )
        }
        "stopped" => format!(
            "estigia: {run_id} no longer holds #{} \u{2014} {}{claimable}{dischargeable}{completed}",
            reading
                .issue
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            reading
                .reason
                .as_deref()
                .unwrap_or("the tracker stopped it"),
        ),
        "unread" => {
            format!(
                "estigia: {run_id}'s claim could not be read from the tracker{claimable}\
                 {dischargeable}{completed}"
            )
        }
        "unreadable" => format!("estigia: {run_id}'s claim could not be read{completed}"),
        _ => format!("estigia: {run_id} holds no issue{claimable}{dischargeable}{completed}"),
    };
    match &reading.stop_after {
        Some(reason) => format!("{line} \u{2014} a stop-after request is pending: {reason}"),
        None => line,
    }
}

thread_local! {
    static GATE_ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

const ASKS_THROUGH: &str = "pi";

fn asks_through(can_ask: bool, runtime: Option<&str>) -> bool {
    can_ask && runtime == Some(ASKS_THROUGH)
}

fn gate_answer(decision: harness::Decision, asks: bool) -> harness::Decision {
    if asks {
        decision
    } else {
        decision.without_ask()
    }
}

fn gate_exit_code(refusal: &Refusal) -> ExitCode {
    if GATE_ASKED.with(|asked| asked.replace(false)) {
        ExitCode::Asked
    } else {
        exit_code_for(refusal)
    }
}

const GATE_INPUT_LIMIT: u64 = 32 * 1024 * 1024;

fn gate_input(input: &str) -> Result<String, Refusal> {
    use std::io::Read as _;
    if input != "-" {
        return Ok(input.to_owned());
    }
    let refused = |what: String| {
        Refusal::not_started(
            "gate-input-not-json",
            what,
            Resolution::run(r#"estigia gate Edit --run-id <run-id> --input -"#),
        )
    };
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(GATE_INPUT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            refused(format!(
                "--input - could not be read from standard input: {error}"
            ))
        })?;
    if bytes.len() as u64 > GATE_INPUT_LIMIT {
        return Err(refused(format!(
            "--input - carried more than {GATE_INPUT_LIMIT} bytes on standard input"
        )));
    }
    String::from_utf8(bytes).map_err(|error| {
        refused(format!(
            "--input is not JSON: standard input is not UTF-8: {error}"
        ))
    })
}

fn show_gate(
    tool: &str,
    input: &str,
    run_id: Option<&str>,
    json: bool,
    asks: bool,
) -> Result<(), Refusal> {
    let parsed: serde_json::Value = serde_json::from_str(input).map_err(|error| {
        Refusal::not_started(
            "gate-input-not-json",
            format!("--input is not JSON: {error}"),
            Resolution::run(
                r#"estigia gate Edit --run-id <run-id> --input '{"file_path":"src/x.rs"}'"#,
            ),
        )
    })?;
    // The checkout the payload names, and the process's own only when it names
    // none. The hook lifts `cwd` out of a payload that nests it and this door
    // discarded it entirely, so the same call measured against two different
    // repositories depending on which one it came through.
    // Two keys, two levels of trust: a covered cwd is taken as given; an uncovered
    // one is dropped. Only when the host named nothing does the call's own workdir
    // get a say, and then only to narrow to one of those two — see
    // `narrowed_by_the_call`.
    let (context, checkout_observed) = context_the_payload_names(&parsed, tool, run_id)?;
    let mut observed = crate::outcome::Observed {
        command: None,
        checkout: Some(checkout_observed),
        push_refs: Vec::new(),
    };
    let (action, how, command_observed) =
        harness::classify_observing(tool, &parsed, &context.boundaries);
    observed.command = Some(command_observed);
    // Before the role road, which decides a call carrying an `agent_type` and would
    // otherwise answer it, through a stand-down, without this rule asked.
    if let Some(denied) = harness::person_only::deny(&action) {
        if let Some(run_id) = run_id.filter(|run_id| !run_id.is_empty()) {
            harness::hook::note(
                &context,
                run_id,
                tool,
                action.subject().as_deref(),
                &denied,
                &[],
            );
        }
        return report_gate(tool, json, denied, &observed);
    }
    // Before anything else, and the least conditional: no claim, no state, no window.
    if let Some(agent) = payload_agent(&parsed) {
        if let Some((seal, template)) = harness::roles::reserved_reviewer_row(agent) {
            // Rendered with the effective row: parsing the `{{TOOLS}}` placeholder
            // would enforce an allowlist naming one tool called `{{TOOLS}}`.
            let reviewer =
                crate::setup::render_reviewer_agent(template, context.evidence, seal, agent);
            if let Some(refusal) = harness::roles::gate(Some(agent), tool, Some(&reviewer)) {
                return report_gate(
                    tool,
                    json,
                    harness::Decision::Deny(Box::new(refusal)),
                    &observed,
                );
            }
        } else {
            match harness::roles::definition_for(
                &context.repo_dir,
                crate::paths::home_dir().ok().as_deref(),
                agent,
            ) {
                // Denied rather than stepped over, and through the stand-down like the
                // hook's own role denial.
                Err(refusal) => {
                    return report_gate(
                        tool,
                        json,
                        harness::standdown::over(
                            harness::Decision::Deny(Box::new(refusal)),
                            context.stand_down.as_ref(),
                            harness::session::now_seconds(),
                        ),
                        &observed,
                    );
                }
                Ok(Some(definition)) => {
                    if let Some(refusal) =
                        harness::roles::gate(Some(agent), tool, Some(&definition))
                    {
                        return report_gate(
                            tool,
                            json,
                            harness::standdown::over(
                                harness::Decision::Deny(Box::new(refusal)),
                                context.stand_down.as_ref(),
                                harness::session::now_seconds(),
                            ),
                            &observed,
                        );
                    }
                }
                Ok(None) => {}
            }
        }
    }
    let (decision, recorded) = match run_id {
        Some(run_id) => {
            let mut run = harness::session::load(&context.state_root, run_id);
            let decided = harness::gate_observing(&context, &mut run, &action, how, &observed);
            if matches!(decided, harness::Decision::Allow(_)) {
                // What the hook does on the same branch: a decision never written down
                // is a window that never applies. Best effort.
                let _ = harness::session::store(&context.state_root, &run);
            }
            (decided, run_id.to_owned())
        }
        // No session to ask with, so ask the checkout instead.
        None => {
            let literal = match observed.command.as_ref().map(|command| &command.literal) {
                Some(crate::outcome::Established::Known(text)) => Some(text.as_str()),
                _ => None,
            };
            let same_tier_fragments = observed
                .command
                .as_ref()
                .map(|command| command.same_tier_fragments.as_slice())
                .unwrap_or_default();
            let adjudication = harness::guard::adjudicate_command(
                &context,
                &context.repo_dir,
                &action,
                how,
                &[],
                literal,
                same_tier_fragments,
            );
            (
                adjudication.decision,
                adjudication.holder.unwrap_or_default(),
            )
        }
    };
    let decision = gate_answer(decision, asks);
    // Recorded: for one adapter this **is** the gate, and `doctor`'s silence check
    // reads the same ledger.
    if !recorded.is_empty() {
        let subject = action.subject();
        harness::hook::note(
            &context,
            &recorded,
            tool,
            subject.as_deref(),
            &decision,
            &[],
        );
    }
    report_gate(tool, json, decision, &observed)
}

/// One decision, reported the way this door reports every decision.
///
/// `observed` is attached to the `outside` and `allow` documents here, and to
/// a `Deny` that does not already carry one. Most do: `harness::gate_observing`'s
/// own `prefixed` sets it on every denial the ordinary claim road produces.
/// A role refusal (`harness::roles::gate`, `harness::roles::definition_for`)
/// never reaches that road at all, and neither does the several-holder or
/// unreadable-pointer refusal `guard::adjudicate_action` builds for a call
/// with no session to gate through — both had the checkout adjudicated and
/// the account built by this door before either produced a `Refusal`, and
/// both used to have it thrown away right here.
fn report_gate(
    tool: &str,
    json: bool,
    decision: harness::Decision,
    observed: &crate::outcome::Observed,
) -> Result<(), Refusal> {
    let asked = matches!(decision, harness::Decision::Ask(_));
    match decision {
        harness::Decision::Outside(aside) => {
            // The reason comes from the decision, not from this line.
            let why = aside.why(tool);
            if json {
                print_json(&serde_json::json!({
                    "decision": "outside",
                    "tool": tool,
                    "aside": aside.code(),
                    "reason": why,
                    "observed": observed,
                }));
            } else {
                say!("outside \u{2014} {why}");
            }
            Ok(())
        }
        harness::Decision::Allow(reason) => {
            if json {
                print_json(&serde_json::json!({
                    "decision": "allow",
                    "tool": tool,
                    "reason": reason,
                    "observed": observed,
                }));
            } else {
                say!("allow — {reason}");
            }
            Ok(())
        }
        harness::Decision::Deny(mut refusal) | harness::Decision::Ask(mut refusal) => {
            if asked {
                GATE_ASKED.with(|flag| flag.set(true));
            }
            // A role refusal, or one `guard::adjudicate_action` built for a
            // call with no session, never went through `prefixed` and so
            // never carries one. Everything that did already carries the
            // same value this door would attach — attaching it again would
            // only repeat it, never correct it, so it is left alone.
            if refusal.observed.is_none() && !observed.is_empty() {
                refusal.observed = Some(Box::new(observed.clone()));
            }
            Err(*refusal)
        }
    }
}

fn nothing_configured(notes: &[String]) -> Refusal {
    let mut message = String::from("no agent has Estigia installed");
    for note in notes {
        message.push_str("\n  note: ");
        message.push_str(note);
    }
    Refusal::not_started(
        "nothing-configured",
        message,
        Resolution::run("estigia setup --all"),
    )
}

/// A write that failed part-way is the one case where the outcome is unknown.
///
/// Setup writes several files; an error on the third says nothing about the
/// first two, so the outcome stays `Unknown`.
fn write_failed(adapter: &AgentAdapter, error: &anyhow::Error) -> Refusal {
    Refusal {
        code: "setup-write-failed",
        message: format!("{}: {error}", adapter.display_name),
        outcome: crate::outcome::MutationOutcome::Unknown,
        class: crate::harness::refusal_class::class_of_code("setup-write-failed"),
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "write access to the file named above \u{2014} it is read-only, held open, or on a \
             filesystem that refused; free it and run `estigia sync` again",
        ),
        observed: None,
    }
}

/// Where one agent stands, in the words `status` prints beside its name.
fn standing(
    configured: bool,
    presence: skill::Presence,
    shared: bool,
    touched: bool,
) -> &'static str {
    match (configured, presence) {
        (true, skill::Presence::Current) => "configured",
        (true, skill::Presence::Stale) => "configured, skill out of date",
        // Not "out of date": `sync` refuses this contract rather than replacing it.
        (true, skill::Presence::Unreadable) => "configured, contract not understood",
        (true, skill::Presence::Absent) => "configured, skill missing",
        (false, skill::Presence::Absent) => "not configured",
        // Untouched, reading somebody else's skill root: nothing here is this agent's.
        (false, _) if shared && !touched => "not configured",
        (false, _) => "skill present, directive missing",
    }
}

/// The word a row gets, or `None` for a file this run left alone.
fn word(change: Change, planned: bool) -> Option<&'static str> {
    if planned {
        // A plan says what it *would* do, in its own words.
        return Some(match change {
            Change::Create => "would create",
            Change::Update => "would update",
            Change::Replace => "would REPLACE",
            Change::Overwrite => "would OVERWRITE",
            Change::Remove => "would remove",
            Change::Kept => "would keep",
            Change::Shared => "shared",
            Change::Unrecorded => "unknown",
            Change::Unchanged => return None,
        });
    }
    Some(match change {
        Change::Create => "create",
        Change::Update => "update",
        Change::Replace => "REPLACE",
        Change::Overwrite => "OVERWRITE",
        Change::Remove => "remove",
        Change::Kept => "kept",
        Change::Shared => "shared",
        Change::Unrecorded => "unknown",
        Change::Unchanged => return None,
    })
}

/// A setup failure, classified before it is reported.
fn setup_failed(adapter: &AgentAdapter, error: &anyhow::Error) -> Refusal {
    // A refusal that already classified itself travels through with its own answers.
    if let Some(refusal) = error.downcast_ref::<Refusal>() {
        return Refusal {
            message: format!("{}: {}", adapter.display_name, refusal.message),
            ..refusal.clone()
        };
    }
    match error.downcast_ref::<setup::NotEditable>() {
        Some(unusable) => Refusal::not_started(
            "agent-file-not-editable",
            format!("{}: {unusable}", adapter.display_name),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "a JSON object in that file, or the file moved aside — Estigia will not guess \
                 what it was meant to hold",
            ),
        ),
        None => write_failed(adapter, error),
    }
}

fn setup_failure_refusal(adapter: &AgentAdapter, failure: &setup::SetupFailure) -> Refusal {
    let before_mutation = failure.phase != setup::SetupFailurePhase::Mutation
        || (!failure.write_attempted && failure.result.changed_files() == 0);
    let mut refusal = if failure.error.downcast_ref::<Refusal>().is_some()
        || failure.error.downcast_ref::<setup::NotEditable>().is_some()
    {
        setup_failed(adapter, &failure.error)
    } else if before_mutation {
        Refusal::not_started(
            "setup-prevalidation-failed",
            format!("{}: {}", adapter.display_name, failure.error),
            Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "the input path or configuration named above readable and valid before retrying",
            ),
        )
    } else {
        setup_failed(adapter, &failure.error)
    };
    if before_mutation {
        refusal.outcome = crate::outcome::MutationOutcome::NotStarted;
        refusal.replay = crate::outcome::Replayability::ExactReplaySafe;
    }
    refusal
}

fn model_advisories(adapter: &AgentAdapter, config: &Config) -> Vec<String> {
    adapter
        .foreign_models_in(&config.models)
        .into_iter()
        .map(|(model, hosts)| {
            format!(
                "{model} is in the model list of {}, which {} does not list; it is stored as given",
                hosts.join(" and "),
                adapter.display_name
            )
        })
        .collect()
}

fn advisories_of(setting: Setting, agent: Option<&str>, config: &Config) -> Vec<String> {
    if setting != Setting::Models {
        return Vec::new();
    }
    agent
        .and_then(|slug| setup::find_installable_agent(slug).ok())
        .map(|adapter| model_advisories(adapter, config))
        .unwrap_or_default()
}

fn say_model_advisories(setting: Setting, agent: Option<&str>, config: &Config) {
    for advisory in advisories_of(setting, agent, config) {
        say!("  note: {advisory}");
    }
}

fn set_answer(label: &str, held: &str, change: Change, file: &std::path::Path) -> String {
    match change {
        Change::Unchanged => format!("{label} was already {held} ({})", file.display()),
        _ => format!("{label} is now {held} ({})", file.display()),
    }
}

fn agents_that_read(setting: Setting, options: &SetupOptions) -> Vec<&'static str> {
    configured(options)
        .into_iter()
        .filter(|adapter| setting.applies_to_agent(adapter))
        .map(|adapter| adapter.slug)
        .collect()
}

fn agent_row_needs_an_agent(setting: Setting, options: &SetupOptions) -> Refusal {
    let slugs = agents_that_read(setting, options);
    let listed = if slugs.is_empty() {
        String::from("`--agent <slug>`")
    } else {
        format!("`--agent <slug>`, one of: {}", slugs.join(", "))
    };
    Refusal::not_started(
        "setting-needs-an-agent",
        format!(
            "{} is answered per agent, and a set without `--agent` would write it into a \
             contract several agents read; name the agent with {listed}",
            setting.label()
        ),
        Resolution::run("estigia config set <setting> <value> --agent <slug>"),
    )
}

fn contract_writable_config(slug: &str, options: &SetupOptions) -> Result<WritableConfig, Refusal> {
    let (target, config, report) = contract_of_reporting(Some(slug), options)?;
    Ok(WritableConfig {
        target,
        config,
        agent_snapshot: None,
        report,
    })
}

fn refuse_moved_agent_rows(
    installed: &Config,
    config: &Config,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    let moved: Vec<Setting> = crate::config::SETTINGS
        .iter()
        .copied()
        .filter(|setting| setting.scope() == crate::config::Scope::Agent)
        .filter(|setting| setting.value_of(config) != setting.value_of(installed))
        .collect();
    let Some(first) = moved.first().copied() else {
        return Ok(());
    };
    let labels: Vec<&str> = moved.iter().map(|setting| setting.label()).collect();
    let mut refusal = agent_row_needs_an_agent(first, options);
    refusal.message = format!(
        "{} {} answered per agent, and the shared editor would write {} into a contract several \
         agents read; edit them with `estigia config edit --agent <slug>`",
        labels.join(", "),
        if labels.len() == 1 { "is" } else { "are" },
        if labels.len() == 1 { "it" } else { "them" },
    );
    refusal.resolution = Resolution::run("estigia config edit --agent <slug>");
    Err(refusal)
}

#[cfg(test)]
mod tests;

/// One line of a `pre-push` hook's stdin: `<local ref> <local sha> <remote
/// ref> <remote sha>`.
struct PushedRef {
    /// The ref on this side — the one that names a branch, and therefore the
    /// worktree it is checked out in. The literal `(delete)` for a branch
    /// deletion, which names no branch.
    local: String,
    /// The ref on the far side: what the ledger has always recorded, because
    /// that is the delivery's destination.
    remote: String,
}

/// The refs a push is about to write, as git names them on the hook's input.
///
/// Read only when standard input is not a terminal; a hook is always given a pipe,
/// and a person running this by hand must not be blocked.
fn refs_being_pushed() -> Vec<PushedRef> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        return Vec::new();
    }
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    pushed_refs_of(&text)
}

fn pushed_refs_of(text: &str) -> Vec<PushedRef> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let local = fields.next()?;
            let _local_sha = fields.next()?;
            let remote = fields.next()?;
            Some(PushedRef {
                local: local.to_owned(),
                remote: remote.to_owned(),
            })
        })
        .collect()
}

/// Why an agent's tool server would not answer, when it would not.
///
/// `None` when the server is fine or when this build has no place to register
/// one — the reason for that last case is [`AgentAdapter::tools_gap`], which is
/// permanent where this is a missing step.
fn tools_fault(adapter: &'static AgentAdapter, options: &SetupOptions) -> Option<String> {
    let Some(named) = setup::tools_command(adapter, options) else {
        return adapter.supports_mcp().then(|| {
            "no Estigia tool server is registered here, and this build can register one".to_owned()
        });
    };
    if !named.is_file() {
        return Some(format!(
            "the tool server names {}, which is not there — every operation the agent asks for \
             fails",
            named.display()
        ));
    }
    (setup::tools_start_the_server(adapter, options) == Some(false)).then(|| {
        format!(
            "the tool server names {} and never says `mcp`, so the binary prints its usage and \
             nothing starts",
            named.display()
        )
    })
}

/// The stand-down in force, in the words `doctor` uses for it.
fn standing_down_line() -> Option<String> {
    let root = harness::session::state_root(None).ok()?;
    let now = harness::session::now_seconds();
    match harness::standdown::standing(&root) {
        harness::standdown::Standing::Away => None,
        harness::standdown::Standing::Unreadable(why) => Some(format!(
            "a stand-down file is here and cannot be read, so whether the gate is standing down \
             is unknown — it is not honoured while it cannot be timed: {why}"
        )),
        harness::standdown::Standing::Declared(record) => match now {
            Some(now) if record.covers(now) => Some(format!(
                "STANDING DOWN for another {} minute(s), declared by {}: {} — writes go through \
                 unadjudicated until it expires",
                record.remaining(now).div_ceil(60),
                record.declared_by,
                record.reason
            )),
            // A record that cannot be timed is worth a line; one that is over is the
            // ordinary state of a machine that used one last week.
            Some(_) => None,
            None => Some(
                "a stand-down is recorded and this machine's clock cannot be read, so whether it \
                 still applies is unknown — the gate does not honour one it cannot time"
                    .to_owned(),
            ),
        },
    }
}
