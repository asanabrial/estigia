use super::*;
use crate::test_env::{
    with_config_home, with_home_env, with_home_env_and_claude_config_dir,
    with_home_env_and_dsh_home, with_home_env_and_pi_agent_dir, with_process_env,
};

fn sandbox() -> (tempfile::TempDir, SetupOptions) {
    crate::paths::set_fsync_skip_for_tests(true);
    let home = tempfile::tempdir().expect("a temporary home");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join(".config")),
        app_data: Some(home.path().join("AppData").join("Roaming")),
        platform: Some(Platform::Unix),
        ..SetupOptions::default()
    };
    (home, options)
}

fn node_safe_dir(dir: &Path) -> PathBuf {
    crate::paths::remove_windows_verbatim_prefix(
        dir.canonicalize().expect("the directory resolves"),
    )
}

fn agent(slug: &str) -> &'static AgentAdapter {
    find_agent(slug).expect("a declared agent")
}

/// Set up every adapter into one home. Adapters with different skill roots run
/// in parallel; those that share a root install their skill files one after
/// another, because those files are the same path.
fn setup_fleet(options: &SetupOptions, config: &Config) {
    fleet_each(options, config, true);
}

/// Take every adapter back out of one home, same grouping as [`setup_fleet`].
fn uninstall_fleet(options: &SetupOptions) {
    fleet_each(options, &Config::default(), false);
}

fn fleet_groups() -> Vec<Vec<&'static AgentAdapter>> {
    let mut groups: std::collections::BTreeMap<u8, Vec<&'static AgentAdapter>> =
        std::collections::BTreeMap::new();
    for adapter in AGENTS {
        let key = match adapter.skills() {
            SkillsRoot::Neutral => 0,
            SkillsRoot::ClaudeCode => 1,
            SkillsRoot::Codex => 2,
            SkillsRoot::DshHarness => 3,
        };
        groups.entry(key).or_default().push(adapter);
    }
    groups.into_values().collect()
}

fn fleet_each(options: &SetupOptions, config: &Config, install: bool) {
    std::thread::scope(|scope| {
        for group in fleet_groups() {
            scope.spawn(move || {
                for adapter in group {
                    if install {
                        setup(adapter, config, options).expect("setup runs");
                    } else {
                        uninstall(adapter, options).expect("uninstall runs");
                    }
                }
            });
        }
    });
}

/// A root and path with no ownership record at all, for a `reviewer_is_ours`
/// assertion that means only the static and historical checks: no digest was
/// ever written for either, so the third arm can never fire and answers
/// nothing either way.
fn unrecorded_reviewer_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a temporary root");
    let path = root.path().join("review-blind.md");
    (root, path)
}

fn owns_reviewer(paths: &AgentPaths, reviewer: &Path) -> bool {
    skill::record::created_outside(&paths.skill_root, reviewer)
}

fn installed_phases(root: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("sdd-"))
        .collect();
    names.sort();
    names
}

fn assert_one_action(result: &SetupResult, path: &Path, kind: ActionKind, change: Change) {
    let matching: Vec<_> = result
        .actions
        .iter()
        .filter(|action| action.path == path)
        .map(|action| (action.kind, action.change))
        .collect();
    assert_eq!(matching, [(kind, change)]);
}

#[test]
fn only_the_verified_adapters_have_a_skill_root_of_their_own() {
    // Stated by its complement, because that is the half that does not grow.
    let own: Vec<&str> = AGENTS
        .iter()
        .filter(|adapter| adapter.discovers_skills())
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(
        own,
        vec!["claude-code", "codex", "deepseek-harness"],
        "an adapter was given a skill directory of its own; read the path out of that agent's own \
         shipping source and say so in the handoff"
    );
}

/// The acceptance for the canonical table: a declaration that disagrees is
/// refused where it is made, not where it is used. The predicate is the same
/// one the compile-time assertion runs over `AGENTS`.
#[test]
fn an_adapter_whose_declaration_disagrees_with_the_table_fails_at_construction() {
    let canonical = agent("claude-code").capabilities();
    assert!(
        AgentAdapter::checked(
            "claude-code",
            "Claude Code",
            InstructionFile::ClaudeCode,
            canonical,
        )
        .is_ok(),
        "the table refused the declaration it holds"
    );

    // Plausible, not absurd: the same runtime, with its skill root moved to the
    // shared tree every other adapter uses.
    let mismatched = Capabilities {
        skills: SkillsRoot::Neutral,
        ..canonical
    };
    assert!(
        AgentAdapter::checked(
            "claude-code",
            "Claude Code",
            InstructionFile::ClaudeCode,
            mismatched,
        )
        .is_err(),
        "an adapter declaring the wrong skills root was accepted"
    );
}

/// A retired slug is still in the table the gate reads, so a hook from a
/// machine that installed it is answered rather than refused as unknown.
#[test]
fn a_retired_runtime_still_carries_what_it_was_installed_with() {
    assert_ne!(
        capabilities_for_slug("cline"),
        Capabilities::NOTHING,
        "a retired runtime fell out of the capabilities lookup"
    );
}

/// The fail-closed default: a runtime the table has no entry for carries
/// nothing, rather than inheriting a row.
#[test]
fn a_runtime_absent_from_the_table_carries_nothing() {
    let absent = capabilities_for_slug("a-runtime-nobody-has-decided-about");
    assert_eq!(
        absent,
        Capabilities::NOTHING,
        "a runtime with no table entry did not carry nothing"
    );
    assert_eq!(absent.gate, Gate::None);
    assert!(!absent.tool_server);
    assert!(!absent.contract);
    assert!(!absent.agent_definitions);
    assert!(!absent.loop_plugin);
    assert!(!absent.offers_loop);
    assert!(!absent.review_relay);
    assert_eq!(absent.skills, SkillsRoot::Neutral);
}

#[test]
fn the_loop_plugin_path_and_the_declaration_agree() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        assert_eq!(
            adapter.has_loop_plugin(),
            paths.loop_plugin.is_some(),
            "{} disagrees with itself about the loop plugin",
            adapter.slug
        );
    }
}

#[test]
fn every_agent_slug_is_unique() {
    let mut slugs = AGENTS.iter().map(|a| a.slug).collect::<Vec<_>>();
    slugs.sort_unstable();
    let before = slugs.len();
    slugs.dedup();
    assert_eq!(before, slugs.len(), "two adapters answer to the same slug");
}

#[test]
fn model_catalog_provenance_belongs_to_each_adapter() {
    assert_eq!(
        agent("claude-code").model_catalog(),
        ModelCatalogSource::Curated(&["fable", "opus", "sonnet", "haiku"])
    );
    assert_eq!(
        agent("codex").model_catalog(),
        ModelCatalogSource::Curated(&["gpt-6-astra", "gpt-6.1-sol", "gpt-6-luna", "gpt-5.5"])
    );
    assert_eq!(
        agent("opencode").model_catalog(),
        ModelCatalogSource::OpenCode
    );
    for adapter in AGENTS
        .iter()
        .filter(|adapter| !["claude-code", "codex", "opencode"].contains(&adapter.slug))
    {
        assert_eq!(
            adapter.model_catalog(),
            ModelCatalogSource::None,
            "{} was given an invented model catalog",
            adapter.slug
        );
    }
}

#[test]
fn default_model_profiles_belong_only_to_adapters_with_stable_catalogs() {
    let claude = agent("claude-code").model_profiles();
    assert_eq!(
        claude
            .iter()
            .map(|profile| profile.name)
            .collect::<Vec<_>>(),
        ["balanced", "performance", "economy"]
    );
    let codex = agent("codex").model_profiles();
    for (profile, expected) in [
        (
            &claude[1],
            "developer=opus, analyst=opus, reviewer=opus, judge=opus, apply=opus, orchestrate=opus",
        ),
        (
            &claude[2],
            "developer=sonnet, analyst=sonnet, reviewer=haiku, judge=sonnet, apply=sonnet, orchestrate=sonnet",
        ),
    ] {
        assert_eq!(
            profile
                .routing()
                .expect("a valid built-in profile")
                .as_value(),
            expected
        );
    }
    assert_eq!(
        claude[0]
            .routing()
            .expect("a valid built-in profile")
            .as_value(),
        "developer=sonnet, analyst=opus, reviewer=sonnet, judge=opus, apply=sonnet, orchestrate=opus"
    );
    for (profile, expected) in [
        (
            &codex[1],
            "developer=gpt-6-astra, analyst=gpt-6-astra, reviewer=gpt-6-astra, judge=gpt-6-astra, apply=gpt-6-astra, orchestrate=gpt-6-astra",
        ),
        (
            &codex[2],
            "developer=gpt-6.1-sol, analyst=gpt-6.1-sol, reviewer=gpt-6-luna, judge=gpt-6.1-sol, apply=gpt-6.1-sol, orchestrate=gpt-6.1-sol",
        ),
    ] {
        assert_eq!(
            profile
                .routing()
                .expect("a valid built-in profile")
                .as_value(),
            expected
        );
    }

    for adapter in [agent("claude-code"), agent("codex")] {
        let ModelCatalogSource::Curated(catalog) = adapter.model_catalog() else {
            panic!("profiled adapters must have stable catalogs");
        };
        for profile in adapter.model_profiles() {
            for route in profile
                .routing()
                .expect("a valid built-in profile")
                .by_role
                .values()
            {
                let model = route.model_value().expect("a profile names a model");
                assert!(
                    catalog.contains(&model),
                    "{} profile {} uses uncatalogued model {route}",
                    adapter.slug,
                    profile.name
                );
            }
        }
    }

    assert_eq!(
        codex.iter().map(|profile| profile.name).collect::<Vec<_>>(),
        ["balanced", "performance", "economy"]
    );
    assert_eq!(
        codex[0]
            .routing()
            .expect("a valid built-in profile")
            .as_value(),
        "developer=gpt-6.1-sol, analyst=gpt-6-astra, reviewer=gpt-6.1-sol, judge=gpt-6-astra, apply=gpt-6.1-sol, orchestrate=gpt-6-astra"
    );

    for adapter in AGENTS
        .iter()
        .filter(|adapter| !["claude-code", "codex"].contains(&adapter.slug))
    {
        assert!(
            adapter.model_profiles().is_empty(),
            "{} received a profile without a stable catalog",
            adapter.slug
        );
    }
}

/// Every preset runs the analyst at its own judge tier.
///
/// `analyst` is a role Estigia installs a definition for, and a preset that
/// omitted it would install `developer` and leave the repository-read-only
/// worker inheriting the parent. The tier is the preset's own `judge` answer,
/// so the crossing is held rather than the six expected names.
#[test]
fn every_preset_runs_the_analyst_at_its_judge_tier() {
    let mut checked = 0;
    for adapter in AGENTS
        .iter()
        .filter(|adapter| !adapter.model_profiles().is_empty())
    {
        for profile in adapter.model_profiles() {
            let routing = profile.routing().expect("a valid built-in profile");
            assert_eq!(
                routing.for_target("analyst"),
                routing.for_target("judge"),
                "{} profile {} runs `analyst` off its judge tier",
                adapter.slug,
                profile.name
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 6, "the reviewed presets moved");
}

/// The balanced posture installs no delegated role: presence in the operator's
/// own routing is what installs a definition that carries `Write`, `Edit` and
/// `Bash`.
#[test]
fn a_balanced_installation_installs_no_role() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    setup(adapter, &Config::default(), &options).expect("setup runs");

    for role in ["developer.md", "analyst.md"] {
        assert!(
            !agents.join(role).exists(),
            "the preset installed `{role}`, which only `Model routing` may name"
        );
    }
    let blind = fs::read_to_string(agents.join("review-blind.md")).expect("the reviewer installs");
    assert!(
        blind.contains("model: inherit"),
        "`review-blind` stops being the literal `inherit` it is defined to be:\n{blind}"
    );
}

/// An installation that carries its rows keeps them; one missing a row takes
/// the default, and the next `setup` writes that down.
///
/// The whole point of a defaulted row is that it is silent. This drives the
/// installed contract: the read an upgrade performs, and the write the next
/// `setup` performs over it.
#[test]
fn an_installed_table_keeps_its_rows_and_a_missing_row_takes_the_new_default_on_setup() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let mut old = Config::default();
    old.set_profile(crate::config::Profile::Cautious);
    setup(adapter, &old, &options).expect("the old install runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let contract = paths.skill_root.join("SKILL.md");

    let read = crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug))
        .expect("the installed table reads");
    assert_eq!(
        read.profile(),
        Some(crate::config::Profile::Cautious),
        "the upgrade moved a row the installation carried"
    );

    // A table that never had the row — and predates `Authorisation posture` —
    // takes the new default.
    let text = fs::read_to_string(&contract).expect("the contract reads");
    let doctored = text
        .lines()
        .filter(|line| {
            !line.starts_with("| Authorisation posture |")
                && !line.starts_with("| Transition authorisation |")
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&contract, &doctored).expect("the contract is doctored");
    let read = crate::skill::installed_config_for(&paths.skill_root, Some(adapter.slug))
        .expect("a table missing a row reads");
    assert_eq!(read.profile(), None, "ask/auto is not a profile");
    assert_eq!(
        crate::config::Setting::Transitions.value_of(&read),
        "auto",
        "a row nobody wrote did not take the new default"
    );

    // And the next `setup` writes it down, without a report.
    setup(adapter, &read, &options).expect("the next setup runs");
    let written = fs::read_to_string(&contract).expect("the rewritten contract reads");
    assert!(
        written.contains("| Transition authorisation | auto |"),
        "the next setup did not write the defaulted row down:\n{written}"
    );
}

#[test]
fn every_agent_resolves_two_absolute_paths_that_differ() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve in the sandbox");
        assert!(paths.skill_root.is_absolute(), "{}", adapter.slug);
        assert!(paths.instructions.is_absolute(), "{}", adapter.slug);
        assert_ne!(
            paths.skill_root, paths.instructions,
            "{} would write the skill over its own instruction file",
            adapter.slug
        );
    }
}

#[test]
fn no_two_agents_share_an_instruction_file() {
    // Sharing one would make uninstalling one agent silently unconfigure the
    // other: the directive is fenced once.
    let (_home, options) = sandbox();
    let mut seen = Vec::new();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        assert!(
            !seen.contains(&paths.instructions),
            "{} shares an instruction file with an earlier adapter",
            adapter.slug
        );
        seen.push(paths.instructions);
    }
}

#[test]
fn an_unknown_agent_is_refused_and_the_refusal_lists_the_known_ones() {
    let refusal = find_agent("emacs").unwrap_err();
    assert_eq!(refusal.code, "agent-unknown");
    let rendered = refusal.to_string();
    for adapter in AGENTS {
        assert!(
            rendered.contains(adapter.slug),
            "{} is not offered",
            adapter.slug
        );
    }
}

#[test]
fn the_directive_names_the_path_the_skill_was_written_to() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let directive = directive_for(adapter, &paths);
        assert!(
            directive.contains(&paths.skill_root.display().to_string()),
            "{} names a skill path it does not install to",
            adapter.slug
        );
        assert!(
            !directive.contains("{path}"),
            "{} left a placeholder",
            adapter.slug
        );
        assert!(
            !directive.contains("{skill}"),
            "{} left a placeholder",
            adapter.slug
        );
    }
}

#[test]
fn setup_installs_the_skill_and_fences_the_directive() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let config = Config::default();

    let result = setup(adapter, &config, &options).expect("setup succeeds");
    assert_eq!(result.changed_files(), skill::FILES.len() + 7);
    assert!(is_configured(adapter, &options));

    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(
        skill::presence(&paths.skill_root, &config),
        skill::Presence::Current
    );
    let instructions = fs::read_to_string(&paths.instructions).expect("the directive was written");
    assert!(instructions.contains(DIRECTIVE_BEGIN));
    assert!(instructions.contains(DIRECTIVE_END));
}

#[test]
fn a_fresh_dry_run_reports_the_one_real_ownership_record_action() {
    // Invariant four: a plan that disagrees with the act is worse than no plan.
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let config = Config::default();
    let record = skill::record::path(
        &resolve_paths(adapter, &options)
            .expect("paths resolve")
            .skill_root,
    );

    let planned = setup(
        adapter,
        &config,
        &SetupOptions {
            dry_run: true,
            ..options.clone()
        },
    )
    .expect("the plan is produced");
    let performed = setup(adapter, &config, &options).expect("setup succeeds");

    assert_eq!(planned.actions, performed.actions);
    assert_one_action(&planned, &record, ActionKind::Skill, Change::Create);
    assert_one_action(&performed, &record, ActionKind::Skill, Change::Create);
}

#[test]
fn an_upgrade_dry_run_reports_the_one_real_ownership_record_action() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    setup(adapter, &Config::default(), &options).expect("the old installation exists");
    fs::remove_file(&reviewer).expect("the old installation has no reviewer");
    skill::record::forget_outside(&paths.skill_root, &reviewer).expect("old ownership is absent");

    let planned = setup(
        adapter,
        &Config::default(),
        &SetupOptions {
            dry_run: true,
            ..options.clone()
        },
    )
    .expect("the upgrade plan succeeds");
    let performed = setup(adapter, &Config::default(), &options).expect("the upgrade succeeds");

    assert_eq!(planned.actions, performed.actions);
    let record = skill::record::path(&paths.skill_root);
    assert_one_action(&planned, &record, ActionKind::Skill, Change::Update);
    assert_one_action(&performed, &record, ActionKind::Skill, Change::Update);
}

#[test]
fn a_dry_run_writes_nothing() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(
        adapter,
        &Config::default(),
        &SetupOptions {
            dry_run: true,
            ..options
        },
    )
    .expect("the plan is produced");
    assert!(
        fs::read_dir(home.path())
            .expect("the sandbox home exists")
            .next()
            .is_none(),
        "a dry run left something behind"
    );
}

#[test]
fn an_instruction_file_that_cannot_be_read_is_unknown_not_absent() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(read_configured(adapter, &options), Installed::Absent);

    fs::create_dir_all(&paths.instructions).expect("a directory stands where the file is expected");
    assert!(
        fs::read_to_string(&paths.instructions).is_err(),
        "the fixture must be unreadable to every user"
    );
    let read = read_configured(adapter, &options);
    assert!(matches!(read, Installed::Unknown(_)), "{read:?}");
    assert!(!is_configured(adapter, &options));
}

#[test]
fn uninstall_is_the_exact_inverse_of_setup() {
    // Invariant two, checked byte for byte against a file Estigia did not own.
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");

    fs::create_dir_all(paths.instructions.parent().expect("a parent")).expect("create the dir");
    let theirs = "# My own instructions\n\nDo the thing my way.\n";
    fs::write(&paths.instructions, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(is_configured(adapter, &options));

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!is_configured(adapter, &options));
    assert_eq!(
        fs::read_to_string(&paths.instructions).expect("their file survives"),
        theirs,
        "uninstall did not restore the file it found"
    );
}

#[test]
fn uninstall_keeps_another_tool_s_block() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    fs::create_dir_all(paths.instructions.parent().expect("a parent")).expect("create the dir");
    let theirs = "<!-- BEGIN LETEO MEMORY PROTOCOL -->\nremember things\n<!-- END LETEO MEMORY PROTOCOL -->\n";
    fs::write(&paths.instructions, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    uninstall(adapter, &options).expect("uninstall succeeds");

    let after = fs::read_to_string(&paths.instructions).expect("their file survives");
    assert!(
        after.contains("LETEO MEMORY PROTOCOL"),
        "a companion's block was taken out"
    );
    assert!(!after.contains("ESTIGIA"));
}

/// Another skill, their own markdown, their own configuration — all still
/// theirs after every adapter installs and leaves. The other tests in this file
/// cover the files an adapter *wires*; nothing else covered the skill directory.
#[test]
fn what_the_operator_keeps_beside_the_skill_is_still_there_afterwards() {
    let (home, options) = sandbox();

    let mut theirs: Vec<(std::path::PathBuf, &'static str)> = Vec::new();
    let mut estigias: Vec<std::path::PathBuf> = Vec::new();
    for adapter in AGENTS {
        let Ok(paths) = resolve_paths(adapter, &options) else {
            continue;
        };
        let root = &paths.skill_root;
        let beside = root.parent().map(|parent| parent.join("their-other-skill"));
        fs::create_dir_all(root.join("references")).expect("the directory");
        if let Some(beside) = &beside {
            fs::create_dir_all(beside).expect("their other skill");
        }
        for (path, body) in [
            (root.join("MY-NOTES.md"), "# my notes\n"),
            (
                root.join(crate::config::LOCAL_FILE),
                "| Evidence standard | measuring |\n",
            ),
            // Inside a directory Estigia creates, so the emptied-directory
            // sweep runs right past it.
            (root.join("references").join("mine.md"), "# mine\n"),
        ] {
            if !path.exists() {
                fs::write(&path, body).expect("their file");
                theirs.push((path, body));
            }
        }
        if let Some(beside) = beside {
            let path = beside.join("SKILL.md");
            if !path.exists() {
                fs::write(&path, "# their other skill\n").expect("their other skill");
                theirs.push((path, "# their other skill\n"));
            }
        }
        // Estigia's own, for contrast: `config set --agent` writes this one,
        // so this one goes and the operator's above stays.
        let own = skill::agent_override(&paths.skill_root, adapter.slug);
        if !own.exists() {
            fs::write(&own, "| Delivery | ask |\n").expect("its own file");
            estigias.push(own);
        }
    }
    assert!(
        theirs.len() >= 8 && !estigias.is_empty(),
        "too little of the operator's was put beside the skill: {}",
        theirs.len()
    );

    setup_fleet(&options, &Config::default());
    uninstall_fleet(&options);

    let short = |path: &std::path::Path| {
        path.strip_prefix(home.path())
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let mut lost: Vec<String> = Vec::new();
    for (path, body) in &theirs {
        match fs::read_to_string(path) {
            Ok(after) if &after == body => {}
            Ok(after) => lost.push(format!("{}: was {body:?}, is {after:?}", short(path))),
            Err(error) => lost.push(format!("{}: {error}", short(path))),
        }
    }
    assert!(
        lost.is_empty(),
        "{} thing(s) of the operator's did not survive an uninstall: {:#?}",
        lost.len(),
        lost
    );
    for path in &estigias {
        assert!(
            !path.exists(),
            "{} is Estigia's own and outlived it — it would sit there configuring an \
             install that no longer exists",
            short(path)
        );
    }
}

#[test]
fn every_command_estigia_hands_a_shell_quotes_the_path_it_names() {
    // A bare path is what this catches: on Windows bash reads the separators as
    // escapes and every hook fails. **Which** quote differs from the git hook —
    // see `render::hook_command`.
    let (home, options) = sandbox();
    setup_fleet(&options, &Config::default());

    let mut checked = 0;
    let mut wrong: Vec<String> = Vec::new();
    let mut pending = vec![home.path().to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let mut stack = vec![value];
            while let Some(node) = stack.pop() {
                match node {
                    serde_json::Value::Object(map) => {
                        for (key, child) in map {
                            // An MCP server entry is handed to the operating
                            // system with no shell; quoting that one breaks it.
                            if key == "command"
                                && child.as_str().is_some_and(|line| line.contains(" hook "))
                            {
                                let line = child.as_str().unwrap_or_default();
                                checked += 1;
                                if !line.starts_with(['\'', '"']) {
                                    wrong.push(format!("{}: {line}", path.display()));
                                }
                            }
                            stack.push(child);
                        }
                    }
                    serde_json::Value::Array(items) => stack.extend(items),
                    _ => {}
                }
            }
        }
    }
    assert!(
        checked >= 3,
        "only {checked} command line(s) were found, so this checked almost nothing"
    );
    assert!(
        wrong.is_empty(),
        "{} command line(s) hand a shell an unquoted path: {:#?}",
        wrong.len(),
        wrong
    );
}

#[test]
fn a_path_no_quoting_can_carry_is_refused_before_a_hook_is_written_around_it() {
    // There is no quoting that covers both `sh` and `cmd`, so the ambiguity is
    // removed instead of resolved. Every case measured by handing `sh` the
    // string a hook actually carries.
    for (path, why) in [
        ("/tmp/$(id)/estigia", "command substitution"),
        ("/tmp/${HOME}/estigia", "parameter expansion"),
        ("/tmp/`id`/estigia", "the older substitution"),
        ("/tmp/a\"b/estigia", "it closes the quoting"),
        ("C:\\Users\\a%USERNAME%b\\estigia.exe", "`cmd` expands it"),
        (
            "\\\\server\\share\\estigia.exe",
            "a shell reads `\\\\` as one",
        ),
    ] {
        let refusal = quotable(Path::new(path)).expect_err(why);
        assert_eq!(
            refusal.code, "executable-path-not-quotable",
            "{path}: {why}"
        );
        // Named, so an operator can act: the path, and a way out.
        assert!(format!("{refusal}").contains(path), "{path} is not named");
        assert!(
            matches!(refusal.resolution, Resolution::NoCommand { .. }),
            "{path}: where Estigia lives is not something a command here can move"
        );
    }

    // And the paths people actually have are not refused; the apostrophe had
    // to be measured rather than assumed.
    for path in [
        "/usr/local/bin/estigia",
        "C:\\Users\\alex\\.cargo\\bin\\estigia.exe",
        "C:\\Users\\O'Brien\\.cargo\\bin\\estigia.exe",
        "/home/a b/estigia",
        "/home/a-b_c.d/estigia",
        "C:\\Program Files\\estigia\\estigia.exe",
    ] {
        quotable(Path::new(path)).unwrap_or_else(|why| panic!("{path} was refused: {why}"));
    }
}

#[test]
fn taking_estigia_out_takes_its_own_state_with_it_and_not_before() {
    // The operator's requirement in their words: *uninstall should remove the
    // app and everything related to it, and only that*. The state is not inert
    // on the way back in, either.
    let (home, options) = sandbox();
    setup_fleet(&options, &Config::default());
    let runs =
        crate::harness::session::state_root(options.home_dir.as_deref()).expect("a state root");
    fs::create_dir_all(&runs).expect("the state directory");
    let mut mine = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    mine.issue = Some(crate::identity::Identity::Number(12));
    crate::harness::session::store(&runs, &mine).expect("a run pointer");
    crate::harness::session::record(&runs, &serde_json::json!({"verdict": "allow"}));
    fs::write(crate::harness::standdown::path(&runs), "{}").expect("a stand-down");
    crate::harness::session::set_stop_after(&runs, "claude-abcd1234", Some("a request"))
        .expect("a stop-after request");
    let state: Vec<std::path::PathBuf> = vec![
        crate::harness::session::pointer_path(&runs, "claude-abcd1234"),
        crate::transport::store::beside(&runs),
        crate::harness::standdown::path(&runs),
        crate::harness::session::stop_after_path(&runs, "claude-abcd1234"),
    ];
    for path in &state {
        assert!(path.is_file(), "{} was not written", path.display());
    }

    // One agent out and others still here: the state is machine-wide.
    uninstall(agent("codex"), &options).expect("uninstall runs");
    assert!(
        forget_state(&options).is_empty(),
        "the state went while agents were still reading it"
    );
    for path in &state {
        assert!(path.is_file(), "{} went early", path.display());
    }

    // And a dry run says what it would do and does none of it.
    let dry = SetupOptions {
        dry_run: true,
        ..options.clone()
    };
    std::thread::scope(|scope| {
        for group in fleet_groups() {
            let dry = &dry;
            scope.spawn(move || {
                for adapter in group {
                    uninstall(adapter, dry).expect("the plan is produced");
                }
            });
        }
    });
    uninstall_fleet(&options);
    let planned = forget_state(&dry);
    assert!(planned.len() >= 3, "the plan names too little: {planned:?}");
    for path in &state {
        assert!(path.is_file(), "a dry run removed {}", path.display());
    }

    let taken = forget_state(&options);
    assert_eq!(taken.len(), planned.len(), "the plan and the act disagree");
    for path in &state {
        assert!(!path.exists(), "{} outlived the uninstall", path.display());
    }
    assert!(
        !runs.exists() && !runs.parent().is_some_and(std::path::Path::exists),
        "the state directory outlived everything in it"
    );
    assert!(
        !crate::harness::session::stop_after_dir(&runs).exists(),
        "the stop-after directory outlived the uninstall"
    );
    // And nothing of anybody else's went with it.
    assert_eq!(
        files_under(home.path()),
        0,
        "something was left behind: {:?}",
        listing(home.path())
    );
}

fn two_claude_roots() -> (tempfile::TempDir, SetupOptions, SetupOptions) {
    let (home, base) = sandbox();
    let first = SetupOptions {
        claude_config_dir: Some(home.path().join(".claude")),
        ..base.clone()
    };
    let second = SetupOptions {
        claude_config_dir: Some(home.path().join(".claude-personal")),
        ..base
    };
    (home, first, second)
}

fn install_both_roots_with_a_store(
    first: &SetupOptions,
    second: &SetupOptions,
) -> std::path::PathBuf {
    setup(agent("claude-code"), &Config::default(), first).expect("install into the first root");
    setup(agent("claude-code"), &Config::default(), second).expect("install into the second root");
    let runs =
        crate::harness::session::state_root(first.home_dir.as_deref()).expect("a state root");
    fs::create_dir_all(&runs).expect("the state directory");
    let store = crate::transport::store::beside(&runs);
    fs::write(&store, "decision history").expect("a store");
    store
}

#[test]
fn uninstalling_one_claude_root_keeps_the_store_the_other_root_reads() {
    let (_home, first, second) = two_claude_roots();
    let store = install_both_roots_with_a_store(&first, &second);
    uninstall(agent("claude-code"), &second).expect("uninstall from the second root");
    assert!(is_present(agent("claude-code"), &first));
    let taken = forget_state(&second);
    assert!(
        taken.is_empty() && store.is_file(),
        "the store the first root reads went: {taken:?}"
    );
}

#[test]
fn uninstalling_the_last_claude_root_takes_the_store_and_its_records() {
    let (home, first, second) = two_claude_roots();
    let store = install_both_roots_with_a_store(&first, &second);
    uninstall(agent("claude-code"), &second).expect("uninstall from the second root");
    assert!(forget_state(&second).is_empty());
    uninstall(agent("claude-code"), &first).expect("uninstall from the first root");
    let taken = forget_state(&first);
    assert!(taken.contains(&store), "the store stayed: {taken:?}");
    assert!(!store.exists());
    assert_eq!(
        files_under(home.path()),
        0,
        "something was left behind: {:?}",
        listing(home.path())
    );
}

#[test]
fn an_unreadable_install_root_record_keeps_the_store_and_is_named() {
    let (_home, first, second) = two_claude_roots();
    let store = install_both_roots_with_a_store(&first, &second);
    uninstall(agent("claude-code"), &first).expect("uninstall from the first root");
    uninstall(agent("claude-code"), &second).expect("uninstall from the second root");
    let registry = install_root_registry(&second).expect("a registry");
    fs::create_dir_all(&registry).expect("the registry");
    let record = registry.join("unreadable.json");
    fs::write(&record, "not json").expect("a damaged record");
    assert!(forget_state(&second).is_empty() && store.is_file());
    assert_eq!(unreadable_install_roots(&second), vec![record]);
}

fn pi_install_root_record(options: &SetupOptions) -> (std::path::PathBuf, serde_json::Value) {
    let records = install_root_records(options);
    assert_eq!(records.len(), 1, "{records:?}");
    let body = fs::read_to_string(&records[0]).expect("the record reads");
    (
        records[0].clone(),
        serde_json::from_str(&body).expect("the record is JSON"),
    )
}

#[test]
fn a_global_pi_install_records_the_agent_directory_it_was_made_under() {
    let (_home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let directory = elsewhere.path().join("pi-home");
    setup(
        agent("pi"),
        &Config::default(),
        &pi_home(&options, &directory),
    )
    .expect("setup succeeds");
    let (_, record) = pi_install_root_record(&options);
    assert_eq!(
        record["pi_agent_dir"],
        serde_json::json!(directory.to_string_lossy()),
        "{record}"
    );
}

#[test]
fn a_relocated_pi_install_is_found_through_its_record_by_a_run_that_was_not_told_where() {
    let (_home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let directory = elsewhere.path().join("pi-home");
    let adapter = agent("pi");
    setup(adapter, &Config::default(), &pi_home(&options, &directory)).expect("setup succeeds");
    assert!(!is_present(adapter, &options));
    assert_eq!(recorded_install_roots(&options), (true, Vec::new()));

    uninstall(adapter, &pi_home(&options, &directory)).expect("uninstall succeeds");
    assert_eq!(recorded_install_roots(&options), (false, Vec::new()));
}

#[test]
fn a_record_written_before_it_named_the_pi_agent_directory_still_reads_as_the_default_one() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let (path, mut record) = pi_install_root_record(&options);
    fs::remove_file(&path).expect("the record goes");
    record
        .as_object_mut()
        .expect("an object")
        .remove("pi_agent_dir");
    fs::write(path.with_file_name("older.json"), record.to_string()).expect("the older record");

    assert_eq!(recorded_install_roots(&options), (true, Vec::new()));
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(recorded_install_roots(&options), (false, Vec::new()));
}

#[test]
fn a_live_run_pointer_survives_a_plain_reinstall() {
    let (_home, options) = sandbox();
    setup_fleet(&options, &Config::default());
    let runs =
        crate::harness::session::state_root(options.home_dir.as_deref()).expect("a state root");
    fs::create_dir_all(&runs).expect("the state directory");
    let mut mine = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    mine.issue = Some(crate::identity::Identity::Number(12));
    crate::harness::session::store(&runs, &mine).expect("a run pointer");
    let pointer = crate::harness::session::pointer_path(&runs, "claude-abcd1234");
    let before = fs::read_to_string(&pointer).expect("the pointer reads");

    // The reinstall: every agent set up again over the existing install.
    setup_fleet(&options, &Config::default());
    let after = crate::harness::session::load(&runs, "claude-abcd1234");
    assert_eq!(
        after.issue,
        Some(crate::identity::Identity::Number(12)),
        "a plain reinstall took the run's pointer, and with it the gate's reading of the claim"
    );
    assert_eq!(
        fs::read_to_string(&pointer).expect("the pointer still reads"),
        before,
        "the pointer bytes moved under a plain reinstall"
    );
}

#[test]
fn uninstalling_what_was_never_installed_creates_nothing() {
    let (home, options) = sandbox();
    let adapter = agent("qwen");
    let result = uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(result.changed_files(), 0);
    assert!(
        fs::read_dir(home.path())
            .expect("the sandbox home exists")
            .next()
            .is_none(),
        "uninstall created a file to say Estigia was not in it"
    );
}

#[test]
fn setting_up_twice_reports_no_change_the_second_time() {
    let (_home, options) = sandbox();
    let adapter = agent("agents");
    let config = Config::default();
    setup(adapter, &config, &options).expect("setup succeeds");
    let again = setup(adapter, &config, &options).expect("setup succeeds");
    assert_eq!(again.changed_files(), 0);
}

#[test]
fn an_agent_that_does_not_discover_skills_still_gets_them_somewhere_it_is_told_about() {
    let (_home, options) = sandbox();
    for adapter in AGENTS.iter().filter(|a| !a.discovers_skills()) {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        assert!(
            paths
                .skill_root
                .ends_with(Path::new(".agents").join("skills").join(skill::DIRECTORY)),
            "{} claims no discovery but does not use the neutral root",
            adapter.slug
        );
    }
}

#[test]
fn changing_the_configuration_does_not_rewrite_the_whole_contract() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    let contract = paths.skill_root.join(skill::CONTRACT);
    // The version to replace is read from the payload rather than spelled: a
    // spelled one went stale and turned this test into a no-op.
    let embedded = skill::version().expect("the payload declares a version");
    let on_disk = fs::read_to_string(&contract).expect("the contract is installed");
    let newer = on_disk.replace(&format!("version: \"{embedded}\""), "version: \"9.9.9\"");
    assert_ne!(
        newer, on_disk,
        "the contract on disk does not carry the embedded version, so nothing was made newer"
    );
    fs::write(&contract, &newer).expect("write the newer contract");

    let changed = Config {
        evidence: crate::config::Evidence::Measuring,
        ..Config::default()
    };
    rewrite_configuration(&contract, &changed).expect("the block is rewritten");

    let after = fs::read_to_string(&contract).expect("the contract survives");
    assert!(
        after.contains("9.9.9"),
        "the newer contract was silently downgraded"
    );
    assert!(after.contains("| Evidence standard | measuring |"));
}

#[test]
fn the_directive_says_what_the_contract_says_about_unknown_results() {
    // Seam test: the always-loaded directive against the contract it summarises.
    assert!(DIRECTIVE_TEMPLATE.contains("not clearance"));
    assert!(
        skill::CONTRACT_CONTENTS.contains("never reinterpret an unknown result as clearance"),
        "the contract no longer says what the directive summarises"
    );
}

/// The phrase the standing-delegation paragraph is held by. One literal, so the
/// template test and the installed-file tests agree on the same words.
const STANDING_DELEGATION_GRANT: &str = "decided by the operator table and not by asking again";

#[test]
fn the_directive_carries_the_standing_delegation_grant() {
    // The always-loaded block states the standing grant, and the installed file
    // really carries it rather than only the template.
    assert!(DIRECTIVE_TEMPLATE.contains(STANDING_DELEGATION_GRANT));
    let (_home, options) = sandbox();
    let adapter = agent("cursor");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let instructions = fs::read_to_string(&paths.instructions).expect("the directive was written");
    assert!(
        instructions.contains(STANDING_DELEGATION_GRANT),
        "the installed directive block does not carry the standing-delegation grant"
    );
}

#[test]
fn the_standing_delegation_paragraph_names_no_row_value() {
    // The paragraph grants a launch; it must not restate the table it points at,
    // or it becomes the second copy `src/setup/mod.rs` was written against. The
    // values, the panel sizes and the delegation thresholds live in the skill.
    let paragraph = DIRECTIVE_TEMPLATE
        .split("\n\n")
        .find(|part| part.contains("operator table"))
        .expect("the directive carries the standing-delegation paragraph");
    for forbidden in ["blind", "auto", "threshold"] {
        assert!(
            !paragraph.to_lowercase().contains(forbidden),
            "the standing-delegation paragraph names `{forbidden}`, a row value the skill owns"
        );
    }
    assert!(
        !paragraph.contains(|character: char| character.is_ascii_digit()),
        "the standing-delegation paragraph names a count the skill owns"
    );
}

#[test]
fn every_adapter_that_gets_a_directive_gets_the_standing_delegation_grant() {
    // Every adapter receives the same paragraph and none receives a
    // host-specific sentence. The directive is built from one template, so this
    // holds by construction; the test says so rather than assuming it.
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let directive = directive_for(adapter, &paths);
        assert!(
            directive.contains(STANDING_DELEGATION_GRANT),
            "{} reads a directive without the standing-delegation grant",
            adapter.slug
        );
    }
}

#[test]
fn every_adapter_names_its_own_instruction_file() {
    for (index, adapter) in AGENTS.iter().enumerate() {
        for other in &AGENTS[index + 1..] {
            assert_ne!(
                adapter.instructions, other.instructions,
                "{} and {} name the same instruction file, so the second inherits the first's \
                 agent-definitions answer rather than answering `None` on purpose",
                adapter.slug, other.slug
            );
        }
    }
}

#[test]
fn crlf_instructions_do_not_produce_a_second_directive() {
    let (_home, options) = sandbox();
    let adapter = agent("cursor");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    fs::create_dir_all(paths.instructions.parent().expect("a parent")).expect("create the dir");
    fs::write(&paths.instructions, "# Notes\r\n\r\nkeep me\r\n").expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    setup(adapter, &Config::default(), &options).expect("setup succeeds again");

    let after = fs::read_to_string(&paths.instructions).expect("their file survives");
    assert_eq!(after.matches(DIRECTIVE_BEGIN).count(), 1);
    assert!(after.contains("keep me"));
}

#[test]
fn the_mcp_entry_keeps_every_other_server_and_comes_out_cleanly() {
    // Invariant two, on a file that is full of other people's servers.
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let config = paths.mcp_config.expect("Claude Code reads an MCP config");
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    let theirs = r#"{"mcpServers":{"leteo":{"command":"leteo","args":["mcp"]}},"theme":"dark"}"#;
    fs::write(&config, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).expect("read")).expect("json");
    assert!(after["mcpServers"]["estigia"]["command"].is_string());
    assert_eq!(after["mcpServers"]["estigia"]["args"][0], "mcp");
    assert_eq!(after["mcpServers"]["leteo"]["command"], "leteo");
    assert_eq!(after["theme"], "dark");

    uninstall(adapter, &options).expect("uninstall succeeds");
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config).expect("read")).expect("json");
    assert!(after["mcpServers"].get("estigia").is_none());
    assert_eq!(after["mcpServers"]["leteo"]["command"], "leteo");
    assert_eq!(after["theme"], "dark");
}

#[test]
fn the_zcode_mcp_entry_nests_under_servers_keeps_a_sibling_and_comes_out_cleanly() {
    // Invariant two, on ZCode's two-level `mcp.servers` shape.
    let (_home, options) = sandbox();
    let adapter = agent("zcode");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let config = paths.mcp_config.expect("ZCode reads an MCP config");
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    let theirs = "{\n    \"mcp\": {\n        \"servers\": {\n            \"leteo\": {\n                \"type\": \"stdio\",\n                \"command\": \"leteo\",\n                \"args\": [\n                    \"mcp\"\n                ]\n            }\n        }\n    }\n}\n";
    fs::write(&config, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let raw = fs::read_to_string(&config).expect("read");
    assert!(
        raw.contains("\n    \"mcp\""),
        "the operator's four-space indentation is not in the written file: {raw:?}"
    );
    let after: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let estigia = after["mcp"]["servers"]["estigia"]
        .as_object()
        .expect("estigia nests under mcp.servers");
    assert_eq!(estigia["type"], "stdio");
    assert!(estigia["command"].is_string());
    assert_eq!(estigia["args"], serde_json::json!(["mcp"]));
    assert_eq!(
        estigia.len(),
        3,
        "the written entry carries a field beyond {{type, command, args}}: {estigia:?}"
    );
    assert_eq!(after["mcp"]["servers"]["leteo"]["command"], "leteo");

    uninstall(adapter, &options).expect("uninstall succeeds");
    let raw = fs::read_to_string(&config).expect("read");
    assert!(
        raw.contains("\n    \"mcp\""),
        "the uninstall lost the operator's indentation: {raw:?}"
    );
    let after: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert!(
        after["mcp"]["servers"].get("estigia").is_none(),
        "the entry survived the uninstall: {after:#}"
    );
    assert_eq!(
        after["mcp"]["servers"]["leteo"]["command"], "leteo",
        "the sibling did not survive the uninstall: {after:#}"
    );
}

/// `mcp_entry` is the only reader of ZCode's nested `mcp.servers.<name>`, and
/// nothing crossed the write it reads against — `exposes_tools` was asserted
/// only for `claude-code`. A read that drops the `servers` hop compiles,
/// leaves every other adapter green, and reports `status`/`doctor` seeing no
/// tools for ZCode right after `setup` wrote them.
#[test]
fn zcodes_registered_server_is_read_back_by_exposes_tools_and_tools_command() {
    let (_home, options) = sandbox();
    let adapter = agent("zcode");
    assert!(!exposes_tools(adapter, &options), "nothing written yet");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        exposes_tools(adapter, &options),
        "the entry setup just wrote under mcp.servers.estigia was not read back"
    );
    let command = tools_command(adapter, &options)
        .expect("tools_command reads the same nested entry exposes_tools does");
    assert!(
        command
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains("estigia")),
        "the executable setup wrote under mcp.servers.estigia was not read back: {command:?}"
    );
}

/// OpenCode writes `command` as `[exe, "mcp"]`; reading it with `as_str` alone
/// answered `None`, which the widened `tools_fault` took for no registration.
#[test]
fn opencodes_registered_server_is_read_back_by_tools_command() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    assert!(!exposes_tools(adapter, &options), "nothing written yet");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        exposes_tools(adapter, &options),
        "the entry setup just wrote under mcp.estigia was not read back"
    );
    let command = tools_command(adapter, &options)
        .expect("tools_command reads the array command OpenCode writes, not only a string");
    assert!(
        command
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains("estigia")),
        "the executable setup wrote under mcp.estigia was not read back: {command:?}"
    );
}

#[test]
fn the_codex_toml_entry_is_edited_by_the_line_and_keeps_their_comments() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let config = paths.mcp_config.expect("Codex reads an MCP config");
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    let theirs = "# my own note
model = \"gpt\"

[mcp_servers.leteo]
command = \"leteo\"
";
    fs::write(&config, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after = fs::read_to_string(&config).expect("read");
    assert!(after.contains("# my own note"), "their comment went");
    assert!(after.contains("[mcp_servers.leteo]"), "their server went");
    assert!(after.contains("[mcp_servers.estigia]"));

    uninstall(adapter, &options).expect("uninstall succeeds");
    let after = fs::read_to_string(&config).expect("read");
    assert!(!after.contains("estigia"));
    assert!(after.contains("# my own note"));
    assert!(after.contains("[mcp_servers.leteo]"));
}

#[test]
fn setting_up_twice_does_not_register_two_servers() {
    let (_home, options) = sandbox();
    for slug in ["claude-code", "codex"] {
        let adapter = agent(slug);
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        let again = setup(adapter, &Config::default(), &options).expect("setup succeeds");
        assert_eq!(again.changed_files(), 0, "{slug} rewrote itself");
    }
}

#[test]
fn every_agent_that_supports_mcp_resolves_a_config_path() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        assert_eq!(
            adapter.supports_mcp(),
            paths.mcp_config.is_some(),
            "{} disagrees with itself about MCP support",
            adapter.slug
        );
    }
}

#[test]
fn the_harness_can_be_declined_without_declining_the_skill() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let result = setup(
        adapter,
        &Config::default(),
        &SetupOptions {
            skip_harness: true,
            ..options.clone()
        },
    )
    .expect("setup succeeds");
    assert!(
        result
            .actions
            .iter()
            .all(|action| action.kind != ActionKind::Hooks && action.kind != ActionKind::McpServer),
        "the harness was installed after being declined"
    );
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(
        skill::presence(&paths.skill_root, &Config::default()),
        skill::Presence::Current
    );
}

#[test]
fn being_told_about_estigia_and_being_stopped_by_it_are_different_questions() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    assert!(!is_configured(adapter, &options));
    assert!(!is_gated(adapter, &options));
    assert!(!exposes_tools(adapter, &options));

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(is_configured(adapter, &options));
    assert!(is_gated(adapter, &options));
    assert!(exposes_tools(adapter, &options));

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!is_gated(adapter, &options));
    assert!(!exposes_tools(adapter, &options));
}

#[test]
fn an_agent_installed_without_a_directive_is_still_one_estigia_is_in() {
    // `--skill-only` leaves a skill installed, a gate registered and an MCP
    // server exposed, with the instruction file untouched — which `is_configured`
    // calls not configured, and `doctor` once filtered on it.
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    assert!(!is_present(adapter, &options), "nothing is installed yet");

    setup(
        adapter,
        &Config::default(),
        &SetupOptions {
            skip_directive: true,
            ..options.clone()
        },
    )
    .expect("setup succeeds");

    assert!(!is_configured(adapter, &options), "the flag did its job");
    assert!(
        is_present(adapter, &options),
        "an agent whose gate Estigia registered is not one Estigia is absent from"
    );

    // And the check that used to be skipped now runs and names the contract.
    let paths = resolve_paths(adapter, &options).expect("paths");
    let checks = crate::harness::doctor::full(
        Some(&paths.skill_root),
        _home.path(),
        &Config::default().tracker,
        &options,
    );
    let contract = checks
        .iter()
        .find(|check| check.name == "contract")
        .expect("doctor asks about the contract");
    assert!(
        !format!("{:?}", contract.health).contains("no agent is configured"),
        "doctor still says nothing reads a contract: {:?}",
        contract.health
    );
}

#[test]
fn declining_the_harness_leaves_the_agent_told_but_not_gated() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(
        adapter,
        &Config::default(),
        &SetupOptions {
            skip_harness: true,
            ..options.clone()
        },
    )
    .expect("setup succeeds");
    assert!(
        is_configured(adapter, &options),
        "the directive was written"
    );
    assert!(
        !is_gated(adapter, &options),
        "the gate was installed anyway"
    );
    assert!(!exposes_tools(adapter, &options));
}

#[test]
fn the_only_agents_without_a_gate_are_the_neutral_root_zcode_and_deepseek_harness() {
    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let (_home, options) = sandbox();
                setup(adapter, &Config::default(), &options).expect("setup succeeds");
                if adapter.can_gate_tools() {
                    assert!(
                        is_gated(adapter, &options),
                        "{} reports no gate",
                        adapter.slug
                    );
                } else {
                    // The phrase each one's reason has to carry; both survive deleting
                    // an arm, so the phrase is pinned rather than trusted.
                    let must_say = match adapter.slug {
                        "agents" => "not an agent",
                        "zcode" => "hooks.enabled",
                        "deepseek-harness" => "shipped harness",
                        other => panic!("{other} is ungated and nobody planned for it"),
                    };
                    assert!(
                        adapter.gate_gap().is_some_and(|gap| gap.contains(must_say)),
                        "{}'s gate gap no longer says {must_say:?}: {:?}",
                        adapter.slug,
                        adapter.gate_gap()
                    );
                }
            });
        }
    });
}

#[test]
fn a_file_that_still_holds_the_operator_s_own_text_survives() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    fs::create_dir_all(paths.instructions.parent().expect("a parent")).expect("create the dir");
    let theirs = "# my own instructions

Do the thing my way.
";
    fs::write(&paths.instructions, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        fs::read_to_string(&paths.instructions)
            .expect("the directive was written")
            .contains(STANDING_DELEGATION_GRANT),
        "the standing-delegation grant is not in the installed block, so removing it proves nothing"
    );
    uninstall(adapter, &options).expect("uninstall succeeds");
    let after = fs::read_to_string(&paths.instructions).expect("their file survives");
    assert!(
        !after.contains(STANDING_DELEGATION_GRANT),
        "the standing-delegation grant outlived the uninstall"
    );
    assert_eq!(after, theirs);
}

#[test]
fn a_file_estigia_created_that_somebody_then_wrote_in_is_kept() {
    // Ownership by name is not a licence to delete.
    let (_home, options) = sandbox();
    let adapter = agent("cursor");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    let theirs = "# my own rule\n\nalways use tabs\n";
    let existing = fs::read_to_string(&paths.instructions).expect("read");
    fs::write(&paths.instructions, format!("{theirs}\n{existing}")).expect("write");

    uninstall(adapter, &options).expect("uninstall succeeds");
    let after = fs::read_to_string(&paths.instructions).expect("their text survives");
    assert!(after.contains("always use tabs"));
    assert!(!after.contains("ESTIGIA"));
}

#[test]
fn opencode_is_gated_by_a_plugin_because_that_is_what_it_reads() {
    // The second gate mechanism; two shapes, one question.
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let file = paths.plugin.expect("OpenCode loads plugins");

    assert!(!adapter.supports_hooks(), "it has no settings hook");
    assert!(adapter.can_gate_tools(), "and it can still be gated");
    assert!(!is_gated(adapter, &options));

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(file.is_file(), "{} was not written", file.display());
    assert!(is_gated(adapter, &options));

    let written = fs::read_to_string(&file).expect("the plugin is readable");
    // It decides nothing: everything it knows is how to ask the binary.
    assert!(written.contains("tool.execute.before"));
    assert!(written.contains("gate"));
    assert!(written.contains("throw new Error"));
    // And it says what it cannot see, beside the code that cannot see it.
    assert!(
        written.contains("subagents"),
        "the known hole is undeclared"
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!file.exists(), "the plugin survived an uninstall");
    assert!(!is_gated(adapter, &options));
}

#[test]
fn a_plugin_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let file = paths.plugin.expect("OpenCode loads plugins");
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export const Theirs = async () => ({});\n";
    fs::write(&file, theirs).expect("write their plugin");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(
        fs::read_to_string(&file).expect("their plugin survives"),
        theirs
    );
    // And `status` says the gate is off, rather than claiming a plugin that
    // does nothing for Estigia is one that does.
    assert!(!is_gated(adapter, &options));

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&file).expect("their plugin survives"),
        theirs
    );
}

/// [`Config::default`] with the loop plugin switched on.
fn loop_on() -> Config {
    Config {
        loop_plugin: crate::config::LoopPlugin::On,
        ..Config::default()
    }
}

#[test]
fn opencode_installs_the_loop_plugin_only_when_the_setting_is_on() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    // The path is named whether or not the plugin is installed, so an uninstall
    // can find a file an earlier install wrote.
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    assert!(
        !loop_file.exists(),
        "a named path was written by resolving it"
    );

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let gate = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .plugin
        .expect("OpenCode loads plugins");
    let gate_bytes = fs::read_to_string(&gate).expect("the gate plugin reads");
    assert!(
        !loop_file.exists(),
        "the default setting wrote the loop plugin: {}",
        loop_file.display()
    );

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    assert!(
        loop_file.is_file(),
        "{} was not written",
        loop_file.display()
    );
    let written = fs::read_to_string(&loop_file).expect("the loop plugin reads");
    assert!(
        written.contains(plugin::LOOP_MARKER),
        "the loop plugin carries no marker"
    );
    assert_eq!(
        fs::read_to_string(&gate).expect("the gate plugin reads"),
        gate_bytes,
        "a loop install changed the gate plugin"
    );

    setup(adapter, &Config::default(), &options).expect("switching the setting off succeeds");
    assert_eq!(
        fs::read_to_string(&gate).expect("the gate plugin reads"),
        gate_bytes,
        "writing the gate with the loop setting off changed it"
    );
    assert!(
        !loop_file.exists(),
        "switching the setting off left the loop plugin it wrote: {}",
        loop_file.display()
    );

    // And back on, so the plain uninstall below meets an owned loop plugin
    // rather than an already-deleted path.
    setup(adapter, &loop_on(), &options).expect("the second loop install succeeds");
    assert!(loop_file.is_file(), "the loop plugin was not written again");

    uninstall(adapter, &options).expect("the plain uninstall succeeds");
    assert!(!gate.exists(), "the gate plugin survived a plain uninstall");
    assert!(
        !loop_file.exists(),
        "a plain uninstall left the loop plugin behind: {}",
        loop_file.display()
    );
}

#[test]
fn a_loop_plugin_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    fs::create_dir_all(loop_file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default { id: 'theirs' };\n";
    fs::write(&loop_file, theirs).expect("write their loop plugin");

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_eq!(
        fs::read_to_string(&loop_file).expect("their loop plugin survives"),
        theirs
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&loop_file).expect("their loop plugin survives"),
        theirs
    );
}

#[test]
fn claude_installs_the_loop_commands_only_when_the_setting_is_on() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let directory = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .commands
        .expect("Claude Code names a commands directory");
    let files: Vec<std::path::PathBuf> = plugin::LOOP_COMMANDS
        .iter()
        .map(|(name, _)| directory.join(name))
        .collect();
    for file in &files {
        assert!(
            !file.exists(),
            "a named path was written by resolving it: {}",
            file.display()
        );
    }

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    for file in &files {
        assert!(
            !file.exists(),
            "the default setting wrote a loop command: {}",
            file.display()
        );
    }

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    for (file, (_, source)) in files.iter().zip(plugin::LOOP_COMMANDS) {
        let written = fs::read_to_string(file).expect("the command reads");
        assert_eq!(written, *source);
        assert!(
            written.contains(plugin::COMMAND_MARKER),
            "the command carries no marker: {}",
            file.display()
        );
    }

    setup(adapter, &Config::default(), &options).expect("switching off succeeds");
    for file in &files {
        assert!(
            !file.exists(),
            "switching off left a loop command: {}",
            file.display()
        );
    }

    setup(adapter, &loop_on(), &options).expect("the second loop install succeeds");
    uninstall(adapter, &options).expect("the plain uninstall succeeds");
    for file in &files {
        assert!(
            !file.exists(),
            "a plain uninstall left a loop command: {}",
            file.display()
        );
    }
}

#[test]
fn claude_installs_the_changes_command_whatever_the_loop_setting_says() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let directory = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .commands
        .expect("Claude Code names a commands directory");
    let files: Vec<std::path::PathBuf> = plugin::CHANGES_COMMANDS
        .iter()
        .map(|(name, _)| directory.join(name))
        .collect();
    assert_eq!(files.len(), 1);

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    for (file, (_, source)) in files.iter().zip(plugin::CHANGES_COMMANDS) {
        let written = fs::read_to_string(file).expect("the command reads");
        assert_eq!(written, *source);
        assert!(
            written.contains(plugin::COMMAND_MARKER),
            "{}",
            file.display()
        );
        assert!(written.contains("estigia changes"), "{}", file.display());
    }

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    setup(adapter, &Config::default(), &options).expect("switching the loop off succeeds");
    for file in &files {
        assert!(
            file.exists(),
            "switching the loop off took back {}",
            file.display()
        );
    }

    uninstall(adapter, &options).expect("uninstall succeeds");
    for file in &files {
        assert!(!file.exists(), "uninstall left {}", file.display());
    }
}

#[test]
fn a_changes_command_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let directory = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .commands
        .expect("Claude Code names a commands directory");
    fs::create_dir_all(&directory).expect("create the commands directory");
    let theirs = "---\ndescription: theirs\n---\n\ntheir own command\n";
    let file = directory.join(plugin::CHANGES_COMMANDS[0].0);
    fs::write(&file, theirs).expect("write their command");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&file).expect("theirs survives"), theirs);

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&file).expect("theirs survives"), theirs);
}

#[test]
fn a_loop_command_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let directory = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .commands
        .expect("Claude Code names a commands directory");
    fs::create_dir_all(&directory).expect("create the commands directory");
    let theirs = "---\ndescription: theirs\n---\n\ntheir own command\n";
    let file = directory.join(plugin::LOOP_COMMANDS[0].0);
    fs::write(&file, theirs).expect("write their command");

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_eq!(
        fs::read_to_string(&file).expect("their command survives"),
        theirs
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&file).expect("their command survives"),
        theirs
    );
}

#[test]
fn the_loop_plugin_has_no_done_input() {
    let source = crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia"));
    for forbidden in ["<promise>", "--done", "done_input"] {
        assert!(
            !source.contains(forbidden),
            "the loop plugin exposes a caller-supplied completion surface: {forbidden}"
        );
    }
}

#[test]
fn only_the_agents_that_can_deny_claim_they_can() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let has_mechanism = paths.hooks.is_some() || paths.plugin.is_some();
        assert_eq!(
            adapter.can_gate_tools(),
            has_mechanism,
            "{} disagrees with itself about whether it can be gated",
            adapter.slug
        );
    }
}

#[test]
fn every_dialect_denies_in_its_own_words() {
    use crate::harness::hook::{Dialect, response_in};
    let refusal = crate::outcome::Refusal::not_started(
        "not-current-live-holder",
        "the claim is not yours",
        Resolution::run("estigia status"),
    );
    let deny = crate::harness::Decision::Deny(Box::new(refusal));

    let claude = response_in(Dialect::ClaudeCode, &deny);
    assert_eq!(
        claude["hookSpecificOutput"]["permissionDecision"], "deny",
        "Claude Code reads a permissionDecision"
    );

    let gemini = response_in(Dialect::GeminiCli, &deny);
    assert_eq!(gemini["decision"], "deny");
    assert!(
        gemini["reason"].as_str().is_some_and(|r| !r.is_empty()),
        "Gemini requires `reason` when denied — it is delivered as a tool error"
    );

    let cursor = response_in(Dialect::Cursor, &deny);
    assert_eq!(cursor["permission"], "deny");
    assert!(
        cursor["agent_message"]
            .as_str()
            .is_some_and(|m| m.contains("not-current-live-holder"))
    );
    assert!(
        cursor["user_message"]
            .as_str()
            .is_some_and(|m| !m.is_empty())
    );

    // And standing aside is an empty object everywhere: an explicit allow would
    // override the operator's own permission settings.
    for dialect in [Dialect::ClaudeCode, Dialect::GeminiCli, Dialect::Cursor] {
        assert_eq!(
            response_in(
                dialect,
                &crate::harness::Decision::Outside(crate::harness::Aside::NothingSworn)
            ),
            serde_json::json!({}),
            "{dialect:?} does not stand aside quietly"
        );
    }
}

#[test]
fn every_gated_agent_registers_its_own_event_and_dialect() {
    std::thread::scope(|scope| {
        for adapter in AGENTS.iter().filter(|a| a.supports_hooks()) {
            scope.spawn(|| {
                let (_home, options) = sandbox();
                let paths = resolve_paths(adapter, &options).expect("paths resolve");
                let hooks = paths.hooks.expect("a gated agent has a hooks file");
                setup(adapter, &Config::default(), &options).expect("setup succeeds");

                let written = fs::read_to_string(&hooks).expect("the hooks file is readable");
                let spec = adapter.gate_spec().expect("a gated agent has a spec");
                assert!(
                    spec.events.iter().all(|event| written.contains(event)),
                    "{} registered no {} entry",
                    adapter.slug,
                    spec.events.join(", ")
                );
                assert!(
                    written.contains(&format!("--dialect {}", spec.dialect.slug())),
                    "{} would be answered in the wrong dialect",
                    adapter.slug
                );
                assert!(
                    is_gated(adapter, &options),
                    "{} reports no gate",
                    adapter.slug
                );

                uninstall(adapter, &options).expect("uninstall succeeds");
                assert!(
                    !is_gated(adapter, &options),
                    "{} kept its gate",
                    adapter.slug
                );
            });
        }
    });
}

#[test]
fn a_gated_agent_s_hooks_file_keeps_everybody_else_s_entries() {
    let (_home, options) = sandbox();
    for slug in ["claude-code", "cursor", "codex"] {
        let adapter = agent(slug);
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let hooks = paths.hooks.expect("a gated agent has a hooks file");
        fs::create_dir_all(hooks.parent().expect("a parent")).expect("create the dir");
        fs::write(
            &hooks,
            r#"{"theirs":true,"hooks":{"sessionStart":[{"command":"./theirs.sh"}]}}"#,
        )
        .expect("write their file");

        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        uninstall(adapter, &options).expect("uninstall succeeds");

        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
        assert_eq!(after["theirs"], true, "{slug} dropped their setting");
        assert_eq!(
            after["hooks"]["sessionStart"][0]["command"], "./theirs.sh",
            "{slug} dropped their hook"
        );
    }
}

#[test]
fn every_agent_without_a_gate_says_why_and_what_would_close_it() {
    for adapter in AGENTS {
        match (adapter.can_gate_tools(), adapter.gate_gap()) {
            (true, None) => {}
            (false, Some(gap)) => {
                assert!(
                    gap.len() > 60,
                    "{} says it cannot be gated without saying why",
                    adapter.slug
                );
                assert!(
                    !gap.to_ascii_lowercase().contains("not supported"),
                    "{} names a dead end instead of what is missing",
                    adapter.slug
                );
            }
            (gated, gap) => panic!(
                "{} disagrees with itself: can_gate_tools={gated}, gate_gap={:?}",
                adapter.slug,
                gap.is_some()
            ),
        }
    }
}

#[test]
fn the_gate_gap_and_the_gate_never_disagree() {
    for adapter in AGENTS {
        assert_eq!(
            adapter.can_gate_tools(),
            adapter.gate_gap().is_none(),
            "{} disagrees with itself",
            adapter.slug
        );
    }
}

#[test]
fn the_tools_gap_and_the_tools_never_disagree() {
    for adapter in AGENTS {
        assert_eq!(
            adapter.supports_mcp(),
            adapter.tools_gap().is_none(),
            "{} disagrees with itself",
            adapter.slug
        );
    }
}

#[test]
fn each_gated_agent_gets_its_own_envelope_and_not_somebody_else_s() {
    // A Codex entry written into Claude Code's envelope parses, looks right,
    // and never fires.
    let (_home, options) = sandbox();
    let read = |slug: &str| {
        let adapter = agent(slug);
        let hooks = resolve_paths(adapter, &options)
            .expect("paths resolve")
            .hooks
            .expect("a gated agent has a hooks file");
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&hooks).expect("the hooks file is readable"),
        )
        .expect("valid json")
    };

    // Codex: wrapped, like Claude Code, because it reads the same shape.
    let codex = read("codex");
    assert!(
        codex["hooks"]["PreToolUse"].is_array(),
        "codex lost the wrapper it refuses to start without: {codex}"
    );
    assert!(
        codex.get("PreToolUse").is_none(),
        "codex still carries an event at the top level, which it refuses: {codex}"
    );
    // Not just the shell: `apply_patch` is how Codex writes files.
    assert_eq!(
        codex["hooks"]["PreToolUse"][0]["matcher"],
        "^(Bash|apply_patch|Write|Edit)$"
    );
    assert_eq!(
        codex["hooks"]["PreToolUse"][0]["hooks"][0]["type"],
        "command"
    );

    let claude = read("claude-code");
    assert!(
        claude["hooks"]["PreToolUse"].is_array(),
        "claude-code lost its wrapper: {claude}"
    );
    assert_eq!(
        claude["hooks"]["PreToolUse"][0]["hooks"][0]["type"],
        "command"
    );

    // Cursor: wrapped, versioned, and the command straight on the entry.
    let cursor = read("cursor");
    assert_eq!(cursor["version"], 1);
    assert!(
        cursor["hooks"]["preToolUse"][0]["command"].is_string(),
        "cursor's entry is not flat: {cursor}"
    );
}

#[test]
fn only_claude_code_is_given_lifecycle_hooks() {
    // Codex shares Claude Code's *dialect* and not its lifecycle. Writing
    // `SessionStart` into a file for an agent that does not have it is an entry
    // nobody runs, in somebody else's settings.
    let (_home, options) = sandbox();
    for slug in ["codex", "cursor"] {
        let adapter = agent(slug);
        let hooks = resolve_paths(adapter, &options)
            .expect("paths resolve")
            .hooks
            .expect("a gated agent has a hooks file");
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        let written = fs::read_to_string(&hooks).expect("readable");
        assert!(
            !written.contains("session-start"),
            "{slug} was given a lifecycle hook it does not have: {written}"
        );
    }
}

#[test]
fn every_agent_names_whether_a_hook_can_continue_its_turn() {
    // The reading, in the gate's own words: three hosts have a
    // continue surface — Claude Code's stop-hook, OpenCode's optional loop
    // plugin and Codex's Stop hook under the Loop plugin setting — five cannot,
    // two are unknown pending the reference that would settle them.
    for (slug, word) in [
        ("claude-code", "able: stop-hook"),
        ("gemini-cli", "none: a denial surfaces as a tool error"),
        ("qwen", "none: a denial surfaces as a tool error"),
        ("windsurf", "none: exit 2 feeds back into the current turn"),
        (
            "crush",
            "none: exit 2 blocks the tool, exit 49 halts the turn",
        ),
        ("cline", "none: exits 0 and routes to a person"),
        ("opencode", "able: loop plugin, when installed"),
        (
            "codex",
            "able: stop-hook, when the Loop plugin setting is on",
        ),
        ("continue", "unknown: hooks reference not retrieved"),
        ("cursor", "unknown: stop hook exists, powers unverified"),
        ("agents", "none: no tool-call gate"),
        ("zcode", "none: no tool-call gate"),
        ("deepseek-harness", "none: no tool-call gate"),
    ] {
        assert_eq!(agent(slug).continuation(), word, "{slug}");
    }

    // And the partition, so a newly verified surface lands here beside the
    // reference that verified it rather than drifting in silently.
    let gated: Vec<&str> = crate::setup::every_adapter()
        .filter(|adapter| adapter.can_gate_tools())
        .map(|adapter| adapter.continuation())
        .collect();
    assert_eq!(
        gated.iter().filter(|word| word.starts_with("able")).count(),
        3,
        "the continue surfaces moved without this partition moving with them: {gated:?}"
    );
    assert_eq!(
        gated
            .iter()
            .filter(|word| word.starts_with("unknown"))
            .count(),
        2,
        "an unknown settled or a new one opened without updating this: {gated:?}"
    );
}

fn hook_commands(written: &serde_json::Value, event: &str) -> Vec<String> {
    written["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
        .filter_map(|hook| hook["command"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn stop_and_session_start_reach_claude_code_always_and_codex_only_under_the_loop_setting() {
    for (config, codex_gets_them) in [(Config::default(), false), (loop_on(), true)] {
        let reached = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for adapter in crate::setup::every_adapter().filter(|a| a.can_gate_tools()) {
                let config = &config;
                let reached = &reached;
                scope.spawn(move || {
                    reached.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let (_home, options) = sandbox();
                    let slug = adapter.slug;
                    if adapter.retired() {
                        install_as_an_earlier_build(adapter, &options);
                    } else {
                        setup(adapter, config, &options).expect("setup succeeds");
                    }
                    let paths = resolve_paths(adapter, &options).expect("paths resolve");
                    let text = fs::read_to_string(
                        paths
                            .hooks
                            .as_ref()
                            .or(paths.plugin.as_ref())
                            .expect("a gated agent has a hooks file or a plugin"),
                    )
                    .expect("readable");
                    match slug {
                        "claude-code" => {
                            let written: serde_json::Value =
                                serde_json::from_str(&text).expect("valid json");
                            let entry = &written["hooks"]["Stop"][0];
                            assert!(
                                entry.get("matcher").is_none(),
                                "Stop fires once per turn, not per tool: {entry}"
                            );
                            let stop = hook_commands(&written, "Stop");
                            if codex_gets_them {
                                assert_eq!(entry["hooks"][0]["timeout"], 180);
                                assert!(
                                    stop.iter().all(|command| command.ends_with(
                                        " hook stop --agent claude-code --dialect claude-code"
                                    )),
                                    "{written}"
                                );
                            } else {
                                assert_eq!(entry["hooks"][0]["timeout"], 5);
                                assert!(
                                    stop.iter().all(|command| command.ends_with(" hook stop")),
                                    "{written}"
                                );
                            }
                            assert_eq!(stop.len(), 1, "{written}");
                            assert_eq!(
                                hook_commands(&written, "SessionStart").len(),
                                1,
                                "{written}"
                            );
                        }
                        "codex" => {
                            let written: serde_json::Value =
                                serde_json::from_str(&text).expect("valid json");
                            let stop = hook_commands(&written, "Stop");
                            let start = hook_commands(&written, "SessionStart");
                            if codex_gets_them {
                                assert_eq!(stop.len(), 1, "{written}");
                                assert!(
                                    stop[0].ends_with(
                                        " hook stop --agent codex --dialect claude-code"
                                    ),
                                    "{stop:?}"
                                );
                                assert_eq!(start.len(), 1, "{written}");
                                assert!(
                                    start[0].ends_with(
                                        " hook session-start --agent codex --dialect claude-code"
                                    ),
                                    "{start:?}"
                                );
                                assert!(
                                    written["hooks"]["Stop"][0].get("matcher").is_none(),
                                    "{written}"
                                );
                            } else {
                                assert!(stop.is_empty() && start.is_empty(), "{written}");
                            }
                        }
                        _ => {
                            assert!(
                                !text.contains("hook stop") && !text.contains("hook session-start"),
                                "{slug} was given a lifecycle hook that was never verified: {text}"
                            );
                            assert!(
                                is_gated(adapter, &options),
                                "{slug} lost its gate while gaining no lifecycle hook"
                            );
                        }
                    }
                });
            }
        });
        assert_eq!(
            reached.load(std::sync::atomic::Ordering::Relaxed),
            crate::setup::every_adapter()
                .filter(|a| a.can_gate_tools())
                .count(),
            "the walk skipped a gated agent"
        );
    }
}

/// The lifecycle is declared per adapter; this holds it declared, and holds
/// that the two things it used to be deduced from no longer pick the agent out.
#[test]
fn the_lifecycle_belongs_to_one_agent_and_is_not_deduced_from_two_others() {
    use crate::harness::hook::Dialect;
    use crate::setup::render::Envelope;

    let carrying: Vec<&str> = AGENTS
        .iter()
        .filter(|adapter| adapter.gate_spec().is_some_and(|spec| spec.lifecycle))
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(
        carrying,
        vec!["claude-code"],
        "the lifecycle events reach an agent that does not have them"
    );

    // And the pair it was deduced from no longer picks that agent out.
    let sharing: Vec<&str> = AGENTS
        .iter()
        .filter(|adapter| {
            adapter.gate_spec().is_some_and(|spec| {
                spec.envelope == Envelope::Settings && spec.dialect == Dialect::ClaudeCode
            })
        })
        .map(|adapter| adapter.slug)
        .collect();
    assert!(
        sharing.len() > 1,
        "the old deduction still happens to be right, so this guard proves nothing yet: {sharing:?}"
    );
}

/// Codex refuses a file whose events sit at the top level; it reads `Settings`,
/// like Claude Code.
#[test]
fn the_codex_hooks_file_is_the_shape_codex_will_start_with() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let hooks = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .hooks
        .expect("codex is gated");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("readable")).expect("valid json");

    // The two fields Codex names in its refusal, and nothing else at the top.
    let top = written.as_object().expect("an object");
    for key in top.keys() {
        assert!(
            key == "hooks" || key == "description",
            "codex refuses to start on an unknown top-level field: {key}"
        );
    }
    assert!(top.contains_key("hooks"), "{written}");

    // And under it, the matcher group and handler Codex's own schema declares.
    let group = &written["hooks"]["PreToolUse"][0];
    assert!(group["matcher"].is_string(), "{written}");
    assert_eq!(group["hooks"][0]["type"], "command");
    assert!(
        group["hooks"][0]["command"]
            .as_str()
            .is_some_and(|command| command.contains("hook pre-tool-use")),
        "{written}"
    );

    // Codex has no lifecycle events, so the corrected envelope must not have
    // handed it Claude Code's.
    assert_eq!(
        written["hooks"].as_object().map(|hooks| hooks.len()),
        Some(1),
        "codex was given an event it does not have: {written}"
    );
}

/// What Estigia writes into an adapter's own file is what that file can hold,
/// checked from **both** ends: the rows against `Scope`, and the file handed to
/// the very check that condemned it.
#[test]
fn an_adapters_own_file_carries_only_what_an_adapter_can_answer() {
    use crate::config::{Scope, Setting};

    let (home, options) = sandbox();
    // An adapter that shares the neutral root, so it has a file of its own.
    let adapter = agent("zcode");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let root = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .skill_root;
    let own = crate::skill::agent_override(&root, adapter.slug);

    // Written by the call the install path makes, through **both** of its
    // branches: the file that does not exist yet, and the one that does. Two
    // pieces of code build this file, and only one of them was wrong once.
    write_agent_configuration_wholly(&own, adapter.slug, &Config::default()).expect("a fresh file");
    let fresh = fs::read_to_string(&own).expect("the adapter's own file");
    write_agent_configuration_wholly(&own, adapter.slug, &Config::default())
        .expect("an existing file");
    let written = fs::read_to_string(&own).expect("the adapter's own file");
    assert_eq!(
        crate::config::table_rows(&fresh),
        crate::config::table_rows(&written),
        "the two write paths build different tables, so which ran last decides what is seen"
    );

    let rows = crate::config::table_rows(&written);
    assert!(!rows.is_empty(), "no table was written at all: {written}");
    for (label, _) in &rows {
        let setting = Setting::from_label(label)
            .unwrap_or_else(|| panic!("{label} is not a setting Estigia publishes"));
        assert_eq!(
            setting.scope(),
            Scope::Agent,
            "{label} is a fact about the repository and was written where nothing reads it"
        );
    }
    // Every row that *can* be answered here is, or the file quietly stops
    // offering half of what it is for. A row whose `applies_to_agent` is false
    // is not one this adapter can answer, so it is not owed a place here.
    let answerable = crate::config::AGENT_SETTINGS
        .iter()
        .filter(|setting| setting.applies_to_agent(adapter))
        .count();
    assert_eq!(
        rows.len(),
        answerable,
        "the adapter's own file does not carry every per-agent row: {written}"
    );

    // And the check that condemned this file agrees now. Crossed rather than
    // reasoned about: the two halves are in different modules, and one of them
    // was already right while the other wrote the file it complains about.
    let condemned: Vec<String> = crate::harness::doctor::examine(Some(&root), home.path())
        .into_iter()
        .filter_map(|check| match check.health {
            crate::harness::doctor::Health::Broken { detail, .. }
                if detail.contains("that nothing reads") =>
            {
                Some(detail)
            }
            _ => None,
        })
        .collect();
    assert!(
        condemned.is_empty(),
        "doctor calls a file Estigia just wrote broken: {condemned:#?}"
    );
}

#[test]
fn the_codex_matcher_covers_how_codex_writes_files() {
    // It was `^Bash$` for one round, taken from a published claim that
    // `PreToolUse` intercepts the shell tool only. `core/src/hook_runtime.rs`
    // says otherwise, and `core/src/tools/hook_names.rs` gives the canonical
    // names: `apply_patch`, carrying the aliases `Write` and `Edit`. A gate on
    // shell alone watches the one thing a delivery does least.
    let spec = agent("codex").gate_spec().expect("codex is gated");
    let matcher = spec.matcher.expect("codex filters by tool");
    for name in ["Bash", "apply_patch", "Write", "Edit"] {
        assert!(
            matcher.contains(name),
            "codex's matcher misses {name}: {matcher}"
        );
    }
}

/// The entry's **first paragraph**, which is the part that states the claim.
fn fires_on_every_tool() -> String {
    let section = crate::test_env::honesty_contract();
    let entry = section
        .split("\n- ")
        .find(|entry| entry.contains("fire on every tool"))
        .expect("the honesty contract names the gates that fire on every tool");
    entry
        .split("\n\n")
        .next()
        .unwrap_or(entry)
        .replace("\r", "")
}

#[test]
fn every_matcher_names_tools_the_gate_can_classify() {
    // A matcher names the tools an agent wakes the hook for; every one has to
    // be a name `harness::classify` recognises, or the gate is theatre. The
    // walk covers every gated agent, including the two gated through a file
    // Estigia owns whole.
    let mut reached = 0;
    for adapter in crate::setup::every_adapter().filter(|a| a.can_gate_tools()) {
        reached += 1;
        let Some(spec) = adapter.gate_spec() else {
            // A plugin that carries a `GATED` array names its own tools and is
            // crossed elsewhere; one that does not fires on everything.
            let written = match adapter.instructions {
                InstructionFile::Cline => {
                    plugin::cline_hook(std::path::Path::new("/opt/estigia"), false)
                }
                _ => plugin::source(std::path::Path::new("/opt/estigia")),
            };
            if !written.contains("const GATED = [") {
                assert!(
                    fires_on_every_tool().to_lowercase().contains(adapter.slug),
                    "{} gates through a script that narrows nothing, so only the classifier's \
                     list decides what it watches \u{2014} and the entry naming the gates that \
                     fire on every tool does not name it",
                    adapter.slug
                );
            }
            continue;
        };
        let Some(matcher) = spec.matcher else {
            assert!(
                crate::test_env::honesty_contract().contains("fire on every tool"),
                "{} registers a gate with no matcher, so only the classifier's list narrows \
                 it \u{2014} and the honesty contract does not say so",
                adapter.slug
            );
            continue;
        };
        for name in matcher
            .trim_start_matches('^')
            .trim_end_matches('$')
            .trim_start_matches('(')
            .trim_end_matches(')')
            .split('|')
        {
            let name = name.trim();
            let (action, _) = crate::harness::classify(
                name,
                &serde_json::json!({"command": "git commit -m x", "file_path": "a.rs"}),
            );
            if action == crate::harness::Action::Untouched {
                assert!(
                    crate::harness::hook::is_prelaunch_tool(Some(adapter.slug), name),
                    "{} wakes the hook for `{name}`, but neither the repository classifier nor \
                     the dedicated prelaunch gate owns it",
                    adapter.slug
                );
            }
        }
    }
    // How many it walked, because narrowing the walk is how the gap got in.
    assert_eq!(
        reached,
        crate::setup::every_adapter()
            .filter(|a| a.can_gate_tools())
            .count(),
        "the walk skipped a gated agent"
    );
    assert!(
        reached >= 10,
        "only {reached} gated agent(s) were reached — this stopped covering the fleet"
    );
}

#[test]
fn claude_matcher_wakes_current_and_legacy_reserved_launch_tools() {
    let matcher = agent("claude-code")
        .gate_spec()
        .and_then(|spec| spec.matcher)
        .expect("Claude narrows PreToolUse");
    for tool in ["Agent", "Task"] {
        assert!(matcher.split('|').any(|name| name == tool), "{matcher}");
        assert!(crate::harness::hook::is_prelaunch_tool(
            Some("claude-code"),
            tool
        ));
        assert!(!crate::harness::hook::is_prelaunch_tool(
            Some("opencode"),
            tool
        ));
    }
}

#[test]
fn the_opencode_plugin_only_names_tools_the_gate_can_classify() {
    // Same seam, through the other mechanism.
    let source = crate::setup::plugin::source(std::path::Path::new("/usr/local/bin/estigia"));
    let gated = source
        .split_once("const GATED = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(list, _)| list.to_owned())
        .expect("the plugin declares which tools it gates");
    for name in gated.split(',') {
        let name = name.trim().trim_matches('"');
        if name.is_empty() {
            continue;
        }
        let (action, _) = crate::harness::classify(
            name,
            &serde_json::json!({"command": "git commit -m x", "file_path": "a.rs"}),
        );
        assert_ne!(
            action,
            crate::harness::Action::Untouched,
            "the OpenCode plugin gates `{name}` and the classifier does not know it"
        );
    }
}

/// Each rule the directive states, and the test that holds the gate to it.
/// Named rather than paraphrased: a test whose name changes takes its rule
/// with it, and a rule with no test is one nobody is holding.
const DIRECTIVE_RULES: &[(&str, &str)] = &[
    (
        "an unreadable control surface permits no write",
        "harness::tests::an_unreadable_control_surface_permits_no_write",
    ),
    (
        "Every push invalidates",
        "harness::tests::a_boundary_never_rides_on_the_window",
    ),
    (
        "Never infer that you hold an issue",
        "harness::tests::the_gate_never_writes_to_the_tracker",
    ),
    (
        "Never report the nearest named state",
        "harness::tracker::tests::an_exit_code_nobody_has_seen_fails_towards_unknown",
    ),
];

#[test]
fn every_rule_the_directive_states_is_one_the_gate_keeps() {
    // The directive promises three things to every agent, on every turn. Each
    // has to be a sentence some test holds the gate to, and each test has to
    // exist under the name given here — a renamed test silently unhooks its
    // rule.
    let sources = [
        crate::source_text::code_of(include_str!("../harness/tests.rs")),
        crate::source_text::code_of(include_str!("../harness/tracker.rs")),
    ];
    // Compared on words, not on bytes: the directive is prose and it wraps, so
    // a rule can sit across a line break without meaning anything different.
    let stated = DIRECTIVE_TEMPLATE
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for (rule, test) in DIRECTIVE_RULES {
        assert!(
            stated.contains(rule),
            "the directive no longer says `{rule}`, and `{test}` still holds the gate to it"
        );
        let name = test.rsplit("::").next().unwrap_or(test);
        assert!(
            sources
                .iter()
                .any(|source| source.contains(&format!("fn {name}("))),
            "the directive promises `{rule}` and `{test}` does not exist — the rule is held by \
             nobody"
        );
    }
}

#[test]
fn the_directive_leaves_the_pre_write_verification_to_the_gate() {
    let stated = DIRECTIVE_TEMPLATE
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        !stated.contains("before the first repository write"),
        "the directive asks for a `verify_claim` the gate already performs"
    );
    assert!(
        stated.contains("The gate re-verifies the claim"),
        "the directive no longer says the gate re-verifies the claim"
    );
}

#[test]
fn the_directive_promises_nothing_the_harness_does_not_do() {
    // The other direction, and the one that cannot be automated: a rule added
    // to the directive without a line here is a promise with no test behind it.
    // Counting is the most this can check — the reading is a person's job, and
    // saying so beats a guard that pretends otherwise.
    let stated = DIRECTIVE_TEMPLATE
        .lines()
        .filter(|line| line.trim_start().starts_with(|c: char| c.is_ascii_digit()))
        .count();
    assert_eq!(
        stated,
        3,
        "the directive states {stated} rules and {} are held by a named test; a rule added here \
         needs a line in DIRECTIVE_RULES and a test to point it at",
        DIRECTIVE_RULES.len()
    );
}

/// The shared body of the two home-containment tests below: every one of
/// [`AgentPaths`]' path fields, destructured out of the struct so a field added
/// to it fails this function to compile until it is named.
fn assert_every_root_resolves_under_home(
    home: &Path,
    options_for: impl Fn(Platform) -> SetupOptions,
    message_tail: &str,
) {
    for adapter in AGENTS {
        for platform in [Platform::Windows, Platform::MacOs, Platform::Unix] {
            let options = options_for(platform);
            let paths = resolve_paths(adapter, &options).expect("the paths resolve");
            let AgentPaths {
                skill_root,
                instructions,
                hooks,
                mcp_config,
                plugin,
                loop_plugin,
                commands: _,
                prompts,
                agents_root,
                notes: _,
            } = &paths;
            for (what, path) in [
                ("skill root", Some(skill_root.clone())),
                ("instructions", Some(instructions.clone())),
                ("hooks", hooks.clone()),
                ("mcp config", mcp_config.clone()),
                ("plugin", plugin.clone()),
                ("loop plugin", loop_plugin.clone()),
                ("prompts", prompts.clone()),
                ("agents root", agents_root.clone()),
            ] {
                let Some(path) = path else { continue };
                assert!(
                    path.starts_with(home),
                    "{} on {platform:?}: {what} resolved to {}, outside the home it was \
                     given{message_tail}",
                    adapter.slug,
                    path.display()
                );
            }
        }
    }
}

#[test]
fn a_moved_home_moves_every_root_under_it() {
    // The isolation invariant: every root under the home it was given,
    // including the two that used to be read from the process environment.
    let home = tempfile::tempdir().expect("a temporary directory");
    assert_every_root_resolves_under_home(
        home.path(),
        |platform| SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            platform: Some(platform),
            ..SetupOptions::default()
        },
        "",
    );
}

#[test]
fn an_inherited_appdata_never_moves_a_root_outside_home() {
    // An inherited `APPDATA` pointing at the operator's
    // real profile while `home` is a scratch directory. `elsewhere` stands in
    // for that real profile, never the machine's actual one.
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        // The fixture's claim, checked before trusting it: the two variables
        // have to actually hold a root outside `home`. On Windows `%TEMP%` sits
        // inside `%USERPROFILE%`, so this is not a formality.
        crate::test_env::assert_inherited_roots_escape(home.path());

        assert_every_root_resolves_under_home(
            home.path(),
            |platform| SetupOptions {
                platform: Some(platform),
                ..SetupOptions::default()
            },
            &format!(" ({})", home.path().display()),
        );
    });
}

#[test]
fn a_dropped_appdata_is_named_on_a_retired_host_s_roaming_root() {
    // The third named consequence: the earlier write was silent.
    // Dropping the escaping variable is right, but only if a caller can also
    // see it happened — measured here on the one adapter and platform where
    // `APPDATA` governs a root at all.
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let options = SetupOptions {
            platform: Some(Platform::Windows),
            ..SetupOptions::default()
        };
        let paths = resolve_paths(agent("gemini-cli"), &options).expect("the paths resolve");
        assert!(
            paths.instructions.starts_with(home.path()),
            "the instruction file still escaped to {}",
            paths.instructions.display()
        );
        assert_eq!(
            paths.notes.len(),
            1,
            "expected exactly one note for the one variable Gemini CLI reads on Windows, got {:?}",
            paths.notes
        );
        let note = &paths.notes[0];
        assert!(
            note.contains("APPDATA"),
            "the note does not name the variable that was dropped: {note:?}"
        );
        assert!(
            note.contains(&elsewhere.path().display().to_string()),
            "the note does not say what the variable pointed at: {note:?}"
        );
        assert!(
            note.contains(
                &home
                    .path()
                    .join("AppData")
                    .join("Roaming")
                    .display()
                    .to_string()
            ),
            "the note does not say what was used instead: {note:?}"
        );
    });
}

/// The other half of "both variables, one rule", measured for the variable the
/// test above does not touch: OpenCode reads `XDG_CONFIG_HOME`.
#[test]
fn a_dropped_xdg_config_home_is_named_rather_than_silently_ignored() {
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let options = SetupOptions {
            platform: Some(Platform::Unix),
            ..SetupOptions::default()
        };
        let paths = resolve_paths(agent("opencode"), &options).expect("the paths resolve");
        assert!(
            paths.instructions.starts_with(home.path()),
            "the instruction file still escaped to {}",
            paths.instructions.display()
        );
        assert_eq!(
            paths.notes.len(),
            1,
            "expected exactly one note for the one variable OpenCode reads, got {:?}",
            paths.notes
        );
        let note = &paths.notes[0];
        assert!(
            note.contains("XDG_CONFIG_HOME"),
            "the note does not name the variable that was dropped: {note:?}"
        );
        assert!(
            note.contains(&elsewhere.path().display().to_string()),
            "the note does not say what the variable pointed at: {note:?}"
        );
        assert!(
            note.contains(&home.path().join(".config").display().to_string()),
            "the note does not say what was used instead: {note:?}"
        );
    });
}

/// The harness's own variable is read the way `XDG_CONFIG_HOME`
/// is, and moves both of its roots under it.
#[test]
fn a_dsh_home_under_the_resolved_home_moves_both_harness_roots_under_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let dsh = home.path().join("harness-home");
    with_home_env_and_dsh_home(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        &dsh,
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths =
                resolve_paths(agent("deepseek-harness"), &options).expect("the paths resolve");
            assert_eq!(
                paths.skill_root,
                dsh.join("skills").join(skill::DIRECTORY),
                "the skill root did not move under $DSH_HOME"
            );
            assert_eq!(
                paths.instructions,
                dsh.join("AGENTS.md"),
                "the instruction file did not move under $DSH_HOME"
            );
            assert!(
                paths.notes.is_empty(),
                "a contained $DSH_HOME left a substitution note: {:?}",
                paths.notes
            );
        },
    );
}

/// The containment half, matching the `XDG_CONFIG_HOME` tests: a value
/// pointing outside the resolved home is ignored and named, never honoured.
#[test]
fn a_dsh_home_outside_the_resolved_home_is_ignored_and_named() {
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a relocated harness home");
    with_home_env_and_dsh_home(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        elsewhere.path(),
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths =
                resolve_paths(agent("deepseek-harness"), &options).expect("the paths resolve");
            assert_eq!(
                paths.skill_root,
                home.path()
                    .join(".dsh")
                    .join("skills")
                    .join(skill::DIRECTORY),
                "a dropped $DSH_HOME still moved the skill root"
            );
            assert_eq!(
                paths.instructions,
                home.path().join(".dsh").join("AGENTS.md"),
                "a dropped $DSH_HOME still moved the instruction file"
            );
            assert_eq!(
                paths.notes.len(),
                1,
                "expected exactly one note for the one variable the harness reads, got {:?}",
                paths.notes
            );
            assert!(
                paths.notes[0].contains("DSH_HOME"),
                "the note does not name the variable that was dropped: {:?}",
                paths.notes[0]
            );
        },
    );
}

#[test]
fn a_claude_config_dir_moves_every_claude_code_root_under_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a relocated claude configuration");
    with_home_env_and_claude_config_dir(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        elsewhere.path(),
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths = resolve_paths(agent("claude-code"), &options).expect("the paths resolve");
            assert_eq!(
                paths.skill_root,
                elsewhere.path().join("skills").join(skill::DIRECTORY)
            );
            assert_eq!(paths.instructions, elsewhere.path().join("CLAUDE.md"));
            assert_eq!(paths.hooks, Some(elsewhere.path().join("settings.json")));
            assert_eq!(
                paths.mcp_config,
                Some(elsewhere.path().join(".claude.json"))
            );
            assert_eq!(paths.agents_root, Some(elsewhere.path().join("agents")));
            assert!(
                paths.notes.is_empty(),
                "the variable moved a root and also left a note about it: {:?}",
                paths.notes
            );

            for slug in ["codex", "opencode"] {
                let AgentPaths {
                    skill_root,
                    instructions,
                    hooks,
                    mcp_config,
                    plugin,
                    loop_plugin,
                    commands: _,
                    prompts,
                    agents_root,
                    notes: _,
                } = resolve_paths(agent(slug), &options).expect("the paths resolve");
                for (what, path) in [
                    ("skill root", Some(skill_root)),
                    ("instructions", Some(instructions)),
                    ("hooks", hooks),
                    ("mcp config", mcp_config),
                    ("plugin", plugin),
                    ("loop plugin", loop_plugin),
                    ("prompts", prompts),
                    ("agents root", agents_root),
                ] {
                    let Some(path) = path else { continue };
                    assert!(
                        path.starts_with(home.path()),
                        "{slug}: {what} moved to {} when only Claude Code's variable was set",
                        path.display()
                    );
                }
            }
        },
    );
}

#[test]
fn an_explicit_claude_config_dir_overrides_the_home() {
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a relocated claude configuration");
    let options = SetupOptions {
        platform: Some(Platform::Unix),
        home_dir: Some(home.path().to_path_buf()),
        claude_config_dir: Some(elsewhere.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let paths = resolve_paths(agent("claude-code"), &options).expect("the paths resolve");
    assert_eq!(
        paths.skill_root,
        elsewhere.path().join("skills").join(skill::DIRECTORY)
    );
    assert_eq!(paths.instructions, elsewhere.path().join("CLAUDE.md"));
    assert_eq!(paths.hooks, Some(elsewhere.path().join("settings.json")));
    assert_eq!(
        paths.mcp_config,
        Some(elsewhere.path().join(".claude.json"))
    );
    assert_eq!(paths.agents_root, Some(elsewhere.path().join("agents")));
    assert!(paths.notes.is_empty());
}

#[test]
fn a_relative_claude_config_dir_falls_back_to_the_home() {
    let home = tempfile::tempdir().expect("a temporary home");
    with_home_env_and_claude_config_dir(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        Path::new("relative/claude"),
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths = resolve_paths(agent("claude-code"), &options).expect("the paths resolve");
            let AgentPaths {
                skill_root,
                instructions,
                hooks,
                mcp_config,
                plugin: _,
                loop_plugin: _,
                commands: _,
                prompts: _,
                agents_root,
                notes,
            } = &paths;
            for (what, path) in [
                ("skill root", Some(skill_root.clone())),
                ("instructions", Some(instructions.clone())),
                ("hooks", hooks.clone()),
                ("agents root", agents_root.clone()),
            ] {
                let Some(path) = path else { continue };
                assert!(
                    path.starts_with(home.path().join(".claude")),
                    "{what} fell through to {} rather than the home fallback",
                    path.display()
                );
            }
            assert_eq!(
                mcp_config.as_deref(),
                Some(home.path().join(".claude.json").as_path())
            );
            // The relative value is still dropped, but no longer in
            // silence — the note names what was ignored and what was used.
            assert_eq!(
                notes.len(),
                1,
                "a relative CLAUDE_CONFIG_DIR was dropped without a note: {notes:?}"
            );
            assert!(
                notes[0].contains("CLAUDE_CONFIG_DIR") && notes[0].contains("relative/claude"),
                "the note does not name the variable and the ignored value: {:?}",
                notes[0]
            );
            let fallback = home.path().join(".claude").display().to_string();
            assert!(
                notes[0].contains(&fallback),
                "the note does not name the fallback it used ({fallback}): {:?}",
                notes[0]
            );
        },
    );
}

#[test]
fn an_absent_claude_config_dir_leaves_claude_code_where_it_was() {
    let home = tempfile::tempdir().expect("a temporary home");
    with_home_env(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths = resolve_paths(agent("claude-code"), &options).expect("the paths resolve");
            assert_eq!(
                paths.skill_root,
                home.path()
                    .join(".claude")
                    .join("skills")
                    .join(skill::DIRECTORY)
            );
            assert_eq!(
                paths.instructions,
                home.path().join(".claude").join("CLAUDE.md")
            );
            assert_eq!(
                paths.hooks,
                Some(home.path().join(".claude").join("settings.json"))
            );
            assert_eq!(paths.mcp_config, Some(home.path().join(".claude.json")));
            assert_eq!(
                paths.agents_root,
                Some(home.path().join(".claude").join("agents"))
            );
        },
    );
}

#[test]
fn a_note_crosses_setup_into_the_result() {
    // Drives the public `setup` an operator calls and reads the note off the
    // value it hands back; deleting the line that carries the note across left
    // the suite green.
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let options = SetupOptions {
            platform: Some(Platform::Unix),
            ..SetupOptions::default()
        };
        crate::test_env::assert_inherited_roots_escape(home.path());
        let result = setup(agent("opencode"), &Config::default(), &options).expect("setup runs");
        // The byte assertion goes first deliberately: with the two the other
        // way round, a mutation sweep was sent at the note plumbing and never
        // measured the escaped write.
        assert_eq!(
            files_under(elsewhere.path()),
            0,
            "setup wrote outside the resolved home, into the directory standing in for the \
             operator's real profile: {:?}",
            listing(elsewhere.path())
        );
        assert!(
            result
                .notes
                .iter()
                .any(|note| note.contains("XDG_CONFIG_HOME")),
            "resolve_paths recorded a note and setup's own result did not carry it: {:?}",
            result.notes
        );
    });
}

#[test]
fn a_note_survives_a_failure_raised_after_the_resolution_that_produced_it() {
    // Drives `setup_into_evidenced` for real, so deleting the fix — carrying
    // `paths.notes` onto a failure raised after `resolve_paths` succeeds —
    // reddens it. The failure is a real filesystem state, not manufactured.
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let options = SetupOptions {
            platform: Some(Platform::Unix),
            ..SetupOptions::default()
        };
        crate::test_env::assert_inherited_roots_escape(home.path());
        let adapter = agent("opencode");
        let paths = resolve_paths(adapter, &options).expect("the paths resolve");
        std::fs::create_dir_all(paths.skill_root.join(skill::CONTRACT))
            .expect("a directory sits where the contract file is read from");

        let failure =
            setup_into_evidenced(adapter, &Config::default(), &options, &mut Pending::new())
                .expect_err("a directory in place of SKILL.md is not a readable contract");

        assert!(
            failure
                .result
                .notes
                .iter()
                .any(|note| note.contains("XDG_CONFIG_HOME")),
            "the substitution recorded by resolve_paths did not survive onto the failure \
             raised after it: {:?}",
            failure.result.notes
        );
    });
}

#[test]
fn a_note_crosses_uninstall_into_the_result_for_a_retired_host() {
    // The same gap on the way out: deleting `notes: paths.notes.clone(),`
    // inside `uninstall_from` left the suite green for the identical reason —
    // nothing drove a real `uninstall` and read its own `SetupResult`.
    let home = tempfile::tempdir().expect("a temporary home");
    let elsewhere = tempfile::tempdir().expect("a second temporary directory");
    with_home_env(home.path(), elsewhere.path(), elsewhere.path(), || {
        let options = SetupOptions {
            platform: Some(Platform::Windows),
            ..SetupOptions::default()
        };
        let result = uninstall(agent("gemini-cli"), &options).expect("uninstall runs");
        assert!(
            result.notes.iter().any(|note| note.contains("APPDATA")),
            "resolve_paths recorded a note and uninstall's own result did not carry it: {:?}",
            result.notes
        );
    });
}

#[test]
fn an_explicit_app_data_is_never_checked_for_containment() {
    // The other half of the explicit/inherited split. A system temporary
    // directory will not do here: on Windows `%TEMP%` sits *inside* the home,
    // so the parent of the home is used as the outside cell instead.
    let home = tempfile::tempdir().expect("a temporary home");
    let outside_the_home = home
        .path()
        .parent()
        .expect("a temporary directory has a parent")
        .join("estigia-app-data-outside-the-home");

    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        app_data: Some(outside_the_home.clone()),
        platform: Some(Platform::Windows),
        ..SetupOptions::default()
    };
    let environment = Environment::resolve(&options).expect("the supplied home is absolute");
    assert_eq!(
        environment.roaming(),
        outside_the_home,
        "an explicit APPDATA outside the home was silently replaced with the home-relative \
         fallback"
    );
    assert!(
        environment.notes.is_empty(),
        "an explicit APPDATA outside the home was noted as though it had been inherited: {:?}",
        environment.notes
    );
}

#[test]
fn uninstall_leaves_no_file_estigia_created() {
    // The exact inverse, checked by counting files. Cursor's hooks file once
    // survived as `{"version": 1, "hooks": {}}` — scaffolding read as content.
    let home = tempfile::tempdir().expect("a temporary directory");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let config = Config::default();

    let before = files_under(home.path());
    setup_fleet(&options, &config);
    // The screen's remembered language, written the way the screen writes it.
    crate::tui::words::remember(Some(home.path()), crate::tui::words::Tongue::Spanish)
        .expect("the screen remembers");
    assert!(
        files_under(home.path()) > before,
        "setup wrote nothing to undo"
    );
    uninstall_fleet(&options);
    // The machine-wide state, taken out the way the command does it.
    forget_state(&options);
    assert_eq!(
        files_under(home.path()),
        before,
        "uninstall left a file behind: {:?}",
        listing(home.path())
    );
}

fn listing(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn files_under(root: &std::path::Path) -> usize {
    listing(root).len()
}

#[test]
fn a_workspace_install_writes_under_the_workspace_and_nowhere_else() {
    // The whole point of `--scope=workspace`: an install that belongs to one
    // checkout. A fallback to the machine's home would configure an install the
    // checkout does not carry, and an uninstall that misses it is the reason
    // this asserts both directions rather than only the write.
    let (home, global) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let options = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..global
    };

    let config = Config::default();
    let mut installed = 0;
    for adapter in AGENTS {
        match setup(adapter, &config, &options) {
            Ok(_) => installed += 1,
            Err(error) => assert!(
                matches!(adapter.slug, "zcode" | "deepseek-harness" | "pi"),
                "{} refused a workspace install: {error}",
                adapter.slug
            ),
        }
    }
    assert!(installed > 0, "a workspace install wrote nothing");

    // An install that was refused is not recorded: a name nothing can take back
    // would pin the machine's state behind it.
    let entry = workspace_entry(&options).expect("the workspace names a record");
    if entry.exists() {
        let recorded = fs::read_to_string(&entry).expect("the record reads");
        assert!(
            !recorded.contains("zcode") && !recorded.contains("deepseek"),
            "a refusal was recorded as an install: {recorded}"
        );
    }

    // Named files, so a write that landed under the workspace but not where the
    // runtime reads it still fails here.
    assert!(workspace.path().join("AGENTS.md").is_file());
    assert!(workspace.path().join("CLAUDE.md").is_file());
    assert!(workspace.path().join("opencode.json").is_file());
    assert!(workspace.path().join(".mcp.json").is_file());
    assert!(workspace.path().join(".codex").join("hooks.json").is_file());
    assert!(
        workspace
            .path()
            .join(".codex")
            .join("config.toml")
            .is_file()
    );
    assert!(
        workspace
            .path()
            .join(".cursor")
            .join("hooks.json")
            .is_file()
    );
    assert!(workspace.path().join(".cursor").join("mcp.json").is_file());
    assert!(
        workspace
            .path()
            .join(".claude")
            .join("settings.json")
            .is_file()
    );
    assert!(
        workspace
            .path()
            .join(".opencode")
            .join(super::plugin::DIRECTORY)
            .join(super::plugin::FILE)
            .is_file()
    );
    assert!(
        workspace
            .path()
            .join(".claude")
            .join("skills")
            .join(crate::skill::DIRECTORY)
            .is_dir()
    );

    // The adapter files all land under the checkout. Estigia's own record of the
    // install lives under `.estigia` beside the run pointers, where its
    // machine-side state already is — no *adapter* file does.
    let strays: Vec<_> = listing(home.path())
        .into_iter()
        .filter(|path| !path.starts_with(home.path().join(".estigia")))
        .collect();
    assert!(
        strays.is_empty(),
        "a workspace install wrote adapter files outside the workspace: {strays:#?}"
    );

    for adapter in AGENTS {
        let _ = uninstall(adapter, &options);
    }
    let left = listing(workspace.path());
    assert!(
        left.is_empty(),
        "a workspace uninstall left {} file(s) behind: {:#?}",
        left.len(),
        left
    );
    assert!(
        !workspace_entry(&options)
            .expect("the workspace names a record")
            .exists(),
        "a record outlived every adapter that was installed"
    );
}

#[test]
fn the_runtimes_that_share_an_instruction_file_share_its_skill_root() {
    // `shared_instructions_survive` keeps the shared directive on the strength
    // of a sibling that reads the same file, while the skill goes on the
    // strength of a sibling that shares its root: a runtime with one and not the
    // other would leave a directive naming a directory another removal had
    // taken.
    let workspace = std::path::Path::new("/tmp/a-workspace");
    let mut root_of: std::collections::BTreeMap<std::path::PathBuf, std::path::PathBuf> =
        std::collections::BTreeMap::new();
    for adapter in AGENTS {
        let Some(paths) = workspace_paths(adapter, workspace) else {
            continue;
        };
        if let Some(previous) = root_of.insert(paths.instructions.clone(), paths.skill_root.clone())
        {
            assert_eq!(
                previous,
                paths.skill_root,
                "{} reads {} but lands its skill on a root of its own",
                adapter.slug,
                paths.instructions.display()
            );
        }
    }
}

#[test]
fn a_workspace_uninstall_takes_the_skill_back_from_one_shared_root() {
    // The runtimes that read `AGENTS.md` share one instruction file and one
    // skill root in a workspace, so the skill-sharing check can mistake an
    // adapter that was never installed for a second install. The skill has to
    // come back when the only adapter that was installed is taken out.
    let (_home, global) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let options = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..global
    };
    let agents = find_agent("agents").expect("agents is known");
    setup(agents, &Config::default(), &options).expect("setup runs");
    assert!(!listing(workspace.path()).is_empty());

    uninstall(agents, &options).expect("uninstall runs");
    let left = listing(workspace.path());
    assert!(
        left.is_empty(),
        "a lone workspace uninstall left {} file(s), so a shared instruction file was mistaken \
         for a second install: {:#?}",
        left.len(),
        left
    );
}

#[test]
fn a_workspace_install_needs_a_workspace_and_a_verified_adapter() {
    let claude = find_agent("claude-code").expect("claude-code is known");
    let no_workspace = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: None,
        ..SetupOptions::default()
    };
    let error = resolve_paths(claude, &no_workspace)
        .expect_err("the workspace scope with no workspace silently wrote to the machine's home");
    assert!(
        error.to_string().contains("--workspace"),
        "the refusal does not name the way out: {error}"
    );

    let zcode = find_agent("zcode").expect("zcode is known");
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let options = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let error = resolve_paths(zcode, &options).expect_err(
        "zcode has no verified project-level destination, so it must be refused rather than guessed at",
    );
    assert!(
        error.to_string().contains("zcode"),
        "the refusal does not name the adapter: {error}"
    );
}

#[test]
fn a_workspace_uninstall_takes_its_loop_plugin_and_leaves_the_machines() {
    // The loop plugin is scope-derived like every other destination, so a
    // workspace uninstall has to read the workspace path. Recomputing it from
    // the machine's roots leaves the workspace copy behind and takes the
    // machine's instead.
    let (_home, machine) = sandbox();
    let adapter = agent("opencode");
    setup(adapter, &loop_on(), &machine).expect("the machine install succeeds");
    let machine_loop = resolve_paths(adapter, &machine)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    assert!(
        machine_loop.is_file(),
        "the machine loop plugin was not written"
    );

    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    setup(adapter, &loop_on(), &scoped).expect("the workspace install succeeds");
    let workspace_loop = resolve_paths(adapter, &scoped)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    assert!(
        workspace_loop.starts_with(workspace.path()),
        "the workspace loop plugin did not land under the workspace: {}",
        workspace_loop.display()
    );
    assert!(
        workspace_loop.is_file(),
        "the workspace loop plugin was not written"
    );

    uninstall(adapter, &scoped).expect("the workspace uninstall succeeds");
    assert!(
        !workspace_loop.exists(),
        "a workspace uninstall left its loop plugin behind: {}",
        workspace_loop.display()
    );
    assert!(
        machine_loop.is_file(),
        "a workspace uninstall removed the machine's loop plugin: {}",
        machine_loop.display()
    );
}

#[test]
fn a_workspace_uninstall_keeps_the_machines_state_while_a_global_install_stands() {
    // The state is machine-wide; only the last install anywhere may take it, so
    // a workspace uninstall must look at the machine's roots too.
    let (home, machine) = sandbox();
    setup(agent("claude-code"), &Config::default(), &machine)
        .expect("the machine install succeeds");
    let pointer = home
        .path()
        .join(".estigia")
        .join("runs")
        .join("another-run.json");
    fs::create_dir_all(pointer.parent().expect("a parent")).expect("create the runs directory");
    fs::write(&pointer, "{}").expect("write a run pointer");

    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let open = agent("opencode");
    setup(open, &Config::default(), &scoped).expect("the workspace install succeeds");
    uninstall(open, &scoped).expect("the workspace uninstall succeeds");

    let taken = forget_state(&scoped);
    assert!(
        taken.is_empty(),
        "a workspace uninstall took machine state a global install still reads: {taken:#?}"
    );
    assert!(
        pointer.is_file(),
        "a run pointer another session owns was removed by a workspace uninstall"
    );
}

#[test]
fn a_global_uninstall_keeps_the_machines_state_while_a_workspace_install_stands() {
    // The store is shared, so a global uninstall must not take it while a
    // checkout still reads it — and a global uninstall cannot read the
    // checkouts, so the install records itself on the machine.
    let (home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine.clone()
    };
    let open = agent("opencode");
    setup(open, &Config::default(), &scoped).expect("the workspace install succeeds");

    let pointer = home
        .path()
        .join(".estigia")
        .join("runs")
        .join("a-workspace-run.json");
    fs::create_dir_all(pointer.parent().expect("a parent")).expect("create the runs directory");
    fs::write(&pointer, "{}").expect("write a run pointer");

    let taken = forget_state(&machine);
    assert!(
        taken.is_empty(),
        "a global uninstall took state a workspace install still reads: {taken:#?}"
    );
    assert!(
        pointer.is_file(),
        "a run pointer a checkout still reads was removed by a global uninstall"
    );

    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    assert!(
        entry.is_file(),
        "the install was not recorded on the machine"
    );
    uninstall(open, &scoped).expect("the workspace uninstall succeeds");
    assert!(!entry.exists(), "the install record outlived the install");
}

#[test]
fn a_dry_run_uninstall_leaves_the_workspace_record_alone() {
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    // A record can outlive the adapter files an interrupted uninstall removed,
    // and a dry run must not touch it either.
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::create_dir_all(entry.parent().expect("a parent")).expect("create the registry");
    let body = serde_json::json!({
        "workspace": workspace.path().to_string_lossy(),
        "adapters": ["opencode"],
    });
    fs::write(&entry, serde_json::to_vec(&body).expect("serialize")).expect("write a record");

    let dry = SetupOptions {
        dry_run: true,
        ..scoped
    };
    uninstall(agent("opencode"), &dry).expect("the dry run succeeds");
    assert!(
        entry.is_file(),
        "a dry run removed the machine-side install record: {}",
        entry.display()
    );
}

#[test]
fn a_workspace_record_is_keyed_by_the_directory_not_its_spelling() {
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let entry_for = |workspace: std::path::PathBuf| {
        workspace_entry(&SetupOptions {
            scope: InstallScope::Workspace,
            workspace: Some(workspace),
            ..SetupOptions::default()
        })
    };
    let plain = entry_for(workspace.path().to_path_buf());
    let dotted = entry_for(workspace.path().join("."));
    assert!(plain.is_some());
    assert_eq!(
        plain, dotted,
        "one directory, two records: a checkout could not be uninstalled by naming it the other way"
    );
}

#[test]
fn a_record_for_a_checkout_that_is_gone_does_not_pin_the_machines_state() {
    let (home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine.clone()
    };
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::create_dir_all(entry.parent().expect("a parent")).expect("create the registry");
    let body = serde_json::json!({ "workspace": workspace.path().to_string_lossy() });
    fs::write(&entry, serde_json::to_vec(&body).expect("serialize")).expect("write a record");
    drop(workspace);

    let pointer = home.path().join(".estigia").join("runs").join("r.json");
    fs::create_dir_all(pointer.parent().expect("a parent")).expect("create the runs directory");
    fs::write(&pointer, "{}").expect("write a run pointer");

    let taken = forget_state(&machine);
    assert!(
        !taken.is_empty(),
        "a record for a checkout that is gone pinned the machine's state forever"
    );
    assert!(!pointer.exists());
}

#[test]
fn a_stray_file_in_the_registry_is_not_an_install() {
    // Finder writes `.DS_Store` into directories it shows and an editor a swap
    // file; neither is a record, and neither may pin machine state.
    let (home, machine) = sandbox();
    let registry = home.path().join(".estigia").join("workspaces");
    fs::create_dir_all(&registry).expect("create the registry");
    fs::write(registry.join(".DS_Store"), b"\x00\x01not json").expect("write a stray file");

    let pointer = home.path().join(".estigia").join("runs").join("r.json");
    fs::create_dir_all(pointer.parent().expect("a parent")).expect("create the runs directory");
    fs::write(&pointer, "{}").expect("write a run pointer");

    let taken = forget_state(&machine);
    assert!(
        !taken.is_empty(),
        "a stray file pinned the machine's state: {taken:#?}"
    );
    assert!(!pointer.exists());
}

#[test]
fn an_unreadable_workspace_record_is_not_the_same_as_no_install() {
    // An unknown is not clearance: a record that cannot be read must keep the
    // store, not release it, or a permission error on the registry would let a
    // wipe through.
    let (home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine.clone()
    };
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::create_dir_all(entry.parent().expect("a parent")).expect("create the registry");
    fs::write(&entry, "{ this is not json").expect("write a broken record");

    let pointer = home.path().join(".estigia").join("runs").join("r.json");
    fs::create_dir_all(pointer.parent().expect("a parent")).expect("create the runs directory");
    fs::write(&pointer, "{}").expect("write a run pointer");

    let taken = forget_state(&machine);
    assert!(
        taken.is_empty(),
        "a record that could not be read was taken for an empty registry: {taken:#?}"
    );
    assert!(
        pointer.is_file(),
        "a run pointer was removed on an unreadable record"
    );
}

#[test]
fn a_workspace_uninstall_with_no_record_leaves_no_dangling_directive() {
    // The record is the only thing that says who is installed; with none, an
    // unknown keeps the shared surface — and the two halves have to agree, or
    // the run leaves a directive naming a directory it has just taken.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let claude = agent("claude-code");
    setup(claude, &Config::default(), &scoped).expect("the install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::remove_file(&entry).expect("drop the record");

    uninstall(claude, &scoped).expect("the uninstall succeeds");
    let instruction = workspace.path().join("CLAUDE.md");
    let skill = resolve_paths(claude, &scoped)
        .expect("paths resolve")
        .skill_root;
    assert_eq!(
        instruction.is_file(),
        skill.is_dir(),
        "the directive and the skill it names disagree: {} vs {}",
        instruction.display(),
        skill.display()
    );
}

#[test]
fn a_workspace_that_does_not_exist_yet_is_installed_and_reversed() {
    // The record is keyed by the checkout's canonical path, and the record is
    // written before anything else: a directory that only exists after the
    // install would be keyed on one name and looked up under another.
    let (_home, machine) = sandbox();
    let parent = tempfile::tempdir().expect("a parent");
    let workspace = parent.path().join("not-there-yet");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.clone()),
        ..machine
    };
    let open = agent("opencode");
    setup(open, &Config::default(), &scoped).expect("the install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    assert!(
        entry.is_file(),
        "the record was not written where the way back looks"
    );

    uninstall(open, &scoped).expect("the uninstall succeeds");
    assert!(
        listing(&workspace).is_empty(),
        "the uninstall did not reverse an install of a checkout that did not exist yet: {:#?}",
        listing(&workspace)
    );
    assert!(!entry.exists(), "the record outlived the install");
}

#[test]
fn a_workspace_install_that_cannot_record_itself_writes_nothing() {
    // The record goes first: an install that cannot record itself is one a
    // machine-wide uninstall cannot see, so it must not land at all.
    let (home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    // A plain file where the registry directory goes makes the record write
    // fail, and no adapter file may have landed by then.
    fs::create_dir_all(home.path().join(".estigia")).expect("create the state directory");
    fs::write(
        home.path().join(".estigia").join("workspaces"),
        "not a directory",
    )
    .expect("block the registry");

    let failed = setup(agent("opencode"), &Config::default(), &scoped);
    assert!(
        failed.is_err(),
        "an install that could not record itself reported success"
    );
    let left = listing(workspace.path());
    assert!(
        left.is_empty(),
        "an install that could not record itself left files a machine-wide uninstall cannot see: \
         {left:#?}"
    );
}

#[test]
fn an_install_over_an_unreadable_record_does_not_drop_the_adapters_it_named() {
    // The record is the only authority for scope-aware reversal; writing over
    // one that cannot be read would silently stop naming a live install, and a
    // later uninstall would then strip the shared surface for it.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    setup(agent("agents"), &Config::default(), &scoped).expect("the install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::write(&entry, "{ this is not json").expect("corrupt the record");

    let failed = setup(agent("codex"), &Config::default(), &scoped);
    assert!(
        failed.is_err(),
        "an install wrote over a record it could not read, dropping the adapters it named"
    );
    assert_eq!(
        fs::read_to_string(&entry).expect("the record reads"),
        "{ this is not json",
        "the record that could not be read was rewritten"
    );
}

#[test]
fn an_unreadable_workspace_record_does_not_make_an_uninstall_a_silent_no_op() {
    // The same unknown: a record that cannot be read must not read as "no
    // adapters here", or an uninstall reports completion having done nothing.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let open = agent("opencode");
    setup(open, &Config::default(), &scoped).expect("the workspace install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    fs::write(&entry, "{ this is not json").expect("corrupt the record");

    let result = uninstall(open, &scoped);
    assert!(
        result.is_err() || result.as_ref().is_ok_and(|done| !done.actions.is_empty()),
        "a record that could not be read turned an uninstall into a silent success"
    );
}

#[test]
fn a_workspace_record_outlives_a_sibling_adapter() {
    // `forget_workspace_install` keeps the machine-side record while any
    // adapter is still installed in the checkout, because a machine-wide
    // uninstall relies on that record to know the store is still read.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let neutral = agent("agents");
    let open = agent("opencode");
    setup(neutral, &Config::default(), &scoped).expect("the first install succeeds");
    setup(open, &Config::default(), &scoped).expect("the second install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    assert!(entry.is_file());

    uninstall(neutral, &scoped).expect("the first uninstall succeeds");
    assert!(
        entry.is_file(),
        "a sibling adapter's uninstall dropped the record a live install still needs"
    );

    uninstall(open, &scoped).expect("the second uninstall succeeds");
    assert!(
        !entry.exists(),
        "the record outlived every adapter in the checkout"
    );
}

#[test]
fn the_workspace_record_sits_beside_the_run_pointers() {
    let (home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    setup(agent("opencode"), &Config::default(), &scoped).expect("the workspace install succeeds");
    let entry = workspace_entry(&scoped).expect("the workspace names a record");
    let expected = home.path().join(".estigia").join("workspaces");
    assert_eq!(
        entry.parent(),
        Some(expected.as_path()),
        "the record is not where docs/what-it-writes.md says it is: {}",
        entry.display()
    );
}

#[test]
fn uninstalling_the_last_writer_leaves_the_shared_directive_naming_a_live_skill() {
    // The four AGENTS.md runtimes share one skill root as well as one file, so
    // the directive the last writer left still names a directory that is there.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let open = agent("opencode");
    let neutral = agent("agents");
    setup(neutral, &Config::default(), &scoped).expect("the install succeeds");
    setup(open, &Config::default(), &scoped).expect("the second install succeeds");
    let shared = resolve_paths(neutral, &scoped)
        .expect("paths resolve")
        .skill_root;
    assert_eq!(
        resolve_paths(open, &scoped)
            .expect("paths resolve")
            .skill_root,
        shared,
        "the two readers do not share the skill root the one file names"
    );

    // OpenCode wrote the block last, so the shared directive names its paths.
    uninstall(open, &scoped).expect("the uninstall succeeds");
    assert!(
        shared.is_dir(),
        "the directive's skill directory is gone: {}",
        shared.display()
    );
    assert!(listing(&shared).len() > 1);
    let directive = fs::read_to_string(workspace.path().join("AGENTS.md")).expect("the file reads");
    assert!(
        directive.contains(&shared.display().to_string()),
        "the shared directive names a skill directory another adapter reads instead:\n{directive}"
    );
}

#[test]
fn a_shared_instruction_file_does_not_make_its_readers_look_installed() {
    // `sync` picks its targets from `is_present`, and the four AGENTS.md
    // runtimes share one file: reading it made a lone `agents` install look like
    // four, and `sync` installed the other three.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let neutral = agent("agents");
    setup(neutral, &Config::default(), &scoped).expect("the install succeeds");
    assert!(is_present(neutral, &scoped));
    for other in ["codex", "opencode", "cursor"] {
        assert!(
            !is_present(agent(other), &scoped),
            "{other} read as installed because the shared AGENTS.md carries the directive"
        );
    }
}

#[test]
fn a_workspace_uninstall_keeps_the_shared_directive_while_a_sibling_is_recorded() {
    // The instruction file is shared, so the directive in it is the sibling's
    // pointer too: it goes with the last adapter that reads it, not the first.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let neutral = agent("agents");
    setup(neutral, &Config::default(), &scoped).expect("the install succeeds");
    setup(agent("codex"), &Config::default(), &scoped).expect("the second install succeeds");
    let directive = workspace.path().join("AGENTS.md");
    assert!(directive.is_file());

    uninstall(neutral, &scoped).expect("the uninstall succeeds");
    assert!(
        directive.is_file(),
        "a recorded sibling still reads the shared directive and it was taken"
    );

    uninstall(agent("codex"), &scoped).expect("the last uninstall succeeds");
    assert!(
        !directive.exists(),
        "the directive outlived every adapter that reads it"
    );
}

#[test]
fn a_workspace_uninstall_of_an_absent_adapter_leaves_a_siblings_skill_alone() {
    // The record, not the shared instruction file, says who is installed: the
    // file is one block and cannot tell an install from a reader.
    let (_home, machine) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let scoped = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..machine
    };
    let neutral = agent("agents");
    setup(neutral, &Config::default(), &scoped).expect("the install succeeds");
    let skill_root = resolve_paths(neutral, &scoped)
        .expect("paths resolve")
        .skill_root;
    assert!(listing(&skill_root).len() > 1, "the install wrote no skill");

    // Codex resolves to the same skill root and instruction file, but owns
    // nothing in this checkout.
    uninstall(agent("codex"), &scoped).expect("the uninstall succeeds");
    assert!(
        listing(&skill_root).len() > 1,
        "uninstalling an adapter that was never installed took the sibling's skill: {}",
        skill_root.display()
    );
    assert!(
        workspace.path().join("AGENTS.md").is_file(),
        "uninstalling an absent adapter took the shared directive"
    );

    // And the adapter that is installed still comes out.
    uninstall(neutral, &scoped).expect("the uninstall succeeds");
    assert!(
        listing(&skill_root).is_empty(),
        "the installed adapter's skill stayed: {}",
        skill_root.display()
    );
}

#[test]
fn a_workspace_uninstall_of_an_adapter_it_never_wrote_is_a_no_op() {
    // ZCode, DeepSeek Harness and the retired adapters have no workspace
    // destination, so `uninstall --all --scope=workspace` must pass over them
    // rather than name eight adapters that could never have been installed.
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let options = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let mut checked = 0;
    for adapter in every_adapter() {
        if workspace_paths(adapter, workspace.path()).is_some() {
            continue;
        }
        checked += 1;
        let result = uninstall(adapter, &options).unwrap_or_else(|error| {
            panic!("{} refused a workspace uninstall: {error}", adapter.slug)
        });
        assert!(
            result.actions.is_empty(),
            "{} reported actions for a workspace it never wrote to",
            adapter.slug
        );
        assert!(result.completed, "{} did not complete", adapter.slug);
    }
    assert!(
        checked >= 2,
        "too few unplaceable adapters to be a test: {checked}"
    );
}

#[test]
fn a_plan_reports_what_the_run_does_even_when_two_steps_share_a_file() {
    // The fourth setup invariant, at the one place it broke. Checked for every
    // agent, both directions, because the invariant is setup's, not Qwen's.
    let config = Config::default();
    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let home = tempfile::tempdir().expect("a temporary directory");
                let real = SetupOptions {
                    home_dir: Some(home.path().to_path_buf()),
                    ..SetupOptions::default()
                };
                let planned = SetupOptions {
                    dry_run: true,
                    ..real.clone()
                };

                let plan = setup(adapter, &config, &planned).expect("the plan is made");
                let run = setup(adapter, &config, &real).expect("the run happens");
                assert_eq!(
                    shape(&plan),
                    shape(&run),
                    "{}: setup planned one thing and did another",
                    adapter.slug
                );

                let plan = uninstall(adapter, &planned).expect("the plan is made");
                let run = uninstall(adapter, &real).expect("the run happens");
                assert_eq!(
                    shape(&plan),
                    shape(&run),
                    "{}: uninstall planned one thing and did another",
                    adapter.slug
                );
            });
        }
    });
}

/// A result as a person reads it: which file, and what happened to it.
#[test]
fn a_plan_over_every_agent_reports_what_that_run_does() {
    // The sibling of the test above, at the `--all` seam where adapters share
    // the neutral skill root and later adapters meet a directory the run itself
    // filled.
    let config = Config::default();
    std::thread::scope(|scope| {
        for foreign in [false, true] {
            let config = &config;
            scope.spawn(move || {
                let home = tempfile::tempdir().expect("a temporary directory");
                let real = SetupOptions {
                    home_dir: Some(home.path().to_path_buf()),
                    ..SetupOptions::default()
                };
                let planned = SetupOptions {
                    dry_run: true,
                    ..real.clone()
                };
                // Once on an empty machine, and once over somebody else's skill.
                if foreign {
                    let skill = home
                        .path()
                        .join(".agents")
                        .join("skills")
                        .join(crate::skill::DIRECTORY);
                    std::fs::create_dir_all(&skill).expect("their directory");
                    std::fs::write(skill.join("SKILL.md"), "# Theirs\n").expect("their contract");
                }

                let mut planning = Pending::new();
                let plan: Vec<Vec<String>> = AGENTS
                    .iter()
                    .map(|adapter| {
                        shape(
                            &setup_into(adapter, config, &planned, &mut planning)
                                .expect("the plan is made"),
                        )
                    })
                    .collect();

                let mut running = Pending::new();
                let run: Vec<Vec<String>> = AGENTS
                    .iter()
                    .map(|adapter| {
                        shape(
                            &setup_into(adapter, config, &real, &mut running)
                                .expect("the run happens"),
                        )
                    })
                    .collect();

                for (adapter, (planned, ran)) in AGENTS.iter().zip(plan.iter().zip(run.iter())) {
                    assert_eq!(
                        planned,
                        ran,
                        "{}: over {} the plan and the run disagree",
                        adapter.slug,
                        if foreign {
                            "somebody else's skill"
                        } else {
                            "an empty machine"
                        }
                    );
                }
            });
        }
    });
}

fn shape(result: &SetupResult) -> Vec<String> {
    result
        .actions
        .iter()
        .map(|action| {
            format!(
                "{:?} {:?} {}",
                action.change,
                action.kind,
                action.path.display()
            )
        })
        .collect()
}

#[test]
fn setting_a_value_leaves_the_block_setup_would_have_written() {
    // Two paths write the managed block — `setup` and `config set` — and they
    // built it separately once.
    let home = tempfile::tempdir().expect("a temporary directory");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let adapter = find_agent("claude-code").expect("claude-code is an adapter");
    let config = Config::default();
    setup(adapter, &config, &options).expect("setup runs");

    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let contract = paths.skill_root.join(crate::skill::CONTRACT);
    let after_setup = fs::read_to_string(&contract).expect("the contract is written");

    rewrite_configuration(&contract, &config).expect("the value is written");
    let after_set = fs::read_to_string(&contract).expect("the contract is still there");

    assert_eq!(
        after_setup, after_set,
        "`config set` wrote a different block than `setup` for the same configuration"
    );
    assert!(
        after_set.contains(crate::config::LOCAL_FILE),
        "the override file is no longer named in the contract"
    );
}

#[test]
fn two_agents_sharing_one_root_can_run_different_models() {
    let root = tempfile::tempdir().expect("a temporary root");
    let contract = root.path().join(crate::skill::CONTRACT);
    let shared = Config::default();
    std::fs::write(
        &contract,
        format!(
            "# Contract\n\n{}\n{}\n{}\n",
            crate::config::BLOCK_BEGIN,
            shared.render_rows().trim_end(),
            crate::config::BLOCK_END
        ),
    )
    .expect("a contract");

    let routed = |models: &str| Config {
        models: crate::config::ModelRouting::parse(models).expect("a routing"),
        ..Config::default()
    };

    write_agent_configuration_wholly(
        &crate::skill::agent_override(root.path(), "claude-code"),
        "claude-code",
        &routed("developer=opus-5, apply=sonnet-5, orchestrate=fable-5"),
    )
    .expect("claude-code's own file");
    write_agent_configuration_wholly(
        &crate::skill::agent_override(root.path(), "opencode"),
        "opencode",
        &routed("orchestrate=gpt-5.6, apply=kimi-k3"),
    )
    .expect("opencode's own file");

    let claude = crate::skill::installed_config_for(root.path(), Some("claude-code"))
        .expect("claude-code reads its own");
    let opencode = crate::skill::installed_config_for(root.path(), Some("opencode"))
        .expect("opencode reads its own");

    assert_eq!(claude.models.for_target("apply"), Some("sonnet-5"));
    assert_eq!(opencode.models.for_target("apply"), Some("kimi-k3"));
    assert_eq!(claude.models.for_target("developer"), Some("opus-5"));
    assert_eq!(opencode.models.for_target("developer"), None);
    assert_eq!(claude.models.for_target("orchestrate"), Some("fable-5"));
    assert_eq!(opencode.models.for_target("orchestrate"), Some("gpt-5.6"));

    // An adapter with no file of its own still reads the shared table.
    let third =
        crate::skill::installed_config_for(root.path(), Some("codex")).expect("the shared table");
    assert_eq!(third.models, shared.models);

    // And a second write moves only the marked block.
    let file = crate::skill::agent_override(root.path(), "opencode");
    let existing = std::fs::read_to_string(&file).expect("it was written");
    std::fs::write(&file, format!("{existing}\n\nA note of my own.\n")).expect("a note");
    write_agent_configuration_wholly(&file, "opencode", &routed("apply=deepseek-v4"))
        .expect("a second write");
    let after = std::fs::read_to_string(&file).expect("still there");
    assert!(after.contains("A note of my own."), "the note was lost");
    assert_eq!(
        crate::skill::installed_config_for(root.path(), Some("opencode"))
            .expect("read back")
            .models
            .for_target("apply"),
        Some("deepseek-v4")
    );
}

#[test]
fn only_the_rows_the_gate_decides_carry_a_caveat_and_the_list_is_pinned() {
    use crate::config::{SETTINGS, Setting};

    // The exceptions are written out rather than derived: adding a
    // gate-enforced setting fails this test until somebody decides whether it
    // is asked. No row is inert any more — `Delegated workers` was the one such
    // row and its effect moved into `Model routing`, which every adapter holds.
    let asked = [Setting::Delivery, Setting::Transitions, Setting::Boundaries];

    for adapter in AGENTS {
        for setting in SETTINGS {
            let applies = adapter.applies(*setting);
            if adapter.can_gate_tools() {
                assert_eq!(
                    applies,
                    Applies::Held,
                    "{} qualifies {setting:?} despite gating its tools",
                    adapter.slug
                );
                continue;
            }
            let expected = if asked.contains(setting) {
                "asked"
            } else {
                "held"
            };
            let actual = match applies {
                Applies::Held => "held",
                Applies::Asked(_) => "asked",
            };
            assert_eq!(
                actual, expected,
                "{setting:?} is {actual} for {} and the pinned list says {expected}",
                adapter.slug
            );
            // Every caveat says why: `asked` is a real answer in a real contract.
            assert_eq!(
                applies.because().is_some(),
                expected != "held",
                "{setting:?} is qualified for {} without saying why",
                adapter.slug
            );
        }
    }
}

#[test]
fn an_adapter_that_gates_tools_and_has_no_agents_root_holds_its_rows() {
    use crate::config::Setting;

    // Codex, Gemini CLI, Cursor and Qwen all qualify: each can gate tool calls
    // and none has an agent-definitions root. The old `Inert` answer that
    // followed from the rootless half is gone with the row that needed it, so a
    // gated adapter holds every row here. Pinned on Codex so a regression that
    // reintroduced a rootless branch is caught outside the matrix above.
    let codex = agent("codex");
    assert!(codex.can_gate_tools(), "codex is expected to gate tools");
    assert!(
        !codex.has_agent_definitions(),
        "codex is expected to have no agent-definitions root"
    );
    for setting in [Setting::Models, Setting::Evidence] {
        assert_eq!(codex.applies(setting), Applies::Held, "{setting:?}");
    }

    // Claude Code is the floor: it also gates tools.
    let claude = agent("claude-code");
    assert!(claude.has_agent_definitions());
    for setting in [Setting::Models, Setting::Evidence] {
        assert_eq!(claude.applies(setting), Applies::Held);
    }
}

#[test]
fn a_plan_over_every_adapter_reports_what_the_whole_run_does() {
    // Setup's fourth promise, held at run scope: most adapters share a skill
    // root, and under `--dry-run` each once re-planned the same files.
    // Measured against the real binary before this was written: the plan said
    // 182 files, the run did 70.
    let home = tempfile::tempdir().expect("a temporary home");
    let plan_options = SetupOptions {
        dry_run: true,
        home_dir: Some(home.path().to_path_buf()),
        config_home: Some(home.path().join("config")),
        app_data: Some(home.path().join("appdata")),
        ..SetupOptions::default()
    };
    let real_options = SetupOptions {
        dry_run: false,
        ..plan_options.clone()
    };
    let config = Config::default();

    // The plan, over every adapter, with one memory for the whole run.
    let mut pending = Pending::new();
    let planned: Vec<usize> = AGENTS
        .iter()
        .map(|adapter| {
            setup_into(adapter, &config, &plan_options, &mut pending)
                .expect("the plan")
                .changed_files()
        })
        .collect();

    // Nothing was written by planning it.
    assert!(
        !home.path().join(".agents").exists(),
        "`--dry-run` wrote something"
    );

    // The run.
    let mut pending = Pending::new();
    let happened: Vec<usize> = AGENTS
        .iter()
        .map(|adapter| {
            setup_into(adapter, &config, &real_options, &mut pending)
                .expect("the run")
                .changed_files()
        })
        .collect();

    for ((adapter, plan), did) in AGENTS.iter().zip(&planned).zip(&happened) {
        assert_eq!(
            plan, did,
            "{} was promised {plan} files and got {did}",
            adapter.slug
        );
    }
    // And the shared root is why: several adapters plan far fewer files than
    // the first, because the first already accounted for the skill tree.
    assert!(
        planned.iter().any(|count| *count < 5),
        "no adapter shares the root, so this test is not testing what it says"
    );

    // The same promise, taking it all back out. `uninstall --all --dry-run` had
    // the mirror fault and the same measurement: seventeen files promised for
    // each of the adapters that share a root, three taken out.
    let mut pending = Pending::new();
    let planned: Vec<usize> = AGENTS
        .iter()
        .map(|adapter| {
            uninstall_from(adapter, &plan_options, &mut pending)
                .expect("the plan")
                .changed_files()
        })
        .collect();
    assert!(
        home.path().join(".agents").exists(),
        "`--dry-run` removed something"
    );

    let mut pending = Pending::new();
    let happened: Vec<usize> = AGENTS
        .iter()
        .map(|adapter| {
            uninstall_from(adapter, &real_options, &mut pending)
                .expect("the run")
                .changed_files()
        })
        .collect();
    for ((adapter, plan), did) in AGENTS.iter().zip(&planned).zip(&happened) {
        assert_eq!(
            plan, did,
            "{} was promised {plan} files removed and got {did}",
            adapter.slug
        );
    }
}

#[test]
fn taking_one_agent_out_leaves_the_skill_the_others_are_still_reading() {
    // Most adapters install into the shared neutral root; uninstalling one
    // once took the skill with it.
    let (_home, options) = sandbox();

    let neutral: Vec<&AgentAdapter> = AGENTS
        .iter()
        .filter(|adapter| adapter.skills() == SkillsRoot::Neutral)
        .collect();
    assert!(
        neutral.len() > 2,
        "the shared root is the whole point of this test"
    );
    let mut pending = Pending::new();
    for adapter in &neutral {
        setup_into(adapter, &Config::default(), &options, &mut pending).expect("install");
    }

    let first = neutral[0];
    let root = resolve_paths(first, &options).expect("paths").skill_root;
    uninstall_from(first, &options, &mut Pending::new()).expect("uninstall one");

    assert!(
        root.join("SKILL.md").is_file(),
        "the skill went out with the first agent, and {} others still read it",
        neutral.len() - 1
    );
    for adapter in neutral.iter().skip(1) {
        assert!(
            is_configured(adapter, &options),
            "{} lost its directive to somebody else\'s uninstall",
            adapter.slug
        );
    }

    // And the last one out does take it.
    let mut pending = Pending::new();
    for adapter in neutral.iter().skip(1) {
        uninstall_from(adapter, &options, &mut pending).expect("uninstall the rest");
    }
    assert!(
        !root.exists(),
        "the last agent out left the skill standing for nobody"
    );
}

#[test]
fn an_agent_that_only_shares_a_skill_root_is_not_half_installed() {
    // Installing into the shared root once made the other adapters on it read
    // as half installed; the skill was not theirs.
    let (_home, options) = sandbox();
    let neutral: Vec<&AgentAdapter> = AGENTS
        .iter()
        .filter(|adapter| adapter.skills() == SkillsRoot::Neutral)
        .collect();

    setup_into(
        neutral[0],
        &Config::default(),
        &options,
        &mut Pending::new(),
    )
    .expect("install one");

    for adapter in neutral.iter().skip(1) {
        assert!(
            !is_configured(adapter, &options),
            "{} was never set up",
            adapter.slug
        );
        assert_eq!(
            skill_shared_with(adapter, &options, &Pending::new())
                .expect("the question is answerable"),
            Some(neutral[0].display_name),
            "{} shares the neutral root with the agent that was set up",
            adapter.slug
        );
    }

    // An agent with a root of its own answers the other way.
    let alone = AGENTS
        .iter()
        .find(|adapter| adapter.skills() != SkillsRoot::Neutral)
        .expect("some agent keeps its own skill directory");
    assert_eq!(
        skill_shared_with(alone, &options, &Pending::new()).expect("answerable"),
        None,
        "{} keeps its own skill directory and shares with nobody",
        alone.slug
    );
}

#[test]
fn an_agent_s_own_answers_do_not_outlive_the_agent() {
    // `config set --agent` writes a file that is not one of `FILES`, so it
    // once outlived the uninstall.
    let (_home, options) = sandbox();
    let shared = AGENTS
        .iter()
        .find(|adapter| adapter.skills() == SkillsRoot::Neutral)
        .expect("an adapter on the shared root");

    setup_into(shared, &Config::default(), &options, &mut Pending::new()).expect("install");
    let root = resolve_paths(shared, &options).expect("paths").skill_root;
    let own = skill::agent_override(&root, shared.slug);
    let theirs = Config {
        change_size: 250,
        ..Config::default()
    };
    write_agent_configuration_wholly(&own, shared.slug, &theirs).expect("their answers");
    assert!(own.is_file(), "the fixture wrote nothing");

    uninstall_from(shared, &options, &mut Pending::new()).expect("uninstall");
    assert!(
        !own.exists(),
        "{} outlived the agent it configures",
        own.display()
    );
}

#[test]
fn every_file_estigia_reads_configuration_from_is_named_in_text_the_agent_reads() {
    // Estigia layers three files and reads all three; the agent only ever does
    // what the text it reads tells it to. A layer named nowhere is a layer
    // that exists in the code and in no behaviour. The contract needs no
    // naming — it is the file; the other two do.
    let body = skill::configuration_body(&Config::default());
    assert!(
        body.contains(crate::config::LOCAL_FILE),
        "the configuration block never names the operator\'s override file"
    );

    let (_home, options) = sandbox();
    let shared = AGENTS
        .iter()
        .find(|adapter| adapter.skills() == SkillsRoot::Neutral)
        .expect("an adapter on the shared root");
    let paths = resolve_paths(shared, &options).expect("paths");
    let own = skill::agent_override(&paths.skill_root, shared.slug);
    let name = own
        .file_name()
        .expect("the override has a file name")
        .to_string_lossy()
        .into_owned();
    let directive = directive_for(shared, &paths);
    assert!(
        directive.contains(&name),
        "nothing {} reads names {name}, and it is where its own answers go:\n{directive}",
        shared.slug
    );

    // And an adapter with a skill directory of its own is not sent looking for
    // a file that will never be written.
    let alone = AGENTS
        .iter()
        .find(|adapter| adapter.skills() != SkillsRoot::Neutral)
        .expect("some adapter keeps its own skill directory");
    let paths = resolve_paths(alone, &options).expect("paths");
    let directive = directive_for(alone, &paths);
    assert!(
        !directive.contains(&format!("estigia.{}.md", alone.slug)),
        "{} is sent to a file nothing will ever write:\n{directive}",
        alone.slug
    );
}

/// The plan matches the act for every adapter, in every state a run meets —
/// and every file an adapter's gate lives in is one the gate measures.
#[test]
fn every_control_file_an_adapter_has_is_one_the_gate_measures() {
    // Every platform, two XDG layouts, and two Claude Code
    // configuration layouts as well: this is the only thing tying `paths_in`
    // to `is_control_surface`.
    let (_home, base) = sandbox();
    for platform in [Platform::Windows, Platform::MacOs, Platform::Unix] {
        for moved in [false, true] {
            for claude_relocated in [false, true] {
                let claude_config_dir = claude_relocated.then(|| _home.path().join("moved-claude"));
                let options = SetupOptions {
                    platform: Some(platform),
                    config_home: if moved {
                        Some(_home.path().join("moved-xdg"))
                    } else {
                        base.config_home.clone()
                    },
                    claude_config_dir: claude_config_dir.clone(),
                    ..base.clone()
                };
                for adapter in AGENTS {
                    let paths = resolve_paths(adapter, &options).expect("paths resolve");
                    // The bare root as well as a file under it: `surface_of`
                    // appends a separator, so the two answer differently.
                    let mut watched: Vec<std::path::PathBuf> =
                        vec![paths.skill_root.clone(), paths.skill_root.join("SKILL.md")];
                    watched.extend(paths.hooks.clone());
                    watched.extend(paths.plugin.clone());
                    watched.extend(paths.mcp_config.clone());
                    // The instruction file: the one `setup` writes the directive
                    // into, so an agent that rewrites it removes the reason it
                    // obeys.
                    watched.push(paths.instructions.clone());
                    // The agent-definition root, which `definition_for` reads the
                    // tool allowlist it enforces from.
                    watched.extend(paths.agents_root.clone());
                    // Estigia's own delivery root, one level up from the contract:
                    // a sibling directory carries the same authority as the
                    // contract beside it.
                    let skills = paths
                        .skill_root
                        .parent()
                        .expect("a skill root has a tree above it")
                        .to_path_buf();
                    watched.push(skills.join("zz-other-skill").join("SKILL.md"));
                    watched.push(skills);

                    // `is_control_surface` reads `$CLAUDE_CONFIG_DIR` from the
                    // real process environment (`matches_relocated_claude_config`)
                    // rather than from `SetupOptions`, so this run's
                    // variable is set to match whatever `resolve_paths` just used
                    // to compute `watched` above.
                    let assert_watched = || {
                        for file in &watched {
                            let target = file.display().to_string();
                            // Both roads: a path containing a space can answer
                            // `Boundary` on the write tool and `Routine` through
                            // the shell — the road an agent takes to delete
                            // something.
                            for (road, payload) in [
                                ("Write", serde_json::json!({ "file_path": target.clone() })),
                                (
                                    "Bash",
                                    serde_json::json!({ "command": format!("rm {target}") }),
                                ),
                            ] {
                                let (_, how) = crate::harness::classify(road, &payload);
                                assert_eq!(
                                    how,
                                    crate::harness::Sensitivity::Boundary,
                                    "{platform:?}/claude_relocated={claude_relocated}/{}: {road} \
                                 on {target} is where its gate lives and the gate calls it \
                                 routine",
                                    adapter.slug
                                );
                            }
                        }
                    };
                    match &claude_config_dir {
                        Some(directory) => with_process_env(
                            "CLAUDE_CONFIG_DIR",
                            &directory.display().to_string(),
                            assert_watched,
                        ),
                        None => with_process_env("CLAUDE_CONFIG_DIR", "", assert_watched),
                    }
                }
            }
        }
    }
}

#[test]
fn the_plan_matches_the_act_for_every_adapter_and_every_state() {
    let strip = |result: SetupResult| {
        result
            .actions
            .into_iter()
            .map(|action| (action.kind, action.change, action.path))
            .collect::<Vec<_>>()
    };

    // Away from the default on two axes, so the update path has something to
    // update rather than rewriting identical bytes.
    let moved = Config {
        evidence: crate::config::Evidence::Measuring,
        change_size: 250,
        ..Config::default()
    };

    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let (_home, options) = sandbox();
                let dry = SetupOptions {
                    dry_run: true,
                    ..options.clone()
                };
                for (state, config) in [
                    ("nothing installed", &Config::default()),
                    ("installed already", &Config::default()),
                    ("installed, and the answers moved", &moved),
                ] {
                    let planned =
                        strip(setup(adapter, config, &dry).expect("the plan is produced"));
                    let performed =
                        strip(setup(adapter, config, &options).expect("setup succeeds"));
                    assert_eq!(
                        planned, performed,
                        "{}: with {state}, the plan and the act disagree",
                        adapter.slug
                    );
                }
            });
        }
    });
}

/// What was theirs before is theirs after, in every file and every adapter,
/// seeded with content of the shape that file really holds and read back
/// **byte for byte** after the round trip.
#[test]
fn every_file_an_adapter_touches_comes_back_exactly_as_it_was() {
    round_trip(false);
}

#[test]
fn every_file_an_adapter_touches_comes_back_exactly_as_it_was_with_scaffolding() {
    round_trip(true);
}

fn round_trip(with_scaffolding: bool) {
    let seed = |path: &std::path::Path| -> String {
        // Shaped like the file really is: parseable JSON, TOML, markdown with
        // headings above and below wherever a fence lands.
        let body = match path.extension().and_then(|kind| kind.to_str()) {
            Some("json") if with_scaffolding => {
                // A Cursor hooks file as a Cursor user really has one.
                "{\n  \"version\": 1,\n  \"mine\": \"do not touch\"\n}\n".to_owned()
            }
            Some("json") => "{\n  \"mine\": \"do not touch\"\n}\n".to_owned(),
            Some("toml") => "[mine]\nkeep = \"do not touch\"\n".to_owned(),
            Some("md") => {
                "# Mine\n\nAlways answer in Spanish.\n\n## Second heading\n\nAnd a line after it.\n"
                    .to_owned()
            }
            Some("js") => "// mine — do not touch\n".to_owned(),
            _ => "#!/bin/sh\n# mine — do not touch\n".to_owned(),
        };
        fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
        fs::write(path, &body).expect("their file");
        body
    };

    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let home = tempfile::tempdir().expect("a temporary home");
                let options = SetupOptions {
                    home_dir: Some(home.path().to_path_buf()),
                    ..SetupOptions::default()
                };
                let paths = resolve_paths(adapter, &options).expect("paths resolve");

                let mut theirs: Vec<(std::path::PathBuf, String)> = Vec::new();
                for file in [
                    paths.hooks.clone(),
                    paths.plugin.clone(),
                    paths.mcp_config.clone(),
                    Some(paths.instructions),
                ]
                .into_iter()
                .flatten()
                {
                    // Two adapters point two of these at one file; seeding it twice
                    // would compare it against the wrong original.
                    if theirs.iter().any(|(had, _)| *had == file) {
                        continue;
                    }
                    let body = seed(&file);
                    theirs.push((file, body));
                }

                setup(adapter, &Config::default(), &options).expect("setup runs");
                uninstall(adapter, &options).expect("uninstall runs");

                for (file, body) in theirs {
                    let after = fs::read_to_string(&file).unwrap_or_default();
                    if after == body {
                        continue;
                    }
                    // No difference is allowed, in either direction: a key in a file
                    // the operator already kept hooks in is theirs by construction.
                    let (Ok(before), Ok(now)) = (
                        serde_json::from_str::<serde_json::Value>(&body),
                        serde_json::from_str::<serde_json::Value>(&after),
                    ) else {
                        panic!(
                            "{}: {} did not come back as it was\n  was: {body:?}\n  now: {after:?}",
                            adapter.slug,
                            file.display()
                        );
                    };
                    let (Some(before), Some(now)) = (before.as_object(), now.as_object()) else {
                        panic!(
                            "{}: {} is no longer an object",
                            adapter.slug,
                            file.display()
                        );
                    };
                    for (key, value) in before {
                        assert_eq!(
                            now.get(key),
                            Some(value),
                            "{}: {} lost or changed {key}, which was theirs",
                            adapter.slug,
                            file.display()
                        );
                    }
                    for key in now.keys() {
                        assert!(
                            before.contains_key(key),
                            "{}: {} came back carrying {key}, which is not theirs",
                            adapter.slug,
                            file.display()
                        );
                    }
                }
            });
        }
    });
}

/// Theirs is theirs even when it names Estigia: a hook that *mentions*
/// `estigia` once read as Estigia's own.
#[test]
fn an_entry_of_theirs_that_names_estigia_is_still_theirs() {
    std::thread::scope(|scope| {
        for slug in [
            "claude-code",
            "cursor",
            "codex",
            "crush",
            "continue",
            "qwen",
        ] {
            scope.spawn(move || {
                let home = tempfile::tempdir().expect("a temporary home");
                let options = SetupOptions {
                    home_dir: Some(home.path().to_path_buf()),
                    ..SetupOptions::default()
                };
                let adapter = agent(slug);
                let paths = resolve_paths(adapter, &options).expect("paths resolve");
                let Some(hooks) = paths.hooks else {
                    return;
                };
                fs::create_dir_all(hooks.parent().expect("a parent")).expect("the directory");
                // Their own pre-tool hook, running a linter whose name starts with
                // `estigia`, in the envelope this adapter reads.
                let theirs = "{\n  \"mine\": \"do not touch\",\n  \"hooks\": {\n    \"PreToolUse\": [\n      {\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\",\n        \"command\": \"estigia-lint --check\", \"timeout\": 3}]}\n    ]\n  }\n}\n";
                fs::write(&hooks, theirs).expect("their file");

                if adapter.retired() {
                    install_as_an_earlier_build(adapter, &options);
                } else {
                    setup(adapter, &Config::default(), &options).expect("setup runs");
                }
                uninstall(adapter, &options).expect("uninstall runs");

                let after = fs::read_to_string(&hooks).unwrap_or_default();
                assert!(
                    after.contains("do not touch"),
                    "{slug}: a key of theirs went with the uninstall: {after}"
                );
                assert!(
                    after.contains("estigia-lint"),
                    "{slug}: their own hook was taken for Estigia's: {after}"
                );
            });
        }
    });
}

/// An uninstall takes Estigia's hooks out and leaves the operator's file
/// alone: an event key dropped for *being* empty rather than for having been
/// emptied here took an operator's own unfilled slot with it.
#[test]
fn an_uninstall_leaves_an_empty_event_the_operator_wrote_where_it_was() {
    let path = std::path::Path::new("settings.json");

    // Nothing of Estigia's, and an empty slot of the operator's.
    let theirs = "{\n  \"hooks\": {\n    \"Notification\": []\n  }\n}\n";
    assert_eq!(
        render::strip_hooks(path, theirs).expect("a readable file"),
        theirs,
        "an uninstall rewrote a file that never mentioned Estigia, and took the operator's own \
         empty event with it"
    );

    // The same rule one level up: an operator's empty *wrapper* is theirs too.
    let wrapper = "{
  \"hooks\": {}
}
";
    assert_eq!(
        render::strip_hooks(path, wrapper).expect("a readable file"),
        wrapper,
        "an uninstall deleted an empty hooks wrapper the operator wrote"
    );

    // And with Estigia in it: ours goes, theirs stays — including the key we
    // emptied.
    let mixed = serde_json::json!({
        "hooks": {
            "Notification": [],
            "SessionStart": [{"hooks": [{"type": "command", "command": "estigia hook session-start"}]}],
            "PreToolUse": [
                {"hooks": [{"type": "command", "command": "estigia hook pre-tool-use"}]},
                {"hooks": [{"type": "command", "command": "their-linter --check"}]}
            ]
        }
    })
    .to_string();
    let after: serde_json::Value =
        serde_json::from_str(&render::strip_hooks(path, &mixed).expect("a readable file"))
            .expect("still json");
    let hooks = after.get("hooks").and_then(serde_json::Value::as_object);
    let hooks = hooks.expect("the wrapper survives");
    assert!(
        hooks.contains_key("Notification"),
        "the operator's empty event was deleted by an uninstall: {after:#}"
    );
    assert!(
        !hooks.contains_key("SessionStart"),
        "an event Estigia emptied was left behind as a key saying nothing: {after:#}"
    );
    assert_eq!(
        hooks
            .get("PreToolUse")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "the other tool's gate was removed with ours: {after:#}"
    );
}

/// Cursor's uninstall leaves the operator's file alone too: the same rule, a
/// separate stripper, written as its own test because a fix applied to one of
/// them once had not reached the other.
#[test]
fn cursors_uninstall_leaves_an_empty_event_the_operator_wrote_where_it_was() {
    let path = std::path::Path::new("hooks.json");

    let theirs = "{\n  \"hooks\": {\n    \"afterFileEdit\": []\n  }\n}\n";
    assert_eq!(
        render::strip_cursor_hooks(path, theirs).expect("a readable file"),
        theirs,
        "Cursor's uninstall rewrote a file that never mentioned Estigia, and took the operator's \
         own empty event with it"
    );

    let mixed = serde_json::json!({
        "hooks": {
            "afterFileEdit": [],
            "beforeShellExecution": [{"command": "estigia hook pre-tool-use --dialect cursor"}]
        }
    })
    .to_string();
    let after: serde_json::Value =
        serde_json::from_str(&render::strip_cursor_hooks(path, &mixed).expect("a readable file"))
            .expect("still json");
    let hooks = after
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .expect("the wrapper survives");
    assert!(
        hooks.contains_key("afterFileEdit"),
        "the operator's empty event was deleted by an uninstall: {after:#}"
    );
    assert!(
        !hooks.contains_key("beforeShellExecution"),
        "an event Estigia emptied was left behind as a key saying nothing: {after:#}"
    );
}

/// `strip_mcp` leaves the operator's empty MCP container where it was: the
/// empty-container rule it copied from `strip_hooks` was itself the defect.
#[test]
fn strip_mcp_leaves_an_empty_mcp_container_the_operator_wrote_where_it_was() {
    let path = std::path::Path::new("settings.json");

    // Theirs, unfilled, with nothing of Estigia's ever in it.
    for key in ["mcpServers", "mcp"] {
        let theirs = format!("{{\n  \"{key}\": {{}}\n}}\n");
        assert_eq!(
            render::strip_mcp(path, &theirs, render::McpFormat::McpServers).expect("readable"),
            theirs,
            "the MCP stripper rewrote a file that never mentioned Estigia, and took the \
             operator's own empty {key} with it"
        );
    }

    // Ours in it and nothing else: the container was emptied here, so it goes.
    let ours =
        serde_json::json!({"mcpServers": {"estigia": {"command": "estigia", "args": ["mcp"]}}})
            .to_string();
    let after: serde_json::Value = serde_json::from_str(
        &render::strip_mcp(path, &ours, render::McpFormat::McpServers).expect("readable"),
    )
    .expect("still json");
    assert!(
        after.get("mcpServers").is_none(),
        "a container Estigia emptied was left behind saying nothing: {after:#}"
    );

    // Somebody else's server keeps the key alive, as it always did.
    let shared = serde_json::json!({
        "mcpServers": {"estigia": {"command": "estigia"}, "theirs": {"command": "other"}}
    })
    .to_string();
    let after: serde_json::Value = serde_json::from_str(
        &render::strip_mcp(path, &shared, render::McpFormat::McpServers).expect("readable"),
    )
    .expect("still json");
    let servers = after
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .expect("their server keeps it alive");
    assert!(
        servers.contains_key("theirs") && !servers.contains_key("estigia"),
        "{after:#}"
    );
}

/// The empty-container rule above, carried through ZCode's second level.
#[test]
fn strip_mcp_leaves_zcodes_operator_written_empty_mcp_containers_where_they_were() {
    let path = std::path::Path::new("config.json");

    // Theirs, unfilled at either level, with nothing of Estigia's ever in it.
    for theirs in [
        "{\n  \"mcp\": {}\n}\n",
        "{\n  \"mcp\": {\n    \"servers\": {}\n  }\n}\n",
    ] {
        assert_eq!(
            render::strip_mcp(path, theirs, render::McpFormat::ZCode).expect("readable"),
            theirs,
            "the MCP stripper rewrote a file that never mentioned Estigia, and took the \
             operator's own empty container with it: {theirs:?}"
        );
    }

    // Ours in it and nothing else: both levels were emptied here, so both go.
    let ours = serde_json::json!({"mcp": {"servers": {"estigia": {"type": "stdio", "command": "estigia", "args": ["mcp"]}}}})
        .to_string();
    let after: serde_json::Value = serde_json::from_str(
        &render::strip_mcp(path, &ours, render::McpFormat::ZCode).expect("readable"),
    )
    .expect("still json");
    assert!(
        after.get("mcp").is_none(),
        "a container Estigia emptied at both levels was left behind saying nothing: {after:#}"
    );

    // Somebody else's server keeps both levels alive, as they always did.
    let shared = serde_json::json!({
        "mcp": {"servers": {"estigia": {"command": "estigia"}, "theirs": {"command": "other"}}}
    })
    .to_string();
    let after: serde_json::Value = serde_json::from_str(
        &render::strip_mcp(path, &shared, render::McpFormat::ZCode).expect("readable"),
    )
    .expect("still json");
    let servers = after
        .get("mcp")
        .and_then(|mcp| mcp.get("servers"))
        .and_then(serde_json::Value::as_object)
        .expect("their server keeps mcp.servers alive");
    assert!(
        servers.contains_key("theirs") && !servers.contains_key("estigia"),
        "{after:#}"
    );

    // A sibling of `servers`, not inside it: emptying `servers` removes it, and
    // `mcp` is the operator's until this call empties that too.
    let sibling = serde_json::json!({
        "mcp": {"servers": {"estigia": {"command": "estigia"}}, "enabled": true}
    })
    .to_string();
    let after: serde_json::Value = serde_json::from_str(
        &render::strip_mcp(path, &sibling, render::McpFormat::ZCode).expect("readable"),
    )
    .expect("still json");
    assert_eq!(
        after.get("mcp").and_then(|mcp| mcp.get("enabled")),
        Some(&serde_json::Value::Bool(true)),
        "an uninstall that emptied `mcp.servers` took the operator's sibling `mcp.enabled` \
         key with it: {after:#}"
    );
    assert!(
        after
            .get("mcp")
            .and_then(|mcp| mcp.get("servers"))
            .is_none(),
        "the `servers` container this uninstall emptied should still be gone: {after:#}"
    );
}

/// The call site, not `strip_mcp`: a file this uninstall changed nothing in is
/// not Estigia's to remove, even when every container in it is empty. The
/// stripper returns such a file byte for byte; the step above it must not read
/// that as an empty husk of Estigia's making.
#[test]
fn uninstall_leaves_an_mcp_config_it_changed_nothing_in_byte_identical() {
    for (slug, theirs) in [
        ("claude-code", "{\n  \"mcpServers\": {}\n}\n"),
        ("zcode", "{\n  \"mcp\": {}\n}\n"),
        ("zcode", "{\n  \"mcp\": {\n    \"servers\": {}\n  }\n}\n"),
    ] {
        let (_home, options) = sandbox();
        let adapter = agent(slug);
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let config = paths.mcp_config.expect("this agent reads an MCP config");
        fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
        fs::write(&config, theirs).expect("write their file");

        uninstall(adapter, &options).expect("uninstall runs");

        assert_eq!(
            fs::read_to_string(&config).unwrap_or_default(),
            theirs,
            "{slug}: an uninstall of a file it changed nothing in took it away"
        );
    }
}

/// Installing and uninstalling into an operator's Cursor file is a round trip:
/// what the operator wrote is what they get back.
#[test]
fn cursor_hooks_come_back_as_the_operator_wrote_them() {
    let path = std::path::Path::new("hooks.json");
    let spec = agent("cursor").gate_spec().expect("cursor can be gated");
    let executable = std::path::Path::new("/opt/estigia");

    for theirs in [
        // No version key of their own, which is the case that bit.
        "{\n  \"hooks\": {\n    \"afterFileEdit\": [\n      {\n        \"command\": \"mine\"\n      }\n    ]\n  }\n}\n",
        "{\n  \"version\": 1,\n  \"hooks\": {\n    \"afterFileEdit\": [\n      {\n        \"command\": \"mine\"\n      }\n    ]\n  }\n}\n",
    ] {
        let installed = render::render_cursor_hooks(path, Some(theirs), executable, "cursor", spec)
            .expect("the gate is written");
        assert!(
            installed.contains("estigia"),
            "the gate was not written at all, so the round trip proves nothing: {installed}"
        );
        assert_eq!(
            render::strip_cursor_hooks(path, &installed).expect("readable"),
            theirs,
            "an install followed by an uninstall did not give the operator their file back"
        );
    }
}

/// Codex's TOML comes back byte for byte when Estigia was never in it: the
/// TOML path rewrites unconditionally once reached, so the early return for a
/// file with nothing of Estigia's is what holds invariant two here.
#[test]
fn codexs_toml_is_untouched_when_it_never_mentioned_estigia() {
    let path = std::path::Path::new("config.toml");
    let theirs = "[mcp_servers.suyo]\r\ncommand = \"otro\"\r\n\r\n";
    assert_eq!(
        render::strip_mcp(path, theirs, render::McpFormat::CodexToml).expect("readable"),
        theirs,
        "an uninstall rewrote a TOML file that never mentioned Estigia"
    );

    // And with Estigia in it the section goes, which is the half that must not
    // regress.
    let mixed = "[mcp_servers.suyo]\ncommand = \"otro\"\n\n[mcp_servers.estigia]\ncommand = \"estigia\"\nargs = [\"mcp\"]\n";
    let after = render::strip_mcp(path, mixed, render::McpFormat::CodexToml).expect("readable");
    assert!(
        !after.contains("estigia") && after.contains("suyo"),
        "the section was not lifted out, or theirs went with it: {after:?}"
    );
}

/// An instruction file the operator kept empty survives the uninstall. Both
/// halves are asserted on purpose: never deleting leaves an empty file per
/// agent, always deleting takes a file that was theirs — only provenance
/// passes both.
#[test]
fn an_instruction_file_the_operator_left_empty_is_not_estigias_to_delete() {
    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let home = tempfile::tempdir().expect("a temporary directory");
                let options = SetupOptions {
                    home_dir: Some(home.path().to_path_buf()),
                    ..SetupOptions::default()
                };
                let paths = resolve_paths(adapter, &options).expect("paths resolve");

                // Theirs, and empty, before Estigia has been anywhere near it.
                if let Some(parent) = paths.instructions.parent() {
                    fs::create_dir_all(parent).expect("a directory to put it in");
                }
                fs::write(&paths.instructions, "").expect("their empty file");

                setup(adapter, &Config::default(), &options).expect("setup runs");
                assert!(
                    DIRECTIVE_FENCE.is_present(
                        &fs::read_to_string(&paths.instructions).expect("the file is readable")
                    ),
                    "{}: the directive was never written, so this proves nothing",
                    adapter.slug
                );

                uninstall(adapter, &options).expect("uninstall runs");
                assert!(
                    paths.instructions.is_file(),
                    "{}: an uninstall deleted {}, which was the operator's and empty before the install",
                    adapter.slug,
                    paths.instructions.display()
                );
                assert_eq!(
                    fs::read_to_string(&paths.instructions)
                        .expect("still readable")
                        .trim(),
                    "",
                    "{}: the directive block was left in a file Estigia was taken off",
                    adapter.slug
                );
            });
        }
    });
}

/// Every tool the classifier judges is one some matcher wakes the hook for:
/// the way back from `every_matcher_names_tools_the_gate_can_classify`.
#[test]
fn every_tool_the_classifier_judges_is_one_some_matcher_wakes_for() {
    use crate::harness::{SHELL_TOOLS, WRITE_TOOLS};

    // Names belonging to an agent that carries no matcher at all, spelled out
    // with the reason so a name arriving here is a decision somebody made.
    const NO_MATCHER_TO_BE_IN: &[(&str, &str)] = &[
        ("pre_write_code", "Windsurf, whose event is the tool name"),
        ("pre_run_command", "Windsurf, whose event is the tool name"),
        ("patch", "OpenCode, whose matcher lives in its plugin body"),
        (
            "shell",
            "Codex's own name for it; its hook layer sends `Bash`",
        ),
    ];

    let woken: Vec<String> = crate::setup::every_adapter()
        .filter_map(|adapter| adapter.gate_spec().and_then(|spec| spec.matcher))
        // Through the reader `doctor` uses on the matcher already on disk.
        .flat_map(wiring::names_in)
        .collect();

    let judged: Vec<&str> = WRITE_TOOLS.iter().chain(SHELL_TOOLS).copied().collect();
    for name in &judged {
        if woken.contains(&(*name).to_owned()) {
            continue;
        }
        assert!(
            NO_MATCHER_TO_BE_IN.iter().any(|(known, _)| known == name),
            "{name:?} is judged by the classifier and no matcher wakes the hook for it — a rule \
             this crate believes in and has arranged never to be asked about. Add it to the \
             matcher of the agent that sends it, or say here which matcher-less agent it belongs to"
        );
    }

    // Floors: a crossing over an empty side proves nothing.
    assert!(
        woken.len() >= 15,
        "only {} matcher names were read",
        woken.len()
    );
    assert!(
        judged.len() >= 12,
        "the classifier tables shrank to {}",
        judged.len()
    );
}

/// OpenCode's plugin gates the tools the classifier judges, and only those:
/// the fourth copy of one rule, and the only one in another language.
#[test]
fn the_plugin_gates_the_tools_the_classifier_judges() {
    use crate::harness::{SHELL_TOOLS, WRITE_TOOLS};

    let source = plugin::source(std::path::Path::new("/opt/estigia"));
    let start = source
        .find("const GATED = [")
        .expect("the plugin names its tools");
    let rest = &source[start + "const GATED = [".len()..];
    let end = rest.find(']').expect("the array closes");
    let gated: Vec<String> = rest[..end]
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .collect();

    let judged: Vec<&str> = WRITE_TOOLS.iter().chain(SHELL_TOOLS).copied().collect();
    for name in &gated {
        assert!(
            judged.contains(&name.as_str()),
            "the plugin wakes for {name:?}, which the classifier does not judge — the gate is \
             called and answers \"not mine\", once per call"
        );
    }

    // The other way, minus the names that belong to other hosts.
    const SOMEBODY_ELSE_S: &[(&str, &str)] = &[
        ("apply_patch", "Codex"),
        ("shell", "Codex's own name for its shell tool"),
        ("write_file", "Gemini CLI and Qwen"),
        ("replace", "Gemini CLI and Qwen"),
        ("run_shell_command", "Gemini CLI and Qwen"),
        ("pre_write_code", "Windsurf, whose event is the tool name"),
        ("pre_run_command", "Windsurf, whose event is the tool name"),
    ];
    for name in &judged {
        if gated.iter().any(|held| held == name) {
            continue;
        }
        assert!(
            SOMEBODY_ELSE_S.iter().any(|(known, _)| known == name),
            "the classifier judges {name:?} and OpenCode's plugin never calls the gate for it — \
             a write through it passes a gate `doctor` reports as live. Add it to `GATED`, or \
             say here which other agent it belongs to"
        );
    }

    // Floors: the crossing says nothing over an empty side.
    assert!(gated.len() >= 5, "the plugin gates only {}", gated.len());
    assert!(
        judged.len() >= 12,
        "the classifier tables shrank to {}",
        judged.len()
    );
}

/// A server entry says how to start the server, or it starts nothing.
///
/// `tools_command` read the `command` and stopped, so `doctor` reported
/// `running <path>` for an entry whose `args` no longer carried `mcp` — while
/// the host ran the binary with no subcommand, got its usage and exit `2`, and
/// every operation the agent asked for failed. Measured on the product.
///
/// Written after the row's own prove-RED silenced this reader and the suite
/// stayed green: a decision nothing tests is one that can be turned off without
/// anybody noticing.
#[test]
fn a_server_entry_says_how_to_start_the_server_or_it_starts_nothing() {
    let says = |args: serde_json::Value| says_mcp(&args);

    assert!(says(serde_json::json!(["mcp"])), "the entry Estigia writes");
    assert!(
        says(serde_json::json!(["-c", "estigia mcp"])),
        "a wrapper puts the subcommand inside an argument, and it still starts one"
    );

    assert!(!says(serde_json::json!([])), "empty args start nothing");
    assert!(
        !says(serde_json::json!(["--help"])),
        "an argument that is not the subcommand starts nothing"
    );
    // Not a list at all: nothing said, so nothing starts.
    assert!(says(serde_json::json!("mcp")), "a bare string is read too");
    assert!(!says(serde_json::json!(7)));
}

#[test]
fn a_file_with_nothing_of_estigias_in_it_comes_back_byte_for_byte() {
    // Invariant two, and the one an operator notices: an uninstall that took
    // nothing out must leave the file exactly as they wrote it. Every stripper
    // holds it the same way — parse, remove what is ours, and hand back the
    // **original text** when the parse is unchanged — because reserialising
    // rewrites their whitespace, reorders their keys, and reports `update` on a
    // run that removed nothing.
    //
    // Four places implement it and a mutation sweep turned all four off with
    // the suite green. The end-to-end check exists — a fake home, install,
    // uninstall, compare — but it runs by hand; this is the one that runs.
    let path = Path::new("theirs.json");
    // Deliberately unlovely: tabs, a trailing newline, keys out of order, and
    // spacing no serialiser would produce. That is the point — anything that
    // survives a round trip through `serde_json` proves nothing.
    let theirs = "{\n\t\"zebra\": 1,\n  \"hooks\":   {\n\t\t\"PreToolUse\": [ ]\n  },\n\t\"alpha\": [1,2,   3]\n}\n";

    /// One stripper, and the name it answers to when it fails.
    type Door = (&'static str, fn(&Path, &str) -> anyhow::Result<String>);
    let doors: [Door; 6] = [
        ("hooks", |path, text| render::strip_hooks(path, text)),
        ("cursor hooks", |path, text| {
            render::strip_cursor_hooks(path, text)
        }),
        ("mcp servers", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::McpServers)
        }),
        ("mcp pi", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::PiMcpServers)
        }),
        ("mcp local", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::Local)
        }),
        ("mcp zcode", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::ZCode)
        }),
    ];

    for (name, strip) in doors {
        let after = strip(path, theirs).expect("a readable file");
        assert_eq!(
            after, theirs,
            "{name}: a file with nothing of Estigia's in it did not come back as it was"
        );
    }
}

#[test]
fn a_repository_file_is_kept_up_to_date_and_never_created_from_nothing() {
    // The half that makes the layer usable: once a repository says it answers
    // for itself, saving keeps that file current. And the half that keeps it a
    // layer rather than a migration: a repository that has never said it is
    // left with no file at all, because creating one would move every
    // operator's rows out of the contract they are in today.
    let repo = tempfile::tempdir().expect("a repository");
    let path = crate::skill::repository_config_path(repo.path());
    assert!(
        !path.exists(),
        "the fixture already has one, so the half below measures nothing"
    );

    let config = Config {
        evidence: crate::config::Evidence::Measuring,
        ..Config::default()
    };
    super::write_repository_configuration(&path, &config, crate::config::EVERYWHERE_SETTINGS)
        .expect("it is written");
    let written = std::fs::read_to_string(&path).expect("it is there");
    assert!(
        written.contains("Integration"),
        "the repository's own rows are not in its own file: {written}"
    );
    assert!(
        !written.contains("Model routing"),
        "a row that is about an agent was written into the repository's file: {written}"
    );

    // Written again over itself, whatever the operator kept around the block.
    let theirs = format!("# My notes\n\nkeep me\n\n{written}");
    std::fs::write(&path, &theirs).expect("their file");
    super::write_repository_configuration(&path, &config, crate::config::EVERYWHERE_SETTINGS)
        .expect("it is written again");
    let after = std::fs::read_to_string(&path).expect("it is there");
    assert!(
        after.contains("keep me"),
        "what the operator wrote around the block did not survive: {after}"
    );
}

#[test]
fn a_repository_configuration_write_preserves_a_document_that_is_not_utf8() {
    let repo = tempfile::tempdir().expect("a repository");
    let path = crate::skill::repository_config_path(repo.path());
    std::fs::create_dir_all(path.parent().expect("the repository file has a parent"))
        .expect("the repository configuration directory exists");
    let invalid = [0xff, 0xfe, 0xfd];
    std::fs::write(&path, invalid).expect("the invalid repository document is written");

    let refusal = super::write_repository_configuration(
        &path,
        &Config::default(),
        crate::config::EVERYWHERE_SETTINGS,
    )
    .expect_err("the writer replaced a repository document it could not read");

    assert_eq!(refusal.code, "config-local-unreadable");
    assert_eq!(
        std::fs::read(path).expect("the repository document still exists"),
        invalid,
        "the writer replaced bytes it could not read"
    );
}

/// Uninstall leaves a checkout's own answers and the list of checkouts alone.
///
/// Both are Estigia's, and neither goes, which is what "removes what Estigia
/// created" needs qualifying by:
///
/// - A checkout's rows sit inside somebody's repository, and uninstall was
///   never given that repository. It takes an *agent* out. Removing files from
///   whichever checkout the operator happened to be standing in is the failure
///   this whole design refuses, and there is already one command that does it
///   on purpose and says which file it removed.
/// - The list of checkouts and the screen's language belong to the person, not
///   to any adapter. Uninstalling one of eleven agents must not take them, and
///   there is no last-one-out for a directory that is in no adapter's tree.
///
/// Measured, because prose about what a remover does not remove is the prose
/// most likely to be wrong: this ran `install` and `setup --uninstall` against a
/// sandbox holding both, and both were still there afterwards.
#[test]
fn uninstall_leaves_the_checkouts_answers_and_the_list_of_checkouts() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");

    // A checkout that answers for itself, and a machine that knows where it is.
    let repo = tempfile::tempdir().expect("a checkout");
    let rows = crate::skill::repository_config_path(repo.path());
    fs::create_dir_all(rows.parent().expect("that file has a directory")).expect("create the dir");
    let theirs = "| Setting | Value here | Skill default |\n|---|---|---|\n\
                  | Tracker | linear | github |\n";
    fs::write(&rows, theirs).expect("the checkout's rows are written");
    crate::skill::remember_repository(home.path(), repo.path());
    let list = crate::skill::known_repositories_path(home.path());
    assert!(list.is_file(), "the list was never written");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    uninstall(adapter, &options).expect("uninstall succeeds");

    // The floor: the agent's own files did go. A run that removed nothing at
    // all would satisfy both assertions below.
    assert!(
        !is_configured(adapter, &options),
        "uninstall removed nothing, so this measures nothing"
    );
    assert_eq!(
        fs::read_to_string(&rows).expect("the checkout's rows survive"),
        theirs,
        "uninstall reached into a repository it was never given"
    );
    assert!(
        list.is_file(),
        "uninstalling one agent took the machine's list of checkouts with it"
    );
}

/// A byte-order mark is not content, and a file carrying one is still JSON.
///
/// Windows is this crate's own platform, and Notepad and `Set-Content` both
/// write a UTF-8 BOM by default. `serde_json` refuses one, so `estigia install`
/// stopped on a perfectly good settings file with
/// *"is not JSON: expected value at line 1 column 1"* and a resolution asking
/// the operator for *"a JSON object in that file, or the file moved aside"* —
/// about a file that already holds one. The message names the wrong cause and
/// the way out destroys somebody's settings.
///
/// It is kept on the way out, not merely tolerated on the way in. The file was
/// written with it and this crate's own promise about these files is that what
/// it did not put there comes back byte for byte.
#[test]
fn a_byte_order_mark_is_not_content() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths
        .hooks
        .expect("claude-code registers its gate in a settings file");
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");

    let theirs = "{\n    \"model\": \"opus\"\n}\n";
    fs::write(&settings, format!("\u{feff}{theirs}")).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("a settings file with a BOM was refused");

    let after = fs::read_to_string(&settings).expect("their file is still there");
    assert!(
        after.starts_with('\u{feff}'),
        "the mark the file was written with is gone: {after:?}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(after.trim_start_matches('\u{feff}')).expect("it is still JSON");
    assert_eq!(
        parsed.get("model").and_then(|value| value.as_str()),
        Some("opus"),
        "the operator's own key did not survive: {after}"
    );
    assert!(
        parsed.get("hooks").is_some(),
        "the gate was not registered: {after}"
    );

    // The floor: a file without one still does not get one, so this is not a
    // change that writes a mark into everybody's settings.
    let (_home, options) = sandbox();
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths.hooks.expect("a settings file");
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");
    fs::write(&settings, theirs).expect("write their file");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        !fs::read_to_string(&settings)
            .expect("their file")
            .starts_with('\u{feff}'),
        "a mark was written into a file that had none"
    );
}

/// A file written with CRLF comes back with CRLF.
///
/// Invariant two says these files come back byte for byte, and the README says
/// it in those words: *Markdown comes back byte for byte, and so does JSON*.
/// Measured, on this crate's own platform, with a settings file and an
/// instruction file the operator wrote the ordinary Windows way: install and
/// uninstall gave both back with **every line ending changed**. Seven CRLF
/// became seven LF in one and three in the other — a whole-file diff in
/// somebody's version control, from a tool that was only supposed to add a
/// block and take it away again.
///
/// `write_file` already half knew: its comparison carried a CRLF rule of its
/// own — `found.replace("\r\n", "\n")` — so an unchanged CRLF file was correctly
/// read as unchanged. The comparison acknowledged the ending and the write did
/// not. That half rule is gone: the comparison now asks
/// [`super::as_the_file_was`] what would actually go on disk, which is the whole
/// of it. See
/// [`a_file_that_ends_without_a_newline_is_unchanged_the_second_time`] for the
/// half it did not know.
#[test]
fn a_file_written_with_crlf_comes_back_with_crlf() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths.hooks.expect("claude-code has a settings file");
    let instructions = paths.instructions;
    for file in [&settings, &instructions] {
        fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    }

    // One of them without a final newline, because every renderer here
    // finishes with one — right for a file this crate creates, and a byte added
    // to a file it did not. The round trip on the installed binary is what
    // found that: this fixture had ended both files the way the renderer does,
    // so it agreed with itself.
    let theirs = [
        (settings, "{\r\n    \"model\": \"opus\"\r\n}".to_owned()),
        (instructions, "# Mine\r\n\r\nDo it my way.\r\n".to_owned()),
    ];
    for (file, text) in &theirs {
        fs::write(file, text).expect("write their file");
    }

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    for (file, _) in &theirs {
        let after = fs::read_to_string(file).expect("their file is still there");
        assert!(
            after.contains("\r\n"),
            "{} came back with its line endings changed: {after:?}",
            file.display()
        );
        assert!(
            !after.replace("\r\n", "").contains('\n'),
            "{} came back with mixed line endings: {after:?}",
            file.display()
        );
    }

    uninstall(adapter, &options).expect("uninstall succeeds");
    for (file, text) in &theirs {
        assert_eq!(
            &fs::read_to_string(file).expect("their file survives"),
            text,
            "{} did not come back byte for byte",
            file.display()
        );
    }

    // The floor: a file written with LF is not handed back with CRLF, which is
    // the same defect facing the other way.
    let (_home, options) = sandbox();
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths.hooks.expect("a settings file");
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");
    fs::write(&settings, "{\n    \"model\": \"opus\"\n}\n").expect("write their file");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        !fs::read_to_string(&settings)
            .expect("their file")
            .contains('\r'),
        "a file written with LF came back with CRLF"
    );
}

/// A file written twice in one run keeps the endings it came with.
///
/// Gemini's `settings.json` is both its hooks file and its MCP file, so one
/// `setup` writes it twice. The second pass reads what the first left — through
/// `pending`, which is how this run avoids reading a file it has already
/// planned to change — and what `pending` held was the text *before* the
/// endings were put back. So eleven of twelve agent files came back byte for
/// byte and this one came back with its CRLF gone.
///
/// The rule is that what goes into `pending` is what goes onto disk. Anything
/// else is a second pass deciding against a file that never existed.
#[test]
fn a_file_written_twice_in_one_run_keeps_the_endings_it_came_with_on_an_earlier_retired_install() {
    let (_home, options) = sandbox();
    let adapter =
        crate::setup::find_agent("gemini-cli").expect("gemini-cli is an agent this build knows");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths
        .hooks
        .expect("gemini-cli registers hooks in a settings file");
    assert_eq!(
        paths.mcp_config.as_deref(),
        Some(settings.as_path()),
        "gemini-cli no longer writes one file twice, so this measures nothing"
    );
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");

    let theirs = "{\r\n    \"theme\": \"dark\"\r\n}\r\n";
    fs::write(&settings, theirs).expect("write their file");
    install_as_an_earlier_build(adapter, &options);

    let after = fs::read_to_string(&settings).expect("their file is still there");
    assert!(
        after.contains("\r\n") && !after.replace("\r\n", "").contains('\n'),
        "the second pass wrote against the first pass's text rather than the file: {after:?}"
    );
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&settings).expect("their file survives"),
        theirs,
        "the file did not come back byte for byte"
    );
}

/// Every agent's own files come back byte for byte, whatever they were written with.
///
/// The sweep, rather than one agent at a time. Three rounds of this were found
/// by hand — a byte-order mark refused outright, every line ending rewritten,
/// a trailing newline added, and then **one file of twelve** still losing its
/// endings because it is written twice in one run and the second pass read the
/// first pass's text. Each was fixed and the next one was found by running the
/// same sweep again.
///
/// So the sweep is the test. Every adapter, every file it edits, written the
/// ordinary Windows way — a byte-order mark, CRLF, and no final newline, which
/// is what Notepad and `Set-Content` leave — installed into and taken back out.
///
/// It is deliberately about files Estigia **did not create**: what it creates
/// it may shape as it likes, and what it found it hands back as it was.
#[test]
fn every_agents_own_files_come_back_byte_for_byte() {
    let swept = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for adapter in AGENTS {
            scope.spawn(|| {
                let (_home, options) = sandbox();
                let Ok(paths) = resolve_paths(adapter, &options) else {
                    return;
                };
                // One of each shape the adapter edits. The skill tree is Estigia's own
                // and is not in this: it creates that.
                let theirs: Vec<(std::path::PathBuf, String)> = [
                    Some((paths.instructions.clone(), "# Mine\r\n\r\nDo it my way.")),
                    paths
                        .hooks
                        .clone()
                        .map(|path| (path, "{\r\n    \"mine\": 1\r\n}")),
                    paths
                        .mcp_config
                        .map(|path| (path, "{\r\n    \"mine\": 2\r\n}")),
                ]
                .into_iter()
                .flatten()
                .map(|(path, text)| (path, format!("\u{feff}{text}")))
                .collect();

                let mut written: std::collections::BTreeMap<std::path::PathBuf, String> =
                    std::collections::BTreeMap::new();
                for (path, text) in &theirs {
                    // A file named twice by one adapter — Gemini's settings is both its
                    // hooks and its MCP file — is written once here, and the second
                    // spelling must not overwrite the first's fixture.
                    if written.contains_key(path) {
                        continue;
                    }
                    fs::create_dir_all(path.parent().expect("a parent")).expect("create the dir");
                    // TOML is not JSON, and an adapter whose file is one gets a shape
                    // it can parse. The endings are what this measures either way.
                    let text = if path.extension().is_some_and(|kind| kind == "toml") {
                        "\u{feff}# Mine\r\nmodel = \"mine\"".to_owned()
                    } else {
                        text.clone()
                    };
                    fs::write(path, &text).expect("write their file");
                    written.insert(path.clone(), text);
                }

                if setup(adapter, &Config::default(), &options).is_err() {
                    return;
                }
                uninstall(adapter, &options).expect("uninstall succeeds");

                for (path, text) in &written {
                    swept.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    assert_eq!(
                        &fs::read_to_string(path).expect("their file survives"),
                        text,
                        "{}: {} did not come back byte for byte",
                        adapter.slug,
                        path.display()
                    );
                }
            });
        }
    });
    // The floor: a sweep that skipped every adapter would pass in silence.
    let swept = swept.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        swept >= 12,
        "only {swept} operator files were swept, so this measured almost nothing"
    );
}

/// An uninstall leaves no empty husk of Estigia's own making.
///
/// A wrapper Estigia created is Estigia's to remove, and a wrapper the operator
/// wrote is not. `strip_hooks` learned that with a note calling it *"the same
/// rule one level up, and it had the same hole"* — and its two siblings kept
/// the hole, so Cursor and Windsurf came back with `"hooks": {}` sitting in a
/// file that never had one.
#[test]
fn an_uninstall_leaves_no_empty_husk_of_its_own_making() {
    for slug in ["cursor", "windsurf"] {
        let (_home, options) = sandbox();
        let adapter = agent(slug);
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let hooks = paths
            .hooks
            .expect("this agent registers a gate in a hooks file");
        fs::create_dir_all(hooks.parent().expect("a parent")).expect("create the dir");

        // Theirs, with no wrapper of their own.
        fs::write(&hooks, "{\n  \"mine\": 1\n}\n").expect("write their file");
        if adapter.retired() {
            install_as_an_earlier_build(adapter, &options);
        } else {
            setup(adapter, &Config::default(), &options).expect("setup succeeds");
        }
        assert!(
            fs::read_to_string(&hooks)
                .expect("their file")
                .contains("hooks"),
            "{slug}: the gate was never registered, so this measures nothing"
        );
        uninstall(adapter, &options).expect("uninstall succeeds");
        assert_eq!(
            fs::read_to_string(&hooks).expect("their file survives"),
            "{\n  \"mine\": 1\n}\n",
            "{slug}: an empty wrapper Estigia created outlived the uninstall"
        );

        // The other side is **not** asserted, and the honesty contract says
        // why: `without_scaffolding` removes an empty `hooks` on the reading
        // that Estigia put it there, and an operator who wrote one themselves
        // loses it. Measured — `estigia setup cursor --uninstall` on a machine
        // where nothing was ever installed rewrites that file. Telling the two
        // apart needs a record of what this crate created, which exists for the
        // instruction file and not for this one.
    }
}

/// What the honesty contract says about a JSON file is what happens to one.
///
/// That entry claimed a JSON file *"is pretty-printed with two spaces whatever
/// it looked like before"*, and four rounds of work had made it false without
/// anybody going back to it — the indentation, a byte-order mark, the line
/// endings and the final newline are all read off the original now. A contract
/// that lists what *used to be* missing is worse than no contract, which is
/// this crate's own rule about the honesty section and the one it broke.
///
/// So the remainder is measured here rather than asserted there: the four
/// properties come back, and the shaping **inside** the document does not. A
/// one-line array the operator wrote comes back over four lines, and a blank
/// line between two keys is gone.
#[test]
fn what_the_contract_says_about_a_json_file_is_what_happens_to_one() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths.hooks.expect("claude-code has a settings file");
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");

    let theirs =
        "\u{feff}{\r\n    \"model\": \"opus\",\r\n\r\n    \"allow\": [\"Read\", \"Grep\"]\r\n}";
    fs::write(&settings, theirs).expect("write their file");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after = fs::read_to_string(&settings).expect("their file is still there");

    // The four that are kept.
    assert!(after.starts_with('\u{feff}'), "the mark is gone: {after:?}");
    assert!(after.contains("\r\n"), "the endings are gone: {after:?}");
    assert!(
        !after.ends_with('\n'),
        "a final newline was added: {after:?}"
    );
    assert!(
        after.contains("\r\n    \"model\""),
        "the indentation is not the operator's: {after:?}"
    );

    // And the one that is not, which is what the entry now says out loud.
    assert!(
        !after.contains("[\"Read\", \"Grep\"]"),
        "the contract says a one-line array is reflowed and it was not: {after:?}"
    );
    assert!(
        after.contains("\"Read\"") && after.contains("\"Grep\""),
        "reflowing it lost the operator's values: {after:?}"
    );
}

/// The Windows hook hands the payload over as it arrived, not as ASCII.
///
/// Windows PowerShell encodes a string piped to a native command with
/// `$OutputEncoding`, and its default there is **`us-ascii`**. Measured on this
/// crate's own platform:
///
/// ```text
/// PS> $p = '{"file_path":"src/anio-ñ.rs"}'; $p | & echo_stdin
/// {"file_path":"src/anio-?.rs"}
/// ```
///
/// So every non-ASCII byte of a Cline tool call reached the gate as `?` — in a
/// script Estigia writes whole. What that costs is not cosmetic: a checkout
/// whose path carries an accent no longer matches the one the claim was made
/// in, so the write reads as `AnotherCheckout` and goes through **outside the
/// oath**. The gate decided about a path that does not exist.
///
/// PowerShell 7 already defaults to UTF-8, so the line is a no-op there and the
/// fix for 5.1 costs nothing.
#[test]
fn the_windows_hook_hands_the_payload_over_as_it_arrived() {
    let script = super::plugin::cline_hook(std::path::Path::new("C:/bin/estigia.exe"), true);
    assert!(
        script.contains("$OutputEncoding"),
        "the payload is piped through PowerShell's default encoding, which is ASCII: {script}"
    );
    // Both of them. Setting only the outgoing one left the payload arriving as
    // `src/anio-├▒.rs`: `[Console]::In.ReadToEnd()` decodes standard input with
    // `[Console]::InputEncoding` before anything is piped, so the first fix
    // turned one mangling into another. Measured with the generated script
    // itself, the binary swapped for a program that echoes its input.
    assert!(
        script.contains("$OutputEncoding = [System.Text.UTF8Encoding]"),
        "the outgoing pipe still encodes with PowerShell's default: {script}"
    );
    assert!(
        script.contains("[Console]::InputEncoding = [System.Text.UTF8Encoding]"),
        "standard input is still decoded with the OEM code page: {script}"
    );
    // The floor: the script still pipes what it read, and still stands aside
    // when the gate cannot answer.
    assert!(script.contains("[Console]::In.ReadToEnd()"));
    assert!(script.contains("exit 0"));

    // And the shell one needs none of this: a pipe there is bytes.
    let shell = super::plugin::cline_hook(std::path::Path::new("/usr/bin/estigia"), false);
    assert!(
        !shell.contains("$OutputEncoding"),
        "the shell script does not go through PowerShell and should not pretend to: {shell}"
    );
}

/// A gate result without a numeric exit code is a gate that did not answer.
///
/// `answer?.exitCode ?? 0` turned a process ended by a signal, a result with no
/// code and an absent result into an allow with nothing on standard error. This
/// drives the generated plugin under `node` with a stand-in shell, and it fails
/// rather than skips when `node` is missing.
#[test]
fn the_plugin_says_so_when_the_gate_returns_no_exit_code() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("estigia-workflow-authority.mjs"),
        crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the plugin writes");
    std::fs::write(
        root.path().join("drive.mjs"),
        r#"import { EstigiaWorkflowAuthority } from "./estigia-workflow-authority.mjs";

const run = async (label, answer) => {
  const stub = () => ({
    cwd() { return this; },
    quiet() { return this; },
    nothrow() { return Promise.resolve(answer); },
  });
  const hooks = await EstigiaWorkflowAuthority({ $: stub, directory: "/tmp", worktree: "/tmp" });
  const errors = [];
  const original = console.error;
  console.error = (line) => errors.push(String(line));
  let outcome;
  try {
    await hooks["tool.execute.before"]({ tool: "write", sessionID: "s" }, { args: {} });
    outcome = "allowed";
  } catch (error) {
    outcome = "refused";
  }
  console.error = original;
  return { label, outcome, errors };
};

console.log(JSON.stringify([
  await run("null", { exitCode: null, stderr: "" }),
  await run("absent", { stderr: "" }),
  await run("undefined", undefined),
  await run("zero", { exitCode: 0 }),
  await run("one", { exitCode: 1, stderr: "denied" }),
  await run("two", { exitCode: 2, stderr: "unread" }),
  await run("127", { exitCode: 127, stderr: "" }),
]));
"#,
    )
    .expect("the driver writes");

    let ran = match std::process::Command::new("node")
        .arg(root.path().join("drive.mjs"))
        .current_dir(root.path())
        .output()
    {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generated plugin, which is JavaScript, and \
             `node` could not be run ({error}). Install Node and re-run; it is \
             not skipped, because a plugin nothing executes is a gate nothing \
             measures."
        ),
    };
    assert!(
        ran.status.success(),
        "the plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    let seen = |label: &str| {
        results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"))
    };

    for label in ["null", "absent", "undefined", "127"] {
        let result = seen(label);
        assert_eq!(result["outcome"], "allowed", "{label}: {result}");
        let errors = result["errors"].as_array().expect("the stderr lines");
        assert_eq!(
            errors.len(),
            1,
            "{label} must say so exactly once: {result}"
        );
        assert!(
            errors[0]
                .as_str()
                .is_some_and(|line| line.contains("went out unchecked")),
            "{label}: {result}"
        );
    }
    let zero = seen("zero");
    assert_eq!(zero["outcome"], "allowed", "{zero}");
    assert!(
        zero["errors"].as_array().expect("lines").is_empty(),
        "{zero}"
    );
    for label in ["one", "two"] {
        assert_eq!(seen(label)["outcome"], "refused", "{label}");
    }

    for (name, source) in [
        (
            "gate",
            crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
        ),
        (
            "loop",
            crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
        ),
        (
            "relay",
            crate::setup::plugin::review_relay_source(std::path::Path::new("/opt/estigia")),
        ),
    ] {
        assert!(
            !source.contains("exitCode ?? 0"),
            "the {name} plugin still reads a missing exit code as success"
        );
    }
}

/// OpenCode's automatic mode answers the host's permission requests; the plugin
/// refuses through `tool.execute.before` and registers nothing on that path.
///
/// Drives the generated plugin under `node` the way both OpenCode generations
/// load it, with the host's automatic-mode fields on the hook input, and fails
/// rather than skips when `node` is missing.
#[test]
fn the_plugin_refuses_under_the_hosts_automatic_mode_and_registers_no_permission_hook() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("estigia-workflow-authority.mjs"),
        crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the plugin writes");
    std::fs::write(
        root.path().join("drive.mjs"),
        r#"import plugin, { EstigiaWorkflowAuthority } from "./estigia-workflow-authority.mjs";

const stub = (code) => () => ({
  cwd() { return this; },
  quiet() { return this; },
  nothrow() { return Promise.resolve({ exitCode: code, stderr: "estigia: issue-required" }); },
});
const modes = { auto: true, permission: "allow", approval: "auto" };
const attempt = async (call) => {
  try { await call(); return "allowed"; } catch (error) { return "refused"; }
};
const results = [];
for (const code of [1, 2]) {
  const hooks = await EstigiaWorkflowAuthority({ $: stub(code), directory: "/tmp", worktree: "/tmp" });
  results.push({
    label: `v1-${code}`,
    keys: Object.keys(hooks),
    outcome: await attempt(() => hooks["tool.execute.before"]({ tool: "write", sessionID: "s", ...modes }, { args: {} })),
  });
  globalThis.Bun = { $: stub(code) };
  const registered = [];
  const ctx = { location: { directory: "/tmp" }, tool: { hook: async (name, handler) => registered.push({ name, handler }) } };
  await plugin.setup(ctx);
  results.push({
    label: `v2-${code}`,
    keys: registered.map((entry) => entry.name),
    outcome: await attempt(() => registered[0].handler({ tool: "write", input: {}, sessionID: "s", ...modes })),
  });
}
console.log(JSON.stringify(results));
"#,
    )
    .expect("the driver writes");

    let ran = match std::process::Command::new("node")
        .arg(root.path().join("drive.mjs"))
        .current_dir(root.path())
        .output()
    {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generated plugin, which is JavaScript, and \
             `node` could not be run ({error}). Install Node and re-run; it is \
             not skipped, because a plugin nothing executes is a gate nothing \
             measures."
        ),
    };
    assert!(
        ran.status.success(),
        "the plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    assert_eq!(results.len(), 4, "{results:?}");
    for result in &results {
        assert_eq!(result["outcome"], "refused", "{result}");
    }
    for result in &results {
        let label = result["label"].as_str().expect("a label");
        let expected = if label.starts_with("v1") {
            serde_json::json!(["tool.execute.before", "tool.execute.after"])
        } else {
            serde_json::json!(["execute.before", "execute.after"])
        };
        assert_eq!(result["keys"], expected, "{result}");
    }
}

/// The plugin distinguishes a refusal from the gate not answering.
///
/// It caught every failure of the call and threw it as a refusal, so a binary
/// that is not there — a `cargo clean`, an uninstall, a moved profile — blocked
/// **every write in the session**, with a message that was not a refusal and a
/// fix nobody inside the agent could guess.
///
/// The other three scripts this crate writes settle it the other way and say so
/// in the same words: *a hook that breaks does not deny*, which is rule 3 of
/// `harness::hook`. The push guard reads the status rather than inheriting it,
/// and both Cline hooks carry the same paragraph. This one was the fourth, and
/// the only one in another language, so nothing crossed it.
#[test]
fn the_plugin_tells_a_refusal_from_a_gate_that_did_not_answer() {
    let source = crate::setup::plugin::source(std::path::Path::new("/usr/local/bin/estigia"));

    // The status is read, not inherited: an exception carries *something went
    // wrong* and no code at all.
    assert!(
        source.contains(".nothrow()"),
        "the plugin turns every failure into an exception again:\n{source}"
    );
    // Only the three codes Estigia defines are decisions.
    for spelling in ["status === 0", "status !== 1 && status !== 2"] {
        assert!(
            source.contains(spelling),
            "the plugin does not separate `{spelling}`:\n{source}"
        );
    }
    // And a code outside them lets the write through **with a word**. Doing it
    // silently is a different stance from doing it.
    assert!(
        source.contains("went out unchecked") && source.contains("console.error"),
        "the plugin lets an unanswered call through without saying so:\n{source}"
    );
    // The refusal is still thrown, or this fixed the false denial by denying
    // nothing at all.
    assert!(
        source.contains("throw new Error("),
        "the plugin no longer refuses anything:\n{source}"
    );

    // And it is still JavaScript. A template with nested backticks inside a Rust
    // raw string is exactly the shape that stops parsing without anybody
    // noticing — the file is only ever read by another program.
    //
    // Through **stdin with `--input-type=module`**, not `node --check <file>`.
    // That spelling was written first and it is inert here: given a path whose
    // contents carry `export`, node detects an ES module and answers `0` for a
    // file it cannot parse. Measured, same node, same bytes:
    //
    // ```text
    // node --check module-with-a-syntax-error.js   -> 0
    // node --check script-with-a-syntax-error.js   -> 1
    // node --input-type=module --check < the same  -> 1
    // ```
    //
    // So the first spelling passed on a plugin holding `const roto = ((;`. A
    // check that cannot fail reads exactly like one that ran.
    let mut node = match std::process::Command::new("node")
        .args(["--input-type=module", "--check"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(node) => node,
        // Node is not a dependency of this crate and the plugin runs on Bun.
        // Said out loud rather than passing quietly, for the same reason.
        Err(error) => {
            eprintln!("node was not run ({error}); the plugin's syntax is unchecked here");
            return;
        }
    };
    use std::io::Write;
    node.stdin
        .as_mut()
        .expect("a pipe to node")
        .write_all(source.as_bytes())
        .expect("the plugin is handed over");
    let checked = node.wait_with_output().expect("node answers");
    assert!(
        checked.status.success(),
        "the plugin this crate writes is not valid JavaScript:\n{}",
        String::from_utf8_lossy(&checked.stderr)
    );
}

/// OpenCode 2 refuses a plugin module with no default definition — its loader
/// records the shape it wants by name — so the file this crate writes must
/// carry one, with the id the host reports and the setup entrypoint it calls.
#[test]
fn the_plugin_declares_the_default_definition_opencode_two_loads() {
    let source = crate::setup::plugin::source(std::path::Path::new("/opt/estigia"));
    for spelling in [
        "export default {",
        "id: \"estigia-workflow-authority\"",
        "async setup(",
    ] {
        assert!(
            source.contains(spelling),
            "the rendered plugin does not carry `{spelling}`, so OpenCode 2 refuses it:\n{source}"
        );
    }
}

/// A comment under Estigia's TOML table is the operator's, and it stays.
///
/// The section ran to the next table header, so a `#` written under
/// `[mcp_servers.estigia]` went with the table — and when that table was last
/// in the file, everything to the end of the file went with it.
///
/// Measured through the binary before this, one `~/.codex/config.toml`: `setup
/// codex`, a note written after the table, `uninstall` — and the note was gone.
/// The same promise `fence::locate` had just been fixed for, in the second of
/// the three mechanisms that edit somebody else's file.
#[test]
fn a_comment_under_the_codex_table_is_not_estigias_to_remove() {
    let ours = concat!(
        "[mcp_servers.estigia]\n",
        "command = \"C:/estigia.exe\"\n",
        "args = [\"mcp\"]\n",
    );

    // The floor: the table really does go, and the operator's other sections
    // stay. Without this the assertions below could pass on a function that
    // removes nothing at all.
    let ordinary =
        format!("# mine, at the top\nmodel = \"gpt-5\"\n\n[tools]\nweb_search = false\n\n{ours}");
    let left = crate::setup::render::strip_codex_mcp(&ordinary);
    assert!(
        !left.contains("mcp_servers.estigia") && !left.contains("\"mcp\""),
        "the table Estigia wrote is still there:\n{left}"
    );
    assert!(
        left.contains("# mine, at the top") && left.contains("web_search = false"),
        "removal took a section that was not Estigia's:\n{left}"
    );

    // The note after it, with nothing following — the shape that lost the most,
    // because the section ran to the end of the file.
    let trailing = format!("{ordinary}\n# MI NOTA, escrita despues\n");
    let left = crate::setup::render::strip_codex_mcp(&trailing);
    assert!(
        left.contains("# MI NOTA, escrita despues"),
        "removal took a comment the operator wrote after the table:\n{left}"
    );
    assert!(
        !left.contains("mcp_servers.estigia"),
        "the table stopped being removed:\n{left}"
    );

    // And one with a table after it, so the comment is not merely the last
    // thing in the file.
    let between = format!("{ours}# MI NOTA ENTRE MEDIAS\n\n[otra]\nk = 1\n");
    let left = crate::setup::render::strip_codex_mcp(&between);
    assert!(
        left.contains("# MI NOTA ENTRE MEDIAS") && left.contains("[otra]"),
        "removal took a comment between two tables:\n{left}"
    );
}

#[test]
fn a_comment_inside_the_codex_table_does_not_orphan_its_keys() {
    let existing = concat!(
        "[tools]\n",
        "web_search = false\n",
        "\n",
        "[mcp_servers.estigia]\n",
        "# registered by estigia setup\n",
        "command = \"/usr/local/bin/estigia\"\n",
        "args = [\"mcp\"]\n",
    );
    let left = crate::setup::render::strip_codex_mcp(existing);
    let parsed: toml_edit::DocumentMut = left.parse().expect("still TOML");
    assert!(
        parsed["tools"].get("command").is_none() && parsed["tools"].get("args").is_none(),
        "Estigia's keys now sit under [tools]:\n{left}"
    );

    let twice = crate::setup::render::render_codex_mcp(
        &crate::setup::render::render_codex_mcp(existing, "/x"),
        "/x",
    );
    assert_eq!(twice.matches("[mcp_servers.estigia]").count(), 1, "{twice}");
    let parsed: toml_edit::DocumentMut = twice.parse().expect("still TOML");
    assert!(parsed["tools"].get("command").is_none(), "{twice}");
}

#[test]
fn a_comment_after_the_codex_tables_last_key_survives_a_comment_inside_it() {
    let existing = concat!(
        "[mcp_servers.estigia]\n",
        "# inside\n",
        "command = \"/x\"\n",
        "\n",
        "# after\n",
        "\n",
        "[otra]\n",
        "k = 1\n",
    );
    let left = crate::setup::render::strip_codex_mcp(existing);
    assert!(
        left.contains("# after") && left.contains("[otra]"),
        "{left}"
    );
    assert!(
        !left.contains("# inside") && !left.contains("command"),
        "{left}"
    );
}

/// And the other direction: every tool a matcher wakes for is one the
/// classifier judges.
///
/// Its sibling above runs one way — for every name the classifier judges, some
/// matcher wakes the hook. That catches a rule nobody is ever asked about. It
/// cannot catch the reverse: a matcher naming a tool the classifier has never
/// heard of, where the hook fires, `classify` answers `Untouched`, and the
/// write goes through — while `doctor` reports the gate as live and the
/// operator has every reason to believe that tool is gated.
///
/// It also costs what the matchers exist to avoid. Naming a tool rather than
/// `*` is deliberate: *waking a process for every read is a cost paid thousands
/// of times to answer "not mine"*. A name in a matcher and in neither list is
/// that cost, paid for nothing.
///
/// Measured by hand first and found clean, which is exactly when to write it
/// down: a property nobody can fail is one nobody has to keep.
#[test]
fn every_tool_a_matcher_wakes_for_is_one_the_classifier_judges() {
    use crate::harness::{SHELL_TOOLS, WRITE_TOOLS};

    let woken: Vec<(&str, String)> = AGENTS
        .iter()
        .filter_map(|adapter| {
            adapter
                .gate_spec()
                .and_then(|spec| spec.matcher)
                .map(|matcher| (adapter.slug, matcher))
        })
        .flat_map(|(agent, matcher)| {
            wiring::names_in(matcher)
                .into_iter()
                .map(move |name| (agent, name))
        })
        .collect();
    // The floor: the matchers were really read. An empty list agrees with
    // everything, which is how this crossing's sibling was once satisfied by a
    // copy of a matcher nothing installs.
    assert!(
        woken.len() >= 6,
        "only {} name(s) were read out of the matchers: {woken:?}",
        woken.len()
    );

    let judged: Vec<String> = WRITE_TOOLS
        .iter()
        .chain(SHELL_TOOLS)
        .map(|name| name.to_ascii_lowercase())
        .collect();
    for (agent, name) in &woken {
        assert!(
            judged.contains(&name.to_ascii_lowercase())
                || crate::harness::hook::is_prelaunch_tool(Some(agent), name),
            "{agent}'s matcher wakes the hook for {name:?}, but neither the repository classifier \
             nor a dedicated prelaunch gate owns it — the hook fires, answers `Untouched`, and the \
             call goes through while every report says it is gated"
        );
    }
}

/// Both managed blocks warn that editing inside them loses the edit.
///
/// Two blocks Estigia writes into somebody else's file and replaces whole: the
/// operator table's, and the directive's. The first has said so since it existed
/// — *this block is replaced whole on the next run, and an edit made in place is
/// lost without being reported*. The second said nothing at all.
///
/// Measured: an operator's line added inside the directive markers is gone after
/// `setup --uninstall`, with the block, and nothing anywhere had told them. The
/// same measurement showed the important half working — content **outside** the
/// markers survives and the file is kept — which is what makes the missing
/// sentence the whole defect.
///
/// Crossed rather than asserted twice, because the two sentences are one rule
/// and a rule written in two places is one that drifts.
#[test]
fn both_managed_blocks_say_an_edit_inside_them_is_lost() {
    let table = crate::skill::configuration_body(&Config::default());
    let directive = super::DIRECTIVE_TEMPLATE;

    for (which, text) in [
        ("the operator table", table.as_str()),
        ("the directive", directive),
    ] {
        assert!(
            text.contains("replaced whole on the next run"),
            "{which} does not warn that an edit inside it is replaced"
        );
        assert!(
            text.contains("without being reported"),
            "{which} does not say the loss is silent, which is the part that costs"
        );
    }

    // And the half that makes the warning true rather than merely present: the
    // directive names where an operator's own text is safe.
    assert!(
        directive.contains("outside"),
        "the directive warns about editing inside it and never says where to write instead"
    );
}

/// A second run over a file the operator left without a final newline says so.
///
/// Measured on the installed binary: `estigia sync` reported
/// `update C:\Users\alex\.claude.json` on three consecutive runs, and the
/// file's digest never moved once. The registration file ends `}` with no
/// newline — the ordinary way an editor leaves JSON — while every renderer here
/// finishes with one.
///
/// `write_file` compared what it found against `desired`, but what it *writes*
/// is [`super::as_the_file_was`] of it, which gives the missing final newline
/// back. So the two texts differed by exactly the byte the writer was about to
/// remove: `Update`, rewrite, identical bytes, for ever.
///
/// `Update` is this crate's word for *we wrote over what was there*. Saying it
/// about a write that changed nothing is the same failure as reporting a state
/// nobody read back, and to say it, it rewrote the operator's own settings file
/// on every run. Somebody watching `sync` for whether anything moved gets a
/// permanent yes.
#[test]
fn a_file_that_ends_without_a_newline_is_unchanged_the_second_time() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let settings = paths.hooks.expect("claude-code has a settings file");
    fs::create_dir_all(settings.parent().expect("a parent")).expect("create the dir");
    // As an editor leaves it, and as the real `.claude.json` on this machine is.
    fs::write(&settings, "{\n    \"model\": \"opus\"\n}").expect("write their file");

    setup(adapter, &Config::default(), &options).expect("the first run succeeds");
    let after_first = fs::read(&settings).expect("their file is there");
    let second = setup(adapter, &Config::default(), &options).expect("the second run succeeds");

    let action = second
        .actions
        .iter()
        .find(|action| action.path == settings)
        .expect("the settings file is among the second run's actions");
    assert_eq!(
        action.change,
        Change::Unchanged,
        "a run that wrote the same bytes reported {:?}",
        action.change
    );
    // The floor: the claim is only worth something because the bytes really did
    // not move. A `Unchanged` over a file that changed is the worse defect.
    assert_eq!(
        after_first,
        fs::read(&settings).expect("their file survives"),
        "the second run changed the file it called unchanged"
    );
    // And it still has no final newline, which is what made the two texts
    // differ. A run that quietly added one would make this pass by changing the
    // operator's file instead of the comparison.
    assert!(
        !after_first.ends_with(b"\n"),
        "the run added a final newline the operator had not written"
    );
}

/// Every hook Estigia registers says which agent it was registered in.
///
/// The command carried `--dialect` alone, and the ledger wrote that slug down
/// under the name `agent`. Eleven agents share five dialects, so an ungated call
/// from Codex, OpenCode, Continue or Windsurf was recorded as `claude-code`,
/// and `doctor`'s silence row — whose whole subject is *which agent sent a call
/// that went through ungated* — said `from claude-code` and sent the operator to
/// somebody else's settings file.
///
/// Through `setup`, not through `render::hook_command`. A test of the renderer
/// is not a test that the command written into an agent's settings carries it:
/// dropping `--agent` from the renderer left the whole suite green.
#[test]
fn every_registered_hook_names_the_agent_it_was_registered_in() {
    // Every file the run wrote, not the settings file this test picked out.
    // Picking one is how the first version of this missed Cline: it asked for
    // `paths.hooks` and skipped any adapter without a `gate_spec`, and Cline's
    // gate is a script `plugin` writes. Nine hook commands on a `setup --all`
    // and Cline's was the only one with no `--agent` in it — found by grepping
    // the tree, which is what this now does.
    fn commands_under(directory: &Path) -> Vec<String> {
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir(directory) else {
            return found;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(commands_under(&path));
            } else if let Ok(text) = fs::read_to_string(&path) {
                found.extend(
                    text.match_indices("hook pre-tool-use")
                        .map(|(at, _)| text[at..].lines().next().unwrap_or_default().to_owned()),
                );
            }
        }
        found
    }

    let checked = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for adapter in crate::setup::every_adapter() {
            if adapter.gate_spec().is_none() && adapter.instructions != InstructionFile::Cline {
                continue;
            }
            scope.spawn(|| {
                let (home, options) = sandbox();
                if adapter.retired() {
                    install_as_an_earlier_build(adapter, &options);
                } else {
                    setup(adapter, &Config::default(), &options).expect("setup runs");
                }
                let commands = commands_under(home.path());
                assert!(
                    !commands.is_empty(),
                    "{} was set up and registered no gate anywhere under its home",
                    adapter.display_name
                );
                for command in &commands {
                    assert!(
                        command.contains(&format!("--agent {}", adapter.slug)),
                        "{} registers a gate that does not say it is {}'s: {command}",
                        adapter.display_name,
                        adapter.slug
                    );
                    // The dialect is still there and is still a different fact.
                    // Replacing one with the other would pass the assertion above and
                    // break every refusal's shape.
                    assert!(
                        command.contains("--dialect "),
                        "{} lost the dialect its answers are written in: {command}",
                        adapter.display_name
                    );
                }
                checked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            });
        }
    });
    // The floor: a loop that checked nothing would pass every assertion in it,
    // and nine is the count a `setup --all` writes — eight through a settings
    // file and Cline's script.
    let checked = checked.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        checked >= 9,
        "only {checked} agent(s) with a gate were reached — this stopped covering the fleet"
    );
}

/// Whether this path is inside the installed skill tree.
///
/// Derived from [`crate::skill::DIRECTORY`] rather than spelled, because the two
/// tests below used to look for the literal `issue-flow` and that put the
/// directory's name in a third place. Renaming it left them treating the whole
/// payload as *the operator's own files*, and they then asserted that Estigia's
/// install record came back byte for byte — a demand nothing can satisfy.
///
/// Matched on a path **component**, not as a substring: the name is short now,
/// and `workflow/` or `dataflow.md` contains `flow` while being nobody's skill.
fn is_payload(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str() == crate::skill::DIRECTORY)
}

/// Taking Estigia out of a machine with every agent on it leaves their things
/// and none of its own.
///
/// The property the operator asked for in those words: *"al desinstalar borra
/// mi config y archivos míos, cuando debería desinstalar sólo la aplicación —
/// todo lo relacionado, pero sólo la app; si tengo otra skill, config propia o
/// mis .md, se deben quedar con lo mío."*
///
/// Every uninstall test beside this one takes one shape or one agent: an empty
/// event the operator wrote, another tool's block, a hook added beside
/// Estigia's. None of them walks the fleet, and the fleet is where the shapes
/// multiply — eleven adapters over markdown, three JSON dialects, a TOML file
/// and two plugin scripts. Run by hand it came back 0 of 27 lost and 0 left
/// behind; this is what keeps it that way.
#[test]
fn taking_estigia_out_of_a_full_machine_leaves_theirs_and_none_of_its_own() {
    let (home, options) = sandbox();
    setup_fleet(&options, &Config::default());

    /// Every file under the home that is not part of an installed skill.
    ///
    /// The payload is Estigia's whole and is meant to go; what must survive is
    /// everything it shares a directory with.
    fn theirs_to_plant(directory: &Path, found: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".estigia") {
                continue;
            }
            if path.is_dir() {
                theirs_to_plant(&path, found);
            } else if !is_payload(&path) {
                found.push(path);
            }
        }
    }

    const MARK: &str = "no-toques-esto";
    let mut planted: Vec<std::path::PathBuf> = Vec::new();
    let mut shared: Vec<std::path::PathBuf> = Vec::new();
    theirs_to_plant(home.path(), &mut shared);
    for file in &shared {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        if plugin::prompt_is_ours(Some(&text)) || plugin::command_is_ours(Some(&text)) {
            continue;
        }
        let name = file.to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".md") {
            fs::write(file, format!("{text}\n\n## Mine\n\n{MARK}\n")).expect("their file");
        } else if name.ends_with(".toml") {
            fs::write(file, format!("{text}\n# {MARK}\n")).expect("their file");
        } else if name.ends_with(".json") {
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let Some(object) = value.as_object_mut() else {
                continue;
            };
            object.insert("mine".to_owned(), serde_json::json!(MARK));
            fs::write(
                file,
                serde_json::to_string_pretty(&value).expect("their file serialises"),
            )
            .expect("their file");
        } else {
            continue;
        }
        planted.push(file.clone());
    }
    // And files that are wholly theirs, beside Estigia's, in each root it wrote
    // into. A directory Estigia created and shares is the case where "leave
    // theirs" and "leave no husk" pull in opposite directions.
    let mut mine: Vec<std::path::PathBuf> = Vec::new();
    for file in &shared {
        let Some(parent) = file.parent() else {
            continue;
        };
        let note = parent.join("notes-of-mine.md");
        if !note.exists() && fs::write(&note, format!("{MARK}\n")).is_ok() {
            mine.push(note);
        }
    }
    // The floor: a fixture that planted nothing would pass every assertion
    // below, and the fleet is the point.
    assert!(
        planted.len() >= 15 && mine.len() >= 4,
        "the fixture reached {} shared file(s) and {} of their own — too few to be the fleet",
        planted.len(),
        mine.len()
    );

    uninstall_fleet(&options);

    for file in planted.iter().chain(&mine) {
        let text = fs::read_to_string(file).unwrap_or_else(|error| {
            panic!(
                "{} was taken away by the uninstall: {error}",
                file.display()
            )
        });
        assert!(
            text.contains(MARK),
            "{} lost what the operator put in it",
            file.display()
        );
    }

    // And nothing of Estigia's is left: not a payload file, not a block, not an
    // entry naming the binary.
    let mut left: Vec<std::path::PathBuf> = Vec::new();
    theirs_to_plant(home.path(), &mut left);
    for file in &left {
        if file
            .file_name()
            .is_some_and(|name| name == "notes-of-mine.md")
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        // Both names: what this build installs, and what the previous one did.
        // A machine upgraded from `issue-flow` must not keep a trace of either.
        for trace in [
            "estigia",
            "ESTIGIA",
            crate::skill::DIRECTORY,
            "issue-flow",
            "hook pre-tool-use",
        ] {
            assert!(
                !text.contains(trace),
                "{} still says {trace:?} after the uninstall:\n{text}",
                file.display()
            );
        }
    }
    assert!(
        !home
            .path()
            .join(".agents")
            .join("skills")
            .join(crate::skill::DIRECTORY)
            .exists(),
        "the installed skill outlived the uninstall"
    );
}

/// Files that were the operator's **before** Estigia arrived come back byte for
/// byte after it leaves.
///
/// The sharpest form of the property, and the direction the fleet test beside
/// this one cannot take: that one plants after `setup`, so what a file holds at
/// the end is theirs *and* whatever Estigia leaves of its own removal. Planting
/// **first** makes the whole file theirs, and then the only correct end state is
/// the bytes they started with.
///
/// Measured by hand across eleven agents and every shape — markdown, three JSON
/// dialects, a TOML file, two plugin scripts — and 25 of 25 came back identical.
/// A marker surviving is not the same claim: an added newline, a reordered key
/// or a changed line ending is a whole-file diff in somebody's version control,
/// from a tool that was only supposed to add a block and take it away again.
#[test]
fn a_file_that_was_theirs_before_estigia_comes_back_byte_for_byte() {
    // Where this build writes, learned from a throwaway install rather than
    // from a list here: a list would be a second answer to the same question,
    // and the next adapter added would not be in it. One home per adapter so
    // the probe runs in parallel; the relative paths are what the fleet shares.
    fn shared(directory: &Path, found: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                shared(&path, found);
            } else if !is_payload(&path) {
                found.push(path);
            }
        }
    }
    let probed: Vec<std::path::PathBuf> = {
        let bag = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|scope| {
            for adapter in AGENTS {
                let bag = &bag;
                scope.spawn(move || {
                    let (probe, probe_options) = sandbox();
                    setup(adapter, &Config::default(), &probe_options).expect("setup runs");
                    let mut written = Vec::new();
                    shared(probe.path(), &mut written);
                    let relative: Vec<std::path::PathBuf> = written
                        .iter()
                        .filter_map(|path| {
                            path.strip_prefix(probe.path()).ok().map(Path::to_path_buf)
                        })
                        .collect();
                    bag.lock().expect("the probe bag").extend(relative);
                });
            }
        });
        bag.into_inner().expect("the probe bag")
    };
    let mut seen = std::collections::BTreeSet::new();
    let relative: Vec<std::path::PathBuf> = probed
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_AGENT.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_LENS.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_RISK.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_READABILITY.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_RELIABILITY.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_READBACK.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_REFUTER.path)))
        .filter(|path| !path.ends_with(Path::new(skill::REVIEW_VALIDATOR.path)))
        .filter(|path| {
            !path.ends_with(Path::new("agents").join(format!(
                "{}.md",
                crate::harness::roles::RESERVED_REVIEWER_SEALED
            )))
        })
        .collect();
    // The floor: a probe that found nothing would make every assertion below
    // vacuous, and the fleet is the point.
    assert!(
        relative.len() >= 15,
        "the probe found {} file(s) this build writes — too few to be the fleet",
        relative.len()
    );

    // A machine that is entirely the operator's, in the places Estigia will
    // want.
    let (home, options) = sandbox();
    let mut before: Vec<(std::path::PathBuf, Vec<u8>)> = Vec::new();
    for rel in &relative {
        let path = home.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("somewhere to put it");
        }
        let name = path.to_string_lossy().to_ascii_lowercase();
        let theirs = if name.ends_with(".json") {
            "{\n  \"mine\": \"no-toques-esto\"\n}".to_owned()
        } else if name.ends_with(".toml") {
            "# mine\nmi_clave = \"no-toques-esto\"\n".to_owned()
        } else {
            "# Mine\n\nno-toques-esto\n".to_owned()
        };
        fs::write(&path, &theirs).expect("their file");
        before.push((path, theirs.into_bytes()));
    }

    setup_fleet(&options, &Config::default());
    uninstall_fleet(&options);

    for (path, theirs) in &before {
        let after = fs::read(path).unwrap_or_else(|error| {
            panic!(
                "{} was theirs before Estigia and is gone after it: {error}",
                path.display()
            )
        });
        assert_eq!(
            String::from_utf8_lossy(&after),
            String::from_utf8_lossy(theirs),
            "{} did not come back byte for byte",
            path.display()
        );
    }
}

/// A plain `setup` retracts an owned retired phase definition, because nothing
/// installs one any more.
#[test]
fn setup_retracts_an_owned_retired_phase_agent() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let target = home
        .path()
        .join(".claude")
        .join("agents")
        .join("sdd-tasks.md");
    fs::write(
        &target,
        "---\nname: sdd-tasks\ntools: Read\n---\n\nA retired phase definition.\n",
    )
    .expect("a retired phase definition");
    skill::record::note_created_outside(&paths.skill_root, &target)
        .expect("the definition is recorded as Estigia's");

    setup(adapter, &Config::default(), &options).expect("setup runs again");
    assert!(
        !target.exists(),
        "an owned retired phase definition outlived the setup that retracts it"
    );
}

/// The retirement list is what retraction reads.
///
/// `sdd-verify` is a name the retired protocol defines and no build here ever
/// installed, so it is absent from the install set — and an owned definition at
/// that name must still come out. This goes red the moment uninstall consults
/// anything narrower than the retirement list.
#[test]
fn uninstall_deletes_an_owned_retired_phase_agent_without_consulting_phase_agents() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let target = home
        .path()
        .join(".claude")
        .join("agents")
        .join("sdd-verify.md");
    fs::write(
        &target,
        "---\nname: sdd-verify\ntools: Read\n---\n\nA retired phase definition.\n",
    )
    .expect("a retired phase definition");
    skill::record::note_created_outside(&paths.skill_root, &target)
        .expect("the definition is recorded as Estigia's");

    uninstall(adapter, &options).expect("uninstall runs");
    assert!(
        !target.exists(),
        "an owned retired phase definition outlived the uninstall"
    );
}

/// A definition this crate shipped once and no longer ships is taken off a
/// machine that has it.
///
/// `implementer.md` was renamed to `developer.md` and left every
/// list uninstall walks: `DELEGATED_AGENTS` names the new file, and no
/// retirement list named the old one, so an owned `implementer.md` stayed on
/// every machine that ever had it. Red before `RETIRED_AGENTS` named it: the
/// file was still present after the call below.
#[test]
fn uninstall_deletes_an_owned_retired_definition_without_consulting_delegated_agents() {
    assert!(
        !skill::RETIRED_AGENTS.is_empty(),
        "the retirement list is empty, so this test measures nothing"
    );
    for name in skill::RETIRED_AGENTS {
        let (home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let target = home.path().join(".claude").join("agents").join(name);
        fs::write(
            &target,
            "---\nname: implementer\n---\n\nA retired definition.\n",
        )
        .expect("a retired definition");
        skill::record::note_created_outside(&paths.skill_root, &target)
            .expect("the definition is recorded as Estigia's");

        uninstall(adapter, &options).expect("uninstall runs");
        assert!(
            !target.exists(),
            "{name}: an owned retired definition outlived the uninstall"
        );
    }
}

/// `setup` retracts an owned retired definition on the way in, and leaves one
/// somebody else owns — ownership decides, never the name.
#[test]
fn setup_retracts_an_owned_retired_definition_and_keeps_somebody_elses() {
    assert!(
        !skill::RETIRED_AGENTS.is_empty(),
        "the retirement list is empty, so this test measures nothing"
    );
    for name in skill::RETIRED_AGENTS {
        for owned in [true, false] {
            let (home, options) = sandbox();
            let adapter = agent("claude-code");
            setup(adapter, &Config::default(), &options).expect("setup runs");
            let paths = resolve_paths(adapter, &options).expect("paths resolve");
            let target = home.path().join(".claude").join("agents").join(name);
            fs::write(
                &target,
                "---\nname: implementer\n---\n\nA retired definition.\n",
            )
            .expect("a retired definition");
            if owned {
                skill::record::note_created_outside(&paths.skill_root, &target)
                    .expect("the definition is recorded as Estigia's");
            }

            setup(adapter, &Config::default(), &options).expect("setup runs again");
            assert_eq!(
                target.exists(),
                !owned,
                "{name} owned={owned}: setup kept or removed the wrong retired definition"
            );
        }
    }
}

/// A delegated worker is installed because the routing named it: naming the
/// role is the consent, and an agent definition is authority.
#[test]
fn a_delegated_worker_is_installed_because_the_row_named_it() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let developer = agents.join("developer.md");
    let analyst = agents.join("analyst.md");

    // `direct`, and no row: nothing is written — the compatibility constraint.
    setup(adapter, &Config::default(), &options).expect("setup runs");
    assert!(installed_phases(&agents).is_empty());
    assert!(
        !developer.exists(),
        "an unnamed worker was installed anyway"
    );
    assert!(!analyst.exists(), "an unnamed worker was installed anyway");

    // Named in the routing, still under `direct`: the definition appears, on
    // the model named beside it. Presence is consent.
    let named = Config {
        models: crate::config::ModelRouting::parse("developer=sonnet").expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &named, &options).expect("setup runs");
    let written = fs::read_to_string(&developer).expect("the worker is installed");
    assert!(
        written.contains("model: sonnet"),
        "the model named for `developer` did not reach its definition:\n{written}"
    );
    assert!(
        !written.contains("{{"),
        "a placeholder reached the disk:\n{written}"
    );
    assert!(
        !analyst.exists(),
        "naming one worker installed the other as well"
    );
    // No planning phase came with it: the two switches are separate.
    assert!(installed_phases(&agents).is_empty());

    // The grant is the role's, and the gate reads it off the installed file.
    let policy = crate::harness::roles::declared_policy(&written)
        .expect("an installed worker declares a policy the gate can read");
    assert_eq!(
        policy.verdict("Write"),
        crate::harness::roles::Verdict::Allow,
        "a developer that cannot write has not been made safer"
    );
    assert_eq!(
        policy.verdict("Bash"),
        crate::harness::roles::Verdict::Allow,
        "a developer that cannot run the suite cannot check its own work"
    );
    assert_eq!(
        policy.verdict("Task"),
        crate::harness::roles::Verdict::Deny,
        "the instruction not to delegate is refused rather than trusted"
    );

    // The read-only half, named beside it.
    let both = Config {
        models: crate::config::ModelRouting::parse("developer=sonnet, analyst=haiku")
            .expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &both, &options).expect("setup runs");
    let reader = fs::read_to_string(&analyst).expect("the analyst is installed");
    assert!(reader.contains("model: haiku"), "{reader}");
    let reading = crate::harness::roles::declared_policy(&reader)
        .expect("an installed worker declares a policy the gate can read");
    for withheld in ["Write", "Edit", "Bash"] {
        assert_eq!(
            reading.verdict(withheld),
            crate::harness::roles::Verdict::Deny,
            "the analyst is repository-read-only and was handed {withheld}"
        );
    }

    // Withdrawn: the name comes out of `Model routing`, and so does the
    // file this run created.
    setup(adapter, &named, &options).expect("setup runs");
    assert!(
        !analyst.exists(),
        "a worker outlived the row that installed it"
    );
    assert!(developer.exists(), "the named worker was retracted too");

    // And the uninstall takes back exactly what it wrote.
    uninstall(adapter, &options).expect("uninstall runs");
    assert!(
        !developer.exists(),
        "a worker this run created outlived the uninstall"
    );
}

/// The same row on OpenCode writes OpenCode's dialect under OpenCode's own
/// root, not Claude Code's: the defect in question.
#[test]
fn an_opencode_delegated_worker_is_written_in_opencode_dialect_under_its_own_root() {
    let (home, options) = sandbox();
    let adapter = agent("opencode");
    let root = home.path().join(".config").join("opencode").join("agents");
    let developer = root.join("developer.md");
    let analyst = root.join("analyst.md");

    // The base is OpenCode's configuration directory, never Claude Code's.
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(paths.agents_root, Some(root));
    assert!(
        !paths
            .agents_root
            .as_deref()
            .expect("a root")
            .starts_with(home.path().join(".claude")),
        "OpenCode's agents root landed under Claude Code's configuration"
    );
    let moved = tempfile::tempdir().expect("a relocated config home");
    let relocated = resolve_paths(
        adapter,
        &SetupOptions {
            config_home: Some(moved.path().to_path_buf()),
            ..options.clone()
        },
    )
    .expect("paths resolve");
    assert_eq!(
        relocated.agents_root,
        Some(moved.path().join("opencode").join("agents")),
        "OpenCode's agents root did not follow the XDG config home"
    );

    let config = Config {
        models: crate::config::ModelRouting::parse("developer=qwen/qwen3-coder; analyst=high")
            .expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &config, &options).expect("setup runs");

    let written = fs::read_to_string(&developer).expect("the developer is installed");
    assert!(written.starts_with("---\n"), "{written}");
    assert!(written.contains("mode: subagent"), "{written}");
    assert!(written.contains("model: qwen/qwen3-coder"), "{written}");
    assert!(written.contains("permission:"), "{written}");
    for line in [
        "edit: allow",
        "bash: allow",
        "webfetch: deny",
        "websearch: deny",
        "task: deny",
    ] {
        assert!(
            written.contains(line),
            "the developer's grant lost `{line}`:\n{written}"
        );
    }
    assert!(
        !written.contains("\ntools:"),
        "OpenCode's dialect kept Claude Code's `tools:` line:\n{written}"
    );
    assert!(
        !written.contains("name: developer"),
        "OpenCode derives the agent name from the file name, so a `name:` field is stray:\n{written}"
    );
    assert!(
        written.contains("You are the developer for one bounded piece of work."),
        "the template's body did not survive the dialect change:\n{written}"
    );

    let reader = fs::read_to_string(&analyst).expect("the analyst is installed");
    assert!(reader.contains("mode: subagent"), "{reader}");
    for line in ["edit: deny", "bash: deny", "webfetch: deny", "task: deny"] {
        assert!(
            reader.contains(line),
            "the analyst's read-only grant lost `{line}`:\n{reader}"
        );
    }
    assert!(
        !reader.contains("model:"),
        "a worker with no model named did not omit the line:\n{reader}"
    );
    assert!(
        !reader.contains("inherit"),
        "OpenCode does not read Claude's literal `inherit`:\n{reader}"
    );

    // The row's Claude Code half has not moved: no definition was written into
    // Claude Code's tree by an OpenCode install.
    assert!(
        !home.path().join(".claude/agents/developer.md").exists(),
        "an OpenCode setup wrote into Claude Code's agents root"
    );
}

#[test]
fn an_opencode_worker_named_inherit_writes_no_model_line() {
    let (home, options) = sandbox();
    let adapter = agent("opencode");
    let developer = home
        .path()
        .join(".config")
        .join("opencode")
        .join("agents")
        .join("developer.md");

    let config = Config {
        models: crate::config::ModelRouting::parse("developer=inherit").expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &config, &options).expect("setup runs");

    let written = fs::read_to_string(&developer).expect("the developer is installed");
    assert!(
        !written.contains("model:"),
        "OpenCode inherits when no model is named and does not read `inherit`, so the line must be absent:\n{written}"
    );
    assert!(
        !written.contains("inherit"),
        "the literal `inherit` reached an OpenCode definition:\n{written}"
    );
}

/// Pi reads a sub-agent definition from the agent directory's
/// `agents/` folder, in the example extension's `name`/`description`/`tools`/
/// `model` dialect, and only while `Model routing` names the role.
#[test]
fn a_pi_delegated_worker_is_written_in_pi_dialect_while_routing_names_it() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let root = home.path().join(".pi").join("agent").join("agents");
    let developer = root.join("developer.md");
    let analyst = root.join("analyst.md");

    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(paths.agents_root, Some(root));

    setup(adapter, &Config::default(), &options).expect("setup runs");
    assert!(
        !developer.exists() && !analyst.exists(),
        "a worker was installed with no role named"
    );

    let config = Config {
        models: crate::config::ModelRouting::parse("developer=anthropic/sonnet; analyst=high")
            .expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &config, &options).expect("setup runs");

    let written = fs::read_to_string(&developer).expect("the developer is installed");
    assert!(written.starts_with("---\nname: developer\n"), "{written}");
    assert!(written.contains("\ndescription: >\n"), "{written}");
    assert!(written.contains("\nmodel: anthropic/sonnet\n"), "{written}");
    assert!(
        written.contains("\ntools: read, grep, find, ls, write, edit, bash\n"),
        "{written}"
    );
    assert!(!written.contains("effort:"), "{written}");
    assert!(!written.contains("{{"), "{written}");
    assert!(
        written.contains("You are the developer for one bounded piece of work."),
        "{written}"
    );

    let reader = fs::read_to_string(&analyst).expect("the analyst is installed");
    assert!(reader.starts_with("---\nname: analyst\n"), "{reader}");
    assert!(
        reader.contains("\ntools: read, grep, find, ls\n"),
        "{reader}"
    );
    assert!(!reader.contains("model:"), "{reader}");
    assert!(!reader.contains("effort:"), "{reader}");

    let before = fs::read(&developer).expect("readable");
    let again = setup(adapter, &config, &options).expect("setup runs again");
    assert_eq!(fs::read(&developer).expect("readable"), before);
    assert_eq!(again.changed_files(), 0, "a second setup rewrote a file");

    let named = Config {
        models: crate::config::ModelRouting::parse("developer=anthropic/sonnet")
            .expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &named, &options).expect("setup runs");
    assert!(
        !analyst.exists(),
        "a worker outlived the row that installed it"
    );
    assert!(developer.exists());

    uninstall(adapter, &options).expect("uninstall runs");
    assert!(!developer.exists(), "a worker outlived the uninstall");

    assert!(
        !home.path().join(".claude/agents/developer.md").exists()
            && !home
                .path()
                .join(".config/opencode/agents/developer.md")
                .exists(),
        "a Pi setup wrote into another host's agents root"
    );
}

#[test]
fn a_pi_definition_estigia_did_not_write_is_left_alone() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let root = home.path().join(".pi").join("agent").join("agents");
    fs::create_dir_all(&root).expect("a root");
    let theirs = root.join("analyst.md");
    fs::write(
        &theirs,
        "---\nname: analyst\ndescription: theirs\n---\nmine\n",
    )
    .expect("written");

    let config = Config {
        models: crate::config::ModelRouting::parse("analyst=high").expect("a routing"),
        ..Config::default()
    };
    assert!(setup(adapter, &config, &options).is_err());
    setup(adapter, &Config::default(), &options).expect("setup runs");
    uninstall(adapter, &options).expect("uninstall runs");
    assert_eq!(
        fs::read_to_string(&theirs).expect("still there"),
        "---\nname: analyst\ndescription: theirs\n---\nmine\n"
    );
}

/// Reviewer definitions stay Claude-only: OpenCode gets the delegated workers
/// and nothing else.
#[test]
fn opencode_gets_no_reviewer_definitions() {
    let (home, options) = sandbox();
    let opencode = home.path().join(".config").join("opencode").join("agents");
    let claude = home.path().join(".claude").join("agents");
    let installed = |root: &Path| -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };

    setup(agent("opencode"), &Config::default(), &options).expect("setup runs");
    assert!(
        !opencode.exists() || installed(&opencode).is_empty(),
        "OpenCode's root received definitions it cannot read: {:?}",
        installed(&opencode)
    );

    // And the host this crate does render them for still gets them, and no
    // retired phase definition comes with them.
    setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
    let names = installed(&claude);
    assert!(
        !names.iter().any(|name| name.starts_with("sdd-")),
        "Claude Code received a retired planning phase: {names:?}"
    );
    assert!(
        names.iter().any(|name| name.starts_with("review-")),
        "Claude Code received no reviewer definitions: {names:?}"
    );
}

/// The ownership rule crosses to OpenCode's root: a name leaving the row
/// retracts only its file, uninstall takes back what it created, and a file
/// Estigia did not author is refused rather than overwritten.
#[test]
fn an_opencode_delegated_worker_is_retracted_and_a_foreign_file_is_refused() {
    let (home, options) = sandbox();
    let adapter = agent("opencode");
    let root = home.path().join(".config").join("opencode").join("agents");
    let developer = root.join("developer.md");
    let analyst = root.join("analyst.md");
    let both = Config {
        models: crate::config::ModelRouting::parse("developer=inherit, analyst=inherit")
            .expect("a routing"),
        ..Config::default()
    };

    setup(adapter, &both, &options).expect("setup runs");
    assert!(
        developer.exists() && analyst.exists(),
        "the pair was not installed"
    );

    let one = Config {
        models: crate::config::ModelRouting::parse("developer=inherit").expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &one, &options).expect("setup runs");
    assert!(developer.exists(), "the named worker was retracted too");
    assert!(
        !analyst.exists(),
        "a worker outlived the role that installed it"
    );

    uninstall(adapter, &options).expect("uninstall runs");
    assert!(
        !developer.exists(),
        "a worker this run created outlived the uninstall"
    );

    let (home, options) = sandbox();
    let theirs = home.path().join(".config/opencode/agents/analyst.md");
    fs::create_dir_all(theirs.parent().expect("a parent")).expect("the agents directory exists");
    let bytes = "---\ndescription: >\n  somebody else's analyst\nmode: subagent\n---\n\nnot ours\n";
    fs::write(&theirs, bytes).expect("their analyst exists");

    let failure =
        setup(adapter, &both, &options).expect_err("setup replaced a definition it did not write");
    let refusal = failure
        .downcast_ref::<crate::outcome::Refusal>()
        .expect("a refusal, not an io error");
    assert_eq!(refusal.code, "delegated-definition-unowned");
    assert_eq!(
        fs::read_to_string(&theirs).expect("their file still reads"),
        bytes
    );
}

/// A root and its renderer arrive together for every adapter.
#[test]
fn a_delegated_definition_root_and_a_renderer_arrive_together() {
    let template = skill::DELEGATED_AGENTS
        .first()
        .expect("a shipped worker")
        .1
        .contents;
    for adapter in AGENTS {
        let rendered =
            render_delegated_definition(adapter, template, "developer", &Config::default());
        assert_eq!(
            rendered.is_some(),
            adapter.has_agent_definitions(),
            "{} answers a root and a renderer that disagree",
            adapter.slug
        );
    }
}

/// The two values the row already took render exactly what they
/// rendered before `by task` existed, byte for byte, on every adapter.
#[test]
fn unset_and_explicit_pairs_render_byte_for_byte_on_every_adapter() {
    let template = skill::DELEGATED_AGENTS
        .first()
        .expect("a shipped worker")
        .1
        .contents;
    let explicit =
        crate::config::ModelRouting::parse("developer=sonnet; developer=low").expect("a routing");
    let by_task =
        crate::config::ModelRouting::parse(crate::config::BY_TASK).expect("the third value parses");

    let claude_unset = render_delegated_agent(template, "developer", &Config::default());
    let expected_unset = template
        .replace("{{MODEL}}", "inherit")
        .split_inclusive('\n')
        .filter(|line| line.trim() != "{{EFFORT}}")
        .collect::<String>();
    assert_eq!(
        claude_unset, expected_unset,
        "unset renders something other than the template's own inherit"
    );

    let claude_paired = render_delegated_agent(
        template,
        "developer",
        &Config {
            models: explicit.clone(),
            ..Config::default()
        },
    );
    let expected_paired = template
        .replace("{{MODEL}}", "sonnet")
        .replace("{{EFFORT}}", "effort: low");
    assert_eq!(
        claude_paired, expected_paired,
        "an explicit pair renders something other than its model and effort"
    );

    for adapter in AGENTS {
        let unset = render_delegated_definition(adapter, template, "developer", &Config::default());
        let paired = render_delegated_definition(
            adapter,
            template,
            "developer",
            &Config {
                models: explicit.clone(),
                ..Config::default()
            },
        );
        let third = render_delegated_definition(
            adapter,
            template,
            "developer",
            &Config {
                models: by_task.clone(),
                ..Config::default()
            },
        );
        assert_eq!(
            unset, third,
            "{} renders `by task` differently from unset, so a value that names no role changed \
             the definitions",
            adapter.slug
        );
        assert_eq!(
            unset.is_some(),
            paired.is_some(),
            "{}: a value the row already took changed whether a root renders",
            adapter.slug
        );
        if let (Some(unset), Some(paired)) = (unset, paired) {
            match adapter.instructions {
                InstructionFile::ClaudeCode => {
                    assert!(
                        unset.contains("model: inherit"),
                        "{}: {unset}",
                        adapter.slug
                    );
                    assert!(
                        paired.contains("model: sonnet"),
                        "{}: {paired}",
                        adapter.slug
                    );
                    assert!(paired.contains("effort: low"), "{}: {paired}", adapter.slug);
                }
                InstructionFile::OpenCode => {
                    assert!(
                        !unset.contains("model:"),
                        "{} inherits but named a model: {unset}",
                        adapter.slug
                    );
                    assert!(
                        paired.contains("model: sonnet"),
                        "{}: {paired}",
                        adapter.slug
                    );
                    assert!(
                        !paired.contains("effort:"),
                        "{} wrote an effort field it does not have: {paired}",
                        adapter.slug
                    );
                }
                _ => {}
            }
            assert!(!unset.contains("{{"), "{}: {unset}", adapter.slug);
            assert!(!paired.contains("{{"), "{}: {paired}", adapter.slug);
        }
    }
}

/// OpenCode has an agent-definitions root, so naming a role in
/// `Model routing` installs a definition there. No row is drawn as having no
/// effect any more — `Delegated workers` was the only one and it is gone.
#[test]
fn the_opencode_model_routing_row_is_held() {
    use crate::config::Setting;

    let opencode = agent("opencode");
    assert!(opencode.has_agent_definitions());
    assert_eq!(opencode.applies(Setting::Models), Applies::Held);

    let without = crate::setup::every_adapter()
        .filter(|adapter| !adapter.has_agent_definitions())
        .collect::<Vec<_>>();
    assert_eq!(without.len(), 11, "the set of rootless adapters moved");
    for adapter in without {
        assert_eq!(
            adapter.applies(Setting::Models),
            Applies::Held,
            "{} answers `Model routing` differently",
            adapter.slug
        );
    }
}

/// Naming a role in the routing is what installs it, presets included.
#[test]
fn a_routing_key_is_what_installs_a_worker() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let claude = find_agent("claude-code").expect("the Claude adapter");

    // Every shipped preset names `developer`, so installing one installs the
    // definition as well: presence in the roles section is consent.
    for profile in claude.model_profiles() {
        let routing = profile.routing().expect("a valid built-in profile");
        let model = routing
            .for_target("developer")
            .expect("the preset names a model for `developer`")
            .to_owned();
        let _ = fs::remove_file(agents.join("developer.md"));
        setup(
            adapter,
            &Config {
                models: routing,
                ..Config::default()
            },
            &options,
        )
        .expect("setup runs");
        let written = fs::read_to_string(agents.join("developer.md"))
            .unwrap_or_else(|_| panic!("the {} preset did not install `developer`", profile.name));
        assert!(
            written.contains(&format!("model: {model}")),
            "the {} preset's model did not reach the definition:\n{written}",
            profile.name
        );
    }

    // And a role named with no model of its own still installs, inheriting the
    // parent's model at the effort named beside it.
    let _ = fs::remove_file(agents.join("developer.md"));
    setup(
        adapter,
        &Config {
            models: crate::config::ModelRouting::parse("; developer=high").expect("a routing"),
            ..Config::default()
        },
        &options,
    )
    .expect("setup runs");
    let written = fs::read_to_string(agents.join("developer.md")).expect("the worker installs");
    assert!(
        written.contains("model: inherit"),
        "a role named with no model did not inherit:\n{written}"
    );
    assert!(written.contains("effort: high"), "{written}");
}

/// A definition Estigia did not author is refused, not replaced — in the
/// preflight, so the refusal leaves nothing half-written behind it.
#[test]
fn a_delegated_definition_estigia_did_not_author_is_refused_before_any_artifact_is_written() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let theirs = home.path().join(".claude/agents/analyst.md");
    fs::create_dir_all(theirs.parent().unwrap()).expect("the agent directory exists");
    let bytes = "---\nname: analyst\n---\n\nSomebody else's analyst.\n";
    fs::write(&theirs, bytes).expect("their analyst exists");

    let config = Config {
        models: crate::config::ModelRouting::parse("analyst=inherit").expect("a routing"),
        ..Config::default()
    };
    let failure = setup(adapter, &config, &options)
        .expect_err("setup replaced a definition it did not write");
    let refusal = failure
        .downcast_ref::<crate::outcome::Refusal>()
        .expect("a refusal, not an io error");
    assert_eq!(refusal.code, "delegated-definition-unowned");
    // Both ways out, asserted separately: written as one `||` they read as two
    // checks and were one.
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("move"), "{resolution}");
    assert!(resolution.contains("Model routing"), "{resolution}");
    assert!(resolution.contains("analyst"), "{resolution}");

    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert!(!paths.skill_root.exists(), "the skill was written anyway");
    assert!(
        !paths.instructions.exists(),
        "the directive was written anyway"
    );
    assert_eq!(fs::read_to_string(&theirs).unwrap(), bytes);

    // Stop naming it and the same machine installs cleanly.
    setup(adapter, &Config::default(), &options).expect("setup runs once the row lets it");
    assert_eq!(fs::read_to_string(&theirs).unwrap(), bytes);
}

/// The second ownership look is crossed where a fixture can reach it: the
/// preflight asks the **disk**, the write pass asks **`pending`**, so a
/// definition present in `pending` and absent from disk arrives at the second
/// observation. It is not the race itself — `docs/honesty.md` says so.
#[test]
fn the_second_ownership_look_refuses_what_arrives_after_the_first() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let target = agents.join("analyst.md");
    let config = Config {
        models: crate::config::ModelRouting::parse("analyst=inherit").expect("a routing"),
        ..Config::default()
    };

    // Nothing on disk, so the preflight has nothing to refuse — and somebody
    // else's definition already in this run's view of the world.
    let mut pending = Pending::new();
    pending.insert(
        target.clone(),
        Some("Somebody else's analyst.\n".to_owned()),
    );
    assert!(!target.exists(), "the fixture must not plant it on disk");

    let failure = setup_adapter_into(
        adapter,
        &Config::default(),
        &config,
        &options,
        &mut pending,
        false,
    )
    .expect_err("the write pass replaced a definition it did not write");
    let refusal = failure
        .error
        .downcast_ref::<crate::outcome::Refusal>()
        .expect("a refusal, not an io error");
    assert_eq!(refusal.code, "delegated-definition-unowned");
    let resolution = format!("{:?}", refusal.resolution);
    assert!(resolution.contains("move"), "{resolution}");
    assert!(resolution.contains("Model routing"), "{resolution}");

    // And it does not fire on a definition this run created.
    let (home, options) = sandbox();
    let agents = home.path().join(".claude").join("agents");
    setup(adapter, &config, &options).expect("the first run installs it");
    let ours = fs::read_to_string(agents.join("analyst.md")).expect("it is installed");
    let mut pending = Pending::new();
    pending.insert(agents.join("analyst.md"), Some(ours));
    setup_adapter_into(
        adapter,
        &Config::default(),
        &config,
        &options,
        &mut pending,
        false,
    )
    .expect("the second look refused a definition Estigia owns");
}

/// A dry run writes nothing and claims no ownership; a second run repeats it.
#[test]
fn a_delegated_worker_is_planned_before_it_is_written_and_settles_after() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let config = Config {
        models: crate::config::ModelRouting::parse("developer=inherit").expect("a routing"),
        ..Config::default()
    };

    let planned = setup(
        adapter,
        &config,
        &SetupOptions {
            dry_run: true,
            ..options.clone()
        },
    )
    .expect("the dry run plans");
    assert!(
        planned
            .actions
            .iter()
            .any(|action| action.kind == ActionKind::DelegatedAgent
                && action.change == Change::Create),
        "the dry run did not plan the worker: {:?}",
        planned.actions
    );
    assert!(
        !agents.join("developer.md").exists(),
        "a dry run wrote the definition"
    );

    let first = setup(adapter, &config, &options).expect("setup runs");
    assert!(
        first
            .actions
            .iter()
            .any(|action| action.kind == ActionKind::DelegatedAgent
                && action.change == Change::Create)
    );

    // Idempotent: the second run has nothing to do, which is what lets `setup`
    // be re-run to repair one artifact without disturbing the rest.
    let again = setup(adapter, &config, &options).expect("setup runs again");
    assert!(
        again
            .actions
            .iter()
            .any(|action| action.kind == ActionKind::DelegatedAgent
                && action.change == Change::Unchanged),
        "a second identical run rewrote the worker: {:?}",
        again.actions
    );
}

/// A reported change is what is on disk afterwards, not what was planned.
/// `write_file` decided `Create`/`Update` before the write, so a
/// swap that never landed was still reported as one. Planned `Update` with the
/// rename silently taking no effect errors instead of reporting an update over
/// bytes that never moved — under the old code the write succeeded falsely
/// and this reddened at the invariant below. A reader may change the planned
/// values but not the rule: whatever `Ok` carries must match the disk.
#[test]
fn a_reported_change_matches_what_is_on_disk_afterwards() {
    let root = tempfile::tempdir().expect("a temporary root");
    let target = root.path().join("settings.json");
    std::fs::write(&target, "old\n").expect("the file starts somewhere");

    crate::paths::inject_silent_swap_noop_for(&target);
    write_file(&target, Some("old\n"), "new\n", ActionKind::Skill, false)
        .expect_err("a write that never landed reported a change");
    assert_eq!(
        std::fs::read_to_string(&target).expect("the disk reads"),
        "old\n",
        "the failed write left anything but the bytes it found"
    );

    let created = root.path().join("fresh.json");
    let action =
        write_file(&created, None, "x\n", ActionKind::Skill, false).expect("a create lands");
    assert_eq!(action.change, Change::Create);
    assert_eq!(
        std::fs::read_to_string(&created).expect("the disk reads"),
        "x\n"
    );
    let action = write_file(&target, Some("old\n"), "new\n", ActionKind::Skill, false)
        .expect("an update lands");
    assert_eq!(action.change, Change::Update);
    assert_eq!(
        std::fs::read_to_string(&target).expect("the disk reads"),
        "new\n"
    );
    let action = write_file(&target, Some("new\n"), "new\n", ActionKind::Skill, false)
        .expect("an identical rewrite is skipped");
    assert_eq!(action.change, Change::Unchanged);
}

/// An effort reaches the definition, and an unnamed one writes no field at all.
#[test]
fn an_effort_named_beside_a_model_reaches_the_definition() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let config = Config {
        models: crate::config::ModelRouting::parse("developer=sonnet; developer=low")
            .expect("a routing"),
        ..Config::default()
    };

    setup(adapter, &config, &options).expect("setup runs");

    let worker = fs::read_to_string(agents.join("developer.md")).expect("the worker is installed");
    assert!(worker.contains("model: sonnet"), "{worker}");
    assert!(
        worker.contains("effort: low"),
        "the effort named for `developer` did not reach its definition:\n{worker}"
    );
    assert!(
        !worker.contains("sonnet/low"),
        "the effort was left on the model line:\n{worker}"
    );
    assert!(
        !worker.contains("{{"),
        "a placeholder reached the disk:\n{worker}"
    );

    // A definition nobody named an effort for carries no such line, and no
    // blank one where the placeholder was.
    let paired = Config {
        models: crate::config::ModelRouting::parse("developer=sonnet, analyst=opus")
            .expect("a routing"),
        ..Config::default()
    };
    setup(adapter, &paired, &options).expect("setup runs");
    let reader = fs::read_to_string(agents.join("analyst.md")).expect("the worker is installed");
    assert!(
        !reader.contains("effort:"),
        "an unnamed effort wrote a field anyway:\n{reader}"
    );
    assert!(
        reader.contains("model: opus\ntools:"),
        "the placeholder left a blank line behind:\n{reader}"
    );
    assert!(
        !reader.contains("{{"),
        "a placeholder reached the disk:\n{reader}"
    );
}

fn replace_once(text: &str, needle: &str, replacement: &str) -> String {
    assert_eq!(
        text.matches(needle).count(),
        1,
        "a frozen-body step no longer matches exactly once:\n{needle}"
    );
    text.replacen(needle, replacement, 1)
}

/// The default rendering is the bytes this binary writes for `reading`.
/// `reviewer_is_static` compares bytes, so the literal below is the current
/// asset. Updating it without adding the previous current body to the
/// historical ledger refuses every existing install at setup — do not update
/// it to make a wording change pass.
#[test]
fn the_default_rendering_is_the_bytes_already_installed() {
    let (root, path) = unrecorded_reviewer_path();
    const READING_BEFORE_BASIS: &str = "---\nname: review-blind\ndescription: >\n  Independently review one immutable publication receipt against the supplied criteria. The\n  orchestrator launches this same definition twice or five times for a blind panel.\nmodel: inherit\ntools: Read, Grep, Glob\n---\n\nYou are one blind reviewer. Review the exact immutable target and criteria in your prompt. Do not\nsubstitute a newer head, a different digest, or criteria inferred from the implementation.\n\nThis definition is inert unless the launch prompt names an active blind mode, the exact publication\nreceipt, and the review criteria. Without all three, stop and report that the panel invocation is\nincomplete.\n\nRemain read-only. Do NOT edit files, run a shell, change tracker state, record the aggregate verdict,\nor repair a finding. Return findings to the orchestrator with severity, precise evidence, and a stable\nidentity based on the affected behavior and location.\n\nYour starting context is this definition and the launch prompt. You issue no tracker operations.\n\nEvery observation you record carries exactly one of three classes. The rule for choosing is here\nrather than in the launch prompt, because a rule that arrives only when somebody remembers it does\nnot arrive.\n\n`severe` is a reproducible correctness, security, reliability or contract defect with concrete\nimpact. It alone can block, and it states `evidence_class` and `causal_disposition`. `warning` is a\nbounded risk with no demonstrated incorrectness. `suggestion` is optional improvement, cosmetic\nwording, style or preference.\n\nCosmetic, style and preference observations are `suggestion` unless they name a concrete defect, in\nwhich case say which and give it that defect's class. Incorrect or stale comments and documentation\nare observations about the words beside the behaviour, not about the behaviour itself: without a\ndemonstrated cost a mismatch is a `warning`, and editorial drift alone stays a `suggestion`. A\nreader who might be misled is the hypothetical this excludes, not a cost.\n\nThe precision gate: if concrete evidence and material impact cannot both be stated, omit the\nobservation or keep it as a non-blocking note. Identity comes from the affected behaviour and\nlocation, never from the wording or from you.\n\nDo NOT delegate, call the Task tool, or launch sub-agents. Do NOT read, request, or infer the output of\nother judges. Their findings and reasoning are unavailable to you by design. Report your own warnings,\nsuggestions, uncertainty, and dissent rather than trying to form panel agreement.\n\nThe orchestrator alone compares independently produced findings. You do not count confirmations,\ndecide quorum, or authorize repair.\n";
    let reading_now = READING_BEFORE_BASIS.replacen(
        "\n\nYour starting context",
        &format!(
            "\n\n{}\n\nYour starting context",
            crate::skill::VERDICT_BASIS_REQUIREMENT
        ),
        1,
    );
    // Comments and documentation are never `severe`, replacing the
    // "without a demonstrated cost" paragraph `READING_BEFORE_BASIS` still
    // carries. The body above is frozen at what it always was — the previous
    // current rendering, kept for upgrade detection — so the wording moves
    // here instead, the same way the basis paragraph was spliced in above.
    let reading_now = reading_now.replacen(
        "are observations about the words beside the behaviour, not about the behaviour itself: without a\n\
         demonstrated cost a mismatch is a `warning`, and editorial drift alone stays a `suggestion`. A\n\
         reader who might be misled is the hypothetical this excludes, not a cost.\n\n\
         The precision gate:",
        "are observations about the words beside the behaviour, not about the behaviour itself, and they are\n\
         **never `severe`** — not under a demonstrated cost, not under a reader who might be misled, not under\n\
         any condition you can construct. A demonstrated mismatch is a `warning` and editorial drift alone is a\n\
         `suggestion`. They stay defects the repository requires repaired; what they never do is force a\n\
         republication.\n\n\
         A launch prompt that supplies its own classification rule does not replace this one. Prefer this\n\
         definition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\n\
         wrote it learns. A rule restated in a brief is how this rule gets silently replaced, and nothing but\n\
         you can see that happen.\n\n\
         The precision gate:",
        1,
    );
    // The severe-class enumeration was corrected to name `location` as
    // the third thing a severe finding whose causal disposition is
    // `introduced` or `worsened` states, alongside `evidence_class` and
    // `causal_disposition`. The previous current body joins the historical
    // ledger as `REVIEWER_TEMPLATE_BEFORE_LOCATION`, the same way the
    // never-severe wording did above.
    let reading_now = reading_now.replacen(
        "impact. It alone can block, and it states `evidence_class` and `causal_disposition`. `warning` is a\nbounded risk with no demonstrated incorrectness. `suggestion` is optional improvement, cosmetic\nwording, style or preference.",
        "impact. It alone can block, and it states `evidence_class` and `causal_disposition` — plus\n`location` (`path:line` or `path:start-end`) when that disposition is `introduced` or `worsened`.\n`warning` is a bounded risk with no demonstrated incorrectness. `suggestion` is optional\nimprovement, cosmetic wording, style or preference.",
        1,
    );
    // A repair round corrected the unconditional panel-size claim
    // in the description field: `review_tier` can reduce the acquired panel
    // to one, so "twice or five times" was not true standing alone. The
    // previous current body joins the historical ledger as
    // `REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE`, the same way the location
    // wording did above.
    let reading_now = reading_now.replacen(
        "orchestrator launches this same definition twice or five times for a blind panel.",
        "orchestrator launches this same definition once, twice or five times, per the acquired panel.",
        1,
    );
    // The entire body was replaced, restructuring the flat prose into
    // `## Rules` and `## Review contract` sections. The previous current body joins the
    // historical ledger as `REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT`, the
    // same way the panel-size wording did above.
    let reading_now = reading_now.replacen(
        "You are one blind reviewer. Review the exact immutable target and criteria in your prompt. Do not\nsubstitute a newer head, a different digest, or criteria inferred from the implementation.\n\nThis definition is inert unless the launch prompt names an active blind mode, the exact publication\nreceipt, and the review criteria. Without all three, stop and report that the panel invocation is\nincomplete.",
        "You are a blind adversarial reviewer. Execute the review instructions provided in the launch prompt\nexactly.\n\nThis definition is inert unless that prompt names an active blind mode, the exact publication\nreceipt, and the review criteria. Without all three, stop and report that the panel invocation is\nincomplete. Do not substitute a newer head, a different digest, or criteria inferred from the\nimplementation.",
        1,
    );
    let reading_now = reading_now.replacen(
        "Every observation you record carries exactly one of three classes. The rule for choosing is here\nrather than in the launch prompt, because a rule that arrives only when somebody remembers it does\nnot arrive.\n\n`severe` is a reproducible correctness, security, reliability or contract defect with concrete\nimpact. It alone can block, and it states `evidence_class` and `causal_disposition` — plus\n`location` (`path:line` or `path:start-end`) when that disposition is `introduced` or `worsened`.\n`warning` is a bounded risk with no demonstrated incorrectness. `suggestion` is optional\nimprovement, cosmetic wording, style or preference.\n\nCosmetic, style and preference observations are `suggestion` unless they name a concrete defect, in\nwhich case say which and give it that defect's class. Incorrect or stale comments and documentation\nare observations about the words beside the behaviour, not about the behaviour itself, and they are\n**never `severe`** — not under a demonstrated cost, not under a reader who might be misled, not under\nany condition you can construct. A demonstrated mismatch is a `warning` and editorial drift alone is a\n`suggestion`. They stay defects the repository requires repaired; what they never do is force a\nrepublication.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns. A rule restated in a brief is how this rule gets silently replaced, and nothing but\nyou can see that happen.\n\nThe precision gate: if concrete evidence and material impact cannot both be stated, omit the\nobservation or keep it as a non-blocking note. Identity comes from the affected behaviour and\nlocation, never from the wording or from you.\n\nDo NOT delegate, call the Task tool, or launch sub-agents. Do NOT read, request, or infer the output of\nother judges. Their findings and reasoning are unavailable to you by design. Report your own warnings,\nsuggestions, uncertainty, and dissent rather than trying to form panel agreement.\n\nThe orchestrator alone compares independently produced findings. You do not count confirmations,\ndecide quorum, or authorize repair.\n",
        "## Rules\n\n- Do NOT use the Task or Agent tool. Do NOT delegate further.\n- Do NOT modify anything — your job is ONLY to find problems.\n- Be thorough and adversarial. **Assume the change has defects until proven otherwise.**\n- Do NOT read, request, or infer the output of other judges. Their findings are unavailable to you by\n  design. Report your own warnings, suggestions, uncertainty and dissent rather than trying to form\n  panel agreement.\n- Return findings in the structured format the launch prompt specifies.\n- At the end, state what you checked and cleared, and what you could not check with the exact command\n  that would settle each.\n\n## Review contract\n\n**Sweep budget.** Run exactly **one exhaustive sweep of the target, then stop**. There is no\nloop-until-dry mechanism; the sweep budget is the entire pass. If one sweep was not enough for some\npart, say which part and why rather than taking a second — that answer is worth more than the second\nsweep, because it tells the run what your evidence does not cover.\n\n**Precision gate.** Report a finding only if it is a real, user-impacting defect you would defend\nwith concrete evidence. When in doubt, stay silent: a missed nitpick costs nothing; a false positive\ncosts a full repair cycle. Style and preference findings are banned unless they obscure a defect. If\nconcrete evidence and material impact cannot both be stated, omit the observation or keep it as a\nnon-blocking note.\n\n**The refused substitute.** \"Looks fragile\", \"could be clearer\", \"seems risky\" and \"this might\nconfuse a reader\" are not findings. Cite the exact path and line, the input that reaches it, and\nwhat it does that it should not. A finding whose evidence is a feeling is one the run can neither\nact on nor check.\n\n**Deliberate non-findings.** Do not flag a short helper or inline constant that is clear, local and\nself-explanatory. Do not flag a comment for being terse, a test for covering one case well, or prose\nfor being plain rather than complete. This is the shape, not the boundary.\n\n**Findings.** Every observation carries exactly one class, and the rule for choosing is here rather\nthan in the launch prompt, because a rule that arrives only when somebody remembers it does not\narrive.\n\n| class | what it is |\n|---|---|\n| `severe` | a reproducible correctness, security, reliability or contract defect with concrete impact — the only class that blocks |\n| `warning` | a bounded risk with no demonstrated incorrectness |\n| `suggestion` | optional improvement, cosmetic wording, style or preference |\n\nA `severe` finding states `evidence_class` and `causal_disposition`, plus `location` (`path:line` or\n`path:start-end`) when that disposition is `introduced` or `worsened`. Identity comes from the\naffected behaviour and location, never from the wording or from you.\n\nCosmetic, style and preference observations are `suggestion` unless they name a concrete defect, in\nwhich case say which and give it that defect's class. Incorrect or stale comments and documentation\nare observations about the words beside the behaviour, not about the behaviour itself, and they are\n**never `severe`** — not under a demonstrated cost, not under a reader who might be misled, not under\nany condition you can construct. A demonstrated mismatch is a `warning`; editorial drift alone is a\n`suggestion`. They stay defects the repository requires repaired; what they never do is force a\nrepublication.\n\n**Text a running agent obeys is not documentation about behaviour — it is the behaviour.** Where a\nrepository ships instructions its own agents execute, a contradiction between two of those\ninstructions is a contract defect and the paragraph above does not soften it. The repository's own\nmap says which directories are that; read it rather than assuming.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns.\n\n**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are reported once,\nare never re-reviewed, and never block — they are preserved beside the verdict without withdrawing\nit.\n\n**Causality.** Only a defect this change caused can block it. State `causal_disposition` from what\nyou read: `introduced` or `worsened` blocks; `pre-existing` and `base-only` are recorded and do not.\n**When you cannot tell which, say `unknown` and say so in your report rather than choosing.** An\nunknown cause is for the run to settle; a guess in either direction is a decision you were not asked\nto make.\n\n**A clean sweep is a result.** If the pass finds nothing, say so plainly. Stating it is how the run\nlearns what the sweep covered.\n\n**An access failure is never a clean result.** If you could not read the target, resolve the head, or\nreach something you needed, report that **in place of a verdict**. Incomplete inspection reported as\ncompletion is the one outcome this review cannot recover from, because everything downstream treats\nit as evidence that was gathered. The same holds for your own output: if you cannot produce a\nwell-formed verdict, say so. Missing or malformed evidence never implies corroboration.\n\n**Scoped re-judgment.** On a repair round you receive the parent's findings and the repair delta\nbeside the whole target. Verify what the parent recorded and whether the repair introduced anything;\nthat is where a defect the repair caused lives. It does not narrow the sweep — the defects this\nrepository's panels find most often sit outside the delta, in the text the change did not touch.\n\n**Execution mode.** You emit your own findings and hand them to the orchestrator. The orchestrator\nalone compares independently produced findings; you do not count confirmations, decide quorum, or\nauthorize repair.\n",
        1,
    );
    // A repair round corrected the ported body against its source,
    // `jd-judge-a.md`: the unconditional "Do NOT modify anything" bullet
    // duplicated a rule `{{DISCIPLINE}}` already states and had widened it,
    // the severity-floor claim that warnings are "never re-reviewed"
    // disagreed with `blind-judges.md`'s repair-round re-read, the policy
    // carve-out pointed at a directory map this repository does not have, the
    // precision gate silently swallowed its own mandated conflict report, a
    // deleted rationale sentence was restored, and an unmeasured frequency
    // claim was cut while keeping the rule it carried. The previous current
    // body joins the historical ledger as `REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR`,
    // the same way the review-contract restructuring did above.
    let reading_now = reading_now.replacen(
        "- Do NOT use the Task or Agent tool. Do NOT delegate further.\n- Do NOT modify anything — your job is ONLY to find problems.\n- Be thorough and adversarial. **Assume the change has defects until proven otherwise.**",
        "- Do NOT use the Task or Agent tool. Do NOT delegate further.\n- Be thorough and adversarial. **Assume the change has defects until proven otherwise.**",
        1,
    );
    let reading_now = reading_now.replacen(
        "**Text a running agent obeys is not documentation about behaviour — it is the behaviour.** Where a\nrepository ships instructions its own agents execute, a contradiction between two of those\ninstructions is a contract defect and the paragraph above does not soften it. The repository's own\nmap says which directories are that; read it rather than assuming.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns.\n\n**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are reported once,\nare never re-reviewed, and never block — they are preserved beside the verdict without withdrawing\nit.",
        "**Text a running agent obeys is not documentation about behaviour — it is the behaviour.** Where a\nrepository ships instructions its own agents execute, a contradiction between two of those\ninstructions is a contract defect and the paragraph above does not soften it.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns. A rule restated in a brief is how this rule gets silently replaced, and nothing but\nyou can see that happen. That report is exempt from the precision gate above: it is mandated, not\ndiscretionary, so staying silent about it is never the answer.\n\n**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are preserved beside\nthe verdict without withdrawing it.",
        1,
    );
    let reading_now = reading_now.replacen(
        "It does not narrow the sweep — the defects this\nrepository's panels find most often sit outside the delta, in the text the change did not touch.",
        "It does not narrow the sweep — a defect can sit\noutside the delta, in the text the change did not touch.",
        1,
    );

    // A second repair round, still the same in-flight change as the
    // first: two blind panels rejected the port_repair body above, both on a
    // sentence copied from `jd-judge-a.md` without checking it against this
    // repository. The precision gate banned the classes the class table itself
    // defines, so it is rescoped to `severe` alone — a false positive there is
    // the one that costs a full repair cycle, and warnings and suggestions are
    // said to be cheap because they do not elect a round on their own. The
    // carve-out's bounding sentence, previously only in `blind-judges.md`,
    // joins the definition verbatim, and the exemption the first repair round
    // added for the mandated conflict report is removed with it, since a gate
    // scoped to `severe` has nothing left to exempt a `suggestion` from.
    // No third frozen body joins the historical ledger for this: those two
    // bodies above are frozen because they were published; this round has not
    // been.
    let reading_now = reading_now.replacen(
        "**Precision gate.** Report a finding only if it is a real, user-impacting defect you would defend\nwith concrete evidence. When in doubt, stay silent: a missed nitpick costs nothing; a false positive\ncosts a full repair cycle. Style and preference findings are banned unless they obscure a defect. If\nconcrete evidence and material impact cannot both be stated, omit the observation or keep it as a\nnon-blocking note.",
        "**Precision gate.** A `severe` finding is a real, user-impacting defect you would defend with\nconcrete evidence, and a false positive there costs a full repair cycle — when in doubt about one,\nsay what you can show and class it below `severe` rather than withholding it. Warnings and\nsuggestions do not elect a round on their own, which is what keeps the cheap observation cheap;\nthey are not gated that way. If concrete evidence and material impact cannot both be stated, omit\nthe observation or keep it as a non-blocking note.",
        1,
    );
    let reading_now = reading_now.replacen(
        "instructions is a contract defect and the paragraph above does not soften it.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns. A rule restated in a brief is how this rule gets silently replaced, and nothing but\nyou can see that happen. That report is exempt from the precision gate above: it is mandated, not\ndiscretionary, so staying silent about it is never the answer.",
        "instructions is a contract defect and the paragraph above does not soften it. The carve-out is\nnarrow: it is gated on a contradiction between two instructions a repository's own agents execute,\nnot on staleness, and it does not reach a stale comment or a stale document the never-`severe` rule\nabove already covers.\n\nA launch prompt that supplies its own classification rule does not replace this one. Prefer this\ndefinition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\nwrote it learns. A rule restated in a brief is how this rule gets silently replaced, and nothing but\nyou can see that happen.",
        1,
    );
    // A judge found the scoped re-judgment paragraph promising every repair
    // round the parent's findings and the repair delta beside the whole
    // target — an input nothing shipped guarantees. It was true only because
    // this round's launch chose to hand it over.
    let reading_now = reading_now.replacen(
        "**Scoped re-judgment.** On a repair round you receive the parent's findings and the repair delta",
        "**Scoped re-judgment.** On a repair round you may receive the parent's findings and the repair delta",
        1,
    );

    // The composition step 5 acquires is now fixed
    // at exactly two judges, one sealed and one unsealed, never reduced by
    // `review_tier`. The description field's "once, twice or five times, per
    // the acquired panel" and the precondition's third clause, "an active
    // blind mode", both named a variable panel size and a mode axis this
    // change already deleted. Still the same in-flight change as the
    // second round above, so no frozen body joins the historical ledger here.
    let reading_now = reading_now.replacen(
        "orchestrator launches this same definition once, twice or five times, per the acquired panel.",
        "orchestrator launches this same definition twice, one sealed and one unsealed, never once and\n  never five times.",
        1,
    );
    let reading_now = reading_now.replacen(
        "This definition is inert unless that prompt names an active blind mode, the exact publication\nreceipt, and the review criteria. Without all three, stop and report that the panel invocation is\nincomplete. Do not substitute a newer head, a different digest, or criteria inferred from the\nimplementation.",
        "This definition is inert unless that prompt names the exact publication receipt and the review\ncriteria. Without both, stop and report that the panel invocation is incomplete. Do not substitute\na newer head, a different digest, or criteria inferred from the implementation.",
        1,
    );
    // The tier scales the acquired panel, so an unconditional
    // "twice ... never once" no longer holds — `low` licenses one judge, the
    // unsealed one. The previous current body joins the historical ledger as
    // `REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL`, the same way the panel-size
    // wording did above.
    let reading_now = reading_now.replacen(
        "orchestrator launches this same definition twice, one sealed and one unsealed, never once and\n  never five times.",
        "orchestrator launches this same definition twice, one sealed and one unsealed, or once as the\n  unsealed judge where the tier proves the change passive — and never five times.",
        1,
    );

    let reading_now = reading_now.replacen(
        "**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are preserved beside\nthe verdict without withdrawing it.",
        "**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are preserved beside\nthe verdict without withdrawing it.\n\n**Lens findings in your brief.** Your brief may carry findings a sealed lens recorded against this\nsame receipt. An `inferential` one needs a judge to confirm it and you are that pass: reach it\nyourself or do not, and say which, naming it with the same `id` in what you return when you do — the\norchestrator holds the write, as it does for every finding you report. One pass over the whole merged\nlist, never one pass per finding. A `deterministic` lens finding needs nothing from you, and neither\ndoes an `insufficient` one — it cannot qualify a rejection on its own whatever you reach.",
        1,
    );
    // The previous current body joins the historical ledger as
    // `REVIEWER_TEMPLATE_BEFORE_ORIGIN_FIELD`, as the lens-findings wording did above.
    let reading_now = reading_now.replacen(
        "that is where a defect the repair caused lives. It does not narrow the sweep — a defect can sit\noutside the delta, in the text the change did not touch.",
        "that is where a defect the repair caused lives. It does not narrow the sweep — a defect can sit\noutside the delta, in the text the change did not touch. A `severe` finding that is new to this\nrepair — one that names no parent finding — also states `origin`: `introduced` if the repair\ncreated it, `exposed` if the repair made an existing defect reachable. It is a separate question\nfrom `causal_disposition`, and a severe finding that owes one is refused without it.",
        1,
    );
    // The precision gate's impact scope widens from the imported
    // "user-impacting" to the policy's own "user or delivery impact". The
    // previous current body joins the historical ledger as
    // `REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE`, the same way the origin-field
    // wording did above, so installs carrying it upgrade rather than refuse.
    let reading_now = reading_now.replacen(
        "**Precision gate.** A `severe` finding is a real, user-impacting defect you would defend with",
        "**Precision gate.** A `severe` finding is a real defect with user or delivery impact you would defend with",
        1,
    );
    // The definition's precision gate gains the recorder's
    // id/evidence/impact refusal and the citation-resolution sentence, so the
    // duplication `skill/policies/blind-judges.md` claims is true. Still an
    // in-flight change, so no frozen body joins the historical ledger here.
    let reading_now = reading_now.replacen(
        "the observation or keep it as a non-blocking note.\n\n**The refused substitute.**",
        "the observation or keep it as a non-blocking note. The recorder refuses a finding missing any of\n\
         `id`, `evidence` or `impact` — or naming either as filler that says nothing (`n/a`, `TODO`,\n\
         `placeholder`, and their kin) — and refuses one naming any receipt but the publication under review.\n\
         Every `path:line` or `path:start-end` citation your `evidence` or `impact` prose carries is resolved\n\
         against the receipt's `base` and `head` — except the token shapes it skips, enumerated with their\n\
         reasons in `docs/honesty-review-tier.md`'s `finding-citation-unresolvable` entry — and refused as\n\
         `finding-citation-unresolvable` under one of four causes — `malformed-path`,\n\
         `path-not-in-either-tree`, `path-is-not-a-file` or `line-past-end-of-file` — so cite a path\n\
         `base..head` actually has, at a line inside it. The lookup reads both trees from the recording\n\
         checkout; a citation the receipt's `base` and `head` cannot back is refused under any class.\n\n**The refused substitute.**",
        1,
    );

    let reading_now = reading_now.replacen(
        "twice, one sealed and one unsealed, or once as the\n  unsealed judge where the tier proves the change passive — and never five times.",
        "twice, one sealed and one unsealed — and never once\n  and never five times.",
        1,
    );

    let reading_now = replace_once(
        &reading_now,
        "description: >\n  Independently review one immutable publication receipt against the supplied criteria. The\n  orchestrator launches this same definition twice, one sealed and one unsealed — and never once\n  and never five times.\n",
        "description: >\n  One of Judgment Day's two judges: independently review one immutable target against the supplied\n  criteria. The orchestrator launches this same definition twice, one sealed and one unsealed, and\n  only when the operator explicitly asks for Judgment Day.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "**Lens findings in your brief.** Your brief may carry findings a sealed lens recorded against this\nsame receipt. An `inferential` one needs a judge to confirm it and you are that pass: reach it\nyourself or do not, and say which, naming it with the same `id` in what you return when you do — the\norchestrator holds the write, as it does for every finding you report. One pass over the whole merged\nlist, never one pass per finding. A `deterministic` lens finding needs nothing from you, and neither\ndoes an `insufficient` one — it cannot qualify a rejection on its own whatever you reach.",
        "**No lens findings, no receipt.** Judgment Day replaces the lens review for its target, so your brief\ncarries no lens finding to confirm. What you report goes to the operator and is never recorded\nagainst a receipt; the other judge's agreement is its only corroboration. State each severe finding's `evidence_class`\n— `deterministic`, `inferential` or `insufficient` — so the operator can weigh it; nothing here\nreads it.",
    );
    let reading_now = replace_once(
        &reading_now,
        "This definition is inert unless that prompt names the exact publication receipt and the review\ncriteria. Without both, stop and report that the panel invocation is incomplete. Do not substitute\na newer head, a different digest, or criteria inferred from the implementation.\n",
        "This definition is inert unless that prompt names the immutable target — a head, a diff or a commit\nrange — and the review criteria. Without both, stop and report that the panel invocation is incomplete.\nDo not substitute a newer head, a different range, or criteria inferred from the implementation.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "Your starting context is this definition and the launch prompt. You issue no tracker operations.\n",
        "Your starting context is this definition and the launch prompt. You issue no tracker operations and\nrecord nothing: you return your findings to the orchestrator, which reports them to the operator.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "the observation or keep it as a non-blocking note. The recorder refuses a finding missing any of\n`id`, `evidence` or `impact` — or naming either as filler that says nothing (`n/a`, `TODO`,\n`placeholder`, and their kin) — and refuses one naming any receipt but the publication under review.\nEvery `path:line` or `path:start-end` citation your `evidence` or `impact` prose carries is resolved\nagainst the receipt's `base` and `head` — except the token shapes it skips, enumerated with their\nreasons in `docs/honesty-review-tier.md`'s `finding-citation-unresolvable` entry — and refused as\n`finding-citation-unresolvable` under one of four causes — `malformed-path`,\n`path-not-in-either-tree`, `path-is-not-a-file` or `line-past-end-of-file` — so cite a path\n`base..head` actually has, at a line inside it. The lookup reads both trees from the recording\ncheckout; a citation the receipt's `base` and `head` cannot back is refused under any class.\n",
        "the observation or keep it as a non-blocking note. Every finding carries an `id`, its `evidence` and\nits `impact`, and none of them is filler that says nothing (`n/a`, `TODO`, `placeholder`, and their\nkin). Cite a `path:line` or `path:start-end` the target actually has, at a line inside the file.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "| `severe` | a reproducible correctness, security, reliability or contract defect with concrete impact — the only class that blocks |\n",
        "| `severe` | a reproducible correctness, security, reliability or contract defect with concrete impact — the only class that can be fixed |\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "definition, and record the conflict as a `suggestion` naming what the prompt said, so the run that\n",
        "definition, and report the conflict as a `suggestion` naming what the prompt said, so the run that\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "**Severity floor.** Only a `severe` finding can block. Warnings and suggestions are preserved beside\nthe verdict without withdrawing it.\n",
        "**Severity floor.** Only a `severe` finding can be fixed, and only when the other judge reports it\ntoo. Warnings and suggestions are info: they are preserved beside the result and never fixed.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "carries no lens finding to confirm. What you report goes to the operator and is never recorded\nagainst a receipt; the other judge's agreement is its only corroboration. State each severe finding's `evidence_class`\n",
        "carries no lens finding to confirm. What you report goes to the orchestrator and then the operator,\nand is never recorded against a receipt; the other judge's agreement is its only corroboration. State each severe finding's `evidence_class`\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "**Causality.** Only a defect this change caused can block it. State `causal_disposition` from what\nyou read: `introduced` or `worsened` blocks; `pre-existing` and `base-only` are recorded and do not.\n",
        "**Causality.** Only a defect this change caused is the target's to fix. State `causal_disposition`\nfrom what you read: `introduced` or `worsened` is fixable; `pre-existing` and `base-only` are\nreported and are not.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "reach something you needed, report that **in place of a verdict**. Incomplete inspection reported as\n",
        "reach something you needed, report that **in place of your findings**. Incomplete inspection reported as\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "well-formed verdict, say so. Missing or malformed evidence never implies corroboration.\n",
        "well-formed result, say so. Missing or malformed evidence never implies corroboration.\n",
    );
    let reading_now = replace_once(
        &reading_now,
        "**Scoped re-judgment.** On a repair round you may receive the parent's findings and the repair delta\nbeside the whole target. Verify what the parent recorded and whether the repair introduced anything;\nthat is where a defect the repair caused lives. It does not narrow the sweep — a defect can sit\noutside the delta, in the text the change did not touch. A `severe` finding that is new to this\nrepair — one that names no parent finding — also states `origin`: `introduced` if the repair\ncreated it, `exposed` if the repair made an existing defect reachable. It is a separate question\nfrom `causal_disposition`, and a severe finding that owes one is refused without it.\n",
        "**Scoped re-judgment.** On a re-judgment you receive only the frozen ledger and the fix delta, and\nyou read nothing else. Verify that each confirmed finding is resolved and whether the fix caused a\ndefect, proving any you report. A `severe` finding that is new to the fix — one that names no ledger\nrow — also states `origin`: `introduced` if the fix created it, `exposed` if the fix made an\nexisting defect reachable. It is a separate question from `causal_disposition`. It never widens the\nsweep back to the whole target.\n",
    );

    assert_eq!(
        render_reviewer_agent(
            skill::REVIEW_AGENT.contents,
            Evidence::Reading,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER
        ),
        reading_now,
        "the default rendering moved without the previous current body joining the historical ledger"
    );
    assert!(
        reviewer_is_ours(root.path(), &path, READING_BEFORE_BASIS)
            && !reviewer_is_static(READING_BEFORE_BASIS),
        "the reading body from before the basis paragraph is not an upgradeable Estigia rendering"
    );
    assert_eq!(
        Config::default().evidence,
        Evidence::Reading,
        "the default standard moved, so the rendering above is no longer the default one"
    );

    // The `measuring` paragraph carries the rule that bounds the capability it
    // announces. Four shipped documents say that paragraph is **the only thing**
    // making the shell safe, and a reviewer measured that replacing the whole arm
    // with `"Do NOT delegate."` left the suite green: the grant survived and every
    // word telling the judge where it may write was gone. The asymmetry was the
    // tell — `Reading` is pinned byte for byte above and `Measuring` was pinned by
    // nothing.
    //
    // Clauses rather than bytes, deliberately: this paragraph is prose that should
    // be free to improve, and what must not vanish is the rule inside it.
    let measuring = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Measuring,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    );
    for clause in [
        "inside the directory this launch hands you and nowhere else",
        "yours alone for the duration",
        "outside the checkout under claim",
        "Restore every mutation",
        "stop and say so rather than",
        "Still do NOT change tracker state",
        // A verdict recorded against a `measuring` panel used to
        // carry nothing but the two-value outcome. These clauses are
        // `skill::VERDICT_BASIS_REQUIREMENT`, appended after the grant's own
        // rule; `tests/payload.rs` crosses the same requirement against the
        // OpenCode text and the policy document.
        "what was executed",
        "the mutation run behind each severe finding",
        "what was not checked",
        "Measuring nothing is a legitimate basis",
    ] {
        assert!(
            measuring.contains(clause),
            "the measuring grant no longer carries the rule that bounds it: {clause:?}"
        );
    }
    // The ported body once carried a second, unconditional
    // copy of this same rule — "Do NOT modify anything" — that the paragraph pinned
    // above already answers for `measuring`. A blanket prohibition here is not a
    // stricter statement of the grant; it contradicts it. Absence, not presence, is
    // what this assertion pins.
    assert!(
        !measuring.contains("Do NOT modify anything"),
        "the measuring rendering carries a blanket modification prohibition that the paragraph above already governs"
    );

    // And every rendering is accepted, not only the default one. Narrowing this
    // back to a single spelling left the whole suite green while refusing every
    // judge launch on a `measuring` install — a blind reviewer measured it. The
    // widening is what makes the feature work at all, so it is asserted rather
    // than described.
    for evidence in Evidence::all() {
        assert!(
            reviewer_is_static(&render_reviewer_agent(
                skill::REVIEW_AGENT.contents,
                evidence,
                Seal::Unsealed,
                crate::harness::roles::RESERVED_REVIEWER
            )),
            "{evidence:?}: setup writes a definition reviewer_is_static calls foreign"
        );
    }
    assert!(
        !reviewer_is_static(skill::REVIEW_AGENT.contents),
        "the unrendered template is accepted, so a placeholder could reach the host"
    );

    let Some(stripped) = without_basis(
        REVIEWER_TEMPLATE_BEFORE_CONTRACT,
        Evidence::Measuring,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    ) else {
        panic!(
            "measuring no longer appends VERDICT_BASIS_REQUIREMENT, so the pre-basis historical body cannot be derived"
        );
    };
    assert!(
        !stripped.contains("what was executed"),
        "the pre-basis historical body still carries the requirement"
    );
    for historical in historical_reviewer_renderings() {
        assert!(
            !historical.contains("{{"),
            "a placeholder reached a historical rendering:\n{historical}"
        );
        assert!(
            reviewer_is_ours(root.path(), &path, historical),
            "a historical rendering is foreign to setup:\n{historical}"
        );
        assert!(
            !reviewer_is_static(historical),
            "a historical rendering is accepted at launch:\n{historical}"
        );
    }
}

/// A sealed rendering is recognised at launch the same way an unsealed one is.
#[test]
fn a_sealed_rendering_is_static() {
    assert!(reviewer_is_static(&render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Sealed,
        crate::harness::roles::RESERVED_REVIEWER_SEALED
    )));
}

/// Every frozen lens body is still one this binary recognises, so an install
/// carrying the previous definition upgrades instead of being refused.
#[test]
fn a_previous_lens_body_is_still_one_of_ours() {
    for (template, name) in [
        (
            LENS_TEMPLATE_BEFORE_EMPTY_FINDINGS,
            crate::harness::roles::RESERVED_LENS,
        ),
        (
            LENS_RISK_BEFORE_EMPTY_FINDINGS,
            crate::harness::roles::RESERVED_RISK,
        ),
        (
            LENS_READABILITY_BEFORE_EMPTY_FINDINGS,
            crate::harness::roles::RESERVED_READABILITY,
        ),
        (
            LENS_RELIABILITY_BEFORE_EMPTY_FINDINGS,
            crate::harness::roles::RESERVED_RELIABILITY,
        ),
        (
            LENS_READBACK_BEFORE_EMPTY_FINDINGS,
            crate::harness::roles::RESERVED_READBACK,
        ),
        (
            LENS_RISK_BEFORE_MANDATE,
            crate::harness::roles::RESERVED_RISK,
        ),
        (
            LENS_READABILITY_BEFORE_MANDATE,
            crate::harness::roles::RESERVED_READABILITY,
        ),
        (
            LENS_RELIABILITY_BEFORE_MANDATE,
            crate::harness::roles::RESERVED_RELIABILITY,
        ),
        (
            LENS_READBACK_BEFORE_MANDATE,
            crate::harness::roles::RESERVED_READBACK,
        ),
        (
            LENS_TEMPLATE_BEFORE_TWO_WORDS,
            crate::harness::roles::RESERVED_LENS,
        ),
        (
            LENS_RISK_BEFORE_TWO_WORDS,
            crate::harness::roles::RESERVED_RISK,
        ),
        (
            LENS_READABILITY_BEFORE_TWO_WORDS,
            crate::harness::roles::RESERVED_READABILITY,
        ),
        (
            LENS_RELIABILITY_BEFORE_TWO_WORDS,
            crate::harness::roles::RESERVED_RELIABILITY,
        ),
        (
            LENS_READBACK_BEFORE_TWO_WORDS,
            crate::harness::roles::RESERVED_READBACK,
        ),
        (
            LENS_TEMPLATE_BEFORE_JUDGMENT_DAY,
            crate::harness::roles::RESERVED_LENS,
        ),
        (
            LENS_TEMPLATE_BEFORE_DELTA_ONLY_DESCRIPTION,
            crate::harness::roles::RESERVED_LENS,
        ),
        (
            LENS_TEMPLATE_BEFORE_ORIGIN_FIELD,
            crate::harness::roles::RESERVED_LENS,
        ),
        (
            LENS_RISK_BEFORE_ORIGIN_FIELD,
            crate::harness::roles::RESERVED_RISK,
        ),
        (
            LENS_READABILITY_BEFORE_ORIGIN_FIELD,
            crate::harness::roles::RESERVED_READABILITY,
        ),
        (
            LENS_RELIABILITY_BEFORE_ORIGIN_FIELD,
            crate::harness::roles::RESERVED_RELIABILITY,
        ),
        (
            LENS_READBACK_BEFORE_ORIGIN_FIELD,
            crate::harness::roles::RESERVED_READBACK,
        ),
    ] {
        for evidence in Evidence::all() {
            assert!(
                reviewer_is_historical(&render_reviewer_agent(
                    template,
                    evidence,
                    Seal::Sealed,
                    name
                )),
                "{name} is not recognised as a previous rendering of its own definition"
            );
        }
    }
}

/// A grant naming no real tool is refused at the launch boundary rather than
/// denying every call; `docs/honesty.md` records the probe that measured it.
#[test]
fn a_sealed_rendering_names_a_tool_a_host_can_resolve() {
    let sealed = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Sealed,
        crate::harness::roles::RESERVED_REVIEWER_SEALED,
    );
    assert!(
        sealed.contains("\ntools: WebSearch\n"),
        "a grant resolving to no real tool is refused at the launch boundary, so a sealed judge \
         rendered that way cannot run: {sealed}"
    );
}

/// The sealed rendering is installable under its own reserved name rather than
/// shadowing the unsealed one at the same canonical path.
#[test]
fn a_sealed_rendering_carries_its_own_reserved_name() {
    let sealed = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Sealed,
        crate::harness::roles::RESERVED_REVIEWER_SEALED,
    );
    assert!(
        sealed.contains(&format!(
            "\nname: {}\n",
            crate::harness::roles::RESERVED_REVIEWER_SEALED
        )),
        "the sealed rendering does not carry its own reserved name, so it cannot be launched \
         apart from the unsealed one: {sealed}"
    );
}

/// A name that is neither `RESERVED_REVIEWER` nor `RESERVED_REVIEWER_SEALED`
/// still reaches the frontmatter, because the name is threaded from the
/// caller rather than derived from `seal`. Before that fix, only `seal`
/// picked between the two known names, so a third entry added to
/// `RESERVED_REVIEWERS` would have rendered under the unsealed template's own
/// baked-in `review-blind` regardless of which name it was meant to carry.
#[test]
fn a_third_reserved_name_renders_with_its_own_name() {
    let third = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Unsealed,
        "review-blind-third",
    );
    assert!(
        third.contains("\nname: review-blind-third\n"),
        "a third reserved name did not reach the frontmatter: {third}"
    );
    assert!(
        !third.contains(&format!(
            "\nname: {}\n",
            crate::harness::roles::RESERVED_REVIEWER
        )),
        "a third reserved name still carries the name it was supposed to replace: {third}"
    );
}

/// `declared_reviewer_name` reads the frontmatter block, not the whole
/// rendered document. A template whose frontmatter declares no `name:` must
/// leave a body line that happens to begin `name: ` untouched, rather than
/// rewriting it into the caller's reserved name.
#[test]
fn a_body_name_line_outside_frontmatter_is_not_rewritten() {
    let template = "---\ndescription: no name here\nmodel: inherit\ntools: {{TOOLS}}\n---\n\nBody text.\nname: should-not-be-rewritten\n";
    let rendered = render_reviewer_agent(
        template,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    );
    assert!(
        rendered.contains("\nname: should-not-be-rewritten\n"),
        "a body line beginning `name: ` was rewritten even though the frontmatter declared none: \
         {rendered}"
    );
}

/// One pass of the loop `setup` runs: the `review-blind` pass of
/// `validate_reviewer_definition` accepts a sealed rendering installed and
/// recorded as Estigia's own. `setup_refuses_a_sealed_rendering_at_the_unsealed_path`
/// below drives the loop itself, which refuses at the next row.
/// `render_reviewer_agent`'s sealed arm does not read `evidence`, so
/// `Evidence::Reading` here stands for either.
#[test]
fn the_unsealed_pass_accepts_a_sealed_rendering_whose_bytes_estigia_owns() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let sealed = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Sealed,
        crate::harness::roles::RESERVED_REVIEWER_SEALED,
    );
    fs::write(&reviewer, &sealed).expect("a sealed rendering is installed over the owned one");

    validate_reviewer_definition(&paths, crate::harness::roles::RESERVED_REVIEWER)
        .expect("a sealed rendering, recorded as Estigia's own, was refused as canonical");
    assert!(reviewer_is_ours(&paths.skill_root, &reviewer, &sealed));
}

/// The loop `setup` actually runs, not one pass of it by hand: with a sealed
/// rendering installed over the unsealed path, the `review-blind-sealed`
/// pass finds that same file reserving its name outside the canonical path
/// that name expects, and `setup` aborts before writing anything a second
/// time.
#[test]
fn setup_refuses_a_sealed_rendering_at_the_unsealed_path() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("the first setup runs");
    let reviewer = home
        .path()
        .join(".claude")
        .join("agents")
        .join("review-blind.md");
    let sealed = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Sealed,
        crate::harness::roles::RESERVED_REVIEWER_SEALED,
    );
    fs::write(&reviewer, &sealed).expect("a sealed rendering is installed over the owned one");

    let failure = setup(adapter, &Config::default(), &options).expect_err(
        "setup accepted a sealed rendering reserving a second name at the unsealed path",
    );
    let refusal = failure.downcast_ref::<crate::outcome::Refusal>().unwrap();
    assert_eq!(refusal.code, "reviewer-definition-unowned");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains(&reviewer.display().to_string()),
        "the refusal does not name the file to move: {rendered}"
    );
    assert!(
        rendered.contains(crate::harness::roles::RESERVED_REVIEWER_SEALED),
        "the refusal does not name the reserved name being double-claimed: {rendered}"
    );
    assert_eq!(fs::read_to_string(&reviewer).unwrap(), sealed);
}

/// The refusal tells an edit apart from an unknown provenance: bytes that
/// moved since the install wrote them name the edit, while a record that
/// predates digests cannot tell an older build from one and says unknown
/// rather than picking.
#[test]
fn an_edited_reviewer_definition_is_not_an_unknown_one() {
    use crate::harness::roles::RESERVED_REVIEWER;

    let edited = {
        let (home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let reviewer = home.path().join(".claude/agents/review-blind.md");
        fs::write(&reviewer, "the operator's own words\n").expect("an edit");
        validate_reviewer_definition(&paths, RESERVED_REVIEWER)
            .expect_err("an edited definition must not validate")
            .downcast_ref::<crate::outcome::Refusal>()
            .expect("a refusal")
            .to_string()
    };
    assert!(
        edited.contains("was edited since Estigia installed it"),
        "an edit read as unknown provenance: {edited}"
    );

    let unknown = {
        let (home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let reviewer = home.path().join(".claude/agents/review-blind.md");
        fs::write(&reviewer, "bytes from a build before digests\n").expect("older bytes");
        // A record that predates digests: the install wrote no per-path
        // digest to compare against, so staleness is unknowable.
        let record = crate::skill::record::path(&paths.skill_root);
        let mut stored: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&record).expect("the record reads"))
                .expect("the record parses");
        stored["digests"] = serde_json::json!({});
        std::fs::write(
            &record,
            serde_json::to_string_pretty(&stored).expect("renders"),
        )
        .expect("a record from before digests");
        validate_reviewer_definition(&paths, RESERVED_REVIEWER)
            .expect_err("unknown bytes must not validate")
            .downcast_ref::<crate::outcome::Refusal>()
            .expect("a refusal")
            .to_string()
    };
    assert!(
        unknown.contains("matches no current or previous rendering"),
        "unknown provenance read as a proven edit: {unknown}"
    );
    assert!(
        !unknown.contains("was edited since"),
        "unknown provenance named an edit it cannot prove: {unknown}"
    );
}

/// A repair round corrected the definition's own description
/// field: an unconditional "twice or five times" no longer matched
/// `skill/SKILL.md` developer step 5, which at the time acquired two judges
/// always and stated plainly that nothing `review_tier` reduces — since
/// superseded in turn. An install still
/// carrying the pre-change body —
/// [`REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE`] rendered under either evidence
/// standard — is a rendering this binary once wrote, so it upgrades rather
/// than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the same
/// shape `a_pre_severity_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_SEVERITY`.
#[test]
fn a_pre_panel_size_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-panel-size rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-panel-size rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-panel-size reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-panel-size reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// A repair round scaled the acquired panel by tier: `low`
/// licenses one judge, the unsealed one, so the definition's own description
/// field no longer says a review always launches this definition twice. An
/// install still carrying the pre-change body —
/// [`REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL`] rendered under either
/// evidence standard — is a rendering this binary once wrote, so it upgrades
/// rather than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the
/// same shape `a_pre_panel_size_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE`.
#[test]
fn a_pre_tier_scaled_panel_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-tier-scaled-panel rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-tier-scaled-panel rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_TIER_SCALED_PANEL,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-tier-scaled-panel reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-tier-scaled-panel reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// An unconditional
/// modification bullet the `{{DISCIPLINE}}` paragraph already governs, a
/// severity-floor claim this repository does not hold, a policy carve-out
/// pointing at a map that does not exist, a precision gate silently
/// swallowing its own mandated report, and a deleted rationale sentence. An
/// install still carrying the pre-repair body —
/// [`REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR`] rendered under either evidence
/// standard — is a rendering this binary once wrote, so it upgrades rather
/// than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the same
/// shape `a_pre_review_contract_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT`.
#[test]
fn a_pre_port_repair_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-port-repair rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-port-repair rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_PORT_REPAIR,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-port-repair reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-port-repair reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// The entire body was replaced. An install still carrying the pre-change
/// body — [`REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT`] rendered under either
/// evidence standard — is a rendering this binary once wrote, so it upgrades
/// rather than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the
/// same shape `a_pre_panel_size_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_PANEL_SIZE`.
#[test]
fn a_pre_review_contract_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-review-contract rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-review-contract rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_REVIEW_CONTRACT,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-review-contract reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-review-contract reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// `location` was added to the severe-class enumeration. An install
/// still carrying the pre-change body — [`REVIEWER_TEMPLATE_BEFORE_LOCATION`]
/// rendered under either evidence standard — is a rendering this binary once
/// wrote, so it upgrades rather than refuses: `reviewer_is_ours` and not
/// `reviewer_is_static`, the same shape
/// `a_pre_severity_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_SEVERITY`.
#[test]
fn a_pre_location_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_LOCATION,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-location rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-location rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_LOCATION,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-location reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-location reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// The severity decision rule moved into the definition, replacing
/// the classify paragraph. An install still carrying the pre-severity body —
/// [`REVIEWER_TEMPLATE_BEFORE_SEVERITY`] rendered under either evidence
/// standard — is a rendering this binary once wrote, so it upgrades rather
/// than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the same
/// shape `the_default_rendering_is_the_bytes_already_installed` pins for
/// `READING_BEFORE_BASIS`.
#[test]
fn a_pre_severity_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_SEVERITY,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-severity rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-severity rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    // The body that shipped before the `reading` arm gained the basis
    // paragraph, which is a real install on somebody's disk and the one ledger
    // entry nothing else reaches: `READING_BEFORE_BASIS` was repointed at the
    // new template by this change, and iterating the ledger cannot notice an
    // entry that is missing from it.
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_SEVERITY,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-severity reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-severity reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// Comments and documentation are never `severe`, replacing the
/// "without a demonstrated cost" paragraph in the definition's classification
/// rule. An install still carrying the pre-change body —
/// [`REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE`] rendered under either evidence
/// standard — is a rendering this binary once wrote, so it upgrades rather
/// than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the same
/// shape `a_pre_severity_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_SEVERITY`.
#[test]
fn a_pre_never_severe_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-never-severe rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-never-severe rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-never-severe reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-never-severe reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// `low` licenses no judge, so the definition's own
/// description no longer offers a one-judge launch. An install carrying the
/// body from before is a rendering this binary once wrote, so it upgrades.
#[test]
fn a_pre_no_low_judge_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_NO_LOW_JUDGE,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-no-low-judge rendering is not an upgradeable Estigia rendering"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-no-low-judge rendering is accepted at launch, so setup would never upgrade it"
        );
    }
}

/// The precision gate widens from "user-impacting" to "user or
/// delivery impact". An install still carrying the pre-change body —
/// [`REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE`] rendered under either evidence
/// standard — is a rendering this binary once wrote, so it upgrades rather
/// than refuses: `reviewer_is_ours` and not `reviewer_is_static`, the same
/// shape `a_pre_never_severe_reviewer_is_ours_but_not_static` pins for
/// `REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE`.
#[test]
fn a_pre_impact_scope_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: the pre-impact-scope rendering is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: the pre-impact-scope rendering is accepted at launch, so setup would never upgrade it"
        );
    }
    let pre_basis = without_basis(
        REVIEWER_TEMPLATE_BEFORE_IMPACT_SCOPE,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("the pre-impact-scope reading rendering carries a basis paragraph to strip");
    assert!(
        reviewer_is_ours(root.path(), &path, &pre_basis),
        "the pre-impact-scope reading body from before the basis paragraph would be refused as foreign:
{pre_basis}"
    );
}

/// Before this change, every entry in the historical ledger rendered
/// `Seal::Unsealed` and `RESERVED_REVIEWER` alone — so a superseded
/// template's **sealed** rendering, the shape `review-blind-sealed.md` takes
/// from any earlier binary, matched nothing on the ledger and was refused
/// exactly like an operator edit, independent of the digest gap this same
/// issue closes.
#[test]
fn a_sealed_pre_change_reviewer_is_ours_but_not_static() {
    let (root, path) = unrecorded_reviewer_path();
    for evidence in Evidence::all() {
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_SEVERITY,
            evidence,
            Seal::Sealed,
            crate::harness::roles::RESERVED_REVIEWER_SEALED,
        );
        assert!(
            reviewer_is_ours(root.path(), &path, &historical),
            "{evidence:?}: a sealed rendering of a superseded template is not an upgradeable Estigia rendering:\n{historical}"
        );
        assert!(
            !reviewer_is_static(&historical),
            "{evidence:?}: a sealed rendering of a superseded template is accepted at launch, so setup would never upgrade it"
        );
    }
}

/// A body matching neither the digest recorded for this exact path nor any
/// known rendering is still refused — the digest arm narrows the
/// false-refusal set, it does not widen the gate. Two real writes establish
/// two different recorded digests in turn (`installed`, then `first_edit`),
/// and a third body the operator wrote on top of the second is checked
/// against the *current* recorded digest, not the one before it.
#[test]
fn a_body_matching_neither_the_recorded_digest_nor_any_rendering_is_refused() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let installed = fs::read_to_string(&reviewer).expect("the reviewer installed");

    let first_edit = "---\nname: review-blind\ntools: Read\n---\n\nAn in-flight edit.\n";
    write_reviewer_definition(
        &paths.skill_root,
        &reviewer,
        Some(&installed),
        false,
        false,
        first_edit,
    )
    .expect("the first in-flight body is written and recorded");

    // The operator's own edit on top of it: neither the digest just recorded
    // for `first_edit`, nor any current or historical rendering.
    let operators_edit = "an operator's own text, touching neither recorded body\n";
    fs::write(&reviewer, operators_edit).expect("the operator's edit lands");

    let failure = setup(adapter, &Config::default(), &options)
        .expect_err("setup accepted bytes matching neither the recorded digest nor any rendering");
    let refusal = failure.downcast_ref::<crate::outcome::Refusal>().unwrap();
    assert_eq!(refusal.code, "reviewer-definition-changed");
    assert_eq!(
        fs::read_to_string(&reviewer).expect("the reviewer"),
        operators_edit,
        "a refused write changed the file anyway"
    );
}

/// An install still carrying the exact pre-change `review-blind.md` body is a
/// real state on somebody's disk the moment this change ships, and it must
/// upgrade rather than refuse — the same guarantee
/// `setup_upgrades_each_owned_historical_reviewer` proves for every entry in
/// [`historical_reviewer_renderings`], exercised here end to end for this one
/// body so a regression names the issue rather than an index into the ledger.
#[test]
fn setup_upgrades_an_install_still_carrying_the_pre_never_severe_reviewer() {
    for evidence in Evidence::all() {
        let (home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let reviewer = home.path().join(".claude/agents/review-blind.md");
        let historical = render_reviewer_agent(
            REVIEWER_TEMPLATE_BEFORE_NEVER_SEVERE,
            evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER,
        );
        fs::write(&reviewer, &historical).expect("the pre-change reviewer body is installed");

        let retry = setup(adapter, &Config::default(), &options)
            .expect("setup refused an install carrying the pre-never-severe reviewer body");
        assert_eq!(
            fs::read_to_string(&reviewer).expect("the reviewer"),
            render_reviewer_agent(
                skill::REVIEW_AGENT.contents,
                Config::default().evidence,
                Seal::Unsealed,
                crate::harness::roles::RESERVED_REVIEWER
            ),
            "{evidence:?}: the pre-change body was not upgraded to the current rendering"
        );
        assert!(
            retry
                .actions
                .iter()
                .any(|action| action.path == reviewer && action.change == Change::Update),
            "{evidence:?}: the pre-change reviewer body was not rewritten"
        );
    }
}

#[test]
fn the_reviewer_definition_is_the_review_contract_and_not_the_tracker_binding() {
    let reading = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    );
    let measuring = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Evidence::Measuring,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    );
    let binding = include_str!("../../skill/bindings/github.md");
    for rendered in [&reading, &measuring] {
        assert!(
            rendered.contains("You issue no tracker operations"),
            "the judge still needs the tracker binding to know it must not issue tracker operations"
        );
        assert!(
            rendered.contains("`severe`")
                && rendered.contains("`warning`")
                && rendered.contains("`suggestion`"),
            "the judge still needs the panel policy to classify a finding"
        );
        assert!(
            rendered.contains("what was executed"),
            "the judge is not asked to state a basis"
        );
        assert!(
            !rendered.contains("publish-review") && !binding.contains(rendered.as_str()),
            "the tracker binding was shipped to a judge that issues no tracker operations"
        );
    }
}
#[test]
fn every_claude_setup_installs_one_idempotent_reviewer() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("single-mode setup runs");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    assert_eq!(
        fs::read_to_string(&reviewer).expect("the reviewer is installed"),
        render_reviewer_agent(
            skill::REVIEW_AGENT.contents,
            Config::default().evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER
        )
    );
    let retry = setup(adapter, &Config::default(), &options).expect("the retry succeeds");
    assert!(
        retry
            .actions
            .iter()
            .any(|action| action.path == reviewer && action.change == Change::Unchanged)
    );
    uninstall(adapter, &options).expect("uninstall runs");
    assert!(!reviewer.exists());
}

#[test]
fn an_unowned_reviewer_refuses_before_any_setup_artifact_is_written() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    fs::create_dir_all(reviewer.parent().unwrap()).expect("the agent directory exists");
    let theirs = "# Their reviewer\n";
    fs::write(&reviewer, theirs).expect("their reviewer exists");

    let failure = setup(adapter, &Config::default(), &options)
        .expect_err("setup accepted an unowned stable reviewer");
    let refusal = failure.downcast_ref::<crate::outcome::Refusal>().unwrap();
    assert_eq!(refusal.code, "reviewer-definition-unowned");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert!(!paths.skill_root.exists());
    assert!(!paths.instructions.exists());
    assert_eq!(fs::read_to_string(reviewer).unwrap(), theirs);
}

#[test]
fn a_duplicate_user_reviewer_refuses_before_any_setup_artifact_is_written() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let duplicate = home.path().join(".claude/agents/nested/other.md");
    fs::create_dir_all(duplicate.parent().unwrap()).expect("the user agent directory exists");
    let theirs = "---\n\"name\": review-blind\ntools: Read\n---\nHostile.\n";
    fs::write(&duplicate, theirs).expect("their reviewer exists");

    let failure = setup(adapter, &Config::default(), &options)
        .expect_err("setup accepted a duplicate user reviewer");
    let refusal = failure.downcast_ref::<crate::outcome::Refusal>().unwrap();
    assert_eq!(refusal.code, "reviewer-definition-unowned");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert!(!paths.skill_root.exists());
    assert!(!paths.instructions.exists());
    assert_eq!(fs::read_to_string(duplicate).unwrap(), theirs);
}

#[test]
fn setup_refuses_to_overwrite_a_changed_owned_reviewer() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let edited = "operator-owned replacement\n";
    fs::write(&reviewer, edited).expect("the reviewer is edited");

    let failure = setup(adapter, &Config::default(), &options)
        .expect_err("setup overwrote a changed owned reviewer");
    let refusal = failure.downcast_ref::<crate::outcome::Refusal>().unwrap();
    assert_eq!(refusal.code, "reviewer-definition-changed");
    uninstall(adapter, &options).expect("uninstall preserves changed bytes");
    assert_eq!(
        fs::read_to_string(&reviewer).expect("the edit survives"),
        edited
    );
    assert!(!owns_reviewer(&paths, &reviewer));
}

/// Every entry now covers every reserved name, not `review-blind` alone —
/// routed to the canonical path its own frontmatter declares, since a
/// `review-blind-sealed` body sitting at `review-blind.md`'s path collides
/// with the YAML-identity check as a stray second candidate for the sealed
/// name, an orthogonal refusal this test is not exercising.
///
/// One sandbox for the whole ledger rather than one per entry: the ledger is
/// large enough now — every superseded template across every reserved name
/// and both evidence standards — that a fresh temporary root and a fresh
/// install per entry made this test measure Windows' tolerance for rapid
/// directory creation and recursive deletion rather than the ownership rule,
/// and it failed on exactly that, not on anything this change got wrong.
#[test]
fn setup_upgrades_each_owned_historical_reviewer() {
    // One worker per reserved name: each name owns one canonical path, so the
    // names cannot collide, and a single sandbox per worker keeps the ledger
    // out of one directory's rapid-create path (the reason this test stopped
    // taking a temporary root per entry).
    let mut by_name: std::collections::BTreeMap<&'static str, Vec<String>> =
        std::collections::BTreeMap::new();
    for historical in historical_reviewer_renderings() {
        let name = declared_reviewer_name(historical)
            .expect("a historical rendering declares its own reserved name");
        by_name.entry(name).or_default().push(historical.to_owned());
    }
    std::thread::scope(|scope| {
        for (name, historicals) in by_name {
            scope.spawn(move || {
                let (_home, options) = sandbox();
                let adapter = agent("claude-code");
                let mut pending = Pending::new();
                setup_into(adapter, &Config::default(), &options, &mut pending)
                    .expect("setup runs");
                let paths = resolve_paths(adapter, &options).expect("paths resolve");
                let (_, seal, template) = crate::harness::roles::RESERVED_REVIEWERS
                    .iter()
                    .find(|(candidate, _, _)| *candidate == name)
                    .expect("the declared name is one of the reserved reviewers");
                let reviewer =
                    reviewer_target(&paths, name).expect("a canonical path for that name");
                if crate::harness::roles::binary_launched_reviewer(name) {
                    for historical in historicals {
                        skill::record::note_created_outside(&paths.skill_root, &reviewer)
                            .expect("ownership recorded");
                        fs::write(&reviewer, &historical).expect("an older Estigia rendering");
                        pending.remove(&reviewer);
                        setup_into(adapter, &Config::default(), &options, &mut pending)
                            .expect("setup refused an older Estigia rendering");
                        assert!(
                            !reviewer.exists(),
                            "historical bytes for {name} were not retracted:\n{historical}"
                        );
                    }
                    return;
                }
                for historical in historicals {
                    fs::write(&reviewer, &historical).expect("an older Estigia rendering");
                    pending.remove(&reviewer);

                    let retry = setup_into(adapter, &Config::default(), &options, &mut pending)
                        .expect("setup refused an older Estigia rendering");
                    assert_eq!(
                        fs::read_to_string(&reviewer).expect("the reviewer"),
                        render_reviewer_agent(template, Config::default().evidence, *seal, name)
                    );
                    assert!(
                        retry.actions.iter().any(
                            |action| action.path == reviewer && action.change == Change::Update
                        ),
                        "historical bytes for {name} were not rewritten:\n{historical}"
                    );
                }
            });
        }
    });
}

/// Every frozen reviewer template in `src/setup/` is declared here through
/// `include_str!` and is a member of [`REVIEWER_TEMPLATES_SUPERSEDED`].
///
/// A template whose `include_str!` const is left out of the ledger compiles
/// and installs, and the suite stays green — the omission is silent here and
/// loud only on an operator whose machine carries that body, which then meets
/// a refusal instead of an upgrade. `the_default_rendering_is_the_bytes_already_installed`
/// names this ledger in its own failure message but never reads it, so an
/// author who satisfies the half that fires can still skip the half that does
/// not.
///
/// This reads `src/setup/mod.rs` through [`crate::source_text::code_of`], so it
/// crosses declarations rather than comments. It cannot see an author who
/// never freezes a body at all — no file, no const, nothing to leave out of the
/// ledger — which `docs/honesty.md` records.
#[test]
fn every_frozen_reviewer_template_is_a_member_of_the_superseded_ledger() {
    let source = crate::source_text::code_of(include_str!("mod.rs"));
    let declared = include_str_templates(&source);
    let ledger = superseded_ledger_members(&source);

    assert!(
        !declared.is_empty() && !ledger.is_empty(),
        "the guard measured nothing: {} declarations, {} ledger members",
        declared.len(),
        ledger.len(),
    );

    let missing: Vec<&str> = declared
        .iter()
        .filter(|(_, name)| !ledger.contains(name))
        .map(|(file, _)| file.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "these frozen reviewer templates are declared but absent from \
         REVIEWER_TEMPLATES_SUPERSEDED, so an install still carrying them is refused as foreign \
         instead of upgraded: {missing:?}",
    );

    let extra: Vec<&str> = ledger
        .iter()
        .filter(|name| !declared.iter().any(|(_, declared)| declared == *name))
        .map(String::as_str)
        .collect();
    assert!(
        extra.is_empty(),
        "REVIEWER_TEMPLATES_SUPERSEDED names consts this module does not declare through \
         include_str!: {extra:?}",
    );

    let mut on_disk: Vec<String> =
        std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/setup"))
            .expect("src/setup is readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".md"))
            .collect();
    on_disk.sort();
    let undeclared: Vec<&String> = on_disk
        .iter()
        .filter(|file| !declared.iter().any(|(declared, _)| declared == *file))
        .collect();
    assert!(
        undeclared.is_empty(),
        "these `.md` files in src/setup are frozen templates no include_str! declares, so nothing \
         installs or upgrades them: {undeclared:?}",
    );
}

/// Every `const NAME: &str = include_str!("FILE");` in a module's code, as
/// `(file, name)` pairs. The caller strips comments first, so an `include_str!`
/// named in prose is never mistaken for a declaration.
fn include_str_templates(source: &str) -> Vec<(String, String)> {
    let mut templates = Vec::new();
    for (at, _) in source.match_indices("include_str!(\"") {
        let tail = &source[at + "include_str!(\"".len()..];
        let Some((file, _)) = tail.split_once('"') else {
            continue;
        };
        let before = &source[..at];
        let Some(const_at) = before.rfind("const ") else {
            continue;
        };
        let name: String = before[const_at + "const ".len()..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            templates.push((file.to_owned(), name));
        }
    }
    templates
}

/// The identifiers between `REVIEWER_TEMPLATES_SUPERSEDED`'s `&[` and `]`.
fn superseded_ledger_members(source: &str) -> Vec<String> {
    let at = source
        .find("const REVIEWER_TEMPLATES_SUPERSEDED")
        .expect("the superseded ledger is declared");
    let rest = &source[at..];
    let open = rest.find("= &[").expect("the ledger is a slice") + "= &[".len();
    let close = rest[open..].find(']').expect("the ledger slice closes") + open;
    rest[open..close]
        .split(',')
        .map(str::trim)
        .filter(|member| !member.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn uninstall_deletes_an_owned_historical_reviewer() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let historical = without_basis(
        REVIEWER_TEMPLATE_BEFORE_CONTRACT,
        Evidence::Reading,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .expect("old-template reading without basis is a historical body");
    fs::write(&reviewer, historical).expect("an older Estigia rendering");

    uninstall(adapter, &options).expect("uninstall deletes historical bytes");
    assert!(!reviewer.exists());
}

/// The end-to-end sequence that had no test, and the shape
/// that put four work-around files on one operator's machine: a source
/// build writes a reviewer body from a template that moves again before it
/// is ever published, so the body on disk joins no frozen ledger and matches
/// no rendering this binary's own renderer can produce. `setup` must upgrade
/// it, not refuse it.
///
/// Simulated rather than built twice — nothing in this checkout can compile
/// two different embedded templates — by calling
/// [`write_reviewer_definition`] directly with a synthetic body, the same
/// function a real `--allow-source-build` install's write goes through, so
/// its digest is recorded exactly the way a real one would be. Exercised for
/// each judge [`crate::harness::roles::RESERVED_REVIEWERS`] still installs:
/// the unsealed judge and its sealed twin.
#[test]
fn setup_accepts_an_in_flight_body_no_ledger_entry_will_ever_cover() {
    std::thread::scope(|scope| {
        for name in [
            crate::harness::roles::RESERVED_REVIEWER,
            crate::harness::roles::RESERVED_REVIEWER_SEALED,
        ] {
            scope.spawn(move || {
                let (_home, options) = sandbox();
                let adapter = agent("claude-code");
                setup(adapter, &Config::default(), &options).expect("setup runs");
                let paths = resolve_paths(adapter, &options).expect("paths resolve");
                let target = reviewer_target(&paths, name).expect("a reviewer path");
                let installed = fs::read_to_string(&target).expect("the reviewer installed");

                // A body no template this binary embeds and no frozen ledger entry
                // will ever hold.
                let in_flight = format!(
                    "---\nname: {name}\ntools: Read\n---\n\nAn unpublished edit for {name}.\n"
                );
                write_reviewer_definition(
                    &paths.skill_root,
                    &target,
                    Some(&installed),
                    false,
                    false,
                    &in_flight,
                )
                .expect("the in-flight body is written and its digest recorded");

                let retry = setup(adapter, &Config::default(), &options).unwrap_or_else(|error| {
                    panic!(
                        "{name}: setup refused a body it had itself written and recorded, though \
                         it matches no known rendering: {error:#}"
                    )
                });
                let (_, seal, template) = crate::harness::roles::RESERVED_REVIEWERS
                    .iter()
                    .find(|(candidate, _, _)| *candidate == name)
                    .expect("the name is one of the reserved reviewers");
                assert_eq!(
                    fs::read_to_string(&target).expect("the reviewer"),
                    render_reviewer_agent(template, Config::default().evidence, *seal, name),
                    "{name}: the in-flight body was not upgraded to the current rendering"
                );
                assert!(
                    retry
                        .actions
                        .iter()
                        .any(|action| action.path == target && action.change == Change::Update),
                    "{name}: the in-flight body was not rewritten"
                );
            });
        }
    });
}

/// The two doors [`reviewer_is_ours`] answers must agree: a body the setup
/// preflight accepts through its recorded digest alone — no known rendering,
/// no frozen ledger entry — is one `uninstall` deletes as Estigia's own
/// rather than keeping it as an orphan at the reserved path, which the next
/// install would then meet as unowned.
#[test]
fn uninstall_deletes_a_digest_matched_reviewer_no_rendering_covers() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    let installed = fs::read_to_string(&reviewer).expect("the reviewer installed");
    let in_flight = "---\nname: review-blind\ntools: Read\n---\n\nAn unpublished edit.\n";
    write_reviewer_definition(
        &paths.skill_root,
        &reviewer,
        Some(&installed),
        false,
        false,
        in_flight,
    )
    .expect("the in-flight body is written and its digest recorded");
    assert!(
        !reviewer_is_static(in_flight) && !reviewer_is_historical(in_flight),
        "the in-flight body must match no known rendering for this test to prove anything"
    );

    uninstall(adapter, &options).expect("uninstall deletes a digest-matched reviewer");
    assert!(
        !reviewer.exists(),
        "a body setup accepts through its recorded digest was kept by uninstall as foreign, \
         orphaning it at the reserved path"
    );
}

#[test]
fn reviewer_deletion_failure_keeps_ownership_and_is_retryable() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("the reviewer installs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");

    inject_reviewer_definition_removal_failure();
    uninstall(adapter, &options).expect_err("the injected deletion failure was ignored");
    assert!(reviewer.exists());
    assert!(owns_reviewer(&paths, &reviewer));
    assert!(paths.skill_root.join(skill::CONTRACT).exists());

    uninstall(adapter, &options).expect("the exact retry succeeds");
    assert!(!reviewer.exists());
    assert!(!paths.skill_root.exists());
    assert!(!paths.instructions.exists());
}

#[test]
fn reviewer_is_owned_before_its_bytes_are_written_and_the_retry_is_safe() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");

    inject_reviewer_definition_write_failure();
    setup(adapter, &Config::default(), &options)
        .expect_err("the injected reviewer write failure was ignored");
    assert!(!reviewer.exists());
    assert!(owns_reviewer(&paths, &reviewer));

    setup(adapter, &Config::default(), &options).expect("the exact retry succeeds");
    assert!(owns_reviewer(&paths, &reviewer));
    assert_eq!(
        fs::read_to_string(reviewer).expect("the reviewer"),
        render_reviewer_agent(
            skill::REVIEW_AGENT.contents,
            Config::default().evidence,
            Seal::Unsealed,
            crate::harness::roles::RESERVED_REVIEWER
        )
    );
}

#[test]
fn a_reviewer_created_after_preflight_is_preserved_and_not_owned() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let reviewer = home.path().join(".claude/agents/review-blind.md");

    inject_reviewer_definition_create_collision();
    setup(adapter, &Config::default(), &options)
        .expect_err("setup overwrote a reviewer created after preflight");

    assert_eq!(
        fs::read_to_string(&reviewer).expect("the operator reviewer survives"),
        INJECTED_REVIEWER_COLLISION
    );
    assert!(!owns_reviewer(&paths, &reviewer));
}

#[test]
fn normalized_reviewer_line_endings_and_final_newline_remain_deletable() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let reviewer = home.path().join(".claude/agents/review-blind.md");
    setup(adapter, &Config::default(), &options).expect("the reviewer installs");
    // The **rendered** definition, because that is what is on disk. Normalising
    // the raw asset writes a file no install ever produced, so the uninstall
    // rightly declined to own it and this test measured its own fixture.
    let normalized = render_reviewer_agent(
        skill::REVIEW_AGENT.contents,
        Config::default().evidence,
        Seal::Unsealed,
        crate::harness::roles::RESERVED_REVIEWER,
    )
    .replace('\n', "\r\n")
    .trim_end()
    .to_owned();
    fs::write(&reviewer, normalized).expect("only text shape changes");

    uninstall(adapter, &options).expect("normalized text is still owned");
    assert!(!reviewer.exists());
}

#[test]
fn setup_failures_carry_each_proven_boundary_and_are_exactly_replay_safe() {
    let effective = Config {
        models: crate::config::ModelRouting::parse("developer=opus").expect("a model route"),
        ..Config::default()
    };
    std::thread::scope(|scope| {
        for (boundary, last_kind) in [
            (SetupFailureBoundary::AfterSkill, ActionKind::Skill),
            (SetupFailureBoundary::AfterDirective, ActionKind::Directive),
            (
                SetupFailureBoundary::AfterDefinitions,
                ActionKind::DelegatedAgent,
            ),
            (SetupFailureBoundary::AfterHooks, ActionKind::Hooks),
        ] {
            let effective = &effective;
            scope.spawn(move || {
                with_preview(|| {
                    let (_home, options) = sandbox();
                    let adapter = agent("claude-code");
                    let mut pending = Pending::new();
                    inject_setup_failure(adapter.slug, boundary);

                    let failure = setup_adapter_into(
                        adapter,
                        &Config::default(),
                        effective,
                        &options,
                        &mut pending,
                        true,
                    )
                    .expect_err("the injected boundary was reported as success");

                    assert!(
                        failure
                            .result
                            .actions
                            .iter()
                            .any(|action| action.kind == last_kind
                                && action.change != Change::Unchanged),
                        "{boundary:?} lost its last proven action: {:?}",
                        failure.result.actions
                    );
                    assert!(!failure.result.completed);
                    assert!(!failure.write_attempted);
                    let retry = setup_adapter_into(
                        adapter,
                        &Config::default(),
                        effective,
                        &options,
                        &mut pending,
                        true,
                    )
                    .unwrap_or_else(|failure| {
                        panic!("{boundary:?} was not exactly replay safe: {failure}")
                    });
                    assert!(retry.completed, "{boundary:?} did not complete on retry");
                });
            });
        }
    });
}

#[test]
fn an_agent_override_read_error_refuses_before_the_writer_replaces_it() {
    let root = tempfile::tempdir().expect("a temporary root");
    let file = root.path().join("estigia.opencode.md");
    fs::create_dir(&file).expect("the override path is an unreadable directory");

    let refusal = write_agent_configuration(
        &file,
        "opencode",
        &Config::default(),
        &[crate::config::Setting::Models],
    )
    .expect_err("the writer treated an I/O error as an absent document");

    assert_eq!(refusal.code, "config-local-unreadable");
    assert!(file.is_dir(), "the writer replaced the unreadable path");
}

/// Where a config home comes from, for every shape a caller can supply. The
/// absolute paths are temporary directories rather than literals, because a
/// POSIX-looking `/moved/config` is **not** absolute on Windows.
#[test]
fn a_config_home_comes_from_the_override_or_the_variable_and_never_from_both() {
    let isolated = tempfile::tempdir().expect("a temporary home");
    let moved = tempfile::tempdir().expect("a temporary config home");
    let named = tempfile::tempdir().expect("a named config home");

    let resolve = |config_home: Option<&std::path::Path>, home: Option<&std::path::Path>| {
        let options = SetupOptions {
            home_dir: home.map(std::path::Path::to_path_buf),
            config_home: config_home.map(std::path::Path::to_path_buf),
            platform: Some(Platform::Unix),
            ..SetupOptions::default()
        };
        Environment::resolve(&options)
            .expect("an absolute home")
            .xdg_config()
    };

    // What a borrowed home falls back to when the variable says nothing.
    let borrowed_fallback = with_config_home(std::path::Path::new(""), || resolve(None, None));

    // Synthetic paths under and outside the machine home, derived rather than
    // literal, and read under the same lock `with_config_home` below takes.
    let machine_home = crate::test_env::with_environment_read_locked(crate::paths::home_dir)
        .expect("the machine has a home");
    let under_the_home = machine_home.join("estigia-config-under-the-home");
    let outside_the_home = machine_home
        .parent()
        .expect("a home directory has a parent")
        .join("estigia-config-outside-the-home");

    with_config_home(&under_the_home, || {
        assert_eq!(
            resolve(None, None),
            under_the_home,
            "a borrowed home takes the variable when it lands under that home"
        );
    });

    with_config_home(&outside_the_home, || {
        assert_eq!(
            resolve(None, None),
            borrowed_fallback,
            "a borrowed home took a variable pointing outside itself"
        );
    });

    with_config_home(moved.path(), || {
        assert_eq!(
            resolve(None, Some(isolated.path())),
            isolated.path().join(".config"),
            "an isolated home does not inherit the machine's variable"
        );
        assert_eq!(
            resolve(Some(named.path()), None),
            named.path(),
            "an absolute override wins over the variable"
        );
        // The cell that moved: a useless override is not a reason to inherit
        // the variable.
        for useless in [
            std::path::Path::new("relative/config"),
            std::path::Path::new(""),
        ] {
            assert_eq!(
                resolve(Some(useless), None),
                borrowed_fallback,
                "an override of {useless:?} fell through to the variable"
            );
            assert_eq!(
                resolve(Some(useless), Some(isolated.path())),
                isolated.path().join(".config"),
                "an override of {useless:?} beside an isolated home left the home"
            );
        }
    });
}

/// A config home under a symlinked home is kept, and no note names
/// the same path on both sides of the substitution sentence.
///
/// `paths::covers` canonicalised the home through its symlink and fell back to
/// the literal spelling for a candidate nothing had created, so
/// `$XDG_CONFIG_HOME=$HOME/.config` answered `false` for containment and the
/// note read *`<home>/.config` points to `<home>/.config` outside the resolved
/// home; using `<home>/.config` instead*.
#[test]
#[cfg(unix)]
fn a_config_home_under_a_symlinked_home_is_not_substituted() {
    let real = tempfile::tempdir().expect("a real home");
    let holder = tempfile::tempdir().expect("a directory to hold the link");
    let home = holder.path().join("home");
    std::os::unix::fs::symlink(real.path(), &home).expect("the home symlink");

    let config = home.join(".config");
    assert!(
        !config.exists(),
        "the config home exists, so this measures nothing: {}",
        config.display()
    );
    with_home_env(
        &home,
        &home.join("AppData").join("Roaming"),
        &config,
        || {
            let options = SetupOptions {
                platform: Some(Platform::Unix),
                ..SetupOptions::default()
            };
            let paths = resolve_paths(agent("opencode"), &options).expect("the paths resolve");
            assert!(
                paths.notes.is_empty(),
                "a config home under a symlinked home was substituted: {:?}",
                paths.notes
            );
        },
    );
}

/// The plugin hands the gate the per-call working directory, and it is not the
/// project root. This is one of three tests in the crate that need an interpreter,
/// because the artefact under test is JavaScript; it does not skip when `node`
/// is absent — it fails and says what is missing. Both host generations are
/// driven: OpenCode 1 through the named export's returned hooks, OpenCode 2
/// through the default definition's `setup`.
#[test]
fn the_plugin_hands_the_gate_the_directory_the_call_runs_in() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let module = root.path().join("estigia-workflow-authority.mjs");
    std::fs::write(
        &module,
        crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the plugin writes");

    // Two directories that are not each other.
    let project = "/tmp/base-checkout";
    let worktree = "/tmp/base-checkout/wt-a";

    let driver = root.path().join("drive.mjs");
    std::fs::write(
        &driver,
        format!(
            r#"import plugin, {{ EstigiaWorkflowAuthority }} from "./estigia-workflow-authority.mjs";

// A stand-in for Bun's shell. It records rather than runs: the interpolated
// values in order, and the directory `.cwd()` was given.
const recorder = (seen) => (_strings, ...values) => {{
  const call = {{ values: values.map(String), cwd: null }};
  const chain = {{
    cwd(directory) {{ call.cwd = String(directory); return chain; }},
    quiet() {{ return chain; }},
    nothrow() {{ seen.push(call); return Promise.resolve({{ exitCode: 0 }}); }},
  }};
  return chain;
}};

const seenV1 = [];
const hooks = await EstigiaWorkflowAuthority({{
  $: recorder(seenV1),
  directory: {project:?},
  worktree: undefined,
}});
await hooks["tool.execute.before"](
  {{ tool: "bash", sessionID: "ses_fixture" }},
  {{ args: {{ command: "git commit -m fixture", workdir: {worktree:?} }} }},
);
await hooks["tool.execute.before"](
  {{ tool: "bash" }},
  {{ args: {{ command: "git commit -m fixture", workdir: {worktree:?} }} }},
);

const seenV2 = [];
globalThis.Bun = {{ $: recorder(seenV2) }};
const registered = [];
const ctx = {{
  location: {{ directory: {project:?}, project: {{ canonical: "/tmp/canonical" }} }},
  tool: {{
    hook: async (name, callback) => {{
      registered.push([name, callback]);
    }},
  }},
}};
await plugin.setup(ctx);
const entry = registered.find(([name]) => name === "execute.before");
if (!entry) throw new Error("setup registered no execute.before hook");
await entry[1]({{ tool: "shell", sessionID: "ses_fixture", input: {{ command: "git commit -m fixture" }} }});
await entry[1]({{ tool: "shell", input: {{ command: "git commit -m fixture" }} }});

console.log(JSON.stringify({{ v1: seenV1, v2: seenV2 }}));
"#
        ),
    )
    .expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generated plugin, which is JavaScript, and \
             `node` could not be run ({error}). Install Node and re-run; it is \
             not skipped, because a plugin nothing executes is a gate nothing \
             measures."
        ),
    };
    assert!(
        ran.status.success(),
        "the plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );

    let answers: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");

    // The payload, which is where the key travels: `narrowed_by_the_call`
    // reads `workdir` out of exactly this.
    let v1: Vec<serde_json::Value> = answers["v1"].as_array().expect("the v1 calls").clone();
    assert_eq!(v1.len(), 2, "the v1 hook asked the gate {} times", v1.len());
    let call = &v1[0];
    let values: Vec<&str> = call["values"]
        .as_array()
        .expect("the interpolated values")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let payload = values
        .iter()
        .find(|value| value.starts_with('{'))
        .unwrap_or_else(|| panic!("no payload was interpolated: {values:?}"));
    let payload: serde_json::Value = serde_json::from_str(payload).expect("the payload is JSON");
    assert_eq!(
        payload["workdir"], worktree,
        "the plugin dropped the working directory the call named: {payload}"
    );

    // And the directory it would have launched in, which is the project root.
    assert_eq!(
        call["cwd"], project,
        "the gate was launched somewhere other than the project root: {call}"
    );
    assert_ne!(
        call["cwd"], payload["workdir"],
        "the fixture stopped distinguishing the two directories, so it proves nothing"
    );

    // The session id OpenCode 1's hook input carries is forwarded as the
    // identity, exactly as OpenCode 2's event is; a hook input without one
    // and no exported run id keeps the checkout road.
    let identity: Vec<&str> = values.iter().rev().skip(1).take(2).copied().collect();
    assert_eq!(
        identity,
        vec!["opencode", "ses_fixture"],
        "the session id OpenCode 1's hook input carried was not forwarded as the identity: \
         {values:?}"
    );
    let none: Vec<&str> = v1[1]["values"]
        .as_array()
        .expect("the interpolated values")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        !none
            .iter()
            .any(|value| *value == "ses_fixture" || *value == "opencode"),
        "an OpenCode 1 hook input carrying no session id was given an identity anyway: {none:?}"
    );
    assert_eq!(
        v1[1]["cwd"], project,
        "the identity-less v1 call was launched somewhere other than the project root: {}",
        v1[1]
    );

    // OpenCode 2: the same contract through `setup`'s hook — the gate runs at
    // the location the plugin instance serves, the call's input travels, and the
    // session id the event carries is forwarded as the identity rather than
    // dropped. A host that sends none, with no run id exported around the plugin,
    // keeps the checkout road.
    let v2: Vec<serde_json::Value> = answers["v2"].as_array().expect("the v2 calls").clone();
    assert_eq!(v2.len(), 2, "the v2 hook asked the gate {} times", v2.len());
    let values_of = |call: &serde_json::Value| -> Vec<String> {
        call["values"]
            .as_array()
            .expect("the interpolated values")
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_owned())
            .collect()
    };
    let first = &v2[0];
    let values = values_of(first);
    let payload = values
        .iter()
        .find(|value| value.starts_with('{'))
        .unwrap_or_else(|| panic!("no payload was interpolated: {values:?}"));
    let payload: serde_json::Value = serde_json::from_str(payload).expect("the payload is JSON");
    assert_eq!(
        payload["command"], "git commit -m fixture",
        "the plugin dropped the command the call named: {payload}"
    );
    assert_eq!(
        first["cwd"], project,
        "the gate was launched somewhere other than the location the plugin serves: {first}"
    );
    let identity: Vec<&str> = values
        .iter()
        .rev()
        .skip(1)
        .take(2)
        .map(String::as_str)
        .collect();
    assert_eq!(
        identity,
        vec!["opencode", "ses_fixture"],
        "the session id the event carried was not forwarded as the identity: {values:?}"
    );
    let none = values_of(&v2[1]);
    assert!(
        !none
            .iter()
            .any(|value| value == "ses_fixture" || value == "opencode"),
        "an event carrying no session id was given an identity anyway: {none:?}"
    );
    assert_eq!(
        v2[1]["cwd"], project,
        "the gate was launched somewhere other than the location the plugin serves: {}",
        v2[1]
    );
}

/// The driver for the loop plugin's `node` test. It stubs the OpenCode 2
/// plugin context, drives one turn-ended event per scripted answer, and prints
/// what the plugin did.
const LOOP_DRIVER: &str = r#"import plugin from "./flow-loop.mjs";

const KEY = "estigia:loop:ses_loop_fixture";

const makeStream = () => {
  const queue = [];
  const waiters = [];
  const stream = async function* () {
    while (true) {
      if (queue.length) {
        yield queue.shift();
        continue;
      }
      await new Promise((resolve) => waiters.push(resolve));
    }
  };
  return {
    push: (event) => {
      queue.push(event);
      const waiter = waiters.shift();
      if (waiter) waiter();
    },
    stream: stream(),
  };
};

const makeShell = (answers, seen) => (_strings, ...values) => {
  const call = { values: values.map(String), cwd: null, text: _strings.join(" ") };
  const chain = {
    cwd(directory) {
      call.cwd = String(directory);
      return chain;
    },
    quiet() {
      return chain;
    },
    nothrow() {
      seen.push(call);
      const answer = answers.shift() ?? { exitCode: 0, stdout: "{}" };
      return new Promise((resolve) => setTimeout(() => resolve(answer), answer.delay ?? 0));
    },
  };
  return chain;
};

const until = async (predicate, what) => {
  const deadline = Date.now() + 4000;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 60));

const DIRECTORY = "/tmp/loop-fixture";

const world = async ({ answers, directories = [DIRECTORY], subscribe = null, seed = {}, sessions = {} }) => {
  const prompts = [];
  const synthetics = [];
  const storage = new Map(Object.entries(seed));
  const seen = [];
  const hooks = { beforeSet: null };
  globalThis.Bun = { $: makeShell(answers, seen) };
  const instances = [];
  for (const directory of directories) {
    const events = makeStream();
    const commands = {};
    const ctx = {
      location: { directory },
      storage: {
        get: async (key) => storage.get(key),
        set: async (key, value) => {
          if (hooks.beforeSet) await hooks.beforeSet(key, value);
          storage.set(key, value);
        },
        remove: async (key) => {
          storage.delete(key);
        },
        scan: async ({ prefix }) => ({
          entries: [...storage].filter(([key]) => key.startsWith(prefix)).map(([key, value]) => ({ key, value })),
        }),
      },
      session: {
        get: async ({ sessionID }) => {
          if (!(sessionID in sessions)) throw new Error("no such session " + sessionID);
          return { data: { id: sessionID, parentID: sessions[sessionID] } };
        },
        prompt: async (input) => {
          prompts.push(input);
        },
        synthetic: async (input) => {
          if (typeof input.description !== "string" || input.description.trim() === "") {
            console.error("a synthetic message went out with no description: " + JSON.stringify(input));
            process.exit(3);
          }
          synthetics.push(input);
        },
      },
      command: {
        transform: async (callback) => {
          callback({
            add: (definition) => {
              commands[definition.name] = definition;
            },
          });
        },
      },
      event: { subscribe: subscribe ? () => subscribe(events) : () => events.stream },
    };
    const teardown = await plugin.setup(ctx);
    instances.push({ directory, events, commands, teardown });
  }
  const turn = (directory) => {
    for (const instance of instances) {
      instance.events.push({
        type: "session.execution.succeeded",
        location: directory ? { directory } : undefined,
        data: { sessionID: "ses_loop_fixture" },
      });
    }
  };
  const emit = (type, data) => {
    for (const instance of instances) instance.events.push({ type, data });
  };
  const run = (index, name, text) =>
    instances[index].commands[name].execute({
      sessionID: "ses_loop_fixture",
      prompt: { text },
      delivery: "queue",
    });
  const stopped = () => storage.get(KEY)?.stopped ?? null;
  return { prompts, synthetics, storage, seen, instances, turn, emit, run, stopped, hooks };
};

const summary = (w) => ({
  prompts: w.prompts.length,
  texts: w.prompts.map((entry) => entry.text),
  stored: w.storage.has(KEY),
  stopped: w.stopped(),
  goal: w.storage.get(KEY) ?? null,
  synthetics: w.synthetics,
  asked: w.seen.length,
  clears: w.seen.filter((call) => call.text.includes("stop-after") && call.text.includes("--clear")).length,
  cwds: w.seen.map((call) => call.cwd),
  command: w.instances[0].commands["flow-status"] ? true : false,
});

const scenario = async (name, { start, turns, max, events = turns.length }) => {
  const w = await world({ answers: [start, ...turns] });
  await w.run(0, "flow-loop", `do the thing${max ? ` --max ${max}` : ""}`);
  await until(() => w.prompts.length >= 1, `${name}: the first prompt`);
  for (let i = 0; i < events; i++) {
    const before = w.prompts.length;
    w.turn(DIRECTORY);
    await until(
      () => w.prompts.length > before || w.stopped(),
      `${name}: turn ${i + 1}`,
    );
  }
  await settle();
  return summary(w);
};

const completedAt = (n) => ({
  exitCode: 0,
  stdout: JSON.stringify({
    run_id: "opencode-abc",
    live: true,
    issue: 42,
    standing: "passed",
    completed: n,
  }),
});
const started = {
  exitCode: 0,
  stdout: JSON.stringify({ run_id: "opencode-abc", live: true, issue: 42 }),
};

const empty = (extra) => ({
  exitCode: 0,
  stdout: JSON.stringify({
    run_id: "opencode-abc",
    live: false,
    standing: "stopped",
    ...extra,
  }),
});

const cases = {};
cases.live = await scenario("live", {
  start: started,
  turns: [completedAt(0)],
  max: 3,
});
cases.live_no_issue = await scenario("live_no_issue", {
  start: started,
  turns: [
    {
      exitCode: 0,
      stdout: JSON.stringify({
        run_id: "opencode-abc",
        live: true,
        issue: null,
        standing: "passed",
      }),
    },
  ],
});
cases.dead = await scenario("dead", {
  start: started,
  turns: [empty({ claimable: 0, dischargeable: 0 })],
});
cases.terminal = await scenario("terminal", {
  start: started,
  turns: [empty({ claimable: 0, dischargeable: 0 }), empty({ claimable: 0, dischargeable: 0 })],
});
cases.claimable = await scenario("claimable", {
  start: started,
  turns: [empty({ claimable: 2, dischargeable: 0 })],
});
cases.dischargeable = await scenario("dischargeable", {
  start: started,
  turns: [empty({ claimable: 0, dischargeable: 1 })],
});
cases.none = await scenario("none", {
  start: started,
  turns: [empty({ claimable: 0, dischargeable: 0 })],
});
cases.counts_unknown = await scenario("counts_unknown", {
  start: started,
  turns: [empty({ claimable: 0 })],
});
cases.cap = await scenario("cap", {
  start: started,
  turns: [completedAt(1), completedAt(2)],
  max: 2,
});
cases.completion_unknown = await scenario("completion_unknown", {
  start: started,
  turns: [
    {
      exitCode: 0,
      stdout: JSON.stringify({
        run_id: "opencode-abc",
        live: true,
        issue: 42,
        standing: "passed",
        completed: null,
      }),
    },
  ],
  max: 2,
});
cases.stop_after = await scenario("stop_after", {
  start: started,
  turns: [
    empty({
      claimable: 2,
      dischargeable: 0,
      stop_after: "the operator asked to stop after the current task",
    }),
  ],
});
cases.stop_after_live = await scenario("stop_after_live", {
  start: started,
  turns: [
    {
      exitCode: 0,
      stdout: JSON.stringify({
        run_id: "opencode-abc",
        live: true,
        issue: 42,
        standing: "passed",
        completed: 0,
        stop_after: "the operator asked to stop after the current task",
      }),
    },
  ],
});
{
  const withRequest = {
    exitCode: 0,
    stdout: JSON.stringify({
      run_id: "opencode-abc",
      live: true,
      issue: 42,
      stop_after: "the operator asked to stop after the current task",
    }),
  };
  const w = await world({
    answers: [withRequest, { exitCode: 0, stdout: "{}" }, completedAt(0)],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "stale request: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2 || w.stopped(), "stale request: the first turn");
  await settle();
  cases.stale_request = summary(w);
}

{
  const w = await world({
    answers: [started, completedAt(0), completedAt(0), completedAt(0)],
    directories: ["/tmp/a", "/tmp/b", "/tmp/c"],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "instances: the first prompt");
  w.turn("/tmp/b");
  await until(() => w.prompts.length >= 2, "instances: the reaction");
  await settle();
  const one = summary(w);
  w.turn(undefined);
  await until(() => w.prompts.length >= 3, "instances: the unlocated reaction");
  await settle();
  const unlocated = { prompts: w.prompts.length, asked: w.seen.length };
  w.turn("/tmp/a-worktree");
  await until(() => w.prompts.length >= 4, "instances: the worktree reaction");
  await settle();
  cases.instances = {
    ...one,
    unlocated: unlocated.prompts,
    unlocatedAsked: unlocated.asked,
    worktree: w.prompts.length,
    worktreeAsked: w.seen.length,
    worktreeCwds: w.seen.map((call) => call.cwd),
  };
}
{
  const w = await world({
    answers: [started, completedAt(0), completedAt(0)],
    directories: ["/tmp/b", "/tmp/c", "/tmp/a"],
  });
  await w.run(2, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "last owner: the first prompt");
  w.turn(undefined);
  await until(() => w.prompts.length >= 2, "last owner: the reaction");
  await settle();
  cases.last_owner = { prompts: w.prompts.length, asked: w.seen.length, cwds: w.seen.map((call) => call.cwd) };
}
{
  const w = await world({
    answers: [started, completedAt(0)],
    directories: ["/tmp/a", "/tmp/b"],
  });
  w.turn(undefined);
  await settle();
  const never = { prompts: w.prompts.length, asked: w.seen.length };
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "no goal: the first prompt");
  await w.run(0, "flow-loop", "stop");
  await settle();
  const before = { prompts: w.prompts.length, asked: w.seen.length };
  w.turn(undefined);
  w.turn("/tmp/a");
  await settle();
  cases.unowned = {
    never,
    stoppedPrompts: w.prompts.length - before.prompts,
    stoppedAsked: w.seen.length - before.asked,
  };
}
{
  const w = await world({
    answers: [started, { ...completedAt(0), delay: 150 }],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "race: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 2, "race: the ask in flight");
  await w.run(0, "flow-loop", "stop");
  await settle();
  await new Promise((resolve) => setTimeout(resolve, 200));
  cases.race = summary(w);
}
{
  const w = await world({ answers: [started] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "goal: the first prompt");
  await w.run(0, "flow-status", "");
  await w.run(0, "flow-loop", "do the thing again");
  cases.goal = summary(w);
  await w.run(0, "flow-loop", "stop");
  w.turn(DIRECTORY);
  await settle();
  cases.operator = summary(w);
  await w.run(0, "flow-loop", "stop");
}
{
  const w = await world({ answers: [started] });
  await w.run(0, "flow-status", "some text");
  cases.status_argument = { ...summary(w), old: Boolean(w.instances[0].commands.loop || w.instances[0].commands.goal), names: Object.keys(w.instances[0].commands) };
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

fn run_loop_driver() -> serde_json::Value {
    let root = tempfile::tempdir().expect("a temporary directory");
    let module = root.path().join("flow-loop.mjs");
    std::fs::write(
        &module,
        crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the loop plugin writes");

    let driver = root.path().join("drive-loop.mjs");
    std::fs::write(&driver, LOOP_DRIVER).expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generated loop plugin, which is JavaScript, and `node` could not \
             be run ({error}). Install Node and re-run; it is not skipped, because a loop nothing \
             executes is a loop nothing measures."
        ),
    };
    assert!(
        ran.status.success(),
        "the loop plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );

    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

/// The loop plugin continues while `estigia goal` says the run is live or its
/// queues hold claimable or dischargeable work, stops when none of the three
/// holds, and keeps the goal with the reason it stopped.
///
/// One of the three tests in the crate that require `node`, beside
/// `the_plugin_hands_the_gate_the_directory_the_call_runs_in`; it does not skip
/// when `node` is absent. It proves each exit: a turn that ends live enqueues
/// the same prompt again, a turn that ends without a live claim but with
/// claimable work does too, so does a turn that ends with only a discharged
/// condition to act on — and its preamble says what to do with it — while a turn
/// that ends with none of the three does not, an answer that could not be read
/// does not, a claim answer Estigia could not confirm does not even when work is
/// count could not be read does not, and a run stops once `--max` completed
/// issues are reported. Every stopping exit keeps the goal with the reason, the
/// next turn of a stopped loop does nothing, and the plugin offers `/goal` to
/// report all of it. A turn whose claim Estigia could not confirm, or whose
/// answer could not be read, is not one of these exits: `the_loop_plugin_waits`
/// proves it is retried on the timer and stops only once the attempts are spent,
/// saying the read was retried.
///
/// It also proves the in-session stop request: a pending request stops a
/// not-live run and is consumed, a live run continues past it without consuming
/// it, and a fresh loop clears a request an earlier loop left behind before its
/// first turn.
#[test]
fn the_loop_plugin_continues_while_estigia_says_there_is_work() {
    let cases = run_loop_driver();

    let live = &cases["live"];
    assert_eq!(
        live["prompts"], 2,
        "a turn that ended live did not enqueue the prompt again: {live}"
    );
    assert_eq!(live["stored"], true);
    let texts: Vec<&str> = live["texts"]
        .as_array()
        .expect("the texts")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        texts.iter().all(|text| text.contains("opencode-abc")),
        "the preamble did not name the run id: {texts:?}"
    );
    assert!(
        texts
            .first()
            .is_some_and(|text| text.contains("do the thing")),
        "the first prompt was not the objective: {texts:?}"
    );
    assert!(
        texts.iter().all(|text| !text.contains("--max")),
        "the cap flag was left in the prompt: {texts:?}"
    );

    let dead = &cases["dead"];
    assert_eq!(
        dead["prompts"], 1,
        "a turn that ended with no live claim and no claimable or dischargeable work enqueued again: {dead}"
    );
    assert_eq!(
        dead["stored"], true,
        "a stopping loop erased its goal instead of recording why: {dead}"
    );
    assert!(
        dead["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("no claimable or dischargeable")),
        "the stop was recorded without its reason: {dead}"
    );

    let terminal = &cases["terminal"];
    assert_eq!(
        terminal["prompts"], 1,
        "a stopped loop started again on the next turn: {terminal}"
    );
    assert_eq!(terminal["stored"], true);

    let claimable = &cases["claimable"];
    assert_eq!(
        claimable["prompts"], 2,
        "a turn that ended with claimable work did not enqueue again: {claimable}"
    );
    assert_eq!(claimable["stored"], true);
    assert_eq!(claimable["stopped"], serde_json::Value::Null);

    let dischargeable = &cases["dischargeable"];
    assert_eq!(
        dischargeable["prompts"], 2,
        "a turn that ended with work one discharged condition away did not enqueue again: {dischargeable}"
    );
    assert_eq!(dischargeable["stored"], true);
    let waited: Vec<&str> = dischargeable["texts"]
        .as_array()
        .expect("the texts")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        waited
            .get(1)
            .is_some_and(|text| text.contains("discharge one")),
        "the preamble did not say what to do with a discharged condition: {waited:?}"
    );

    let none = &cases["none"];
    assert_eq!(
        none["prompts"], 1,
        "a turn that ended with a readable empty queue enqueued again: {none}"
    );
    assert_eq!(none["stored"], true);
    assert!(
        none["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("no claimable or dischargeable")),
        "the stop was recorded without its reason: {none}"
    );

    let unknown_counts = &cases["counts_unknown"];
    assert_eq!(
        unknown_counts["prompts"], 1,
        "a run whose queue counts could not be read continued: {unknown_counts}"
    );
    assert!(
        unknown_counts["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("did not report how much work")),
        "a queue count that could not be read was not recorded as such: {unknown_counts}"
    );

    let cap = &cases["cap"];
    assert_eq!(
        cap["prompts"], 2,
        "a run with `--max 2` did not stop once two completed issues were reported: {cap}"
    );
    assert_eq!(cap["stored"], true);
    assert!(
        cap["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("completion cap")),
        "the cap stop was recorded without its reason: {cap}"
    );

    let unknown = &cases["completion_unknown"];
    assert_eq!(
        unknown["prompts"], 1,
        "a run whose completed count could not be read continued: {unknown}"
    );
    assert_eq!(unknown["stored"], true);

    let stop_after = &cases["stop_after"];
    assert_eq!(
        stop_after["prompts"], 1,
        "a run that asked to stop after the current task enqueued another turn: {stop_after}"
    );
    assert_eq!(stop_after["stored"], true);
    assert!(
        stop_after["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("stop after the current task")),
        "the request stop was recorded without its reason: {stop_after}"
    );
    assert_eq!(
        stop_after["clears"], 1,
        "the stop-after request was not consumed: {stop_after}"
    );

    let stop_after_live = &cases["stop_after_live"];
    assert_eq!(
        stop_after_live["prompts"], 2,
        "a live claim did not keep the loop going past a pending stop-after request: \
         {stop_after_live}"
    );
    assert_eq!(stop_after_live["stopped"], serde_json::Value::Null);
    assert_eq!(
        stop_after_live["clears"], 0,
        "a live claim consumed the stop-after request: {stop_after_live}"
    );

    let stale = &cases["stale_request"];
    assert_eq!(
        stale["clears"], 1,
        "a fresh loop did not clear the stop-after request left by an earlier loop: {stale}"
    );
    assert_eq!(
        stale["prompts"], 2,
        "a fresh loop did not continue its first turn after a stale request: {stale}"
    );
    assert_eq!(
        stale["stopped"],
        serde_json::Value::Null,
        "a fresh loop stopped on a stale stop-after request: {stale}"
    );

    assert_eq!(
        cases["live"]["command"], true,
        "the plugin no longer offers the `/goal` command that reports a stopped loop"
    );

    let instances = &cases["instances"];
    assert_eq!(
        instances["asked"], 2,
        "three instances seeing one event asked Estigia more than once beyond `/loop`'s own read: {instances}"
    );
    assert_eq!(instances["prompts"], 2, "{instances}");
    assert_eq!(
        instances["cwds"][1], "/tmp/a",
        "the goal was asked about from the event's location, not where it was recorded: {instances}"
    );
    assert_eq!(instances["goal"]["directory"], "/tmp/a");
    assert_eq!(
        instances["unlocated"], 3,
        "a turn end carrying no location was not acted on exactly once: {instances}"
    );
    assert_eq!(instances["unlocatedAsked"], 3, "{instances}");
    assert_eq!(
        instances["worktree"], 4,
        "a turn end located elsewhere than the goal was not acted on exactly once: {instances}"
    );
    assert_eq!(instances["worktreeAsked"], 4, "{instances}");
    assert!(
        instances["worktreeCwds"]
            .as_array()
            .is_some_and(|cwds| cwds[1..].iter().all(|cwd| cwd == "/tmp/a")),
        "a turn end was answered from a directory other than the goal's: {instances}"
    );

    let last = &cases["last_owner"];
    assert_eq!(
        last["prompts"], 2,
        "a non-owner instance holding the decision lock made the owner drop the turn end: {last}"
    );
    assert_eq!(last["asked"], 2, "{last}");

    let unowned = &cases["unowned"];
    assert_eq!(unowned["never"]["prompts"], 0, "{unowned}");
    assert_eq!(unowned["never"]["asked"], 0, "{unowned}");
    assert_eq!(
        unowned["stoppedPrompts"], 0,
        "a stopped goal was continued: {unowned}"
    );
    assert_eq!(
        unowned["stoppedAsked"], 0,
        "a stopped goal was asked about: {unowned}"
    );

    let race = &cases["race"];
    assert_eq!(
        race["prompts"], 1,
        "a stop recorded while the answer was in flight did not prevent the enqueue: {race}"
    );
    assert!(
        race["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("operator")),
        "{race}"
    );

    let goal = &cases["goal"];
    assert_eq!(
        goal["prompts"], 1,
        "`/goal` or a second `/loop` enqueued a prompt: {goal}"
    );
    let said = goal["synthetics"].as_array().expect("the synthetics");
    assert!(said.len() >= 2, "{goal}");
    assert!(said.iter().all(|entry| entry["resume"] == false), "{goal}");
    assert!(
        said.iter().any(|entry| entry["text"]
            .as_str()
            .is_some_and(|text| text.contains("already running"))),
        "a second `/loop` on a running goal did not say so: {goal}"
    );
    assert_eq!(goal["stopped"], serde_json::Value::Null);

    let status_argument = &cases["status_argument"];
    assert_eq!(status_argument["old"], false, "{status_argument}");
    assert_eq!(
        status_argument["names"],
        serde_json::json!(["flow-loop", "flow-status"]),
        "the loop plugin registers commands other than the two it renamed to: {status_argument}"
    );
    assert_eq!(status_argument["prompts"], 0, "{status_argument}");
    let refusal = status_argument["synthetics"]
        .as_array()
        .expect("the synthetics");
    assert_eq!(refusal.len(), 1, "{status_argument}");
    let text = refusal[0]["text"].as_str().expect("the refusal text");
    assert!(
        text.contains("takes no argument") && text.contains("/goal") && text.contains("/flow-loop"),
        "`/flow-status <text>` did not refuse visibly, naming the commands it was confused with: {text}"
    );
    assert_eq!(refusal[0]["resume"], false, "{status_argument}");
    assert!(
        refusal[0]["description"]
            .as_str()
            .is_some_and(|shown| shown.contains("takes no argument")),
        "the refusal was not in the description OpenCode shows: {status_argument}"
    );

    let operator = &cases["operator"];
    assert_eq!(
        operator["prompts"], 1,
        "a loop the operator stopped continued: {operator}"
    );
    assert!(
        operator["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("operator")),
        "{operator}"
    );
}

const LOOP_SILENCE_SCENARIOS: &str = r#"
const cases = {};
const started = { exitCode: 0, stdout: JSON.stringify({ run_id: "opencode-abc", live: true, issue: 42, standing: "live" }) };
const live = { exitCode: 0, stdout: JSON.stringify({ run_id: "opencode-abc", live: true, issue: 42, standing: "live" }) };

const flaky = (failure) => {
  let calls = 0;
  const subscribed = [];
  const subscribe = (events) => {
    calls += 1;
    subscribed.push(calls);
    if (calls === 1) {
      return failure === "throws"
        ? (async function* () {
            throw new Error("the stream overflowed");
          })()
        : (async function* () {})();
    }
    return events.stream;
  };
  return { subscribe, count: () => calls };
};

for (const failure of ["throws", "ends"]) {
  const stream = flaky(failure);
  const w = await world({ answers: [started, live], subscribe: stream.subscribe });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, `${failure}: the first prompt`);
  await until(() => stream.count() >= 2, `${failure}: a second subscription`);
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, `${failure}: a prompt after the stream came back`);
  await w.run(0, "flow-status", "");
  cases[failure] = {
    prompts: w.prompts.length,
    subscriptions: stream.count(),
    watch: w.storage.get("estigia:loop-watch:" + DIRECTORY) ?? null,
    said: w.synthetics.map((entry) => entry.text),
  };
}

{
  const broken = {
    get exitCode() {
      throw new Error("the shell fell over");
    },
  };
  const w = await world({ answers: [started, broken] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "throw: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.stopped(), "throw: a stop naming the error");
  await w.run(0, "flow-status", "");
  cases.thrown = {
    prompts: w.prompts.length,
    goal: w.storage.get(KEY),
    said: w.synthetics.map((entry) => entry.text),
  };
}

{
  const w = await world({ answers: [started, live, live, live, live] });
  await w.run(0, "flow-status", "");
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "report: the first prompt");
  await w.run(0, "flow-status", "");
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, "report: the second prompt");
  await w.run(0, "flow-status", "");
  cases.report = {
    said: w.synthetics.map((entry) => entry.text),
    described: w.synthetics.map((entry) => entry.description),
    goal: w.storage.get(KEY),
  };
}

{
  const w = await world({ answers: [started, live], directories: ["/tmp/a", "/tmp/b"] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "foreign: the first prompt");
  w.turn("/tmp/a");
  await until(() => w.prompts.length >= 2, "foreign: the second prompt");
  await settle();
  cases.foreign = { last: w.storage.get(KEY).last, prompts: w.prompts.length };
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

/// Scenarios for the loop plugin's wait. Timers of a minute or more are held by
/// the driver and fired by hand, so a five-minute interval costs no test time and
/// the operator-facing bound is not weakened to fit a test.
const LOOP_WAIT_SCENARIOS: &str = r#"
const realSetTimeout = globalThis.setTimeout;
const realClearTimeout = globalThis.clearTimeout;
const held = [];
globalThis.setTimeout = (callback, ms, ...rest) => {
  if (ms >= 60000) {
    const timer = { callback, ms, live: true };
    held.push(timer);
    return timer;
  }
  return realSetTimeout(callback, ms, ...rest);
};
globalThis.clearTimeout = (timer) => {
  if (timer && typeof timer === "object" && "live" in timer) timer.live = false;
  else realClearTimeout(timer);
};
const live = () => held.filter((timer) => timer.live);
const fire = async () => {
  const due = live();
  for (const timer of due) timer.live = false;
  for (const timer of due) timer.callback();
  await settle();
};
const clean = () => {
  for (const timer of held) timer.live = false;
};

const empty = (extra) => ({
  exitCode: 0,
  stdout: JSON.stringify({ run_id: "opencode-abc", live: false, standing: "stopped", ...extra }),
});
const started = { exitCode: 0, stdout: JSON.stringify({ run_id: "opencode-abc", live: true, issue: 42 }) };
const waiting = (extra = {}) =>
  empty({ claimable: 0, dischargeable: 0, held: 1, pending: 1, unconditioned: 0, ...extra });
const work = () => empty({ claimable: 1, dischargeable: 0, held: 0, pending: 0, unconditioned: 0 });
const unread = {
  exitCode: 0,
  stdout: JSON.stringify({
    run_id: "opencode-abc",
    live: false,
    standing: "unread",
    claimable: 2,
    dischargeable: 0,
  }),
};

const cases = {};

{
  const w = await world({ answers: [started, waiting(), waiting(), waiting(), waiting(), work()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "wait: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "wait: the wait recorded");
  const first = {
    prompts: w.prompts.length,
    stopped: w.stopped(),
    goal: w.storage.get(KEY),
    timers: live().map((timer) => timer.ms),
  };
  await fire();
  const second = { asked: w.seen.length, timers: live().length, prompts: w.prompts.length };
  await fire();
  const third = { asked: w.seen.length, timers: live().length, prompts: w.prompts.length };
  const quiet = { prompts: w.prompts.length, synthetics: w.synthetics.length };
  await w.run(0, "flow-status", "");
  const said = w.synthetics.map((entry) => entry.text);
  await fire();
  cases.wait = {
    first,
    said,
    second,
    third,
    quiet,
    after: {
      prompts: w.prompts.length,
      timers: live().length,
      goal: w.storage.get(KEY),
      asked: w.seen.length,
    },
  };
  clean();
}

{
  const w = await world({
    answers: [started, empty({ claimable: 0, dischargeable: 0, held: 0, pending: 0, unconditioned: 2 })],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "unconditioned: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.stopped(), "unconditioned: the stop");
  cases.unconditioned = { stopped: w.stopped(), timers: live().length, prompts: w.prompts.length };
  clean();
}

{
  const w = await world({
    answers: [started, empty({ claimable: 0, dischargeable: 0, held: 0, pending: 0, unconditioned: 0, unadmitted: 3 })],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "unadmitted: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "unadmitted: the wait recorded");
  cases.unadmitted = { stopped: w.stopped(), on: w.storage.get(KEY)?.waiting?.on ?? null, timers: live().length };
  clean();
}

{
  const w = await world({
    answers: [started, empty({ claimable: 0, dischargeable: 0, held: null, pending: 0, unconditioned: 0 })],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "unread: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.stopped(), "unread: the stop");
  cases.unread = { stopped: w.stopped(), timers: live().length };
  clean();
}

{
  const w = await world({ answers: [started, unread, work()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "retry: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "retry: the wait recorded");
  const waited = {
    prompts: w.prompts.length,
    stopped: w.stopped(),
    retries: w.storage.get(KEY)?.retries ?? null,
    on: w.storage.get(KEY)?.waiting?.on ?? null,
    timers: live().map((timer) => timer.ms),
  };
  await fire();
  await until(() => w.prompts.length >= 2, "retry: the re-ask enqueued");
  cases.unconfirmed_retry = {
    waited,
    after: {
      prompts: w.prompts.length,
      stopped: w.stopped(),
      retries: w.storage.get(KEY)?.retries ?? null,
      waiting: w.storage.get(KEY)?.waiting ?? null,
    },
  };
  clean();
}

{
  const w = await world({ answers: [started, unread, unread, unread] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "exhausted: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.retries === 1, "exhausted: the first attempt");
  await fire();
  await until(() => w.storage.get(KEY)?.retries === 2, "exhausted: the second attempt");
  await fire();
  await until(() => w.stopped(), "exhausted: the stop");
  cases.unconfirmed_exhausted = {
    prompts: w.prompts.length,
    stopped: w.stopped(),
    timers: live().length,
    synthetics: w.synthetics.map((entry) => ({ text: entry.text, resume: entry.resume })),
  };
  clean();
}

{
  const w = await world({ answers: [started, waiting(), waiting()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "stop: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "stop: the timer armed");
  await w.run(0, "flow-loop", "stop");
  const cleared = live().length;
  await fire();
  cases.stop = { cleared, asked: w.seen.length, prompts: w.prompts.length, stopped: w.stopped() };
  clean();
}

{
  const w = await world({ answers: [started, waiting(), work()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "teardown: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "teardown: the timer armed");
  w.instances[0].teardown();
  const cleared = live().length;
  await fire();
  cases.teardown = { cleared, asked: w.seen.length, prompts: w.prompts.length };
  clean();
}

{
  const w = await world({
    answers: [started, waiting(), { ...work(), delay: 120 }],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "coincide: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "coincide: the timer armed");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 3, "coincide: the turn end asks");
  await fire();
  await new Promise((resolve) => realSetTimeout(resolve, 300));
  cases.coincide_turn_first = { asked: w.seen.length, prompts: w.prompts.length, timers: live().length };
  clean();
}

{
  const w = await world({
    answers: [started, waiting(), { ...work(), delay: 120 }],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "coincide: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "coincide: the timer armed");
  const due = live();
  due[0].live = false;
  due[0].callback();
  await until(() => w.seen.length >= 3, "coincide: the tick asks");
  w.turn(DIRECTORY);
  await new Promise((resolve) => realSetTimeout(resolve, 300));
  cases.coincide_tick_first = { asked: w.seen.length, prompts: w.prompts.length, timers: live().length };
  clean();
}

{
  const w = await world({
    answers: [started, waiting(), waiting()],
    directories: ["/tmp/a", "/tmp/b"],
  });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "owner: the first prompt");
  w.turn("/tmp/a");
  await until(() => w.storage.get(KEY)?.waiting, "owner: the wait");
  await settle();
  const one = { timers: live().length, asked: w.seen.length };
  w.turn("/tmp/b");
  await until(() => w.seen.length >= 3, "owner: the other location");
  await settle();
  cases.owner = { one, stopped: w.stopped(), timers: live().length, asked: w.seen.length };
  clean();
}

for (const [name, flag] of [
  ["given", "--wait 3"],
  ["equals", "--wait=7"],
  ["zero", "--wait=0"],
  ["seconds", "--wait=30s"],
  ["huge", "--wait 100000"],
  ["fraction", "--wait=0.5"],
]) {
  const w = await world({ answers: [started, waiting()] });
  await w.run(0, "flow-loop", `do the thing ${flag}`);
  await settle();
  const goal = w.storage.get(KEY) ?? null;
  let timers = null;
  if (goal) {
    w.turn(DIRECTORY);
    await until(() => w.storage.get(KEY)?.waiting || w.stopped(), `${name}: the reaction`);
    timers = live().map((timer) => timer.ms);
  }
  cases[`wait_${name}`] = {
    goal,
    prompts: w.prompts.length,
    said: w.synthetics.map((entry) => entry.text),
    timers,
  };
  clean();
}

{
  const goal = (extra) => ({
    prompt: "do the thing",
    run_id: "opencode-abc",
    directory: DIRECTORY,
    wait: 5,
    waiting: { since: "2020-01-01T00:00:00.000Z", next: "2020-01-01T00:05:00.000Z", on: "1 held" },
    ...extra,
  });
  const seeded = { [KEY]: goal({}) };
  const w = await world({ answers: [work()], seed: seeded });
  await until(() => w.prompts.length >= 1, "boot: the re-ask after boot");
  cases.boot = { prompts: w.prompts.length, asked: w.seen.length, goal: w.storage.get(KEY) };
  clean();

  const other = await world({
    answers: [work()],
    directories: ["/tmp/elsewhere"],
    seed: seeded,
  });
  await settle();
  cases.boot_elsewhere = { prompts: other.prompts.length, asked: other.seen.length };

  const done = await world({
    answers: [work()],
    seed: { [KEY]: goal({ stopped: { reason: "the operator stopped the loop", at: "2020-01-01T00:00:00.000Z" } }) },
  });
  await settle();
  cases.boot_stopped = { prompts: done.prompts.length, asked: done.seen.length };

  const later = await world({
    answers: [work()],
    seed: { [KEY]: goal({ waiting: { since: "x", next: new Date(Date.now() + 600000).toISOString(), on: "1 held" } }) },
  });
  await settle();
  cases.boot_future = { prompts: later.prompts.length, timers: live().map((timer) => timer.ms > 0 && timer.ms <= 600000) };
  clean();
}

{
  const w = await world({ answers: [started, waiting(), work(), work()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "stale: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "stale: the timer armed");
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, "stale: the enqueue");
  await settle();
  const before = { prompts: w.prompts.length, timers: live().length };
  await fire();
  cases.stale_timer = { before, prompts: w.prompts.length, asked: w.seen.length };
  clean();
}

for (const [name, directories] of [
  ["owner_first", [DIRECTORY, "/tmp/elsewhere"]],
  ["other_first", ["/tmp/elsewhere", DIRECTORY]],
]) {
  const w = await world({
    answers: [work(), work()],
    directories,
    seed: {
      [KEY]: {
        prompt: "do the thing",
        run_id: "opencode-abc",
        directory: DIRECTORY,
        wait: 5,
        waiting: { since: "x", next: new Date(Date.now() + 600000).toISOString(), on: "1 held" },
      },
    },
  });
  const timers = live().length;
  await fire();
  cases[`boot_${name}`] = { timers, prompts: w.prompts.length, asked: w.seen.length };
  clean();
}

{
  const w = await world({ answers: [started, { ...waiting(), delay: 200 }] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "race: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 2, "race: the turn end asks");
  await w.run(0, "flow-loop", "stop");
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.race = { stopped: w.stopped(), timers: live().length, goal: w.storage.get(KEY) };
  clean();
}

{
  const w = await world({ answers: [started, waiting(), { ...waiting(), delay: 200 }] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "tick race: the first prompt");
  w.turn(DIRECTORY);
  await until(() => live().length === 1, "tick race: the timer armed");
  const due = live();
  due[0].live = false;
  due[0].callback();
  await until(() => w.seen.length >= 3, "tick race: the tick asks");
  await w.run(0, "flow-loop", "stop");
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.tick_race = { stopped: w.stopped(), timers: live().length };
  clean();
}

{
  const w = await world({ answers: [started, { ...waiting(), delay: 200 }] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "disposed: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 2, "disposed: the turn end asks");
  w.instances[0].teardown();
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.disposed = { timers: live().length, prompts: w.prompts.length };
  clean();
}

{
  const w = await world({ answers: [started, waiting()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "between: the first prompt");
  let fired = false;
  w.hooks.beforeSet = async (_key, value) => {
    if (!value?.waiting || fired) return;
    fired = true;
    void w.run(0, "flow-loop", "stop");
    await new Promise((resolve) => realSetTimeout(resolve, 150));
  };
  w.turn(DIRECTORY);
  await until(() => fired, "between: the wait is being written");
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.between = {
    stopped: w.stopped(),
    timers: live().length,
    prompts: w.prompts.length,
    said: w.synthetics.map((entry) => entry.text),
  };
  clean();
}

{
  const w = await world({ answers: [started, { ...waiting(), delay: 200 }, started, waiting()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "leak: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 2, "leak: the turn end asks");
  await w.run(0, "flow-loop", "stop");
  let second = false;
  w.hooks.beforeSet = async (_key, value) => {
    if (!value?.stopped || second) return;
    second = true;
    await w.run(0, "flow-loop", "stop");
  };
  await until(() => w.stopped(), "leak: the deferred stop lands");
  w.hooks.beforeSet = null;
  await new Promise((resolve) => realSetTimeout(resolve, 300));
  await w.run(0, "flow-loop", "go again");
  await until(() => w.prompts.length >= 2, "leak: the new loop starts");
  w.turn(DIRECTORY);
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.leak = { second, stopped: w.stopped(), goal: w.storage.get(KEY), timers: live().length };
  clean();
}

{
  const w = await world({ answers: [started, { ...waiting(), delay: 200 }, started, waiting()] });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "twice: the first prompt");
  w.turn(DIRECTORY);
  await until(() => w.seen.length >= 2, "twice: the turn end asks");
  void w.run(0, "flow-loop", "stop");
  void w.run(0, "flow-loop", "stop");
  await until(() => w.stopped(), "twice: the deferred stop lands");
  await new Promise((resolve) => realSetTimeout(resolve, 300));
  await w.run(0, "flow-loop", "go again");
  await until(() => w.prompts.length >= 2, "twice: the new loop starts");
  w.turn(DIRECTORY);
  await new Promise((resolve) => realSetTimeout(resolve, 500));
  cases.twice = { stopped: w.stopped(), timers: live().length };
  clean();
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

/// A turn that ends while the run still holds a claim continues with the held
/// text alone — the same bytes the Claude and Codex `Stop` hooks hand back —
/// and not with the preamble and the objective again.
#[test]
fn the_loop_plugin_continues_a_live_claim_with_the_held_text_alone() {
    let cases = run_loop_driver();
    let live = &cases["live"];
    let texts: Vec<&str> = live["texts"]
        .as_array()
        .expect("the texts")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let held =
        crate::harness::hook::stop_block(&crate::identity::Identity::Number(42), "opencode-abc");
    assert_eq!(
        texts.get(1).copied(),
        held["reason"].as_str(),
        "a live claim was not continued with the held text: {texts:?}"
    );
    assert!(
        !texts[1].contains("/flow"),
        "a live continuation carried the objective again: {texts:?}"
    );
}

/// A live answer that names no issue is not a claim the plugin can confirm, so
/// the loop stops rather than guessing what to say.
#[test]
fn the_loop_plugin_stops_on_a_live_answer_that_names_no_issue() {
    let cases = run_loop_driver();
    let case = &cases["live_no_issue"];
    assert_eq!(case["prompts"], 1, "{case}");
    assert!(
        case["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("did not report which issue")),
        "{case}"
    );
}

/// The loop plugin waits, rather than stops, while the only work left is held
/// by another run or blocked on a condition that may still come true.
///
/// Requires `node`, like the loop test above, and does not skip. It
/// proves: the wait is recorded where `/goal` reads it and nothing is posted into
/// the session across several re-asks (no `session.prompt`, no
/// `session.synthetic` — either would enter the next turn's context); one timer
/// at a time, at the default five minutes; the re-ask enqueues once work appears
/// and clears the wait; only unconditioned work left stops the loop and names it;
/// a count that could not be read stops it; a claim or an answer Estigia could
/// not confirm is retried on that same timer up to a bounded number of attempts —
/// a later confirmed read continues, and exhaustion stops it with a resumable
/// notice naming the retries; `/loop stop` and plugin teardown
/// clear the timer; a turn end and a tick that coincide enqueue once, in either
/// order; only the instance in the goal's directory holds a timer; and `--wait`
/// is stored, bounded on both sides, and refused with a reason outside them.
#[test]
fn the_loop_plugin_waits_on_held_or_pending_work_and_asks_again() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("flow-loop.mjs"),
        crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the loop plugin writes");
    let harness = LOOP_DRIVER
        .split_once("const scenario = ")
        .expect("the driver's harness ends where its scenarios begin")
        .0;
    let driver = root.path().join("drive-loop.mjs");
    std::fs::write(&driver, format!("{harness}{LOOP_WAIT_SCENARIOS}")).expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this test drives the generated loop plugin, which is JavaScript, and `node` could \
                 not be run ({error}). Install Node and re-run; it is not skipped."
            )
        });
    assert!(
        ran.status.success(),
        "the driver failed: {}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    let cases: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");

    let wait = &cases["wait"];
    assert_eq!(wait["first"]["prompts"], 1, "a wait enqueued: {wait}");
    assert_eq!(
        wait["first"]["stopped"],
        serde_json::Value::Null,
        "held and pending work stopped the loop: {wait}"
    );
    assert_eq!(
        wait["first"]["timers"],
        serde_json::json!([300_000]),
        "the wait did not arm one five-minute timer: {wait}"
    );
    let outcome = wait["first"]["goal"]["last"]["outcome"]
        .as_str()
        .expect("an outcome");
    assert!(
        outcome.starts_with("waiting:")
            && outcome.contains("1 in progress")
            && outcome.contains("1 blocked"),
        "the wait did not say what it waits on: {outcome}"
    );
    for step in ["second", "third"] {
        assert_eq!(
            wait[step]["timers"], 1,
            "a re-ask left {} timers: {wait}",
            wait[step]["timers"]
        );
        assert_eq!(wait[step]["prompts"], 1, "a wait enqueued: {wait}");
    }
    assert_eq!(
        wait["third"]["asked"].as_i64().expect("asked") - wait["second"]["asked"].as_i64().unwrap(),
        1,
        "a tick did not ask exactly once: {wait}"
    );
    assert_eq!(
        wait["quiet"],
        serde_json::json!({ "prompts": 1, "synthetics": 0 }),
        "waiting posted into the session, which costs the next turn tokens: {wait}"
    );
    let said = wait["said"].as_array().expect("said");
    assert!(
        said.iter()
            .filter_map(serde_json::Value::as_str)
            .any(|text| {
                text.contains("Waiting since")
                    && text.contains("1 in progress")
                    && text.contains("next re-ask is at")
                    && text.contains("5 minutes")
            }),
        "`/goal` did not print the wait: {said:?}"
    );
    assert_eq!(wait["after"]["prompts"], 2, "work did not enqueue: {wait}");
    assert_eq!(wait["after"]["timers"], 0, "{wait}");
    assert_eq!(wait["after"]["goal"]["last"]["outcome"], "enqueued");
    assert!(
        wait["after"]["goal"]["waiting"].is_null(),
        "the wait outlived the work that ended it: {wait}"
    );

    let unconditioned = &cases["unconditioned"];
    assert_eq!(unconditioned["timers"], 0, "{unconditioned}");
    assert!(
        unconditioned["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("2 blocked") && reason.contains("a person")),
        "the stop did not name the unconditioned work: {unconditioned}"
    );

    let unadmitted = &cases["unadmitted"];
    assert_eq!(
        unadmitted["stopped"],
        serde_json::Value::Null,
        "an unadmitted backlog ended the loop for good: {unadmitted}"
    );
    assert_eq!(unadmitted["timers"], 1, "{unadmitted}");
    assert!(
        unadmitted["on"]
            .as_str()
            .is_some_and(|on| on.contains("3 open issue(s)") && on.contains("transition")),
        "the wait did not name the unadmitted count and how to admit it: {unadmitted}"
    );

    let unread = &cases["unread"];
    assert!(
        unread["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("could not be read")),
        "an unread count was taken for none: {unread}"
    );

    let retry = &cases["unconfirmed_retry"];
    assert_eq!(
        retry["waited"]["prompts"], 1,
        "a retry enqueued before its wait: {retry}"
    );
    assert_eq!(
        retry["waited"]["stopped"],
        serde_json::Value::Null,
        "an unconfirmed read stopped the loop instead of waiting: {retry}"
    );
    assert_eq!(
        retry["waited"]["timers"],
        serde_json::json!([300_000]),
        "the retry did not arm one wait timer: {retry}"
    );
    assert_eq!(
        retry["waited"]["retries"], 1,
        "the retry did not count its first attempt: {retry}"
    );
    assert!(
        retry["waited"]["on"]
            .as_str()
            .is_some_and(|on| on.contains("could not confirm") && on.contains("unread")),
        "the wait did not say what it was retrying: {retry}"
    );
    assert_eq!(
        retry["after"]["prompts"], 2,
        "a later confirmed read did not continue the loop: {retry}"
    );
    assert_eq!(
        retry["after"]["stopped"],
        serde_json::Value::Null,
        "{retry}"
    );
    assert!(
        retry["after"]["waiting"].is_null() && retry["after"]["retries"].is_null(),
        "the retry count outlived the read that ended it: {retry}"
    );

    let exhausted = &cases["unconfirmed_exhausted"];
    assert_eq!(
        exhausted["prompts"], 1,
        "an exhausted retry enqueued: {exhausted}"
    );
    assert_eq!(exhausted["timers"], 0, "{exhausted}");
    let reason = exhausted["stopped"]["reason"]
        .as_str()
        .expect("the recorded stop reason");
    assert!(
        reason.contains("could not confirm")
            && reason.contains("unread")
            && reason.contains("2 retries"),
        "the exhausted stop did not say the read was retried: {exhausted}"
    );
    assert!(
        !reason.contains("second read"),
        "the exhausted stop still names a second read: {exhausted}"
    );
    assert!(
        exhausted["synthetics"]
            .as_array()
            .expect("the synthetics")
            .iter()
            .any(|entry| entry["resume"] == true
                && entry["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(reason))),
        "the exhausted stop was not said resumably: {exhausted}"
    );

    let stop = &cases["stop"];
    assert_eq!(stop["cleared"], 0, "`/loop stop` left the timer: {stop}");
    assert_eq!(stop["asked"], 2, "a tick asked after the stop: {stop}");
    assert_eq!(stop["prompts"], 1, "{stop}");

    let teardown = &cases["teardown"];
    assert_eq!(
        teardown["cleared"], 0,
        "teardown left the timer: {teardown}"
    );
    assert_eq!(
        teardown["prompts"], 1,
        "a torn-down plugin still enqueued: {teardown}"
    );

    for name in ["coincide_turn_first", "coincide_tick_first"] {
        let case = &cases[name];
        assert_eq!(
            case["prompts"], 2,
            "a turn end and a tick enqueued twice ({name}): {case}"
        );
        assert_eq!(case["asked"], 3, "{name}: {case}");
        assert_eq!(case["timers"], 0, "{name}: {case}");
    }

    let owner = &cases["owner"];
    assert_eq!(
        owner["one"]["timers"], 1,
        "the goal's directory did not hold exactly one timer: {owner}"
    );
    assert_eq!(
        owner["timers"], 1,
        "a turn end located elsewhere left the goal's directory without exactly one timer: {owner}"
    );
    assert_eq!(owner["asked"], 3, "{owner}");
    assert!(
        owner["stopped"].is_null(),
        "a turn end located elsewhere stopped the loop: {owner}"
    );

    let boot = &cases["boot"];
    assert_eq!(
        boot["prompts"], 1,
        "a waiting goal found at boot was not re-asked without a turn end: {boot}"
    );
    assert_eq!(boot["asked"], 1, "{boot}");
    assert!(
        boot["goal"]["waiting"].is_null(),
        "the wait outlived the work that ended it: {boot}"
    );
    assert_eq!(
        cases["boot_elsewhere"],
        serde_json::json!({ "prompts": 0, "asked": 0 }),
        "an instance in another location re-armed the goal"
    );
    assert_eq!(
        cases["boot_stopped"],
        serde_json::json!({ "prompts": 0, "asked": 0 }),
        "a stopped goal was re-armed"
    );
    assert_eq!(
        cases["boot_future"]["prompts"], 0,
        "{}",
        cases["boot_future"]
    );
    assert_eq!(
        cases["boot_future"]["timers"],
        serde_json::json!([true]),
        "a future re-ask was not armed for the time left"
    );

    assert_eq!(
        cases["stale_timer"],
        serde_json::json!({ "before": { "prompts": 2, "timers": 0 }, "prompts": 2, "asked": 3 }),
        "a timer armed before work appeared survived the enqueue and enqueued again"
    );
    let between = &cases["between"];
    assert_eq!(between["timers"], 0, "{between}");
    assert_eq!(between["prompts"], 1, "{between}");
    assert_eq!(
        between["stopped"]["reason"], "the operator stopped the loop",
        "a stop landing between the wait's read and its write was overwritten: {between}"
    );
    assert!(
        between["said"]
            .as_array()
            .expect("said")
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|text| text.contains("stopping as soon as")),
        "the operator was not told the stop was in flight: {between}"
    );
    for name in ["leak", "twice"] {
        let case = &cases[name];
        assert_eq!(
            case["stopped"],
            serde_json::Value::Null,
            "a stop request outlived its loop and stopped the next one ({name}): {case}"
        );
        assert_eq!(
            case["timers"], 1,
            "the new loop did not wait ({name}): {case}"
        );
    }
    for name in ["race", "tick_race"] {
        let case = &cases[name];
        assert_eq!(
            case["timers"], 0,
            "a stop that raced a read armed a timer ({name}): {case}"
        );
        assert_eq!(
            case["stopped"]["reason"], "the operator stopped the loop",
            "the wait overwrote the operator's stop ({name}): {case}"
        );
    }
    assert_eq!(
        cases["disposed"],
        serde_json::json!({ "timers": 0, "prompts": 1 }),
        "a decision that was reading at teardown armed a timer afterwards"
    );
    for name in ["boot_owner_first", "boot_other_first"] {
        assert_eq!(
            cases[name],
            serde_json::json!({ "timers": 1, "prompts": 1, "asked": 1 }),
            "the owner's re-ask was lost when a second location booted ({name})"
        );
    }

    for (name, minutes) in [("given", 3), ("equals", 7)] {
        let case = &cases[format!("wait_{name}")];
        assert_eq!(case["goal"]["wait"], minutes, "{name}: {case}");
        assert_eq!(
            case["goal"]["prompt"], "/flow do the thing",
            "{name}: {case}"
        );
        assert_eq!(
            case["timers"],
            serde_json::json!([minutes * 60_000]),
            "{name}: {case}"
        );
    }
    for name in ["zero", "seconds", "huge", "fraction"] {
        let case = &cases[format!("wait_{name}")];
        assert_eq!(
            case["goal"],
            serde_json::Value::Null,
            "a refused --wait started the loop ({name}): {case}"
        );
        assert_eq!(case["prompts"], 0, "{name}: {case}");
        assert!(
            case["said"][0]
                .as_str()
                .is_some_and(|text| text.contains("--wait takes whole minutes from 1 to 1440")),
            "the refusal did not say why ({name}): {case}"
        );
    }
}

/// Scenarios for the loop plugin's hold on a running child session. Timers of a
/// minute or more are held and fired by hand, as in `LOOP_WAIT_SCENARIOS`.
const LOOP_CHILD_SCENARIOS: &str = r#"
const realSetTimeout = globalThis.setTimeout;
const realClearTimeout = globalThis.clearTimeout;
const held = [];
globalThis.setTimeout = (callback, ms, ...rest) => {
  if (ms >= 60000) {
    const timer = { callback, ms, live: true };
    held.push(timer);
    return timer;
  }
  return realSetTimeout(callback, ms, ...rest);
};
globalThis.clearTimeout = (timer) => {
  if (timer && typeof timer === "object" && "live" in timer) timer.live = false;
  else realClearTimeout(timer);
};
const live = () => held.filter((timer) => timer.live);
const fire = async () => {
  const due = live();
  for (const timer of due) timer.live = false;
  for (const timer of due) timer.callback();
  await settle();
};
const clean = () => {
  for (const timer of held) timer.live = false;
};

const PARENT = "ses_loop_fixture";
const liveAnswer = { exitCode: 0, stdout: JSON.stringify({ run_id: "opencode-abc", live: true, issue: 42 }) };
const answers = (n) => Array.from({ length: n }, () => liveAnswer);

const open = async (n, sessions) => {
  const w = await world({ answers: answers(n), sessions });
  await w.run(0, "flow-loop", "do the thing");
  await until(() => w.prompts.length >= 1, "the first prompt");
  return w;
};
const begin = async (w, child, announce = true) => {
  if (announce) w.emit("session.created", { sessionID: child, parentID: PARENT });
  w.emit("session.execution.started", { sessionID: child });
  await settle();
};
const end = async (w, child, type = "session.execution.succeeded") => {
  w.emit(type, { sessionID: child });
  await settle();
};
const outcome = (w) => w.storage.get(KEY)?.last?.outcome ?? null;

const cases = {};

{
  const w = await open(4, {});
  await begin(w, "ses_child_a");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "hold: the wait recorded");
  const held1 = {
    prompts: w.prompts.length,
    outcome: outcome(w),
    on: w.storage.get(KEY).waiting.on,
    timers: live().map((timer) => timer.ms),
  };
  w.turn(DIRECTORY);
  await settle();
  const again = { prompts: w.prompts.length };
  await end(w, "ses_child_a");
  await until(() => w.prompts.length >= 2, "hold: the resumption");
  await settle();
  cases.hold = {
    held: held1,
    again,
    after: {
      prompts: w.prompts.length,
      to: w.prompts.map((entry) => entry.sessionID),
      waiting: w.storage.get(KEY)?.waiting ?? null,
      outcome: outcome(w),
      timers: live().length,
    },
  };
  clean();
}

{
  const w = await open(2, {});
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, "none: the continuation");
  await settle();
  cases.none = { prompts: w.prompts.length, outcome: outcome(w), timers: live().length };
  clean();
}

{
  const w = await open(3, { ses_child_b: PARENT });
  await begin(w, "ses_child_b", false);
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "looked up: the wait recorded");
  const prompts = w.prompts.length;
  await end(w, "ses_child_b", "session.execution.failed");
  await until(() => w.prompts.length >= 2, "looked up: the resumption");
  await settle();
  cases.looked_up = { held: prompts, after: w.prompts.length };
  clean();
}

{
  const w = await open(2, {});
  await begin(w, "ses_unknown", false);
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, "unreadable: the continuation");
  await settle();
  cases.unreadable = { prompts: w.prompts.length, stopped: w.stopped() };
  clean();
}

{
  const w = await open(4, {});
  await begin(w, "ses_child_c");
  await begin(w, "ses_child_d");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "two: the wait recorded");
  const on = w.storage.get(KEY).waiting.on;
  await end(w, "ses_child_c", "session.execution.interrupted");
  const afterFirst = w.prompts.length;
  await end(w, "ses_child_d");
  await until(() => w.prompts.length >= 2, "two: the resumption");
  await settle();
  cases.two = { on, afterFirst, after: w.prompts.length };
  clean();
}

{
  const w = await open(4, {});
  await begin(w, "ses_child_e");
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "backstop: the wait recorded");
  await fire();
  await until(() => w.prompts.length >= 2, "backstop: the timer continues");
  await settle();
  const fired = { prompts: w.prompts.length, waiting: w.storage.get(KEY)?.waiting ?? null };
  w.turn(DIRECTORY);
  await until(() => w.storage.get(KEY)?.waiting, "backstop: the wait recorded again");
  cases.backstop = { fired, prompts: w.prompts.length, timers: live().length };
  clean();
}

{
  const w = await open(3, {});
  await begin(w, "ses_child_f");
  await end(w, "ses_child_f");
  await settle();
  const mid = w.prompts.length;
  w.turn(DIRECTORY);
  await until(() => w.prompts.length >= 2, "ended mid-turn: the continuation");
  await settle();
  cases.mid_turn = { mid, prompts: w.prompts.length };
  clean();
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

/// The loop plugin does not re-prompt a session that ended its turn to wait on a
/// child session it can see running, records a wait naming the child instead,
/// and continues the parent once when the last child ends.
///
/// One of the tests in the crate that require `node`; it does not skip when
/// `node` is absent. It proves: a live claim with a running child enqueues
/// nothing and a second turn end enqueues nothing either; the parent is
/// continued once when the child ends, however it ends, and only after the last of
/// two children; a child learned from `session.created` and one looked up
/// through `session.get` are both held, and one whose lookup throws is not; with
/// no child the continuation is immediate; the wait timer continues the parent
/// once even though the child's end was never seen; a child that ended before the
/// turn ended does not hold it.
#[test]
fn the_loop_plugin_waits_while_a_child_session_runs_and_resumes_when_it_ends() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("flow-loop.mjs"),
        crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the loop plugin writes");
    let harness = LOOP_DRIVER
        .split_once("const scenario = ")
        .expect("the driver's harness ends where its scenarios begin")
        .0;
    let driver = root.path().join("drive-loop.mjs");
    std::fs::write(&driver, format!("{harness}{LOOP_CHILD_SCENARIOS}")).expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this test drives the generated loop plugin, which is JavaScript, and `node` could \
                 not be run ({error}). Install Node and re-run; it is not skipped."
            )
        });
    assert!(
        ran.status.success(),
        "the driver failed: {}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    let cases: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");

    let hold = &cases["hold"];
    assert_eq!(
        hold["held"]["prompts"], 1,
        "a running child did not hold the parent: {hold}"
    );
    let outcome = hold["held"]["outcome"].as_str().expect("an outcome");
    assert!(
        outcome.starts_with("waiting:") && outcome.contains("ses_child_a"),
        "the wait did not name the child: {hold}"
    );
    assert!(
        hold["held"]["on"]
            .as_str()
            .is_some_and(|on| on.contains("ses_child_a")),
        "the stored wait did not name the child: {hold}"
    );
    assert_eq!(
        hold["held"]["timers"],
        serde_json::json!([300_000]),
        "{hold}"
    );
    assert_eq!(
        hold["again"]["prompts"], 1,
        "a second turn end enqueued: {hold}"
    );
    assert_eq!(
        hold["after"]["prompts"], 2,
        "the child's end did not continue the parent once: {hold}"
    );
    assert_eq!(
        hold["after"]["to"],
        serde_json::json!(["ses_loop_fixture", "ses_loop_fixture"])
    );
    assert_eq!(hold["after"]["waiting"], serde_json::Value::Null, "{hold}");
    assert_eq!(hold["after"]["outcome"], "enqueued", "{hold}");
    assert_eq!(hold["after"]["timers"], 0, "{hold}");

    let none = &cases["none"];
    assert_eq!(
        none["prompts"], 2,
        "with no child the continuation was not immediate: {none}"
    );
    assert_eq!(none["outcome"], "enqueued", "{none}");
    assert_eq!(none["timers"], 0, "{none}");

    let looked_up = &cases["looked_up"];
    assert_eq!(
        looked_up["held"], 1,
        "a child found by lookup did not hold the parent: {looked_up}"
    );
    assert_eq!(
        looked_up["after"], 2,
        "a failed child did not resume the parent once: {looked_up}"
    );

    let unreadable = &cases["unreadable"];
    assert_eq!(
        unreadable["prompts"], 2,
        "a failed lookup held the parent: {unreadable}"
    );
    assert_eq!(
        unreadable["stopped"],
        serde_json::Value::Null,
        "a failed lookup stopped it: {unreadable}"
    );

    let two = &cases["two"];
    assert!(
        two["on"]
            .as_str()
            .is_some_and(|on| on.contains("ses_child_c") && on.contains("ses_child_d")),
        "the wait did not name both children: {two}"
    );
    assert_eq!(
        two["afterFirst"], 1,
        "the parent resumed with a child still running: {two}"
    );
    assert_eq!(
        two["after"], 2,
        "the last child's end did not resume the parent once: {two}"
    );

    let backstop = &cases["backstop"];
    assert_eq!(
        backstop["fired"]["prompts"], 2,
        "the timer did not continue a parent whose child never reported: {backstop}"
    );
    assert_eq!(
        backstop["fired"]["waiting"],
        serde_json::Value::Null,
        "{backstop}"
    );
    assert_eq!(
        backstop["prompts"], 2,
        "the next turn end did not wait again: {backstop}"
    );
    assert_eq!(backstop["timers"], 1, "{backstop}");

    let mid_turn = &cases["mid_turn"];
    assert_eq!(
        mid_turn["mid"], 1,
        "a child's end continued a parent that was not waiting: {mid_turn}"
    );
    assert_eq!(
        mid_turn["prompts"], 2,
        "an ended child held the parent: {mid_turn}"
    );
}

const LOOP_DEFAULT_SCENARIOS: &str = r#"
const cases = {};
for (const [name, text] of [
  ["bare", ""],
  ["blank", "   "],
  ["max", "--max 2"],
  ["wait", "--wait=10"],
  ["both", "--wait 3 --max 4"],
  ["free", "only Alicante and Murcia"],
  ["slash", "/flow triage x"],
  ["word", "flow dev 12"],
  ["alone", "flow"],
  ["slash_alone", "/flow"],
  ["flowchart", "flowchart x"],
  ["slash_prefixed", "/flowchart x"],
  ["flagged_free", "--max 2 do the thing"],
  ["flagged_flow", "--max 2 /flow triage x"],
]) {
  const w = await world({ answers: [started] });
  await w.run(0, "flow-loop", text);
  await settle();
  const goal = w.storage.get(KEY) ?? null;
  await w.run(0, "flow-status", "");
  cases[name] = {
    prompt: goal?.prompt ?? null,
    max: goal?.max ?? null,
    wait: goal?.wait ?? null,
    texts: w.prompts.map((entry) => entry.text),
    status: w.synthetics.map((entry) => entry.text),
  };
}
for (const [given] of shared) {
  const w = await world({ answers: [started] });
  await w.run(0, "flow-loop", given);
  await settle();
  cases["shared:" + given] = {
    prompt: w.storage.get(KEY)?.prompt ?? null,
    texts: w.prompts.map((entry) => entry.text),
  };
}
{
  const w = await world({ answers: [] });
  await w.run(0, "flow-loop", "stop");
  cases.stop = {
    stored: w.storage.has(KEY),
    prompts: w.prompts.length,
    said: w.synthetics.map((entry) => entry.text),
    described: w.synthetics.map((entry) => entry.description),
  };
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

/// Requires `node`, like the loop test above, and does not skip.
#[test]
fn the_loop_plugin_defaults_its_objective_to_flow_dev() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("flow-loop.mjs"),
        crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the loop plugin writes");
    let harness = LOOP_DRIVER
        .split_once("const scenario = ")
        .expect("the driver's harness ends where its scenarios begin")
        .0;
    let driver = root.path().join("drive-loop.mjs");
    std::fs::write(
        &driver,
        format!(
            "{harness}const completedAt = () => null;\nconst started = {{ exitCode: 0, stdout: JSON.stringify({{ run_id: \"opencode-abc\", live: true, issue: 42 }}) }};\nconst shared = {};\n{LOOP_DEFAULT_SCENARIOS}",
            serde_json::to_string(crate::harness::loop_text::SLASH_OBJECTIVES)
                .expect("the shared cases encode")
        ),
    )
    .expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this test drives the generated loop plugin, which is JavaScript, and `node` could \
                 not be run ({error}). Install Node and re-run; it is not skipped."
            )
        });
    assert!(
        ran.status.success(),
        "the driver failed: {}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    let cases: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");

    for (name, prompt, max, wait) in [
        ("bare", "/flow dev", None, 5),
        ("blank", "/flow dev", None, 5),
        ("max", "/flow dev", Some(2), 5),
        ("wait", "/flow dev", None, 10),
        ("both", "/flow dev", Some(4), 3),
        ("free", "/flow only Alicante and Murcia", None, 5),
        ("slash", "/flow triage x", None, 5),
        ("word", "flow dev 12", None, 5),
        ("alone", "flow", None, 5),
        ("slash_alone", "/flow", None, 5),
        ("flowchart", "/flow flowchart x", None, 5),
        ("slash_prefixed", "/flow /flowchart x", None, 5),
        ("flagged_free", "/flow do the thing", Some(2), 5),
        ("flagged_flow", "/flow triage x", Some(2), 5),
    ] {
        let case = &cases[name];
        assert_eq!(case["prompt"], prompt, "{name}: {case}");
        assert_eq!(
            case["max"],
            max.map_or(serde_json::Value::Null, serde_json::Value::from),
            "{name}: {case}"
        );
        assert_eq!(case["wait"], wait, "{name}: {case}");
        let texts = case["texts"].as_array().expect("the texts");
        assert_eq!(texts.len(), 1, "{name}: {case}");
        assert!(
            texts[0]
                .as_str()
                .is_some_and(|text| text.contains("opencode-abc") && text.ends_with(prompt)),
            "{name}: the enqueued text did not end with the effective prompt: {case}"
        );
        assert!(
            case["status"][0]
                .as_str()
                .is_some_and(|text| text.contains(&format!("Estigia goal: {prompt}\n"))),
            "{name}: /flow-status did not print the effective prompt: {case}"
        );
    }

    for (given, expected) in crate::harness::loop_text::SLASH_OBJECTIVES {
        let case = &cases[format!("shared:{given}")];
        assert_eq!(case["prompt"], *expected, "{given:?}: {case}");
        assert_eq!(
            case["texts"][0],
            format!(
                "{}\n\n{expected}",
                crate::harness::loop_text::preamble("opencode-abc", 0, true)
            ),
            "{given:?}: the plugin's continuation is not the shared preamble and objective"
        );
    }

    let stop = &cases["stop"];
    assert_eq!(stop["stored"], false, "{stop}");
    assert_eq!(stop["prompts"], 0, "{stop}");
    assert!(
        stop["said"][0]
            .as_str()
            .is_some_and(|text| text.contains("no running loop")),
        "`stop` was taken as an objective: {stop}"
    );
}

#[test]
fn the_opencode_plugin_hands_an_oversized_input_to_the_gate_on_standard_input() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    std::fs::write(
        base.join("estigia-workflow-authority.mjs"),
        crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the plugin writes");
    std::fs::write(
        base.join("gate"),
        r#"const fs = require("node:fs");
const stdin = fs.readFileSync(0, "utf8");
fs.writeFileSync(
  process.env.STUB_LOG,
  JSON.stringify({ args: process.argv.slice(2), bytes: stdin.length, content: JSON.parse(stdin).command.length }),
);
process.stderr.write("refused: big");
process.exit(1);
"#,
    )
    .expect("the stand-in writes");
    std::fs::write(
        base.join("drive.mjs"),
        r#"import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import plugin, { EstigiaWorkflowAuthority } from "./estigia-workflow-authority.mjs";

const standIn = (_strings, ...values) => {
  const input = values.at(-1);
  if (!Buffer.isBuffer(input)) throw new Error("the payload was not handed over as a redirected buffer");
  let line = _strings[0];
  values.slice(0, -1).forEach((value, index) => { line += String(value) + _strings[index + 1]; });
  const args = line.split("<")[0].trim().split(/\s+/).slice(1);
  const chain = {
    cwd() { return chain; },
    quiet() { return chain; },
    nothrow() {
      const ran = spawnSync(process.execPath, ["./gate", ...args], { input, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
      return Promise.resolve({ exitCode: ran.status, stderr: ran.stderr });
    },
  };
  return chain;
};

const command = "echo " + "x".repeat(3 * 1024 * 1024);
const attempt = async (label, run) => {
  let said = "";
  try { await run(); } catch (error) { said = String(error.message); }
  const recorded = JSON.parse(readFileSync(process.env.STUB_LOG, "utf8"));
  return { label, said, recorded };
};

const hooks = await EstigiaWorkflowAuthority({ $: standIn, directory: process.cwd() });
const v1 = await attempt("v1", () => hooks["tool.execute.before"]({ tool: "bash", sessionID: "ses_big" }, { args: { command } }));

globalThis.Bun = { $: standIn };
const registered = [];
await plugin.setup({ location: { directory: process.cwd() }, tool: { hook: async (name, callback) => { registered.push([name, callback]); } } });
const v2 = await attempt("v2", () => registered.find(([name]) => name === "execute.before")[1]({ tool: "shell", sessionID: "ses_big", input: { command } }));

console.log(JSON.stringify([v1, v2]));
"#,
    )
    .expect("the driver writes");

    let ran = std::process::Command::new(node_executable())
        .arg("drive.mjs")
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.json"))
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    for result in &results {
        assert_eq!(result["said"], "refused: big", "{}", result["label"]);
        let recorded = &result["recorded"];
        assert_eq!(recorded["content"], 5 + 3 * 1024 * 1024, "{result}");
        assert_eq!(recorded["args"][2], "--input", "{result}");
        assert_eq!(recorded["args"][3], "-", "{result}");
    }
}

/// The loop plugin leaves a record whenever it fails to react, and keeps
/// listening after its event stream ends or throws.
///
/// Requires `node`, like the loop test above, and does not skip.
#[test]
fn the_loop_plugin_records_what_it_saw_and_listens_again() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("flow-loop.mjs"),
        crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the loop plugin writes");
    let harness = LOOP_DRIVER
        .split_once("const scenario = ")
        .expect("the driver's harness ends where its scenarios begin")
        .0;
    let driver = root.path().join("drive-loop.mjs");
    std::fs::write(&driver, format!("{harness}{LOOP_SILENCE_SCENARIOS}"))
        .expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this test drives the generated loop plugin, which is JavaScript, and `node` could \
                 not be run ({error}). Install Node and re-run; it is not skipped."
            )
        });
    assert!(
        ran.status.success(),
        "the driver failed: {}\n{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr)
    );
    let cases: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");

    for name in ["throws", "ends"] {
        let case = &cases[name];
        assert_eq!(
            case["prompts"], 2,
            "a stream that {name} left the loop deaf to the next turn end: {case}"
        );
        assert!(
            case["watch"]["outcome"]
                .as_str()
                .is_some_and(|outcome| outcome.starts_with("interrupted")),
            "the interruption was not recorded where `/goal` reads it ({name}): {case}"
        );
        let last = case["said"]
            .as_array()
            .and_then(|said| said.last())
            .and_then(serde_json::Value::as_str)
            .expect("`/goal` said something");
        assert!(
            last.contains("interrupted"),
            "`/goal` hid the interruption: {last}"
        );
    }

    let thrown = &cases["thrown"];
    assert_eq!(thrown["prompts"], 1, "{thrown}");
    assert!(
        thrown["goal"]["stopped"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("the shell fell over")),
        "an exception inside the reaction left the goal running: {thrown}"
    );
    assert!(
        thrown["goal"]["last"]["outcome"]
            .as_str()
            .is_some_and(|outcome| outcome.starts_with("failed:")),
        "{thrown}"
    );
    let printed = thrown["said"].as_array().expect("said");
    assert!(
        printed
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|text| {
                text.contains("The loop stopped at") && text.contains("the shell fell over")
            }),
        "`/goal` did not print the failure: {thrown}"
    );

    let report = cases["report"]["said"].as_array().expect("said");
    let texts: Vec<&str> = report
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        texts
            .iter()
            .all(|text| !text.contains("The loop has not stopped")),
        "`/goal` still asserts liveness it cannot see: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|text| text.contains("No turn end has been observed")),
        "`/goal` before any turn end did not say none was observed: {texts:?}"
    );
    assert!(
        texts.last().is_some_and(
            |text| text.contains("Last turn end observed at") && text.contains("enqueued")
        ),
        "`/goal` after a reaction did not print it: {texts:?}"
    );

    let described: Vec<&str> = cases["report"]["described"]
        .as_array()
        .expect("described")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        described
            .iter()
            .any(|shown| shown.contains("Estigia goal: /flow do the thing")),
        "`/flow-status` did not put the stored goal in the description OpenCode shows: {described:?}"
    );

    let foreign = &cases["foreign"];
    assert_eq!(foreign["prompts"], 2, "{foreign}");
    assert_eq!(
        foreign["last"]["outcome"], "enqueued",
        "an instance in another location overwrote the owner's record: {foreign}"
    );
}

/// Every write-shaped tool in the plugin's `GATED` list hands the gate the
/// target path its write lands on.
///
/// The test above drives the shell tools — the one kind the earlier
/// cure reached — and
/// nothing else, while the gated write tools carry a target path rather
/// than a working directory. Their calls were adjudicated at the project root,
/// so two concurrent isolated runs refused each other's writes. A tool added
/// to `GATED` without evidence of where its call lands is a test failure, not
/// a silent fallback: each write tool is driven here — through both host
/// generations, with a `file_path` naming a worktree on OpenCode 1 and a `path`
/// on OpenCode 2 — and the payload the gate receives must carry that path, the
/// only evidence the Rust side has of where the write lands.
///
/// The driven list is **parsed out of the plugin source**, the same way
/// `the_plugin_gates_the_tools_the_classifier_judges` and
/// `the_opencode_plugin_only_names_tools_the_gate_can_classify` already read
/// `const GATED = [...]`, rather than copied by hand — a hardcoded name list
/// stays green when another name is added to `GATED` and nothing
/// drives it. A name in `GATED` that is not a shell tool and is not in
/// [`crate::harness::WRITE_TOOLS`] fails loudly below, closing the escape a
/// blind panel traced: such a name already satisfies
/// `the_plugin_gates_the_tools_the_classifier_judges` (which only asks
/// membership in `WRITE_TOOLS` **or** `SHELL_TOOLS`) and then falls off the
/// end of `narrowed_by_the_call` with no evidence at all — silently, unless
/// something here refuses to let it pass unclamped.
///
/// This is the same interpreter requirement as its sibling: it drives the
/// generated plugin, which is JavaScript, and does not skip when `node` is
/// absent.
#[test]
fn the_plugin_hands_the_gate_the_directory_every_write_lands_in() {
    let source = crate::setup::plugin::source(std::path::Path::new("/opt/estigia"));
    let gated: Vec<String> = source
        .split_once("const GATED = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(list, _)| list.to_owned())
        .expect("the plugin declares which tools it gates")
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_owned())
        .filter(|name| !name.is_empty())
        .collect();

    // The shell tools carry a working directory, not a target, and are the
    // sibling test's tools. Everything else in `GATED` is claimed as
    // write-shaped — and every one of those claims is checked, not assumed.
    let write_tools: Vec<String> = gated
        .iter()
        .filter(|name| !name.eq_ignore_ascii_case("bash") && !name.eq_ignore_ascii_case("shell"))
        .cloned()
        .collect();
    for name in &write_tools {
        assert!(
            crate::harness::WRITE_TOOLS
                .iter()
                .any(|write| name.eq_ignore_ascii_case(write)),
            "the plugin gates {name:?} as a write-shaped tool, but \
             `harness::WRITE_TOOLS` does not know it: `narrowed_by_the_call` \
             would have no evidence for where its call lands, and this test \
             would drive it with a target no Rust code reads for it"
        );
    }

    let root = tempfile::tempdir().expect("a temporary directory");
    let module = root.path().join("estigia-workflow-authority.mjs");
    std::fs::write(&module, &source).expect("the plugin writes");

    let project = "/tmp/base-checkout";
    let worktree = "/tmp/base-checkout/wt-a";

    let tool_list = write_tools
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let driver = root.path().join("drive-writes.mjs");
    std::fs::write(
        &driver,
        format!(
            r#"import plugin, {{ EstigiaWorkflowAuthority }} from "./estigia-workflow-authority.mjs";

const recorder = (seen) => (_strings, ...values) => {{
  const call = {{ values: values.map(String), cwd: null }};
  const chain = {{
    cwd(directory) {{ call.cwd = String(directory); return chain; }},
    quiet() {{ return chain; }},
    nothrow() {{ seen.push(call); return Promise.resolve({{ exitCode: 0 }}); }},
  }};
  return chain;
}};

const seenV1 = [];
const hooks = await EstigiaWorkflowAuthority({{
  $: recorder(seenV1),
  directory: {project:?},
  worktree: undefined,
}});
for (const tool of [{tool_list}]) {{
  await hooks["tool.execute.before"](
    {{ tool }},
    {{ args: {{ file_path: "{worktree}/src/main.rs" }} }},
  );
}}

const seenV2 = [];
globalThis.Bun = {{ $: recorder(seenV2) }};
const registered = [];
const ctx = {{
  location: {{ directory: {project:?} }},
  tool: {{
    hook: async (name, callback) => {{
      registered.push([name, callback]);
    }},
  }},
}};
await plugin.setup(ctx);
const entry = registered.find(([name]) => name === "execute.before");
if (!entry) throw new Error("setup registered no execute.before hook");
for (const tool of [{tool_list}]) {{
  await entry[1]({{ tool, input: {{ path: "{worktree}/src/main.rs" }} }});
}}

console.log(JSON.stringify({{ v1: seenV1, v2: seenV2 }}));
"#
        ),
    )
    .expect("the driver writes");

    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generated plugin, which is JavaScript, and \
             `node` could not be run ({error}). Install Node and re-run; it is \
             not skipped, because a plugin nothing executes is a gate nothing \
             measures."
        ),
    };
    assert!(
        ran.status.success(),
        "the plugin would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );

    let answers: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    for (generation, field) in [("v1", "file_path"), ("v2", "path")] {
        let calls = answers[generation].as_array().expect("a list of calls");
        assert_eq!(
            calls.len(),
            write_tools.len(),
            "the {generation} hook asked the gate {} times, expected once per gated write tool",
            calls.len()
        );
        for (index, call) in calls.iter().enumerate() {
            // The target path travels in the payload — that is the evidence of
            // where the write lands, and it is what `narrowed_by_the_call` reads
            // on the Rust side. A plugin that filtered the arguments down to the
            // ones it recognised would break the fix without touching it, exactly
            // as the `workdir` half documented for the shell tools. The plugin
            // launches the gate in the project root; the payload is the only
            // thing that tells it the write is headed for a worktree.
            let values: Vec<&str> = call["values"]
                .as_array()
                .expect("the interpolated values")
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect();
            let payload = values
                .iter()
                .find(|value| value.starts_with('{'))
                .unwrap_or_else(|| panic!("no payload was interpolated: {values:?}"));
            let payload: serde_json::Value =
                serde_json::from_str(payload).expect("the payload is JSON");
            assert_eq!(
                payload[field],
                format!("{worktree}/src/main.rs"),
                "{generation} call {index} dropped the target path the call named: {payload}"
            );
            // And the gate is launched at the project root, so the payload is the
            // only evidence the Rust side has — reading it is not optional.
            assert_eq!(
                call["cwd"], project,
                "{generation} call {index} launched the gate somewhere other than the \
                 project root: {call}"
            );
        }
    }
}

#[test]
fn a_fresh_opencode_adapter_file_names_task_judges_not_handoff() {
    // The machine-local estigia.opencode.md is not installed by setup. The
    // next clone only has what fresh_agent_file writes. If that file still
    // says nothing, the next OpenCode run repeats the earlier handoff.
    let dir = tempfile::tempdir().expect("a temporary skill root");
    let opencode = dir.path().join("estigia.opencode.md");
    let other = dir.path().join("estigia.cursor.md");
    write_agent_configuration_wholly(&opencode, "opencode", &Config::default())
        .expect("opencode's file writes");
    write_agent_configuration_wholly(&other, "cursor", &Config::default())
        .expect("cursor's file writes");
    let text = fs::read_to_string(&opencode).expect("opencode's file reads");
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("Call`handoff_review`onlyifthatlaunchfailed,neverasthefirstmove"),
        "a fresh OpenCode adapter no longer bounds handoff to a failed launch: {text}"
    );
    assert!(
        compact.contains("After`publish_review`")
            && compact
                .contains("call`launch_review_input`andthen`collect_review_input`foreachinput"),
        "a fresh OpenCode adapter no longer names the post-publish launch_review_input call: {text}"
    );
    assert!(
        compact.contains("withoutlaunchingfirstisadefectagainstthissentence"),
        "a fresh OpenCode adapter no longer names handoff-first as a defect: {text}"
    );
    assert!(
        compact.contains("emptyscratchdirectory")
            && compact.contains("notthetrackerbinding")
            && compact.contains("--setting-sources"),
        "a fresh OpenCode adapter no longer names what a reviewer does not start with: {text}"
    );
    assert!(
        !text.contains("configured panel") && !text.contains("two or five"),
        "a fresh OpenCode adapter still tells a host to read a judge count from review_tier: {text}"
    );
    let other = fs::read_to_string(&other).expect("cursor's file reads");
    assert!(
        !other.contains("## Review on this host"),
        "the OpenCode review section leaked onto another adapter: {other}"
    );
}

#[test]
fn an_upgraded_opencode_adapter_file_replaces_a_stale_review_section() {
    // `write_agent_configuration_from_snapshot`/`_into` used to touch only
    // `CONFIG_FENCE` on an existing file — the settings table — so a host
    // that installed before this fix kept whatever `fresh_agent_file` wrote
    // at install time forever: the settings table refreshed on every
    // `estigia setup`, the review section never did, and it could still say
    // to read a judge count from `review_tier` after that stopped being one.
    let dir = tempfile::tempdir().expect("a temporary skill root");
    let opencode = dir.path().join("estigia.opencode.md");
    let stale = "# Estigia configuration for `opencode`\n\n\
                 This adapter shares a skill directory with others, so its settings live here \
                 rather than in the contract. Rows here override the contract's table for this \
                 adapter only. `estigia.local.md`, if you have one, still overrides both.\n\n\
                 ## Notes\n\n\
                 The operator's own paragraph, ahead of the settings table.\n\n\
                 <!-- estigia:config:start -->\n\nold table\n\n<!-- estigia:config:end -->\n\n\
                 ## Review on this host\n\n\
                 `review-blind` is a Claude launch type. It is not a missing capability here.\n\
                 Launch the configured panel with `Task` in one turn \u{2014} the number a `review_tier`\n\
                 reading acquired: two or five contexts when unreduced, or one when that reading\n\
                 reduced the panel \u{2014} same receipt, no sibling output, each in its own directory \u{2014}\n\
                 then record one aggregate `review_verdict`. Call `handoff_review` only if that launch failed.\n\n\
                 ## Operator's own section\n\n\
                 A paragraph the operator wrote below the review section, kept for their own record.\n";
    fs::write(&opencode, stale).expect("the stale file writes");
    write_agent_configuration_wholly(&opencode, "opencode", &Config::default())
        .expect("opencode's file upgrades");
    let text = fs::read_to_string(&opencode).expect("opencode's file reads");
    assert!(
        !text.contains("configured panel") && !text.contains("two or five"),
        "an upgraded OpenCode adapter still carries the stale review-tier judge count: {text}"
    );
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("call`launch_review_input`andthen`collect_review_input`foreachinput")
            && compact.contains("After`publish_review`")
            && compact.contains("thesealedlenses,"),
        "an upgraded OpenCode adapter did not replace the stale section with the current one: {text}"
    );
    assert_eq!(
        text.matches("## Review on this host").count(),
        1,
        "the upgrade left two review sections instead of replacing the one it found: {text}"
    );
    assert!(
        text.contains("## Notes")
            && text.contains("The operator's own paragraph, ahead of the settings table."),
        "the upgrade lost operator text that stood before the config fence: {text}"
    );
    assert!(
        text.contains("## Operator's own section")
            && text.contains(
                "A paragraph the operator wrote below the review section, kept for their own record."
            ),
        "the upgrade deleted everything after the stale review section instead of stopping at its \
         own end: {text}"
    );
}

const GOAL_LOOP_DRIVER: &str = r#"import plugin from "./goal-loop.mjs";

const realSetTimeout = globalThis.setTimeout;
const realClearTimeout = globalThis.clearTimeout;
const held = [];
globalThis.setTimeout = (callback, ms, ...rest) => {
  if (ms >= 60000) {
    const timer = { callback, ms, live: true };
    held.push(timer);
    return timer;
  }
  return realSetTimeout(callback, ms, ...rest);
};
globalThis.clearTimeout = (timer) => {
  if (timer && typeof timer === "object" && "live" in timer) timer.live = false;
  else realClearTimeout(timer);
};
const live = () => held.filter((timer) => timer.live);
const settle = () => new Promise((resolve) => realSetTimeout(resolve, 60));
const fire = async () => {
  const due = live();
  for (const timer of due) timer.live = false;
  for (const timer of due) timer.callback();
  await settle();
};
const until = async (predicate, what) => {
  const deadline = Date.now() + 4000;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => realSetTimeout(resolve, 5));
  }
};

const makeStream = () => {
  const queue = [];
  const waiters = [];
  const stream = async function* () {
    while (true) {
      if (queue.length) {
        yield queue.shift();
        continue;
      }
      await new Promise((resolve) => waiters.push(resolve));
    }
  };
  return {
    push: (event) => {
      queue.push(event);
      const waiter = waiters.shift();
      if (waiter) waiter();
    },
    stream: stream(),
  };
};

const world = async ({ answers = [], directories = ["/tmp/goal-a"] }) => {
  const prompts = [];
  const synthetics = [];
  const generated = [];
  const storage = new Map();
  const hooks = { afterGet: null };
  const instances = [];
  for (const directory of directories) {
    const events = makeStream();
    const commands = {};
    const ctx = {
      location: { directory },
      storage: {
        get: async (key) => {
          const value = storage.get(key);
          const hook = hooks.afterGet;
          if (hook) {
            hooks.afterGet = null;
            await hook(key);
          }
          return value;
        },
        set: async (key, value) => {
          storage.set(key, value);
        },
        remove: async (key) => {
          storage.delete(key);
        },
        scan: async ({ prefix }) => ({
          entries: [...storage].filter(([key]) => key.startsWith(prefix)).map(([key, value]) => ({ key, value })),
        }),
      },
      session: {
        prompt: async (input) => {
          prompts.push(input);
        },
        synthetic: async (input) => {
          if (typeof input.description !== "string" || input.description.trim() === "") {
            console.error("a synthetic message went out with no description: " + JSON.stringify(input));
            process.exit(3);
          }
          synthetics.push(input);
        },
        generate: async (input) => {
          generated.push(input);
          const answer = answers.shift();
          if (answer && answer.delay) await new Promise((resolve) => realSetTimeout(resolve, answer.delay));
          if (answer && answer.throws) throw new Error(answer.throws);
          return answer;
        },
      },
      command: {
        transform: async (callback) => {
          callback({
            add: (definition) => {
              commands[definition.name] = definition;
            },
          });
        },
      },
      event: { subscribe: () => events.stream },
    };
    const teardown = await plugin.setup(ctx);
    instances.push({ directory, events, commands, teardown });
  }
  const turn = () => {
    for (const instance of instances) {
      instance.events.push({ type: "session.execution.succeeded", data: { sessionID: "ses_goal" } });
    }
  };
  const run = (index, name, text) =>
    instances[index].commands[name].execute({ sessionID: "ses_goal", prompt: { text }, delivery: "queue" });
  return { prompts, synthetics, generated, storage, instances, turn, run, hooks };
};

const summary = (w) => ({
  prompts: w.prompts.map((entry) => entry.text),
  said: w.synthetics.map((entry) => entry.text),
  resumes: w.synthetics.map((entry) => entry.resume),
  described: w.synthetics.map((entry) => entry.description),
  generated: w.generated.length,
  asked: w.generated.map((entry) => entry.prompt),
  goal: w.storage.get("goal-loop:goal:ses_goal") ?? null,
  loop: w.storage.get("goal-loop:loop:ses_goal") ?? null,
  timers: live().map((timer) => timer.ms),
});

const silentTurns = async (w, count) => {
  for (let i = 0; i < count; i++) {
    const before = w.prompts.length;
    w.turn();
    await until(() => w.prompts.length > before, `silent turn ${i + 1}`);
  }
};

const reachJudge = async (w, count) => {
  await silentTurns(w, 2);
  w.turn();
  await until(() => w.generated.length >= count, `judgement ${count}`);
};

const turnEnds = async (w, count) => {
  await reachJudge(w, count);
  await settle();
};

const cases = {};

{
  const w = await world({ answers: [{ text: "NOT MET\nthe tests still fail" }] });
  await w.run(0, "goal", "all tests pass");
  await until(() => w.prompts.length >= 1, "unmet: the first prompt");
  await turnEnds(w, 1);
  cases.unmet = summary(w);
}
{
  const w = await world({ answers: [{ text: "MET\nthe suite is green" }] });
  await w.run(0, "goal", "all tests pass");
  await turnEnds(w, 1);
  cases.met = summary(w);
  w.turn();
  await settle();
  cases.met_after = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }, { text: "NOT MET\nstill no" }, { text: "NOT MET\nnever" }] });
  await w.run(0, "goal", "--max 2 all tests pass");
  await silentTurns(w, 1);
  const first = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cap: the judgement");
  await settle();
  cases.cap = { first, second: summary(w) };
  w.turn();
  await settle();
  cases.cap_after = summary(w);
}
for (const [name, answer] of [
  ["maybe", { text: "maybe, hard to say" }],
  ["empty", { text: "" }],
  ["shapeless", { unexpected: true }],
  ["thrown", { throws: "the model was unavailable" }],
  ["mentions", { text: "The condition is not met, so: MET is wrong" }],
]) {
  const w = await world({ answers: [answer] });
  await w.run(0, "goal", "all tests pass");
  await turnEnds(w, 1);
  cases[`unreadable_${name}`] = summary(w);
  w.turn();
  await settle();
  cases[`unreadable_${name}_after`] = summary(w);
}
{
  const w = await world({ answers: [{ data: { text: "MET\nread from the wire shape" } }] });
  await w.run(0, "goal", "all tests pass");
  await turnEnds(w, 1);
  cases.wire_shape = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run(0, "goal", "");
  cases.status_none = summary(w);
  await w.run(0, "goal", "all tests pass");
  await turnEnds(w, 1);
  await w.run(0, "goal", "");
  cases.status_running = summary(w);
  await w.run(0, "goal", "clear");
  cases.cleared = summary(w);
  w.turn();
  await settle();
  cases.cleared_after = summary(w);
  await w.run(0, "goal", "--max 0 all tests pass");
  cases.refused_max = summary(w);
}
{
  const w = await world({ answers: [{ text: "MET\nyes" }], directories: ["/tmp/goal-a", "/tmp/goal-b"] });
  await w.run(0, "goal", "all tests pass");
  await turnEnds(w, 1);
  cases.foreign = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }], directories: ["/tmp/goal-b", "/tmp/goal-a", "/tmp/goal-c"] });
  await w.run(1, "goal", "all tests pass");
  await turnEnds(w, 1);
  cases.owner_second = summary(w);
}
{
  const w = await world({});
  await w.run(0, "loop", "5m check the build");
  cases.interval_first = summary(w);
  await fire();
  cases.interval_second = summary(w);
  await fire();
  cases.interval_third = summary(w);
  await w.run(0, "loop", "stop");
  cases.interval_stopped = summary(w);
  await fire();
  cases.interval_after = summary(w);
}
{
  const w = await world({});
  await w.run(0, "loop", "poll the queue");
  cases.paced_first = summary(w);
  w.turn();
  await until(() => w.prompts.length >= 2, "paced: the second prompt");
  await settle();
  cases.paced_second = summary(w);
  await w.run(0, "loop", "stop");
  w.turn();
  await settle();
  cases.paced_after = summary(w);
  await w.run(0, "loop", "stop");
  cases.paced_stop_again = summary(w);
}
{
  const w = await world({});
  await w.run(0, "loop", "--max 2 poll the queue");
  w.turn();
  await until(() => w.prompts.length >= 2, "capped: the second prompt");
  w.turn();
  await settle();
  cases.loop_capped = summary(w);
}
for (const [name, text] of [["short", "5s check"], ["long", "25d check"], ["bare", "5m"], ["nothing", ""], ["capless", "--max x poll"]]) {
  const w = await world({});
  await w.run(0, "loop", text);
  cases[`loop_refused_${name}`] = summary(w);
}
{
  const w = await world({});
  await w.run(0, "loop", "5m check the build");
  await w.instances[0].teardown();
  await fire();
  cases.torn_down = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno", delay: 200 }] });
  await w.run(0, "goal", "all tests pass");
  await reachJudge(w, 1);
  await w.run(0, "goal", "clear");
  await new Promise((resolve) => realSetTimeout(resolve, 400));
  cases.race_clear = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno", delay: 200 }] });
  await w.run(0, "goal", "the first condition");
  await reachJudge(w, 1);
  await w.run(0, "goal", "the second condition");
  await new Promise((resolve) => realSetTimeout(resolve, 400));
  cases.race_replace = summary(w);
}
{
  const w = await world({});
  await w.run(0, "loop", "5m check the build");
  w.hooks.afterGet = async (key) => {
    if (key.startsWith("goal-loop:loop:")) await w.run(0, "loop", "stop");
  };
  await fire();
  cases.race_loop = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run(0, "goal", "all tests pass");
  await silentTurns(w, 1);
  cases.cadence_first = summary(w);
  await silentTurns(w, 1);
  cases.cadence_second = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cadence: the third turn is judged");
  await settle();
  cases.cadence_third = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run(0, "goal", "--max 2 all tests pass");
  await silentTurns(w, 1);
  cases.capturn_before = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cap turn: the judgement");
  await settle();
  cases.capturn_at = summary(w);
}
{
  const w = await world({});
  cases.commands = Object.keys(w.instances[0].commands);
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

fn run_goal_loop_driver() -> serde_json::Value {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("goal-loop.mjs"),
        crate::setup::plugin::GOAL_LOOP_SOURCE,
    )
    .expect("the goal loop plugin writes");
    let driver = root.path().join("drive-goal-loop.mjs");
    std::fs::write(&driver, GOAL_LOOP_DRIVER).expect("the driver writes");
    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the generic goal and loop plugin, which is JavaScript, and `node` \
             could not be run ({error}). Install Node and re-run; it is not skipped."
        ),
    };
    assert!(
        ran.status.success(),
        "the goal loop plugin would not load or the driver failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

fn said(case: &serde_json::Value) -> String {
    case["said"]
        .as_array()
        .expect("what was said")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

fn shown(case: &serde_json::Value) -> String {
    case["described"]
        .as_array()
        .expect("what was shown")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_stopped(case: &serde_json::Value, path: &str, reason: &str) -> bool {
    case[path]["stopped"]["reason"]
        .as_str()
        .is_some_and(|text| text.contains(reason))
}

#[test]
fn the_generic_goal_continues_on_an_unmet_judgement_and_stops_on_a_met_one() {
    let cases = run_goal_loop_driver();

    let unmet = &cases["unmet"];
    assert_eq!(unmet["prompts"].as_array().expect("prompts").len(), 4);
    assert!(
        unmet["prompts"][3]
            .as_str()
            .is_some_and(|text| text.contains("all tests pass") && text.contains("still fail")),
        "the continuation did not carry the condition and the reason: {unmet}"
    );
    assert_eq!(unmet["goal"]["stopped"], serde_json::Value::Null);
    assert_eq!(unmet["goal"]["turns"], 3);
    assert!(
        unmet["asked"][0]
            .as_str()
            .is_some_and(|text| text.contains("all tests pass") && text.contains("NOT MET")),
        "the evaluator was not asked about the condition: {unmet}"
    );

    let met = &cases["met"];
    assert_eq!(met["prompts"].as_array().expect("prompts").len(), 3);
    assert!(is_stopped(met, "goal", "judged met"), "{met}");
    assert!(said(met).contains("Goal met"), "{met}");
    assert_eq!(met["resumes"], serde_json::json!([false]));
    assert_eq!(
        cases["met_after"]["generated"], 1,
        "a goal that was met was judged again on the next turn"
    );

    let wire = &cases["wire_shape"];
    assert!(
        is_stopped(wire, "goal", "judged met"),
        "an answer shaped {{ data: {{ text }} }} was not read: {wire}"
    );
}

#[test]
fn the_generic_goal_stops_at_its_turn_cap() {
    let cases = run_goal_loop_driver();
    let cap = &cases["cap"];
    assert_eq!(
        cap["first"]["prompts"].as_array().expect("prompts").len(),
        2
    );
    assert_eq!(
        cap["second"]["prompts"].as_array().expect("prompts").len(),
        2
    );
    assert!(is_stopped(&cap["second"], "goal", "turn cap (2)"), "{cap}");
    assert!(said(&cap["second"]).contains("Goal stopped"), "{cap}");
    assert!(
        shown(&cap["second"]).contains("turn cap (2)"),
        "a goal stop's reason was not in the description OpenCode shows: {cap}"
    );
    assert_eq!(cases["cap_after"]["generated"], 1);
    assert_eq!(
        cases["refused_max"]["goal"]["stopped"]["reason"],
        "the operator cleared the goal"
    );
    assert!(said(&cases["refused_max"]).contains("--max takes a whole number"));
}

#[test]
fn the_generic_goal_stops_on_an_evaluator_answer_it_cannot_read() {
    let cases = run_goal_loop_driver();
    for name in ["maybe", "empty", "shapeless", "thrown", "mentions"] {
        let case = &cases[format!("unreadable_{name}")];
        assert_eq!(
            case["prompts"].as_array().expect("prompts").len(),
            3,
            "{name}: an unreadable answer continued the goal: {case}"
        );
        assert!(
            is_stopped(case, "goal", "could not be read"),
            "{name}: the goal did not stop on an unreadable answer: {case}"
        );
        assert_eq!(case["resumes"], serde_json::json!([false]), "{name}");
        assert_eq!(
            cases[format!("unreadable_{name}_after")]["generated"],
            1,
            "{name}: a stopped goal was judged again"
        );
    }
}

#[test]
fn a_stop_or_a_replacement_that_lands_mid_judgement_is_not_overwritten() {
    let cases = run_goal_loop_driver();
    let count = |case: &serde_json::Value| case["prompts"].as_array().expect("prompts").len();

    let clear = &cases["race_clear"];
    assert_eq!(count(clear), 3, "a cleared goal was continued: {clear}");
    assert_eq!(
        clear["goal"]["stopped"]["reason"], "the operator cleared the goal",
        "the judgement in flight overwrote the clear: {clear}"
    );
    assert_eq!(clear["goal"]["turns"], 2, "{clear}");

    let replace = &cases["race_replace"];
    assert_eq!(count(replace), 4, "{replace}");
    assert!(
        replace["prompts"][3]
            .as_str()
            .is_some_and(|text| text.contains("the second condition")),
        "{replace}"
    );
    assert_eq!(replace["goal"]["condition"], "the second condition");
    assert_eq!(
        replace["goal"]["turns"], 0,
        "the old judgement was counted against the new goal: {replace}"
    );
    assert_eq!(
        replace["goal"]["judged"],
        serde_json::Value::Null,
        "{replace}"
    );
    assert_eq!(
        replace["goal"]["stopped"],
        serde_json::Value::Null,
        "{replace}"
    );

    let looped = &cases["race_loop"];
    assert_eq!(
        count(looped),
        1,
        "a tick that raced `/loop stop` enqueued: {looped}"
    );
    assert!(is_stopped(looped, "loop", "operator"), "{looped}");
    assert_eq!(looped["loop"]["runs"], 1, "{looped}");
}

#[test]
fn the_generic_goal_reports_clears_and_is_owned_by_its_location() {
    let cases = run_goal_loop_driver();
    assert!(said(&cases["status_none"]).contains("No goal is set"));
    let running = said(&cases["status_running"]);
    assert!(
        running.contains("Goal: all tests pass")
            && running.contains("Last judgement")
            && running.contains("not a check"),
        "{running}"
    );
    assert!(
        shown(&cases["status_running"]).contains("Goal: all tests pass"),
        "bare `/goal` did not put the goal in the description OpenCode shows"
    );
    assert!(said(&cases["cleared"]).contains("Goal cleared"));
    assert_eq!(cases["cleared_after"]["generated"], 1);
    assert_eq!(
        cases["foreign"]["generated"], 1,
        "an instance for another location judged a goal it does not own"
    );
    assert_eq!(
        cases["foreign"]["prompts"]
            .as_array()
            .expect("prompts")
            .len(),
        3
    );

    let second = &cases["owner_second"];
    assert_eq!(
        second["generated"], 1,
        "the owner was locked out of its own turn end by another location: {second}"
    );
    assert_eq!(
        second["prompts"].as_array().expect("prompts").len(),
        4,
        "{second}"
    );
}

/// The evaluator is asked on every third turn, not on every turn: the turns
/// between continue the goal without a judge call.
#[test]
fn the_generic_goal_judges_every_third_turn_and_continues_the_others_unasked() {
    let cases = run_goal_loop_driver();
    let count = |case: &serde_json::Value| case["prompts"].as_array().expect("prompts").len();
    for name in ["cadence_first", "cadence_second"] {
        let case = &cases[name];
        assert_eq!(case["generated"], 0, "{name}: the judge was called: {case}");
        let last = case["prompts"].as_array().expect("prompts").last().cloned();
        assert!(
            last.as_ref()
                .and_then(serde_json::Value::as_str)
                .is_some_and(|text| text.contains("all tests pass")
                    && text.contains("Keep working toward it")
                    && !text.contains("judged not yet met")),
            "{name}: the unjudged turn did not continue the goal: {case}"
        );
    }
    assert_eq!(count(&cases["cadence_second"]), 3);
    let third = &cases["cadence_third"];
    assert_eq!(third["generated"], 1, "{third}");
    assert_eq!(count(third), 4, "{third}");
}

/// The turn that reaches the cap is judged even when it is not a third turn.
#[test]
fn the_generic_goal_judges_the_turn_that_reaches_its_cap() {
    let cases = run_goal_loop_driver();
    assert_eq!(cases["capturn_before"]["generated"], 0);
    let at = &cases["capturn_at"];
    assert_eq!(at["generated"], 1, "the cap turn was not judged: {at}");
    assert!(is_stopped(at, "goal", "turn cap (2)"), "{at}");
}

#[test]
fn the_generic_loop_repeats_on_an_interval_and_after_each_turn_and_stops() {
    let cases = run_goal_loop_driver();
    let count = |case: &serde_json::Value| case["prompts"].as_array().expect("prompts").len();

    assert_eq!(count(&cases["interval_first"]), 1);
    assert_eq!(
        cases["interval_first"]["timers"],
        serde_json::json!([300_000])
    );
    assert_eq!(cases["interval_first"]["prompts"][0], "check the build");
    assert_eq!(count(&cases["interval_second"]), 2);
    assert_eq!(count(&cases["interval_third"]), 3);
    assert_eq!(cases["interval_stopped"]["timers"], serde_json::json!([]));
    assert_eq!(
        count(&cases["interval_after"]),
        3,
        "a stopped interval loop fired"
    );
    assert!(said(&cases["interval_stopped"]).contains("Loop stopped"));
    assert_eq!(
        count(&cases["torn_down"]),
        1,
        "a torn-down plugin still fired its timer"
    );

    assert_eq!(count(&cases["paced_first"]), 1);
    assert_eq!(cases["paced_first"]["timers"], serde_json::json!([]));
    assert_eq!(
        count(&cases["paced_second"]),
        2,
        "a turn end did not repeat the self-paced prompt"
    );
    assert_eq!(
        count(&cases["paced_after"]),
        2,
        "a stopped self-paced loop repeated"
    );
    assert!(said(&cases["paced_stop_again"]).contains("No running loop"));
    assert_eq!(count(&cases["loop_capped"]), 2);
    assert!(is_stopped(&cases["loop_capped"], "loop", "run cap (2)"));

    for name in ["short", "long", "bare", "nothing", "capless"] {
        let case = &cases[format!("loop_refused_{name}")];
        assert_eq!(count(case), 0, "{name}: a refused loop enqueued: {case}");
        assert!(said(case).contains("Loop not started"), "{name}: {case}");
        assert_eq!(case["resumes"], serde_json::json!([false]), "{name}");
    }

    assert_eq!(cases["commands"], serde_json::json!(["goal", "loop"]));
}

#[test]
fn the_generic_plugin_holds_no_estigia_path_and_runs_no_estigia_command() {
    let source = crate::setup::plugin::GOAL_LOOP_SOURCE;
    let without_marker: String = source
        .lines()
        .filter(|line| !line.contains(crate::setup::plugin::GOAL_LOOP_MARKER))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !without_marker.to_lowercase().contains("estigia"),
        "the generic plugin names Estigia beyond its marker"
    );
    for forbidden in [
        "Bun",
        "child_process",
        "spawn",
        "execFile",
        "execSync",
        "$`",
        "--session-id",
        "process.env",
    ] {
        assert!(
            !source.contains(forbidden),
            "the generic plugin can run a command or reach the environment: {forbidden}"
        );
    }
    assert!(
        !source.contains(crate::setup::plugin::LOOP_MARKER),
        "the generic plugin would be taken for the Estigia loop plugin"
    );
    let loop_source = crate::setup::plugin::loop_source(std::path::Path::new("/opt/estigia"));
    assert!(
        !loop_source.contains(crate::setup::plugin::GOAL_LOOP_MARKER),
        "the Estigia loop plugin would be taken for the generic plugin"
    );
}

fn goal_loop_on() -> Config {
    Config {
        goal_loop_plugin: crate::config::LoopPlugin::On,
        ..Config::default()
    }
}

#[test]
fn opencode_installs_the_generic_plugin_only_when_its_own_setting_is_on() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    let generic = loop_file.with_file_name(plugin::GOAL_LOOP_FILE);

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    assert!(loop_file.is_file());
    assert!(
        !generic.exists(),
        "the loop plugin row wrote the generic plugin"
    );

    setup(adapter, &goal_loop_on(), &options).expect("the generic install succeeds");
    assert!(generic.is_file(), "{} was not written", generic.display());
    assert!(
        !loop_file.exists(),
        "the generic row wrote or kept the Estigia loop plugin"
    );
    assert_eq!(
        fs::read_to_string(&generic).expect("the generic plugin reads"),
        plugin::GOAL_LOOP_SOURCE
    );

    setup(adapter, &Config::default(), &options).expect("switching it off succeeds");
    assert!(!generic.exists(), "off left the generic plugin it wrote");

    setup(adapter, &goal_loop_on(), &options).expect("the second install succeeds");
    assert!(generic.is_file());
    uninstall(adapter, &options).expect("the plain uninstall succeeds");
    assert!(
        !generic.exists(),
        "a plain uninstall left the generic plugin behind"
    );
}

#[test]
fn a_goal_loop_plugin_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let generic = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path")
        .with_file_name(plugin::GOAL_LOOP_FILE);
    fs::create_dir_all(generic.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default { id: 'theirs' };\n";
    fs::write(&generic, theirs).expect("write their plugin");

    setup(adapter, &goal_loop_on(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&generic).expect("survives"), theirs);
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&generic).expect("survives"), theirs);
}

#[test]
fn a_loop_plugin_from_an_earlier_build_is_replaced_by_the_renamed_one() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    fs::create_dir_all(loop_file.parent().expect("a parent")).expect("create the dir");
    let earlier = format!(
        "// {}\nexport default {{ id: \"loop\", name: \"loop\", other: \"goal\" }};\n",
        plugin::LOOP_MARKER
    );
    fs::write(&loop_file, &earlier).expect("write the earlier build's plugin");

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    let now = fs::read_to_string(&loop_file).expect("the plugin reads");
    assert_ne!(now, earlier, "the earlier build's plugin was kept");
    assert!(now.contains("name: \"flow-loop\""));
    assert!(now.contains("name: \"flow-status\""));
    assert!(!now.contains("name: \"loop\"") && !now.contains("name: \"goal\""));
}

fn legacy_loop_file(options: &SetupOptions) -> PathBuf {
    resolve_paths(agent("opencode"), options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path")
        .with_file_name(plugin::LEGACY_LOOP_FILE)
}

fn write_legacy_loop(options: &SetupOptions, text: &str) -> PathBuf {
    let legacy = legacy_loop_file(options);
    fs::create_dir_all(legacy.parent().expect("a parent")).expect("create the dir");
    fs::write(&legacy, text).expect("write the legacy plugin");
    legacy
}

#[test]
fn setup_over_a_legacy_loop_file_leaves_only_the_renamed_one() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path");
    let legacy = write_legacy_loop(&options, &format!("// {}\n", plugin::LOOP_MARKER));

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert!(
        loop_file.is_file(),
        "{} was not written",
        loop_file.display()
    );
    assert_eq!(
        loop_file.file_name().and_then(|n| n.to_str()),
        Some("flow-loop.js")
    );
    assert!(
        !legacy.exists(),
        "setup left the legacy loop plugin beside the renamed one"
    );
}

#[test]
fn setup_with_the_loop_off_removes_a_legacy_loop_file() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let legacy = write_legacy_loop(&options, &format!("// {}\n", plugin::LOOP_MARKER));

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        !legacy.exists(),
        "turning the loop off left the legacy file"
    );
}

#[test]
fn uninstall_removes_a_stray_legacy_loop_file() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let legacy = write_legacy_loop(&options, &format!("// {}\n", plugin::LOOP_MARKER));

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!legacy.exists(), "uninstall left the legacy loop plugin");
}

#[test]
fn a_legacy_named_file_estigia_did_not_write_is_left_alone() {
    let (_home, options) = sandbox();
    let adapter = agent("opencode");
    let theirs = "export default { id: 'theirs' };\n";
    let legacy = write_legacy_loop(&options, theirs);

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&legacy).expect("survives"), theirs);
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&legacy).expect("survives"), theirs);
}

fn codex_files(options: &SetupOptions) -> (PathBuf, PathBuf) {
    let paths = resolve_paths(agent("codex"), options).expect("paths resolve");
    (
        paths.hooks.expect("codex is gated"),
        paths.mcp_config.expect("codex reads an MCP config"),
    )
}

fn toml_key(key: &str) -> String {
    toml_edit::Key::new(key).display_repr().into_owned()
}

fn trust_of(options: &SetupOptions) -> Vec<(String, Option<String>)> {
    let (hooks, config) = codex_files(options);
    let text = fs::read_to_string(&config).unwrap_or_default();
    codex_trust::owned(&hooks, &fs::read_to_string(&hooks).expect("hooks written"))
        .into_iter()
        .map(|entry| {
            let held = codex_trust::state_of(&text, &entry.key).trusted_hash;
            assert!(
                held.as_deref().is_none_or(|held| held == entry.hash),
                "trust names another hash than the hook's own"
            );
            (entry.key, held)
        })
        .collect()
}

#[test]
fn codex_setup_trusts_the_hook_it_registers_because_codex_skips_an_untrusted_one() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    let trust = trust_of(&options);
    assert_eq!(trust.len(), 1, "{trust:?}");
    assert!(
        trust[0].0.ends_with("hooks.json:pre_tool_use:0:0"),
        "{trust:?}"
    );
    assert!(
        trust[0]
            .1
            .as_deref()
            .is_some_and(|hash| hash.starts_with("sha256:")),
        "the registered hook carries no trust: {trust:?}"
    );
    assert!(
        skipped_by_agent(adapter, &options).is_none(),
        "a freshly set up gate is reported as one Codex skips"
    );
}

#[test]
fn codex_trust_is_written_once_and_left_alone_by_the_next_setup() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let first = fs::read_to_string(&config).expect("read");
    let again = setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(again.changed_files(), 0, "the second setup rewrote a file");
    assert_eq!(fs::read_to_string(&config).expect("read"), first);
    assert_eq!(first.matches("[hooks.state.").count(), 1, "{first}");
}

fn trusted_events(options: &SetupOptions) -> Vec<String> {
    trust_of(options)
        .into_iter()
        .map(|(key, held)| {
            assert!(held.is_some(), "{key} carries no trust");
            key.rsplit_once("hooks.json:")
                .expect("a key names its hooks file")
                .1
                .to_owned()
        })
        .collect()
}

#[test]
fn codex_setup_with_the_loop_on_registers_and_trusts_exactly_stop_and_session_start_beside_the_gate()
 {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    setup(adapter, &loop_on(), &options).expect("setup succeeds");

    let mut events = trusted_events(&options);
    events.sort();
    assert_eq!(
        events,
        ["pre_tool_use:0:0", "session_start:0:0", "stop:0:0"],
        "the loop's two entries and the gate are the only hooks trusted"
    );
    assert!(
        skipped_by_agent(adapter, &options).is_none(),
        "a freshly set up loop is reported as one Codex skips"
    );

    let first = fs::read_to_string(&config).expect("read");
    let again = setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_eq!(again.changed_files(), 0, "the second setup rewrote a file");
    assert_eq!(fs::read_to_string(&config).expect("read"), first);
    assert_eq!(first.matches("[hooks.state.").count(), 3, "{first}");
}

#[test]
fn codex_setup_with_the_loop_off_takes_back_both_entries_and_their_trust() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let without = (
        fs::read_to_string(&hooks).expect("read"),
        fs::read_to_string(&config).expect("read"),
    );

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_ne!(fs::read_to_string(&hooks).expect("read"), without.0);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    assert_eq!(fs::read_to_string(&hooks).expect("read"), without.0);
    assert_eq!(
        fs::read_to_string(&config).expect("read"),
        without.1,
        "turning the loop off left trust for a hook that is gone"
    );
}

#[test]
fn codex_loop_entries_and_trust_beside_foreign_ones_leave_the_foreign_ones_alone() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    fs::create_dir_all(hooks.parent().expect("a parent")).expect("create the dir");
    let theirs_hooks = serde_json::json!({
        "hooks": {
            "Stop": [{"hooks": [{"type": "command", "command": "theirs stop"}]}],
            "SessionStart": [{"hooks": [{"type": "command", "command": "theirs start"}]}],
        }
    });
    fs::write(&hooks, theirs_hooks.to_string()).expect("write their hooks");
    let theirs_trust = format!(
        "[hooks.state.{}]\ntrusted_hash = \"sha256:theirs-stop\"\n\n\
         [hooks.state.{}]\ntrusted_hash = \"sha256:theirs-start\"\n",
        toml_key(&format!("{}:stop:0:0", hooks.display())),
        toml_key(&format!("{}:session_start:0:0", hooks.display())),
    );
    fs::write(&config, &theirs_trust).expect("write their trust");

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    for (event, theirs) in [("Stop", "theirs stop"), ("SessionStart", "theirs start")] {
        let commands = hook_commands(&written, event);
        assert_eq!(commands.len(), 2, "{written}");
        assert_eq!(commands[0], theirs, "{written}");
    }

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    assert_eq!(hook_commands(&written, "Stop"), ["theirs stop"]);
    assert_eq!(hook_commands(&written, "SessionStart"), ["theirs start"]);
    let after = fs::read_to_string(&config).expect("read");
    for kept in ["sha256:theirs-stop", "sha256:theirs-start"] {
        assert!(after.contains(kept), "their trust `{kept}` went:\n{after}");
    }

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    uninstall(adapter, &options).expect("uninstall succeeds");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    assert_eq!(hook_commands(&written, "Stop"), ["theirs stop"]);
    assert_eq!(hook_commands(&written, "SessionStart"), ["theirs start"]);
    assert_eq!(
        fs::read_to_string(&config).expect("read"),
        theirs_trust,
        "uninstall did not return their trust as it was"
    );
}

#[test]
fn codex_setup_keeps_every_foreign_table_and_the_trust_of_other_hooks() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    let theirs = "# my own note
model = \"gpt\"

[features]
hooks = true

[hooks.state.\"/elsewhere/hooks.json:stop:0:0\"]
trusted_hash = \"sha256:theirs\"
enabled = true

[mcp_servers.leteo]
command = \"leteo\"
";
    fs::write(&config, theirs).expect("write their file");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after = fs::read_to_string(&config).expect("read");
    for kept in [
        "# my own note",
        "hooks = true",
        "[hooks.state.\"/elsewhere/hooks.json:stop:0:0\"]",
        "trusted_hash = \"sha256:theirs\"",
        "[mcp_servers.leteo]",
    ] {
        assert!(after.contains(kept), "setup lost `{kept}`:\n{after}");
    }
    assert_eq!(trust_of(&options).len(), 1);

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&config).expect("read"),
        theirs,
        "uninstall did not return their file as it was"
    );
}

#[test]
fn uninstalling_codex_leaves_no_trust_behind_and_no_file_it_created() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!hooks.exists());
    assert!(
        !config.exists(),
        "{}",
        fs::read_to_string(&config).unwrap_or_default()
    );
}

#[test]
fn a_rewrite_of_the_codex_gate_does_not_move_the_hooks_after_it() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let ours = trust_of(&options)[0].0.clone();

    let mut written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    written["hooks"]["PreToolUse"]
        .as_array_mut()
        .expect("groups")
        .push(serde_json::json!({"hooks": [{"type": "command", "command": "theirs"}]}));
    fs::write(&hooks, written.to_string()).expect("edit the hooks file");
    let theirs = ours.replace(":0:0", ":1:0");
    let held = fs::read_to_string(&config).expect("read");
    fs::write(
        &config,
        format!(
            "{held}\n[hooks.state.{}]\ntrusted_hash = \"sha256:theirs\"\n",
            serde_json::to_string(&theirs).expect("json")
        ),
    )
    .expect("they trust their own hook");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(trust_of(&options)[0].0, ours, "the gate moved");
    let after = fs::read_to_string(&config).expect("read");
    assert!(
        after.contains("sha256:theirs"),
        "their trust went:\n{after}"
    );
    let rewritten: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    assert_eq!(
        rewritten["hooks"]["PreToolUse"][1]["hooks"][0]["command"], "theirs",
        "their hook is no longer where their trust says it is"
    );
}

#[test]
fn trust_written_for_a_hook_that_moved_out_of_its_group_is_taken_back() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    let mut written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&hooks).expect("read")).expect("json");
    let group = written["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_mut()
        .expect("handlers");
    group.insert(
        0,
        serde_json::json!({"type": "command", "command": "theirs"}),
    );
    fs::write(&hooks, written.to_string()).expect("edit the hooks file");
    let before = trust_of(&options);
    assert!(before[0].0.ends_with(":pre_tool_use:0:1"), "{before:?}");
    let held = fs::read_to_string(&config).expect("read");
    fs::write(&config, held.replace(":0:0", ":0:1")).expect("trust where it now sits");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after = trust_of(&options);
    assert!(after[0].0.ends_with(":pre_tool_use:1:0"), "{after:?}");
    let text = fs::read_to_string(&config).expect("read");
    assert_eq!(text.matches("[hooks.state.").count(), 1, "{text}");
    assert!(after[0].1.is_some());
}

#[test]
fn codex_setup_refuses_a_config_it_cannot_add_trust_to_and_writes_nothing() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    let theirs = "hooks = { state = {} }\n";
    fs::write(&config, theirs).expect("write their file");

    assert!(setup(adapter, &Config::default(), &options).is_err());
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
    assert!(
        !hooks.exists(),
        "the hooks file was written before the refusal"
    );
}

#[test]
fn codex_uninstall_refuses_a_config_it_cannot_parse_and_removes_nothing() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let hooks_before = fs::read_to_string(&hooks).expect("hooks written");
    assert!(
        paths.instructions.exists() && paths.skill_root.exists(),
        "setup left an install to take back"
    );

    // TOML Codex cannot read: the trust removal below has no safe edit of it,
    // and the refusal must come before the skill, directive, hooks and plugins
    // are gone — a half-uninstall whose retry fails the same way.
    let theirs = "[hooks\n";
    fs::write(&config, theirs).expect("their unparseable config");

    let error =
        uninstall(adapter, &options).expect_err("uninstall accepted a config it cannot parse");
    assert!(
        error.to_string().contains("is not TOML"),
        "the refusal does not name the unparseable config: {error}"
    );
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
    assert_eq!(
        fs::read_to_string(&hooks).expect("read"),
        hooks_before,
        "the hooks file was removed before the refusal"
    );
    assert!(
        paths.instructions.exists(),
        "the directive was removed before the refusal"
    );
    assert!(
        paths.skill_root.exists(),
        "the skill was removed before the refusal"
    );
}

#[test]
fn a_dry_run_of_codex_setup_writes_no_trust() {
    let (_home, mut options) = sandbox();
    options.dry_run = true;
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("dry run succeeds");
    assert!(!config.exists());
}

#[test]
fn a_gate_codex_would_skip_is_reported_for_each_way_it_is_skipped() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let trusted = fs::read_to_string(&config).expect("read");

    fs::write(
        &config,
        codex_trust::without_trust(&config, &trusted, &[trust_of(&options)[0].0.clone()])
            .expect("the trust is removed"),
    )
    .expect("take the trust out");
    let untrusted = skipped_by_agent(adapter, &options).expect("untrusted is reported");
    assert!(untrusted.cleared_by_setup);
    assert!(
        untrusted.detail.contains("holds no trust"),
        "{}",
        untrusted.detail
    );

    fs::write(&config, trusted.replace("sha256:", "sha256:0")).expect("drift the hash");
    let drifted = skipped_by_agent(adapter, &options).expect("drift is reported");
    assert!(
        drifted.detail.contains("another hash"),
        "{}",
        drifted.detail
    );

    fs::write(
        &config,
        trusted.replace("\n\n[mcp", "\nenabled = false\n\n[mcp"),
    )
    .expect("disable it");
    let disabled = skipped_by_agent(adapter, &options).expect("disabled is reported");
    assert!(!disabled.cleared_by_setup);

    fs::write(&config, &trusted).expect("restore");
    assert!(skipped_by_agent(adapter, &options).is_none());

    let mut moved = fs::read_to_string(&hooks).expect("read");
    moved = moved.replace("\"timeout\": 10", "\"timeout\": 11");
    fs::write(&hooks, moved).expect("edit the hook after it was trusted");
    let edited = skipped_by_agent(adapter, &options).expect("an edited hook is reported");
    assert!(edited.detail.contains("another hash"), "{}", edited.detail);

    assert!(
        skipped_by_agent(agent("claude-code"), &options).is_none(),
        "an agent with no trust mechanism was reported as skipped"
    );
}

#[test]
fn codex_setup_and_uninstall_leave_the_trust_of_a_handler_that_only_names_an_event() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    fs::write(
        &hooks,
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/usr/bin/githook stop"}]}]}}"#,
    )
    .expect("their hooks");
    let theirs = format!(
        "[hooks.state.{}]\ntrusted_hash = \"sha256:theirs\"\n",
        serde_json::to_string(&format!("{}:stop:0:0", hooks.display())).expect("json")
    );
    fs::write(&config, &theirs).expect("their trust");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        fs::read_to_string(&config)
            .expect("read")
            .contains("sha256:theirs")
    );
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
}

#[test]
fn an_unreadable_codex_config_is_not_reported_as_holding_no_trust() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    fs::remove_file(&config).expect("remove");
    fs::create_dir(&config).expect("a directory where the file was");
    let skip = skipped_by_agent(adapter, &options).expect("reported");
    assert!(skip.detail.contains("read"), "{}", skip.detail);
    assert!(!skip.detail.contains("holds no trust"), "{}", skip.detail);
    assert!(!skip.cleared_by_setup);
}

fn trust_entry(key: &str, hash: &str) -> codex_trust::Entry {
    codex_trust::Entry {
        key: key.to_owned(),
        hash: hash.to_owned(),
    }
}

fn seed_codex_config(options: &SetupOptions, text: &str) -> PathBuf {
    let (_, config) = codex_files(options);
    fs::create_dir_all(config.parent().expect("a parent")).expect("create the dir");
    fs::write(&config, text).expect("write the seed");
    config
}

fn toml_values(text: &str) -> std::collections::BTreeMap<String, String> {
    fn walk(
        item: &toml_edit::Item,
        prefix: &str,
        out: &mut std::collections::BTreeMap<String, String>,
    ) {
        match item {
            toml_edit::Item::Table(table) => {
                for (key, child) in table.iter() {
                    let path = if prefix.is_empty() {
                        key.to_owned()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(child, &path, out);
                }
            }
            toml_edit::Item::Value(value) => {
                out.insert(prefix.to_owned(), value.to_string().trim().to_owned());
            }
            toml_edit::Item::ArrayOfTables(array) => {
                out.insert(prefix.to_owned(), format!("[{} tables]", array.len()));
            }
            toml_edit::Item::None => {}
        }
    }
    let doc: toml_edit::DocumentMut = text.parse().expect("a TOML document");
    let mut out = std::collections::BTreeMap::new();
    walk(doc.as_item(), "", &mut out);
    out
}

#[test]
fn a_hooks_header_carrying_a_url_and_an_inline_state_is_refused() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    let theirs = "[hooks] # https://x/?a=b\nstate = { }\n";
    seed_codex_config(&options, theirs);

    assert!(
        setup(adapter, &Config::default(), &options).is_err(),
        "setup appended a table beside an inline hooks.state"
    );
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
    assert!(
        !hooks.exists(),
        "the hooks file was written before the refusal"
    );
}

#[test]
fn a_single_quoted_state_key_is_the_inline_state_it_is() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    let theirs = "[hooks]\n'state' = { }\n";
    seed_codex_config(&options, theirs);

    assert!(
        setup(adapter, &Config::default(), &options).is_err(),
        "a single-quoted state was not read as the same inline table"
    );
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
    assert!(!hooks.exists());
}

#[test]
fn a_bracket_in_a_comment_does_not_hide_the_hooks_state_table() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    let theirs = "[hooks.state] # see ]\nother = 1\n";
    seed_codex_config(&options, theirs);

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let after = fs::read_to_string(&config).expect("read");
    after
        .parse::<toml_edit::DocumentMut>()
        .expect("the written config parses");
    assert!(after.contains("other = 1"), "{after}");
    assert_eq!(trust_of(&options).len(), 1, "{after}");
    assert_eq!(after.matches("trusted_hash").count(), 1, "{after}");

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
}

#[test]
fn a_trust_table_written_in_another_spelling_is_not_duplicated() {
    let spellings: [fn(&str) -> String; 2] = [
        |key| format!("[hooks.\"state\".{key}]"),
        |key| format!("[ hooks.state.{key} ]"),
    ];
    for spelling in spellings {
        let (_home, options) = sandbox();
        let adapter = agent("codex");
        let (_, config) = codex_files(&options);
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        let (key, held) = trust_of(&options).pop().expect("one entry");
        assert!(held.is_some(), "{key} carries no trust");

        let quoted = toml_key(&key);
        let first = fs::read_to_string(&config).expect("read");
        let seeded = first.replace(&format!("[hooks.state.{quoted}]"), &spelling(&quoted));
        assert_ne!(
            seeded, first,
            "the standard header was not found to substitute"
        );
        fs::write(&config, &seeded).expect("write the alternate spelling");

        let again = setup(adapter, &Config::default(), &options).expect("setup succeeds");
        assert_eq!(again.changed_files(), 0, "a second setup rewrote {seeded}");
        assert_eq!(fs::read_to_string(&config).expect("read"), seeded);
        assert_eq!(
            trust_of(&options).len(),
            1,
            "the trust was duplicated:\n{seeded}"
        );
    }
}

#[test]
fn a_quoted_dotted_key_is_not_the_hooks_state_table() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (_, config) = codex_files(&options);
    let theirs = "[\"hooks.state\"]\nk = 1\n";
    seed_codex_config(&options, theirs);

    setup(adapter, &Config::default(), &options)
        .expect("a quoted-dotted key is not the table setup refuses");
    let after = fs::read_to_string(&config).expect("read");
    after
        .parse::<toml_edit::DocumentMut>()
        .expect("the written config parses");
    assert!(after.contains("[\"hooks.state\"]"), "{after}");
    assert!(after.contains("k = 1"), "{after}");
    assert_eq!(trust_of(&options).len(), 1, "{after}");
}

#[test]
fn a_foreign_table_the_line_reader_misses_survives_without_trust() {
    let path = Path::new("config.toml");
    let ours = "/h/hooks.json:pre_tool_use:0:0";
    let quoted = serde_json::to_string(ours).expect("json");
    let text = format!(
        "[hooks.state.{quoted}]\ntrusted_hash = \"sha256:ours\"\n\n\
         [other.\"a=b\"]\ntrusted_hash = \"sha256:theirs\"\n"
    );

    let after =
        codex_trust::without_trust(path, &text, &[ours.to_owned()]).expect("the trust is removed");
    assert!(
        after.contains("sha256:theirs") && after.contains("[other.\"a=b\"]"),
        "the foreign header went with ours:\n{after}"
    );
    assert!(!after.contains("sha256:ours"), "{after}");
}

#[test]
fn a_line_inside_a_multiline_string_is_not_a_trust_table() {
    let path = Path::new("config.toml");
    let text = "model = \"gpt\"\nnote = \"\"\"\n[hooks.state.\"k\"]\ntrusted_hash = \"sha256:fake\"\n\"\"\"\n";
    assert_eq!(
        codex_trust::state_of(text, "k").trusted_hash,
        None,
        "a line inside a string was read as a trust table"
    );

    let after = codex_trust::with_trust(path, text, &[], &[trust_entry("k", "sha256:real")])
        .expect("the trust is written");
    assert_eq!(
        codex_trust::state_of(&after, "k").trusted_hash.as_deref(),
        Some("sha256:real")
    );
    after
        .parse::<toml_edit::DocumentMut>()
        .expect("the written config parses");
    assert!(
        after.contains("sha256:fake"),
        "their string value changed:\n{after}"
    );
}

#[test]
fn the_parse_guard_covers_the_mcp_table_edit() {
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let (hooks, config) = codex_files(&options);
    let theirs = "model = \"gpt\"\nnote = \"\"\"\n[mcp_servers.estigia]\n\"\"\"\n";
    seed_codex_config(&options, theirs);

    let refusal = with_preview(|| setup(adapter, &Config::default(), &options));
    assert!(
        refusal.is_err(),
        "setup wrote a config its own parser rejects"
    );
    assert_eq!(fs::read_to_string(&config).expect("read"), theirs);
    assert!(!hooks.exists());
}

#[test]
fn every_seeded_codex_config_parses_after_setup_and_keeps_their_tables() {
    let seeds: &[(&str, &str)] = &[
        (
            "comments and another server",
            "# my own note\nmodel = \"gpt\"\n\n[mcp_servers.leteo]\ncommand = \"leteo\"\n",
        ),
        (
            "a foreign trust and features",
            "# my own note\nmodel = \"gpt\"\n\n[features]\nhooks = true\n\n\
             [hooks.state.\"/elsewhere/hooks.json:stop:0:0\"]\ntrusted_hash = \"sha256:theirs\"\n\
             enabled = true\n\n[mcp_servers.leteo]\ncommand = \"leteo\"\n",
        ),
        (
            "a multiline string that looks like a header",
            "model = \"gpt\"\nnote = \"\"\"\n[not a header]\n\"\"\"\n\n\
             [mcp_servers.leteo]\ncommand = \"leteo\"\n",
        ),
        (
            "a bracket inside a quoted key",
            "model = \"gpt\"\n\n[other.\"weird]name\"]\nx = 1\n",
        ),
        (
            "a quoted dotted key",
            "model = \"gpt\"\n\n[\"hooks.state\"]\nk = 1\n",
        ),
    ];
    for (name, seed) in seeds {
        let (_home, options) = sandbox();
        let adapter = agent("codex");
        let config = seed_codex_config(&options, seed);
        setup(adapter, &Config::default(), &options)
            .unwrap_or_else(|error| panic!("{name}: setup failed: {error}"));

        let after = fs::read_to_string(&config).expect("read");
        after
            .parse::<toml_edit::DocumentMut>()
            .unwrap_or_else(|error| {
                panic!("{name}: the written config does not parse: {error}\n{after}")
            });
        let before = toml_values(seed);
        let now = toml_values(&after);
        for (path, value) in &before {
            assert_eq!(
                now.get(path),
                Some(value),
                "{name}: foreign `{path}` changed or went:\n{after}"
            );
        }
        assert_eq!(
            trust_of(&options).len(),
            1,
            "{name}: the gate was not trusted"
        );

        uninstall(adapter, &options).expect("uninstall succeeds");
        assert_eq!(
            fs::read_to_string(&config).expect("read"),
            *seed,
            "{name}: uninstall did not return the seed"
        );
    }
}

const FROZEN_CRUSH_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "name": "estigia",
        "matcher": "edit|write|multiedit|bash",
        "command": "\"/opt/estigia/bin/estigia\" hook pre-tool-use --agent crush --dialect exit-code",
        "timeout": 10
      }
    ]
  },
  "mcp": {
    "estigia": {
      "type": "stdio",
      "command": "/opt/estigia/bin/estigia",
      "args": [
        "mcp"
      ]
    }
  }
}
"#;

const FROZEN_WINDSURF_HOOKS: &str = r#"{
  "hooks": {
    "pre_write_code": [
      {
        "command": "\"/opt/estigia/bin/estigia\" hook pre-tool-use --agent windsurf --dialect exit-code",
        "powershell": "\"/opt/estigia/bin/estigia\" hook pre-tool-use --agent windsurf --dialect exit-code"
      }
    ],
    "pre_run_command": [
      {
        "command": "\"/opt/estigia/bin/estigia\" hook pre-tool-use --agent windsurf --dialect exit-code",
        "powershell": "\"/opt/estigia/bin/estigia\" hook pre-tool-use --agent windsurf --dialect exit-code"
      }
    ]
  }
}
"#;

pub(crate) fn install_as_an_earlier_build(
    adapter: &AgentAdapter,
    options: &SetupOptions,
) -> SetupResult {
    let frozen = match adapter.slug {
        "crush" => Some(FROZEN_CRUSH_JSON),
        "windsurf" => Some(FROZEN_WINDSURF_HOOKS),
        _ => None,
    };
    let earlier = SetupOptions {
        skip_harness: options.skip_harness || frozen.is_some(),
        ..options.clone()
    };
    let result = install_into_evidenced(adapter, &Config::default(), &earlier, &mut Pending::new())
        .unwrap_or_else(|failure| {
            panic!(
                "an earlier build's {} install: {:#}",
                adapter.slug, failure.error
            )
        });
    if let Some(bytes) = frozen {
        let hooks = resolve_paths(adapter, options)
            .expect("paths resolve")
            .hooks
            .expect("an adapter with a gate has a hooks file");
        fs::create_dir_all(hooks.parent().expect("a parent")).expect("the hooks directory");
        let theirs = fs::read_to_string(&hooks).ok();
        let written = match theirs.as_deref() {
            Some(text) if !text.trim().is_empty() => {
                let mut root: serde_json::Value =
                    serde_json::from_str(render::without_mark(text)).expect("their file is JSON");
                fold_in(
                    &mut root,
                    serde_json::from_str(bytes).expect("frozen bytes are JSON"),
                );
                render::as_written_public(theirs.as_deref(), &root).expect("their file renders")
            }
            _ => bytes.to_owned(),
        };
        fs::write(&hooks, written).expect("the earlier build's hooks");
    }
    result
}

fn fold_in(into: &mut serde_json::Value, from: serde_json::Value) {
    use serde_json::Value;
    match (into, from) {
        (Value::Object(held), Value::Object(added)) => {
            for (key, value) in added {
                match held.get_mut(&key) {
                    Some(slot) => fold_in(slot, value),
                    None => {
                        held.insert(key, value);
                    }
                }
            }
        }
        (Value::Array(held), Value::Array(added)) => held.extend(added),
        (slot, value) => *slot = value,
    }
}

const RETIRED_SLUGS: [&str; 6] = [
    "gemini-cli",
    "continue",
    "cline",
    "crush",
    "windsurf",
    "qwen",
];

#[test]
fn the_retired_table_holds_exactly_the_six_dropped_adapters() {
    let slugs: Vec<&str> = RETIRED_AGENTS.iter().map(|adapter| adapter.slug).collect();
    assert_eq!(slugs, RETIRED_SLUGS, "a retired row was added or dropped");
    for adapter in RETIRED_AGENTS {
        assert!(
            adapter.retired(),
            "{} does not read as retired",
            adapter.slug
        );
        assert!(
            !AGENTS
                .iter()
                .any(|supported| supported.slug == adapter.slug),
            "{} is both supported and retired",
            adapter.slug
        );
    }
    assert!(AGENTS.iter().all(|adapter| !adapter.retired()));
    assert_eq!(
        every_adapter().count(),
        AGENTS.len() + RETIRED_AGENTS.len(),
        "every_adapter skips a table"
    );
}

#[test]
fn every_retired_agent_is_refused_for_installation_and_names_the_way_out() {
    for slug in RETIRED_SLUGS {
        let refusal = find_installable_agent(slug).unwrap_err();
        assert_eq!(refusal.code, "agent-retired", "{slug}");
        assert_eq!(
            refusal.resolution,
            Resolution::run(format!("estigia uninstall {slug}")),
            "{slug} names no way out"
        );
        assert_eq!(
            find_agent(slug).expect("uninstall still finds it").slug,
            slug
        );
    }
    for adapter in AGENTS {
        assert!(
            find_installable_agent(adapter.slug).is_ok(),
            "{}",
            adapter.slug
        );
    }
}

#[test]
fn uninstall_takes_back_what_an_earlier_build_wrote_into_every_retired_agent() {
    for adapter in RETIRED_AGENTS {
        let (_home, options) = sandbox();
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        install_as_an_earlier_build(adapter, &options);

        assert!(is_configured(adapter, &options), "{}", adapter.slug);
        assert_eq!(
            is_gated(adapter, &options),
            adapter.can_gate_tools(),
            "{} was not gated by the earlier install",
            adapter.slug
        );
        assert_eq!(
            exposes_tools(adapter, &options),
            adapter.capabilities.tool_server,
            "{} exposed no tools after the earlier install",
            adapter.slug
        );

        uninstall(adapter, &options).expect("uninstall reaches a retired agent");

        assert!(
            !is_present(adapter, &options),
            "{} still holds something",
            adapter.slug
        );
        let mut files = vec![paths.instructions.clone()];
        files.extend(paths.hooks.clone());
        files.extend(paths.mcp_config.clone());
        for file in files {
            let left = fs::read_to_string(&file).unwrap_or_default();
            assert!(
                !left.to_lowercase().contains("estigia"),
                "{} left Estigia in {}",
                adapter.slug,
                file.display()
            );
        }
    }
}

#[test]
fn uninstall_removes_the_override_an_earlier_build_wrote_for_every_retired_agent() {
    for adapter in RETIRED_AGENTS {
        let (_home, options) = sandbox();
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        install_as_an_earlier_build(adapter, &options);
        let file = crate::skill::agent_override(&paths.skill_root, adapter.slug);
        write_agent_configuration_wholly(&file, adapter.slug, &Config::default())
            .expect("an earlier build's override");
        assert!(file.exists(), "{} wrote no override", adapter.slug);

        uninstall(adapter, &options).expect("uninstall reaches a retired agent");

        assert!(!file.exists(), "{} left {}", adapter.slug, file.display());
    }
}

fn doctor_in(
    home: &tempfile::TempDir,
    options: &SetupOptions,
    paths: &AgentPaths,
) -> Vec<crate::harness::doctor::Check> {
    with_home_env(
        home.path(),
        &home.path().join("AppData").join("Roaming"),
        &home.path().join(".config"),
        || {
            crate::harness::doctor::full(
                Some(&paths.skill_root),
                home.path(),
                &Config::default().tracker,
                options,
            )
        },
    )
}

#[test]
fn doctor_names_the_uninstall_for_every_retired_agent_still_installed() {
    for adapter in RETIRED_AGENTS {
        let (home, options) = sandbox();
        install_as_an_earlier_build(adapter, &options);
        let paths = resolve_paths(adapter, &options).expect("paths resolve");

        let checks = doctor_in(&home, &options, &paths);
        let named: Vec<_> = checks
            .iter()
            .filter(|check| check.name == "retired-agent")
            .collect();
        assert_eq!(named.len(), 1, "{}: {named:?}", adapter.slug);
        assert_eq!(
            named[0].health.resolution(),
            Some(&Resolution::run(format!(
                "estigia uninstall {}",
                adapter.slug
            ))),
            "{}",
            adapter.slug
        );

        uninstall(adapter, &options).expect("uninstall runs");
        let after = doctor_in(&home, &options, &paths);
        assert!(
            after.iter().all(|check| check.name != "retired-agent"),
            "{} is reported after it was taken out",
            adapter.slug
        );
    }
}

#[test]
fn uninstalling_one_agent_keeps_the_state_a_retired_agent_still_reads() {
    let (_home, options) = sandbox();
    setup(agent("claude-code"), &Config::default(), &options).expect("claude-code installs");
    install_as_an_earlier_build(agent("gemini-cli"), &options);
    let runs =
        crate::harness::session::state_root(options.home_dir.as_deref()).expect("a state root");
    fs::create_dir_all(&runs).expect("the state directory");
    let ledger = crate::harness::session::ledger_path(&runs);
    fs::write(&ledger, "{}\n").expect("a ledger");

    uninstall(agent("claude-code"), &options).expect("uninstall runs");
    assert!(is_present(agent("gemini-cli"), &options));

    assert!(
        forget_state(&options).is_empty(),
        "the state went while gemini-cli was still installed"
    );
    assert!(ledger.is_file(), "the ledger went early");
}

#[test]
fn uninstalling_one_agent_keeps_the_skill_a_retired_agent_still_reads() {
    let (_home, options) = sandbox();
    let cursor = agent("cursor");
    let gemini = agent("gemini-cli");
    setup(cursor, &Config::default(), &options).expect("cursor installs");
    install_as_an_earlier_build(gemini, &options);
    let root = resolve_paths(cursor, &options).expect("paths").skill_root;
    assert_eq!(
        root,
        resolve_paths(gemini, &options).expect("paths").skill_root
    );
    assert!(root.is_dir(), "the skill was never installed");

    uninstall(cursor, &options).expect("uninstall runs");

    assert!(
        root.is_dir(),
        "the shared skill went while gemini-cli still read it"
    );
    assert_eq!(
        skill_shared_with(cursor, &options, &Pending::new()).expect("answerable"),
        Some(gemini.display_name)
    );
}

#[test]
fn the_library_setup_refuses_every_retired_agent_and_writes_nothing() {
    for adapter in RETIRED_AGENTS {
        let (home, options) = sandbox();
        let config = Config::default();

        let direct = setup(adapter, &config, &options)
            .expect_err(&format!("setup installed into {}", adapter.slug));
        let evidenced =
            setup_into_evidenced(adapter, &config, &options, &mut Pending::new()).expect_err(
                &format!("the evidenced setup installed into {}", adapter.slug),
            );
        let shared = setup_adapter_into(
            adapter,
            &config,
            &config,
            &options,
            &mut Pending::new(),
            true,
        )
        .expect_err(&format!("the shared entry installed into {}", adapter.slug));

        for error in [&direct, &evidenced.error, &shared.error] {
            let refusal = error
                .downcast_ref::<crate::outcome::Refusal>()
                .unwrap_or_else(|| panic!("{}: not a refusal: {error:#}", adapter.slug));
            assert_eq!(refusal.code, "agent-retired", "{}", adapter.slug);
        }
        assert!(!evidenced.write_attempted && !shared.write_attempted);
        assert!(
            fs::read_dir(home.path())
                .expect("the home exists")
                .next()
                .is_none(),
            "{} was refused after something was written",
            adapter.slug
        );
    }
}

const BINARY_LAUNCHED: [&str; 7] = [
    crate::harness::roles::RESERVED_RISK,
    crate::harness::roles::RESERVED_READBACK,
    crate::harness::roles::RESERVED_READABILITY,
    crate::harness::roles::RESERVED_RELIABILITY,
    crate::harness::roles::RESERVED_LENS,
    crate::harness::roles::RESERVED_REFUTER,
    crate::harness::roles::RESERVED_VALIDATOR,
];

fn rendering_of(name: &str) -> String {
    let (_, seal, template) = crate::harness::roles::RESERVED_REVIEWERS
        .iter()
        .find(|(candidate, _, _)| *candidate == name)
        .expect("a reserved reviewer");
    render_reviewer_agent(template, Config::default().evidence, *seal, name)
}

fn seed_owned_reviewer(paths: &AgentPaths, name: &str, body: &str) -> PathBuf {
    let target = reviewer_target(paths, name).expect("a reviewer path");
    fs::create_dir_all(target.parent().expect("a parent")).expect("the agents directory");
    skill::record::note_created_outside(&paths.skill_root, &target).expect("ownership recorded");
    fs::write(&target, body).expect("the reviewer is written");
    record_reviewer_write(&paths.skill_root, &target, body).expect("the digest is recorded");
    target
}

fn changed(result: &SetupResult, path: &Path) -> Vec<Change> {
    result
        .actions
        .iter()
        .filter(|action| action.path == path)
        .map(|action| action.change)
        .collect()
}

#[test]
fn a_fresh_claude_setup_writes_only_the_two_judges() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let result = setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");

    let mut installed: Vec<String> = fs::read_dir(home.path().join(".claude/agents"))
        .expect("the agents directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with("review-"))
        .collect();
    installed.sort();
    assert_eq!(installed, ["review-blind-sealed.md", "review-blind.md"]);
    for name in BINARY_LAUNCHED {
        let target = reviewer_target(&paths, name).expect("a reviewer path");
        assert!(!owns_reviewer(&paths, &target), "{name} is owned");
        assert!(changed(&result, &target).is_empty(), "{name} was acted on");
    }
}

#[test]
fn setup_retracts_an_owned_unedited_binary_launched_reviewer() {
    for name in BINARY_LAUNCHED {
        let (_home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let target = seed_owned_reviewer(&paths, name, &rendering_of(name));

        let result = setup(adapter, &Config::default(), &options).expect("setup runs again");
        assert!(!target.exists(), "{name} survived setup");
        assert!(!owns_reviewer(&paths, &target), "{name} is still owned");
        assert_eq!(changed(&result, &target), [Change::Remove], "{name}");
    }
}

#[test]
fn setup_retracts_an_older_shipped_rendering_of_a_binary_launched_reviewer() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let older = render_reviewer_agent(
        LENS_RISK_BEFORE_MANDATE,
        Config::default().evidence,
        Seal::Sealed,
        crate::harness::roles::RESERVED_RISK,
    );
    assert!(reviewer_is_historical(&older) && !reviewer_is_static(&older));
    let target = seed_owned_reviewer(&paths, crate::harness::roles::RESERVED_RISK, &older);
    skill::record::forget_outside(&paths.skill_root, &target).expect("digest dropped");
    skill::record::note_created_outside(&paths.skill_root, &target).expect("owned, no digest");

    setup(adapter, &Config::default(), &options).expect("setup runs again");
    assert!(!target.exists());
    assert!(!owns_reviewer(&paths, &target));
}

#[test]
fn setup_keeps_and_relinquishes_an_edited_binary_launched_reviewer() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let name = crate::harness::roles::RESERVED_RISK;
    let target = seed_owned_reviewer(&paths, name, &rendering_of(name));
    let edited = "---\nname: review-risk\ntools: Read\n---\n\nThe operator's own words.\n";
    fs::write(&target, edited).expect("the operator edits it");

    let result = setup(adapter, &Config::default(), &options).expect("setup runs again");
    assert_eq!(fs::read_to_string(&target).expect("kept"), edited);
    assert!(!owns_reviewer(&paths, &target));
    assert_eq!(changed(&result, &target), [Change::Kept]);

    let again = setup(adapter, &Config::default(), &options).expect("a replay");
    assert_eq!(fs::read_to_string(&target).expect("kept"), edited);
    assert!(changed(&again, &target).is_empty());
}

#[test]
fn setup_leaves_an_unowned_binary_launched_reviewer_alone() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let name = crate::harness::roles::RESERVED_RISK;
    let target = reviewer_target(&paths, name).expect("a reviewer path");
    let theirs = rendering_of(name);
    fs::write(&target, &theirs).expect("somebody else's file");

    let result = setup(adapter, &Config::default(), &options).expect("setup does not refuse");
    assert_eq!(fs::read_to_string(&target).expect("untouched"), theirs);
    assert!(!owns_reviewer(&paths, &target));
    assert!(changed(&result, &target).is_empty());
}

#[test]
fn uninstall_retracts_an_owned_unedited_binary_launched_reviewer() {
    for name in BINARY_LAUNCHED {
        let (_home, options) = sandbox();
        let adapter = agent("claude-code");
        setup(adapter, &Config::default(), &options).expect("setup runs");
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let target = seed_owned_reviewer(&paths, name, &rendering_of(name));

        uninstall(adapter, &options).expect("uninstall runs");
        assert!(!target.exists(), "{name} survived uninstall");
    }
}

#[test]
fn uninstall_keeps_an_edited_binary_launched_reviewer() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let name = crate::harness::roles::RESERVED_RISK;
    let target = seed_owned_reviewer(&paths, name, &rendering_of(name));
    let edited = "---\nname: review-risk\ntools: Read\n---\n\nThe operator's own words.\n";
    fs::write(&target, edited).expect("the operator edits it");

    let result = uninstall(adapter, &options).expect("uninstall runs");
    assert_eq!(fs::read_to_string(&target).expect("kept"), edited);
    assert_eq!(changed(&result, &target), [Change::Kept]);
}

#[test]
fn uninstall_leaves_an_unowned_binary_launched_reviewer_alone() {
    let (_home, options) = sandbox();
    let adapter = agent("claude-code");
    setup(adapter, &Config::default(), &options).expect("setup runs");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let name = crate::harness::roles::RESERVED_RISK;
    let target = reviewer_target(&paths, name).expect("a reviewer path");
    let theirs = rendering_of(name);
    fs::write(&target, &theirs).expect("somebody else's file");

    let result = uninstall(adapter, &options).expect("uninstall runs");
    assert_eq!(fs::read_to_string(&target).expect("untouched"), theirs);
    assert!(changed(&result, &target).is_empty());
}

/// The driver for the review relay plugin's `node` test. It stubs the OpenCode 2
/// plugin context and a shell that records what the plugin asked, then prints
/// what the plugin did.
const REVIEW_RELAY_DRIVER: &str = r#"import plugin, { EstigiaReviewRelay } from "./estigia-review-relay.mjs";

const state = { calls: [], answers: {}, breakStdin: false, throwAwait: false };

const makeShell = () => (_strings, ...values) => {
  const rendered = values.map(String);
  const action = rendered.find((value) => value === "prompt" || value === "admit") ?? "";
  const call = { action, values: rendered, cwd: null, stdinBlob: null, stdinFailed: false };
  state.calls.push(call);
  const scripted = state.answers[action] ?? { exitCode: 0, stdout: "", stderr: "" };
  const chain = {
    then: (resolve, reject) =>
      state.throwAwait
        ? reject(new Error("the shell failed"))
        : resolve({ exitCode: scripted.exitCode, stdout: scripted.stdout, stderr: scripted.stderr }),
    cwd(directory) {
      call.cwd = String(directory);
      return chain;
    },
    quiet() {
      return chain;
    },
    nothrow() {
      return chain;
    },
    stdin(blob) {
      if (state.breakStdin) {
        call.stdinFailed = true;
        throw new Error("EPIPE: the relay exited before reading the answer");
      }
      call.stdinBlob = blob;
      return chain;
    },
  };
  return chain;
};

globalThis.Bun = { $: makeShell() };

const registered = [];
const ctx = {
  location: { directory: "/tmp/relay-project" },
  tool: {
    hook: async (name, callback) => {
      registered.push([name, callback]);
    },
  },
};
await plugin.setup(ctx);
const before = registered.find(([name]) => name === "execute.before")?.[1];
const after = registered.find(([name]) => name === "execute.after")?.[1];
if (!before || !after) throw new Error("setup registered no relay hooks");

const out = {};

state.answers = { prompt: { exitCode: 0, stdout: "THE EXACT PROMPT" } };
const task = {
  tool: "task",
  sessionID: "ses_relay",
  input: { subagent_type: "review-risk", prompt: "the orchestrator's own words" },
};
await before(task);
out.prompt_injected = task.input.prompt;
out.prompt_session = state.calls.at(-1).values.includes("ses_relay");

state.answers = { prompt: { exitCode: 0, stdout: "MARKED PROMPT" } };
const marked = {
  tool: "task",
  sessionID: "ses_relay",
  input: { description: "x", prompt: "<!-- estigia:review name=review-lens --> go" },
};
await before(marked);
out.marker_injected = marked.input.prompt;

state.answers = { prompt: { exitCode: 0, stdout: "PROMPT FOR THE ANSWER'S LAUNCH" } };
const prescribed = JSON.parse(process.argv[2]);
const launched = {
  tool: "task",
  sessionID: "ses_relay",
  input: { subagent_type: prescribed.agent, description: "review", prompt: prescribed.prompt },
};
await before(launched);
out.launch_injected = launched.input.prompt;
out.launch_asked = state.calls.at(-1).values.includes("review-reliability");

state.answers = { prompt: { exitCode: 0, stdout: "PROMPT FOR A SUBAGENT TOOL" } };
const viaSubagent = {
  tool: "subagent",
  sessionID: "ses_relay",
  input: { agent: prescribed.agent, prompt: prescribed.prompt },
};
await before(viaSubagent);
out.subagent_injected = viaSubagent.input.prompt;

state.answers = { admit: { exitCode: 0, stdout: "{\"ok\":true}" } };
state.breakStdin = false;
const admitted = {
  tool: "task",
  sessionID: "ses_relay",
  input: { subagent_type: "review-risk" },
  status: "completed",
  result: "RAW REVIEWER ANSWER",
};
await after(admitted);
out.admitted_result = admitted.result;
out.admitted_status = admitted.status ?? null;

state.answers = {
  admit: {
    exitCode: 1,
    stdout: "",
    stderr: JSON.stringify({
      code: "capture-result-stale",
      message: "the review moved after job j1 was launched",
    }),
  },
};
state.breakStdin = true;
const stale = {
  tool: "task",
  sessionID: "ses_relay",
  input: { subagent_type: "review-risk" },
  status: "completed",
  result: "RAW",
};
await after(stale);
out.stale_status = stale.status;
out.stale_error = stale.error;
out.stale_result = stale.result;

state.throwAwait = true;
state.answers = { admit: { exitCode: 0, stdout: "{\"ok\":true}" } };
const thrown = {
  tool: "task",
  sessionID: "ses_relay",
  input: { subagent_type: "review-risk" },
  status: "completed",
  result: "RAW",
};
await after(thrown);
out.thrown_status = thrown.status;
out.thrown_result = thrown.result;
state.throwAwait = false;
const v4Admit = () => {
  try {
    throw new Error("EPIPE: the relay exited before reading the answer");
  } catch (error) {
    return "relay failed: " + error.message;
  }
};
out.v4_lost_reason = v4Admit();

state.breakStdin = false;
state.answers = {
  prompt: {
    exitCode: 3,
    stdout: "",
    stderr: JSON.stringify({ code: "authority_unavailable", message: "the binary did not answer" }),
  },
};
let unavailableCode = null;
try {
  await before({ tool: "task", sessionID: "ses_relay", input: { subagent_type: "review-risk" } });
} catch (error) {
  unavailableCode = error.code;
}
out.unavailable_code = unavailableCode;

state.answers = {
  prompt: {
    exitCode: 1,
    stdout: "",
    stderr: JSON.stringify({ code: "capture-result-stale", message: "the review moved" }),
  },
};
let staleCode = null;
try {
  await before({ tool: "task", sessionID: "ses_relay", input: { subagent_type: "review-risk" } });
} catch (error) {
  staleCode = error.code;
}
out.stale_code = staleCode;

const refusedWith = async (exitCode, stderr) => {
  state.answers = { prompt: { exitCode, stdout: "", stderr } };
  try {
    await before({ tool: "task", sessionID: "ses_relay", input: { subagent_type: "review-risk" } });
  } catch (error) {
    return error.code;
  }
  return null;
};
out.silent_code = await refusedWith(1, "");
out.signal_code = await refusedWith(127, "gone");
const v4Classify = (status, stderr) => {
  let said = null;
  try {
    said = JSON.parse(stderr);
  } catch (error) {
    said = null;
  }
  if (status === 0) return null;
  if (said?.code === "authority_unavailable") return "stale_authority";
  return said?.code ?? "authority_unavailable";
};
out.v4_unavailable = v4Classify(3, JSON.stringify({ code: "authority_unavailable" }));

const v1 = await EstigiaReviewRelay({
  $: makeShell(),
  directory: "/tmp/relay-project",
  worktree: undefined,
});
out.v1_hooks = Object.keys(v1).sort();

const delivered = [];
for (const call of state.calls) {
  if (call.stdinBlob) delivered.push(await call.stdinBlob.text());
}
out.delivered = delivered;

console.log(JSON.stringify(out));
"#;

fn run_review_relay_driver() -> serde_json::Value {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("estigia-review-relay.mjs"),
        crate::setup::plugin::review_relay_source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the review relay plugin writes");
    let driver = root.path().join("drive-review-relay.mjs");
    std::fs::write(&driver, REVIEW_RELAY_DRIVER).expect("the driver writes");
    let launch = crate::transport::claim::opencode_relay_launch("review-reliability");
    let ran = std::process::Command::new("node")
        .arg(&driver)
        .arg(launch.to_string())
        .current_dir(root.path())
        .output();
    let ran = match ran {
        Ok(ran) => ran,
        Err(error) => panic!(
            "this test drives the review relay plugin, which is JavaScript, and `node` could not \
             be run ({error}). Install Node and re-run; it is not skipped."
        ),
    };
    assert!(
        ran.status.success(),
        "the review relay plugin would not load or the driver failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

/// The review relay plugin puts the binary's exact prompt in the task input and
/// hands the reviewer's raw answer back for admission, never to the orchestrator.
#[test]
fn the_review_relay_materialises_the_prompt_and_admits_the_answer() {
    let cases = run_review_relay_driver();
    assert_eq!(
        cases["prompt_injected"], "THE EXACT PROMPT",
        "the plugin did not put the binary's prompt in the task input: {cases}"
    );
    assert_eq!(
        cases["prompt_session"], true,
        "the plugin did not forward the session id the event carried: {cases}"
    );
    assert_eq!(
        cases["marker_injected"], "MARKED PROMPT",
        "a task naming the reviewer only in its marker was not relayed: {cases}"
    );

    let admitted = cases["admitted_result"]
        .as_str()
        .expect("the result the orchestrator reads is a string");
    assert!(
        !admitted.contains("RAW REVIEWER ANSWER"),
        "the reviewer's raw answer reached the orchestrator: {admitted}"
    );
    assert_ne!(
        cases["admitted_status"], "error",
        "a successful admission was reported as an error: {cases}"
    );
    let delivered: Vec<&str> = cases["delivered"]
        .as_array()
        .expect("what the relay received")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        delivered.contains(&"RAW REVIEWER ANSWER"),
        "the reviewer's raw answer never reached the relay: {delivered:?}"
    );

    let v1: Vec<&str> = cases["v1_hooks"]
        .as_array()
        .expect("the v1 hooks")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(
        v1,
        vec!["tool.execute.after", "tool.execute.before"],
        "the OpenCode 1 fallback does not carry both hooks: {v1:?}"
    );
}

/// The launch `launch_review_input` prescribes on an OpenCode run, a built-in
/// sub-agent with the marker prompt, is intercepted: the plugin asks the binary
/// for the job's prompt and puts it in the task input.
#[test]
fn the_review_relay_intercepts_the_launch_the_relay_answer_prescribes() {
    let cases = run_review_relay_driver();
    assert_eq!(
        cases["launch_injected"], "PROMPT FOR THE ANSWER'S LAUNCH",
        "the launch the relay answer prescribes was not intercepted: {cases}"
    );
    assert_eq!(
        cases["launch_asked"], true,
        "the plugin did not ask the binary for the reviewer the marker names: {cases}"
    );
    assert_eq!(
        cases["subagent_injected"], "PROMPT FOR A SUBAGENT TOOL",
        "a host whose sub-agent tool is named `subagent` was not intercepted: {cases}"
    );
}

/// v4 defect #1: a refusal reason was lost when the relay exited before reading
/// standard input, and the plugin treated the failed write as the outcome. The
/// reason is read from standard output to EOF first.
#[test]
fn the_review_relay_keeps_the_reason_when_the_relay_exits_before_reading_stdin() {
    let cases = run_review_relay_driver();
    assert_eq!(
        cases["stale_status"], "error",
        "a refused admission was not reported: {cases}"
    );
    let error = cases["stale_error"]
        .as_str()
        .expect("the refusal the orchestrator reads");
    assert!(
        error.contains("the review moved after job j1 was launched"),
        "the refusal's reason was lost: {error}"
    );
    assert!(
        !cases["stale_result"].as_str().unwrap_or("").contains("RAW"),
        "a refused admission left the reviewer's raw answer for the orchestrator: {cases}"
    );
    assert_eq!(
        cases["thrown_status"], "error",
        "a shell that threw was read as an admission: {cases}"
    );
    assert!(
        !cases["thrown_result"]
            .as_str()
            .unwrap_or("RAW")
            .contains("RAW"),
        "a shell that threw left the reviewer's raw answer for the orchestrator: {cases}"
    );
    let v4 = cases["v4_lost_reason"].as_str().expect("the v4 shape");
    assert!(
        !v4.contains("the review moved"),
        "the fixture does not reproduce the v4 defect it claims to: {v4}"
    );
}

/// v4 defect #2: `authority_unavailable` was reported as `stale_authority`. The
/// two are kept apart: an authority that did not answer is never a moved receipt.
#[test]
fn the_review_relay_keeps_unavailable_and_stale_apart() {
    let cases = run_review_relay_driver();
    assert_eq!(
        cases["unavailable_code"], "authority_unavailable",
        "an authority that did not answer was not reported as unavailable: {cases}"
    );
    assert_eq!(
        cases["stale_code"], "capture-result-stale",
        "a result for a stale receipt was not refused with the stale class: {cases}"
    );
    assert_eq!(
        cases["v4_unavailable"], "stale_authority",
        "the fixture does not reproduce the v4 defect it claims to: {cases}"
    );
}

fn count_of(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

#[test]
fn every_runtime_gets_exactly_one_managed_block() {
    let (_home, options) = sandbox();
    setup_fleet(&options, &Config::default());
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let text = fs::read_to_string(&paths.instructions).expect("the directive was written");
        assert_eq!(count_of(&text, DIRECTIVE_BEGIN), 1, "{}", adapter.slug);
        assert_eq!(count_of(&text, DIRECTIVE_END), 1, "{}", adapter.slug);
    }
}

#[test]
fn setup_over_a_stale_managed_block_replaces_it_in_place() {
    for adapter in AGENTS {
        let (_home, options) = sandbox();
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        fs::create_dir_all(paths.instructions.parent().expect("a parent")).expect("a directory");
        fs::write(
            &paths.instructions,
            format!(
                "before the block\n\n{DIRECTIVE_BEGIN}\n\nSTALE-DIRECTIVE-BODY\n\n{DIRECTIVE_END}\n\nafter the block\n"
            ),
        )
        .expect("a stale file");

        setup(adapter, &Config::default(), &options).expect("setup runs");
        let first = fs::read_to_string(&paths.instructions).expect("read back");
        assert!(!first.contains("STALE-DIRECTIVE-BODY"), "{}", adapter.slug);
        assert_eq!(count_of(&first, DIRECTIVE_BEGIN), 1, "{}", adapter.slug);
        assert_eq!(count_of(&first, DIRECTIVE_END), 1, "{}", adapter.slug);
        assert!(first.contains("before the block"), "{}", adapter.slug);
        assert!(first.contains("after the block"), "{}", adapter.slug);
        assert!(
            first.find("before the block") < first.find(DIRECTIVE_BEGIN),
            "{} moved the block out of its place",
            adapter.slug
        );

        setup(adapter, &Config::default(), &options).expect("setup runs again");
        let second = fs::read_to_string(&paths.instructions).expect("read back");
        assert_eq!(first, second, "{} is not idempotent", adapter.slug);
    }
}

#[test]
fn every_runtime_is_pointed_at_a_skill_that_describes_the_binary_launched_review() {
    let (_home, options) = sandbox();
    setup_fleet(&options, &Config::default());
    for adapter in AGENTS {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let directive = directive_for(adapter, &paths);
        assert!(
            directive.contains(&format!(
                "`{}` skill at `{}`",
                skill::DIRECTORY,
                paths.skill_root.display()
            )),
            "{} does not name the skill it installed",
            adapter.slug
        );
        let installed = fs::read_to_string(paths.skill_root.join(skill::CONTRACT))
            .expect("the skill was installed where the directive points");
        for tool in ["launch_review_input", "collect_review_input"] {
            assert!(
                installed.contains(tool),
                "{} is pointed at a skill that does not describe {tool}",
                adapter.slug
            );
        }
    }
}

const RELAY_MARKERS: &[&str] = &["relay", "review transport", "--input"];

fn relay_markers_in(text: &str) -> Vec<&'static str> {
    let lowered = text.to_lowercase();
    RELAY_MARKERS
        .iter()
        .copied()
        .filter(|marker| lowered.contains(marker))
        .collect()
}

fn review_surfaces(adapter: &AgentAdapter, paths: &AgentPaths) -> Vec<(&'static str, String)> {
    let mut surfaces = vec![("directive", directive_for(adapter, paths))];
    if adapter.slug == "opencode" {
        surfaces.push(("opencode review section", opencode_review_section()));
    }
    if adapter.slug == "pi" {
        surfaces.push(("pi review section", pi_review_section()));
    }
    surfaces
}

#[test]
fn no_runtime_without_a_review_relay_receives_relay_wording() {
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        if adapter.capabilities().review_relay {
            continue;
        }
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        for (surface, text) in review_surfaces(adapter, &paths) {
            assert_eq!(
                relay_markers_in(&text),
                Vec::<&str>::new(),
                "{} has no review relay, and its {surface} carries relay wording",
                adapter.slug
            );
        }
    }
}

#[test]
fn the_relay_marker_check_sees_a_marker_where_one_is_put() {
    assert_eq!(relay_markers_in("a Relay for reviews"), vec!["relay"]);
    assert_eq!(relay_markers_in("pass it with --input"), vec!["--input"]);
    assert!(relay_markers_in("launch_review_input").is_empty());
}

#[test]
fn only_opencode_and_pi_declare_a_review_relay() {
    for adapter in every_adapter() {
        assert_eq!(
            adapter.capabilities().review_relay,
            matches!(adapter.slug, "opencode" | "pi"),
            "{} declares a review relay it does not build, or lacks one it does",
            adapter.slug
        );
    }
}

#[test]
fn a_foreign_command_containing_hook_stop_is_not_estigias() {
    let (home, options) = sandbox();
    fs::create_dir_all(home.path().join(".claude")).unwrap();
    let settings = home.path().join(".claude").join("settings.json");
    let foreign = "/home/me/bin/slack-webhook stop";
    fs::write(
        &settings,
        serde_json::json!({"hooks": {"Stop": [{"hooks": [
            {"type": "command", "command": foreign}]}]}})
        .to_string(),
    )
    .unwrap();
    assert!(!is_gated(agent("claude-code"), &options));
    setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
    assert!(
        fs::read_to_string(&settings).unwrap().contains(foreign),
        "setup dropped the operator's hook"
    );
    uninstall(agent("claude-code"), &options).expect("uninstall runs");
    assert!(
        fs::read_to_string(&settings).unwrap().contains(foreign),
        "uninstall dropped the operator's hook"
    );
}

#[test]
fn a_command_is_estigias_by_shape_whatever_the_executable_path() {
    for ours in [
        "estigia hook pre-tool-use --dialect cursor",
        "\"/opt/estigia/bin/estigia\" hook stop --agent claude-code --dialect claude-code",
        "\"/Users/me/My Tools/estigia\" hook session-start",
        "\"C:\\Program Files\\estigia\\estigia.exe\" hook session-end --agent x",
        "/usr/local/bin/shim hook pre-tool-use",
    ] {
        assert!(render::runs_estigia_hook(ours, None), "{ours}");
    }
    for foreign in [
        "/home/me/bin/slack-webhook stop",
        "git-hook stop",
        "my-prehook session-start",
        "estigia hooks stop",
        "estigia doctor hook stop",
        "estigia hook unknown-event",
        "estigia hook",
        "",
    ] {
        assert!(!render::runs_estigia_hook(foreign, None), "{foreign}");
    }
    assert!(render::runs_estigia_hook(
        "estigia hook pre-tool-use",
        Some(crate::harness::hook::Event::PreToolUse)
    ));
    assert!(!render::runs_estigia_hook(
        "estigia hook stop",
        Some(crate::harness::hook::Event::PreToolUse)
    ));
}

#[cfg(unix)]
mod configuration_ownership {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn symlinked_settings(home: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let dotfiles = home.join("dotfiles");
        fs::create_dir_all(&dotfiles).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        let real = dotfiles.join("settings.json");
        fs::write(&real, "{\"theme\": \"dark\"}\n").unwrap();
        let link = home.join(".claude").join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        (real, link)
    }

    #[test]
    fn setup_keeps_a_symlinked_settings_file_a_symlink_and_writes_its_target() {
        let (home, options) = sandbox();
        let (real, link) = symlinked_settings(home.path());
        setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let target = fs::read_to_string(&real).unwrap();
        assert!(target.contains("\"theme\""), "{target}");
        assert!(target.contains("estigia"), "{target}");
    }

    #[test]
    fn uninstall_edits_the_target_of_a_symlinked_settings_file_and_keeps_the_link() {
        let (home, options) = sandbox();
        let (real, link) = symlinked_settings(home.path());
        setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
        uninstall(agent("claude-code"), &options).expect("uninstall runs");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let target = fs::read_to_string(&real).unwrap();
        assert!(target.contains("\"theme\""), "{target}");
        assert!(!target.contains("estigia hook"), "{target}");
    }

    #[test]
    fn setup_keeps_an_owner_only_mode() {
        let (home, options) = sandbox();
        let mcp = home.path().join(".claude.json");
        fs::write(&mcp, "{\"oauthAccount\": {\"x\": 1}}\n").unwrap();
        fs::set_permissions(&mcp, fs::Permissions::from_mode(0o600)).unwrap();
        setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
        let mode = fs::metadata(&mcp).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn a_dangling_symlink_is_refused_and_left_alone() {
        let (home, _options) = sandbox();
        let link = home.path().join("settings.json");
        std::os::unix::fs::symlink(home.path().join("missing.json"), &link).unwrap();
        let refusal = crate::paths::replace_atomically(&link, "{}\n").expect_err("a dangling link");
        assert!(refusal.to_string().contains("dangling"), "{refusal}");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!home.path().join("missing.json").exists());
    }

    #[test]
    fn a_chain_of_symlinks_writes_the_final_file() {
        let (home, _options) = sandbox();
        let real = home.path().join("real.json");
        let middle = home.path().join("middle.json");
        let outer = home.path().join("outer.json");
        fs::write(&real, "old\n").unwrap();
        std::os::unix::fs::symlink(&real, &middle).unwrap();
        std::os::unix::fs::symlink(&middle, &outer).unwrap();
        crate::paths::replace_atomically(&outer, "new\n").expect("replaced");
        assert!(
            fs::symlink_metadata(&outer)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(&middle)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "new\n");
    }
}

mod byte_order_mark {
    use super::*;

    #[test]
    fn a_bom_settings_file_still_reads_as_gated_and_tooled_after_setup() {
        let (home, options) = sandbox();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        fs::write(
            home.path().join(".claude").join("settings.json"),
            "\u{feff}{}\n",
        )
        .unwrap();
        fs::write(home.path().join(".claude.json"), "\u{feff}{}\n").unwrap();
        setup(agent("claude-code"), &Config::default(), &options).expect("setup runs");
        let settings =
            fs::read_to_string(home.path().join(".claude").join("settings.json")).unwrap();
        let mcp = fs::read_to_string(home.path().join(".claude.json")).unwrap();
        assert!(settings.starts_with('\u{feff}') && mcp.starts_with('\u{feff}'));
        assert!(is_gated(agent("claude-code"), &options));
        assert!(exposes_tools(agent("claude-code"), &options));
        assert_eq!(
            tools_start_the_server(agent("claude-code"), &options),
            Some(true)
        );
    }

    #[test]
    fn narrowed_and_wires_answer_the_same_with_or_without_a_mark() {
        let text = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/opt/estigia hook pre-tool-use"}]}]}}"#;
        let marked = format!("\u{feff}{text}");
        let judged = ["Bash", "Edit", "Write"];
        assert_eq!(
            wiring::narrowed(&marked, &judged),
            wiring::narrowed(text, &judged)
        );
        assert!(!wiring::narrowed(text, &judged).is_empty());
        assert_eq!(wiring::wires(&marked), wiring::wires(text));
        assert!(!wiring::wires(text).is_empty());
    }
}

#[test]
fn a_duplicated_directive_block_is_removed_whole_and_upserted_once() {
    let fence = DIRECTIVE_FENCE;
    let one = fence.upsert("# mine\n", "old directive");
    let doubled = format!("{one}\n{}", &one[one.find(fence.begin).unwrap()..]);

    let removed = fence.remove(&doubled);
    assert!(!fence.is_present(&removed), "{removed}");
    assert_eq!(removed, "# mine\n");

    let upserted = fence.upsert(&doubled, "new directive");
    assert_eq!(upserted.matches(fence.begin).count(), 1, "{upserted}");
    assert!(!upserted.contains("old directive"), "{upserted}");
    assert_eq!(upserted, fence.upsert(&one, "new directive"));
}

#[test]
fn pi_setup_writes_the_directive_the_server_and_the_extension_and_uninstall_takes_back_those() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let agent_dir = home.path().join(".pi").join("agent");
    let extension = agent_dir.join(plugin::PI_DIRECTORY).join(plugin::PI_FILE);
    let relay = agent_dir
        .join(plugin::PI_DIRECTORY)
        .join(plugin::PI_REVIEW_FILE);
    assert_eq!(review_plugin_path(adapter, &paths), Some(relay.clone()));
    assert_eq!(paths.instructions, agent_dir.join("AGENTS.md"));
    assert_eq!(paths.mcp_config, Some(agent_dir.join("mcp.json")));
    assert_eq!(paths.plugin, Some(extension.clone()));
    assert_eq!(
        paths.skill_root,
        home.path()
            .join(".agents")
            .join("skills")
            .join(skill::DIRECTORY)
    );
    assert!(paths.hooks.is_none());
    assert_eq!(
        paths.loop_plugin,
        Some(
            agent_dir
                .join(plugin::PI_DIRECTORY)
                .join(plugin::PI_LOOP_FILE)
        )
    );
    assert!(!is_gated(adapter, &options));

    fs::create_dir_all(&agent_dir).expect("the agent directory");
    fs::write(agent_dir.join("AGENTS.md"), "# mine\n").expect("their instructions");
    fs::write(
        agent_dir.join("mcp.json"),
        "{\"mcpServers\":{\"theirs\":{\"command\":\"x\"}}}\n",
    )
    .expect("their servers");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");

    let instructions = fs::read_to_string(&paths.instructions).expect("the instructions read");
    assert!(instructions.starts_with("# mine\n"));
    assert!(DIRECTIVE_FENCE.is_present(&instructions));
    let servers: Value = serde_json::from_str(
        &fs::read_to_string(agent_dir.join("mcp.json")).expect("the servers read"),
    )
    .expect("the servers parse");
    assert_eq!(servers["mcpServers"]["theirs"]["command"], "x");
    assert_eq!(servers["mcpServers"][SERVER_NAME]["args"][0], "mcp");
    assert!(
        extension.is_file(),
        "{} was not written",
        extension.display()
    );
    assert!(
        plugin::review_relay_is_ours(fs::read_to_string(&relay).ok().as_deref()),
        "{} was not written",
        relay.display()
    );
    assert!(is_gated(adapter, &options));
    assert!(exposes_tools(adapter, &options));
    assert!(tools_start_the_server(adapter, &options).unwrap_or(false));

    let registered = wiring::registered(adapter, &options);
    assert_eq!(registered.len(), 1, "{registered:?}");
    assert_eq!(registered[0].0, extension);
    let wire = &registered[0].1[0];
    assert_eq!(wire.named, "tool_call");
    assert!(wire.is_live(), "{:?}", wire.fault());

    let before: Vec<String> = [
        &paths.instructions,
        &agent_dir.join("mcp.json"),
        &extension,
        &relay,
    ]
    .iter()
    .map(|file| fs::read_to_string(file).expect("the file reads"))
    .collect();
    let again = setup(adapter, &Config::default(), &options).expect("the second setup succeeds");
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.change == Change::Unchanged),
        "{:?}",
        again.actions
    );
    let after: Vec<String> = [
        &paths.instructions,
        &agent_dir.join("mcp.json"),
        &extension,
        &relay,
    ]
    .iter()
    .map(|file| fs::read_to_string(file).expect("the file reads"))
    .collect();
    assert_eq!(before, after);

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!extension.exists(), "the extension survived an uninstall");
    assert!(!relay.exists(), "the review relay survived an uninstall");
    assert!(!is_gated(adapter, &options));
    assert!(wiring::registered(adapter, &options).is_empty());
    assert_eq!(
        fs::read_to_string(&paths.instructions).expect("their instructions survive"),
        "# mine\n"
    );
    let servers: Value = serde_json::from_str(
        &fs::read_to_string(agent_dir.join("mcp.json")).expect("their servers survive"),
    )
    .expect("the servers parse");
    assert_eq!(
        servers,
        serde_json::json!({ "mcpServers": { "theirs": { "command": "x" } } })
    );
}

#[test]
fn an_extension_estigia_did_not_write_is_neither_replaced_nor_removed_for_pi() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    let file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .plugin
        .expect("Pi loads extensions");
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default function () {}\n";
    fs::write(&file, theirs).expect("write their extension");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
    assert!(!is_gated(adapter, &options));

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
}

fn pi_mcp_json(options: &SetupOptions) -> PathBuf {
    resolve_paths(agent("pi"), options)
        .expect("paths resolve")
        .mcp_config
        .expect("Pi has an MCP file")
}

#[test]
fn setup_declares_the_tools_to_pi_directly() {
    let (_home, options) = sandbox();
    setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");

    let servers: Value =
        serde_json::from_str(&fs::read_to_string(pi_mcp_json(&options)).expect("the servers read"))
            .expect("the servers parse");
    let entry = &servers["mcpServers"][SERVER_NAME];
    assert_eq!(entry["exposure"], "direct", "{entry:#}");
    assert_eq!(entry["args"], serde_json::json!(["mcp"]));
    assert!(entry["command"].is_string());
    assert!(exposes_tools(agent("pi"), &options));
}

fn pi_entry_for(version: Option<PiVersion>) -> Value {
    let (_home, mut options) = sandbox();
    options.pi_version = version;
    setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");
    let servers: Value =
        serde_json::from_str(&fs::read_to_string(pi_mcp_json(&options)).expect("the servers read"))
            .expect("the servers parse");
    servers["mcpServers"][SERVER_NAME].clone()
}

fn pi_at(major: u64, minor: u64, patch: u64) -> Option<PiVersion> {
    Some(PiVersion {
        major,
        minor,
        patch,
    })
}

#[test]
fn setup_defers_the_tools_on_a_pi_that_can_and_declares_them_directly_on_one_that_cannot() {
    for version in [pi_at(1, 2, 0), pi_at(1, 3, 4), pi_at(2, 0, 0)] {
        let entry = pi_entry_for(version);
        assert_eq!(entry["exposure"], "deferred", "{version:?}: {entry:#}");
        let description = entry["description"].as_str().expect("a description");
        assert!(description.contains("workflow"), "{description}");
    }
    for version in [pi_at(1, 1, 9), pi_at(1, 1, 0), pi_at(0, 99, 0), None] {
        let entry = pi_entry_for(version);
        assert_eq!(entry["exposure"], "direct", "{version:?}: {entry:#}");
        assert!(entry.get("description").is_none(), "{entry:#}");
    }
}

#[test]
fn a_pi_dry_run_says_which_exposure_it_would_write_and_why() {
    for (version, exposure, reason) in [
        (pi_at(1, 2, 0), "deferred", "Pi 1.2.0"),
        (pi_at(1, 1, 0), "direct", "predates"),
        (None, "direct", "not read"),
    ] {
        let (_home, mut options) = sandbox();
        options.pi_version = version;
        options.dry_run = true;
        let result = setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");
        let note = result
            .notes
            .iter()
            .find(|note| note.contains("exposure"))
            .unwrap_or_else(|| panic!("no exposure note: {:?}", result.notes));
        assert!(note.contains(exposure), "{note}");
        assert!(note.contains(reason), "{note}");
    }
}

#[test]
fn setup_migrates_a_pi_entry_written_without_an_exposure_and_a_second_run_changes_nothing() {
    let (_home, options) = sandbox();
    let file = pi_mcp_json(&options);
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    fs::write(
        &file,
        serde_json::json!({
            "mcpServers": {
                "theirs": { "command": "x" },
                SERVER_NAME: { "command": "old-estigia", "args": ["mcp"], "exposure": "codemode" }
            }
        })
        .to_string(),
    )
    .expect("an older setup's entry");

    setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");
    let first = fs::read_to_string(&file).expect("the servers read");
    let servers: Value = serde_json::from_str(&first).expect("the servers parse");
    assert_eq!(servers["mcpServers"][SERVER_NAME]["exposure"], "direct");
    assert_eq!(servers["mcpServers"]["theirs"]["command"], "x");

    setup(agent("pi"), &Config::default(), &options).expect("setup succeeds again");
    assert_eq!(fs::read_to_string(&file).expect("the servers read"), first);
}

#[test]
fn uninstall_removes_the_pi_entry_with_an_exposure_and_keeps_their_servers() {
    let (_home, options) = sandbox();
    let file = pi_mcp_json(&options);
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    fs::write(&file, "{\"mcpServers\":{\"theirs\":{\"command\":\"x\"}}}\n").expect("their servers");
    setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");

    uninstall(agent("pi"), &options).expect("uninstall succeeds");
    let servers: Value =
        serde_json::from_str(&fs::read_to_string(&file).expect("the servers read"))
            .expect("the servers parse");
    assert_eq!(
        servers,
        serde_json::json!({ "mcpServers": { "theirs": { "command": "x" } } })
    );
}

#[test]
fn no_other_adapter_is_given_an_exposure_in_its_mcp_entry() {
    for adapter in every_adapter().filter(|adapter| adapter.capabilities().tool_server) {
        let written = render::render_mcp(
            std::path::Path::new("mcp.json"),
            None,
            std::path::Path::new("estigia"),
            adapter.mcp_format(),
            PiExposure::Deferred,
        )
        .expect("renders");
        assert_eq!(
            written.contains("exposure"),
            adapter.slug == "pi",
            "{} {} a Pi-only field: {written}",
            adapter.slug,
            if adapter.slug == "pi" {
                "was not handed"
            } else {
                "was handed"
            }
        );
    }
}

#[test]
fn the_pi_extension_gates_the_tools_the_classifier_judges_by_the_arguments_pi_sends() {
    use crate::harness::{Action, SHELL_TOOLS, Sensitivity, WRITE_TOOLS, classify};

    for name in plugin::PI_GATED {
        assert!(
            WRITE_TOOLS.contains(name) || SHELL_TOOLS.contains(name),
            "the Pi extension wakes the gate for {name:?}, which the classifier does not judge"
        );
    }
    assert!(
        plugin::pi_source(std::path::Path::new("/opt/estigia"))
            .contains("const GATED = [\"edit\",\"write\",\"bash\"];")
    );

    let surface = "/home/me/.pi/agent/extensions/estigia-workflow-authority.ts";
    let (action, how) = classify(
        "write",
        &serde_json::json!({ "path": surface, "content": "x" }),
    );
    assert_eq!(how, Sensitivity::Boundary);
    assert_eq!(
        action,
        Action::Write {
            target: surface.to_owned()
        }
    );
    let (_, how) = classify(
        "edit",
        &serde_json::json!({
            "path": "/home/me/.pi/agent/mcp.json",
            "edits": [{ "oldText": "a", "newText": "b" }],
        }),
    );
    assert_eq!(how, Sensitivity::Boundary);
    let (_, how) = classify(
        "bash",
        &serde_json::json!({ "command": format!("rm {surface}") }),
    );
    assert_eq!(how, Sensitivity::Boundary);
    let (_, how) = classify(
        "write",
        &serde_json::json!({ "path": "/home/me/src/main.rs", "content": "x" }),
    );
    assert_eq!(how, Sensitivity::Routine);
}

fn node_executable() -> String {
    match std::process::Command::new("node")
        .args(["-p", "process.execPath"])
        .output()
    {
        Ok(ran) if ran.status.success() => String::from_utf8_lossy(&ran.stdout).trim().to_owned(),
        Ok(ran) => panic!(
            "`node` could not report its own path: {}",
            String::from_utf8_lossy(&ran.stderr)
        ),
        Err(error) => panic!(
            "this test drives the generated Pi extension, which is JavaScript, and `node` could \
             not be run ({error}). Install Node and re-run; it is not skipped, because an \
             extension nothing executes is a gate nothing measures."
        ),
    }
}

/// Drives the generated Pi extension under `node` against a stand-in binary.
///
/// The stand-in is `node` itself, preloaded through `NODE_OPTIONS` with a script
/// that records its arguments and working directory and then exits with the
/// code and standard error the scenario asks for, so the extension spawns a real
/// process on every platform without a shell script.
#[test]
fn the_pi_extension_blocks_on_one_and_two_and_lets_a_gate_that_did_not_answer_through_with_a_word()
{
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    let project = base.join("project");
    fs::create_dir_all(&project).expect("the project directory");
    fs::write(
        base.join("gate.mjs"),
        plugin::pi_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("missing.mjs"),
        plugin::pi_source(&base.join("no-such-estigia")),
    )
    .expect("the extension writes");
    fs::write(
        base.join("stub.cjs"),
        r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "gate") {
  let stdin = "";
  try { stdin = fs.readFileSync(0, "utf8"); } catch {}
  fs.appendFileSync(
    process.env.STUB_LOG,
    JSON.stringify({ args: [path.basename(process.argv[1]), ...process.argv.slice(2)], cwd: process.cwd(), stdin }) + "\n",
  );
  process.stderr.write(process.env.STUB_STDERR ?? "");
  process.exit(Number(process.env.STUB_EXIT ?? 0));
}
"#,
    )
    .expect("the stand-in writes");
    fs::write(
        base.join("drive.mjs"),
        r#"import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;

const calls = () => readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const run = async (label, file, event, ctx, exit, stderr) => {
  const handlers = {};
  (await import(`./${file}`)).default({ on: (name, handler) => { handlers[name] = handler; } });
  process.env.STUB_EXIT = String(exit);
  process.env.STUB_STDERR = stderr;
  const warnings = [];
  const original = console.error;
  console.error = (line) => warnings.push(String(line));
  const before = calls().length;
  const result = await handlers.tool_call(event, ctx);
  console.error = original;
  return { label, result: result ?? null, warnings, calls: calls().slice(before) };
};

const ctx = { cwd: process.env.PROJECT, sessionManager: { getSessionId: () => "sess-0198" } };
const bash = { toolName: "bash", input: { command: "git commit -m fixture" } };

console.log(JSON.stringify([
  await run("zero", "gate.mjs", bash, ctx, 0, ""),
  await run("one", "gate.mjs", bash, ctx, 1, "refused: no live claim"),
  await run("two", "gate.mjs", { toolName: "write", input: { path: "a.rs", content: "x" } }, ctx, 2, "unread"),
  await run("silent", "gate.mjs", bash, ctx, 1, ""),
  await run("127", "gate.mjs", bash, ctx, 127, "gone"),
  await run("missing", "missing.mjs", bash, ctx, 0, ""),
  await run("read", "gate.mjs", { toolName: "read", input: { path: "a.rs" } }, ctx, 1, "refused"),
  await run("upper", "gate.mjs", { toolName: "Edit", input: { path: "a.rs", edits: [] } }, ctx, 1, "refused"),
  await run("anonymous", "gate.mjs", bash, { cwd: process.env.PROJECT }, 0, ""),
  await run("blank", "gate.mjs", bash, { cwd: process.env.PROJECT, sessionManager: { getSessionId: () => "  " } }, 0, ""),
  await run("throwing", "gate.mjs", bash, { cwd: process.env.PROJECT, sessionManager: { getSessionId: () => { throw new Error("no session"); } } }, 0, ""),
]));
"#,
    )
    .expect("the driver writes");

    let ran = std::process::Command::new(&node)
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.log"))
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("PROJECT", &project)
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the extension would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    let seen = |label: &str| {
        results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"))
    };

    let zero = seen("zero");
    assert!(
        zero["result"].is_null() && zero["warnings"] == serde_json::json!([]),
        "{zero}"
    );
    assert_eq!(
        zero["calls"],
        serde_json::json!([{
            "args": [
                "gate",
                "bash",
                "--input",
                "-",
                "--session-id",
                "sess-0198",
                "--runtime",
                "pi",
            ],
            "cwd": project,
            "stdin": "{\"command\":\"git commit -m fixture\"}",
        }])
    );
    assert_eq!(
        seen("one")["result"],
        serde_json::json!({ "block": true, "reason": "refused: no live claim" })
    );
    assert_eq!(
        seen("two")["result"],
        serde_json::json!({ "block": true, "reason": "unread" })
    );
    let silent = seen("silent")["result"]["reason"]
        .as_str()
        .unwrap_or_default();
    assert!(
        silent.contains("refused this bash") && silent.contains("exit 1"),
        "{silent}"
    );

    let unread = seen("127");
    assert_eq!(unread["result"]["block"], true, "{unread}");
    assert!(
        unread["result"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("cannot read") && reason.contains("exit 127")),
        "{unread}"
    );

    let result = seen("missing");
    assert!(result["result"].is_null(), "{result}");
    let warnings = result["warnings"].as_array().expect("the warnings");
    assert_eq!(warnings.len(), 1, "{result}");
    assert!(
        warnings[0]
            .as_str()
            .is_some_and(|line| line.contains("went out unchecked")),
        "{result}"
    );
    assert_eq!(seen("missing")["calls"], serde_json::json!([]));

    let read = seen("read");
    assert!(
        read["result"].is_null() && read["calls"] == serde_json::json!([]),
        "{read}"
    );
    let upper = seen("upper");
    assert_eq!(upper["result"]["block"], true, "{upper}");
    assert_eq!(upper["calls"][0]["args"][1], "edit", "{upper}");

    for label in ["anonymous", "blank", "throwing"] {
        let without = seen(label);
        assert_eq!(
            without["calls"][0]["args"],
            serde_json::json!(["gate", "bash", "--input", "-"]),
            "{label} sent a session or a runtime without a usable session id"
        );
    }
}

#[test]
fn the_pi_extension_puts_a_question_to_a_person_only_in_the_tui_and_blocks_on_any_code_it_cannot_read()
 {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    let project = base.join("project");
    fs::create_dir_all(&project).expect("the project directory");
    fs::write(
        base.join("gate.mjs"),
        plugin::pi_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("stub.cjs"),
        r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "gate") {
  fs.appendFileSync(
    process.env.STUB_LOG,
    JSON.stringify({ args: process.argv.slice(2) }) + "\n",
  );
  process.stderr.write(process.env.STUB_STDERR ?? "");
  process.exit(Number(process.env.STUB_EXIT ?? 0));
}
"#,
    )
    .expect("the stand-in writes");
    fs::write(
        base.join("drive.mjs"),
        r#"import { readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;

const calls = () => readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const run = async (label, mode, confirm, exit, stderr) => {
  const handlers = {};
  (await import("./gate.mjs")).default({ on: (name, handler) => { handlers[name] = handler; } });
  process.env.STUB_EXIT = String(exit);
  process.env.STUB_STDERR = stderr;
  const shown = [];
  const ctx = {
    cwd: process.env.PROJECT,
    mode,
    hasUI: mode !== "print",
    sessionManager: { getSessionId: () => "sess-0198" },
    ui: {
      confirm: async (title, message) => {
        shown.push({ title, message });
        return confirm();
      },
      setStatus: () => {},
    },
  };
  const warnings = [];
  const original = console.error;
  console.error = (line) => warnings.push(String(line));
  const before = calls().length;
  const result = await handlers.tool_call({ toolName: "write", input: { path: "a.rs", content: "x" } }, ctx);
  console.error = original;
  return { label, result: result ?? null, shown, warnings, calls: calls().slice(before) };
};

const yes = () => true;
const no = () => false;
const throws = () => { throw new Error("no dialog"); };

console.log(JSON.stringify([
  await run("yes", "tui", yes, 4, "issue-required: claim first"),
  await run("no", "tui", no, 4, "issue-required: claim first"),
  await run("cancelled", "tui", () => undefined, 4, "issue-required: claim first"),
  await run("throws", "tui", throws, 4, "issue-required: claim first"),
  await run("rpc", "rpc", yes, 1, "refused: no live claim"),
  await run("json", "json", yes, 1, "refused: no live claim"),
  await run("print", "print", yes, 1, "refused: no live claim"),
  await run("three", "tui", yes, 3, "unknown option"),
  await run("seven", "tui", yes, 7, ""),
  await run("allowed", "tui", yes, 0, ""),
]));
"#,
    )
    .expect("the driver writes");

    let ran = std::process::Command::new(&node)
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.log"))
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("PROJECT", &project)
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the extension would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let results: Vec<serde_json::Value> =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    let seen = |label: &str| {
        results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"))
    };
    let passes_flag = |result: &serde_json::Value| {
        result["calls"][0]["args"]
            .as_array()
            .is_some_and(|args| args.iter().any(|arg| arg == "--can-ask"))
    };
    let blocked_with = |result: &serde_json::Value, text: &str| {
        result["result"]["block"] == true
            && result["result"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains(text))
    };

    let yes = seen("yes");
    assert!(passes_flag(yes), "{yes}");
    assert!(
        yes["result"].is_null() && yes["warnings"] == serde_json::json!([]),
        "{yes}"
    );
    assert_eq!(
        yes["shown"],
        serde_json::json!([{ "title": "Estigia", "message": "issue-required: claim first" }]),
        "{yes}"
    );

    for label in ["no", "cancelled", "throws"] {
        let declined = seen(label);
        assert!(passes_flag(declined), "{label}: {declined}");
        assert!(
            blocked_with(declined, "issue-required: claim first"),
            "{label}: {declined}"
        );
    }

    for label in ["rpc", "json", "print"] {
        let result = seen(label);
        assert!(!passes_flag(result), "{label}: {result}");
        assert_eq!(result["shown"], serde_json::json!([]), "{label}: {result}");
        assert_eq!(result["result"]["block"], true, "{label}: {result}");
    }
    let rpc = seen("rpc");
    assert_eq!(
        rpc["result"],
        serde_json::json!({ "block": true, "reason": "refused: no live claim" }),
        "{rpc}"
    );

    for label in ["three", "seven"] {
        let unread = seen(label);
        assert!(
            blocked_with(unread, "cannot read") && blocked_with(unread, "estigia setup pi"),
            "{label}: {unread}"
        );
        assert_eq!(unread["shown"], serde_json::json!([]), "{label}: {unread}");
    }

    let allowed = seen("allowed");
    assert!(
        allowed["result"].is_null() && allowed["shown"] == serde_json::json!([]),
        "{allowed}"
    );
}

#[test]
fn the_pi_extension_hands_the_binary_the_root_session_only_from_a_process_that_inherited_one() {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    fs::write(
        base.join("gate.mjs"),
        plugin::pi_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("stub.cjs"),
        r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "gate") {
  fs.appendFileSync(process.env.STUB_LOG, JSON.stringify(process.argv.slice(3)) + "\n");
  process.exit(0);
}
"#,
    )
    .expect("the stand-in writes");
    fs::write(
        base.join("drive.mjs"),
        r#"import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
const session = process.env.OWN_SESSION;
const handlers = {};
(await import("./gate.mjs")).default({ on: (name, handler) => { handlers[name] = handler; } });
const ctx = { cwd: process.cwd(), sessionManager: { getSessionId: () => session } };
if (process.env.FIRST_EVENT === "session_start") await handlers.session_start({ type: "session_start" }, ctx);
await handlers.tool_call({ toolName: "read", input: {} }, ctx);
await handlers.tool_call({ toolName: "write", input: { path: "a.rs", content: "x" } }, ctx);
if (process.env.SPAWN_CHILD) {
  spawnSync(process.execPath, [fileURLToPath(import.meta.url)], {
    env: { ...process.env, OWN_SESSION: "child-3", SPAWN_CHILD: "", FIRST_EVENT: "" },
  });
}
const logged = readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
console.log(JSON.stringify({ logged, exported: process.env.ESTIGIA_PI_PARENT_SESSION ?? null }));
"#,
    )
    .expect("the driver writes");

    let drive = |session: &str, first: &str, spawn: bool, inherited: Option<(&str, &str)>| {
        let log = base.join("calls.log");
        fs::write(&log, "").expect("the log resets");
        let mut command = std::process::Command::new(&node);
        command
            .arg(base.join("drive.mjs"))
            .current_dir(&base)
            .env("STUB_LOG", &log)
            .env(
                "STUB_PATH",
                base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
            )
            .env("OWN_SESSION", session)
            .env("FIRST_EVENT", first)
            .env("SPAWN_CHILD", if spawn { "1" } else { "" })
            .env_remove("NODE_OPTIONS")
            .env_remove("ESTIGIA_PI_PARENT_SESSION")
            .env_remove("ESTIGIA_PI_PARENT_PID");
        if let Some((id, pid)) = inherited {
            command
                .env("ESTIGIA_PI_PARENT_SESSION", id)
                .env("ESTIGIA_PI_PARENT_PID", pid);
        }
        let ran = command.output().expect("node runs");
        assert!(
            ran.status.success(),
            "the driver failed: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let said: serde_json::Value =
            serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
        let calls: Vec<Vec<String>> =
            serde_json::from_value(said["logged"].clone()).expect("calls");
        (calls, said["exported"].clone())
    };
    let parent_flag = |call: &[String]| {
        call.iter()
            .position(|word| word == "--parent-session-id")
            .map(|at| call[at + 1].clone())
    };

    for first in ["session_start", ""] {
        let (calls, exported) = drive("root-1", first, true, None);
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            parent_flag(&calls[0]),
            None,
            "the root named a parent: {calls:?}"
        );
        assert_eq!(exported, "root-1");
        assert_eq!(
            parent_flag(&calls[1]).as_deref(),
            Some("root-1"),
            "a child spawned with the inherited environment named no parent: {calls:?}"
        );
        assert!(
            calls[1]
                .windows(2)
                .any(|pair| pair == ["--session-id", "child-3"]),
            "{calls:?}"
        );
    }

    let (calls, _) = drive("child-2", "", false, Some(("root-1", "1")));
    assert_eq!(
        parent_flag(&calls[0]).as_deref(),
        Some("root-1"),
        "{calls:?}"
    );

    let (calls, _) = drive("child-2", "", false, Some(("child-2", "1")));
    assert_eq!(
        parent_flag(&calls[0]),
        None,
        "a session named itself its parent: {calls:?}"
    );
}

#[test]
fn the_pi_extension_links_a_child_by_its_parent_header_whatever_its_pid() {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    fs::write(
        base.join("gate.mjs"),
        plugin::pi_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("2026-10-14T00-00-00_root-1.jsonl"),
        "{\"type\":\"session\",\"id\":\"root-1\"}\n",
    )
    .expect("the parent session writes");
    fs::write(
        base.join("stub.cjs"),
        r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "gate") {
  fs.appendFileSync(process.env.STUB_LOG, JSON.stringify(process.argv.slice(3)) + "\n");
  process.exit(0);
}
"#,
    )
    .expect("the stand-in writes");
    fs::write(
        base.join("drive.mjs"),
        r#"import { readFileSync } from "node:fs";

process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
const handlers = {};
(await import("./gate.mjs")).default({ on: (name, handler) => { handlers[name] = handler; } });
for (const { session, parent, reason } of JSON.parse(process.env.SESSIONS)) {
  const header = parent ? { parentSession: `${process.cwd()}/${parent}` } : {};
  const ctx = { cwd: process.cwd(), sessionManager: { getSessionId: () => session, getHeader: () => header } };
  if (reason) await handlers.session_start({ type: "session_start", reason }, ctx);
  await handlers.tool_call({ toolName: "write", input: { path: "a.rs", content: "x" } }, ctx);
}
const logged = readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
console.log(JSON.stringify({ logged, exported: process.env.ESTIGIA_PI_PARENT_SESSION ?? null }));
"#,
    )
    .expect("the driver writes");

    let drive = |sessions: serde_json::Value, inherited: Option<(&str, &str)>| {
        let log = base.join("calls.log");
        fs::write(&log, "").expect("the log resets");
        let mut command = std::process::Command::new(&node);
        command
            .arg(base.join("drive.mjs"))
            .current_dir(&base)
            .env("STUB_LOG", &log)
            .env(
                "STUB_PATH",
                base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
            )
            .env("SESSIONS", sessions.to_string())
            .env_remove("NODE_OPTIONS")
            .env_remove("ESTIGIA_PI_PARENT_SESSION")
            .env_remove("ESTIGIA_PI_PARENT_PID");
        if let Some((id, pid)) = inherited {
            command
                .env("ESTIGIA_PI_PARENT_SESSION", id)
                .env("ESTIGIA_PI_PARENT_PID", pid);
        }
        let ran = command.output().expect("node runs");
        assert!(
            ran.status.success(),
            "the driver failed: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        let said: serde_json::Value =
            serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
        let calls: Vec<Vec<String>> =
            serde_json::from_value(said["logged"].clone()).expect("calls");
        (calls, said["exported"].clone())
    };
    let parent_flag = |call: &[String]| {
        call.iter()
            .position(|word| word == "--parent-session-id")
            .map(|at| call[at + 1].clone())
    };

    let (calls, exported) = drive(
        serde_json::json!([
            {"session": "root-1"},
            {"session": "child-3", "parent": "2026-10-14T00-00-00_root-1.jsonl"},
            {"session": "root-1"}
        ]),
        None,
    );
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert_eq!(parent_flag(&calls[0]), None, "{calls:?}");
    assert_eq!(
        parent_flag(&calls[1]).as_deref(),
        Some("root-1"),
        "an in-process child was decided as an unrelated session: {calls:?}"
    );
    assert_eq!(
        parent_flag(&calls[2]),
        None,
        "the parent was read as a child after the in-process child ran: {calls:?}"
    );
    assert_eq!(exported, "root-1", "the child overwrote the exported root");

    let (calls, exported) = drive(
        serde_json::json!([
            {"session": "child-3", "parent": "2026-10-14T00-00-00_root-1.jsonl"}
        ]),
        None,
    );
    assert_eq!(
        parent_flag(&calls[0]).as_deref(),
        Some("root-1"),
        "{calls:?}"
    );
    assert_eq!(
        exported,
        serde_json::Value::Null,
        "a child exported itself as a root"
    );

    let (calls, exported) = drive(
        serde_json::json!([
            {"session": "child-3", "parent": "2026-10-14T00-00-00_gone-5.jsonl"}
        ]),
        Some(("root-1", "1")),
    );
    assert_eq!(
        parent_flag(&calls[0]).as_deref(),
        Some("root-1"),
        "{calls:?}"
    );
    assert_eq!(exported, "root-1");

    let (calls, exported) = drive(
        serde_json::json!([
            {"session": "child-3", "parent": "2026-10-14T00-00-00_gone-5.jsonl"}
        ]),
        None,
    );
    assert_eq!(
        parent_flag(&calls[0]).as_deref(),
        Some("gone-5"),
        "the parent id was not read from the session file name: {calls:?}"
    );
    assert_eq!(exported, serde_json::Value::Null);

    for reason in ["new", "fork", "resume", "reload"] {
        let (calls, exported) = drive(
            serde_json::json!([
                {"session": "root-1"},
                {"session": "next-4", "parent": "2026-10-14T00-00-00_root-1.jsonl", "reason": reason},
                {"session": "next-4", "parent": "2026-10-14T00-00-00_root-1.jsonl"}
            ]),
            None,
        );
        assert_eq!(calls.len(), 3, "{calls:?}");
        assert_eq!(
            parent_flag(&calls[1]),
            None,
            "a session begun by `{reason}` was read as a child of the one before it: {calls:?}"
        );
        assert_eq!(parent_flag(&calls[2]), None, "{calls:?}");
        assert_eq!(
            exported, "next-4",
            "`{reason}` did not export itself as root"
        );
    }

    let (calls, _) = drive(
        serde_json::json!([
            {"session": "root-1"},
            {"session": "child-3", "parent": "2026-10-14T00-00-00_root-1.jsonl", "reason": "startup"}
        ]),
        None,
    );
    assert_eq!(
        parent_flag(&calls[1]).as_deref(),
        Some("root-1"),
        "a child that began at startup lost its parent: {calls:?}"
    );
}

fn drive_pi_session(
    steps: serde_json::Value,
    session: &str,
    plan: serde_json::Value,
    inherited: Option<&str>,
) -> serde_json::Value {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    fs::write(
        base.join("gate.mjs"),
        plugin::pi_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("previous.jsonl"),
        "{\"type\":\"session\",\"version\":3,\"id\":\"prev-9\",\"timestamp\":\"t\",\"cwd\":\"/p\"}\n{\"type\":\"message\"}\n",
    )
    .expect("the previous session writes");
    fs::write(
        base.join("stub.cjs"),
        r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "session") {
  const plan = JSON.parse(process.env.STUB_PLAN);
  fs.appendFileSync(process.env.STUB_LOG, JSON.stringify(["session", ...process.argv.slice(2)]) + "\n");
  if (process.argv[2] === "deliver") {
    if (plan.deliver_exit) process.exit(plan.deliver_exit);
    fs.writeSync(1, plan.deliver ?? "");
    process.exit(0);
  }
  if (plan.hang) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 60000);
  else {
    if (plan.signal) process.kill(process.pid, plan.signal);
    fs.writeSync(1, plan.stdout ?? "");
    fs.writeSync(2, plan.stderr ?? "");
    process.exit(plan.exit ?? 0);
  }
}
"#,
    )
    .expect("the stand-in writes");
    fs::write(
        base.join("drive.mjs"),
        r#"import { readFileSync } from "node:fs";

process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
const handlers = {};
(await import("./gate.mjs")).default({ on: (name, handler) => { handlers[name] = handler; } });
const ctx = { cwd: process.cwd(), sessionManager: { getSessionId: () => process.env.OWN_SESSION } };
const results = [];
for (const [name, event] of JSON.parse(process.env.STEPS)) {
  const sent = { type: name, ...event };
  if (name === "session_start" && sent.previousSessionFile) sent.previousSessionFile = `${process.cwd()}/${sent.previousSessionFile}`;
  if (name === "before_agent_start") sent.systemPromptOptions = { sections: { other: "kept" } };
  const returned = await handlers[name](sent, ctx);
  results.push({ returned: returned ?? null, sections: sent.systemPromptOptions?.sections ?? null });
}
const logged = readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
console.log(JSON.stringify({ logged, results }));
"#,
    )
    .expect("the driver writes");
    let log = base.join("calls.log");
    fs::write(&log, "").expect("the log resets");
    let mut command = std::process::Command::new(&node);
    command
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", &log)
        .env("STUB_PLAN", plan.to_string())
        .env("STEPS", steps.to_string())
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("OWN_SESSION", session)
        .env_remove("NODE_OPTIONS")
        .env_remove("ESTIGIA_PI_PARENT_SESSION")
        .env_remove("ESTIGIA_PI_PARENT_PID");
    if let Some(id) = inherited {
        command
            .env("ESTIGIA_PI_PARENT_SESSION", id)
            .env("ESTIGIA_PI_PARENT_PID", "1");
    }
    let ran = command.output().expect("node runs");
    assert!(
        ran.status.success(),
        "the driver failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

#[test]
fn the_pi_extension_appends_the_binarys_session_context_to_the_prompt_sections_and_replaces_nothing()
 {
    let said = drive_pi_session(
        serde_json::json!([["session_start", {"reason": "startup"}], ["before_agent_start", {}]]),
        "sess-1",
        serde_json::json!({"stdout": "RUN CONTEXT\n"}),
        None,
    );
    assert_eq!(
        said["logged"][0],
        serde_json::json!([
            "session",
            "start",
            "--session-id",
            "sess-1",
            "--runtime",
            "pi",
            "--reason",
            "startup"
        ])
    );
    let run = &said["results"][1];
    assert_eq!(run["sections"]["estigia"], "RUN CONTEXT");
    assert_eq!(run["sections"]["other"], "kept");
    assert!(
        run["returned"].is_null(),
        "the handler replaced the prompt: {run}"
    );
}

#[test]
fn the_pi_extension_forwards_why_a_session_started_and_which_session_it_came_from() {
    let said = drive_pi_session(
        serde_json::json!([
            ["session_start", {"reason": "fork", "previousSessionFile": "previous.jsonl"}],
            ["before_agent_start", {}]
        ]),
        "sess-2",
        serde_json::json!({"stdout": "FORKED\n"}),
        None,
    );
    assert_eq!(
        said["logged"][0],
        serde_json::json!([
            "session",
            "start",
            "--session-id",
            "sess-2",
            "--runtime",
            "pi",
            "--reason",
            "fork",
            "--previous-session-id",
            "prev-9"
        ])
    );

    let said = drive_pi_session(
        serde_json::json!([
            ["session_start", {"reason": "fork", "previousSessionFile": "gone.jsonl"}],
            ["before_agent_start", {}]
        ]),
        "sess-2",
        serde_json::json!({"stdout": "FORKED\n"}),
        None,
    );
    assert_eq!(
        said["logged"][0],
        serde_json::json!([
            "session",
            "start",
            "--session-id",
            "sess-2",
            "--runtime",
            "pi",
            "--reason",
            "fork"
        ]),
        "a previous session that cannot be read was invented"
    );
}

#[test]
fn the_pi_extension_says_the_session_context_is_unknown_when_the_binary_does_not_answer() {
    for (plan, why) in [
        (serde_json::json!({"exit": 3, "stderr": "boom"}), "exit 3"),
        (
            serde_json::json!({"signal": "SIGKILL"}),
            if cfg!(windows) { "exit 1" } else { "SIGKILL" },
        ),
        (serde_json::json!({"hang": true}), "within 5s"),
    ] {
        let said = drive_pi_session(
            serde_json::json!([["session_start", {"reason": "startup"}], ["before_agent_start", {}]]),
            "sess-3",
            plan,
            None,
        );
        let text = said["results"][1]["sections"]["estigia"]
            .as_str()
            .expect("a section is always written");
        assert!(
            text.starts_with("Estigia: session context unknown (") && text.contains(why),
            "{why}: {text}"
        );
        assert_eq!(said["results"][1]["sections"]["other"], "kept");
    }
}

#[test]
fn the_pi_extension_asks_again_for_every_run_so_a_claim_made_since_is_never_stale() {
    let said = drive_pi_session(
        serde_json::json!([
            ["session_start", {"reason": "startup"}],
            ["before_agent_start", {}],
            ["before_agent_start", {}]
        ]),
        "sess-4",
        serde_json::json!({"stdout": "X\n"}),
        None,
    );
    assert_eq!(said["logged"].as_array().map(Vec::len), Some(4), "{said}");
}

#[test]
fn the_pi_extension_delivers_waiting_mail_as_its_own_prompt_section_at_before_agent_start() {
    let said = drive_pi_session(
        serde_json::json!([["session_start", {"reason": "startup"}], ["before_agent_start", {}]]),
        "sess-6",
        serde_json::json!({"stdout": "RUN CONTEXT\n", "deliver": "MAIL TEXT\n"}),
        None,
    );
    assert_eq!(
        said["logged"][1],
        serde_json::json!([
            "session",
            "deliver",
            "--session-id",
            "sess-6",
            "--runtime",
            "pi"
        ])
    );
    let run = &said["results"][1];
    assert_eq!(run["sections"]["estigia-mail"], "MAIL TEXT");
    assert_eq!(run["sections"]["estigia"], "RUN CONTEXT");
    assert_eq!(run["sections"]["other"], "kept");
}

#[test]
fn the_pi_extension_adds_no_mail_section_when_nothing_waits_or_the_mailbox_cannot_be_read() {
    for plan in [
        serde_json::json!({"stdout": "RUN CONTEXT\n"}),
        serde_json::json!({"stdout": "RUN CONTEXT\n", "deliver_exit": 3}),
    ] {
        let said = drive_pi_session(
            serde_json::json!([["session_start", {"reason": "startup"}], ["before_agent_start", {}]]),
            "sess-7",
            plan,
            None,
        );
        let sections = &said["results"][1]["sections"];
        assert_eq!(sections["estigia"], "RUN CONTEXT");
        assert!(sections.get("estigia-mail").is_none(), "{sections}");
    }
}

#[test]
fn the_pi_extension_ends_the_session_once_even_when_shutdown_fires_twice() {
    let said = drive_pi_session(
        serde_json::json!([
            ["session_start", {"reason": "startup"}],
            ["session_shutdown", {"reason": "quit"}],
            ["session_shutdown", {"reason": "quit"}]
        ]),
        "sess-5",
        serde_json::json!({}),
        None,
    );
    assert_eq!(
        said["logged"],
        serde_json::json!([[
            "session",
            "end",
            "--session-id",
            "sess-5",
            "--runtime",
            "pi",
            "--reason",
            "quit"
        ]])
    );
}

#[test]
fn a_pi_child_asks_about_the_root_session_and_never_ends_it() {
    let said = drive_pi_session(
        serde_json::json!([
            ["session_start", {"reason": "startup"}],
            ["before_agent_start", {}],
            ["session_shutdown", {"reason": "quit"}]
        ]),
        "child-7",
        serde_json::json!({"stdout": "ROOT CONTEXT\n"}),
        Some("root-1"),
    );
    assert_eq!(
        said["logged"],
        serde_json::json!([[
            "session",
            "start",
            "--session-id",
            "root-1",
            "--runtime",
            "pi",
            "--reason",
            "startup"
        ]])
    );
    assert_eq!(said["results"][1]["sections"]["estigia"], "ROOT CONTEXT");
}

const PI_REVIEW_STUB: &str = r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "review") {
  const args = process.argv.slice(2);
  const plan = JSON.parse(process.env.STUB_PLAN)[args[0]] ?? { exit: 0 };
  const stdin = plan.readStdin === false ? null : fs.readFileSync(0, "utf8");
  fs.appendFileSync(
    process.env.STUB_LOG,
    JSON.stringify({ args: ["review", ...args], cwd: process.cwd(), stdin }) + "\n",
  );
  fs.writeSync(1, plan.stdout ?? "");
  fs.writeSync(2, plan.stderr ?? "");
  process.exit(plan.exit ?? 0);
}
"#;

const PI_REVIEW_DRIVER: &str = r##"import { readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
const alive = setInterval(() => {}, 1000);

const calls = () =>
  readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const load = async (file) => {
  let tool = null;
  (await import(`./${file}`)).default({ registerTool: (definition) => { tool = definition; } });
  return tool;
};

const session = { provider: "p", id: "session-model" };
const listed = { provider: "anthropic", id: "claude-x" };

const run = async (label, file, name, plan, completion, size = 0, shape = {}) => {
  const tool = await load(file);
  process.env.STUB_PLAN = JSON.stringify(plan);
  const completed = [];
  const registry = {
    find: (provider, id) => (provider === listed.provider && id === listed.id ? listed : undefined),
    streamSimple: (model, context, options) => {
      completed.push({ model, context, hasSignal: options?.signal instanceof AbortSignal });
      return {
        result: async () => {
          if (completion.throws) throw new Error(completion.throws);
          if (completion.hangs) {
            await new Promise((_, reject) =>
              options.signal.addEventListener("abort", () => reject(new Error("aborted by signal"))),
            );
          }
          if (completion.lateMs) await new Promise((resolve) => setTimeout(resolve, completion.lateMs));
          const text = completion.text === undefined ? "ANSWER" : completion.text;
          return {
            stopReason: completion.stopReason ?? "stop",
            errorMessage: completion.errorMessage,
            content: [{ type: "text", text: text + "x".repeat(size) }],
          };
        },
      };
    },
  };
  if (shape.noStream) delete registry.streamSimple;
  const sessionId =
    shape.session === "throws"
      ? () => { throw new Error("no session"); }
      : () => (shape.session === "blank" ? "  " : "sess-0198");
  const ctx = {
    cwd: process.env.PROJECT,
    model: shape.noModel ? undefined : session,
    modelRegistry: registry,
    sessionManager: { getSessionId: sessionId },
  };
  const before = calls().length;
  let outcome;
  try {
    outcome = { result: await tool.execute("call-1", { name }, undefined, undefined, ctx) };
  } catch (error) {
    outcome = { error: String(error.message), code: error.code ?? null };
  }
  return { label, ...outcome, completed, calls: calls().slice(before) };
};

const ok = { prompt: { stdout: "PROMPT BYTES\n" }, model: { stdout: "" }, admit: {} };
const tool = await load("relay.mjs");

const out = [
  await run("inherit", "relay.mjs", "review-risk", ok, {}),
  await run("named", "relay.mjs", "review-risk", { ...ok, model: { stdout: "anthropic/claude-x\n" } }, {}),
  await run("unlisted", "relay.mjs", "review-risk", { ...ok, model: { stdout: "anthropic/nope\n" } }, {}),
  await run("bare", "relay.mjs", "review-risk", { ...ok, model: { stdout: "opus\n" } }, {}),
  await run("stranger", "relay.mjs", "general-purpose", ok, {}),
  await run("early", "relay.mjs", "review-risk", {
    ...ok,
    admit: { exit: 1, readStdin: false, stderr: JSON.stringify({ code: "capture-job-missing", message: "no job holds the slot" }) },
  }, {}, 2 * 1024 * 1024),
  await run("unavailable", "relay.mjs", "review-risk", {
    ...ok,
    admit: { exit: 3, stderr: JSON.stringify({ code: "authority_unavailable", message: "the store did not answer" }) },
  }, {}),
  await run("stale", "relay.mjs", "review-risk", {
    ...ok,
    admit: { exit: 1, stderr: JSON.stringify({ code: "capture-result-stale", message: "the receipt moved" }) },
  }, {}),
  await run("silent", "relay.mjs", "review-risk", { ...ok, admit: { exit: 1 } }, {}),
  await run("signal", "relay.mjs", "review-risk", { ...ok, prompt: { exit: 127, stderr: "gone" } }, {}),
  await run("empty", "relay.mjs", "review-risk", ok, { text: "  " }),
  await run("failed", "relay.mjs", "review-risk", ok, { stopReason: "error", errorMessage: "quota" }),
  await run("threw", "relay.mjs", "review-risk", ok, { throws: "boom" }),
  await run("nobinary", "missing.mjs", "review-risk", ok, {}),
  await run("deadline-abort", "short.mjs", "review-risk", ok, { hangs: true }),
  await run("deadline-late", "short.mjs", "review-risk", ok, { lateMs: 300 }),
  await run("aborted", "relay.mjs", "review-risk", ok, { stopReason: "aborted", errorMessage: "cancelled" }),
  await run("nostream", "relay.mjs", "review-risk", ok, {}, 0, { noStream: true }),
  await run("session-blank", "relay.mjs", "review-risk", ok, {}, 0, { session: "blank" }),
  await run("session-throws", "relay.mjs", "review-risk", ok, {}, 0, { session: "throws" }),
  await run("nomodel", "relay.mjs", "review-risk", ok, {}, 0, { noModel: true }),
];

const registered = {
  name: tool.name,
  required: tool.parameters.required,
  names: tool.parameters.properties.name.enum,
};

clearInterval(alive);
console.log(JSON.stringify({ registered, out }));
"##;

fn run_pi_review_relay_driver() -> serde_json::Value {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    let project = base.join("project");
    fs::create_dir_all(&project).expect("the project directory");
    fs::write(
        base.join("relay.mjs"),
        plugin::pi_review_relay_source(std::path::Path::new(&node)),
    )
    .expect("the relay writes");
    fs::write(
        base.join("missing.mjs"),
        plugin::pi_review_relay_source(&base.join("no-such-estigia")),
    )
    .expect("the relay writes");
    let deadline = format!(
        "const DEADLINE_MS = {};",
        crate::reviewer::DEFAULT_DEADLINE.as_millis()
    );
    let relay = plugin::pi_review_relay_source(std::path::Path::new(&node));
    assert!(relay.contains(&deadline));
    fs::write(
        base.join("short.mjs"),
        relay.replace(&deadline, "const DEADLINE_MS = 100;"),
    )
    .expect("the short-deadline relay writes");
    fs::write(base.join("stub.cjs"), PI_REVIEW_STUB).expect("the stand-in writes");
    fs::write(base.join("drive.mjs"), PI_REVIEW_DRIVER).expect("the driver writes");
    let ran = std::process::Command::new(&node)
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.log"))
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("PROJECT", &project)
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the Pi review relay would not load or the driver failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let mut parsed: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    parsed["project"] = serde_json::Value::String(project.to_string_lossy().into_owned());
    parsed
}

fn pi_review_case<'a>(driven: &'a serde_json::Value, label: &str) -> &'a serde_json::Value {
    driven["out"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["label"] == label))
        .unwrap_or_else(|| panic!("no result for {label}"))
}

fn review_args(action: &str, name: &str) -> serde_json::Value {
    serde_json::json!([
        "review",
        action,
        "--name",
        name,
        "--session-id",
        "sess-0198",
        "--runtime",
        "pi"
    ])
}

#[test]
fn the_pi_review_relay_materialises_the_prompt_and_admits_the_answer() {
    let driven = run_pi_review_relay_driver();
    let project = driven["project"].clone();

    let registered = &driven["registered"];
    assert_eq!(registered["name"], plugin::PI_REVIEW_TOOL);
    assert_eq!(registered["required"], serde_json::json!(["name"]));
    let names: Vec<&str> = crate::reviewer::Role::ALL
        .iter()
        .map(|role| role.name())
        .collect();
    assert_eq!(registered["names"], serde_json::json!(names));

    let inherit = pi_review_case(&driven, "inherit");
    assert!(inherit["error"].is_null(), "{inherit}");
    let note = inherit["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        note.contains("admitted") && !note.contains("ANSWER"),
        "the answer reached the model that asked: {note}"
    );
    assert_eq!(
        inherit["calls"],
        serde_json::json!([
            { "args": review_args("prompt", "review-risk"), "cwd": project, "stdin": "" },
            { "args": review_args("model", "review-risk"), "cwd": project, "stdin": "" },
            { "args": review_args("admit", "review-risk"), "cwd": project, "stdin": "ANSWER" },
        ])
    );
    let completed = inherit["completed"].as_array().expect("the completions");
    assert_eq!(completed.len(), 1, "{inherit}");
    assert_eq!(
        completed[0]["model"],
        serde_json::json!({ "provider": "p", "id": "session-model" })
    );
    assert_eq!(
        completed[0]["context"]["messages"][0]["content"][0]["text"],
        "PROMPT BYTES\n"
    );
    assert!(
        completed[0]["context"].get("systemPrompt").is_none()
            && completed[0]["context"].get("tools").is_none()
            && completed[0]["hasSignal"] == true,
        "{inherit}"
    );

    let named = pi_review_case(&driven, "named");
    assert_eq!(
        named["completed"][0]["model"],
        serde_json::json!({ "provider": "anthropic", "id": "claude-x" })
    );

    for label in ["unlisted", "bare"] {
        let case = pi_review_case(&driven, label);
        assert_eq!(case["code"], "reviewer-unavailable", "{case}");
        assert_eq!(case["completed"], serde_json::json!([]), "{case}");
        assert_eq!(case["calls"].as_array().map(Vec::len), Some(2), "{case}");
    }

    let stranger = pi_review_case(&driven, "stranger");
    assert_eq!(stranger["code"], "reviewer-unavailable", "{stranger}");
    assert_eq!(stranger["calls"], serde_json::json!([]), "{stranger}");
}

#[test]
fn the_pi_review_relay_names_a_missed_deadline_an_aborted_completion_and_a_registry_that_cannot_complete()
 {
    let driven = run_pi_review_relay_driver();
    let admissions = |case: &serde_json::Value| {
        case["calls"].as_array().map(|calls| {
            calls
                .iter()
                .filter(|call| call["args"][1] == "admit")
                .count()
        })
    };
    for label in ["deadline-abort", "deadline-late"] {
        let case = pi_review_case(&driven, label);
        assert_eq!(case["code"], "reviewer-deadline", "{label}: {case}");
        assert_eq!(
            case["completed"].as_array().map(Vec::len),
            Some(1),
            "{label}: {case}"
        );
        assert_eq!(admissions(case), Some(0), "{label} admitted an answer");
    }
    let aborted = pi_review_case(&driven, "aborted");
    assert_eq!(aborted["code"], "reviewer-error-result", "{aborted}");
    assert!(
        aborted["error"]
            .as_str()
            .is_some_and(|text| text.contains("ended aborted") && text.contains("cancelled")),
        "{aborted}"
    );
    assert_eq!(admissions(aborted), Some(0));
    let nostream = pi_review_case(&driven, "nostream");
    assert_eq!(nostream["code"], "reviewer-unavailable", "{nostream}");
    assert!(
        nostream["error"]
            .as_str()
            .is_some_and(|text| text.contains("completes")),
        "{nostream}"
    );
    assert_eq!(nostream["calls"].as_array().map(Vec::len), Some(2));
}

#[test]
fn the_pi_review_relay_leaves_out_the_session_flags_it_cannot_fill_and_refuses_inherit_with_no_model()
 {
    let driven = run_pi_review_relay_driver();
    let project = driven["project"].clone();
    for label in ["session-blank", "session-throws"] {
        let case = pi_review_case(&driven, label);
        assert!(case["error"].is_null(), "{label}: {case}");
        assert_eq!(
            case["calls"],
            serde_json::json!([
                { "args": ["review", "prompt", "--name", "review-risk"], "cwd": project, "stdin": "" },
                { "args": ["review", "model", "--name", "review-risk"], "cwd": project, "stdin": "" },
                { "args": ["review", "admit", "--name", "review-risk"], "cwd": project, "stdin": "ANSWER" },
            ]),
            "{label}"
        );
    }
    let nomodel = pi_review_case(&driven, "nomodel");
    assert_eq!(nomodel["code"], "reviewer-unavailable", "{nomodel}");
    assert!(
        nomodel["error"]
            .as_str()
            .is_some_and(|text| text.contains("has none")),
        "{nomodel}"
    );
    assert_eq!(nomodel["completed"], serde_json::json!([]));
    assert_eq!(nomodel["calls"].as_array().map(Vec::len), Some(2));
}

#[test]
fn the_pi_review_relay_keeps_the_reason_when_the_binary_exits_before_reading_stdin() {
    let driven = run_pi_review_relay_driver();
    let early = pi_review_case(&driven, "early");
    assert_eq!(early["code"], "capture-job-missing", "{early}");
    assert!(
        early["error"]
            .as_str()
            .is_some_and(|text| text.contains("no job holds the slot")),
        "{early}"
    );
}

#[test]
fn the_pi_review_relay_keeps_unavailable_and_stale_apart() {
    let driven = run_pi_review_relay_driver();

    let unavailable = pi_review_case(&driven, "unavailable");
    assert_eq!(
        unavailable["code"], "authority_unavailable",
        "{unavailable}"
    );
    let stale = pi_review_case(&driven, "stale");
    assert_eq!(stale["code"], "capture-result-stale", "{stale}");
    let silent = pi_review_case(&driven, "silent");
    assert_eq!(silent["code"], "transport-refused", "{silent}");

    for label in ["signal", "nobinary"] {
        let case = pi_review_case(&driven, label);
        assert_eq!(case["code"], "authority_unavailable", "{label}: {case}");
        assert_eq!(case["completed"], serde_json::json!([]), "{label}: {case}");
    }
    assert_eq!(
        pi_review_case(&driven, "nobinary")["calls"],
        serde_json::json!([])
    );

    let admissions = |label: &str| {
        pi_review_case(&driven, label)["calls"]
            .as_array()
            .map(|calls| {
                calls
                    .iter()
                    .filter(|call| call["args"][1] == "admit")
                    .count()
            })
    };
    for (label, code) in [
        ("empty", "reviewer-no-answer"),
        ("failed", "reviewer-error-result"),
        ("threw", "reviewer-error-result"),
    ] {
        let case = pi_review_case(&driven, label);
        assert_eq!(case["code"], code, "{label}: {case}");
        assert_eq!(admissions(label), Some(0), "{label} admitted an answer");
    }
}

#[test]
fn a_review_relay_is_installed_when_either_host_has_written_its_own() {
    let (home, options) = sandbox();
    let roaming = home.path().join("AppData").join("Roaming");
    let config = home.path().join(".config");
    with_home_env(home.path(), &roaming, &config, || {
        assert!(!review_relay_installed());

        setup(agent("claude-code"), &Config::default(), &options).expect("setup succeeds");
        assert!(
            !review_relay_installed(),
            "a host with no relay installed one"
        );

        setup(agent("pi"), &Config::default(), &options).expect("setup succeeds");
        assert!(review_relay_installed());
        uninstall(agent("pi"), &options).expect("uninstall succeeds");
        assert!(!review_relay_installed());

        setup(agent("opencode"), &Config::default(), &options).expect("setup succeeds");
        assert!(review_relay_installed());
        uninstall(agent("opencode"), &options).expect("uninstall succeeds");
        assert!(!review_relay_installed());
    });
}

#[test]
fn a_review_relay_estigia_did_not_write_is_neither_replaced_nor_removed_for_pi() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    let file = review_plugin_path(adapter, &paths).expect("Pi has a review relay");
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default function () {}\n";
    fs::write(&file, theirs).expect("write their extension");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
}

#[test]
fn only_a_host_with_a_review_relay_has_a_review_relay_path() {
    let (_home, options) = sandbox();
    for adapter in every_adapter() {
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        assert_eq!(
            review_plugin_path(adapter, &paths).is_some(),
            adapter.capabilities().review_relay,
            "{} has a review relay path it does not declare, or declares one with no path",
            adapter.slug
        );
    }
}

#[test]
fn a_fresh_pi_adapter_file_names_the_review_tool_and_no_other_adapter_does() {
    let dir = tempfile::tempdir().expect("a temporary skill root");
    let pi = dir.path().join("estigia.pi.md");
    let other = dir.path().join("estigia.opencode.md");
    write_agent_configuration_wholly(&pi, "pi", &Config::default()).expect("pi's file writes");
    write_agent_configuration_wholly(&other, "opencode", &Config::default())
        .expect("opencode's file writes");
    let text = fs::read_to_string(&pi).expect("pi's file reads");
    assert!(text.contains(plugin::PI_REVIEW_TOOL), "{text}");
    assert!(text.contains("<!-- estigia:pi-review:start -->"), "{text}");
    assert!(text.contains("`reviewer` routing row"), "{text}");
    assert!(!text.contains("OpenCode"), "{text}");
    let other = fs::read_to_string(&other).expect("opencode's file reads");
    assert!(!other.contains(plugin::PI_REVIEW_TOOL), "{other}");
}

#[test]
fn an_upgraded_pi_adapter_file_refreshes_its_review_section_and_keeps_the_operators() {
    let dir = tempfile::tempdir().expect("a temporary skill root");
    let pi = dir.path().join("estigia.pi.md");
    let theirs = "## Review on this host\n\nThe operator's own notes on reviewing.\n";
    fs::write(&pi, theirs).expect("the file writes");
    write_agent_configuration_wholly(&pi, "pi", &Config::default()).expect("pi's file upgrades");
    write_agent_configuration_wholly(&pi, "pi", &Config::default()).expect("and again");
    let text = fs::read_to_string(&pi).expect("pi's file reads");
    assert!(
        text.contains(theirs),
        "the operator's section was cut: {text}"
    );
    assert_eq!(
        text.matches("<!-- estigia:pi-review:start -->").count(),
        1,
        "{text}"
    );
    assert_eq!(text.matches(plugin::PI_REVIEW_TOOL).count(), 1, "{text}");
}

#[test]
fn the_pi_review_relay_reads_its_tool_and_reviewer_names_from_the_binary() {
    let source = plugin::pi_review_relay_source(std::path::Path::new("/opt/estigia"));
    assert!(source.contains(&format!("const TOOL = \"{}\";", plugin::PI_REVIEW_TOOL)));
    for role in crate::reviewer::Role::ALL {
        assert!(source.contains(role.name()), "{}", role.name());
    }
    assert!(source.contains(&format!(
        "const DEADLINE_MS = {};",
        crate::reviewer::DEFAULT_DEADLINE.as_millis()
    )));
    assert!(!source.contains("__"), "a placeholder was left unfilled");
}

#[test]
fn the_review_relays_share_one_reading_of_the_exit_codes() {
    let opencode = run_review_relay_driver();
    let pi = run_pi_review_relay_driver();
    for (opencode_key, pi_label, code) in [
        ("unavailable_code", "unavailable", "authority_unavailable"),
        ("stale_code", "stale", "capture-result-stale"),
        ("silent_code", "silent", "transport-refused"),
        ("signal_code", "signal", "authority_unavailable"),
    ] {
        assert_eq!(opencode[opencode_key], code, "opencode {opencode_key}");
        assert_eq!(pi_review_case(&pi, pi_label)["code"], code, "pi {pi_label}");
    }
}

#[test]
fn pi_installs_the_loop_extension_only_when_the_setting_is_on() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let directory = home
        .path()
        .join(".pi")
        .join("agent")
        .join(plugin::PI_DIRECTORY);
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("Pi names a loop extension path");
    assert_eq!(loop_file, directory.join(plugin::PI_LOOP_FILE));
    assert!(!loop_file.exists(), "resolving a path wrote it");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        !loop_file.exists(),
        "the default setting wrote the loop extension"
    );
    let bytes_of = |name: &str| fs::read_to_string(directory.join(name)).expect("the file reads");
    let gate = bytes_of(plugin::PI_FILE);
    let relay = bytes_of(plugin::PI_REVIEW_FILE);

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    let written = fs::read_to_string(&loop_file).expect("the loop extension reads");
    assert!(plugin::loop_is_ours(Some(&written)));
    assert!(written.contains("registerCommand"));
    assert_eq!(bytes_of(plugin::PI_FILE), gate);
    assert_eq!(bytes_of(plugin::PI_REVIEW_FILE), relay);

    let again = setup(adapter, &loop_on(), &options).expect("the second setup succeeds");
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.change == Change::Unchanged),
        "{:?}",
        again.actions
    );

    let both = Config {
        goal_loop_plugin: crate::config::LoopPlugin::On,
        ..loop_on()
    };
    setup(adapter, &both, &options).expect("setup with both rows on succeeds");
    assert!(
        !directory.join(plugin::GOAL_LOOP_FILE).exists(),
        "the OpenCode-named generic goal loop was written for Pi, which does not read it"
    );
    assert!(
        !directory.join(plugin::LEGACY_LOOP_FILE).exists()
            && !directory.join("flow-loop.js").exists()
    );
    assert_eq!(
        fs::read_to_string(directory.join(plugin::PI_GOAL_LOOP_FILE))
            .expect("the generic goal loop extension reads"),
        plugin::PI_GOAL_LOOP_SOURCE
    );
    assert_eq!(
        fs::read_to_string(&loop_file).expect("the loop extension survives"),
        written
    );

    setup(adapter, &Config::default(), &options).expect("switching the setting off succeeds");
    assert!(!loop_file.exists(), "turning the loop off left the file");
    assert!(
        !directory.join(plugin::PI_GOAL_LOOP_FILE).exists(),
        "turning both rows off left the generic goal loop extension"
    );
    assert_eq!(bytes_of(plugin::PI_FILE), gate);
    assert_eq!(bytes_of(plugin::PI_REVIEW_FILE), relay);
}

#[test]
fn a_pi_loop_extension_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("Pi names a loop extension path");
    fs::create_dir_all(loop_file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default function () {}\n";
    fs::write(&loop_file, theirs).expect("write their extension");

    setup(adapter, &loop_on(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&loop_file).expect("it survives"), theirs);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&loop_file).expect("it survives"), theirs);
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&loop_file).expect("it survives"), theirs);
}

#[test]
fn uninstall_takes_back_the_pi_loop_extension_even_when_this_install_did_not_ask_for_it() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    let loop_file = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("Pi names a loop extension path");
    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    assert!(loop_file.is_file());

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!loop_file.exists(), "uninstall left the loop extension");
}

#[test]
fn only_the_adapters_that_run_a_loop_are_offered_the_loop_row() {
    let pi = agent("pi");
    assert!(pi.offers_loop());
    assert!(
        pi.has_loop_plugin(),
        "Pi reads the generic goal and loop extension"
    );
    assert!(capabilities::capabilities_for_slug("pi").offers_loop);
    assert!(capabilities::capabilities_for_slug("pi").loop_plugin);
}

fn pi_loop_files() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    fs::create_dir_all(base.join("project")).expect("the project directory");
    fs::write(
        base.join("pi-loop.mjs"),
        plugin::pi_loop_source(std::path::Path::new(&node)),
    )
    .expect("the extension writes");
    fs::write(
        base.join("missing.mjs"),
        plugin::pi_loop_source(&base.join("no-such-estigia")),
    )
    .expect("the extension writes");
    fs::write(base.join("stub.cjs"), PI_LOOP_STUB).expect("the stand-in writes");
    fs::write(base.join("drive.mjs"), PI_LOOP_DRIVER).expect("the driver writes");
    (root, base, node)
}

fn run_pi_loop_driver() -> &'static serde_json::Value {
    static CASES: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    CASES.get_or_init(|| {
        let (_root, base, node) = pi_loop_files();
        let ran = std::process::Command::new(&node)
            .arg(base.join("drive.mjs"))
            .current_dir(&base)
            .env("STUB_LOG", base.join("calls.log"))
            .env(
                "STUB_PATH",
                base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
            )
            .env("PROJECT", base.join("project"))
            .env_remove("NODE_OPTIONS")
            .output()
            .expect("node runs");
        assert!(
            ran.status.success(),
            "the loop extension would not load or the driver failed: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
    })
}

fn strings(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect()
}

const PI_LOOP_STUB: &str = r#"const fs = require("node:fs");
const path = require("node:path");
const Module = require("node:module");
if (path.basename(process.argv[1] ?? "") === "goal") {
  const load = Module._load;
  Module._load = function (request, parent, isMain) {
    if (isMain && path.basename(String(request)) === "goal") return {};
    return load.apply(this, arguments);
  };
  fs.appendFileSync(
    process.env.STUB_LOG,
    JSON.stringify({ args: process.argv.slice(2), cwd: process.cwd() }) + "\n",
  );
  const mode = process.env.STUB_MODE ?? "";
  const finish = () => {
    fs.writeSync(1, process.env.STUB_ANSWER ?? "");
    process.exit(Number(process.env.STUB_EXIT ?? 0));
  };
  if (mode === "hang") setInterval(() => {}, 1000);
  else if (mode === "signal") process.kill(process.pid, "SIGKILL");
  else if (mode === "delay") setTimeout(finish, Number(process.env.STUB_DELAY_MS ?? 0));
  else finish();
}
"#;

const PI_LOOP_DRIVER: &str = r#"import { readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;

const real = { setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout };
let held = [];
globalThis.setTimeout = (fn, delay, ...rest) => {
  if (typeof delay === "number" && delay >= 30000) {
    const entry = { fn, delay };
    held.push(entry);
    return entry;
  }
  return real.setTimeout(fn, delay, ...rest);
};
globalThis.clearTimeout = (handle) => {
  const at = held.indexOf(handle);
  if (at >= 0) held.splice(at, 1);
  else real.clearTimeout(handle);
};

const sleep = (ms) => new Promise((resolve) => real.setTimeout(resolve, ms));
const until = async (test) => {
  for (let i = 0; i < 1000; i++) {
    if (test()) return true;
    await sleep(10);
  }
  return false;
};
const calls = () =>
  readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const use = ({ answer = null, exit = 0, mode = "", delay = 0 }) => {
  process.env.STUB_ANSWER = answer === null ? "" : JSON.stringify(answer);
  process.env.STUB_EXIT = String(exit);
  process.env.STUB_MODE = mode;
  process.env.STUB_DELAY_MS = String(delay);
};

const LIVE = { run_id: "pi-abc", live: true, issue: 42, standing: "held" };
const DEAD = {
  run_id: "pi-abc",
  live: false,
  standing: "nothing-sworn",
  claimable: 0,
  dischargeable: 0,
  held: 0,
  pending: 0,
  unconditioned: 0,
  unadmitted: 0,
};

let runs = 0;
const scenario = async ({ file = "pi-loop.mjs", id = "sess-1", header = {} } = {}) => {
  held = [];
  const handlers = {};
  const commands = {};
  const sent = [];
  const notes = [];
  const state = { id, header, idle: true };
  const pi = {
    on: (name, handler) => {
      handlers[name] = handler;
    },
    registerCommand: (name, options) => {
      commands[name] = options;
    },
    sendMessage: (message, options) => {
      sent.push({ message, options });
    },
  };
  const loaded = await import(`./${file}?run=${runs++}`);
  loaded.default(pi);
  const strangers = [];
  const loadAgain = () =>
    loaded.default({
      on: () => {},
      registerCommand: () => {},
      sendMessage: (message) => strangers.push(message),
    });
  const ctx = {
    cwd: process.env.PROJECT,
    sessionManager: { getSessionId: () => state.id, getHeader: () => state.header },
    isIdle: () => state.idle,
    ui: { notify: (text, level) => notes.push({ text, level }) },
  };
  const mark = calls().length;
  return { handlers, commands, sent, notes, state, ctx, strangers, loadAgain, asked: () => calls().slice(mark) };
};

const start = (s, text) => s.commands["flow-loop"].handler(text, s.ctx);
const turn = (s, aborted = false) => s.handlers.agent_settled({ type: "agent_settled", aborted }, s.ctx);
const texts = (s) => s.sent.map((entry) => entry.message.content);
const said = (s) => s.notes.map((note) => note.text);
const snapshot = (s) => ({
  prompts: s.sent.length,
  texts: texts(s),
  said: said(s),
  asked: s.asked().length,
  timers: held.map((entry) => entry.delay),
});

const out = {};

{
  const s = await scenario();
  out.registered = {
    commands: Object.keys(s.commands),
    described: Object.values(s.commands).every((c) => typeof c.description === "string" && c.description !== ""),
    events: Object.keys(s.handlers).sort(),
  };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "do the thing");
  await turn(s);
  out.live = {
    ...snapshot(s),
    options: s.sent.map((entry) => entry.options),
    customTypes: s.sent.map((entry) => entry.message.customType),
    args: s.asked().map((call) => call.args),
    cwds: s.asked().map((call) => call.cwd),
  };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "do the thing");
  use({ answer: { ...LIVE, issue: null } });
  await turn(s);
  out.live_no_issue = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "do the thing");
  use({ answer: DEAD });
  await turn(s);
  const stopped = snapshot(s);
  await turn(s);
  out.dead = { ...stopped, afterStop: snapshot(s) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: { ...DEAD, claimable: 2 } });
  await turn(s);
  out.claimable = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: { ...DEAD, dischargeable: 1 } });
  await turn(s);
  out.dischargeable = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "do it --max 2");
  use({ answer: { ...LIVE, completed: 1 } });
  await turn(s);
  const below = snapshot(s);
  use({ answer: { ...LIVE, completed: 2 } });
  await turn(s);
  out.cap = { below, at: snapshot(s), args: s.asked().map((call) => call.args) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x --max 2");
  use({ answer: { ...LIVE } });
  await turn(s);
  out.completion_unknown = snapshot(s);
}

{
  use({ answer: { ...DEAD, held: 1 } });
  const s = await scenario();
  await start(s, "x");
  await turn(s);
  s.loadAgain();
  use({ answer: { ...DEAD, claimable: 1 } });
  held.shift().fn();
  await until(() => s.sent.length === 2 || s.strangers.length > 0);
  out.timer_after_reload = { prompts: s.sent.length, strangers: s.strangers.length };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: { ...DEAD, claimable: 2, stop_after: "finish the current task" } });
  await turn(s);
  const stopped = snapshot(s);
  const calls = s.asked().map((call) => call.args);
  await turn(s);
  out.stop_after_midloop = { ...stopped, calls, afterStop: snapshot(s) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: { ...DEAD, claimable: 2, stop_after: "" } });
  await turn(s);
  out.stop_after_empty = { ...snapshot(s), calls: s.asked().map((call) => call.args) };
}

{
  use({ answer: { ...LIVE, stop_after: "left over from before" } });
  const s = await scenario();
  await start(s, "x");
  out.stop_after_at_start = { ...snapshot(s), calls: s.asked().map((call) => call.args) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  await start(s, "stop");
  await turn(s);
  out.stopped_by_operator = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  await start(s, "again");
  out.already_running = snapshot(s);
}

{
  use({ answer: { ...DEAD, held: 1 } });
  const s = await scenario();
  await start(s, "x");
  await turn(s);
  const waiting = snapshot(s);
  use({ answer: { ...DEAD, claimable: 1 } });
  const due = held.shift();
  due.fn();
  await until(() => s.sent.length === 2);
  const resumed = snapshot(s);
  use({ answer: { ...DEAD, pending: 1 } });
  await turn(s);
  const waitingAgain = held.length;
  await s.handlers.session_shutdown({ type: "session_shutdown" }, s.ctx);
  out.waits = { waiting, resumed, waitingAgain, afterShutdown: held.length };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ exit: 3 });
  await turn(s);
  const waiting = () => held.filter((entry) => entry.delay !== 30000);
  const delays = [waiting().map((entry) => entry.delay)];
  for (let round = 0; round < 2; round++) {
    const due = waiting()[0];
    held.splice(held.indexOf(due), 1);
    due.fn();
    await until(() => waiting().length === 1 || s.notes.length > 0);
    delays.push(waiting().map((entry) => entry.delay));
  }
  out.unreadable = { ...snapshot(s), delays };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: { ...LIVE, standing: "unread" } });
  await turn(s);
  out.unconfirmed = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ mode: "hang" });
  const before = calls().length;
  const decision = turn(s);
  await until(() => calls().length > before);
  held.find((entry) => entry.delay === 30000).fn();
  await decision;
  out.timeout = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ mode: "signal" });
  await turn(s);
  out.no_exit_code = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario({ file: "missing.mjs" });
  await start(s, "x");
  out.missing_binary = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  use({ answer: LIVE, mode: "delay", delay: 300 });
  const decision = turn(s);
  await until(() => s.asked().length === 2);
  s.state.id = "sess-2";
  await decision;
  out.session_changed = { ...snapshot(s), asked: s.asked().map((call) => call.args[2]) };
}

{
  use({ answer: LIVE });
  const s = await scenario({ header: { parentSession: "/sessions/parent.jsonl" } });
  await start(s, "x");
  out.child_by_parent = snapshot(s);
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  const before = s.asked().length;
  await turn(s, true);
  const aborted = snapshot(s);
  await turn(s);
  out.aborted = { ...aborted, before, afterStop: snapshot(s) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x");
  const before = s.asked().length;
  s.state.idle = false;
  await turn(s);
  const busy = snapshot(s);
  s.state.idle = true;
  await turn(s);
  out.busy = { before, busy, idle: snapshot(s) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await turn(s);
  out.no_loop = snapshot(s);
}

{
  const defaults = {};
  for (const [label, given] of [
    ["empty", ""],
    ["text", "do the thing"],
    ["named", "flow dev 12"],
    ["slashed", "/flow triage x"],
  ]) {
    use({ answer: LIVE });
    const s = await scenario();
    await start(s, given);
    defaults[label] = texts(s)[0].split("\n\n").slice(1).join("\n\n");
  }
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x --wait 0");
  out.defaults = { ...defaults, refused: snapshot(s) };
}

{
  use({ answer: LIVE });
  const s = await scenario();
  await start(s, "x --max 3 --wait 9");
  const status = s.commands["flow-status"].handler("", s.ctx);
  await status;
  out.status = { texts: texts(s), said: said(s) };
}

console.log(JSON.stringify(out));
process.exit(0);
"#;

#[test]
fn the_pi_loop_registers_its_commands_and_listens_for_the_settled_turn() {
    let cases = run_pi_loop_driver();
    assert_eq!(
        cases["registered"]["commands"],
        serde_json::json!(["flow-loop", "flow-status"])
    );
    assert_eq!(cases["registered"]["described"], true);
    assert_eq!(
        cases["registered"]["events"],
        serde_json::json!(["agent_settled", "session_shutdown"])
    );
    let source = plugin::pi_loop_source(std::path::Path::new("/opt/estigia"));
    assert!(!source.contains("__"), "a placeholder was left unfilled");
    assert!(
        !source.contains("agent_end"),
        "the loop listens for the turn before Pi has finished retrying or compacting"
    );
}

#[test]
fn the_pi_loop_continues_while_estigia_says_there_is_work() {
    let cases = run_pi_loop_driver();

    let live = &cases["live"];
    assert_eq!(live["prompts"], 2, "{live}");
    assert_eq!(
        live["customTypes"],
        serde_json::json!(["estigia-loop", "estigia-loop"])
    );
    assert_eq!(
        live["options"],
        serde_json::json!([
            { "triggerTurn": true, "deliverAs": "followUp" },
            { "triggerTurn": true, "deliverAs": "followUp" },
        ]),
        "the continuation did not start a turn: {live}"
    );
    let texts = strings(&live["texts"]);
    assert!(
        texts.iter().all(|text| text.contains("pi-abc")),
        "{texts:?}"
    );
    assert!(texts[0].contains("do the thing"));
    assert!(texts.iter().all(|text| !text.contains("--max")));
    assert_eq!(live["asked"], 2);
    for call in live["args"].as_array().expect("the calls") {
        assert_eq!(
            call,
            &serde_json::json!(["--json", "--session-id", "sess-1", "--runtime", "pi"])
        );
    }
    assert!(
        live["cwds"]
            .as_array()
            .expect("the cwds")
            .iter()
            .all(|cwd| cwd.as_str().is_some_and(|cwd| cwd.ends_with("project")))
    );

    let dead = &cases["dead"];
    assert_eq!(
        dead["prompts"], 1,
        "a run with nothing left continued: {dead}"
    );
    assert!(
        strings(&dead["said"])
            .iter()
            .any(|line| line.contains("no claimable or dischargeable")),
        "{dead}"
    );
    assert_eq!(
        dead["afterStop"]["asked"], dead["asked"],
        "a stopped loop asked again on the next turn: {dead}"
    );

    assert_eq!(cases["claimable"]["prompts"], 2, "{}", cases["claimable"]);
    let discharge = &cases["dischargeable"];
    assert_eq!(discharge["prompts"], 2, "{discharge}");
    assert!(
        strings(&discharge["texts"])[1].contains("discharge one"),
        "{discharge}"
    );

    let cap = &cases["cap"];
    assert_eq!(cap["below"]["prompts"], 2, "{cap}");
    assert_eq!(cap["at"]["prompts"], 2, "the cap was not honoured: {cap}");
    assert!(
        strings(&cap["at"]["said"])
            .iter()
            .any(|line| line.contains("completion cap")),
        "{cap}"
    );
    assert!(
        cap["args"]
            .as_array()
            .expect("the calls")
            .iter()
            .skip(1)
            .all(|args| strings(args).contains(&"--completed"))
    );
    let unknown = &cases["completion_unknown"];
    assert_eq!(unknown["prompts"], 1, "{unknown}");
    assert!(
        strings(&unknown["said"])
            .iter()
            .any(|line| line.contains("did not report how many")),
        "{unknown}"
    );

    let operator = &cases["stopped_by_operator"];
    assert_eq!(operator["prompts"], 1, "{operator}");
    assert_eq!(
        operator["asked"], 1,
        "a stopped loop asked again: {operator}"
    );
    assert_eq!(cases["already_running"]["prompts"], 1);
}

/// A turn that ends while the run still holds a claim continues with the held
/// text alone, byte for byte what the `Stop` hooks hand back.
#[test]
fn the_pi_loop_continues_a_live_claim_with_the_held_text_alone() {
    let cases = run_pi_loop_driver();
    let texts = strings(&cases["live"]["texts"]);
    let held = crate::harness::hook::stop_block(&crate::identity::Identity::Number(42), "pi-abc");
    assert_eq!(
        texts.get(1).copied(),
        held["reason"].as_str(),
        "a live claim was not continued with the held text: {texts:?}"
    );
    assert!(!texts[1].contains("/flow"), "{texts:?}");
}

/// A live answer that names no issue stops the loop and says so.
#[test]
fn the_pi_loop_stops_on_a_live_answer_that_names_no_issue() {
    let cases = run_pi_loop_driver();
    let case = &cases["live_no_issue"];
    assert_eq!(case["prompts"], 1, "{case}");
    assert!(
        strings(&case["said"])
            .iter()
            .any(|line| line.contains("did not report which issue")),
        "{case}"
    );
}

#[test]
fn the_pi_loop_waits_on_held_or_pending_work_and_asks_again() {
    let cases = run_pi_loop_driver();

    let waits = &cases["waits"];
    assert_eq!(waits["waiting"]["prompts"], 1, "{waits}");
    assert_eq!(waits["waiting"]["timers"], serde_json::json!([300_000]));
    assert_eq!(
        waits["resumed"]["prompts"], 2,
        "the re-ask did not continue once work appeared: {waits}"
    );
    assert_eq!(waits["resumed"]["asked"], 3, "{waits}");
    assert_eq!(waits["waitingAgain"], 1, "{waits}");
    assert_eq!(
        waits["afterShutdown"], 0,
        "a timer outlived the session: {waits}"
    );

    let unreadable = &cases["unreadable"];
    assert_eq!(
        unreadable["delays"],
        serde_json::json!([[300_000], [300_000], []]),
        "{unreadable}"
    );
    assert_eq!(unreadable["asked"], 4, "{unreadable}");
    assert_eq!(unreadable["prompts"], 1, "{unreadable}");
    assert!(
        strings(&unreadable["said"])
            .iter()
            .any(|line| line.contains("after 2 retries")),
        "{unreadable}"
    );

    let unconfirmed = &cases["unconfirmed"];
    assert_eq!(unconfirmed["prompts"], 1, "{unconfirmed}");
    assert_eq!(unconfirmed["timers"], serde_json::json!([300_000]));
}

#[test]
fn the_pi_loop_stops_and_says_so_when_estigia_does_not_answer() {
    let cases = run_pi_loop_driver();
    for (label, word) in [
        ("timeout", "gave no answer within 30 seconds"),
        ("no_exit_code", "no exit code"),
    ]
    .into_iter()
    .filter(|(label, _)| !(cfg!(windows) && *label == "no_exit_code"))
    {
        let case = &cases[label];
        assert_eq!(case["prompts"], 1, "{label} counted as work: {case}");
        assert_eq!(
            case["timers"],
            serde_json::json!([]),
            "{label} was retried as if it were a wait: {case}"
        );
        assert!(
            strings(&case["said"])
                .iter()
                .any(|line| line.contains(word)),
            "{label} was not said: {case}"
        );
    }
    let missing = &cases["missing_binary"];
    assert_eq!(missing["prompts"], 0, "{missing}");
    assert!(
        strings(&missing["said"])
            .iter()
            .any(|line| line.contains("did not start") && line.contains("could not be run")),
        "{missing}"
    );
}

#[test]
fn the_pi_loop_acts_only_in_the_session_that_asked_and_on_a_settled_idle_turn() {
    let cases = run_pi_loop_driver();

    let changed = &cases["session_changed"];
    assert_eq!(changed["prompts"], 1, "{changed}");
    assert_eq!(changed["asked"], serde_json::json!(["sess-1", "sess-1"]));

    let child = &cases["child_by_parent"];
    assert_eq!(child["prompts"], 0, "{child}");
    assert_eq!(child["asked"], 0, "{child}");
    assert!(
        strings(&child["said"])
            .iter()
            .any(|line| line.contains("child session")),
        "{child}"
    );

    let aborted = &cases["aborted"];
    assert_eq!(aborted["prompts"], 1, "{aborted}");
    assert_eq!(aborted["asked"], aborted["before"], "{aborted}");
    assert_eq!(
        aborted["afterStop"]["asked"], aborted["before"],
        "{aborted}"
    );

    let busy = &cases["busy"];
    assert_eq!(busy["busy"]["prompts"], 1, "{busy}");
    assert_eq!(busy["busy"]["asked"], busy["before"], "{busy}");
    assert_eq!(busy["idle"]["prompts"], 2, "{busy}");

    let none = &cases["no_loop"];
    assert_eq!(none["prompts"], 0);
    assert_eq!(none["asked"], 0, "a session with no loop asked: {none}");
}

#[test]
fn the_pi_loop_defaults_its_objective_to_flow_dev() {
    let defaults = &run_pi_loop_driver()["defaults"];
    assert_eq!(defaults["empty"], "/flow dev");
    assert_eq!(defaults["text"], "/flow do the thing");
    assert_eq!(defaults["named"], "flow dev 12");
    assert_eq!(defaults["slashed"], "/flow triage x");
    assert_eq!(defaults["refused"]["prompts"], 0, "{defaults}");
    assert!(
        strings(&defaults["refused"]["said"])
            .iter()
            .any(|line| line.contains("--wait takes whole minutes")),
        "{defaults}"
    );
}

#[test]
fn a_pi_loop_timer_enqueues_through_the_extension_that_armed_it() {
    let cases = run_pi_loop_driver();
    assert_eq!(
        cases["timer_after_reload"],
        serde_json::json!({ "prompts": 2, "strangers": 0 })
    );
}

#[test]
fn the_pi_loop_stops_on_a_requested_stop_and_clears_the_request_once() {
    let cases = run_pi_loop_driver();
    let clear = |session: &str| {
        serde_json::json!([
            "stop-after",
            "--clear",
            "--session-id",
            session,
            "--runtime",
            "pi"
        ])
    };

    let midloop = &cases["stop_after_midloop"];
    assert_eq!(
        midloop["prompts"], 1,
        "a requested stop continued: {midloop}"
    );
    assert!(
        strings(&midloop["said"])
            .iter()
            .any(|line| line.contains("asked to stop after the current task")
                && line.contains("finish the current task")),
        "{midloop}"
    );
    let calls = midloop["calls"].as_array().expect("the calls");
    assert_eq!(
        calls
            .iter()
            .filter(|call| **call == clear("sess-1"))
            .count(),
        1,
        "{midloop}"
    );
    assert_eq!(midloop["afterStop"]["asked"], midloop["asked"], "{midloop}");
    assert_eq!(midloop["afterStop"]["prompts"], 1, "{midloop}");

    let empty = &cases["stop_after_empty"];
    assert_eq!(
        empty["prompts"], 2,
        "an empty request stopped the loop: {empty}"
    );
    assert!(
        !empty["calls"]
            .as_array()
            .expect("the calls")
            .contains(&clear("sess-1")),
        "{empty}"
    );

    let start = &cases["stop_after_at_start"];
    assert_eq!(start["prompts"], 1, "{start}");
    let calls = start["calls"].as_array().expect("the calls");
    assert_eq!(calls.len(), 2, "{start}");
    assert_eq!(calls[1], clear("sess-1"), "{start}");
}

#[test]
fn the_pi_loop_reports_what_it_holds_and_what_the_tracker_says() {
    let status = &run_pi_loop_driver()["status"];
    let said = strings(&status["said"]).join("\n");
    assert!(said.contains("Estigia goal: /flow x"), "{said}");
    assert!(said.contains("Loop run id: pi-abc"), "{said}");
    assert!(said.contains("Completion cap: 3"), "{said}");
    assert!(said.contains("interval while waiting: 9 minutes"), "{said}");
    assert!(said.contains("Tracker answer:"), "{said}");
}

fn pi_goal_directory(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".pi").join("agent").join(plugin::PI_DIRECTORY)
}

#[test]
fn pi_installs_the_generic_goal_loop_only_when_its_own_setting_is_on() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let directory = pi_goal_directory(home.path());
    let goal_file = directory.join(plugin::PI_GOAL_LOOP_FILE);
    let loop_file = directory.join(plugin::PI_LOOP_FILE);

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(!goal_file.exists(), "the default setting wrote it");
    let bytes_of = |name: &str| fs::read_to_string(directory.join(name)).expect("the file reads");
    let gate = bytes_of(plugin::PI_FILE);
    let relay = bytes_of(plugin::PI_REVIEW_FILE);

    setup(adapter, &loop_on(), &options).expect("the loop install succeeds");
    assert!(loop_file.is_file());
    assert!(
        !goal_file.exists(),
        "the loop row wrote the generic goal loop"
    );

    setup(adapter, &goal_loop_on(), &options).expect("the goal loop install succeeds");
    let written = fs::read_to_string(&goal_file).expect("the goal loop extension reads");
    assert_eq!(written, plugin::PI_GOAL_LOOP_SOURCE);
    assert!(plugin::goal_loop_is_ours(Some(&written)));
    assert!(
        !loop_file.exists(),
        "the goal loop row wrote or kept the Estigia loop extension"
    );
    assert_eq!(bytes_of(plugin::PI_FILE), gate);
    assert_eq!(bytes_of(plugin::PI_REVIEW_FILE), relay);

    let again = setup(adapter, &goal_loop_on(), &options).expect("the second setup succeeds");
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.change == Change::Unchanged),
        "{:?}",
        again.actions
    );

    setup(adapter, &Config::default(), &options).expect("switching it off succeeds");
    assert!(!goal_file.exists(), "off left the extension it wrote");
    assert_eq!(bytes_of(plugin::PI_FILE), gate);
    assert_eq!(bytes_of(plugin::PI_REVIEW_FILE), relay);

    setup(adapter, &goal_loop_on(), &options).expect("the third setup succeeds");
    uninstall(adapter, &options).expect("the plain uninstall succeeds");
    assert!(
        !goal_file.exists(),
        "a plain uninstall left the generic goal loop extension behind"
    );
}

#[test]
fn a_pi_goal_loop_extension_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let goal_file = pi_goal_directory(home.path()).join(plugin::PI_GOAL_LOOP_FILE);
    fs::create_dir_all(goal_file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default function () {}\n";
    fs::write(&goal_file, theirs).expect("write their extension");

    setup(adapter, &goal_loop_on(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&goal_file).expect("it survives"), theirs);
    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&goal_file).expect("it survives"), theirs);
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&goal_file).expect("it survives"), theirs);
}

#[test]
fn the_pi_goal_loop_touches_no_other_adapters_files() {
    let (home, options) = sandbox();
    setup(agent("opencode"), &goal_loop_on(), &options).expect("the OpenCode install succeeds");
    let opencode = resolve_paths(agent("opencode"), &options)
        .expect("paths resolve")
        .loop_plugin
        .expect("OpenCode names a loop plugin path")
        .with_file_name(plugin::GOAL_LOOP_FILE);
    let before = fs::read_to_string(&opencode).expect("the OpenCode plugin reads");

    setup(agent("pi"), &goal_loop_on(), &options).expect("the Pi install succeeds");
    assert!(
        pi_goal_directory(home.path())
            .join(plugin::PI_GOAL_LOOP_FILE)
            .is_file()
    );
    assert_eq!(
        fs::read_to_string(&opencode).expect("the OpenCode plugin survives"),
        before
    );
    uninstall(agent("pi"), &options).expect("the Pi uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&opencode).expect("the OpenCode plugin survives"),
        before,
        "uninstalling Pi took back OpenCode's generic plugin"
    );
}

#[test]
fn the_goal_loop_row_is_offered_to_pi() {
    use crate::config::Setting;
    assert!(Setting::GoalLoopPlugin.applies_to_agent(agent("pi")));
    assert!(Setting::LoopPlugin.applies_to_agent(agent("pi")));
}

#[test]
fn the_pi_goal_loop_holds_no_estigia_path_and_runs_no_command() {
    let source = plugin::PI_GOAL_LOOP_SOURCE;
    let without_marker: String = source
        .lines()
        .filter(|line| !line.contains(plugin::GOAL_LOOP_MARKER))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !without_marker.to_lowercase().contains("estigia"),
        "the generic extension names Estigia beyond its marker"
    );
    for forbidden in [
        "child_process",
        "spawn",
        "execFile",
        "execSync",
        "--session-id",
        "__BINARY__",
    ] {
        assert!(
            !source.contains(forbidden),
            "the generic extension can run a command: {forbidden}"
        );
    }
    assert!(
        !source.contains(plugin::LOOP_MARKER),
        "the generic extension would be taken for the Estigia loop extension"
    );
    let loop_source = plugin::pi_loop_source(std::path::Path::new("/opt/estigia"));
    assert!(
        !loop_source.contains(plugin::GOAL_LOOP_MARKER),
        "the Estigia loop extension would be taken for the generic one"
    );
}

const PI_GOAL_LOOP_DRIVER: &str = r#"const realSetTimeout = globalThis.setTimeout;
const realClearTimeout = globalThis.clearTimeout;
const held = [];
globalThis.setTimeout = (callback, ms, ...rest) => {
  if (ms >= 60000) {
    const timer = { callback, ms, live: true };
    held.push(timer);
    return timer;
  }
  return realSetTimeout(callback, ms, ...rest);
};
globalThis.clearTimeout = (timer) => {
  if (timer && typeof timer === "object" && "live" in timer) timer.live = false;
  else realClearTimeout(timer);
};
const live = () => held.filter((timer) => timer.live);
const settle = () => new Promise((resolve) => realSetTimeout(resolve, 60));
const fire = async (ms) => {
  const due = live().filter((timer) => ms === undefined || timer.ms === ms);
  for (const timer of due) timer.live = false;
  for (const timer of due) timer.callback();
  await settle();
};
const until = async (predicate, what) => {
  const deadline = Date.now() + 4000;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => realSetTimeout(resolve, 5));
  }
};

const TRANSCRIPT = [
  { role: "user", content: "please make the tests pass" },
  { role: "assistant", content: [{ type: "thinking", thinking: "private" }, { type: "text", text: "I fixed the parser" }] },
  { role: "toolResult", toolName: "bash", content: [{ type: "text", text: "12 passed" }] },
];

let runs = 0;
const world = async ({ answers = [], header = {}, model = { id: "judge" }, file = "pi-goal-loop.mjs", transcript = TRANSCRIPT } = {}) => {
  held.length = 0;
  const handlers = {};
  const commands = {};
  const sent = [];
  const notes = [];
  const generated = [];
  const state = { id: "ses_goal", header, idle: true, model };
  const pi = {
    on: (name, handler) => {
      handlers[name] = handler;
    },
    registerCommand: (name, options) => {
      commands[name] = options;
    },
    sendMessage: (message, options) => {
      sent.push({ message, options });
    },
  };
  (await import(`./${file}?run=${runs++}`)).default(pi);
  const ctx = {
    cwd: "/tmp/goal-a",
    get model() {
      return state.model;
    },
    sessionManager: {
      getSessionId: () => state.id,
      getHeader: () => state.header,
      buildSessionProjection: () => ({ messages: transcript }),
    },
    modelRegistry: {
      streamSimple: (model, context, options) => {
        generated.push({ model, context, options });
        return {
          result: async () => {
            const answer = answers.shift();
            if (answer && answer.hang) {
              await new Promise((resolve, reject) =>
                options.signal.addEventListener("abort", () => reject(new Error("aborted")), { once: true }),
              );
            }
            if (answer && answer.delay) await new Promise((resolve) => realSetTimeout(resolve, answer.delay));
            if (answer && answer.throws) throw new Error(answer.throws);
            if (answer && answer.message) return answer.message;
            if (answer && typeof answer.text === "string") {
              return { role: "assistant", stopReason: "stop", content: [{ type: "text", text: answer.text }] };
            }
            return answer;
          },
        };
      },
    },
    isIdle: () => state.idle,
    ui: { notify: (text, level) => notes.push({ text, level }) },
  };
  const run = (name, text, using = ctx) => commands[name].handler(text, using);
  const turn = (aborted = false, using = ctx) => handlers.agent_settled({ type: "agent_settled", aborted }, using);
  return { handlers, commands, sent, notes, generated, state, ctx, run, turn };
};

const said = (w) => w.notes.map((note) => note.text);
const summary = (w) => ({
  prompts: w.sent.map((entry) => entry.message.content),
  options: w.sent.map((entry) => entry.options),
  customTypes: w.sent.map((entry) => entry.message.customType),
  said: said(w),
  generated: w.generated.length,
  asked: w.generated.map((entry) => entry.context.messages.map((message) => message.content).join("\n")),
  timers: live().map((timer) => timer.ms),
});
const silentTurns = async (w, count) => {
  for (let i = 0; i < count; i++) {
    const before = w.sent.length;
    w.turn();
    await until(() => w.sent.length > before, `silent turn ${i + 1}`);
  }
};
const reachJudge = async (w, count) => {
  await silentTurns(w, 2);
  w.turn();
  await until(() => w.generated.length >= count, `judgement ${count}`);
};
const turnEnds = async (w, count) => {
  await reachJudge(w, count);
  await settle();
};
const report = async (w, name) => {
  const before = w.notes.length;
  await w.run(name, name === "loop" ? "status" : "");
  return w.notes.slice(before).map((note) => note.text).join("\n");
};

const cases = {};

{
  const w = await world();
  cases.registered = {
    commands: Object.keys(w.commands),
    described: Object.values(w.commands).every((c) => typeof c.description === "string" && c.description !== ""),
    events: Object.keys(w.handlers).sort(),
  };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nthe tests still fail" }] });
  await w.run("goal", "all tests pass");
  cases.started = summary(w);
  await turnEnds(w, 1);
  cases.unmet = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ answers: [{ text: "MET\nthe suite is green" }] });
  await w.run("goal", "all tests pass");
  await turnEnds(w, 1);
  cases.met = { ...summary(w), report: await report(w, "goal") };
  w.turn();
  await settle();
  cases.met_after = summary(w);
}
{
  const w = await world({ answers: [{ message: { role: "assistant", stopReason: "stop", content: [{ type: "thinking", thinking: "MET" }, { type: "text", text: "NOT MET\nlater" }] } }] });
  await w.run("goal", "all tests pass");
  await turnEnds(w, 1);
  cases.blocks = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }, { text: "NOT MET\nstill no" }, { text: "NOT MET\nnever" }] });
  await w.run("goal", "--max 2 all tests pass");
  await silentTurns(w, 1);
  const first = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cap: the judgement");
  await settle();
  cases.cap = { first, second: summary(w), report: await report(w, "goal") };
  w.turn();
  await settle();
  cases.cap_after = summary(w);
}
for (const [name, answer] of [
  ["maybe", { text: "maybe, hard to say" }],
  ["empty", { text: "" }],
  ["shapeless", { unexpected: true }],
  ["nothing", undefined],
  ["thrown", { throws: "the model was unavailable" }],
  ["errored", { message: { role: "assistant", stopReason: "error", errorMessage: "overloaded", content: [] } }],
  ["mentions", { text: "The condition is not met, so: MET is wrong" }],
]) {
  const w = await world({ answers: [answer] });
  await w.run("goal", "all tests pass");
  await turnEnds(w, 1);
  cases[`unreadable_${name}`] = { ...summary(w), report: await report(w, "goal") };
  w.turn();
  await settle();
  cases[`unreadable_${name}_after`] = summary(w);
}
{
  const w = await world({ model: null });
  await w.run("goal", "all tests pass");
  await silentTurns(w, 2);
  w.turn();
  await settle();
  cases.no_model = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ answers: [{ hang: true }] });
  await w.run("goal", "all tests pass");
  await reachJudge(w, 1);
  cases.timeout_signal = w.generated[0].options.signal instanceof AbortSignal;
  cases.timeout_armed = live().map((timer) => timer.ms);
  await fire(live()[0].ms);
  cases.timeout = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run("goal", "all tests pass");
  await silentTurns(w, 1);
  cases.cadence_first = summary(w);
  await silentTurns(w, 1);
  cases.cadence_second = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cadence: the third turn is judged");
  await settle();
  cases.cadence_third = summary(w);
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run("goal", "--max 2 all tests pass");
  await silentTurns(w, 1);
  cases.capturn_before = summary(w);
  w.turn();
  await until(() => w.generated.length >= 1, "cap turn: the judgement");
  await settle();
  cases.capturn_at = { ...summary(w), report: await report(w, "goal") };
}
{
  const long = (marker) => marker + " " + "x".repeat(9999);
  const transcript = [];
  for (let i = 0; i < 30; i++) {
    transcript.push({ role: "user", content: long("user-" + i) });
    transcript.push({ role: "assistant", content: [{ type: "text", text: long("assistant-" + i) }] });
  }
  transcript.push({ role: "assistant", content: [{ type: "text", text: "NEWEST-MESSAGE" }] });
  const w = await world({ answers: [{ text: "NOT MET\nno" }], transcript });
  await w.run("goal", "all tests pass");
  await turnEnds(w, 1);
  const shown = summary(w).asked[0];
  cases.bounded = {
    length: shown.length,
    newest: shown.includes("NEWEST-MESSAGE"),
    oldest: shown.includes("user-0 "),
    truncated: shown.includes("[truncated]"),
  };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  cases.status_none = await report(w, "goal");
  await w.run("goal", "all tests pass");
  await turnEnds(w, 1);
  cases.status_running = await report(w, "goal");
  await w.run("goal", "clear");
  cases.cleared = summary(w);
  w.turn();
  await settle();
  cases.cleared_after = summary(w);
  await w.run("goal", "clear");
  cases.cleared_again = summary(w);
}
for (const [name, text] of [
  ["refused_max", "--max 0 all tests pass"],
  ["bare_max", "--max"],
  ["bare_max_after_condition", "all tests pass --max"],
  ["empty_max_value", "--max= all tests pass"],
  ["word_max", "--max soon all tests pass"],
]) {
  const w = await world({ answers: [{ text: "NOT MET\nno" }] });
  await w.run("goal", text);
  const refused = summary(w);
  w.turn();
  await settle();
  cases[name] = { ...refused, generatedAfter: w.generated.length };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno", delay: 200 }] });
  await w.run("goal", "all tests pass");
  await reachJudge(w, 1);
  await w.run("goal", "clear");
  await new Promise((resolve) => realSetTimeout(resolve, 400));
  cases.race_clear = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno", delay: 200 }] });
  await w.run("goal", "the first condition");
  await reachJudge(w, 1);
  await w.run("goal", "the second condition");
  await new Promise((resolve) => realSetTimeout(resolve, 400));
  cases.race_replace = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ answers: [{ text: "NOT MET\nno", delay: 200 }] });
  await w.run("goal", "all tests pass");
  await reachJudge(w, 1);
  w.state.id = "ses_other";
  await new Promise((resolve) => realSetTimeout(resolve, 400));
  cases.session_changed = summary(w);
}
{
  const w = await world({ answers: [{ text: "MET\nyes" }] });
  await w.run("goal", "all tests pass");
  w.state.idle = false;
  w.turn();
  await settle();
  cases.busy = summary(w);
  w.state.idle = true;
  const other = { ...w.ctx, sessionManager: { ...w.ctx.sessionManager, getSessionId: () => "ses_other" } };
  w.turn(false, other);
  await settle();
  cases.other_session = summary(w);
  w.turn(true);
  await settle();
  cases.aborted = { ...summary(w), report: await report(w, "goal") };
}
{
  const w = await world({ header: { parentSession: "/tmp/parent.jsonl" } });
  await w.run("goal", "all tests pass");
  await w.run("loop", "5m check the build");
  cases.child = summary(w);
}
{
  const w = await world();
  await w.run("loop", "5m check the build");
  cases.interval_first = summary(w);
  await fire();
  cases.interval_second = summary(w);
  await fire();
  cases.interval_third = summary(w);
  w.turn();
  await settle();
  cases.interval_turn = summary(w);
  cases.interval_report = await report(w, "loop");
  await w.run("loop", "stop");
  cases.interval_stopped = summary(w);
  await fire();
  cases.interval_after = summary(w);
}
{
  const w = await world();
  await w.run("loop", "poll the queue");
  cases.paced_first = summary(w);
  w.turn();
  await settle();
  cases.paced_second = summary(w);
  w.state.idle = false;
  w.turn();
  await settle();
  cases.paced_busy = summary(w);
  w.state.idle = true;
  await w.run("loop", "again");
  cases.paced_already = summary(w);
  await w.run("loop", "stop");
  w.turn();
  await settle();
  cases.paced_after = summary(w);
  await w.run("loop", "stop");
  cases.paced_stop_again = summary(w);
}
{
  const w = await world();
  await w.run("loop", "--max 2 poll the queue");
  w.turn();
  await settle();
  w.turn();
  await settle();
  cases.loop_capped = { ...summary(w), report: await report(w, "loop") };
}
{
  const w = await world();
  await w.run("loop", "poll the queue");
  w.turn(true);
  await settle();
  cases.paced_aborted = { ...summary(w), report: await report(w, "loop") };
}
for (const [name, text] of [["short", "5s check"], ["long", "25d check"], ["bare", "5m"], ["nothing", ""], ["capless", "--max x poll"]]) {
  const w = await world();
  await w.run("loop", text);
  cases[`loop_refused_${name}`] = summary(w);
}
{
  const w = await world();
  await w.run("loop", "5m check the build");
  await w.handlers.session_shutdown({ type: "session_shutdown" }, w.ctx);
  await fire();
  cases.torn_down = summary(w);
  cases.torn_down_report = await report(w, "loop");
}
{
  const w = await world();
  await w.run("loop", "5m check the build");
  w.state.id = "ses_other";
  await fire();
  cases.loop_session_changed = summary(w);
}

console.log(JSON.stringify(cases));
process.exit(0);
"#;

fn run_pi_goal_loop_driver() -> &'static serde_json::Value {
    static CASES: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    CASES.get_or_init(|| {
        let root = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(
            root.path().join("pi-goal-loop.mjs"),
            plugin::PI_GOAL_LOOP_SOURCE,
        )
        .expect("the goal loop extension writes");
        let driver = root.path().join("drive-pi-goal-loop.mjs");
        std::fs::write(&driver, PI_GOAL_LOOP_DRIVER).expect("the driver writes");
        let ran = std::process::Command::new("node")
            .arg(&driver)
            .current_dir(root.path())
            .env_remove("NODE_OPTIONS")
            .output();
        let ran = match ran {
            Ok(ran) => ran,
            Err(error) => panic!(
                "this test drives the Pi goal and loop extension, which is JavaScript, and `node` \
                 could not be run ({error}). Install Node and re-run; it is not skipped."
            ),
        };
        assert!(
            ran.status.success(),
            "the Pi goal loop extension would not load or the driver failed: {}",
            String::from_utf8_lossy(&ran.stderr)
        );
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
    })
}

fn pi_prompts(case: &serde_json::Value) -> Vec<&str> {
    strings(&case["prompts"])
}

fn pi_said(case: &serde_json::Value) -> String {
    strings(&case["said"]).join("\n")
}

fn report_text(case: &serde_json::Value) -> &str {
    case["report"].as_str().expect("a report")
}

#[test]
fn the_pi_goal_loop_registers_goal_and_loop_and_nothing_that_collides() {
    let cases = run_pi_goal_loop_driver();
    assert_eq!(
        cases["registered"]["commands"],
        serde_json::json!(["goal", "loop"])
    );
    assert_eq!(cases["registered"]["described"], true);
    assert_eq!(
        cases["registered"]["events"],
        serde_json::json!(["agent_settled", "session_shutdown"])
    );
    let loop_source = plugin::pi_loop_source(std::path::Path::new("/opt/estigia"));
    for name in ["flow-loop", "flow-status"] {
        assert!(loop_source.contains(&format!("\"{name}\"")));
        assert!(
            !plugin::PI_GOAL_LOOP_SOURCE.contains(&format!("\"{name}\"")),
            "the generic extension registers `{name}`, which the Estigia loop extension owns"
        );
    }
}

#[test]
fn the_pi_goal_continues_on_an_unmet_judgement_and_stops_on_a_met_one() {
    let cases = run_pi_goal_loop_driver();

    let started = &cases["started"];
    assert_eq!(pi_prompts(started).len(), 1, "{started}");
    assert!(
        pi_prompts(started)[0].contains("all tests pass"),
        "{started}"
    );
    assert_eq!(
        started["options"][0],
        serde_json::json!({ "triggerTurn": true, "deliverAs": "followUp" })
    );

    let unmet = &cases["unmet"];
    assert_eq!(pi_prompts(unmet).len(), 4, "{unmet}");
    assert!(
        pi_prompts(unmet)[3].contains("all tests pass")
            && pi_prompts(unmet)[3].contains("still fail"),
        "the continuation did not carry the condition and the reason: {unmet}"
    );
    assert_eq!(
        unmet["options"][3],
        serde_json::json!({ "triggerTurn": true, "deliverAs": "followUp" })
    );
    let asked = unmet["asked"][0].as_str().expect("what was asked");
    assert!(
        asked.contains("all tests pass") && asked.contains("NOT MET"),
        "the evaluator was not asked about the condition: {unmet}"
    );
    assert!(
        asked.contains("I fixed the parser") && asked.contains("12 passed"),
        "the evaluator did not see the conversation: {unmet}"
    );
    assert!(
        !asked.contains("private"),
        "the evaluator was shown the model's thinking: {unmet}"
    );
    assert!(report_text(unmet).contains("Still running"), "{unmet}");
    assert!(
        report_text(unmet).contains("Turns: 3 of at most 25"),
        "{unmet}"
    );

    let met = &cases["met"];
    assert_eq!(pi_prompts(met).len(), 3, "a met goal continued: {met}");
    assert!(pi_said(met).contains("Goal met after 3 turns"), "{met}");
    assert!(
        report_text(met).contains("the condition was judged met"),
        "{met}"
    );
    assert_eq!(
        cases["met_after"]["generated"], 1,
        "a goal that was met was judged again on the next turn"
    );

    let blocks = &cases["blocks"];
    assert_eq!(
        pi_prompts(blocks).len(),
        4,
        "the thinking block was read as the answer: {blocks}"
    );
}

#[test]
fn the_pi_goal_stops_at_its_turn_cap() {
    let cases = run_pi_goal_loop_driver();
    let cap = &cases["cap"];
    assert_eq!(pi_prompts(&cap["first"]).len(), 2, "{cap}");
    assert_eq!(pi_prompts(&cap["second"]).len(), 2, "{cap}");
    assert!(pi_said(&cap["second"]).contains("Goal stopped"), "{cap}");
    assert!(report_text(cap).contains("turn cap (2)"), "{cap}");
    assert_eq!(cases["cap_after"]["generated"], 1);
    for name in [
        "refused_max",
        "bare_max",
        "bare_max_after_condition",
        "empty_max_value",
        "word_max",
    ] {
        let case = &cases[name];
        let said = strings(&case["said"]);
        assert_eq!(said.len(), 1, "{name}: {case}");
        assert!(
            said[0].starts_with("Goal not set: --max takes a whole number"),
            "{name}: {case}"
        );
        assert!(
            pi_prompts(case).is_empty(),
            "{name}: a refused goal enqueued: {case}"
        );
        assert_eq!(
            case["generatedAfter"], 0,
            "{name}: a refused goal was judged: {case}"
        );
    }
}

#[test]
fn the_pi_goal_stops_on_an_evaluator_answer_it_cannot_read() {
    let cases = run_pi_goal_loop_driver();
    for name in [
        "maybe",
        "empty",
        "shapeless",
        "nothing",
        "thrown",
        "errored",
        "mentions",
    ] {
        let case = &cases[format!("unreadable_{name}")];
        assert_eq!(
            pi_prompts(case).len(),
            3,
            "{name}: an unreadable answer continued the goal: {case}"
        );
        assert!(
            report_text(case).contains("could not be read"),
            "{name}: the goal did not stop on an unreadable answer: {case}"
        );
        assert!(pi_said(case).contains("Goal stopped"), "{name}: {case}");
        assert_eq!(
            cases[format!("unreadable_{name}_after")]["generated"],
            1,
            "{name}: a stopped goal was judged again"
        );
    }
    let no_model = &cases["no_model"];
    assert_eq!(no_model["generated"], 0, "{no_model}");
    assert!(
        report_text(no_model).contains("could not be read"),
        "{no_model}"
    );
}

#[test]
fn a_judgement_that_never_answers_is_bounded_and_read_as_no_answer() {
    let cases = run_pi_goal_loop_driver();
    assert_eq!(cases["timeout_signal"], true);
    assert_eq!(cases["timeout_armed"], serde_json::json!([120_000]));
    let timeout = &cases["timeout"];
    assert_eq!(pi_prompts(timeout).len(), 3, "{timeout}");
    assert!(
        report_text(timeout).contains("could not be read"),
        "{timeout}"
    );
    assert_eq!(timeout["timers"], serde_json::json!([]));
}

#[test]
fn a_stop_a_replacement_or_a_session_change_mid_judgement_is_not_overwritten() {
    let cases = run_pi_goal_loop_driver();

    let clear = &cases["race_clear"];
    assert_eq!(
        pi_prompts(clear).len(),
        3,
        "a cleared goal was continued: {clear}"
    );
    assert!(
        report_text(clear).contains("the operator cleared the goal"),
        "the judgement in flight overwrote the clear: {clear}"
    );
    assert!(report_text(clear).contains("Turns: 2"), "{clear}");

    let replace = &cases["race_replace"];
    assert_eq!(pi_prompts(replace).len(), 4, "{replace}");
    assert!(
        pi_prompts(replace)[3].contains("the second condition"),
        "{replace}"
    );
    let text = report_text(replace);
    assert!(text.contains("Goal: the second condition"), "{replace}");
    assert!(
        text.contains("Turns: 0"),
        "the old judgement was counted: {replace}"
    );
    assert!(text.contains("No judgement has been made yet"), "{replace}");
    assert!(text.contains("Still running"), "{replace}");

    let changed = &cases["session_changed"];
    assert_eq!(pi_prompts(changed).len(), 3, "{changed}");
    assert!(
        !pi_said(changed).contains("Goal"),
        "a judgement that outlived its session said something: {changed}"
    );
}

#[test]
fn the_pi_goal_reports_clears_and_acts_only_on_a_settled_idle_turn_of_its_session() {
    let cases = run_pi_goal_loop_driver();
    assert!(
        cases["status_none"]
            .as_str()
            .expect("text")
            .contains("No goal is set")
    );
    let running = cases["status_running"].as_str().expect("text");
    assert!(
        running.contains("Goal: all tests pass")
            && running.contains("Last judgement")
            && running.contains("not a check"),
        "{running}"
    );
    assert!(pi_said(&cases["cleared"]).contains("Goal cleared"));
    assert_eq!(cases["cleared_after"]["generated"], 1);
    assert!(pi_said(&cases["cleared_again"]).contains("No running goal"));

    assert_eq!(cases["busy"]["generated"], 0, "a busy session was judged");
    assert_eq!(
        cases["other_session"]["generated"], 0,
        "another session's turn end was judged"
    );
    let aborted = &cases["aborted"];
    assert_eq!(aborted["generated"], 0, "{aborted}");
    assert!(
        report_text(aborted).contains("the turn was aborted"),
        "{aborted}"
    );
}

#[test]
fn a_child_session_runs_neither_goal_nor_loop() {
    let cases = run_pi_goal_loop_driver();
    let case = &cases["child"];
    assert_eq!(pi_prompts(case).len(), 0, "{case}");
    assert!(
        pi_said(case).matches("a child session").count() == 2,
        "{case}"
    );
    assert_eq!(case["timers"], serde_json::json!([]));
}

/// The evaluator is asked on every third turn, not on every turn.
#[test]
fn the_pi_goal_judges_every_third_turn_and_continues_the_others_unasked() {
    let cases = run_pi_goal_loop_driver();
    for name in ["cadence_first", "cadence_second"] {
        let case = &cases[name];
        assert_eq!(case["generated"], 0, "{name}: the judge was called: {case}");
        let last = pi_prompts(case).last().copied().unwrap_or_default();
        assert!(
            last.contains("all tests pass")
                && last.contains("Keep working toward it")
                && !last.contains("judged not yet met"),
            "{name}: the unjudged turn did not continue the goal: {case}"
        );
    }
    assert_eq!(pi_prompts(&cases["cadence_second"]).len(), 3);
    let third = &cases["cadence_third"];
    assert_eq!(third["generated"], 1, "{third}");
    assert_eq!(pi_prompts(third).len(), 4, "{third}");
}

/// The turn that reaches the cap is judged even when it is not a third turn.
#[test]
fn the_pi_goal_judges_the_turn_that_reaches_its_cap() {
    let cases = run_pi_goal_loop_driver();
    assert_eq!(cases["capturn_before"]["generated"], 0);
    let at = &cases["capturn_at"];
    assert_eq!(at["generated"], 1, "the cap turn was not judged: {at}");
    assert!(report_text(at).contains("turn cap (2)"), "{at}");
}

/// The transcript handed to the evaluator is bounded as a whole and keeps the
/// newest messages.
#[test]
fn the_pi_goal_bounds_the_transcript_the_evaluator_is_shown() {
    let cases = run_pi_goal_loop_driver();
    let bounded = &cases["bounded"];
    let length = bounded["length"].as_u64().expect("the length");
    assert!(
        length <= 20_000 + 1_000,
        "the evaluator was shown {length} characters"
    );
    assert_eq!(bounded["newest"], true, "the newest message was dropped");
    assert_eq!(bounded["oldest"], false, "the oldest message was kept");
    assert_eq!(bounded["truncated"], true, "a long message was not capped");
}

#[test]
fn the_pi_loop_repeats_on_an_interval_and_after_each_turn_and_stops() {
    let cases = run_pi_goal_loop_driver();
    let count = |case: &serde_json::Value| pi_prompts(case).len();

    assert_eq!(count(&cases["interval_first"]), 1);
    assert_eq!(
        cases["interval_first"]["timers"],
        serde_json::json!([300_000])
    );
    assert_eq!(pi_prompts(&cases["interval_first"])[0], "check the build");
    assert_eq!(
        cases["interval_first"]["options"][0],
        serde_json::json!({ "triggerTurn": true, "deliverAs": "followUp" })
    );
    assert_eq!(count(&cases["interval_second"]), 2);
    assert_eq!(count(&cases["interval_third"]), 3);
    assert_eq!(
        count(&cases["interval_turn"]),
        3,
        "a turn end repeated an interval loop"
    );
    assert!(
        cases["interval_report"]
            .as_str()
            .expect("text")
            .contains("check the build"),
        "{}",
        cases["interval_report"]
    );
    assert_eq!(cases["interval_stopped"]["timers"], serde_json::json!([]));
    assert_eq!(
        count(&cases["interval_after"]),
        3,
        "a stopped interval loop fired"
    );
    assert!(pi_said(&cases["interval_stopped"]).contains("Loop stopped"));
    assert_eq!(
        count(&cases["torn_down"]),
        1,
        "a torn-down extension still fired"
    );
    assert_eq!(cases["torn_down"]["timers"], serde_json::json!([]));
    assert!(
        cases["torn_down_report"]
            .as_str()
            .expect("text")
            .contains("No loop")
    );
    assert_eq!(
        count(&cases["loop_session_changed"]),
        1,
        "a loop fired into another session"
    );

    assert_eq!(count(&cases["paced_first"]), 1);
    assert_eq!(cases["paced_first"]["timers"], serde_json::json!([]));
    assert_eq!(
        count(&cases["paced_second"]),
        2,
        "a turn end did not repeat the prompt"
    );
    assert_eq!(
        count(&cases["paced_busy"]),
        2,
        "a busy session was prompted"
    );
    assert!(pi_said(&cases["paced_already"]).contains("already running"));
    assert_eq!(count(&cases["paced_already"]), 2);
    assert_eq!(
        count(&cases["paced_after"]),
        2,
        "a stopped self-paced loop repeated"
    );
    assert!(pi_said(&cases["paced_stop_again"]).contains("No running loop"));

    let capped = &cases["loop_capped"];
    assert_eq!(count(capped), 2, "{capped}");
    assert!(report_text(capped).contains("run cap (2)"), "{capped}");
    let aborted = &cases["paced_aborted"];
    assert_eq!(count(aborted), 1, "{aborted}");
    assert!(
        report_text(aborted).contains("the turn was aborted"),
        "{aborted}"
    );

    for name in ["short", "long", "bare", "nothing", "capless"] {
        let case = &cases[format!("loop_refused_{name}")];
        assert_eq!(count(case), 0, "{name}: a refused loop sent: {case}");
        assert!(pi_said(case).contains("Loop not started"), "{name}: {case}");
    }
}

#[test]
fn a_model_in_another_hosts_curated_catalog_is_foreign_to_an_adapter_that_does_not_list_it() {
    assert_eq!(
        agent("codex").foreign_model_hosts("sonnet"),
        ["Claude Code"]
    );
    assert_eq!(
        agent("claude-code").foreign_model_hosts("gpt-5.5"),
        ["Codex"]
    );
    assert_eq!(agent("pi").foreign_model_hosts("opus"), ["Claude Code"]);
    assert_eq!(agent("opencode").foreign_model_hosts("gpt-5.5"), ["Codex"]);
}

#[test]
fn a_model_the_adapter_lists_itself_or_no_curated_catalog_lists_is_not_foreign() {
    assert!(
        agent("claude-code")
            .foreign_model_hosts("sonnet")
            .is_empty()
    );
    assert!(agent("codex").foreign_model_hosts("gpt-6-luna").is_empty());
    assert!(
        agent("codex")
            .foreign_model_hosts("my-fine-tune")
            .is_empty()
    );
    assert!(agent("pi").foreign_model_hosts("inherit").is_empty());
}

#[test]
fn a_routing_reports_each_foreign_model_once() {
    let mut routing = ModelRouting::default();
    routing.assign("developer", "sonnet");
    routing.assign("analyst", "sonnet");
    routing.assign("reviewer", "gpt-5.5");
    routing.assign("judge", "custom-id");

    assert_eq!(
        agent("codex").foreign_models_in(&routing),
        [("sonnet".to_owned(), vec!["Claude Code"])]
    );
    assert_eq!(
        agent("claude-code").foreign_models_in(&routing),
        [("gpt-5.5".to_owned(), vec!["Codex"])]
    );
}

const PI_CLAIM_STUB: &str = r#"const fs = require("node:fs");
const path = require("node:path");
const verb = path.basename(process.argv[1] ?? "");
if (verb === "goal" || verb === "gate" || verb === "doctor" || verb === "changes") {
  const entry = { args: [verb, ...process.argv.slice(2)], cwd: process.cwd() };
  if (process.env.STUB_CASE) entry.case = process.env.STUB_CASE;
  if (verb === "gate") {
    try { entry.stdin = fs.readFileSync(0, "utf8"); } catch { entry.stdin = ""; }
  }
  fs.appendFileSync(process.env.STUB_LOG, JSON.stringify(entry) + "\n");
  const prefix = `STUB_${verb.toUpperCase()}`;
  const mode = process.env[`${prefix}_MODE`] ?? "";
  if (mode === "sleep") Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 60000);
  if (mode === "signal") process.kill(process.pid, "SIGKILL");
  if (mode === "flood") {
    const chunk = "x".repeat(65536);
    try {
      for (let sent = 0; sent < 32 * 1024 * 1024; sent += chunk.length) fs.writeSync(2, chunk);
    } catch {}
    process.exit(0);
  }
  process.stdout.write(process.env[`${prefix}_OUT`] ?? "");
  process.stderr.write(process.env[`${prefix}_ERR`] ?? "");
  process.exit(Number(process.env[`${prefix}_EXIT`] ?? 0));
}
"#;

const PI_CLAIM_DRIVER: &str = r##"import { readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
delete process.env.ESTIGIA_PI_PARENT_SESSION;
delete process.env.ESTIGIA_PI_PARENT_PID;

const calls = () => readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const handlers = {};
const renderers = {};
const appended = [];
const sent = [];
(await import("./gate.mjs")).default({
  on: (name, handler) => { handlers[name] = handler; },
  registerEntryRenderer: (type, renderer) => { renderers[type] = renderer; },
  appendEntry: (type, data) => { appended.push({ type, data }); },
  sendMessage: (...args) => { sent.push(args); },
  sendUserMessage: (...args) => { sent.push(args); },
});

const quickHandlers = {};
(await import("./quick.mjs")).default({
  on: (name, handler) => { quickHandlers[name] = handler; },
  registerEntryRenderer: () => {},
  appendEntry: () => {},
  sendMessage: () => {},
  sendUserMessage: () => {},
});

const statuses = [];
const shelf = [];
const ctxOf = (session, hasUI = true, widgets = true) => ({
  cwd: process.env.PROJECT,
  hasUI,
  sessionManager: { getSessionId: () => session },
  ui: {
    setStatus: (key, text) => { statuses.push({ key, text: text ?? null }); },
    ...(widgets ? { setWidget: (key, content) => { shelf.push({ key, content: content ?? null }); } } : {}),
  },
});

const answer = (env) => {
  for (const key of ["STUB_GOAL_MODE", "STUB_GOAL_OUT", "STUB_GOAL_EXIT", "STUB_GATE_ERR", "STUB_GATE_EXIT", "STUB_DOCTOR_MODE", "STUB_DOCTOR_OUT", "STUB_DOCTOR_ERR", "STUB_DOCTOR_EXIT", "STUB_CHANGES_MODE", "STUB_CHANGES_OUT", "STUB_CHANGES_ERR", "STUB_CHANGES_EXIT"]) delete process.env[key];
  Object.assign(process.env, env);
};

const status = async (label, event, ctx, env, wait = 3000, via = handlers) => {
  answer(env);
  const before = statuses.length;
  const spawned = calls().length;
  const returned = await via[event]({ type: event }, ctx);
  for (let waited = 0; waited < wait && statuses.length === before; waited += 20) await pause(20);
  if (statuses.length === before) await pause(150);
  return { label, returned: returned ?? null, statuses: statuses.slice(before), calls: calls().slice(spawned).filter((call) => call.args[0] !== "changes") };
};

const widget = async (label, event, ctx, env, wait = 3000, via = handlers) => {
  answer({ ...env, STUB_CASE: label });
  const before = shelf.length;
  await via[event]({ type: event }, ctx);
  for (let waited = 0; waited < wait && shelf.length === before; waited += 20) await pause(20);
  if (shelf.length === before) await pause(150);
  const own = calls().filter((call) => call.case === label && call.args[0] === "changes");
  delete process.env.STUB_CASE;
  return { label, widgets: shelf.slice(before), calls: own.map(({ case: _, ...call }) => call) };
};

const checkout = (path, files, unreadable = null) => ({ path, files: files.map((name) => ({ marker: "U", path: name, added: 1, removed: null })), unreadable });
const tally = (...worktrees) => JSON.stringify({ worktrees });

const held = JSON.stringify({ run_id: "r", issue: "#42", state: "in-progress", standing: "passed", live: true });
const none = JSON.stringify({ run_id: "r", issue: null, state: null, standing: "nothing-sworn", live: false });
const stopped = JSON.stringify({ run_id: "r", issue: "#7", state: "review", standing: "stopped", live: false });

const out = [];
out.push(await status("held", "session_start", ctxOf("sess-1"), { STUB_GOAL_OUT: held }));
out.push(await status("settled", "agent_settled", ctxOf("sess-1"), { STUB_GOAL_OUT: held }));
out.push(await status("after_tool", "tool_result", ctxOf("sess-1"), { STUB_GOAL_OUT: stopped }));
out.push(await status("none", "session_start", ctxOf("sess-1"), { STUB_GOAL_OUT: none }));
out.push(await status("no_exit", "session_start", ctxOf("sess-1"), { STUB_GOAL_MODE: "signal" }));
out.push(await status("exit_two", "session_start", ctxOf("sess-1"), { STUB_GOAL_EXIT: "2", STUB_GOAL_OUT: held }));
out.push(await status("garbled", "session_start", ctxOf("sess-1"), { STUB_GOAL_OUT: "not json" }));
out.push(await status("slow", "session_start", ctxOf("sess-1"), { STUB_GOAL_MODE: "sleep" }, 3000, quickHandlers));
out.push(await status("headless", "session_start", ctxOf("sess-1", false), { STUB_GOAL_OUT: held }));
process.env.ESTIGIA_PI_PARENT_SESSION = "root-9";
process.env.ESTIGIA_PI_PARENT_PID = "1";
out.push(await status("child", "session_start", ctxOf("child-3"), { STUB_GOAL_OUT: held }));

const shelved = [
  await widget("started", "session_start", ctxOf("sess-1"), { STUB_CHANGES_OUT: tally(checkout("/a", ["x"]), checkout("/b", []), checkout("/c", ["y", "z"])) }),
  await widget("one", "agent_settled", ctxOf("sess-1"), { STUB_CHANGES_OUT: tally(checkout("/a", ["x"])) }),
  await widget("mixed", "agent_settled", ctxOf("sess-1"), { STUB_CHANGES_OUT: tally(checkout("/a", ["x"]), checkout("/b", [], "not a git repository")) }),
  await widget("unreadable_only", "agent_settled", ctxOf("sess-1"), { STUB_CHANGES_OUT: tally(checkout("/b", [], "not a git repository")) }),
  await widget("clean", "agent_settled", ctxOf("sess-1"), { STUB_CHANGES_OUT: tally(checkout("/a", []), checkout("/b", [])) }),
  await widget("silent", "session_start", ctxOf("sess-1"), { STUB_CHANGES_EXIT: "3" }),
  await widget("garbled", "session_start", ctxOf("sess-1"), { STUB_CHANGES_OUT: "not json" }),
  await widget("slow", "session_start", ctxOf("sess-1"), { STUB_CHANGES_MODE: "sleep" }, 3000, quickHandlers),
  await widget("headless", "session_start", ctxOf("sess-1", false), { STUB_CHANGES_OUT: tally(checkout("/a", ["x"])) }),
  await widget("no_widget_surface", "session_start", ctxOf("sess-1", true, false), { STUB_CHANGES_OUT: tally(checkout("/a", ["x"])) }),
];

const refusal = async (label, ctx, tool, exit, err) => {
  answer({ STUB_GATE_EXIT: String(exit), STUB_GATE_ERR: err });
  const before = appended.length;
  const sentBefore = sent.length;
  const result = await handlers.tool_call({ toolName: tool, input: { command: "git commit -m x", path: "a.rs", content: "x" } }, ctx);
  return { label, result: result ?? null, appended: appended.slice(before), sent: sent.length - sentBefore };
};

const reason = "estigia: refused: no live claim\n  run `claim` first\nsecond clause";
const refused = [
  await refusal("one", ctxOf("sess-1"), "bash", 1, reason),
  await refusal("two", ctxOf("sess-1"), "write", 2, "unread"),
  await refusal("headless", ctxOf("sess-1", false), "bash", 1, reason),
  await refusal("allowed", ctxOf("sess-1"), "bash", 0, ""),
];
const card = refused[0].appended[0];
const lines = (width) => renderers[card.type](card, { expanded: false }, undefined).render(width);

const load = async (file) => {
  const seen = { commands: {}, renderers: {}, appended: [], sent: [] };
  (await import(file)).default({
    on: () => {},
    registerCommand: (name, options) => { seen.commands[name] = options; },
    registerEntryRenderer: (type, renderer) => { seen.renderers[type] = renderer; },
    appendEntry: (type, data) => { seen.appended.push({ type, data }); },
    sendMessage: (...args) => { seen.sent.push(args); },
    sendUserMessage: (...args) => { seen.sent.push(args); },
  });
  return seen;
};

const present = await load("./gate.mjs");
const absent = await load("./missing.mjs");
const quick = await load("./quick.mjs");

const doctor = async (label, seen, hasUI, env, args = "", command = "estigia-doctor") => {
  answer(env);
  const logged = [];
  const errors = [];
  const notes = [];
  const { log, error } = console;
  console.log = (...parts) => { logged.push(parts.join(" ")); };
  console.error = (...parts) => { errors.push(parts.join(" ")); };
  const before = seen.appended.length;
  const spawned = calls().length;
  let returned;
  try {
    returned = await seen.commands[command]?.handler(args, {
      cwd: process.env.PROJECT,
      hasUI,
      ui: { notify: (text, kind) => { notes.push({ text, kind }); }, setStatus: () => {} },
    });
  } finally {
    console.log = log;
    console.error = error;
  }
  return { label, returned: returned ?? null, appended: seen.appended.slice(before), sent: seen.sent.length, logged, errors, notes, calls: calls().slice(spawned) };
};

const report = "estigia doctor\n  [ok] one\n  [broken] two: not here\n    run: estigia setup pi";
const doctors = [
  await doctor("fine", present, true, { STUB_DOCTOR_OUT: `${report}\n` }),
  await doctor("red", present, true, { STUB_DOCTOR_EXIT: "1", STUB_DOCTOR_OUT: report, STUB_DOCTOR_ERR: "refused: two is broken" }),
  await doctor("silent", present, true, { STUB_DOCTOR_EXIT: "3" }),
  await doctor("signal", present, true, { STUB_DOCTOR_MODE: "signal" }),
  await doctor("slow", quick, true, { STUB_DOCTOR_MODE: "sleep" }),
  await doctor("flood", present, true, { STUB_DOCTOR_MODE: "flood" }),
  await doctor("missing", absent, true, {}),
  await doctor("headless", present, false, { STUB_DOCTOR_OUT: `${report}\n` }),
  await doctor("headless_missing", absent, false, {}),
];
const listing = "estigia changes\n  /work/issue-5-parked  issue-5-parked  1 uncommitted\n    U new.txt";
const changes = [
  await doctor("listed", present, true, { STUB_CHANGES_OUT: `${listing}\n` }, "", "estigia-changes"),
  await doctor("silent", present, true, { STUB_CHANGES_EXIT: "3" }, "", "estigia-changes"),
  await doctor("missing", absent, true, {}, "", "estigia-changes"),
  await doctor("headless", present, false, { STUB_CHANGES_OUT: `${listing}\n` }, "", "estigia-changes"),
];
const doctorCard = doctors[0].appended[0];
const doctorLines = (width) => present.renderers[doctorCard.type](doctorCard, { expanded: false }, undefined).render(width);

console.log(JSON.stringify({
  doctors,
  changes,
  listing,
  shelved,
  doctorTypes: Object.keys(present.renderers),
  doctorWide: doctorCard ? doctorLines(200) : null,
  doctorNarrow: doctorCard ? doctorLines(10) : null,
  report,
  out,
  refused,
  rendererTypes: Object.keys(renderers),
  wide: card ? lines(200) : null,
  narrow: card ? lines(12) : null,
  reason,
}));
"##;

fn run_pi_claim_driver(timeout_ms: u32) -> serde_json::Value {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    let project = base.join("project");
    fs::create_dir_all(&project).expect("the project directory");
    let source = plugin::pi_source(std::path::Path::new(&node));
    let quick = source.replace(
        "const STATUS_TIMEOUT_MS = 5000;",
        &format!("const STATUS_TIMEOUT_MS = {timeout_ms};"),
    );
    let quick = quick.replace(
        "const CHANGES_TIMEOUT_MS = 20000;",
        &format!("const CHANGES_TIMEOUT_MS = {timeout_ms};"),
    );
    assert_ne!(
        source, quick,
        "the status timeout constant is not where the test looks"
    );
    let quick = quick.replace(
        "const DOCTOR_TIMEOUT_MS = 60000;",
        &format!("const DOCTOR_TIMEOUT_MS = {timeout_ms};"),
    );
    let missing = source.replace(
        &serde_json::to_string(&node).expect("a path"),
        "\"/nonexistent/estigia-binary\"",
    );
    assert_ne!(
        source, missing,
        "the binary path is not where the test looks"
    );
    fs::write(base.join("gate.mjs"), &source).expect("the extension writes");
    fs::write(base.join("quick.mjs"), quick).expect("the extension writes");
    fs::write(base.join("missing.mjs"), missing).expect("the extension writes");
    fs::write(base.join("stub.cjs"), PI_CLAIM_STUB).expect("the stand-in writes");
    fs::write(base.join("drive.mjs"), PI_CLAIM_DRIVER).expect("the driver writes");
    let ran = std::process::Command::new(&node)
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.log"))
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("PROJECT", &project)
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the extension would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    let mut said: serde_json::Value =
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON");
    said["project"] = serde_json::json!(project);
    said
}

const PI_SHAPE_DRIVER: &str = r##"import { readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
delete process.env.ESTIGIA_PI_PARENT_SESSION;
delete process.env.ESTIGIA_PI_PARENT_PID;

const calls = () => readFileSync(process.env.STUB_LOG, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const handlers = {};
const appended = [];
(await import(`./${process.env.EXTENSION}`)).default({
  on: (name, handler) => { handlers[name] = handler; },
  registerEntryRenderer: () => {},
  appendEntry: (type, data) => { appended.push({ type, data }); },
});

const ctxOf = (hasUI, notifications, notify = true, throws = false) => ({
  cwd: process.env.PROJECT,
  hasUI,
  sessionManager: { getSessionId: () => "sess-1" },
  ui: notify
    ? { setStatus: () => {}, notify: (text, kind) => { notifications.push({ text, kind }); if (throws) throw new Error("notify broke"); } }
    : { setStatus: () => {} },
});

const drive = async (label, event, exit, { hasUI = true, mode = "", notify = true, throws = false } = {}) => {
  for (const key of ["STUB_GATE_MODE", "STUB_GATE_EXIT", "STUB_GATE_ERR"]) delete process.env[key];
  process.env.STUB_GATE_EXIT = String(exit);
  process.env.STUB_GATE_ERR = exit === 1 || exit === 2 ? "refused: no live claim" : "";
  if (mode) process.env.STUB_GATE_MODE = mode;
  const notifications = [];
  const warnings = [];
  const original = console.error;
  console.error = (line) => warnings.push(String(line));
  const before = calls().length;
  const entries = appended.length;
  const started = Date.now();
  const result = await handlers.tool_call(event, ctxOf(hasUI, notifications, notify, throws));
  console.error = original;
  return { label, result: result ?? null, warnings, notifications, calls: calls().slice(before), cards: appended.slice(entries), took: Date.now() - started };
};

const shell = (toolName) => ({ toolName, input: { command: "git push origin main" } });
const sentTo = (result) => result.calls.map((call) => ({ verb: call.args[1], input: call.args[3] }));

const groups = {
  answered: async () => [
    await drive("background", shell("bash_background"), 1),
    await drive("monitor", shell("monitor"), 2),
    await drive("monitor_ok", shell("monitor"), 0),
    await drive("powershell", shell("PowerShell"), 1),
    await drive("write_shape", { toolName: "save_note", input: { path: "a.rs", content: "x" } }, 1),
    await drive("edit_shape", { toolName: "patch_file", input: { path: "a.rs", edits: [{ oldText: "a", newText: "b" }] } }, 1),
    await drive("path_only", { toolName: "open_file", input: { path: "a.rs" } }, 1),
    await drive("no_shape", { toolName: "search", input: { query: "x" } }, 1),
    await drive("not_a_string", { toolName: "monitor", input: { command: 5 } }, 1),
    await drive("command_and_write", { toolName: "weird", input: { command: "git push", path: "a.rs", content: "x" } }, 1),
    await drive("write_and_edit", { toolName: "weird", input: { path: "a.rs", content: "x", edits: [] } }, 1),
    await drive("string_input", { toolName: "weird", input: "git push" }, 1),
    await drive("null_input", { toolName: "weird", input: null }, 1),
    await drive("missing_input", { toolName: "weird" }, 1),
    await drive("number_input", { toolName: "weird", input: 7 }, 1),
    await drive("named_without_input", { toolName: "bash" }, 0),
    await drive("refused_with_throwing_notify", shell("bash"), 1, { throws: true }),
  ],
  hang: async () => [
    await drive("hang_ui", shell("bash"), 0, { hasUI: true, mode: "sleep" }),
    await drive("hang_headless", shell("bash"), 0, { hasUI: false, mode: "sleep" }),
    await drive("hang_no_notify", shell("bash"), 0, { hasUI: true, mode: "sleep", notify: false }),
  ],
  flood: async () => [
    await drive("flood_ui", shell("bash"), 0, { hasUI: true, mode: "flood" }),
    await drive("flood_headless", shell("bash"), 0, { hasUI: false, mode: "flood" }),
  ],
  big: async () => {
    const content = "x".repeat(3 * 1024 * 1024);
    const result = await drive("big", { toolName: "write", input: { path: "a.rs", content } }, 1);
    return [{ ...result, calls: result.calls.map((call) => ({ args: call.args, bytes: call.stdin.length, parsed: JSON.parse(call.stdin).content.length })) }];
  },
  missing: async () => [
    await drive("missing_ui", shell("bash"), 0, { hasUI: true }),
    await drive("missing_headless", shell("bash"), 0, { hasUI: false }),
    await drive("missing_throwing_notify", shell("bash"), 0, { hasUI: true, throws: true }),
  ],
};

console.log(JSON.stringify(await groups[process.env.GROUP]()));
"##;

fn run_pi_shape_driver(extension: &str, source: &str, group: &str) -> Vec<serde_json::Value> {
    let node = node_executable();
    let root = tempfile::tempdir().expect("a temporary directory");
    let base = node_safe_dir(root.path());
    let project = base.join("project");
    fs::create_dir_all(&project).expect("the project directory");
    fs::write(base.join(extension), source).expect("the extension writes");
    fs::write(base.join("stub.cjs"), PI_CLAIM_STUB).expect("the stand-in writes");
    fs::write(base.join("drive.mjs"), PI_SHAPE_DRIVER).expect("the driver writes");
    let ran = std::process::Command::new(&node)
        .arg(base.join("drive.mjs"))
        .current_dir(&base)
        .env("STUB_LOG", base.join("calls.log"))
        .env(
            "STUB_PATH",
            base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
        )
        .env("EXTENSION", extension)
        .env("GROUP", group)
        .env("PROJECT", &project)
        .env_remove("NODE_OPTIONS")
        .output()
        .expect("node runs");
    assert!(
        ran.status.success(),
        "the extension would not load: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

fn pi_hang_source() -> String {
    let source = plugin::pi_source(std::path::Path::new(&node_executable()));
    let fast = source.replace(
        "const GATE_TIMEOUT_MS = 30000;",
        "const GATE_TIMEOUT_MS = 300;",
    );
    assert_ne!(
        source, fast,
        "the gate timeout constant is not where the test looks"
    );
    fast
}

fn pi_shape_source() -> String {
    plugin::pi_source(std::path::Path::new(&node_executable()))
}

#[test]
fn the_pi_extension_gates_a_tool_by_the_shape_of_its_input_and_not_only_by_its_name() {
    let results = run_pi_shape_driver("gate.mjs", &pi_shape_source(), "answered");
    let seen = |label: &str| {
        results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"))
    };
    let sent = |label: &str| seen(label)["calls"][0]["args"][1].clone();

    for (label, tool, reason) in [
        ("background", "bash_background", "refused: no live claim"),
        ("monitor", "monitor", "refused: no live claim"),
    ] {
        let result = seen(label);
        assert_eq!(sent(label), "bash", "{result}");
        assert_eq!(
            result["calls"][0]["stdin"], "{\"command\":\"git push origin main\"}",
            "{result}"
        );
        assert_eq!(result["result"]["block"], true, "{result}");
        assert_eq!(result["result"]["reason"], reason, "{result}");
        assert_eq!(result["cards"][0]["data"]["tool"], tool, "{result}");
    }
    let allowed = seen("monitor_ok");
    assert!(
        allowed["result"].is_null() && allowed["calls"].as_array().is_some_and(|c| c.len() == 1),
        "{allowed}"
    );

    let powershell = seen("powershell");
    assert_eq!(sent("powershell"), "bash", "{powershell}");
    assert_eq!(powershell["result"]["block"], true, "{powershell}");

    assert_eq!(sent("write_shape"), "write");
    assert_eq!(sent("edit_shape"), "edit");
    for label in ["path_only", "no_shape", "not_a_string"] {
        let result = seen(label);
        assert!(
            result["result"].is_null() && result["calls"] == serde_json::json!([]),
            "{label} was sent to the gate: {result}"
        );
    }

    for (label, verb) in [("command_and_write", "bash"), ("write_and_edit", "write")] {
        let result = seen(label);
        assert_eq!(
            sent(label),
            verb,
            "{label} took the wrong precedence: {result}"
        );
    }
    for label in [
        "string_input",
        "null_input",
        "missing_input",
        "number_input",
    ] {
        let result = seen(label);
        assert!(
            result["result"].is_null() && result["calls"] == serde_json::json!([]),
            "{label} was sent to the gate: {result}"
        );
    }
    let named = seen("named_without_input");
    assert_eq!(named["calls"][0]["stdin"], "{}", "{named}");
    assert!(named["result"].is_null(), "{named}");

    let broken = seen("refused_with_throwing_notify");
    assert_eq!(broken["result"]["block"], true, "{broken}");
    assert_eq!(
        broken["result"]["reason"], "refused: no live claim",
        "{broken}"
    );
}

#[test]
fn the_pi_extension_blocks_an_answer_larger_than_it_can_read_and_does_not_call_it_a_timeout() {
    let results = run_pi_shape_driver("gate.mjs", &pi_shape_source(), "flood");
    for label in ["flood_ui", "flood_headless"] {
        let result = results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"));
        assert_eq!(result["result"]["block"], true, "{result}");
        let reason = result["result"]["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains("larger than this extension can read") && !reason.contains("within"),
            "{result}"
        );
        assert_eq!(result["warnings"], serde_json::json!([]), "{result}");
        assert!(
            result["took"].as_u64().is_some_and(|took| took < 25000),
            "{result}"
        );
    }
}

#[test]
fn the_pi_extension_hands_an_oversized_input_to_the_gate_on_standard_input() {
    let results = run_pi_shape_driver("gate.mjs", &pi_shape_source(), "big");
    let result = &results[0];
    assert_eq!(result["result"]["block"], true, "{result}");
    assert_eq!(
        result["result"]["reason"], "refused: no live claim",
        "{result}"
    );
    assert_eq!(result["warnings"], serde_json::json!([]), "{result}");
    let call = &result["calls"][0];
    assert_eq!(
        call["args"],
        serde_json::json!([
            "gate",
            "write",
            "--input",
            "-",
            "--session-id",
            "sess-1",
            "--runtime",
            "pi"
        ]),
        "{call}"
    );
    assert_eq!(call["parsed"], 3 * 1024 * 1024, "{call}");
}

#[test]
fn the_pi_extension_lets_a_gate_that_cannot_be_started_through_with_a_word_whatever_the_ui_does() {
    let source = plugin::pi_source(std::path::Path::new("/nonexistent-estigia/estigia"));
    let results = run_pi_shape_driver("missing.mjs", &source, "missing");
    for label in ["missing_ui", "missing_headless", "missing_throwing_notify"] {
        let result = results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"));
        assert!(result["result"].is_null(), "{result}");
        assert_eq!(result["calls"], serde_json::json!([]), "{result}");
        let warnings = result["warnings"].as_array().expect("the warnings");
        assert_eq!(warnings.len(), 1, "{result}");
        assert!(
            warnings[0]
                .as_str()
                .is_some_and(|line| line.contains("ENOENT") && line.contains("went out unchecked")),
            "{result}"
        );
    }
}

#[test]
fn the_pi_extension_bounds_the_gate_call_and_shows_the_person_a_write_that_went_out_unchecked() {
    let results = run_pi_shape_driver("gate.mjs", &pi_hang_source(), "hang");
    let seen = |label: &str| {
        results
            .iter()
            .find(|result| result["label"] == label)
            .unwrap_or_else(|| panic!("no result for {label}"))
    };

    let shown = seen("hang_ui");
    assert!(
        shown["result"].is_null(),
        "a timeout is not a refusal: {shown}"
    );
    assert!(
        shown["took"].as_u64().is_some_and(|took| took < 5000),
        "the hung gate was not cut off: {shown}"
    );
    let notifications = shown["notifications"]
        .as_array()
        .expect("the notifications");
    assert_eq!(notifications.len(), 1, "{shown}");
    assert_eq!(notifications[0]["kind"], "error", "{shown}");
    assert!(
        notifications[0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("went out unchecked") && text.contains("bash")),
        "{shown}"
    );
    assert!(
        shown["warnings"][0]
            .as_str()
            .is_some_and(|line| line.contains("went out unchecked")),
        "{shown}"
    );

    let headless = seen("hang_headless");
    assert!(headless["result"].is_null(), "{headless}");
    assert_eq!(
        headless["notifications"],
        serde_json::json!([]),
        "{headless}"
    );
    assert_eq!(
        headless["warnings"].as_array().map(Vec::len),
        Some(1),
        "{headless}"
    );

    let bare = seen("hang_no_notify");
    assert!(bare["result"].is_null(), "{bare}");
    assert_eq!(bare["warnings"].as_array().map(Vec::len), Some(1), "{bare}");
}

#[test]
fn the_pi_extension_shows_the_claim_the_binary_reports_and_clears_it_when_there_is_none() {
    let said = run_pi_claim_driver(400);
    let seen = |label: &str| {
        said["out"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["label"] == label))
            .unwrap_or_else(|| panic!("no result for {label}: {said}"))
            .clone()
    };
    let text = |case: &serde_json::Value| case["statuses"][0]["text"].as_str().map(str::to_owned);

    let held = seen("held");
    assert_eq!(
        held["calls"],
        serde_json::json!([{
            "args": ["goal", "--json", "--session-id", "sess-1", "--runtime", "pi"],
            "cwd": said["project"],
        }])
    );
    assert_eq!(held["statuses"].as_array().map(Vec::len), Some(1), "{held}");
    assert_eq!(held["statuses"][0]["key"], "estigia-claim", "{held}");
    let shown = text(&held).expect("a held claim is shown");
    assert!(
        shown.contains("#42") && shown.contains("in-progress"),
        "{shown}"
    );
    assert!(held["returned"].is_null(), "{held}");

    for label in ["settled", "after_tool"] {
        assert_eq!(
            seen(label)["statuses"].as_array().map(Vec::len),
            Some(1),
            "{label}"
        );
    }
    let stopped = text(&seen("after_tool")).expect("a stopped claim is still named");
    assert!(
        stopped.contains("#7") && stopped.contains("review") && stopped.contains("stopped"),
        "{stopped}"
    );

    let none = seen("none");
    assert_eq!(
        none["statuses"][0]["text"],
        serde_json::Value::Null,
        "{none}"
    );

    for label in ["no_exit", "exit_two", "garbled", "slow"] {
        let case = seen(label);
        let shown = text(&case).unwrap_or_else(|| panic!("{label} must say unknown: {case}"));
        assert!(
            shown.contains("unknown") && !shown.contains("#42"),
            "{label}: {shown}"
        );
    }

    let headless = seen("headless");
    assert_eq!(headless["statuses"], serde_json::json!([]), "{headless}");
    assert_eq!(headless["calls"], serde_json::json!([]), "{headless}");

    let child = seen("child");
    assert_eq!(
        child["calls"][0]["args"],
        serde_json::json!([
            "goal",
            "--json",
            "--session-id",
            "root-9",
            "--runtime",
            "pi"
        ]),
        "{child}"
    );
    assert!(
        text(&child).is_some_and(|shown| shown.contains("#42")),
        "{child}"
    );
}

#[test]
fn the_pi_extension_draws_a_refusal_card_with_the_binarys_words_and_asks_the_model_for_nothing() {
    let said = run_pi_claim_driver(400);
    let refused = |label: &str| {
        said["refused"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["label"] == label))
            .unwrap_or_else(|| panic!("no result for {label}: {said}"))
            .clone()
    };
    let reason = said["reason"].as_str().expect("the reason");

    let one = refused("one");
    assert_eq!(
        one["result"],
        serde_json::json!({ "block": true, "reason": reason })
    );
    assert_eq!(one["sent"], 0, "a card must not become a message: {one}");
    let entries = one["appended"].as_array().expect("entries");
    assert_eq!(entries.len(), 1, "{one}");
    assert_eq!(entries[0]["data"]["reason"], reason, "{one}");
    assert!(
        said["rendererTypes"]
            .as_array()
            .is_some_and(|types| types.contains(&entries[0]["type"])),
        "{said}"
    );

    let wide: Vec<&str> = said["wide"]
        .as_array()
        .expect("the card renders")
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        wide[1..],
        reason.split('\n').collect::<Vec<_>>(),
        "{wide:?}"
    );
    assert!(wide[0].contains("bash"), "{wide:?}");
    for line in said["narrow"].as_array().expect("the card renders narrow") {
        assert!(
            line.as_str().unwrap_or_default().chars().count() <= 12,
            "{line}"
        );
    }

    let two = refused("two");
    assert_eq!(
        two["result"],
        serde_json::json!({ "block": true, "reason": "unread" })
    );
    assert_eq!(two["appended"].as_array().map(Vec::len), Some(1), "{two}");

    for label in ["headless", "allowed"] {
        let case = refused(label);
        assert_eq!(case["appended"], serde_json::json!([]), "{label}: {case}");
        assert_eq!(case["sent"], 0, "{label}: {case}");
    }
    assert!(refused("allowed")["result"].is_null());
}

#[test]
fn the_pi_extension_shows_estigia_doctor_unchanged_to_the_person_and_asks_the_model_for_nothing() {
    let said = run_pi_claim_driver(400);
    let case = |label: &str| {
        said["doctors"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["label"] == label))
            .unwrap_or_else(|| panic!("no result for {label}: {said}"))
            .clone()
    };
    let report = said["report"].as_str().expect("the report");
    let shown = |case: &serde_json::Value| -> String {
        case["appended"][0]["data"]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("nothing was shown: {case}"))
            .to_owned()
    };

    let fine = case("fine");
    assert_eq!(
        fine["calls"],
        serde_json::json!([{ "args": ["doctor"], "cwd": said["project"] }]),
        "{fine}"
    );
    assert_eq!(fine["appended"].as_array().map(Vec::len), Some(1), "{fine}");
    assert_eq!(shown(&fine), report, "{fine}");
    assert_eq!(fine["appended"][0]["data"]["answered"], true, "{fine}");
    assert_eq!(
        fine["sent"], 0,
        "doctor output must not reach the model: {fine}"
    );
    assert_eq!(fine["notes"], serde_json::json!([]), "{fine}");
    assert!(
        said["doctorTypes"]
            .as_array()
            .is_some_and(|types| types.contains(&fine["appended"][0]["type"])),
        "{said}"
    );
    let wide: Vec<&str> = said["doctorWide"]
        .as_array()
        .expect("the report renders")
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect();
    assert_eq!(wide, report.split('\n').collect::<Vec<_>>(), "{wide:?}");
    for line in said["doctorNarrow"]
        .as_array()
        .expect("the report renders narrow")
    {
        assert!(
            line.as_str().unwrap_or_default().chars().count() <= 10,
            "{line}"
        );
    }

    let red = case("red");
    let text = shown(&red);
    assert!(
        text.starts_with(report) && text.contains("refused: two is broken"),
        "{text}"
    );
    assert_eq!(red["appended"][0]["data"]["answered"], true, "{red}");

    let silent = case("silent");
    assert!(shown(&silent).contains("exit 3"), "{silent}");
    assert_eq!(silent["appended"][0]["data"]["answered"], false, "{silent}");

    for label in ["signal", "slow", "missing"] {
        let case = case(label);
        let text = shown(&case);
        assert!(
            text.contains("did not answer") && !text.contains("[ok]"),
            "{label}: {text}"
        );
        assert_eq!(
            case["appended"][0]["data"]["answered"], false,
            "{label}: {case}"
        );
        assert_eq!(case["sent"], 0, "{label}: {case}");
    }
    assert!(shown(&case("slow")).contains("within"));
    let flood = case("flood");
    let text = shown(&flood);
    assert!(
        text.contains("did not answer") && text.contains("larger than 8388608 bytes"),
        "{text}"
    );
    assert_eq!(flood["appended"][0]["data"]["answered"], false, "{flood}");
    assert_eq!(flood["sent"], 0, "{flood}");
    assert!(shown(&case("missing")).contains("ENOENT"));

    let headless = case("headless");
    assert_eq!(headless["appended"], serde_json::json!([]), "{headless}");
    assert_eq!(
        headless["logged"],
        serde_json::json!([report]),
        "{headless}"
    );
    assert_eq!(headless["sent"], 0, "{headless}");

    let gone = case("headless_missing");
    assert_eq!(gone["appended"], serde_json::json!([]), "{gone}");
    assert_eq!(gone["logged"], serde_json::json!([]), "{gone}");
    assert!(
        gone["errors"][0]
            .as_str()
            .is_some_and(|said| said.contains("did not answer")),
        "{gone}"
    );
}

#[test]
fn the_pi_extension_shows_a_widget_counting_the_checkouts_that_hold_uncommitted_work() {
    let said = run_pi_claim_driver(400);
    let case = |label: &str| {
        said["shelved"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["label"] == label))
            .unwrap_or_else(|| panic!("no result for {label}: {said}"))
            .clone()
    };
    let shown = |label: &str| {
        let case = case(label);
        assert_eq!(case["widgets"].as_array().map(Vec::len), Some(1), "{case}");
        assert_eq!(case["widgets"][0]["key"], "estigia-changes", "{case}");
        case["widgets"][0]["content"].clone()
    };

    let started = case("started");
    assert_eq!(
        started["calls"],
        serde_json::json!([{ "args": ["changes", "--json"], "cwd": said["project"] }]),
        "{started}"
    );
    assert_eq!(
        shown("started"),
        serde_json::json!(["estigia: uncommitted work in 2 checkouts \u{b7} /estigia-changes"])
    );
    assert_eq!(
        shown("one"),
        serde_json::json!(["estigia: uncommitted work in 1 checkout \u{b7} /estigia-changes"])
    );
    assert_eq!(
        shown("mixed"),
        serde_json::json!([
            "estigia: uncommitted work in 1 checkout, 1 unreadable \u{b7} /estigia-changes"
        ])
    );
    assert_eq!(
        shown("unreadable_only"),
        serde_json::json!(["estigia: 1 checkout unreadable \u{b7} /estigia-changes"])
    );
    assert!(
        shown("clean").is_null(),
        "a clean repository clears the widget"
    );
    for label in ["silent", "garbled", "slow"] {
        assert_eq!(
            shown(label),
            serde_json::json!(["estigia: uncommitted work unknown \u{b7} /estigia-changes"]),
            "{label}"
        );
    }
    for label in ["headless", "no_widget_surface"] {
        let case = case(label);
        assert_eq!(case["widgets"], serde_json::json!([]), "{case}");
        assert_eq!(case["calls"], serde_json::json!([]), "{case}");
    }
}

#[test]
fn the_pi_extension_shows_estigia_changes_to_the_person_and_asks_the_model_for_nothing() {
    let said = run_pi_claim_driver(400);
    let case = |label: &str| {
        said["changes"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["label"] == label))
            .unwrap_or_else(|| panic!("no result for {label}: {said}"))
            .clone()
    };
    let listing = said["listing"].as_str().expect("the listing");

    let listed = case("listed");
    assert_eq!(
        listed["calls"],
        serde_json::json!([{ "args": ["changes"], "cwd": said["project"] }]),
        "{listed}"
    );
    assert_eq!(
        listed["appended"].as_array().map(Vec::len),
        Some(1),
        "{listed}"
    );
    assert_eq!(listed["appended"][0]["data"]["text"], listing, "{listed}");
    assert_eq!(listed["appended"][0]["data"]["answered"], true, "{listed}");
    assert_eq!(
        listed["sent"], 0,
        "changes output must not reach the model: {listed}"
    );
    assert_eq!(listed["notes"], serde_json::json!([]), "{listed}");
    assert!(
        said["doctorTypes"]
            .as_array()
            .is_some_and(|types| types.contains(&listed["appended"][0]["type"])),
        "{said}"
    );

    for label in ["silent", "missing"] {
        let case = case(label);
        let text = case["appended"][0]["data"]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(
            text.contains("estigia changes did not answer"),
            "{label}: {case}"
        );
        assert_eq!(
            case["appended"][0]["data"]["answered"], false,
            "{label}: {case}"
        );
        assert_eq!(case["sent"], 0, "{label}: {case}");
    }

    let headless = case("headless");
    assert_eq!(headless["appended"], serde_json::json!([]), "{headless}");
    assert_eq!(
        headless["logged"],
        serde_json::json!([listing]),
        "{headless}"
    );
}

fn pi_home(options: &SetupOptions, directory: &Path) -> SetupOptions {
    SetupOptions {
        pi_agent_dir: Some(directory.to_path_buf()),
        ..options.clone()
    }
}

#[test]
fn pi_setup_writes_under_the_agent_directory_pi_is_told_to_read() {
    let (home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let directory = elsewhere.path().join("pi-home");
    let options = pi_home(&options, &directory);
    let adapter = agent("pi");

    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(paths.instructions, directory.join("AGENTS.md"));
    assert_eq!(paths.mcp_config, Some(directory.join("mcp.json")));
    assert_eq!(paths.agents_root, Some(directory.join("agents")));
    let extensions = directory.join(plugin::PI_DIRECTORY);
    assert_eq!(paths.plugin, Some(extensions.join(plugin::PI_FILE)));
    assert_eq!(
        paths.loop_plugin,
        Some(extensions.join(plugin::PI_LOOP_FILE))
    );

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(is_gated(adapter, &options));
    assert!(directory.join("AGENTS.md").is_file());
    assert!(directory.join("mcp.json").is_file());
    assert!(
        !home.path().join(".pi").exists(),
        "the default directory was written while another was named"
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!is_gated(adapter, &options));
}

#[test]
fn pi_setup_writes_its_two_prompt_templates_and_uninstall_takes_back_exactly_those() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let directory = home.path().join(".pi").join("agent").join("prompts");
    let paths = resolve_paths(adapter, &options).expect("paths resolve");
    assert_eq!(paths.prompts, Some(directory.clone()));
    for other in AGENTS.iter().filter(|other| other.slug != "pi") {
        assert_eq!(
            resolve_paths(other, &options)
                .expect("paths resolve")
                .prompts,
            None,
            "{} names a prompt directory",
            other.slug
        );
    }
    let names: Vec<&str> = plugin::PI_PROMPTS.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["claim.md", "review-status.md"]);

    fs::create_dir_all(&directory).expect("the prompt directory");
    let mine = directory.join("mine.md");
    fs::write(&mine, "---\ndescription: mine\n---\nmine\n").expect("their template");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    for (name, source) in plugin::PI_PROMPTS {
        let written = fs::read_to_string(directory.join(name)).expect("the template reads");
        assert_eq!(written, *source);
        assert!(written.starts_with("---\ndescription: "), "{name}");
        assert!(plugin::prompt_is_ours(Some(&written)), "{name}");
    }
    let claim = fs::read_to_string(directory.join("claim.md")).expect("claim reads");
    assert!(
        claim.contains("`claim`") && claim.contains("$ARGUMENTS"),
        "{claim}"
    );
    let status = fs::read_to_string(directory.join("review-status.md")).expect("status reads");
    assert!(
        status.contains("`review_status`") && status.contains("$ARGUMENTS"),
        "{status}"
    );

    let again = setup(adapter, &Config::default(), &options).expect("the second setup succeeds");
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.change == Change::Unchanged),
        "{:?}",
        again.actions
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    for name in names {
        assert!(
            !directory.join(name).exists(),
            "{name} survived an uninstall"
        );
    }
    assert_eq!(
        fs::read_to_string(&mine).expect("their template survives"),
        "---\ndescription: mine\n---\nmine\n"
    );
}

#[test]
fn a_prompt_template_estigia_did_not_write_is_neither_replaced_nor_removed_for_pi() {
    let (_home, options) = sandbox();
    let adapter = agent("pi");
    let directory = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .prompts
        .expect("Pi reads prompt templates");
    fs::create_dir_all(&directory).expect("create the directory");
    let theirs = "---\ndescription: theirs\n---\ntheir own claim\n";
    let file = directory.join("claim.md");
    fs::write(&file, theirs).expect("write their template");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
    assert!(directory.join("review-status.md").is_file());

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
    assert!(!directory.join("review-status.md").exists());
}

#[test]
fn pi_prompt_templates_follow_the_agent_directory_pi_is_told_to_read() {
    let (home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let directory = elsewhere.path().join("pi-home");
    let options = pi_home(&options, &directory);
    let adapter = agent("pi");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    for (name, _) in plugin::PI_PROMPTS {
        assert!(directory.join("prompts").join(name).is_file(), "{name}");
    }
    assert!(!home.path().join(".pi").exists());

    uninstall(adapter, &options).expect("uninstall succeeds");
    for (name, _) in plugin::PI_PROMPTS {
        assert!(!directory.join("prompts").join(name).exists(), "{name}");
    }
}

#[test]
fn doctor_follows_the_agent_directory_pi_is_told_to_read() {
    let (home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let named = pi_home(&options, &elsewhere.path().join("pi-home"));
    let adapter = agent("pi");
    setup(adapter, &Config::default(), &options).expect("setup under the default directory");

    let skill_root = resolve_paths(adapter, &options)
        .expect("paths resolve")
        .skill_root;
    let run = |options: &SetupOptions| {
        crate::harness::doctor::full(
            Some(&skill_root),
            home.path(),
            &Config::default().tracker,
            options,
        )
    };
    let gates = |checks: &[crate::harness::doctor::Check]| {
        checks.iter().filter(|check| check.name == "gate").count()
    };
    let pi_config = |checks: &[crate::harness::doctor::Check]| {
        checks
            .iter()
            .find(|check| check.name == "pi-config")
            .map(|check| check.health.clone())
            .expect("doctor has a pi-config row")
    };
    let not_configured = |health: &crate::harness::doctor::Health| {
        matches!(
            health,
            crate::harness::doctor::Health::Skipped { detail } if detail == "pi is not configured"
        )
    };

    let default = run(&options);
    assert_eq!(
        gates(&default),
        1,
        "the install under the default directory"
    );
    assert!(!not_configured(&pi_config(&default)));

    let moved = run(&named);
    assert_eq!(
        gates(&moved),
        0,
        "doctor reports a Pi gate installed where Pi, told to read another directory, does not"
    );
    assert!(
        not_configured(&pi_config(&moved)),
        "{:?}",
        pi_config(&moved)
    );

    fs::create_dir_all(named.pi_agent_dir.as_ref().expect("named")).expect("the directory");
    fs::write(
        named
            .pi_agent_dir
            .as_ref()
            .expect("named")
            .join("settings.json"),
        r#"{"extensions":["-builtin:mcp"]}"#,
    )
    .expect("a settings file");
    setup(adapter, &Config::default(), &named).expect("setup under the named directory");
    let after = run(&named);
    assert_eq!(gates(&after), 1);
    assert!(
        matches!(
            pi_config(&after),
            crate::harness::doctor::Health::Broken { detail, .. } if detail.contains("builtin:mcp")
        ),
        "the pi-config row read the default directory: {:?}",
        pi_config(&after)
    );
}

#[test]
fn doctor_reports_the_permission_mode_of_every_configured_host_it_can_read() {
    let (home, options) = sandbox();
    let repo = tempfile::tempdir().expect("a repository");
    for slug in ["claude-code", "codex", "opencode", "pi"] {
        setup(agent(slug), &Config::default(), &options).expect("setup succeeds");
    }
    fs::create_dir_all(repo.path().join(".claude")).expect("a project settings directory");
    fs::write(
        repo.path().join(".claude").join("settings.json"),
        r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#,
    )
    .expect("a project settings file");
    fs::create_dir_all(home.path().join(".codex")).expect("a codex directory");
    fs::write(
        home.path().join(".codex").join("config.toml"),
        "approval_policy = [",
    )
    .expect("a broken codex config");

    let skill_root = resolve_paths(agent("claude-code"), &options)
        .expect("paths resolve")
        .skill_root;
    let rows: Vec<crate::harness::doctor::Check> = crate::harness::doctor::full(
        Some(&skill_root),
        repo.path(),
        &Config::default().tracker,
        &options,
    )
    .into_iter()
    .filter(|check| check.name == "permission")
    .collect();
    let text = |slug: &str| {
        rows.iter()
            .filter_map(|check| match &check.health {
                crate::harness::doctor::Health::Fine { detail }
                | crate::harness::doctor::Health::Skipped { detail } => Some(detail.as_str()),
                _ => None,
            })
            .find(|detail| {
                detail.starts_with(&format!("{slug}:"))
                    || detail.starts_with(&format!("{slug} has"))
                    || detail.contains(&format!("; {slug}:"))
            })
            .unwrap_or_else(|| panic!("no permission row for {slug}: {rows:?}"))
    };
    assert!(text("claude-code").contains("bypassPermissions"));
    assert!(text("codex").contains("could not be read"));
    assert!(text("pi").contains("no permission prompts"));
    assert!(text("opencode").contains("no permission mode"));
    assert_eq!(rows.len(), 4, "{rows:?}");
}

#[test]
fn the_environment_variable_moves_pi_and_a_relative_value_is_noted_and_not_followed() {
    let home = tempfile::tempdir().expect("a home");
    let outside = tempfile::tempdir().expect("a directory outside the home");
    let adapter = agent("pi");
    let options = SetupOptions {
        platform: Some(Platform::Unix),
        ..SetupOptions::default()
    };

    with_home_env_and_pi_agent_dir(
        home.path(),
        outside.path(),
        outside.path(),
        outside.path(),
        || {
            let paths = resolve_paths(adapter, &options).expect("paths resolve");
            assert_eq!(paths.instructions, outside.path().join("AGENTS.md"));
            assert!(paths.notes.is_empty(), "{:?}", paths.notes);
        },
    );

    with_home_env_and_pi_agent_dir(
        home.path(),
        outside.path(),
        outside.path(),
        Path::new("relative/pi"),
        || {
            let paths = resolve_paths(adapter, &options).expect("paths resolve");
            assert_eq!(
                paths.instructions,
                home.path().join(".pi").join("agent").join("AGENTS.md")
            );
            assert!(
                paths
                    .notes
                    .iter()
                    .any(|note| note.contains("PI_CODING_AGENT_DIR is not an absolute path")),
                "{:?}",
                paths.notes
            );
        },
    );
}

#[test]
fn the_gate_side_reading_of_the_pi_agent_directory_expands_the_home_and_refuses_the_rest() {
    let home = tempfile::tempdir().expect("a home");
    let outside = tempfile::tempdir().expect("a directory outside the home");
    let read = |value: &str| {
        with_home_env_and_pi_agent_dir(
            home.path(),
            outside.path(),
            outside.path(),
            Path::new(value),
            pi_agent_dir,
        )
    };
    assert_eq!(read("~"), Some(home.path().to_path_buf()));
    assert_eq!(read("~/sub"), Some(home.path().join("sub")));
    assert_eq!(read(""), None);
    assert_eq!(read("relative/pi"), None);
    assert_eq!(read("~someone/pi"), None);
    let absolute = outside.path().join("pi-home");
    assert_eq!(read(absolute.to_str().expect("utf-8")), Some(absolute));
}

#[test]
fn an_exported_pi_agent_directory_does_not_reach_a_sandboxed_home() {
    let (home, options) = sandbox();
    let elsewhere = tempfile::tempdir().expect("a directory outside the home");
    let adapter = agent("pi");
    with_process_env(
        "PI_CODING_AGENT_DIR",
        elsewhere.path().to_str().expect("utf-8"),
        || {
            let paths = resolve_paths(adapter, &options).expect("paths resolve");
            assert_eq!(
                paths.instructions,
                home.path().join(".pi").join("agent").join("AGENTS.md")
            );
        },
    );
}

#[test]
fn a_workspace_install_refuses_pi_and_writes_nothing_for_it() {
    let (_home, global) = sandbox();
    let workspace = tempfile::tempdir().expect("a temporary workspace");
    let options = SetupOptions {
        scope: InstallScope::Workspace,
        workspace: Some(workspace.path().to_path_buf()),
        ..global
    };

    let error = setup(agent("pi"), &Config::default(), &options)
        .expect_err("--scope=workspace must refuse pi")
        .to_string();
    assert!(error.contains("--scope=workspace"), "{error}");
    assert!(error.contains("pi"), "{error}");
    assert!(
        fs::read_dir(workspace.path())
            .expect("the workspace")
            .next()
            .is_none(),
        "a refused install left files in the workspace"
    );
}

const OPENCODE_MAIL_DRIVER: &str = r#"import plugin, { EstigiaWorkflowAuthority } from "./estigia-workflow-authority.mjs";

const stub = (mail, calls) => (strings, ...values) => {
  const session = strings[1]?.trim() === "session";
  const call = session
    ? ["session", String(values[1]), "--session-id", String(values[2]), "--runtime", String(values[3])]
    : ["gate", ...values.slice(1).map(String)];
  const chain = {
    cwd() { return chain; },
    quiet() { return chain; },
    nothrow() {
      calls.push(call);
      const taking = session && call[1] === "deliver";
      return Promise.resolve({ exitCode: 0, stdout: taking ? (mail.shift() ?? "") : "" });
    },
  };
  return chain;
};

const verbs = (calls) => calls.filter((call) => call[0] === "session");

const seenV1 = [];
const hooks = await EstigiaWorkflowAuthority({
  $: stub(["WAKE UP\n", "", "LOST\n"], seenV1),
  directory: "/p",
});
const first = { output: "file contents" };
await hooks["tool.execute.after"]({ tool: "read", sessionID: "ses_a" }, first);
const second = { output: "more" };
await hooks["tool.execute.after"]({ tool: "read", sessionID: "ses_a" }, second);
const structured = { output: { not: "text" } };
await hooks["tool.execute.after"]({ tool: "read", sessionID: "ses_a" }, structured);

const record = (handed, kind, fails) => async (argument) => {
  handed.push([kind, argument]);
  if (fails) throw new Error("the service did not answer");
};

const deliverTo = async (sessionID, mail, session, calls = []) => {
  const registered = [];
  globalThis.Bun = { $: stub(mail, calls) };
  await plugin.setup({
    location: { directory: "/p" },
    tool: { hook: async (name, callback) => { registered.push([name, callback]); } },
    session,
  });
  await registered.find(([name]) => name === "execute.after")[1]({ tool: "read", sessionID });
};

const steered = [];
const steeredCalls = [];
await deliverTo(
  "ses_b",
  ["STEER ME\n"],
  { prompt: record(steered, "prompt", false), synthetic: record(steered, "synthetic", false) },
  steeredCalls,
);

const refused = [];
await deliverTo(
  "ses_c",
  ["FALL BACK\n"],
  { prompt: record(refused, "prompt", true), synthetic: record(refused, "synthetic", false) },
);

const absent = [];
await deliverTo("ses_d", ["NO PROMPT\n"], { synthetic: record(absent, "synthetic", false) });

const bareCalls = [];
await deliverTo("ses_e", ["KEEP ME\n"], {}, bareCalls);

console.log(JSON.stringify({
  bare: verbs(bareCalls),
  v1: {
    first: first.output,
    second: second.output,
    structured: structured.output,
    asked: verbs(seenV1),
  },
  steered: { handed: steered, asked: verbs(steeredCalls) },
  refused,
  absent,
}));
"#;

fn run_opencode_mail_driver() -> serde_json::Value {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(
        root.path().join("estigia-workflow-authority.mjs"),
        crate::setup::plugin::source(std::path::Path::new("/opt/estigia")),
    )
    .expect("the plugin writes");
    let driver = root.path().join("drive-mail.mjs");
    std::fs::write(&driver, OPENCODE_MAIL_DRIVER).expect("the driver writes");
    let ran = std::process::Command::new("node")
        .arg(&driver)
        .current_dir(root.path())
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "this test drives the generated OpenCode plugin, which is JavaScript, and `node` \
                 could not be run ({error}). Install Node and re-run; it is not skipped, because \
                 a plugin nothing executes is a delivery nothing measures."
            )
        });
    assert!(
        ran.status.success(),
        "the mail driver failed: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
}

#[test]
fn the_opencode_plugin_appends_waiting_mail_to_a_tool_result_after_registering_the_session() {
    let said = run_opencode_mail_driver();
    let v1 = &said["v1"];
    let first = v1["first"].as_str().expect("the first result");
    assert!(
        first.starts_with("file contents\n\n") && first.ends_with("WAKE UP"),
        "{first}"
    );
    assert_eq!(v1["second"], "more", "no mail changed the result: {v1}");
    assert_eq!(
        v1["structured"],
        serde_json::json!({"not": "text"}),
        "a result the plugin cannot append to was left alone"
    );
    assert_eq!(
        v1["asked"],
        serde_json::json!([
            [
                "session",
                "start",
                "--session-id",
                "ses_a",
                "--runtime",
                "opencode"
            ],
            [
                "session",
                "deliver",
                "--session-id",
                "ses_a",
                "--runtime",
                "opencode"
            ],
            [
                "session",
                "deliver",
                "--session-id",
                "ses_a",
                "--runtime",
                "opencode"
            ]
        ]),
        "a session is registered once, a mailbox is not read for a result that cannot carry it"
    );
}

#[test]
fn the_opencode_plugin_steers_mail_in_through_the_service_prompt_when_it_answers() {
    let said = run_opencode_mail_driver();
    let handed = said["steered"]["handed"]
        .as_array()
        .expect("what was handed");
    assert_eq!(handed.len(), 1, "{handed:?}");
    assert_eq!(handed[0][0], "prompt");
    assert_eq!(handed[0][1]["sessionID"], "ses_b");
    assert_eq!(handed[0][1]["delivery"], "steer");
    assert_eq!(handed[0][1]["text"], "STEER ME");
}

#[test]
fn the_opencode_plugin_falls_back_to_a_synthetic_message_when_the_prompt_fails_or_is_absent() {
    let said = run_opencode_mail_driver();
    for (case, text) in [("refused", "FALL BACK"), ("absent", "NO PROMPT")] {
        let handed = said[case].as_array().expect("what was handed");
        let kinds: Vec<&str> = handed.iter().filter_map(|h| h[0].as_str()).collect();
        assert_eq!(kinds.last(), Some(&"synthetic"), "{case}: {handed:?}");
        let last = handed.last().expect("a hand-over");
        assert_eq!(last[1]["text"], text, "{case}");
        assert_eq!(last[1]["resume"], false, "{case}");
        assert!(
            handed
                .iter()
                .all(|h| h[0] != "prompt" || h[1]["delivery"] == "steer"),
            "{case}: {handed:?}"
        );
    }
}

#[test]
fn the_opencode_plugin_leaves_the_mailbox_unread_when_the_host_offers_no_way_to_hand_mail_over() {
    let said = run_opencode_mail_driver();
    assert_eq!(
        said["bare"],
        serde_json::json!([]),
        "a host with neither prompt nor synthetic had its mailbox read, so the rows would be stamped delivered and lost"
    );
}

#[test]
fn pi_installs_reports_and_removes_the_subagent_runner_on_every_install() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let file = pi_goal_directory(home.path()).join(plugin::PI_AGENTS_FILE);

    let dry = SetupOptions {
        dry_run: true,
        ..options.clone()
    };
    let reported = setup(adapter, &Config::default(), &dry).expect("the dry run succeeds");
    assert!(
        reported
            .actions
            .iter()
            .any(|action| action.kind == ActionKind::AgentsExtension && action.path == file),
        "{:?}",
        reported.actions
    );
    assert!(!file.exists(), "the dry run wrote the runner");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let written = fs::read_to_string(&file).expect("the runner reads");
    assert_eq!(written, plugin::pi_agents_source());
    assert!(plugin::agents_is_ours(Some(&written)));
    assert!(!written.contains("__"), "a placeholder was left unfilled");

    let again = setup(adapter, &Config::default(), &options).expect("the second setup succeeds");
    assert!(
        again
            .actions
            .iter()
            .all(|action| action.change == Change::Unchanged),
        "{:?}",
        again.actions
    );

    uninstall(adapter, &options).expect("uninstall succeeds");
    assert!(!file.exists(), "uninstall left the runner behind");
}

#[test]
fn a_pi_subagent_runner_estigia_did_not_write_is_neither_replaced_nor_removed() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let file = pi_goal_directory(home.path()).join(plugin::PI_AGENTS_FILE);
    fs::create_dir_all(file.parent().expect("a parent")).expect("create the dir");
    let theirs = "export default function () {}\n";
    fs::write(&file, theirs).expect("write their extension");

    setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(fs::read_to_string(&file).expect("it survives"), theirs);
}

#[cfg(unix)]
const PI_AGENTS_FAKE_PI: &str = r##"#!__NODE__
import { appendFileSync, writeFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { pathToFileURL } from "node:url";

writeFileSync(process.env.FAKE_PID, String(process.pid));
appendFileSync(process.env.FAKE_SEEN, JSON.stringify({ argv: process.argv.slice(2) }) + "\n");
const out = (record) => process.stdout.write(JSON.stringify(record) + "\n");
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const handlers = {};
const commands = [];
if (!process.env.FAKE_NO_GATE) {
  (await import(pathToFileURL(process.env.GATE).href)).default({
    on: (name, handler) => { handlers[name] = handler; },
    registerCommand: (name) => { commands.push(name); },
    registerEntryRenderer: () => {},
    appendEntry: () => {},
    sendMessage: () => {},
    sendUserMessage: () => {},
  });
}
const ctx = { cwd: process.cwd(), sessionManager: { getSessionId: () => "child-7" } };

const work = async () => {
  await handlers.session_start?.({ type: "session_start" }, ctx);
  const decided = await handlers.tool_call?.({ toolName: "write", input: { path: "a.rs", content: "x" } }, ctx);
  out({ type: "tool_execution_start", toolCallId: "t1", toolName: "write", args: { path: "a.rs" } });
  out({
    type: "tool_execution_end",
    toolCallId: "t1",
    toolName: "write",
    isError: Boolean(decided?.block),
    result: { content: [{ type: "text", text: decided?.block ? decided.reason : "written" }] },
  });
  out({ type: "message_update", usage: { totalTokens: 700, cost: { total: 0.005 } }, assistantMessageEvent: { type: "text_delta", delta: "d" } });
  out({
    type: "message_end",
    message: { role: "assistant", content: [{ type: "text", text: "all done" }], usage: { totalTokens: 1500, cost: { total: 0.0123 } } },
  });
  if (process.env.FAKE_HANG) {
    setInterval(() => {}, 1000);
    await new Promise(() => {});
  }
  await pause(Number(process.env.FAKE_HOLD_MS ?? 0));
  out({ type: "agent_settled" });
  process.exit(0);
};

const lines = createInterface({ input: process.stdin });
for await (const line of lines) {
  const command = JSON.parse(line);
  appendFileSync(process.env.FAKE_SEEN, JSON.stringify({ type: command.type, message: command.message }) + "\n");
  if (command.type === "get_commands") {
    out({ id: command.id, type: "response", command: "get_commands", success: true, data: { commands: commands.map((name) => ({ name, source: "extension" })) } });
  } else if (command.type === "get_state") {
    out({ id: command.id, type: "response", command: "get_state", success: true, data: { model: { id: "fake-model" } } });
  } else if (command.type === "prompt") {
    out({ id: command.id, type: "response", command: "prompt", success: true, data: { disposition: "started" } });
    work();
  }
}
"##;

#[cfg(unix)]
const PI_AGENTS_DRIVER: &str = r##"import { existsSync, readFileSync, writeFileSync } from "node:fs";

writeFileSync(process.env.STUB_LOG, "");
process.env.NODE_OPTIONS = `--require "${process.env.STUB_PATH}"`;
process.env.PI_CODING_AGENT_DIR = process.env.AGENT_DIR;
delete process.env.ESTIGIA_PI_PARENT_SESSION;
delete process.env.ESTIGIA_PI_PARENT_PID;

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const until = async (check, ms = 8000) => {
  for (let waited = 0; waited < ms; waited += 25) {
    const found = check();
    if (found) return found;
    await pause(25);
  }
  return null;
};
const jsonLines = (file) => readFileSync(file, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const gate = {};
(await import("./gate.mjs")).default({
  on: (name, handler) => { gate[name] = handler; },
  registerCommand: () => {},
  registerEntryRenderer: () => {},
  appendEntry: () => {},
  sendMessage: () => {},
  sendUserMessage: () => {},
});

const agents = {};
const piCalls = [];
const appended = [];
agents.on = {};
const record = (name, impl) => (...args) => { piCalls.push(name); return impl?.(...args); };
(await import("./agents.mjs")).default({
  registerTool: record("registerTool", (tool) => { agents.tool = tool; }),
  registerCommand: record("registerCommand", (name, options) => { agents.command = { name, ...options }; }),
  registerShortcut: record("registerShortcut", (key, options) => { agents.shortcut = { key, ...options }; }),
  on: record("on", (name, handler) => { agents.on[name] = handler; }),
  appendEntry: record("appendEntry", (type, data) => { appended.push({ type, data }); }),
  sendMessage: record("sendMessage"),
  sendUserMessage: record("sendUserMessage"),
});

let factory;
let cleared = 0;
let view;
let closed = 0;
let confirmAnswer = true;
let asked = 0;
let inputs = 0;
const notes = [];
const ctx = {
  cwd: process.env.PROJECT,
  hasUI: true,
  sessionManager: { getSessionId: () => "root-1" },
  ui: {
    setWidget: (key, made) => { if (key === "estigia-changes") return; if (made === undefined) cleared += 1; else factory = made; },
    notify: (text) => { notes.push(text); },
    confirm: async () => { asked += 1; return confirmAnswer; },
    input: async () => { inputs += 1; return "please add tests"; },
    custom: (make) => { view = make({ requestRender() {} }, {}, {}, () => { closed += 1; }); return Promise.resolve(); },
  },
};
const card = () => (factory ? factory({ requestRender() {} }, {}).render(200) : []);
const rowOf = (label) => card().filter((line) => line.includes(`${label} (developer)`));

const seen = () => jsonLines(process.env.FAKE_SEEN);
const forget = () => writeFileSync(process.env.FAKE_SEEN, "");
const settings = (env) => {
  for (const key of ["FAKE_HOLD_MS", "FAKE_HANG", "FAKE_NO_GATE", "STUB_GATE_EXIT", "STUB_GATE_ERR"]) delete process.env[key];
  Object.assign(process.env, env);
};

await gate.session_start({ type: "session_start" }, ctx);
const exported = process.env.ESTIGIA_PI_PARENT_SESSION;

settings({ FAKE_HOLD_MS: "1600", STUB_GATE_EXIT: "0" });
forget();
const running = agents.tool.execute("c1", { agent: "developer", task: "do it", label: "alpha" }, undefined, undefined, ctx);
const live = await until(() => {
  const row = rowOf("alpha").join("\n");
  return row.includes("1.5k tok") && /\b1s\b/.test(row) ? card() : null;
});
const settled = await running;
const gateCalls = jsonLines(process.env.STUB_LOG);
const childArgv = seen().filter((entry) => entry.argv).map((entry) => entry.argv);
const finalRow = rowOf("alpha");

settings({ FAKE_HOLD_MS: "300", STUB_GATE_EXIT: "1", STUB_GATE_ERR: "estigia: refused: no live claim\n  run `claim` first\nsecond clause" });
const refusedRun = agents.tool.execute("c2", { agent: "developer", task: "write", label: "beta" }, undefined, undefined, ctx);
const refused = await until(() => (rowOf("beta").length > 0 && card().join("\n").includes("second clause") ? card() : null));
await refusedRun;

settings({ FAKE_HANG: "1", STUB_GATE_EXIT: "0" });
const hanging = agents.tool.execute("c3", { agent: "developer", task: "wait", label: "gamma" }, undefined, undefined, ctx);
await until(() => rowOf("gamma").join("\n").includes("1.5k tok"));
const pid = Number(readFileSync(process.env.FAKE_PID, "utf8"));
const aliveBefore = (() => { try { process.kill(pid, 0); return true; } catch { return false; } })();
await agents.command.handler("stop all", ctx);
const stopped = await Promise.race([hanging, pause(6000).then(() => ({ content: [{ type: "text", text: "still running" }], details: { status: "running" } }))]);
const gone = await until(() => { try { process.kill(pid, 0); return null; } catch { return true; } }, 6000);

const timeout = pause(6000).then(() => ({ content: [{ type: "text", text: "still running" }], details: { status: "running" } }));
const press = async (...keys) => { for (const key of keys) await view.handleInput(key); };
const steers = () => seen().filter((entry) => entry.type === "steer");

settings({ FAKE_HANG: "1", STUB_GATE_EXIT: "0" });
forget();
const zeta = agents.tool.execute("c7", { agent: "developer", task: "look", label: "omega" }, undefined, undefined, ctx);
await until(() => rowOf("omega").join("\n").includes("1.5k tok"));
await agents.command.handler("", ctx);
const viewAtStart = view.render(200);
await press("j");
const viewOnFinished = view.render(200);
await press("f");
const steersOnFinished = steers().length;
const inputsOnFinished = inputs;
await press("k", "f");
const steered = await until(() => (steers().length > 0 ? steers() : null));
const viewAfterSteer = view.render(200);
const promptsWhileViewing = seen().filter((entry) => entry.type === "prompt").length;
await press("s");
const zetaStopped = await Promise.race([zeta, timeout]);
asked = 0;

forget();
const eta = agents.tool.execute("c8", { agent: "developer", task: "more", label: "iota" }, undefined, undefined, ctx);
await until(() => rowOf("iota").join("\n").includes("1.5k tok"));
confirmAnswer = false;
await press("x");
const askedWhenDeclined = asked;
const etaWhenDeclined = rowOf("iota").join("\n");
confirmAnswer = true;
await press("x");
const etaStopped = await Promise.race([eta, timeout]);
await press("q");
const closedAfterQ = closed;

const entries = [
  { type: "custom", customType: "estigia-agent", data: { id: "a99", agent: "analyst", label: "old-one", status: "done", model: "m", tokens: 2000, cost: 0.5, startedAt: 1000, endedAt: 61000, task: "old task", text: "yesterday" } },
  ...appended.map((entry) => ({ type: "custom", customType: entry.type, data: entry.data })),
  { type: "custom", customType: "something-else", data: { id: "a98", agent: "x", label: "decoy", status: "done", tokens: 0, cost: 0, startedAt: 0, endedAt: 1, task: "t", text: "t" } },
  { type: "message" },
];
agents.on.session_start({ type: "session_start" }, { sessionManager: { getEntries: () => entries } });
await agents.command.handler("", ctx);
await press(...Array(40).fill("j"));
const resumed = view.render(200);
await press("q");

settings({ FAKE_NO_GATE: "1" });
forget();
const ungated = await agents.tool.execute("c4", { agent: "developer", task: "x", label: "delta" }, undefined, undefined, ctx);
const ungatedSeen = seen();

settings({});
forget();
const exportedSession = process.env.ESTIGIA_PI_PARENT_SESSION;
delete process.env.ESTIGIA_PI_PARENT_SESSION;
const unlinked = await agents.tool.execute("c5", { agent: "developer", task: "x", label: "eps" }, undefined, undefined, ctx);
const unlinkedSeen = seen();
process.env.ESTIGIA_PI_PARENT_SESSION = exportedSession;
const missing = await agents.tool.execute("c6", { agent: "nobody", task: "x" }, undefined, undefined, ctx);

try { process.kill(pid, "SIGKILL"); } catch {}
console.log(JSON.stringify({
  exported,
  live,
  settled: settled.content[0].text,
  gateCalls,
  childArgv,
  finalRow,
  refused,
  stopped: stopped.content[0].text,
  stoppedStatus: stopped.details.status,
  aliveBefore,
  gone: gone === true,
  ungated: ungated.content[0].text,
  ungatedSeen,
  unlinked: unlinked.content[0].text,
  unlinkedSeen,
  missing: missing.content[0].text,
  command: agents.command.name,
  shortcut: agents.shortcut.key,
  viewAtStart,
  viewOnFinished,
  viewAfterSteer,
  steersOnFinished,
  inputsOnFinished,
  steered,
  promptsWhileViewing,
  zetaStopped: zetaStopped.details.status,
  askedWhenDeclined,
  etaWhenDeclined,
  asked,
  etaStopped: etaStopped.details.status,
  closedAfterQ,
  closed,
  appended,
  piCalls,
  resumed,
}));
"##;

#[cfg(unix)]
fn run_pi_agents_driver() -> &'static serde_json::Value {
    use std::os::unix::fs::PermissionsExt;
    static SAID: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    SAID.get_or_init(|| {
        let node = node_executable();
        let root = tempfile::tempdir().expect("a temporary directory");
        let base = root.path().canonicalize().expect("the directory resolves");
        let project = base.join("project");
        let agent_dir = base.join("agent");
        fs::create_dir_all(&project).expect("the project directory");
        fs::create_dir_all(agent_dir.join("agents")).expect("the agents directory");
        fs::write(
            agent_dir.join("agents").join("developer.md"),
            "---\nname: developer\ndescription: d\nmodel: provider/model-x\ntools: read, write\n---\n\nYou are the developer.\n",
        )
        .expect("the definition writes");
        fs::write(
            base.join("gate.mjs"),
            plugin::pi_source(std::path::Path::new(&node)),
        )
        .expect("the gate extension writes");
        let runner = plugin::pi_agents_source();
        let fake = base.join("pi");
        let pointed = runner.replace(
            "const PI_COMMAND = \"pi\";",
            &format!("const PI_COMMAND = {};", serde_json::to_string(&fake).unwrap()),
        );
        assert_ne!(runner, pointed, "the pi command constant is not where the test looks");
        fs::write(base.join("agents.mjs"), pointed).expect("the runner writes");
        fs::write(&fake, PI_AGENTS_FAKE_PI.replace("__NODE__", &node)).expect("the stand-in pi writes");
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("executable");
        fs::write(
            base.join("stub.cjs"),
            r#"const fs = require("node:fs");
const path = require("node:path");
if (path.basename(process.argv[1] ?? "") === "gate") {
  fs.appendFileSync(process.env.STUB_LOG, JSON.stringify(process.argv.slice(3)) + "\n");
  fs.writeSync(2, process.env.STUB_GATE_ERR ?? "");
  process.exit(Number(process.env.STUB_GATE_EXIT ?? 0));
}
"#,
        )
        .expect("the gate stand-in writes");
        fs::write(base.join("drive.mjs"), PI_AGENTS_DRIVER).expect("the driver writes");
        let ran = std::process::Command::new(&node)
            .arg(base.join("drive.mjs"))
            .current_dir(&base)
            .env("STUB_LOG", base.join("calls.log"))
            .env(
                "STUB_PATH",
                base.join("stub.cjs").to_string_lossy().replace('\\', "/"),
            )
            .env("FAKE_PID", base.join("pi.pid"))
            .env("FAKE_SEEN", base.join("seen.log"))
            .env("GATE", base.join("gate.mjs"))
            .env("AGENT_DIR", &agent_dir)
            .env("PROJECT", &project)
            .env_remove("NODE_OPTIONS")
            .output()
            .expect("node runs");
        assert!(
            ran.status.success(),
            "the driver failed: {}\n{}",
            String::from_utf8_lossy(&ran.stderr),
            String::from_utf8_lossy(&ran.stdout)
        );
        serde_json::from_slice(&ran.stdout).expect("the driver prints JSON")
    })
}

#[cfg(unix)]
fn card_text(value: &serde_json::Value) -> String {
    value
        .as_array()
        .expect("a card")
        .iter()
        .map(|line| line.as_str().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(unix)]
#[test]
fn the_pi_subagent_runner_links_the_child_to_the_parent_and_shows_what_the_events_say() {
    let said = run_pi_agents_driver();
    assert_eq!(said["exported"], "root-1");
    assert_eq!(said["command"], "estigia-agents");
    assert_eq!(said["settled"], "all done");

    let calls = said["gateCalls"].as_array().expect("gate calls");
    assert!(
        !calls.is_empty(),
        "the child never reached the gate: {said}"
    );
    let first: Vec<String> = serde_json::from_value(calls[0].clone()).expect("an argument list");
    assert!(
        first
            .windows(2)
            .any(|pair| pair == ["--parent-session-id", "root-1"]),
        "the child's gate call named no parent: {first:?}"
    );
    assert!(
        first
            .windows(2)
            .any(|pair| pair == ["--session-id", "child-7"]),
        "{first:?}"
    );

    let argv: Vec<String> = serde_json::from_value(said["childArgv"][0].clone()).expect("argv");
    assert_eq!(&argv[..2], ["--mode", "rpc"], "{argv:?}");
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--model", "provider/model-x"]),
        "{argv:?}"
    );
    assert!(
        argv.windows(2)
            .any(|pair| pair == ["--tools", "read, write"]),
        "{argv:?}"
    );
    assert!(
        argv.iter()
            .any(|word| word.contains("You are the developer.")),
        "{argv:?}"
    );
    assert!(
        !argv.iter().any(|word| word == "--no-extensions"),
        "{argv:?}"
    );

    let live = card_text(&said["live"]);
    for needed in [
        "alpha (developer)",
        "fake-model",
        "1.5k tok",
        "$0.0123",
        "1s",
    ] {
        assert!(
            live.contains(needed),
            "{needed} missing from the card:\n{live}"
        );
    }
    let done = card_text(&said["finalRow"]);
    assert!(done.contains("✓ alpha"), "{done}");
}

#[cfg(unix)]
#[test]
fn a_child_refused_by_the_gate_shows_the_refusal_on_its_card_unchanged() {
    let said = run_pi_agents_driver();
    let card = card_text(&said["refused"]);
    assert!(card.contains("estigia: refused: no live claim"), "{card}");
    assert!(card.contains("  run `claim` first"), "{card}");
    assert!(card.contains("second clause"), "{card}");
}

#[cfg(unix)]
#[test]
fn stopping_a_pi_subagent_terminates_its_process_and_answers_the_tool() {
    let said = run_pi_agents_driver();
    assert_eq!(said["aliveBefore"], true, "{said}");
    assert_eq!(said["gone"], true, "the child outlived the stop: {said}");
    assert_eq!(said["stoppedStatus"], "stopped");
    assert!(
        said["stopped"]
            .as_str()
            .unwrap_or_default()
            .contains("stopped")
    );
}

#[cfg(unix)]
#[test]
fn a_pi_subagent_is_not_given_work_unless_it_loaded_the_gate_and_is_linked_to_a_session() {
    let said = run_pi_agents_driver();
    assert!(
        said["ungated"]
            .as_str()
            .unwrap_or_default()
            .contains("did not load the gate"),
        "{said}"
    );
    let types: Vec<&str> = said["ungatedSeen"]
        .as_array()
        .expect("what the child saw")
        .iter()
        .filter_map(|entry| entry["type"].as_str())
        .collect();
    assert!(
        !types.contains(&"prompt"),
        "work reached a child without the gate: {types:?}"
    );
    assert!(
        said["unlinked"]
            .as_str()
            .unwrap_or_default()
            .contains("was not launched"),
        "{said}"
    );
    assert_eq!(
        said["unlinkedSeen"],
        serde_json::json!([]),
        "a child was spawned unlinked"
    );
    assert!(
        said["missing"]
            .as_str()
            .unwrap_or_default()
            .contains("no agent definition nobody")
    );
}

#[cfg(unix)]
#[test]
fn the_pi_subagent_view_lists_children_and_shows_each_transcript_with_no_model_call() {
    let said = run_pi_agents_driver();
    assert_eq!(said["shortcut"], "ctrl+alt+a");
    let start = card_text(&said["viewAtStart"]);
    for needed in [
        "> a",
        "omega (developer)",
        "task: look",
        "tool: write",
        "assistant: all done",
    ] {
        assert!(
            start.contains(needed),
            "{needed} missing from the view:\n{start}"
        );
    }
    assert!(
        start.contains("alpha (developer)"),
        "a finished child is not listed:\n{start}"
    );
    let finished = card_text(&said["viewOnFinished"]);
    assert!(finished.contains("task: wait"), "{finished}");
    assert!(!finished.contains("task: look"), "{finished}");
    assert_eq!(
        said["promptsWhileViewing"], 1,
        "the view sent a prompt to a child"
    );
}

#[cfg(unix)]
#[test]
fn a_running_pi_subagent_is_steered_from_the_view_and_a_finished_one_is_not() {
    let said = run_pi_agents_driver();
    assert_eq!(said["steersOnFinished"], 0, "{said}");
    assert_eq!(
        said["inputsOnFinished"], 0,
        "a finished child was offered a follow-up"
    );
    let steered = said["steered"].as_array().expect("what the child received");
    assert_eq!(steered.len(), 1, "{steered:?}");
    assert_eq!(steered[0]["message"], "please add tests");
    let view = card_text(&said["viewAfterSteer"]);
    assert!(view.contains("you: please add tests"), "{view}");
}

#[cfg(unix)]
#[test]
fn the_pi_subagent_view_stops_one_child_and_stops_all_only_after_a_confirm() {
    let said = run_pi_agents_driver();
    assert_eq!(said["zetaStopped"], "stopped", "{said}");
    assert_eq!(said["askedWhenDeclined"], 1, "{said}");
    assert!(
        said["etaWhenDeclined"]
            .as_str()
            .unwrap_or_default()
            .contains('●'),
        "a declined confirm still stopped the child: {said}"
    );
    assert_eq!(said["etaStopped"], "stopped", "{said}");
    assert_eq!(said["asked"], 2, "{said}");
    assert_eq!(said["closedAfterQ"], 1, "{said}");
}

#[cfg(unix)]
#[test]
fn finished_pi_subagent_tasks_are_kept_as_session_entries_and_never_sent_to_the_model() {
    let said = run_pi_agents_driver();
    let entries = said["appended"].as_array().expect("appended entries");
    let alpha = entries
        .iter()
        .find(|entry| entry["data"]["label"] == "alpha")
        .expect("the finished child was not kept");
    assert_eq!(alpha["type"], "estigia-agent");
    assert_eq!(alpha["data"]["status"], "done");
    assert_eq!(alpha["data"]["text"], "all done");
    assert_eq!(alpha["data"]["tokens"], 1500);
    assert!(
        entries
            .iter()
            .any(|entry| entry["data"]["label"] == "gamma" && entry["data"]["status"] == "stopped"),
        "a stopped child was not kept: {entries:?}"
    );

    let calls: Vec<&str> = said["piCalls"]
        .as_array()
        .expect("the calls on the host")
        .iter()
        .filter_map(|call| call.as_str())
        .collect();
    assert!(calls.contains(&"appendEntry"), "{calls:?}");
    for call in &calls {
        assert!(
            [
                "registerTool",
                "registerCommand",
                "registerShortcut",
                "on",
                "appendEntry"
            ]
            .contains(call),
            "the runner called {call}, which can reach the model: {calls:?}"
        );
    }

    let resumed = card_text(&said["resumed"]);
    assert!(resumed.contains("old-one (analyst)"), "{resumed}");
    assert!(resumed.contains("task: old task"), "{resumed}");
    assert!(resumed.contains("assistant: yesterday"), "{resumed}");
    assert!(!resumed.contains("decoy"), "{resumed}");
}

#[test]
fn setup_says_so_when_another_pi_subagent_runner_is_present_and_still_installs_its_own() {
    let (home, options) = sandbox();
    let adapter = agent("pi");
    let alone = setup(adapter, &Config::default(), &options).expect("setup succeeds");
    assert!(
        !alone
            .notes
            .iter()
            .any(|note| note.contains("another Pi subagent runner")),
        "{:?}",
        alone.notes
    );

    let directory = pi_goal_directory(home.path());
    fs::write(
        directory.join("subagent-other.ts"),
        "spawn(\"pi\", args);\n",
    )
    .expect("a runner");
    let beside = setup(adapter, &Config::default(), &options).expect("setup succeeds");
    let note = beside
        .notes
        .iter()
        .find(|note| note.contains("another Pi subagent runner"))
        .expect("setup did not say another runner is present");
    assert!(note.contains("subagent-other.ts"), "{note}");
    assert!(directory.join(plugin::PI_AGENTS_FILE).is_file());
}
