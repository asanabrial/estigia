//! Dispatch, and the only place a [`Refusal`] becomes an exit code.
//!
//! Every path out of here goes through `report`, so no command can invent its
//! own way of failing — which is how issue-flow ended up with 87 rejection
//! sites and not one of them naming a continuation.

use std::io::Write;
use std::process::ExitCode as ProcessExitCode;

use anyhow::Result;
use clap::Parser;

use crate::config::{Config, Setting};
use crate::harness;
use crate::lifecycle;
use crate::outcome::{ExitCode, Refusal, Resolution, exit_code_for};
use crate::setup::{
    self, AGENTS, AgentAdapter, COMPANIONS, Change, SetupOptions, SetupResult, find_agent,
};
use crate::skill;

// Crate-visible so the harness can ask **clap** which verbs exist rather than
// grep this file for them: a verb is spelled from its variant name, so
// `StandDown` is `stand-down` and no search of the source text finds it. See
// `a_gated_spelling_that_carries_a_flag_still_names_its_verb`.
pub(crate) mod args;

pub use args::{Cli, Command, ConfigAction};

/// Writes one line to standard output, and stops quietly when nobody is reading.
///
/// `println!` panics when the pipe closes, so `estigia status | head` ends in a
/// backtrace — a backtrace where a clean exit belonged, which is the thing this
/// project exists not to do. A reader that went away is not an error: it is the
/// most ordinary way a person uses a command that prints a list.
/// A note for whoever is watching, on standard error.
///
/// Not [`say!`], which writes the document on standard output. Under `--json`
/// that document has to stay parseable, so a note printed there could only be
/// suppressed — and one of them was: `sync` told a person that a skill root
/// with no contract had taken the defaults rather than inheriting an answer,
/// and told a script nothing at all. The fact is the same in both modes; only
/// the channel differs.
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

/// The same line, into a string rather than onto the terminal.
///
/// A report the screen shows in place and the shell keeps afterwards has to be
/// the *same* report, so it is written once and rendered twice. Building it as
/// text is what makes that possible.
macro_rules! line {
    ($text:expr, $($argument:tt)*) => {{
        use std::fmt::Write as _;
        // Writing into a `String` cannot fail; there is no reader to lose.
        let _ = writeln!($text, $($argument)*);
    }};
}

/// Parses the command line and runs it.
pub fn run() -> ProcessExitCode {
    // `try_parse`, not `parse`. clap's own exit code for a usage error is **2**,
    // and 2 is the one code this crate says must never mean anything else:
    // *the command was interrupted and the world may have changed*. So a
    // mistyped invocation told a caller to re-read the tracker before retrying,
    // about a command that never ran.
    //
    // The callers are not hypothetical. Every hook Estigia writes reads the
    // status and treats `1` and `2` as decisions — `guard::script` says so in
    // those words — so a hook file left from an older build, passing a flag this
    // one no longer takes, exits 2 and **blocks every push in the repository**
    // instead of reporting that the gate did not answer. That is the failure
    // that script exists to prevent, arriving through the argument parser.
    //
    // `Refused` was the answer here, on the grounds that nothing was written
    // and nothing was attempted. Both true, and neither is what `1` means to
    // the things that read it: every script this crate writes treats `1` and
    // `2` as **decisions** and propagates them. So the fix moved the failure
    // one code over and left it — a hook file from another build, passing a
    // flag this one no longer takes, went on blocking every push in the
    // repository, now with a clap usage message instead of a stop.
    //
    // Measured on a real `git push`: `error: unexpected argument
    // --from-a-newer-build found`, push refused. `Unreadable` puts it where
    // both readers already send anything that is not a decision — they say the
    // gate did not answer, and let the write through.
    //
    // Help and version are not failures at all — clap reports both as errors
    // and prints them to stdout, which `use_stderr` is how to tell apart.
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
            exit_code_for(&refusal).into()
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

fn dispatch(cli: &Cli) -> Result<(), Refusal> {
    match &cli.command {
        Command::RecordInstall => record_install(cli.json),

        Command::Setup {
            agent,
            all,
            dry_run,
            uninstall,
            skill_only,
            companion,
            interactive,
            allow_source_build,
        } => {
            if let Some(slug) = companion {
                return show_companion(slug);
            }
            let options = SetupOptions {
                dry_run: *dry_run,
                skip_directive: *skill_only,
                ..SetupOptions::default()
            };
            // Bare `estigia setup` opens the screen. Refusing with "name an
            // agent" was answering the question the screen exists to ask —
            // somebody who has just installed the binary does not yet know the
            // slugs, and telling them to go and find out is the thing a setup
            // command is for.
            // Bare `estigia setup` opens the screen — but only where there is
            // somebody to look at it. Without a terminal the screen would wait
            // for a key that is never coming, and a command that hangs forever
            // in a script is worse than one that refuses: the refusal at least
            // says what to run instead.
            let interactive_possible = std::io::IsTerminal::is_terminal(&std::io::stdin());
            if *interactive || (agent.is_none() && !*all && !*uninstall && interactive_possible) {
                if !interactive_possible {
                    return Err(setup::no_terminal());
                }
                return after_lifecycle_preflight(
                    lifecycle_preflight(
                        &options,
                        *allow_source_build,
                        "estigia setup --interactive",
                    ),
                    || guided(&options, cli.json),
                );
            }
            let targets = select(agent.as_deref(), *all)?;
            if *uninstall {
                take_out(&targets, &options, cli.json)
            } else {
                lifecycle_preflight(&options, *allow_source_build, "estigia setup")?;
                let shared = installed_configs(&targets, &options);
                report_disagreement(&shared, &targets, &options);
                run_setup_over(&targets, &shared, &options, cli.json)
            }
        }

        Command::Uninstall {
            agent,
            all,
            dry_run,
        } => {
            let options = SetupOptions {
                dry_run: *dry_run,
                ..SetupOptions::default()
            };
            let targets = select(agent.as_deref(), *all)?;
            take_out(&targets, &options, cli.json)
        }

        Command::Sync {
            agent,
            all: _,
            dry_run,
            allow_source_build,
        } => {
            let options = SetupOptions {
                dry_run: *dry_run,
                ..SetupOptions::default()
            };
            let targets = match agent {
                Some(slug) => vec![find_agent(slug)?],
                None => configured(&options),
            };
            if targets.is_empty() {
                return Err(nothing_configured());
            }
            lifecycle_preflight(&options, *allow_source_build, "estigia sync")?;
            // Sync keeps whatever is already written in the contract: the point
            // is to move the markdown forward without resetting the operator's
            // table to defaults. Per adapter, because `sync` runs over every
            // *configured* agent — the population most likely to disagree, and
            // the one where flattening them would be hardest to notice.
            let shared = installed_configs(&targets, &options);
            report_disagreement(&shared, &targets, &options);
            let mut pending = setup::Pending::new();
            run_setup_evidenced_over(&targets, cli.json, |adapter| {
                // Forward, not wider. `setup --skill-only` leaves the
                // instruction file alone on purpose, and a sync that wrote a
                // directive there would undo that choice on the operator's
                // behalf — quietly, in a command whose whole job is to move the
                // markdown forward without resetting anything.
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
            })
        }

        Command::Status => status(cli.json),

        Command::Update => update(cli.json),

        Command::Doctor => doctor(cli.json),

        Command::Mcp => mcp(),

        Command::Hook {
            event,
            dialect,
            agent,
        } => hook(event, dialect, agent.as_deref()),

        Command::Guard { uninstall, dry_run } => guard(*uninstall, *dry_run, cli.json),

        Command::Claim {
            issue,
            run_id,
            horizon,
            state,
        } => claim(issue, run_id, horizon, state, cli.json),

        Command::Release { run_id } => release(run_id, cli.json),

        Command::Gate {
            tool,
            input,
            run_id,
        } => show_gate(tool, input, run_id.as_deref(), cli.json),

        Command::StandDown {
            reason,
            minutes,
            lift,
        } => stand_down(reason, *minutes, *lift, cli.json),

        // The machine these three read is a parameter rather than something each
        // resolves for itself. It was the latter, and the cost was a test: the
        // install screen and `config set` were held to one answer through a
        // temporary home the screen honoured and `config set` ignored, so the
        // second door read the **developer's own profile**. It agreed on every
        // machine that already had a skill installed and nowhere else, which is
        // the shape of a green test that measures nothing.
        Command::Config { action } => match action {
            ConfigAction::List { agent } => {
                config_list(agent.as_deref(), &SetupOptions::default(), cli.json)
            }
            ConfigAction::Edit { agent } => config_edit(agent.as_deref(), &SetupOptions::default()),
            ConfigAction::Set {
                setting,
                value,
                agent,
                repo,
            } => config_set(
                setting,
                value,
                agent.as_deref(),
                *repo,
                &SetupOptions::default(),
                cli.json,
            ),
            ConfigAction::Repos => config_repos(cli.json),
            ConfigAction::Forget { repo } => config_forget(repo.as_deref(), cli.json),
        },
    }
}

/// Asks the questions, then applies the answers through the ordinary path.
///
/// The screen decides and this applies, so there is exactly one piece of code
/// that installs anything. It starts from the table already installed rather
/// than from the defaults: a second run that quietly undoes the first one's
/// answers is worse than no screen at all.
///
/// There is no question-and-answer wizard beside it, and that is deliberate:
/// two ways to answer the same questions is two things to keep in step, and the
/// one that walks somebody through fifteen prompts in a fixed order is the
/// worse of the two for what people do most often — change one row.
fn guided(options: &SetupOptions, json: bool) -> Result<(), Refusal> {
    // Each adapter's own table, not one taken from whichever answered first.
    // Flattening them is the defect `setup --all` already had and had fixed:
    // two agents configured differently on purpose come out agreeing, the
    // second overwritten by the first, with nothing said.
    let targets: Vec<&'static AgentAdapter> = AGENTS.iter().collect();
    let mut installed: std::collections::BTreeMap<&'static str, Config> =
        std::collections::BTreeMap::new();
    let mut configured: Vec<&str> = Vec::new();
    // The same tables twice: as the checkout answers with them, and with no
    // checkout's rows on them at all. The screen opens on the first — the
    // answers in force *here* rather than the ones the machine holds for every
    // checkout at once — and lays another repository's rows over the second,
    // because laying them over the first keeps this one's answer for every row
    // the other is silent about.
    let mut bare: std::collections::BTreeMap<&'static str, Config> =
        std::collections::BTreeMap::new();
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    for adapter in &targets {
        if setup::is_configured(adapter, options) {
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
        // Tolerated here and refused at the write, the way an unparsable table
        // already is: a checkout whose file cannot be read costs the operator
        // the rows it holds, and costing them the screen as well would leave
        // them nothing to fix it with. `install_planned` asks again.
        installed.insert(
            adapter.slug,
            layers
                .as_ref()
                .map(|layers| layers.effective().clone())
                .unwrap_or_else(|| own.clone()),
        );
        bare.insert(adapter.slug, own);
    }

    // The screen gets the verdict and the shell gets the lines. Neither is the
    // other: a box in the middle of a terminal answers *did it work*, and thirty
    // rows of paths answer *what changed* — put in one place they cost each
    // other, and the alternate buffer takes whatever is in it away on the way
    // out, so the rows have to be printed here to survive at all.
    let mut written: Option<String> = None;
    crate::tui::setup(&installed, &bare, &configured, &mut |plan| {
        let (receipt, report) = install_planned(plan, options, json)?;
        written = Some(report);
        Ok(receipt)
    })?;

    // Leaving without installing is a choice, not a failure.
    let Some(report) = written.take() else {
        say!("nothing installed");
        return Ok(());
    };
    say!("{report}");
    Ok(())
}

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
    for adapter in &chosen {
        let paths = setup::resolve_paths(adapter, options).map_err(|_| skill::no_skill_root())?;
        let repository = (!here.as_os_str().is_empty()).then_some(here.as_path());
        let layers =
            skill::config_layers_for_install(&paths.skill_root, Some(adapter.slug), repository)?;
        paths_by_slug.insert(adapter.slug, paths);
        layers_before.insert(adapter.slug, layers);
    }

    let changed = |slug: &'static str| {
        crate::config::SETTINGS
            .iter()
            .copied()
            .filter(|setting| {
                plan.opened.get(slug).is_none_or(|opened| {
                    setting.value_of(opened) != setting.value_of(&plan.rows[slug])
                })
            })
            .collect::<Vec<_>>()
    };
    let changed_by_slug: std::collections::BTreeMap<_, _> = chosen
        .iter()
        .map(|adapter| (adapter.slug, changed(adapter.slug)))
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
                        .filter(|setting| setting.scope() == crate::config::Scope::Everywhere)
                        .collect::<Vec<_>>(),
                )?;
            }
            Ok((adapter.slug, effective))
        })
        .collect::<Result<_, Refusal>>()?;

    let mut pending = setup::Pending::new();
    let mut done: Vec<(&'static AgentAdapter, SetupResult)> = Vec::new();
    let mut completed = std::collections::BTreeSet::new();
    let mut roots_written = std::collections::BTreeSet::new();
    for adapter in &chosen {
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
            Ok(result) => {
                completed.insert(adapter.slug);
                done.push((*adapter, result));
            }
            Err(failure) => {
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
        repository_settings.extend(
            changed_by_slug[adapter.slug]
                .iter()
                .copied()
                .filter(|setting| setting.scope() == crate::config::Scope::Everywhere),
        );
        repository_settings.sort_by_key(|setting| {
            crate::config::EVERYWHERE_SETTINGS
                .iter()
                .position(|candidate| candidate == setting)
                .unwrap_or(usize::MAX)
        });
        repository_settings.dedup();
    }
    let repository_action = if !here.as_os_str().is_empty()
        && let Some((path, _)) = skill::repository_rows(here)?
        && let Some((adapter, result)) = done.first_mut()
        && let Some(any) = plan.rows.get(adapter.slug)
    {
        if let Some(refusal) = injected_install_failure(InstallFailurePoint::AtRepository) {
            repository_failure = Some(refusal);
            false
        } else {
            match setup::write_repository_configuration_into(
                &path,
                any,
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

    // Counted from what the run reported rather than from what it was asked
    // for: "installed into 11 agents" beside a run that wrote nothing is the
    // kind of confident wrong answer this whole tool exists to refuse.
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
        chosen.len(),
        if chosen.len() == 1 { "" } else { "s" },
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
            chosen.iter().map(|adapter| adapter.slug),
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
    // Once, and without the flag: a repository-wide row is a fact about the
    // repository, and `config set --agent` refuses it as such. The same
    // list printed on the screen, so both ways out say the same thing.
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
                        replay: crate::outcome::Replayability::StatusRequired,
                        resolution: Resolution::no_command(
                            crate::outcome::NoCommandReason::OperatorKnowledge,
                            "read the repository configuration path named above before deciding whether to retry",
                        ),
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
fn select(agent: Option<&str>, all: bool) -> Result<Vec<&'static AgentAdapter>, Refusal> {
    match (agent, all) {
        (Some(slug), _) => Ok(vec![find_agent(slug)?]),
        (None, true) => Ok(AGENTS.iter().collect()),
        (None, false) => Err(setup::no_agent_named()),
    }
}

/// The adapters Estigia is installed in, by any of the three ways it can be.
///
/// Not "carries the directive", which is what this asked and is a narrower
/// question wearing this one's name. `setup --skill-only` writes the skill, the
/// gate and the MCP server and leaves the instruction file alone, and every
/// caller here then reported `no agent has Estigia installed` about a machine
/// holding an installed contract — and named `estigia setup --all`, the one
/// command that writes the file the flag exists to leave alone.
fn configured(options: &SetupOptions) -> Vec<&'static AgentAdapter> {
    AGENTS
        .iter()
        .filter(|adapter| setup::is_present(adapter, options))
        .collect()
}

/// The distinct configurations already written under the targets' skill roots.
///
/// De-duplicated, because most adapters share the neutral root and reading one
/// file through two adapters is not two answers. A root with no contract
/// contributes nothing: it has no answer yet.
fn installed_configs(targets: &[&'static AgentAdapter], options: &SetupOptions) -> Vec<Config> {
    let mut found: Vec<Config> = Vec::new();
    for adapter in targets {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        let Ok(config) = skill::installed_config_for(&paths.skill_root, Some(adapter.slug)) else {
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
/// A root with no contract has no answer of its own, so it inherits — but only
/// when there is a single answer to inherit. With two in play, picking one is
/// exactly the guess that caused the defect, so it gets the portable defaults
/// and [`report_disagreement`] says so out loud.
/// The contract already installed for this adapter, when there is one to keep.
///
/// Three answers where the code had two. `Ok(Some)` is an answer to preserve.
/// `Ok(None)` is no contract yet, which inherits. `Err` is a contract that is
/// *there* and will not parse — not an absence, and the one case nothing may
/// write over: the rows exist and cannot be read, so nothing can say what
/// writing would cost.
///
/// Named once because two write paths ask it. `sync` and `setup` go through
/// [`config_for`]; the screen reads every adapter's table to show it, and read
/// the unreadable ones as defaults — so it displayed `merge commit` to an
/// operator whose file said `squash`, and wrote back what it had shown.
fn existing_config(
    adapter: &'static AgentAdapter,
    options: &SetupOptions,
) -> Result<Option<Config>, Refusal> {
    let Ok(paths) = setup::resolve_paths(adapter, options) else {
        return Ok(None);
    };
    match skill::installed_config_for(&paths.skill_root, Some(adapter.slug)) {
        // Read layered, returned unlayered, and both halves are load-bearing.
        //
        // The read has to layer, because a bad row in `estigia.local.md` must
        // still refuse here rather than be written over — that is what the
        // comment below is about.
        //
        // What is *written* must not. This value renders the versioned block,
        // and an override exists to change behaviour without changing that
        // block: returning the layered one promoted a machine-local choice into
        // the file that is committed and shared, silently, under a note reading
        // *Configure the ignored local file, never this versioned block*.
        // Measured: `| Merge strategy | rebase |` in `estigia.local.md`, one
        // `estigia sync`, and the versioned row read `rebase` afterwards —
        // still `rebase` once the override was deleted.
        Ok(_) => skill::contract_config(&paths.skill_root).map(Some),
        // The same spelling `doctor` uses to tell absent from unreadable.
        Err(refusal) if refusal.code != "skill-not-installed" => Err(refusal),
        Err(_) => Ok(None),
    }
}

fn config_for(
    adapter: &'static AgentAdapter,
    shared: &[Config],
    options: &SetupOptions,
) -> Result<Config, Refusal> {
    // Its own answer wins. No contract yet inherits an unambiguous neighbour,
    // which is what makes a second agent land configured rather than default.
    // A contract that will not parse travels out as the refusal it is — one bad
    // row in the operator's own `estigia.local.md`, a file this never edits,
    // cost them every choice in a file it does: `squash` and `sdd openspec`
    // came back `merge commit` and `direct`, under one line reading `update`.
    if let Some(own) = existing_config(adapter, options)? {
        return Ok(own);
    }
    Ok(match shared {
        [only] => only.clone(),
        _ => Config::default(),
    })
}

/// Says that the targets do not agree, before anything is written.
///
/// Not a refusal: every adapter that has an answer keeps it, so the run is
/// correct either way. What an operator cannot be left to discover on their own
/// is that a root with no contract did **not** inherit, because there was
/// nothing unambiguous to inherit.
///
/// Counted by skill **root**, not by adapter, and read with the run's own
/// options. Eight adapters share the neutral root, so naming adapters printed
/// "agents, opencode, gemini-cli, cursor, continue, cline, crush, windsurf, qwen
/// had none" for a *single file* — a line that reads like nine decisions and
/// describes one. It also asked `is_configured` with `SetupOptions::default()`
/// rather than the options in hand, which is the wrong home and the wrong
/// question: a directive is not a contract.
fn report_disagreement(
    shared: &[Config],
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
) {
    if shared.len() < 2 {
        return;
    }
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
    note!(
        "note: {} different configurations are installed across these agents, and each keeps its \
         own.",
        shared.len()
    );
    for root in &fresh {
        note!(
            "  {root} had none, so it got the defaults rather than a guess — `estigia config set` \
             names one."
        );
    }
}

/// Runs setup across every target without discarding a failed adapter's proof.
fn run_setup_over(
    targets: &[&'static AgentAdapter],
    shared: &[Config],
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
    // One memory across the whole run: eight adapters share a skill root, and a
    // plan that read the untouched disk for each of them promised 182 files
    // where the run does 70.
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
    report_over(&done, refused, json, false)
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

/// Takes Estigia out of every named agent, and its own state with it.
///
/// One function because there are two ways to ask — `estigia uninstall` and
/// `estigia setup --uninstall` — and they were two copies. `forget_state` was
/// wired into the first and not the second, so `setup --all --uninstall` left
/// the ledger, the run pointers and any stand-down behind while `uninstall
/// --all` took them. Found by a test that walks the whole home, once its
/// fixture was made to *use* the machine rather than only install on it.
fn take_out(
    targets: &[&'static AgentAdapter],
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
    // The mirror of setup's: eight adapters share a skill root, and a plan that
    // read the untouched disk for each of them promised seventeen files apiece
    // where the run takes out three.
    let mut pending = setup::Pending::new();
    let (done, refused) = collect_over(targets, |adapter| {
        setup::uninstall_from(adapter, options, &mut pending)
    });
    let results: Vec<SetupResult> = done.iter().map(|(_, result)| result.clone()).collect();
    // Estigia's own state, once no agent is left to read it. Named on the way
    // out like everything else this writes: a file that appears without being
    // announced and disappears the same way is one an operator has no way to
    // account for.
    let state = setup::forget_state(options);
    // What is still in the skill directory once ours is out of it. Estigia
    // removes every file it installed, so whatever is left is the operator's —
    // their `estigia.local.md`, their own notes — and the directory stays
    // because of it.
    //
    // Said because the silence reads as failure. The `kept` note already exists
    // for the mirror of this, on the grounds that "eleven unexplained `kept`
    // lines read as an uninstall that failed"; a directory that survives an
    // uninstall with no line about it reads exactly the same way, and the answer
    // an operator wants — *it did not touch my things* — is the one thing the
    // output was not saying.
    // In a plan as well as in an act, and the plan is where it matters more.
    // This was computed only once something had been removed, so the sentence an
    // operator runs `--dry-run` to read — *will it take my things?* — was the
    // one sentence missing from the run they read before deciding. `--dry-run`
    // is sold as *see exactly what would change first*.
    //
    // The listing is the same either way, because it names the files no action
    // named and a plan has actions: nothing has been removed yet, so the walk
    // sees Estigia's files too, and every one of them is named by a `remove`.
    let left = kept_by_the_operator(targets, options, &results);
    if json {
        // One document, not two. These lines were `say!` after the array had
        // already been printed, so `uninstall --all --json` on a machine with
        // any state emitted JSON followed by prose — *Extra data: line 859* to
        // anything parsing it.
        //
        // An object rather than the array, because two different things
        // happened and the array could only carry one of them. Nothing could
        // have depended on the old shape here: it was not parseable whenever
        // there was state to report.
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
        if !left.is_empty() {
            // In the tense of whichever this is. A plan reporting that files
            // "were left there" is a plan claiming to have done something, and
            // an operator reading it has no way to tell it apart from the run
            // that did.
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
        // The one thing Estigia installs outside these directories, and the one
        // it cannot go and find: `estigia guard` writes a `pre-push` hook into
        // whichever repository it was run in, and nothing anywhere records
        // which. So this run removed the agent integration and said nothing
        // about hooks that are still there — under a command whose whole
        // promise is *take the application back out*.
        //
        // Said always, because "cannot know" is the fact: a note that appeared
        // only when Estigia had found one would be claiming it had looked.
        // Nothing here asserts a guard exists — it names where one would be and
        // the command that removes it, which is the only shape that discharges.
        //
        // A leftover hook does not break anything: it fails open and says so
        // when the binary is gone, which `guard::script` explains at length. It
        // is stale rather than dangerous, and stale is still something an
        // operator is entitled to be told about.
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
///
/// Read after the removal rather than before it, and then **checked against the
/// install record**. Reading alone was the whole of it, on the premise that
/// everything of ours is gone by now so what is there is theirs. That premise
/// holds for the three adapters with a skill root of their own and fails for the
/// eight that share `~/.agents/skills`: their skill is deliberately left
/// standing for the agents still configured, and the run says so one line
/// earlier — `n file(s) are the skill … it goes out with the last one`.
///
/// So one output said both. Measured: `estigia setup opencode --uninstall` with
/// two other sharers configured named sixteen files as the operator's, of which
/// **one** was — the other fifteen being `SKILL.md`, the bindings, the transport
/// and Estigia's own install record.
///
/// Both halves do harm. The sentence exists to answer *it did not touch my
/// things*, and it buried its real answer in fifteen files that are not the
/// operator's; and it invited them to delete a skill two configured agents are
/// reading, which is the exact outcome the `shared` note is there to prevent.
///
/// The same premise fails a second way, and the second was measured too:
/// deleting the install record and uninstalling prints `unknown` against fifteen
/// files — *Estigia has no record of installing here, so nothing in this
/// directory is shown to be its own* — and then this note called those fifteen
/// "not Estigia's". `Change::Kept` and `Change::Unrecorded` are separate
/// variants because a fact and the absence of one are not the same sentence;
/// this renderer flattened them back together, which is the defect that split
/// them, in a second place.
///
/// One remedy for both: **the run already classified every file it touched, and
/// each class already has its own note.** So this names what no action named —
/// the files the run has nothing to say about, which are the operator's and the
/// only ones the surviving directory needs explaining by. It is not a fourth
/// copy of the rule; it is what is left when the other three have spoken.
///
/// Directories are skipped because the removal already takes out the empty
/// containers it left.
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
        // Eight adapters share one root; walking it once per adapter would name
        // the operator's file eight times.
        if roots.contains(&paths.skill_root) {
            continue;
        }
        roots.push(paths.skill_root.clone());
        let mut here = Vec::new();
        collect_files(&paths.skill_root, &mut here);
        // The record is the one file with no action of its own, and it survives
        // for the same reason the shared skill does: the agents that remain read
        // it. Naming it as the operator's is the same error one file over.
        let ledger = skill::record::path(&paths.skill_root);
        here.retain(|path| {
            // And not a write of ours that never finished. `replace_atomically`
            // stages beside its target and cleans up when the write *fails*; a
            // process killed between the create and the rename leaves the
            // temporary, and nothing sweeps it — so Estigia's own residue was
            // reported as the operator's, in the sentence that exists to answer
            // *did it touch my things?*.
            *path != ledger
                && !spoken.contains(path.as_path())
                && !crate::paths::is_staged_write(path)
        });
        // Taken away with the rest. A name this tool chose is a file this tool
        // owns, and an uninstall that leaves it is one that did not remove what
        // it created.
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
///
/// Split from [`run_over`] so a caller can decide *where* the report goes. The
/// screen needs it as text to show in place; every other caller prints it. The
/// operation itself is written once either way — two ways to install is the one
/// thing `guided` exists not to have.
/// Runs every target, and hands back what happened **and** what refused.
///
/// It used to stop at the first refusal and propagate it with `?`, which threw
/// away every result collected before it. Measured: one hand-edited
/// `~/.cursor/hooks.json` and `estigia setup --all` wrote **58 files** across
/// six agents, left five untouched, printed nothing but *Cursor: … is not
/// JSON*, and exited `1`.
///
/// Two things wrong with that, and the second is the one this crate is about.
/// The five agents behind the failing one had nothing wrong with them and got
/// nothing, over a file that is not theirs. And exit `1` is
/// [`crate::outcome::ExitCode::Refused`], whose sentence is *the command
/// refused, and nothing was written* — said about a run that had written
/// fifty-eight files and named none of them.
///
/// So: every target is attempted, the report is printed for the ones that
/// worked, and the refusal at the end names all of them that did not. What the
/// refusal *claims about the world* is decided by whether anything was written
/// — see [`refusal_over`].
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
///
/// `NotStarted` only when nothing was: a batch that wrote files and then
/// refused has to say so, because the exit code is the whole of what a script
/// reads. When something landed, the outcome is the one that means *the world
/// moved and this stopped part-way* — which is exactly what happened, and
/// exactly what `ExitCode::Indeterminate` is reserved for.
///
/// The first `Unknown` refusal controls identity and resolution when there is
/// one: resolving an earlier preflight refusal cannot settle a write whose
/// outcome is still unknown. Without uncertainty, the first refusal controls.
/// Every message remains in the combined context either way.
fn refusal_over(
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
    // What happened to the *rest* of the batch, because the outcome's own line
    // is written for a single write — *the write landed; what failed came after
    // it* — and a reader who has just been handed one refusal has no way to
    // tell from it that ten other agents were done. The count is here rather
    // than left to the report above it: the two are read together, and only one
    // of them survives being piped somewhere.
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
///
/// Carrying what the earlier ones moved, because eight of the eleven adapters
/// share one skill root and only the first of them is charged with writing it.
/// The rest come out at nought files — correctly, the run does the work once —
/// and said `already current`, which is a claim about the disk and was false:
/// those files were stale, and are repaired three lines up under another name.
/// An operator reading it concluded that agent needed nothing, where running
/// `sync --agent <that one>` on its own changes fifteen files.
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
    // One word per row, and one row per file. `None` is a file this run did not
    // touch, which earns no line at all.
    for action in &result.actions {
        let Some(mark) = word(action.change, result.dry_run) else {
            continue;
        };
        line!(text, "  {mark:<13} {}", action.path.display());
    }
    if changed == 0 {
        // Two sentences, and one of them was said for both. *Nothing needed
        // doing* and *what this needs is done above under another name* are
        // read by the same operator to opposite ends, and only the first is a
        // statement about the disk they have.
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
            // Not `already current`. That sentence is about an installation
            // being up to date, and it was printed by the command whose whole
            // job is to take one away — so `estigia uninstall` on a machine it
            // had already left answered *already current*, which reads as
            // *your install is fine*. The note two lines up says this exact
            // thing one level down: two sentences, one of them said for both,
            // read by the same operator to opposite ends.
            //
            // `estigia guard --uninstall` had it right all along — *is not
            // there — nothing to remove* — and this is that sentence, for the
            // command beside it.
            line!(text, "  nothing of Estigia's is here");
        } else {
            line!(text, "  already current");
        }
    }
    // Said once rather than beside every row: a directory that already held the
    // skill leaves most of it behind, and eleven unexplained `kept` lines read
    // as an uninstall that failed.
    let count = |wanted| {
        result
            .actions
            .iter()
            .filter(|action| action.change == wanted)
            .count()
    };
    // Said loudly, and before the `kept` notes, because it is the only one that
    // costs the operator something they cannot get back by running a command.
    // Told in the tense it is in. Said the same way in both, the plan closed
    // with "run with --dry-run first" — which is what the reader had just run.
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
    // Its own sentence, because the operator's next move is different. This
    // shared the one above, which told somebody whose own edit had just been
    // discarded that the file had never been Estigia's.
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
        // Not "these were here first" — that is a fact, and this is the absence
        // of one. An install writes the record; without it Estigia cannot show
        // that any of this is its own, including files it did write.
        line!(
            text,
            "  note: {} file(s) stay because Estigia has no record of installing here. An \
             install writes one, so this directory was filled by something else, by a build \
             from before the record, or after somebody removed it. Estigia will not guess \
             on the way out: delete the directory yourself if you know it is its.",
            count(Change::Unrecorded)
        );
    }
    // An instruction file emptied and left where it was. On the way out a
    // `Directive` row is either the file going or the file staying, so this is
    // the second — and it stays for one reason: nothing recorded that Estigia
    // created it, so it reads as the operator's and is not Estigia's to delete.
    //
    // Said because the alternative is a stray empty file with no explanation,
    // which is what an operator upgrading from a build before the record
    // actually gets: their old record names no instruction file, every one of
    // them is treated as theirs, and the uninstall prints `update` and stops
    // talking. The rule that leftovers are named is the rest of that decision,
    // not a separate courtesy.
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
            replay: crate::outcome::Replayability::StatusRequired,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "inspect the lifecycle evidence under ~/.estigia/lifecycle before retrying the installer",
            ),
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
    command: &str,
) -> Result<(), Refusal> {
    if options.dry_run {
        return Ok(());
    }
    let status = lifecycle_status(options)?;
    match status.relation {
        lifecycle::Relation::SourceOrUnrecorded if !allow_source_build => {
            Err(Refusal::not_started(
                "source-build-not-allowed",
                "this build has no matching observed-path installer record, so it may not rewrite installed assets by default",
                Resolution::run(format!("{command} --allow-source-build")),
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

fn after_lifecycle_preflight<T>(
    preflight: Result<(), Refusal>,
    enter: impl FnOnce() -> Result<T, Refusal>,
) -> Result<T, Refusal> {
    preflight?;
    enter()
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
        ///
        /// `current` above is a boolean, and the same two-state answer this
        /// enum was created to replace: "out of date" and "not there" are
        /// different sentences and lead to different commands. The text output
        /// says which; the JSON said `current: false` for both and dropped this
        /// on the floor, so the one reader that cannot ask a follow-up question
        /// got the collapsed answer.
        presence: skill::Presence,
        /// Whether the lifecycle gate is registered for this agent.
        gated: bool,
        /// Whether the workflow tools are registered for this agent.
        tools: bool,
        /// Why there is no gate, when there is none to be had.
        gate_gap: Option<&'static str>,
        /// Whether the skill under this root belongs to another agent too.
        #[serde(skip)]
        shared: bool,
        /// What is wrong with the gate that is registered, if anything.
        ///
        /// `gated` says an entry exists. That is not the same as an entry
        /// naming an event this build has and an executable still on disk —
        /// the entry on the machine this was written on named a debug build
        /// inside a working tree. `doctor` reports it in full; this is here
        /// because `status` is what people read first, and a line saying
        /// `gate on` about a gate that cannot run is the failure the whole
        /// tool is written against.
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
        // Through `presence_of`, which reads the configuration rather than
        // defaulting it: comparing what is installed against what this binary
        // would write out of a table nobody wrote reported an unrecognised value
        // as a stale skill, and sent the reader to a `sync` that refuses.
        let presence = skill::presence_of(&paths.skill_root);
        rows.push(Row {
            agent: adapter.slug,
            configured: setup::is_configured(adapter, &options),
            current: presence == skill::Presence::Current,
            presence,
            gated: setup::is_gated(adapter, &options),
            // A wire's own fault, and — when there is no wire at all — the fact
            // that one reader sees an entry here and the other cannot read it.
            //
            // `is_gated` recognises the entry by `hook pre-tool-use`; `wire_in`
            // requires the executable's own file name to hold `estigia`. A copy
            // renamed to anything else is plain to the first and invisible to
            // the second, so `registered` comes back empty and this found no
            // fault to report. Measured: pointing `.claude/settings.json` at
            // `…\ausente.exe` made `doctor` answer `BROKEN gate` and this line
            // go on saying `gate on, tools on` about the same machine.
            //
            // The doctor row learned it one round earlier and this one did not,
            // which is the *fifth* time this crate has found the two commands
            // answering differently — and `status` is the one people read
            // first, which is the argument written three lines below.
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
            // The same two questions `doctor`'s own row asks, asked here for the
            // reason the gate's fault is asked here: *somebody who reads `gate
            // on` closes the terminal*. That argument was written for one half
            // of this line and the other half went on saying `tools on` about a
            // server that names a binary which is not there — measured by moving
            // the path in `.claude.json`, where the gate half said `REGISTERED
            // BUT DEAD` and this one did not blink.
            tools_fault: tools_fault(adapter, &options),
            gate_gap: adapter.gate_gap(),
            shared: setup::skill_shared_with(adapter, &options, &setup::Pending::new())
                .ok()
                .flatten()
                .is_some(),
            skill_root: paths.skill_root.display().to_string(),
            instructions: paths.instructions.display().to_string(),
        });
    }

    if json {
        // Everything the prose says, not only its first section. This printed
        // the agents and stopped, so a machine asking `status --json` could not
        // find out **which runs hold which issues** or **which pointers cannot
        // be read** — the two answers this command exists to give. The first is
        // incident I06, five runs that died after claiming and sat unnoticed;
        // the second is what stops a push from a checkout nothing else holds.
        //
        // `--json` was honoured in form and not in content, which is the same
        // defect `gate` had and the same words its note uses: a machine reading
        // prose has to parse a sentence to find out what is so.
        //
        // An object rather than the bare array it was. A shape that omits half
        // the answer is worse than a shape that changes once.
        let holdings = harness::session::state_root(None)
            .map(|root| {
                let held: Vec<serde_json::Value> = harness::session::holdings(&root)
                    .into_iter()
                    .map(|run| {
                        serde_json::json!({
                            "run_id": run.run_id,
                            "issue": run.issue,
                            "state": run.state,
                            // Both: the seconds a program compares, and the
                            // words a person reads. Publishing only the prose
                            // would be the same defect facing the other way.
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
            "estigia": env!("CARGO_PKG_VERSION"),
            "skill": skill::version(),
            "agents": rows,
            "holdings": holdings.0,
            "unreadable_pointers": holdings.1,
            // A machine reading this has to be able to see it too. Publishing
            // the prose only would be the same defect facing the other way —
            // the note beside `silent_for` above says so for its own pair.
            "standing_down": standing_down_line(),
        }));
        return Ok(());
    }

    say!(
        "estigia {} — skill {} ({})",
        env!("CARGO_PKG_VERSION"),
        skill::version().unwrap_or("unknown"),
        skill::DIRECTORY
    );
    // Before the agents, because it changes how every line under it reads:
    // `gate on` while the gate is standing down means the hook is registered
    // and deciding nothing.
    //
    // `doctor` said this all along and `status` did not, in either shape — the
    // command an operator runs to see the state of their machine reported the
    // gate as on while every write went through unadjudicated. The module that
    // declares stand-downs ends on the sentence this broke: *what it does not
    // do is make anything quiet. That is the whole difference.*
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
            // The harness is the difference between a contract and a gate, so
            // it is reported separately rather than folded into "configured".
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
            if let Some(fault) = &row.tools_fault {
                for line in wrapped(fault, 68) {
                    say!("                   {line}");
                }
                say!("                   run: estigia setup {}", row.agent);
            }
            // Said here rather than left to `doctor`: somebody who reads
            // `gate on` closes the terminal.
            if let Some(fault) = &row.gate_fault {
                for line in wrapped(fault, 68) {
                    say!("                   {line}");
                }
                say!("                   run: estigia setup {}", row.agent);
            }
            // Why, when there is no gate to be had. "gate off" alone leaves an
            // operator wondering whether they declined it or Estigia cannot
            // give it, and those call for different actions.
            if let Some(gap) = row.gate_gap {
                for line in wrapped(gap, 68) {
                    say!("                   {line}");
                }
            }
        }
    }
    // The push guard is per-repository, so it is reported for the one the
    // caller is standing in rather than for the machine. Reported and never
    // installed from here: a `setup --all` that wrote a hook into whatever
    // checkout somebody happened to be in would be a surprise, and surprises in
    // other people's repositories are how a tool gets uninstalled.
    if let Ok(repo_dir) = std::env::current_dir() {
        let state = harness::guard::state(&repo_dir);
        if state != harness::guard::State::Absent
            || harness::guard::hooks_directory(&repo_dir).is_ok()
        {
            println_guard(&repo_dir, state);
        }
    }

    // Incident I06: runs that died after claiming left issues assigned and
    // labelled `ready` until a person noticed. This is where they become
    // visible — the tracker is still the authority on whether the claim is
    // live, and these are the issues worth asking it about.
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
                run.issue.unwrap_or_default(),
                run.state.as_deref().unwrap_or("unknown"),
            );
        }
        say!("  a claim is live only while the tracker says so — `estigia release --run-id <id>`");
        say!("  puts one down, and a run that ended without releasing still holds its issue.");
    }

    // The pointers that answered nothing. Said here because the push guard now
    // refuses on them by name, and this is where an operator comes next: a
    // guard saying "a run pointer cannot be read" and a status that lists what
    // is held without mentioning it are two commands describing one machine and
    // disagreeing.
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
///
/// Counted in characters rather than bytes. A terminal wraps at columns, and
/// `len()` answers in bytes — so every accented character in a message made the
/// line it sat on a column shorter than asked, and a sentence of mostly
/// non-ASCII wrapped at roughly half the width. Nothing crashed, which is why it
/// went unnoticed: the output was merely ragged, in the messages read by whoever
/// configured a tracker in a language that is not English.
///
/// Characters, not display columns. A CJK glyph occupies two columns and counts
/// as one here, and correcting that needs a table of widths this crate does not
/// carry. Named rather than quietly approximated.
///
/// A word longer than `width` gets a line of its own and overruns it, because
/// the alternative is splitting it — and the long words here are filesystem
/// paths, where a break in the middle is worse than a long line.
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
        // Left alone for the same reason, and said differently because there is
        // something for the operator to fix: nothing here knows whose hook it is.
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
    let (_, config) = contract_of(agent, options)?;
    // With this checkout's own rows on it, because that is what a run here
    // reads. Without this, `config set --repo` named the file it had written
    // and this command answered with the contract's value one command later —
    // a write this tool confirmed and a read this tool contradicted, on the
    // same row, which is the defect `contract_of`'s note records being fixed
    // once already for `--agent`.
    //
    // `writable_config` deliberately does not: what `config set` edits is the
    // contract, and showing it the repository's answer would write one layer's
    // values into the other.
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let config = skill::layer_repository(&config, &here)?;
    if json {
        #[derive(serde::Serialize)]
        struct Row {
            setting: &'static str,
            value: String,
            accepts: &'static str,
        }
        let rows = crate::config::SETTINGS
            .iter()
            .map(|setting| Row {
                setting: setting.label(),
                value: setting.value_of(&config),
                accepts: setting.accepted(),
            })
            .collect::<Vec<_>>();
        print_json(&rows);
        return Ok(());
    }
    for setting in crate::config::SETTINGS {
        say!(
            "{:<26} {:<24} accepts: {}",
            setting.label(),
            setting.value_of(&config),
            setting.accepted()
        );
    }
    Ok(())
}

/// Declares — or lifts — a stand-down.
///
/// Writing it is a boundary of its own: it loosens the gate, so it is announced
/// on standard output with what it covers and when it stops. A stand-down an
/// operator has to go looking for is one they can forget they declared.
fn stand_down(reason: &str, minutes: u64, lift: bool, json: bool) -> Result<(), Refusal> {
    let root = harness::session::state_root(None)?;
    let file = harness::standdown::path(&root);
    // A clock the process cannot read is not a reason to refuse a stand-down —
    // it lands as the epoch, the window is still bounded, and `doctor` is where
    // a broken clock gets reported. Refusing here would leave somebody unable
    // to loosen a gate that is already wrong.
    let now = harness::session::now_seconds().unwrap_or_default();
    let who = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "an operator".to_owned());
    // What is already in force, read before anything replaces it.
    //
    // The record on disk is **state**, not history: the next declaration writes
    // over it, and the reason and the declarer of the one it replaced are gone.
    // For the one act that loosens the gate itself, that leaves nothing to be
    // answerable for — which is the whole shape this feature claims to have.
    //
    // Through `standing`, so that a record which is there and will not open is
    // not read as no record at all. Declaring over one is fine — a later
    // declaration supersedes — but reporting `superseded: null` about a file
    // that existed is the ledger asserting something it did not check, in the
    // one act whose whole claim is being answerable for itself.
    let existing = harness::standdown::standing(&root);
    let standing = match &existing {
        harness::standdown::Standing::Declared(record) if record.covers(now) => {
            Some(record.clone())
        }
        _ => None,
    };
    let unreadable = matches!(existing, harness::standdown::Standing::Unreadable(_));

    if lift {
        // Removing the record is the operator undoing their own declaration
        // before it expires — the direction that never needs *guarding*, which
        // is not the same as the direction that never needs checking. The
        // result used to be discarded, and everything below it says the window
        // is closed: `stand-down lifted; the gate decides on its own again`,
        // and, when the file could not be read, `it has been taken away`. Both
        // are assertions about a removal nobody looked at.
        //
        // Measured on this machine, with the file held open by another process
        // — an editor, a backup, an antivirus, which on Windows is ordinary:
        // the command printed *lifted*, exited **0**, and `doctor` in the next
        // breath said *the gate is standing down for another 30 minute(s) —
        // writes go through unadjudicated until it expires*. The one act whose
        // whole job is closing a loosened gate reported closing one it had not.
        //
        // Read back rather than trusted: a removal that failed because it was
        // already gone is a lift, and only a file still on disk is a refusal.
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
            // Said rather than passed over. Somebody who lifts a stand-down that
            // had already expired should not walk away believing they closed a
            // window that was open.
            // Said rather than passed over, and the two are different
            // sentences: a window that had expired is not a file nobody could
            // open. The second is a machine state somebody has to know about,
            // because nothing else mentions it and it has just been removed.
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
            // No command, because none exists. This is a defect in this build,
            // not a state of the machine, and `doctor` — which it named — has
            // nothing to say about a serialiser. Naming a dead end is worse
            // than naming nothing.
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

    // Into the ledger, which is append-only and is where this belongs. The
    // ledger exists because "why did it stop me?" had no answer an hour later;
    // "why did it *not* stop me?" is the same question about the same authority,
    // and standing the gate down is the most consequential answer to it.
    //
    // Every declaration lands here, including one that replaces another — so a
    // window held open by repeated declaration is a row of lines saying so,
    // rather than one file that only ever shows the latest.
    //
    // Bound once for both readers below. They are the same statement said to a
    // person and to a program, and they were written out twice.
    //
    // `null` means *nothing was in force*. A record that was there and would not
    // open is not that, and reporting it as that would be this line asserting
    // something it never read — in the one act whose claim is being answerable
    // for itself.
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
///
/// `1 minute(s)` reads as a defect in the tool rather than as a rounding of
/// English, and this is the one message an operator sees at the moment they are
/// loosening their own gate.
fn minutes_left(seconds: u64) -> String {
    let minutes = seconds.div_ceil(60);
    match minutes {
        0 => "less than a minute".to_owned(),
        1 => "1 minute".to_owned(),
        _ => format!("{minutes} minutes"),
    }
}

/// Appends one line to the decision ledger, best effort.
///
/// Never fails the command: a declaration that could not be written down is
/// still a declaration, and refusing to stand the gate down because a log file
/// was unwritable would be a harness that gets uninstalled instead.
fn trail(state_root: &std::path::Path, entry: &serde_json::Value) {
    harness::session::record(state_root, entry);
}

/// A record that could not be written.
/// Takes the stand-down record away, or says it is still there.
///
/// The removal used to be `let _ = remove_file(...)`, and everything printed
/// after it asserts the window is closed: *stand-down lifted; the gate decides
/// on its own again*, and, when the file could not be read, *it has been taken
/// away*. Both are claims about a removal nobody looked at.
///
/// Measured with the file held open by another process — an editor, a backup,
/// an antivirus, which on Windows is ordinary rather than exotic: the command
/// printed *lifted*, exited **0**, and `doctor` in the next breath said *the
/// gate is standing down for another 30 minute(s) — writes go through
/// unadjudicated until it expires*. The one act whose whole job is closing a
/// loosened gate reported closing one that was still open, which is the third
/// rule of this project said backwards.
///
/// Read back rather than trusted, and in that order: a removal that failed
/// because the file was already gone is a lift like any other, so the error
/// alone is not the finding — a file still on disk is.
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
        });
    }
    Ok(())
}

/// The record could not be written, and only a person can change that.
///
/// It named `estigia doctor`, which reports and writes nothing — and in this
/// state does not even mention the file: the record is still perfectly
/// **readable**, so the stand-down row answers about the window in force and
/// the operator reads a correct report about something else. The path was in
/// this refusal's own message the whole time.
///
/// Found by walking every `Resolution::run` in the crate and asking of each
/// whether running it clears what produced it. The same dead end as
/// `setup-write-failed`, one command over, and it takes the same answer.
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
///
/// The screen decides and this applies, exactly as setup does — so the
/// configuration still has one writer, and a value that reaches the file has
/// passed the same validation whichever way it was typed.
fn config_edit(agent: Option<&str>, options: &SetupOptions) -> Result<(), Refusal> {
    let writable = writable_config(agent, options)?;
    let target = writable.target.clone();
    let installed = writable.config;
    let Some(config) = crate::tui::edit(agent, installed)? else {
        // Leaving without saving is a choice, not a failure. Saying nothing
        // would leave an operator wondering whether it wrote; saying it failed
        // would be a lie.
        println!("nothing changed");
        return Ok(());
    };
    let agent_write = writable
        .agent_snapshot
        .as_ref()
        .map(|snapshot| (agent.unwrap_or_default(), snapshot));
    write_edited_table(&target, agent_write, &config)?;
    match agent_write {
        Some((slug, _)) => println!("configuration for {slug} written to {}", target.display()),
        None => println!("configuration written to {}", target.display()),
    }
    Ok(())
}

/// Writes an edited table, and refuses when a run will not read back what it
/// wrote.
///
/// One function, because it was two branches and only one of them checked. The
/// shared contract read back every row and named the ones `estigia.local.md`
/// still overrode, on the argument written beside it — *`config set` already
/// refuses that for one row; a screen that writes fourteen has fourteen chances
/// to do it*. The branch above it wrote an adapter's own file, printed
/// `configuration for <agent> written to …` and returned, with nothing read
/// back at all. Both files sit **under** the operator's local override, so both
/// have the same fourteen chances.
///
/// Only the rows the file being written can carry. An adapter's own file holds
/// [`crate::config::AGENT_SETTINGS`] and nothing else — a repository fact in it
/// is put back from the contract by the reader, deliberately — so comparing the
/// whole table there would report every repository row as overridden and refuse
/// a write that was perfectly good. Reading that back as shadowing is the fix
/// that would have been worse than the defect.
fn write_edited_table(
    target: &std::path::Path,
    agent: Option<(&str, &setup::AgentConfigurationSnapshot)>,
    config: &Config,
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
        }
    }
    let effective = read_back(match agent {
        Some((slug, _)) => skill::installed_config_for(root, Some(slug)),
        None => skill::installed_config(root),
    })?;
    let rows = match agent {
        Some(_) => crate::config::AGENT_SETTINGS,
        None => crate::config::SETTINGS,
    };
    let shadowed = shadowed_rows(rows, config, &effective);
    if shadowed.is_empty() {
        return Ok(());
    }
    Err(shadowed_table(root, &shadowed))
}

/// Sets one repository-wide row in every installed contract but `written`.
///
/// Returns how many others took it, and the roots that did not. Each keeps the
/// rest of its own table: the row is applied to the configuration that contract
/// already holds, not copied over it, so an agent's own answers survive a
/// repository-wide change.
///
/// **Read back, one root at a time.** The primary write is checked and refuses
/// when `estigia.local.md` overrides the row; these were written and never read,
/// and their count was reported as *"written into N other installed contract(s)
/// … so every agent's copy says it"*. Measured on the installed pair, with the
/// operator's own file in the shared neutral root answering that row: the
/// message said every copy said it and `config list --agent opencode` answered
/// `none`.
///
/// That is the incident the paragraph at the call site describes — *the agent
/// reading the other was never told what it had walked into* — reproduced by
/// the loop written to prevent it, on the row that declares a one-way door.
fn elsewhere(
    written: &std::path::Path,
    setting: Setting,
    value: &str,
) -> Result<(usize, Vec<std::path::PathBuf>), Refusal> {
    let options = SetupOptions::default();
    let mut done: Vec<std::path::PathBuf> = vec![written.to_owned()];
    let mut refused: Vec<std::path::PathBuf> = Vec::new();
    for adapter in configured(&options) {
        let Ok(paths) = setup::resolve_paths(adapter, &options) else {
            continue;
        };
        let contract = paths.skill_root.join(skill::CONTRACT);
        // Eight adapters share the neutral root, so this comes round eight
        // times for one file.
        if done.contains(&contract) {
            continue;
        }
        // Left alone rather than overwritten: the rule from every other write
        // path here — a contract that will not read back is not one anything
        // can say the cost of replacing.
        let Ok(mut held) = skill::installed_config(&paths.skill_root) else {
            continue;
        };
        setting.apply(&mut held, value)?;
        setup::rewrite_configuration(&contract, &held)?;
        // What that root will actually answer, not what was just written into
        // it. A root whose local override carries this row keeps answering what
        // that file says, and saying otherwise is the reported effect that did
        // not happen.
        let took = skill::installed_config(&paths.skill_root)
            .is_ok_and(|effective| setting.value_of(&effective) == setting.value_of(&held));
        if !took {
            refused.push(paths.skill_root.clone());
        }
        done.push(contract);
    }
    Ok((done.len() - 1, refused))
}

/// Writes one row into this repository's own file, creating it if need be.
///
/// The one place that creates it. Everywhere else only keeps it current, and
/// deliberately: a file made on every install would move every operator's rows
/// out of the contract they are in today. Here somebody has typed `--repo`,
/// which is a repository saying *I answer for myself* in as many words.
///
/// A row that is about an agent is refused rather than written. It is the same
/// asymmetry the per-agent files already keep from the other side — *a file may
/// narrow what its own agent does, never restate what the repository is* — and
/// it is enforced at the door because the reader puts such a row back silently,
/// which would report a value as set that nothing would ever read.
fn repository_set(
    setting: Setting,
    value: &str,
    here: &std::path::Path,
    options: &SetupOptions,
) -> Result<(), Refusal> {
    if setting.scope() != crate::config::Scope::Everywhere {
        // Two reasons a row is not the repository's, and they do not have the
        // same way out. An agent row is set with `--agent`; a machine row is
        // set with neither flag, and saying `--agent` for one of those would
        // name the refusal this crate raises one command later — a dead end,
        // which its own rules call worse than naming nothing. That is exactly
        // what this said before machine rows were made to propagate, because
        // until then `--agent` was the only other door.
        let (mine, way_out) = match setting.scope() {
            crate::config::Scope::Machine => (
                "a fact about this machine, not what this repository is",
                // Not a named command: the label has spaces in it, and a line
                // an operator has to re-quote by hand is not one that running
                // clears the block.
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
    // There has to be a repository first. `.git` is where this file lives, and
    // creating that directory where none exists does not make a checkout — it
    // leaves a `.git` that git itself will not recognise, in somebody's
    // ordinary folder, put there by a tool they asked to record a setting.
    //
    // Found by running the command in this crate's own directory, which is
    // deliberately not a git repository: it created one.
    if !here.join(".git").exists() {
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
        // A row this repository already carries and cannot be parsed is a
        // stop, not an empty table: writing over it would throw away an answer
        // somebody gave.
        Some((_, text)) => crate::config::Config::read_scope_over(
            &crate::config::Config::default(),
            &text,
            crate::config::Scope::Everywhere,
        )
        .map(|(read, settings)| {
            spoken_for = settings;
            read
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
    setting.apply(&mut config, value)?;
    // The rows this file already answered for, plus the one being asked now.
    // Writing the whole scope turned every row nobody asked about into an
    // answer — with the **defaults**, because a file that is not there yet is
    // read as `Config::default()`. Measured through the binary, one command:
    // a checkout configured for `github acme/web` and `C:/trees` came back
    // `github` and `unset` after being asked about `Merge strategy` alone.
    let mut speaks_for = spoken_for;
    if !speaks_for.contains(&setting) {
        speaks_for.push(setting);
    }
    setup::write_repository_configuration(&path, &config, &speaks_for)?;
    // Recorded so the checkout can be found again from anywhere. Best effort:
    // the answer is already in the repository's own file, and a machine that
    // cannot keep a convenience list has lost nothing the gate reads.
    // The caller's home, falling back to the machine's only when nobody named
    // one. It asked `paths::home_dir()` unconditionally, which is the real
    // profile whatever it was handed — so every `config set --repo` in the test
    // suite recorded into the developer's own `~/.estigia/repositories`. Found
    // by reading it: 896 entries, and all 896 were temporary directories from
    // test runs. A suite that writes into the profile it is pretending to
    // sandbox is measuring a machine nobody will ever have.
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

/// Lists the checkouts that answer for themselves.
///
/// Pruned by the reader, so a repository that has been deleted or has had its
/// file taken away is simply not here — the list is what is true now rather
/// than what was ever true.
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
    // `--json` is declared once, on the root parser, and honoured in as many
    // places as there are commands. This one and `config forget` were written
    // after the list that checks it and never added to it, so both printed
    // prose under the flag — including the empty case, which is the one a
    // program most needs to tell apart from a failure.
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
    // The registry prunes itself on the next read, so nothing has to be taken
    // out of it here — one place decides what is on the list, and it is the
    // reader.
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
    options: &SetupOptions,
    json: bool,
) -> Result<(), Refusal> {
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
    // The value is checked before the machine is, against a default table —
    // parsing a value never depends on what the other rows say, and this is the
    // one thing `config set --help` promises: *validating it before anything is
    // written*.
    //
    // The order was the other way round for the value and not for the label,
    // which is two different answers to one question. `config set "Merge
    // strategy" octopus` on a machine with nothing installed said "run setup" —
    // so the operator installed, retyped, and only then learned that `octopus`
    // was never a merge strategy. Two round trips for two faults that were both
    // knowable from the command line alone, and the one they could fix without
    // leaving the terminal was the one that went unmentioned.
    //
    // The rule, once, so it does not have to be rediscovered per argument:
    // **everything knowable from the command line is settled before anything on
    // disk is read**, in the order the arguments were written. The agent names
    // which table, so it goes first even though nothing here needs it yet.
    if repo {
        // The directory is read here and handed down, so the writer can be
        // driven from a test without moving the process — the shape this crate
        // already learned the hard way with `PATH`.
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
    if let Some(slug) = agent {
        find_agent(slug)?;
        // A setting whose scope is the repository has one answer, and the gate
        // reads it without asking which agent is holding the tools. Written
        // into one adapter's file it was reported as set, read back as set by
        // `config list --agent`, and ignored by every decision that consults
        // it — and once the agent is told to read that file too, the agent and
        // the gate disagree about which tracker they are talking to.
        //
        // A repository row and a machine row are both *not per agent*, and this
        // door named only the first. `Scope::Machine` fell straight through it:
        // the write landed in a per-agent file, `render_some_agent_rows`
        // intersects those rows with `AGENT_SETTINGS` and dropped it on the way
        // out, and the command then answered `setting-shadowed-by-local-file` —
        // blaming an operator file that need not exist in that root at all. So
        // two agents on one machine could be left answering a question that has
        // one answer differently, with no command that made them agree.
        //
        // Knowable from the command line, so it is settled before anything on
        // disk is read, like the two checks around it.
        if setting.scope() != crate::config::Scope::Agent {
            // Both arms named, so a fourth `Scope` stops the build here rather
            // than being described as a fact about this machine by falling off
            // the end of a wildcard. `Agent` is excluded by the `if` above and
            // is the one arm that cannot be reached.
            let about = match setting.scope() {
                crate::config::Scope::Everywhere => "this repository",
                // A person writes in one language across every checkout they
                // have — `Scope::Machine`'s own words. What makes the row
                // un-per-agent is that they are one person whichever agent is
                // holding the tools.
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
    }
    setting.apply(&mut Config::default(), value)?;

    let writable = writable_config(agent, options)?;
    let target = writable.target;
    let mut config = writable.config;
    // Against the tracker this checkout chose, before anything is written. A
    // board is a GitHub Projects thing: `bindings/linear.md` and
    // `bindings/trello.md` declare no mirror and the transport asks for one
    // only under GitHub, so a board written under either is a value nothing
    // will ever read — reported as set. The screen stopped offering the row;
    // this is the door that writes, and two doors disagreeing about one rule is
    // the fault this crate keeps finding.
    // Putting one **back** is always allowed. Refusing every write to the row
    // refused `none` as well, so a board answered before the tracker moved could
    // not be taken away — and `doctor` reports that row and names this very
    // command as the way out, which made the health report point at a refusal.
    // Two changes that were each right on their own.
    //
    // Removing an inert answer is not configuring a setting the tracker does not
    // read: it is the one act that ends the state being complained about.
    let clearing = setting.value_of(&config) != setting.value_of(&Config::default())
        && Setting::from_label(label)
            .and_then(|setting| {
                let mut wanted = Config::default();
                setting.apply(&mut wanted, value).ok()?;
                Some(setting.value_of(&wanted) == setting.value_of(&Config::default()))
            })
            .unwrap_or(false);
    if !clearing && !setting.applies_to(&config.tracker) {
        return Err(Refusal::not_started(
            "setting-not-for-this-tracker",
            format!(
                "{} has no meaning under {}: that binding declares no board mirror",
                setting.label(),
                config.tracker.as_value()
            ),
            Resolution::run("estigia config set Tracker github"),
        ));
    }
    setting.apply(&mut config, value)?;
    if let Some(snapshot) = writable.agent_snapshot.as_ref() {
        // A shared skill root: this adapter's own file, so `--agent opencode`
        // cannot write the table `claude-code` reads.
        let slug = agent.unwrap_or_default();
        // The rows this file already answers for, plus the one being asked now
        // — the same rule the repository's file follows, and for the same
        // reason: a row it does not carry is one this adapter does not answer
        // for, and the shared contract's answer stands. Writing the whole scope
        // pinned five rows for every one asked about, so moving the machine's
        // `Planning` afterwards moved every adapter except the one somebody had
        // once asked a different question about.
        let mut speaks_for = snapshot.settings();
        if !speaks_for.contains(&setting) {
            speaks_for.push(setting);
        }
        setup::write_agent_configuration_from_snapshot(snapshot, slug, &config, &speaks_for)?;
        // Then read back, exactly as the shared table below does. This path
        // returned on the strength of the write alone.
        let root = target.parent().unwrap_or(&target);
        let effective = read_back(skill::installed_config_for(root, Some(slug)))?;
        if setting.value_of(&effective) != setting.value_of(&config) {
            return Err(shadowed(root, setting, &config, &effective));
        }
        say!(
            "{} is now {:?} for {slug} ({})",
            setting.label(),
            setting.value_of(&config),
            target.display()
        );
        return Ok(());
    }
    let contract = target;
    // Passed through rather than re-wrapped: the refusal already carries the
    // precise code and the resolution, and folding it into a second one would
    // replace `skill-not-installed` with a permissions story that is not true.
    let change = setup::rewrite_configuration(&contract, &config)?;

    // Read back what the operator will actually read. The value goes into the
    // versioned table, and `estigia.local.md` beside it overrides row for row —
    // so setting a row that file also carries wrote a value nobody would ever
    // see, under a message saying it was now in force. Reporting an effect that
    // did not happen is the failure this whole tool exists to refuse, and it was
    // happening in the tool's own configuration.
    //
    // The same move issue-flow's `transition` makes: write, then read *both*
    // back, and believe the readback rather than the write.
    let root = contract.parent().unwrap_or(&contract);
    let effective = read_back(skill::installed_config(root))?;
    if setting.value_of(&effective) != setting.value_of(&config) {
        return Err(shadowed(root, setting, &config, &effective));
    }
    // And through each agent's own file, which is what a run actually reads.
    //
    // The readback above is the shared table, and the sentence above it —
    // *"read back what the operator will actually read"* — was not true of it:
    // eight adapters share this root, `installed_config_for` lays each one's
    // `estigia.<slug>.md` over the table, and a row answered there is the row a
    // run gets. Measured on a real machine: `config set Planning direct`
    // answered *Planning is now direct*, all three shared tables said `direct`,
    // and `config list` one command later said `sdd lite` — because nine
    // per-agent files still carried it and nothing here looked at them.
    //
    // That is this tool reporting a state it did not read back, in its own
    // configuration, which is the one failure it exists to refuse.
    let unread: Vec<&'static str> = configured(options)
        .into_iter()
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
    //
    // The refusal `config set --agent` raises for one of these says why: "a
    // setting whose scope is the repository has one answer, and the gate reads
    // it without asking which agent is holding the tools". Written into one
    // contract and no other, it varied by agent anyway — an operator with two
    // agents installed declared `make deploy` a one-way door, and only one of
    // the two contracts said so. The gate read the one that did and enforced
    // it; the agent reading the other was never told what it had walked into,
    // and uninstalling the first would have taken the boundary with it.
    //
    // Each contract keeps its own per-agent rows: this sets one row in the
    // configuration each of them already holds, rather than copying one over
    // the rest. And a contract that will not read back is left alone — nothing
    // can preserve what it could not read.
    // A machine row spreads for the same reason a repository row does, and it
    // used to be left behind here because the condition named one scope where
    // two belong. `Scope::Machine` is a fact about the person at this keyboard,
    // so a root that goes on answering something else is exactly the divergence
    // `doctor`'s `canonical` row reports — and, before this, reported with no
    // way out to offer.
    let (spread, unread) = if setting.scope() != crate::config::Scope::Agent {
        elsewhere(&contract, setting, value)?
    } else {
        (0, Vec::new())
    };
    // Named before the count is reported, because the count is the sentence that
    // was wrong. A root that will go on answering something else is the whole of
    // what the propagation exists to prevent.
    if let Some(root) = unread.first() {
        return Err(shadowed_elsewhere(root, setting, unread.len()));
    }

    // Both branches report the value the table holds, not the one that was
    // typed. They used to disagree: the changed branch resolved and this one
    // echoed, so `config set "Irreversible commands" ""` answered "was already
    // " with a blank, and `NONE` answered "was already NONE". All three hold
    // `none`. Reporting the input back is the tool agreeing with whoever spoke
    // last rather than saying what is so.
    let held = setting.value_of(&config);
    // The whole answer, for the reader that cannot ask a follow-up question:
    // what the row now holds, whether it moved, how many other contracts it
    // landed in, and — the part the prose learned last — what this checkout
    // answers instead when it answers differently.
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
        }));
        return Ok(());
    }
    match change {
        Change::Unchanged => say!("{} was already {held}", setting.label()),
        _ => say!("{} is now {held}", setting.label()),
    }
    // And what this checkout answers, when it answers differently. The
    // read-back above sees the contract and the operator's own file; the
    // repository layer sits above both and it never learned about it, so
    // `config set "Merge strategy" rebase` reported the row as now in force
    // while `config list` one command later answered `squash`. A write this
    // tool confirmed and a read this tool contradicted, on the same row.
    //
    // Said rather than refused, and that is the difference from the operator's
    // file. That one shadows the row in *every* checkout, so the write is in
    // force nowhere and refusing is the honest answer. This one shadows it
    // here: the value is now the machine's everywhere else, which is usually
    // what was wanted. What was missing is the other half of the sentence — and
    // the one command that makes the two agree.
    let here = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Ok(layered) = skill::layer_repository(&config, &here)
        && setting.value_of(&layered) != held
    {
        say!(
            "  this checkout answers {} instead \u{2014} it says so in its own file, and \
             `estigia config set --repo` is what changes that",
            setting.value_of(&layered)
        );
    }
    // Counted out loud. A row that landed in four files is four files an
    // operator's next `--dry-run` will report as changed, and a number they did
    // not ask for is better than four lines they cannot account for.
    if spread > 0 {
        // Which kind of fact it is, rather than "this repository" for both. A
        // machine row propagates through this same branch now, and saying it is
        // about the repository here while the refusal two hundred lines up says
        // it is about the machine is one fact written twice, disagreeing with
        // itself in the same tool's own output.
        let about = match setting.scope() {
            crate::config::Scope::Machine => "this machine",
            _ => "this repository",
        };
        say!(
            "  written into {spread} other installed contract(s): it is a fact about {about}, so \
             every agent's copy says it"
        );
    }
    Ok(())
}

/// The row was written, and still does not read back.
///
/// Built once because there are two write paths — the shared table and one
/// adapter's own file — and only one of them used to look. `config set --agent
/// opencode` reported a row as now in force for that agent while every run went
/// on reading the operator's `estigia.local.md`, which sits *above* the
/// adapter's file by design and so makes this the likelier path, not the rarer.
/// Every row that was written and still reads as something else.
///
/// One reader for the two screens that write a whole table. `config_edit` had it
/// inline and said why in its own comment — *a screen that writes fourteen has
/// fourteen chances to do it* — and the screen that installs writes the same
/// fourteen and had nothing. Measured through both doors: `config set "Merge
/// strategy" squash` refuses with `setting-shadowed-by-local-file`, the guided
/// install accepts the identical value, and what the operator then reads is
/// `rebase`.
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
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what answers those rows instead \u{2014} `estigia config list --agent <agent>` \
                 reports what each one reads",
            ),
        };
    };
    // Committed, not `not_started`: the table *was* written. What did not happen
    // is that a run will read it — and saying "nothing was written" would send
    // the operator to repeat a write that already landed.
    Refusal {
        code: "setting-shadowed-by-local-file",
        message: format!("written, and {file} still overrides {rows}"),
        outcome: crate::outcome::MutationOutcome::Committed,
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!("those rows changed or removed in {file}, which Estigia will not edit"),
        ),
    }
}

/// The refusal for a contract the row was propagated into and does not reach.
///
/// A sibling of [`shadowed`], and separate because the sentence is a different
/// one: that root's own table did take the value, and the operator's file beside
/// it answers first. The count of *others* is carried because it is the number
/// the message used to report as agreement — *written into N other installed
/// contract(s) … so every agent's copy says it* — and knowing how many are in
/// that state is the difference between one adapter to look at and eight.
fn shadowed_elsewhere(
    root: &std::path::Path,
    setting: crate::config::Setting,
    roots: usize,
) -> Refusal {
    let Some(shadow) = overriding_file(root) else {
        // The reach of this one grew when machine rows started propagating, so
        // it is the sibling most likely to be met with no file to blame.
        return Refusal {
            code: "setting-not-read-back",
            message: format!(
                "{} was written into {roots} other installed contract(s), and {} answers that row \
                 as something else \u{2014} with no local override there to account for it",
                setting.label(),
                root.display()
            ),
            outcome: crate::outcome::MutationOutcome::Committed,
            replay: crate::outcome::Replayability::NotReplayable,
            resolution: Resolution::no_command(
                crate::outcome::NoCommandReason::OperatorKnowledge,
                "what answers that row in that root instead \u{2014} `estigia config list --agent \
                 <agent>` reports what each one reads",
            ),
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
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!(
                "that row changed or removed in {shadow} \u{2014} it is the operator's file, \
                 and Estigia will not edit it"
            ),
        ),
    }
}

/// The refusal for a row an agent's **own** file answers instead of the table.
///
/// A third sibling, and the sentence is different again: the operator's file is
/// not involved, the shared table did take the value, and what overrides it is a
/// file **Estigia itself wrote** through `config set --agent`. So unlike the two
/// above, the way out is a command rather than operator knowledge — and naming
/// one that clears the block is what the ratchet requires.
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
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::run(format!(
            "estigia config set {:?} {value:?} --agent {}",
            setting.label(),
            agents[0]
        )),
    }
}

fn shadowed(
    root: &std::path::Path,
    setting: crate::config::Setting,
    written: &Config,
    effective: &Config,
) -> Refusal {
    // Only when there is one. This used to fall back to the words "the local
    // override" and blame it anyway, so a row that read back wrong for any
    // other reason was reported as an operator file overriding it, and the
    // resolution sent somebody to edit a file that was not there. Naming a
    // cause the tool cannot see is the same failure as reporting a state it
    // did not read back.
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
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            format!(
                "that row changed or removed in {shadow} — it is the operator's file, and Estigia will not edit it"
            ),
        ),
    }
}

/// The operator's own file overriding rows in this root, when there is one.
///
/// The three refusals below each ended in their own `unwrap_or_else`, naming
/// "the local override" — or "a local override under <root>" — whether or not
/// one was found. So a row that did not take for any other reason was reported
/// as that file's doing, and the resolution sent somebody to edit something
/// that is not there. Three copies of one lookup, each inventing the same
/// answer when it came back empty.
///
/// One place asks now, and it answers `None` rather than a sentence. What to
/// say when there is no file is each refusal's own business, because the three
/// are looking at different things — one row, a table of them, or a row
/// propagated into somebody else's root.
fn overriding_file(root: &std::path::Path) -> Option<String> {
    skill::local_override(root).map(|path| path.display().to_string())
}

/// The refusal for a row that read back wrong with nothing there to explain it.
///
/// [`shadowed`]'s sibling for the case it used to absorb: the table took the
/// value, the row still reads as something else, and no `estigia.local.md` sits
/// beside it. Why is unknown here, so this says what was observed and stops.
/// The alternative was a sentence naming a file, which reads as a diagnosis and
/// sent an operator to edit something that does not exist.
///
/// No command, because none is known to clear it. `config list` is what shows
/// where the answer comes from, so it is named as knowledge rather than as a
/// fix — the rule this crate keeps about naming commands is that running one
/// has to clear the block.
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
        replay: crate::outcome::Replayability::NotReplayable,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "what answers that row instead — `estigia config list --agent <agent>` reports what \
             each one reads",
        ),
    }
}

/// The installed contract to read or write, and the configuration in it.
/// The file `--agent` should write, and the configuration it should start from.
///
/// On an adapter with a skill directory of its own, that is the contract, as
/// before. On one sharing the neutral root it is that adapter's **own** file —
/// otherwise `--agent opencode` would write the table `claude-code` reads, and
/// two agents that were meant to run different models would run the same one
/// under a message saying otherwise.
/// The configuration a write can be checked against, or why it cannot be.
///
/// Three sites read the table back after writing it, for the reason one of them
/// states: "write, then read *both* back, and believe the readback rather than
/// the write". All three then wrote `unwrap_or(config)` — which believes the
/// write, in the single case where the readback had something to say.
///
/// Reaching this means Estigia wrote a table Estigia cannot read: every other
/// route to an unparseable contract is refused before the write, by
/// `writable_config`. That is precisely why it must not be swallowed — this is
/// the only place that would ever notice.
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
        // A table with one bad row is still a table to write into — and only
        // here, where something is being **written**. `contract_of` serves the
        // readers too, and an unreadable contract is a thing they must report:
        // `a_contract_that_will_not_parse_is_not_reported_as_one_that_is_not_
        // there` and its neighbour say so, and putting the rescue there broke
        // both.
        //
        // Measured after tightening `BoardRef::parse`: a `Project board` an
        // older build had accepted made every `config set` on that machine
        // refuse, naming the file and no command — including the one way out,
        // which is setting the row back to `none`.
        let (contract, config) = match contract_of(None, options) {
            Ok(pair) => pair,
            Err(refusal) if refusal.code == "skill-not-installed" => return Err(refusal),
            Err(refusal) => {
                let repaired = configured(options).into_iter().find_map(|adapter| {
                    let paths = setup::resolve_paths(adapter, options).ok()?;
                    let contract = paths.skill_root.join(skill::CONTRACT);
                    contract.is_file().then(|| {
                        let (config, _) = skill::installed_config_in_keeping_what_parses(
                            &paths.skill_root,
                            options
                                .home_dir
                                .as_deref()
                                .unwrap_or_else(|| std::path::Path::new("")),
                        );
                        (contract, config)
                    })
                });
                repaired.ok_or(refusal)?
            }
        };
        return Ok(WritableConfig {
            target: contract,
            config,
            agent_snapshot: None,
        });
    };
    let adapter = find_agent(slug)?;
    // The caller's machine, not a fresh default. This built its own
    // `SetupOptions` and so resolved the **real profile** whatever it was handed:
    // a test driving `config set --agent` through a temporary home wrote into
    // `~/.agents/skills/…` on the developer's own machine, and reported the path
    // it had written while the sandbox beside it stayed empty. The shared branch
    // above was threaded and this one was not, which is the same rule held in
    // two places disagreeing.
    let paths = setup::resolve_paths(adapter, options).map_err(|_| skill::no_skill_root())?;
    if !adapter.discovers_skills() {
        let target = skill::agent_override(&paths.skill_root, adapter.slug);
        let agent_snapshot = setup::agent_configuration_snapshot(&target)?;
        let config = skill::installed_config_for_agent_write(
            &paths.skill_root,
            adapter.slug,
            agent_snapshot.document(),
        )?;
        return Ok(WritableConfig {
            target,
            config,
            agent_snapshot: Some(agent_snapshot),
        });
    }
    // The same tolerance  gained below, for the same reason: a row
    // this build refuses must still be repairable through the tool that wrote
    // it. Strict first, so an intact table is read exactly as before.
    let config = match skill::installed_config_for(&paths.skill_root, Some(adapter.slug)) {
        Ok(config) => config,
        Err(refusal) if refusal.code == "skill-not-installed" => return Err(refusal),
        Err(_) => {
            skill::installed_config_in_keeping_what_parses(
                &paths.skill_root,
                options
                    .home_dir
                    .as_deref()
                    .unwrap_or_else(|| std::path::Path::new("")),
            )
            .0
        }
    };
    Ok(WritableConfig {
        target: paths.skill_root.join(skill::CONTRACT),
        config,
        agent_snapshot: None,
    })
}

struct WritableConfig {
    target: std::path::PathBuf,
    config: Config,
    agent_snapshot: Option<setup::AgentConfigurationSnapshot>,
}

fn contract_of(
    agent: Option<&str>,
    options: &SetupOptions,
) -> Result<(std::path::PathBuf, Config), Refusal> {
    let targets = match agent {
        Some(slug) => vec![find_agent(slug)?],
        // With no agent named the question is *what governs here*, and what
        // governs is the root the gate decides in. This walked `AGENTS` order
        // and answered from the first configured adapter, which is the shared
        // neutral root on almost every machine — so `config list` reported one
        // table while the gate adjudicated another, and `config set` wrote into
        // the one nothing decides by. Both halves were reading a real file;
        // neither was reading the same one.
        //
        // A stable sort, so this only moves the canonical root's owner to the
        // front and leaves the declared order intact behind it — including as
        // the fallback for a machine where no root can be resolved at all.
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
    // A contract that is *there* and does not parse is a different answer from
    // one that is not there, and only the second is "nothing is installed". One
    // bad row in the operator's own `estigia.local.md` made this report `no
    // agent has Estigia installed` and send them to `estigia setup --all` — a
    // command that reinstalls a skill that was never missing and cannot touch
    // their file, which is the ratchet broken: the message named a command that
    // does not discharge the block.
    let mut unreadable: Option<Refusal> = None;
    for adapter in targets {
        let Ok(paths) = setup::resolve_paths(adapter, options) else {
            continue;
        };
        // Narrowed to the adapter, always. Eight of them share one skill root,
        // so the table there is everybody's and an adapter's own answers are in
        // the file beside it. Read without the slug, `config list --agent
        // opencode` reported the shared value while `config set --agent
        // opencode` had just written a different one: a write this tool
        // confirmed and a read this tool contradicted, on the same row, one
        // command apart.
        match skill::installed_config_for(&paths.skill_root, Some(adapter.slug)) {
            Ok(config) => return Ok((paths.skill_root.join(skill::CONTRACT), config)),
            // Absent is the one this loop is allowed to walk past — and only
            // when nobody named an adapter. With no agent named it is trying
            // every configured one and the next may be installed where this is
            // not; with an agent named there is no next, so walking past it
            // fell off the end of the loop and answered `nothing-configured`.
            //
            // Which is false twice over and was measured being false twice
            // over: the agent that was asked for is registered, and another one
            // on the same machine is installed and working. `estigia status`
            // says *configured, contract not understood* and `estigia doctor`
            // says *nothing at …/SKILL.md → run: estigia setup claude-code*,
            // while this said nothing is installed anywhere and sent the
            // operator to `estigia setup --all` — the widest command there is,
            // with the narrow one in hand two lines earlier.
            Err(refusal) if refusal.code == "skill-not-installed" && agent.is_none() => {}
            // Kept rather than raised at once, for the same reason: with no
            // agent named, an adapter that does not parse should not hide one
            // that does. Raised only if nothing readable turns up.
            Err(refusal) => {
                unreadable.get_or_insert(refusal);
            }
        }
    }
    Err(unreadable.unwrap_or_else(nothing_configured))
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
    // Assembled in the library, not here: half of what `doctor` reports used to
    // be stitched together in this function, where no test could reach it.
    let checks = harness::doctor::full(
        skill_root.as_deref(),
        &repo_dir,
        &tracker,
        &SetupOptions::default(),
    );

    if json {
        print_json(&checks);
    } else {
        for check in &checks {
            match &check.health {
                harness::doctor::Health::Fine { detail } => {
                    say!("  ok       {:<10} {detail}", check.name);
                }
                harness::doctor::Health::Skipped { detail } => {
                    say!("  skipped  {:<10} {detail}", check.name);
                }
                harness::doctor::Health::Broken { detail, resolution } => {
                    say!("  BROKEN   {:<10} {detail}", check.name);
                    say!("           {:<10} {resolution}", "");
                }
            }
        }
    }

    // Taken apart rather than asked twice. `is_broken` then `resolution()`
    // hands back an `Option` the type system cannot see is always `Some` here,
    // and the fallback written for that impossible `None` was
    // `Resolution::run("estigia doctor")` — a command that, on a refusal whose
    // whole content is *this doctor check is broken*, prints the same broken
    // check back. That is the dead end the ratchet forbids, sitting in code
    // nothing could reach, and the inventory never asked about it because its
    // entry for this code was reconstructed by hand around one real case.
    // The first broken check **about the present**. `silence` reports on runs
    // that already happened, and turning it into `environment-not-ready` said
    // "a run cannot swear yet" about a machine where one could — permanently,
    // because the ledger keeps its history.
    let broken = harness::doctor::first_blocking(&checks).and_then(|check| match &check.health {
        harness::doctor::Health::Broken { resolution, .. } => Some((check, resolution)),
        _ => None,
    });
    match broken {
        // Nothing broken is not yet the whole answer. A tracker that ships a
        // binding and no executable cannot be claimed against — `claim` refuses
        // with `tracker-has-no-transport` — and a run that cannot swear is a run
        // the gate never adjudicates. So on `linear` and `trello` this printed
        // its rows, said nothing, and exited `0`, while `status` said `gate on`.
        // Every write on that machine goes through unmeasured.
        //
        // Said rather than refused, and the difference is deliberate: choosing
        // one of those trackers is a **choice**, not a fault, and exiting
        // non-zero would report a healthy machine as broken. What was missing is
        // the sentence, not the failure. *A gate with a hole is still a gate; a
        // gate whose hole nobody mentions is a lie.*
        None => {
            if !json && tracker.transport().is_none() {
                say!(
                    "estigia: `{}` has no executable transport, so no run can swear here \u{2014} \
                     the contract is installed and the gate adjudicates nothing",
                    tracker.as_value()
                );
            }
            Ok(())
        }
        // The check's own resolution is carried through, not replaced. A
        // generic one here would tell somebody with no git remote that they
        // need "a git remote", which they already knew, instead of that they
        // can set `Tracker` to `github <owner>/<name>` and skip it.
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

/// Serves MCP over stdio until the client closes it.
///
/// The gate context is resolved once and handed in as a `Result`: a harness
/// that is not installed still serves — `tools/list` answers, and every call
/// comes back with the refusal that names what to run. A server that refuses to
/// start is a server whose absence the agent cannot explain.
fn mcp() -> Result<(), Refusal> {
    let context = gate_context("");
    let stdin = std::io::stdin();
    harness::mcp::serve(stdin.lock(), std::io::stdout(), context).map_err(|error| {
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
///
/// A hook that exits non-zero is, to some agents, a hook that blocks. Estigia
/// failing is not the tracker saying stop, so every path out of here prints JSON
/// and exits zero — the only thing that stops a tool call is a `deny` the gate
/// actually decided.
fn hook(event: &str, dialect: &str, agent: Option<&str>) -> Result<(), Refusal> {
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
    // Git's hooks answer with an exit code, not with JSON. Printing a decision
    // where git expects a status is a refusal that prints itself and lets the
    // push through.
    if harness::hook::is_git(event) {
        return git_hook(event);
    }

    let input = harness::hook::read_input(std::io::stdin());
    let context = gate_context(&input.cwd).ok();
    let named = dialect;
    let dialect = harness::hook::Dialect::from_slug(named);
    // Falling back is the right answer and `from_slug` says why: a settings
    // file naming a dialect this build does not have would otherwise block
    // every edit until somebody worked out which word was wrong. Doing it
    // **without a word** is not the same answer — the same distinction the push
    // guard makes about a binary it cannot find, and the MCP server about a
    // protocol revision it does not speak.
    //
    // It is the expensive silence here. This dialect decides what a refusal
    // *looks* like, and `Dialect::ExitCode` carries the reason in its own doc:
    // a decision printed as JSON where an exit code is expected is a refusal
    // that prints itself and lets the write through. An agent registered under
    // a slug this build has since renamed gets exactly that, and nothing says
    // so — the shape `from_slug`'s own comment records happening to Cline once.
    //
    // Standard error, because standard output is the answer. Only on a slug
    // that is not one, so an ordinary run stays quiet.
    if dialect.slug() != named {
        note!(
            "estigia: `{named}` is not a dialect this build knows, so this answer is written in \
             `{}` — if the agent ignores it, that is why",
            dialect.slug()
        );
    }
    let answer = harness::hook::run_as(dialect, agent, event, &input, context.as_ref());

    // One dialect reads a process status rather than a document. Printing JSON
    // where an exit code is expected is a refusal that prints itself and lets
    // the write through, which is the failure the git hook is written against —
    // and it arrives through an agent here.
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

/// Runs a git hook, whose contract is the exit code.
///
/// A harness that is not installed lets the push through rather than blocking
/// every push in the repository until somebody works out why — the same rule
/// the agent hook follows, at a boundary where being wrong is more expensive in
/// the other direction than a lock would be.
fn git_hook(event: harness::hook::Event) -> Result<(), Refusal> {
    // Not `.`. Every checkout a claim covers is recorded absolute, and the gate
    // decides coverage by comparing them — so an invented `.` matched none of
    // them and the push left as `Decision::Outside`, which is the branch that
    // lets it through. A working directory this process cannot read is not a
    // checkout no claim covers; it is a question nobody asked.
    //
    // Still let through: not blocking every push in a repository is the stated
    // stance at this boundary, ten lines above. Doing it without a word is not
    // the same stance — it is the silence the ledger check exists to find.
    // `estigia guard`, in this same file, already refuses this way rather than
    // inventing a path.
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
            // Two states this boundary cannot tell apart once it starts
            // deciding, and only one of them is normal.
            //
            // *No claim covers this checkout* is the ordinary case the guard's
            // own caveats describe: the oath binds once sworn, so a push before
            // anybody claims goes through. *There is no skill installed* is not
            // ordinary — the hook can never decide anything, ever, and it will
            // go on saying nothing about every push in the repository.
            //
            // Measured: `estigia guard` in a home where nothing is installed
            // writes the hook and reports `wrote …/pre-push`; every push then
            // left with exit `0`, nothing on either stream and no ledger line,
            // and `doctor` printed no push-guard row at all because it stops at
            // the missing skill. Three surfaces, no signal, on a repository
            // whose owner had just installed a guard.
            //
            // Still let through — not blocking every push is the stated stance
            // at this boundary, twenty lines above. Doing it without a word is
            // not the same stance.
            let adjudication = harness::guard::adjudicate_action(
                &context,
                &repo_dir,
                &harness::Action::Boundary {
                    command: "git push".to_owned(),
                    pr: None,
                    local_fast_forward_target: None,
                },
                harness::Sensitivity::Boundary,
            );
            let harness::guard::Adjudication { decision, holder } = adjudication;
            // And when it decided nothing, whether it *could* have.
            //
            // Asked after the decision, not before: the guard works off the run
            // pointer and not off the contract, so a machine can hold a claim
            // with no skill directory beside it — `a_push_the_guard_decided_on_
            // is_one_the_ledger_holds` builds exactly that, and asking first
            // silenced a guard that was deciding.
            if matches!(decision, harness::Decision::Outside(_)) {
                // Two reasons a guard can never decide, and neither is *nobody
                // has claimed yet*. The first is no skill at all; the second is
                // a tracker with no executable transport, where `claim` refuses
                // and no run on this machine can ever hold an issue.
                if let Err(refusal) = crate::skill::installed_config(&context.skill_root) {
                    note!(
                        "estigia: this push was not checked against any claim \u{2014} {}",
                        refusal.message
                    );
                    note!("  run: estigia setup --all");
                    return Ok(());
                }
                if let Some(tracker) = inert_tracker() {
                    note!(
                        "estigia: this push was not checked against any claim \u{2014} `{tracker}` \
                         has no executable transport, so nothing can swear here"
                    );
                    note!("  run: estigia config set Tracker github");
                    return Ok(());
                }
            }
            // Written down through the same function the tool path uses, so a
            // push sits in the ledger in the shape everything else does. This
            // arm decided and recorded nothing, at the boundary the honesty
            // contract calls the unconditional one: nobody could say afterwards
            // which pushes had been adjudicated, and `doctor`'s silence row —
            // which reads that same file — answered *no call has reached the
            // gate yet* on a machine where every push had been decided.
            //
            // The run id is the holder's, because a push is found by its
            // checkout rather than by a session. `note` returns without writing
            // for `Outside`, which is why asking for a holder here is safe: when
            // there is nothing to record there was no holder either.
            let run_id = holder.unwrap_or_default();
            harness::hook::note(
                &context,
                &run_id,
                "git push",
                // The same word twice, and deliberately: this door has no tool
                // name to pass, so it has always put the command where a tool
                // goes. Now that an allow records its subject, a push through
                // git and a push through an agent's shell tool leave lines that
                // say the same thing about the same step.
                Some("git push"),
                &decision,
                &refs_being_pushed(),
            );
            match decision {
                harness::Decision::Outside(_) => Ok(()),
                harness::Decision::Allow(reason) => {
                    say!("estigia: {reason}");
                    Ok(())
                }
                harness::Decision::Deny(refusal) => Err(*refusal),
            }
        }
        _ => Ok(()),
    }
}

/// Installs or removes this repository's push guard.
/// Installs or removes the push guard, and says which of the ten things it did.
///
/// **The flag reaches here now.** `--json` is declared once on the root parser
/// and threaded through every other command; this one never took the parameter,
/// so all ten of its answers printed prose under it. The test that exists to
/// catch exactly that named `guard` and could not see it: it ran the command
/// from a home directory, which is not a checkout, so the refusal went to
/// standard error, standard output came back empty, and an empty answer counts
/// as *said nothing, which is allowed*.
///
/// The ten answers are built rather than printed, in one place. They were ten
/// `say!` calls in two arms two screens apart, which is how the install arm came
/// to answer `wrote <path>` about somebody else's hook while the uninstall arm,
/// looking at the same state, got it right.
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
            // A plan, in the tense of one. Saying `removed` here was a command
            // that had touched nothing reporting that it had.
            harness::guard::Removal::WouldBeTaken => (
                "would-be-removed",
                format!("{shown} would be removed"),
                false,
            ),
            // And the third thing `Absent` used to mean. An operator running
            // this twice, or in a repository that never had the guard, was told
            // a file had been taken off their machine.
            harness::guard::Removal::NothingThere => (
                "nothing-there",
                format!("{shown} is not there \u{2014} nothing to remove"),
                false,
            ),
            // Chained is somebody else's file too, and the same answer: the
            // line calling Estigia is inside a script Estigia did not write, so
            // taking it out means editing their hook.
            harness::guard::Removal::LeftAlone(
                harness::guard::State::Chained | harness::guard::State::Foreign,
            ) => (
                "left-alone-theirs",
                format!("{shown} was written by somebody else and is left alone"),
                false,
            ),
            // Not ours to remove either, and for a stronger reason than theirs:
            // nothing read it, so nothing can say it was ever ours.
            harness::guard::Removal::LeftAlone(harness::guard::State::Unreadable) => (
                "left-alone-unreadable",
                format!("{shown} cannot be read and is left alone \u{2014} nothing was removed"),
                false,
            ),
            // Estigia's own hook, left where it is because somebody added to
            // it. The reason **is** the message: *still there* on its own reads
            // as a failure, and this is a refusal to take away work that is not
            // Estigia's — the same rule that stops the install overwriting a
            // hook it did not write, applied from the other end.
            //
            // No command is named, because none clears it. Deleting the file is
            // the operator's call and they can see it; naming a dead end is
            // worse than naming nothing.
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
            // `NothingThere` is the arm for an empty repository, so this is the
            // file disappearing between the read and the removal. Nothing was
            // taken out by this run, and saying so beats claiming it was.
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
            // Their hook, already handing the push to Estigia. Nothing was
            // written, and saying `wrote <path>` about a file this run did not
            // touch is the one thing this tool refuses — it fell into the
            // catch-all below, two screens from the `--uninstall` arm that names
            // this state and gets it right. The gate still runs, so what it does
            // is worth saying; what is not true is that anything was installed.
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

    if json {
        print_json(&serde_json::json!({
            "action": if uninstall { "uninstall" } else { "install" },
            "outcome": outcome,
            "path": shown,
            "dry_run": dry_run,
            "message": prose,
            // Under the machine's answer as well as the person's. "every push
            // is now checked" is what a caller concludes otherwise, and it is
            // not what having the hook means.
            "caveats": if caveats { GUARD_CAVEATS } else { &[] as &[&str] },
        }));
        return Ok(());
    }
    for line in prose.lines() {
        say!("{line}");
    }
    if caveats {
        for caveat in GUARD_CAVEATS {
            say!("  {caveat}");
        }
        // And whether any of that can happen here.
        //
        // `Tracker` accepts `linear` and `trello`, and both ship a binding the
        // agent reads and nothing that answers, so `claim` refuses with
        // `tracker-has-no-transport` and no run on this machine can ever hold
        // an issue. The caveats above then promise a refusal that cannot
        // happen — and name `estigia claim` as what starts it, which is a
        // command that cannot succeed under this tracker.
        //
        // The hook is still written: a tracker is a row somebody can change,
        // and the guard is already there when they do.
        if let Some(tracker) = inert_tracker() {
            say!(
                "  `{tracker}` has no executable transport, so nothing can swear here and this \
                 guard adjudicates nothing until `estigia config set Tracker github`."
            );
        }
    }
    Ok(())
}

/// The configured tracker, when it is one no run can swear against.
///
/// `None` when the tracker has a transport, when nothing is configured, or when
/// the configuration cannot be read — this exists to add a sentence, and a
/// sentence is not worth refusing a guard over.
///
/// **Asked of `Tracker::transport`, not of a list here.** The first cut of this
/// wrote `["linear", "trello"]` — a second copy of the one rule that says which
/// trackers can be operated, in a crate whose own contributing note is *prefer
/// removing a copy to adding a check*. A tracker added with no transport would
/// have been missing from it, and the guard would have gone on promising
/// refusals that cannot happen for exactly the tracker nobody had thought
/// about. `every_reader_of_a_transportless_tracker_asks_the_same_function`
/// holds it.
fn inert_tracker() -> Option<String> {
    let skill_root = harness::control_surface();
    let config = skill::installed_config(&skill_root).ok()?;
    config
        .tracker
        .transport()
        .is_none()
        .then(|| crate::config::Setting::Tracker.value_of(&config))
}

/// What the push guard actually does, which is narrower than having one.
///
/// Said wherever the guard is reported as running, because "every push is now
/// checked" is what an operator hears otherwise. A checkout no run holds is
/// `Decision::Outside` — the oath binds once sworn — so until somebody claims,
/// this hook reads the state, finds no holder, and lets the push through.
/// Leaving that out has somebody believing a repository is covered on exactly
/// the days nobody has claimed anything.
const GUARD_CAVEATS: &[&str] = &[
    "a push from a checkout a live claim holds is refused unless that claim",
    "justifies it \u{2014} whoever types it, agent or person.",
    "a checkout no run has claimed is outside the gate: the oath binds once",
    "sworn, so this refuses nothing until `estigia claim` has been run.",
    "`git push --no-verify` bypasses it \u{2014} a guard rail working as one.",
];

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
    // The configured tracker, read from the contract the agent reads. Defaults
    // rather than refusing: a contract that cannot be parsed is a problem
    // `doctor` reports, not one that should stop a hook mid-edit.
    // Defaults, yes — but defaults are not neutral. `Config::default` carries
    // no declared boundaries and the full renewal window, so falling back to it
    // *loosens*: the operator's one-way doors stop being one-way doors, and a
    // window they narrowed goes back to its widest. That is the declared
    // asymmetry run backwards by a parse error, silently.
    //
    // What can be taken back is taken back below: with no readable contract
    // nothing rides a cached answer. What cannot is the phase check on a
    // declared boundary — that needs the list, and the list is what went
    // missing. `doctor` names the row; this is the cost of not fixing it.
    // With what this repository says about itself on top. Absent is the
    // ordinary case and reads exactly as it did before the layer existed, so a
    // checkout that has never been configured is unaffected.
    let (installed, complaint) =
        skill::installed_config_in_keeping_what_parses(&skill_root, &repo_dir);
    let unreadable = complaint.is_some();
    let tracker = installed.tracker;
    let boundaries = installed.boundaries;
    // Read here rather than at each decision: a record that cannot be parsed is
    // **not** a stand-down. Treating an unreadable one as in force would let a
    // corrupt file open the gate, which is the exact shape of the failure the
    // unreadable run pointer already refuses.
    let state_root = harness::session::state_root(None)?;
    // Through `standing`, which tells the two apart, and then deliberately
    // discarding the unreadable one here: the decision is the same as before and
    // is now written rather than implied by a pair of `.ok()`s. `doctor` reports
    // the file, because ignoring one is a thing somebody has to be told.
    let stand_down = match harness::standdown::standing(&state_root) {
        harness::standdown::Standing::Declared(record) => Some(record),
        harness::standdown::Standing::Away | harness::standdown::Standing::Unreadable(_) => None,
    };

    Ok(harness::GateContext {
        integration: installed.integration,
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
        // The operator's, when they asked for a shorter one. The parser refuses
        // anything longer, so this can only ever narrow what rides on a cached
        // answer — and a contract nothing can read narrows it to nothing. The
        // window is permission to skip asking the tracker, granted by a document
        // that could not be read: the one thing a fault must not hand out.
        window: if unreadable {
            std::time::Duration::ZERO
        } else {
            installed.window.min(harness::hook::default_window())
        },
        tracker,
        boundaries,
    })
}

/// Claims an issue, and binds this run's writes to it.
///
/// A thin front door onto the same operation the MCP tool runs. Two
/// implementations of argument assembly, idempotency keys and pointer
/// bookkeeping would eventually answer differently, and the one a person typed
/// would be the one nobody noticed had drifted.
fn claim(issue: &str, run_id: &str, horizon: &str, state: &str, json: bool) -> Result<(), Refusal> {
    let number: u64 = issue
        .trim()
        .parse()
        .map_err(|_| harness::issue_not_a_number(issue))?;
    let result = tool(
        "claim",
        &serde_json::json!({
            "issue": number,
            "run_id": run_id,
            "horizon": horizon,
            "state": state,
        }),
    )?;
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
    say!("{result}");
    say!("sworn: {run_id} holds #{number} in {state}");
    // Narrowed for the same reason the push guard's line was. This used to
    // claim the gate measures *all* of a run's repository writes; it sees one
    // made through a tool its matcher covers, in an agent whose calls Estigia
    // can gate, and `status` reports eight of the eleven adapters as contract
    // only. The claim was false for most installations, in the message somebody
    // reads at the moment they swear.
    say!("  a repository write the gate sees is now measured against that claim, and");
    say!("  every irreversible boundary re-reads the timeline.");
    say!("  writes from an agent Estigia does not gate are not seen — `estigia status`");
    say!("  says which agents it holds the tools for, and the push guard holds the push");
    say!("  whoever types it.");
    Ok(())
}

/// Puts the issue down and forgets the run.
fn release(run_id: &str, json: bool) -> Result<(), Refusal> {
    let context = gate_context("")?;
    let run = harness::session::load(&context.state_root, run_id);
    // Present and unreadable is not the same as absent, and `session::load`
    // takes care to keep them apart — "an unknown is not clearance" is the rule
    // it is written against. This read them back as one: a pointer this run
    // wrote and Estigia can no longer parse was reported as `{run_id} holds no
    // issue`, which is a statement of fact about a thing Estigia does not know.
    // The tracker may still show the issue held by this run, and an operator
    // told there was nothing to put down would leave it there.
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
    let Some(issue) = run.issue else {
        return Err(Refusal::not_started(
            "nothing-held",
            format!("{run_id} holds no issue"),
            Resolution::run("estigia status"),
        ));
    };
    // Both phases, because the caller asked for one thing. `unassign` is a
    // discovery call first: it answers `write_performed: false` with the epoch
    // it found and *"repeat unassign with --target-operation and the same
    // operation ID"*, and this verb threw that answer away and printed
    // `released: <run> no longer holds #<issue>`.
    //
    // Measured on the installed binary: after that sentence the pointer still
    // held the issue — correctly, the harness reads `write_performed` and keeps
    // it — and the very next write went through on the renewal window with
    // `allow — issue #12 was verified inside the renewal window`. The gate was
    // right and the message was wrong, which is the third rule broken in the
    // one command whose whole job is putting a claim down.
    //
    // The split exists so the *caller* names what the discovery found, and here
    // the caller is a person who typed `estigia release`. Naming it back is
    // this function's to do; deciding whether to release is not.
    let answer = tool(
        "release",
        &serde_json::json!({ "issue": issue, "run_id": run_id }),
    )?;
    let found: serde_json::Value = serde_json::from_str(&answer).unwrap_or_default();
    if found.get("write_performed") == Some(&serde_json::json!(false)) {
        let Some(target) = found
            .get("target_operation")
            .and_then(|value| value.as_str())
        else {
            // Discovery that found nothing to name is not a release, and saying
            // so is the whole of this fix: a state nobody reached must never be
            // reported as reached.
            return Err(Refusal::not_started(
                "release-not-performed",
                format!(
                    "{run_id} still holds #{issue}: the release read the timeline and found no \
                     epoch of its own to put down"
                ),
                Resolution::run("estigia status"),
            ));
        };
        tool(
            "release",
            &serde_json::json!({
                "issue": issue,
                "run_id": run_id,
                "target_operation": target,
            }),
        )?;
    }
    if json {
        print_json(&serde_json::json!({
            "released": true,
            "issue": issue,
            "run_id": run_id,
        }));
        return Ok(());
    }
    say!("released: {run_id} no longer holds #{issue}");
    Ok(())
}

/// Runs one workflow tool and returns its text, or the refusal it carried.
///
/// The refusal is passed through **whole**. It was built with the transport's
/// own answer about what happened to the world and whether a replay is safe,
/// and re-wrapping it would fabricate both — which is how `estigia claim --json`
/// came to report `not_started` for a write whose fate nobody knows.
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

/// Prints what the gate would decide, using the same code path as the hook.
/// The checkout a gate payload names, if it names one.
///
/// Read at the top level and one level in, because both shapes arrive: an agent
/// that sends the whole hook envelope puts it beside `tool_input`, and one that
/// sends the tool's own arguments puts it among them. `hook::normalise` already
/// lifts the second for the door it serves; this is the other door.
/// Which sub-agent a gate payload names, if it names one.
///
/// The caller spellings `hook::Input` already accepts, read here
/// because this door parses the payload itself rather than through that type.
/// Only top-level: nested `tool_input.subagent_type` is a launch target, not the
/// role making this call.
fn payload_agent(parsed: &serde_json::Value) -> Option<&str> {
    ["agent_type", "agent_name"]
        .into_iter()
        .find_map(|key| parsed.get(key).and_then(serde_json::Value::as_str))
        .filter(|agent| !agent.trim().is_empty())
}

/// The directory this process was launched in.
///
/// Two callers, and the difference between them is the whole of the trust
/// model below: it is the fallback when nothing names a checkout, and it is the
/// ceiling a directory named by a *tool call* may not escape.
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

/// The checkout the **host** named, or nothing.
///
/// The adapter's own hook writes this: it lifts `cwd` out of a payload that
/// nests it, and it is authoritative, because the adapter knows which checkout
/// it is gating and the model does not compose it. It is taken as given, and a
/// path outside this process's own directory is the ordinary case rather than a
/// suspicious one — the hook may run from anywhere.
///
/// Deliberately **not** where OpenCode's `workdir` is read. See
/// [`narrowed_by_the_call`] for why that one cannot be treated as this one is.
fn payload_cwd(parsed: &serde_json::Value) -> &str {
    parsed
        .get("cwd")
        .or_else(|| parsed.get("tool_input").and_then(|inner| inner.get("cwd")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

/// The directory the **call itself** named, clamped to somewhere the gate was
/// already going to look.
///
/// OpenCode's plugin launches this process from the project directory, because
/// its plugin context carries a project and no session identity to mint a run id
/// from. With two runs each holding an isolated worktree inside one base
/// checkout, that directory *is* the base, and both cover it at equal depth.
/// Measured on 2026-08-16 with two live holders of this repository: a
/// `git commit` explicitly targeting one worktree came back *"2 runs on this
/// machine hold this checkout"* and advised releasing one of them — which is the
/// concurrent isolation both runs were using. `holders_of` was right about the
/// directory it was given; the directory was the wrong one. The Bash call's own
/// `workdir` argument is the only evidence it carries about where the command
/// will actually run.
///
/// **It may only narrow, and that is the whole difference from `cwd`.** This is
/// a tool *argument*: whatever composed the call wrote it, which on every
/// runtime here means a model wrote it. Read as freely as a host's `cwd`, it
/// stops being evidence and becomes a lever — measured, before the clamp
/// existed, with two live pointers and a `git commit` under a claim:
/// `workdir` of `..`, of the parent checkout, of `C:\Windows`, all resolved,
/// were covered by no run, and were answered `outside` with exit **zero**. The
/// command still ran where it was going to run; the gate simply stopped
/// adjudicating it. That is a payload steering a write out of the claim, and it
/// is strictly worse than the false ambiguity being fixed — a widened gate that
/// looks exactly like working correctly.
///
/// So the answer is `None` unless the resolved directory lies **inside** the one
/// this process was launched in, and `None` puts the decision back where it was
/// before this key was read at all.
///
/// A relative value is resolved against that same launch directory first. That
/// is *close to* the host's own resolution and not provably identical to it:
/// OpenCode resolves `workdir` against its tool context's `directory`, while the
/// plugin launches this process in `worktree ?? directory`. Those are two fields
/// of one record and they coincide in the ordinary case, so the narrowing is
/// right whenever it matters and the clamp above holds either way — both
/// candidates lie inside the project. `docs/honesty.md` carries the case where
/// they diverge, which is a wrong holder rather than an escaped one.
///
/// Bash only, because Bash is the only tool measured to carry the key and the
/// only one the issue this closes scopes. Every other gated tool sends no
/// working directory, so honouring the key on them would be inventing evidence
/// out of an argument nothing documents — and it is exactly how the escape above
/// reached `write` and `edit` as well.
fn narrowed_by_the_call(
    parsed: &serde_json::Value,
    tool: &str,
    launched: &std::path::Path,
) -> Option<std::path::PathBuf> {
    if !tool.eq_ignore_ascii_case("bash") {
        return None;
    }
    let named = parsed
        .get("workdir")
        .or_else(|| {
            parsed
                .get("tool_input")
                .and_then(|inner| inner.get("workdir"))
        })
        .and_then(serde_json::Value::as_str)
        .filter(|named| !named.trim().is_empty())?;
    let named = std::path::PathBuf::from(named);
    let resolved = if named.is_absolute() {
        named
    } else {
        launched.join(named)
    };
    // Placed before compared, and the difference is the whole clamp.
    //
    // `covers` was written for **working directories, which exist**, and resolves
    // an unresolvable path literally — so `wt-a/../../nope` still *starts with*
    // the launch directory, `..` never cancelled, and the comparison answers
    // *inside* for a path that is not. Measured: that spelling was not merely
    // let past, it was attributed to whichever worktree the lexical prefix
    // happened to name and **allowed**, so a run holding one worktree could
    // borrow the other's claim by writing one `..`. Strictly worse than the
    // escape it replaced, which at least reached `outside`.
    //
    // `placed` is the primitive for a path a caller wrote rather than one the
    // filesystem already has: it collapses the spelling the way this platform
    // collapses it, then resolves as much as exists. Its own doc names this
    // failure. `None` from it means the path cannot be placed at all, and its
    // contract says read that as inside — which here is the launch directory,
    // the answer the caller falls back to.
    crate::paths::placed(&resolved).filter(|placed| crate::paths::covers(launched, placed))
}

fn show_gate(tool: &str, input: &str, run_id: Option<&str>, json: bool) -> Result<(), Refusal> {
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
    //
    // This was said to be **not** for OpenCode, on the grounds that its plugin
    // sends `output?.args` — the tool's own arguments and nothing else — and
    // sets the working directory on the process instead, so the fallback was
    // already right for it. The first half is true and the conclusion was not:
    // for a Bash call those arguments *include* `workdir`, the directory the
    // command will run in, while the process directory is the project root. The
    // two differ exactly when it matters — concurrent runs in isolated
    // worktrees under one base — and that is the case the fallback got wrong.
    //
    // Two keys, two levels of trust, and the order says which is which. What the
    // host named is taken as given. Only when it named nothing does the call's
    // own `workdir` get a say, and then only to point somewhere inside the
    // directory this process was already standing in — never to move the
    // decision elsewhere. `narrowed_by_the_call` carries the measurement.
    let stated = payload_cwd(&parsed);
    let context = if stated.trim().is_empty() {
        let launched = launch_directory()?;
        let looking_at = narrowed_by_the_call(&parsed, tool, &launched).unwrap_or(launched);
        gate_context(&looking_at.to_string_lossy())?
    } else {
        gate_context(stated)?
    };
    // Before anything else, in the order the hook asks it: a sub-agent reaching
    // past the tool list its own definition declares. It is the cheapest
    // question and the least conditional — no claim, no state, no window.
    //
    // It was asked in the hook and nowhere else, and this is a public verb any
    // adapter or script may call.
    //
    // Its reach is narrower than the note added with it claimed, and the
    // measurement is worth keeping: it named OpenCode, and OpenCode cannot
    // reach this. That plugin sends the tool's arguments and no agent name, and
    // its own documented limit is that `tool.execute.before` never sees a call
    // made by a subagent at all — so there is no name to send. What this closes
    // is the door for a caller that names one; nothing in this tree does yet.
    if let Some(agent) = payload_agent(&parsed) {
        if agent == "review-blind" {
            if let Some(refusal) =
                harness::roles::gate(Some(agent), tool, Some(crate::skill::REVIEW_AGENT.contents))
            {
                return report_gate(tool, json, harness::Decision::Deny(Box::new(refusal)));
            }
        } else {
            match harness::roles::definition_for(
                &context.repo_dir,
                crate::paths::home_dir().ok().as_deref(),
                agent,
            ) {
                // Denied rather than stepped over, as the hook does it: a
                // definition that is there and will not open is an unknown, and an
                // unknown is not clearance.
                //
                // Through the stand-down, both of them, because the hook puts its
                // own role denial through it. Returning the refusal straight made
                // this the one refusal an operator's stand-down did not reach —
                // through the agent gated by a plugin, and in the change that added
                // the question here in the first place.
                Err(refusal) => {
                    return report_gate(
                        tool,
                        json,
                        harness::standdown::over(
                            harness::Decision::Deny(Box::new(refusal)),
                            context.stand_down.as_ref(),
                            harness::session::now_seconds(),
                        ),
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
                        );
                    }
                }
                Ok(None) => {}
            }
        }
    }
    let (action, how) = harness::classify_with(tool, &parsed, &context.boundaries);
    let (decision, recorded) = match run_id {
        Some(run_id) => {
            let mut run = harness::session::load(&context.state_root, run_id);
            let decided = harness::gate(&context, &mut run, &action, how);
            if matches!(decided, harness::Decision::Allow(_)) {
                // What the hook does on the same branch, and this door did not:
                // the renewal window is carried in the pointer, so a decision
                // that is never written down is a window that never applies.
                //
                // Which branch matters, and it is not the one it first looked
                // like. The arm below — no run named, ask the checkout — has
                // stored it since `decide_action` learned to, and that note
                // records why: OpenCode's plugin lands *there*. This is the arm
                // a caller reaches by **naming** a run, and nothing wrote the
                // answer down on it.
                //
                // Best effort, as there: failing to record when we last asked
                // costs one extra read and must never become a denial.
                let _ = harness::session::store(&context.state_root, &run);
            }
            (decided, run_id.to_owned())
        }
        // No session to ask with, so ask the checkout instead.
        None => {
            let adjudication =
                harness::guard::adjudicate_action(&context, &context.repo_dir, &action, how);
            (
                adjudication.decision,
                adjudication.holder.unwrap_or_default(),
            )
        }
    };
    // Recorded, because for one adapter this **is** the gate: OpenCode's plugin
    // shells out to exactly this command, so a decision that never reaches the
    // ledger is a decision `doctor`'s silence check cannot see. It is the one
    // check about whether calls went undecided, and it was blind to the one
    // adapter gated by a plugin.
    //
    // Under the run this decision was made for, or under the checkout's holder
    // when no run was named — the same identity the decision itself used.
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
    // `--json` is global and every other command honours it. This one did not,
    // and it is the command a program calls: the OpenCode plugin shells out to
    // `estigia gate <tool> --input <json>` on every edit. A machine reading
    // prose has to parse a sentence to find out whether it may write.
    report_gate(tool, json, decision)
}

/// One decision, reported the way this door reports every decision.
///
/// Its own function because the role check above ends the same way and must end
/// it the same: `--json` is honoured for one refusal and not another only if
/// two places render.
fn report_gate(tool: &str, json: bool, decision: harness::Decision) -> Result<(), Refusal> {
    match decision {
        harness::Decision::Outside(aside) => {
            // The reason comes from the decision, not from this line. Inventing
            // it here is what made all four read as "the tool is not covered".
            let why = aside.why(tool);
            if json {
                print_json(&serde_json::json!({
                    "decision": "outside",
                    "tool": tool,
                    "aside": aside.code(),
                    "reason": why,
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
                }));
            } else {
                say!("allow — {reason}");
            }
            Ok(())
        }
        harness::Decision::Deny(refusal) => Err(*refusal),
    }
}

fn nothing_configured() -> Refusal {
    Refusal::not_started(
        "nothing-configured",
        "no agent has Estigia installed",
        Resolution::run("estigia setup --all"),
    )
}

/// A write that failed part-way is the one case where the outcome is unknown.
///
/// Setup writes several files, and an error on the third says nothing about the
/// first two. Reporting `NotStarted` here would be the lie the taxonomy exists
/// to prevent — so the outcome stays `Unknown` and the caller is still told to
/// read the state before retrying.
///
/// **What it may not do is name a command that answers with the question.** It
/// named `estigia status`, and running it says *configured, skill out of date*
/// — which is what sent the operator to `sync` in the first place, and
/// `doctor` closes the loop by naming `sync` again. Measured on a read-only
/// payload file: `sync` → `status` → `sync`, three commands, none of them
/// saying that a file could not be written, with the path and the operating
/// system's own words already in this refusal's first line.
///
/// So it asks for the thing only a person can supply, the way the arm above it
/// does for a settings file that is not a JSON object. `sync` is named inside
/// that sentence rather than as the resolution, because it is the command that
/// discharges this **once the obstacle is gone** and not before — the same
/// shape `CompanionState::Unpublished` uses.
fn write_failed(adapter: &AgentAdapter, error: &anyhow::Error) -> Refusal {
    Refusal {
        code: "setup-write-failed",
        message: format!("{}: {error}", adapter.display_name),
        outcome: crate::outcome::MutationOutcome::Unknown,
        replay: crate::outcome::Replayability::StatusRequired,
        resolution: Resolution::no_command(
            crate::outcome::NoCommandReason::OperatorKnowledge,
            "write access to the file named above \u{2014} it is read-only, held open, or on a \
             filesystem that refused; free it and run `estigia sync` again",
        ),
    }
}

/// Where one agent stands, in the words `status` prints beside its name.
///
/// `touched` is the fourth question, and it is the one that was missing. Eight
/// adapters have no skill directory of their own and share the neutral root, so
/// configuring one of them made the other seven read as half installed —
/// `estigia setup agents` on a clean machine reported seven faults it had not
/// caused. The remedy was to call a sharer with no directive "not configured",
/// which is right for the seven and wrong for the eighth: an agent installed
/// with `--skill-only` has the skill, the gate and the MCP server and no
/// directive, and it read exactly like one Estigia had never touched. Gate on
/// beneath, and `not configured` above it.
///
/// Two states, one label, and the label is the part somebody reads — the same
/// fault the report's `kept` had for three different things.
fn standing(
    configured: bool,
    presence: skill::Presence,
    shared: bool,
    touched: bool,
) -> &'static str {
    match (configured, presence) {
        (true, skill::Presence::Current) => "configured",
        (true, skill::Presence::Stale) => "configured, skill out of date",
        // Not "out of date": `sync` refuses this contract rather than replacing
        // it, so naming that state would name a command that changes nothing.
        (true, skill::Presence::Unreadable) => "configured, contract not understood",
        (true, skill::Presence::Absent) => "configured, skill missing",
        (false, skill::Presence::Absent) => "not configured",
        // Untouched, and reading somebody else's skill root: nothing here is
        // this agent's, and nothing was half done to it.
        (false, _) if shared && !touched => "not configured",
        (false, _) => "skill present, directive missing",
    }
}

/// The word a row gets, or `None` for a file this run left alone.
///
/// One word each, because they are one sentence each. Three of them shared
/// `kept`, and `kept` is a **fact**: "the file was there before Estigia, so
/// uninstalling leaves it", which only the install record can establish.
///
/// - `shared` is a different fact — the file is Estigia's and stays because
///   another configured agent still reads it. Eight adapters share one skill
///   root, and the skill goes out with the last of them.
/// - `unknown` is the absence of a fact. There is no record, so nothing here is
///   shown to be Estigia's to remove — including files Estigia wrote. Printed
///   as `kept`, an operator whose record was deleted read fourteen lines
///   claiming their files predate an install that in fact wrote every one of
///   them. That sentence is written out in [`Change::Unrecorded`]'s own doc as
///   the reason the two are separate variants; the renderer put them back
///   together at the last step.
///
/// The same argument [`Change::Replace`] was split out on, and the same remedy:
/// its own word, in the column somebody reads.
fn word(change: Change, planned: bool) -> Option<&'static str> {
    if planned {
        // A plan says what it *would* do. The header above already carries the
        // tense — `would change 18 file(s)` against `changed 18 file(s)` — and
        // the lines under it were byte-identical between a plan and a run, so a
        // line read on its own, grepped, or pasted into an issue could not be
        // told apart from one that had happened.
        //
        // This module states the rule two hundred lines up, having applied it
        // to the note about kept files and not to these: *a plan reporting that
        // files "were left there" is a plan claiming to have done something, and
        // an operator reading it has no way to tell it apart from the run that
        // did*. `--dry-run` is the one command whose entire job is to be
        // believed before anything happens.
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
///
/// Most of them really are unknown: a write that may or may not have landed, and
/// `estigia status` is the command that says which. A file Estigia could not
/// read is not one of those. Nothing was written, the operator owns the file,
/// and sending them to `status` names a command that reports the same thing
/// again — a dead end, which is the one thing the ratchet forbids.
///
/// The taxonomy already had the distinction. This is the path that flattened it.
fn setup_failed(adapter: &AgentAdapter, error: &anyhow::Error) -> Refusal {
    // A refusal that already classified itself travels through with its own
    // answers, and only the adapter's name is added. Re-wrapping one as
    // `setup-write-failed` reports `Unknown` for a run that wrote nothing —
    // `config_for` refuses before `setup_into` is called — and sends the
    // operator to `status` over a bad row in a file `status` cannot fix. That is
    // the flattening the note above describes, arriving from a third direction.
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

#[cfg(test)]
mod tests;

/// The refs a push is about to write, as git names them on the hook's input.
///
/// git hands a `pre-push` hook one line per ref —
/// `<local ref> <local sha> <remote ref> <remote sha>` — and this arm read none
/// of them. Nothing adjudicates a destination, and the honesty contract records
/// that; what this changes is that the ledger can say **which** push was
/// decided on rather than only that one was.
///
/// Read only when standard input is not a terminal. A hook is always given a
/// pipe, and a person running this by hand to see what it does is not: blocking
/// there would be a command that hangs, which at this boundary is worse than a
/// record that is missing. *A hook that breaks does not deny* — nor does it
/// wait.
fn refs_being_pushed() -> Vec<String> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        return Vec::new();
    }
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        return Vec::new();
    }
    text.lines()
        .filter_map(|line| line.split_whitespace().nth(2).map(ToOwned::to_owned))
        .collect()
}

/// Why an agent's tool server would not answer, when it would not.
///
/// The two shapes `doctor`'s own row separates, asked with the same readers so
/// the two cannot drift: the entry names a binary that is **not there**, or it
/// names one and never says `mcp` — where the host runs it with no subcommand,
/// gets the usage and exit `2`, and every operation the agent asks for fails.
///
/// `None` when the server is fine or when there is none registered at all;
/// `status` already says `off` for the second.
fn tools_fault(adapter: &'static AgentAdapter, options: &SetupOptions) -> Option<String> {
    let named = setup::tools_command(adapter, options)?;
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
///
/// Three answers, not two, for the reason `standdown::Standing` exists: a file
/// that is there and will not open is not the absence of one, and the gate
/// treats it as absent — so saying nothing about it here would be the silence
/// that arm was written to break, in the command people read most.
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
            // A record that is there and cannot be timed, and one that is over.
            // The first is worth a line; the second is the ordinary state of a
            // machine that used one last week, and `in_force` keeps it as
            // evidence on purpose.
            Some(_) => None,
            None => Some(
                "a stand-down is recorded and this machine's clock cannot be read, so whether it \
                 still applies is unknown — the gate does not honour one it cannot time"
                    .to_owned(),
            ),
        },
    }
}
