use super::*;
use crate::test_env::with_config_home;

fn sandbox() -> (tempfile::TempDir, SetupOptions) {
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

fn agent(slug: &str) -> &'static AgentAdapter {
    find_agent(slug).expect("a declared agent")
}

#[test]
fn only_the_two_verified_adapters_have_a_skill_root_of_their_own() {
    // The handoff used to name the adapters on the shared root one by one, and
    // said "four" while there were eight. Stated by its complement instead,
    // because that is the half that does not grow: an adapter gets its own root
    // only once somebody has checked a real installation, and this fails the
    // moment a ninth is promoted without that.
    let own: Vec<&str> = AGENTS
        .iter()
        .filter(|adapter| adapter.discovers_skills())
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(
        own,
        vec!["claude-code", "codex"],
        "an adapter was given a skill directory of its own; verify it against a real installation \
         and say so in the handoff"
    );
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
        ModelCatalogSource::Curated(&[
            "gpt-5.6-sol",
            "gpt-5.6-terra",
            "gpt-5.6-luna",
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.3-codex",
            "gpt-5.2-codex",
        ])
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
            "implementer=opus, reviewer=opus, judge=opus, explore=opus, propose=opus, spec=opus, design=opus, tasks=opus, apply=opus, orchestrate=opus",
        ),
        (
            &claude[2],
            "implementer=sonnet, reviewer=haiku, judge=sonnet, explore=sonnet, propose=sonnet, spec=haiku, design=sonnet, tasks=haiku, apply=sonnet, orchestrate=sonnet",
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
        "implementer=sonnet, reviewer=sonnet, judge=opus, explore=sonnet, propose=opus, spec=sonnet, design=opus, tasks=sonnet, apply=sonnet, orchestrate=opus"
    );
    for (profile, expected) in [
        (
            &codex[1],
            "implementer=gpt-5.6-sol, reviewer=gpt-5.6-sol, judge=gpt-5.6-sol, explore=gpt-5.6-sol, propose=gpt-5.6-sol, spec=gpt-5.6-sol, design=gpt-5.6-sol, tasks=gpt-5.6-sol, apply=gpt-5.6-sol, orchestrate=gpt-5.6-sol",
        ),
        (
            &codex[2],
            "implementer=gpt-5.6-terra, reviewer=gpt-5.6-luna, judge=gpt-5.6-terra, explore=gpt-5.6-terra, propose=gpt-5.6-terra, spec=gpt-5.6-luna, design=gpt-5.6-terra, tasks=gpt-5.6-luna, apply=gpt-5.6-terra, orchestrate=gpt-5.6-terra",
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
            for model in profile
                .routing()
                .expect("a valid built-in profile")
                .by_role
                .values()
            {
                assert!(
                    catalog.contains(&model.as_str()),
                    "{} profile {} uses uncatalogued model {model}",
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
        "implementer=gpt-5.6-terra, reviewer=gpt-5.6-terra, judge=gpt-5.6-sol, explore=gpt-5.6-terra, propose=gpt-5.6-sol, spec=gpt-5.6-terra, design=gpt-5.6-sol, tasks=gpt-5.6-terra, apply=gpt-5.6-terra, orchestrate=gpt-5.6-sol"
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
    // other, since the directive is fenced once.
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
    // The ratchet applied to prose. A directive that names a directory setup
    // did not write to is a dead end the agent follows at runtime.
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
    // Every file of the skill, the directive, the hooks, and the MCP entry.
    // The shipped files, the install record, and the three this adapter
    // wires up: the instruction file, the hooks and the MCP entry.
    assert_eq!(result.changed_files(), skill::FILES.len() + 4);
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
fn a_dry_run_reports_exactly_what_the_real_run_does() {
    // Invariant four. A plan that disagrees with the act is worse than no plan.
    let (_home, options) = sandbox();
    let adapter = agent("codex");
    let config = Config::default();

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

    let strip = |result: SetupResult| {
        result
            .actions
            .into_iter()
            .map(|action| (action.kind, action.path, action.change))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(planned), strip(performed));
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
/// theirs after every adapter installs and leaves.
///
/// `every_file_an_adapter_touches_comes_back_exactly_as_it_was` seeds the four
/// files an adapter *wires* — hooks, plugin, MCP registration, instructions. It
/// never puts anything in the skill directory, which is the one the operator
/// named first: *if I have another skill, my own config or my own `.md`, they
/// stay mine*.
///
/// `uninstalling_over_somebody_else_s_checkout_leaves_the_checkout` does work
/// in that directory, but only over files that are **in Estigia's manifest**
/// and happened to be there first. A file the manifest never heard of was
/// covered by nothing.
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
            // Their own note, in the root Estigia writes into.
            (root.join("MY-NOTES.md"), "# my notes\n"),
            // Their own rows. `installed_config_for` calls this "the one file
            // Estigia never edits", and never editing it has to include not
            // deleting it.
            (
                root.join(crate::config::LOCAL_FILE),
                "| Merge strategy | rebase |\n",
            ),
            // Inside a directory Estigia does create, so the emptied-directory
            // sweep runs right past it. `remove_dir` rather than
            // `remove_dir_all` is what makes this survive, and nothing said so.
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
        // Estigia's own, for contrast: `config set --agent` writes this one, and
        // `installed_config_for` names the difference — the per-agent file is
        // "a file Estigia writes", `estigia.local.md` is the operator's. So
        // this one goes and the one above stays. Asserted rather than left to
        // be rediscovered, because the two files sit side by side and differ
        // only in what wrote them.
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

    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup runs");
    }
    for adapter in AGENTS {
        uninstall(adapter, &options).expect("uninstall runs");
    }

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
    // escapes and every hook fails with "command not found", no space anywhere
    // in sight.
    //
    // **Which** quote is a measured difference and not a preference, and it is
    // not the same here as in the git hook — see `render::hook_command`. Git
    // bundles its own POSIX shell, so `guard::script` is always read by `sh`,
    // where single quotes are strictly safer. An agent's command is read by
    // whatever shell that agent uses, and `cmd //c` answers *el nombre de
    // archivo … no son correctos* to a single-quoted one.
    //
    // Found by walking what setup actually wrote, not by listing the callers —
    // a fourth place would be covered the day somebody adds it.
    let (home, options) = sandbox();
    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup runs");
    }

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
                            // A hook entry is a **command line**. An MCP server
                            // entry is a program and its `args`, handed to the
                            // operating system with no shell in between —
                            // quoting that one would break it.
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
    // Round 73 left this open and named it: the git hook can be single-quoted
    // because git bundles its own `sh`, and an agent's hook command cannot,
    // because `cmd //c "'C:\…\estigia.exe' hook …"` answers *el nombre de
    // archivo … no son correctos*. There is no quoting that covers both.
    //
    // So the ambiguity is removed instead of resolved: a path that survives
    // double quoting in every shell needs none of these characters, and one
    // that does not is refused before a hook is built around it. A gate
    // registered, reported installed, and pointing somewhere else is the one
    // state this crate exists to refuse.
    //
    // Every case measured by handing `sh` the string a hook actually carries,
    // not reasoned about.
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

    // And the paths people actually have are not refused. An apostrophe is the
    // one that had to be measured rather than assumed: `sh` hands
    // `"C:\Users\O'Brien\estigia.exe"` back unchanged, so refusing it would
    // have locked out an ordinary Windows profile for nothing.
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
    // The operator's requirement, in their words: *uninstall should remove the
    // app and everything related to it, and only that*. Measured, it was
    // inverted — after `uninstall --all` the **only** thing left under the home
    // was `~/.estigia`: the ledger, the run pointers, and any stand-down.
    //
    // Not inert, either. Run pointers are what the push guard reads to decide
    // whether a claim covers a checkout, the ledger is what `doctor` reads to
    // decide whether a run may swear at all, and a stand-down is bounded by a
    // clock rather than by an installation — so uninstalling and reinstalling
    // inside its window brings the gate back already standing down.
    let (home, options) = sandbox();
    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup runs");
    }
    let runs =
        crate::harness::session::state_root(options.home_dir.as_deref()).expect("a state root");
    fs::create_dir_all(&runs).expect("the state directory");
    let mut mine = crate::harness::session::Run::new("claude-abcd1234".to_owned());
    mine.issue = Some(12);
    crate::harness::session::store(&runs, &mine).expect("a run pointer");
    crate::harness::session::record(&runs, &serde_json::json!({"verdict": "allow"}));
    fs::write(crate::harness::standdown::path(&runs), "{}").expect("a stand-down");
    let state: Vec<std::path::PathBuf> = vec![
        crate::harness::session::pointer_path(&runs, "claude-abcd1234"),
        crate::harness::session::ledger_path(&runs),
        crate::harness::standdown::path(&runs),
    ];
    for path in &state {
        assert!(path.is_file(), "{} was not written", path.display());
    }

    // One agent out and others still here: the state is machine-wide, and
    // `uninstall codex` on a machine that still runs Claude Code must not take
    // the other one's claims with it.
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
    for adapter in AGENTS {
        uninstall(adapter, &dry).expect("the plan is produced");
    }
    for adapter in AGENTS {
        uninstall(adapter, &options).expect("uninstall runs");
    }
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
    // And nothing of anybody else's went with it.
    assert_eq!(
        files_under(home.path()),
        0,
        "something was left behind: {:?}",
        listing(home.path())
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
    // A contract newer than the one this binary embeds. The version to replace
    // is read from the payload rather than spelled: written out, it was
    // `1.14.0`, and a bump to `1.15.0` turned this line into a no-op — so the
    // "newer" contract was never written, and the assertion below failed while
    // the behaviour it guards was untouched. A test that has to be edited every
    // time the skill ships is a test that will one day be edited into agreement.
    let embedded = skill::version().expect("the payload declares a version");
    let on_disk = fs::read_to_string(&contract).expect("the contract is installed");
    let newer = on_disk.replace(&format!("version: \"{embedded}\""), "version: \"9.9.9\"");
    assert_ne!(
        newer, on_disk,
        "the contract on disk does not carry the embedded version, so nothing was made newer"
    );
    fs::write(&contract, &newer).expect("write the newer contract");

    let changed = Config {
        merge: crate::config::MergeStrategy::Squash,
        ..Config::default()
    };
    rewrite_configuration(&contract, &changed).expect("the block is rewritten");

    let after = fs::read_to_string(&contract).expect("the contract survives");
    assert!(
        after.contains("9.9.9"),
        "the newer contract was silently downgraded"
    );
    assert!(after.contains("| Merge strategy | squash |"));
}

#[test]
fn the_directive_says_what_the_contract_says_about_unknown_results() {
    // Seam test: the always-loaded directive against the contract it summarises.
    // A directive that softens the contract is worse than none, because it is
    // the copy that is always in context.
    assert!(DIRECTIVE_TEMPLATE.contains("not clearance"));
    assert!(
        skill::CONTRACT_CONTENTS.contains("never reinterpret an unknown result as clearance"),
        "the contract no longer says what the directive summarises"
    );
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
fn the_codex_toml_entry_is_edited_by_the_line_and_keeps_their_comments() {
    // A round trip through a TOML parser would reorder their keys and drop
    // their comments, which breaks invariant three as surely as rewriting the
    // file wholesale would.
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
    // An operator looking at a run that wrote without a claim needs to know
    // whether the gate is on. Folding that into `is_configured` hides it.
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
    // `--skill-only` is a documented flag, and what it leaves behind is a skill
    // installed, a gate registered and an MCP server exposed, with the
    // instruction file untouched. `is_configured` says no — right for its own
    // question, "was the agent told", and wrong for "is Estigia here". `doctor`
    // filtered on it and announced "no agent is configured, so nothing reads a
    // contract" on a machine where the contract was installed and being read,
    // resolving to `setup --all`: the one command that writes the instruction
    // file this flag exists to leave alone. Every per-contract check went
    // unrun there too, which is the part that costs.
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
fn the_only_thing_without_a_gate_is_the_thing_that_is_not_an_agent() {
    // Every agent Estigia knows can now be gated. What is left is the
    // agent-neutral root — a convention for where skills live, with no tool
    // loop to gate — and it is reported as such rather than as a gap.
    let (_home, options) = sandbox();
    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
        if adapter.can_gate_tools() {
            assert!(
                is_gated(adapter, &options),
                "{} reports no gate",
                adapter.slug
            );
        } else {
            assert_eq!(
                adapter.slug, "agents",
                "{} is an agent and has no gate",
                adapter.slug
            );
            assert!(
                adapter
                    .gate_gap()
                    .is_some_and(|gap| gap.contains("not an agent")),
                "the neutral root does not say what it is"
            );
        }
    }
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
    uninstall(adapter, &options).expect("uninstall succeeds");
    assert_eq!(
        fs::read_to_string(&paths.instructions).expect("their file survives"),
        theirs
    );
}

#[test]
fn a_file_estigia_created_that_somebody_then_wrote_in_is_kept() {
    // Ownership by name is not a licence to delete. If somebody wrote in it,
    // the directive comes out and their text stays.
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
    // The second gate mechanism. Claude Code takes a line in a settings file;
    // OpenCode takes a module whose `tool.execute.before` blocks by throwing.
    // Two shapes, one question — `is_gated` answers it for both.
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

#[test]
fn only_the_agents_that_can_deny_claim_they_can() {
    // The seam under `status`: an adapter that says it gates and has no
    // mechanism would report a gate nobody installed.
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
    // Three agents can deny a tool call and none of them spells it the same
    // way. Getting one wrong produces a hook that runs, decides correctly, and
    // is ignored — which reports success and enforces nothing.
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
    // The seam: a gate written into the wrong event never fires, and one
    // answered in the wrong dialect fires and is ignored.
    let (_home, options) = sandbox();
    for adapter in AGENTS.iter().filter(|a| a.supports_hooks()) {
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
    }
}

#[test]
fn a_gated_agent_s_hooks_file_keeps_everybody_else_s_entries() {
    let (_home, options) = sandbox();
    for slug in ["claude-code", "cursor", "gemini-cli"] {
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
    // The honesty contract, said where somebody reads it. "gate off" alone
    // leaves an operator wondering whether they declined it or Estigia cannot
    // give it, and those call for different actions. "not supported" is a dead
    // end, and this project does not ship those.
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
    // The two answers come from different places — a spec table and a prose
    // line — and a reader trusts them together. One saying "no gate" while the
    // other says nothing about why is how a report stops being read.
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
fn each_gated_agent_gets_its_own_envelope_and_not_somebody_else_s() {
    // The failure this test exists for: a Codex entry written into Claude
    // Code's envelope parses, looks right, and never fires. Three shapes, and
    // the difference is invisible in a diff unless somebody checks it.
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
    //
    // This asserted the opposite for a long time — the events at the top level,
    // no wrapper — and the assertion was as wrong as the code, because both came
    // from one claim that was never crossed against Codex itself. What Codex
    // actually says, on startup, is:
    //
    //   failed to parse hooks config ~/.codex/hooks.json:
    //   unknown field `PreToolUse`, expected `description` or `hooks`
    //
    // So it did not merely fail to fire. It stopped the agent from starting.
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

    // Claude Code and Gemini: wrapped, with the entry's own nested `hooks`.
    for (slug, event) in [("claude-code", "PreToolUse"), ("gemini-cli", "BeforeTool")] {
        let settings = read(slug);
        assert!(
            settings["hooks"][event].is_array(),
            "{slug} lost its wrapper: {settings}"
        );
        assert_eq!(settings["hooks"][event][0]["hooks"][0]["type"], "command");
    }

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
    for slug in ["codex", "gemini-cli", "cursor"] {
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

/// The lifecycle is a fact about the agent, and nothing infers it.
///
/// It was inferred: "settings envelope **and** Claude Code dialect", which held
/// only for as long as Codex used a different envelope. The day Codex's
/// envelope was corrected to the one it actually reads, that coincidence ended
/// and Codex silently started receiving `SessionStart` and the rest — two true
/// facts multiplied into a third that was never true.
///
/// So the property is declared per adapter, and this is what holds it declared:
/// exactly one adapter carries it, and the two things it used to be deduced
/// from are no longer enough to tell the adapters apart.
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

    // And the pair it was deduced from no longer picks that agent out, which is
    // the whole reason the deduction had to go.
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

/// Codex refuses the file Estigia used to write it, and says so on startup.
///
/// Not "the gate never fires", which is the usual cost of the wrong envelope.
/// This one stopped the agent from starting:
///
/// ```text
/// failed to parse hooks config ~/.codex/hooks.json:
///   unknown field `PreToolUse`, expected `description` or `hooks`
/// ```
///
/// Held as a shape rather than as a claim in a comment, because a claim in a
/// comment is exactly what put it there.
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

/// What Estigia writes into an adapter's own file is what that file can hold.
///
/// It was not. `write_agent_configuration` rendered the **whole** table, and
/// `doctor` then read the result back and called it broken — in as many words:
///
/// ```text
/// BROKEN contract qwen: 11 rows in its own file that nothing reads.
///   Delivery route, Merge strategy, Worktree location, Tracker, … — the
///   repository answers for them, and the contract's value is what every
///   command reports
/// ```
///
/// About a file Estigia had just written, on nine of the eleven adapters. The
/// scope split is built carefully in `Scope`, carried through the screen, the
/// contract and the gate, and was thrown away in the last rendering step —
/// which is the defect family this repository keeps meeting.
///
/// Held from **both** ends, because either alone can drift into agreement with
/// a wrong answer: the rows are checked against `Scope`, and the file is handed
/// to the very check that condemned it.
#[test]
fn an_adapters_own_file_carries_only_what_an_adapter_can_answer() {
    use crate::config::{Scope, Setting};

    let (home, options) = sandbox();
    // An adapter that shares the neutral root, so it has a file of its own.
    let adapter = agent("qwen");
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
    // offering half of what it is for.
    assert_eq!(
        rows.len(),
        crate::config::AGENT_SETTINGS.len(),
        "the adapter's own file does not carry every per-agent row: {written}"
    );

    // And the check that condemned this file agrees now. Crossed rather than
    // reasoned about: the two halves are in different modules, and one of them
    // was already right while the other wrote the file it complains about.
    let condemned: Vec<String> =
        crate::harness::doctor::examine(Some(&root), home.path(), &Config::default().tracker)
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

/// The document that says what this crate does not measure.
///
/// One reader, because two branches of the test below ask the same question of
/// the same paragraphs, and the second was about to be a copy of the first.
///
/// It used to carve the section out of the README by heading, which was a second
/// copy of the same carving in `tests/honesty.rs` — and when the contract moved
/// to its own file, that copy is what stayed behind pointing at a heading no
/// longer followed by anything. Reading the whole file removes the carving
/// rather than duplicating the fix.
///
/// The length check guards the failure this reader can otherwise cause silently:
/// callers all ask *does the contract contain this phrase?*, so an empty read
/// answers no to every one of them, and the assertions that depend on it fail
/// for a reason nobody would look for here.
fn honesty_contract() -> String {
    let text = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/honesty.md"),
    )
    .expect("docs/honesty.md ships with the crate");
    assert!(
        text.len() > 10_000,
        "docs/honesty.md is {} bytes, too short to be the contract these tests cross against",
        text.len()
    );
    text
}

/// The one entry in it about gates nothing narrows but the classifier's list.
///
/// Its **first paragraph**, which is the part that states the claim. Taking the
/// whole entry let a trailing note about how the entry came to be written
/// satisfy the check on its own: mutating the sentence so it named the wrong
/// agent left the test green, because the word was still further down.
fn fires_on_every_tool() -> String {
    let section = honesty_contract();
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
    // The seam that four gates fell through. Estigia registered a hook for
    // Codex, Gemini CLI, Qwen and Cursor, every one of them fired, reached the
    // classifier, found a tool name it did not know, and stood aside —
    // registered, running, and deciding nothing.
    //
    // A matcher names the tools an agent will wake the hook for. Every one of
    // them has to be a name `harness::classify` recognises, or the gate is
    // theatre.
    // Every agent whose tool calls Estigia gates **by any mechanism**, not only
    // the ones registered through a settings file. This filtered on
    // `supports_hooks` — which is *has a gate spec* — so the two agents gated
    // through a file Estigia owns whole were never reached, and those are
    // exactly the two where nothing narrows the gate but the classifier's own
    // list. OpenCode's plugin names the tools it wakes for and
    // `the_plugin_gates_the_tools_the_classifier_judges` crosses that list.
    // Cline's script pipes **every** payload through with no filter, no matcher
    // and no named list — so it belongs in the honesty contract beside Cursor
    // and Windsurf, and the sentence there counted two.
    let mut reached = 0;
    for adapter in AGENTS.iter().filter(|a| a.can_gate_tools()) {
        reached += 1;
        let Some(spec) = adapter.gate_spec() else {
            // Derived, not listed: a plugin that carries a `GATED` array names
            // its own tools and is crossed elsewhere. One that does not fires on
            // everything, and the next agent gated this way inherits the rule
            // rather than a comment about Cline.
            let written = match adapter.instructions {
                InstructionFile::Cline => {
                    plugin::cline_hook(std::path::Path::new("/opt/estigia"), false)
                }
                _ => plugin::source(std::path::Path::new("/opt/estigia")),
            };
            if !written.contains("const GATED = [") {
                // The entry that says so, not the section. Asked of the whole
                // section this passed on the first try — Cline is named a few
                // lines above, about which shape it reads a refusal in, which
                // says nothing about what its gate watches. An assertion a
                // wrong document satisfies is worse than none.
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
            // No matcher means every tool, with the classifier's own list as the
            // only thing narrowing it. That is not "nothing to cross here" — it
            // is the case where the list does all the work and nothing checks it
            // holds this host's names, which is the seam this whole test exists
            // for, one adapter along.
            //
            // What can be crossed without a payload captured from a real session
            // is that the gap is written where an operator reads it rather than
            // in a comment here.
            if !spec.event_is_the_tool_name {
                assert!(
                    honesty_contract().contains("fire on every tool"),
                    "{} registers a gate with no matcher, so only the classifier's list narrows \
                     it \u{2014} and the honesty contract does not say so",
                    adapter.slug
                );
            }
            // Unless the event name is what arrives as the tool name, in which
            // case the events are exactly what has to be recognised, and
            // skipping them is how a gate gets registered and stays silent.
            if spec.event_is_the_tool_name {
                for event in spec.events {
                    let (action, _) = crate::harness::classify(
                        event,
                        &serde_json::json!({"command": "git commit -m x", "file_path": "a.rs"}),
                    );
                    assert!(
                        !matches!(action, crate::harness::Action::Untouched),
                        "{} registers `{event}` and the classifier does not know it: the gate \
                         fires and decides nothing",
                        adapter.slug
                    );
                }
            }
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
            assert_ne!(
                action,
                crate::harness::Action::Untouched,
                "{} wakes the hook for `{name}` and the classifier does not know it — the gate \
                 runs and decides nothing",
                adapter.slug
            );
        }
    }
    // How many it walked, because narrowing the walk is how the gap this test
    // was widened to close got in. The filter was `supports_hooks`, which reads
    // as *every gated agent* and means *every agent with a settings spec* — and
    // putting it back leaves every assertion above passing, on two fewer
    // agents. Only a count notices that.
    assert_eq!(
        reached,
        AGENTS.iter().filter(|a| a.can_gate_tools()).count(),
        "the walk skipped a gated agent"
    );
    assert!(
        reached >= 10,
        "only {reached} gated agent(s) were reached — this stopped covering the fleet"
    );
}

#[test]
fn the_opencode_plugin_only_names_tools_the_gate_can_classify() {
    // Same seam, through the other mechanism: the plugin filters by name before
    // it spends a process, and a name the classifier does not know is a process
    // spent to answer "not mine".
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
///
/// The sixth and last place in this crate where a hand-written description sits
/// opposite the code it describes. It is also the worst one to get wrong: the
/// directive is the text an agent has in context on **every** turn, so a rule
/// the gate no longer keeps is a promise made thousands of times a day by the
/// one document nobody re-reads.
///
/// Named rather than paraphrased. A test whose name changes takes its rule with
/// it, and a rule with no test is one nobody is holding.
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
        include_str!("../harness/tests.rs"),
        include_str!("../harness/tracker.rs"),
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

#[test]
fn a_moved_home_moves_every_root_under_it() {
    // The isolation invariant. `APPDATA` is always set on Windows, so before
    // this the Gemini instruction file resolved to the real machine while the
    // skill resolved to the sandbox — one command reading two disks and saying
    // nothing about it.
    //
    // Every root has to sit under the home it was given, including the two that
    // used to be read from the process environment.
    let home = tempfile::tempdir().expect("a temporary directory");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    for adapter in AGENTS {
        for platform in [Platform::Windows, Platform::MacOs, Platform::Unix] {
            let options = SetupOptions {
                platform: Some(platform),
                ..options.clone()
            };
            let paths = resolve_paths(adapter, &options).expect("the paths resolve");
            for (what, path) in [
                ("skill root", Some(paths.skill_root.clone())),
                ("instructions", Some(paths.instructions.clone())),
                ("hooks", paths.hooks.clone()),
                ("mcp config", paths.mcp_config.clone()),
                ("plugin", paths.plugin.clone()),
            ] {
                let Some(path) = path else { continue };
                assert!(
                    path.starts_with(home.path()),
                    "{} on {platform:?}: {what} resolved to {}, outside the home it was given",
                    adapter.slug,
                    path.display()
                );
            }
        }
    }
}

#[test]
fn uninstall_leaves_no_file_estigia_created() {
    // The exact inverse, checked by counting files rather than by trusting the
    // plan. Cursor's hooks file survived as `{"version": 1, "hooks": {}}` — the
    // hooks were gone, and the scaffolding Estigia wrote around them read as
    // content worth keeping.
    let home = tempfile::tempdir().expect("a temporary directory");
    let options = SetupOptions {
        home_dir: Some(home.path().to_path_buf()),
        ..SetupOptions::default()
    };
    let config = Config::default();

    let before = files_under(home.path());
    for adapter in AGENTS {
        setup(adapter, &config, &options).expect("setup runs");
    }
    // The screen's remembered language, written the way the screen writes it.
    // It was the one file under `~/.estigia` the uninstall did not name, and
    // this corpus never opened the screen — so the test whose whole claim is
    // *no file Estigia created is left behind* went on passing while
    // `.estigia/screen`, and the directory holding it, stayed on every machine
    // where somebody had ever changed the language.
    crate::tui::words::remember(Some(home.path()), crate::tui::words::Tongue::Spanish)
        .expect("the screen remembers");
    assert!(
        files_under(home.path()) > before,
        "setup wrote nothing to undo"
    );
    for adapter in AGENTS {
        uninstall(adapter, &options).expect("uninstall runs");
    }
    // The machine-wide state, taken out the way an operator's uninstall takes
    // it: `forget_state` is called from the command, not from `uninstall`, so
    // walking the adapters alone never reaches the state directory. This test's
    // claim is *no file Estigia created is left behind* and its reach stopped
    // at the adapters' own files — it would not have noticed a run pointer, a
    // ledger, a stand-down record or the screen's language, and it noticed none
    // of them because the fixture created none either.
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
fn a_plan_reports_what_the_run_does_even_when_two_steps_share_a_file() {
    // The fourth setup invariant, at the one place it broke. Qwen keeps its gate
    // and its MCP server in a single `settings.json`, so the second step used to
    // read the file the first step had already changed — on disk in a real run,
    // and not at all under `--dry-run`. The plan said `create` twice where the
    // run does `create` and then `update`.
    //
    // Checked for every agent, both directions, because the invariant is not
    // Qwen's: it is setup's.
    let config = Config::default();
    for adapter in AGENTS {
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
    }
}

/// A result as a person reads it: which file, and what happened to it.
#[test]
fn a_plan_over_every_agent_reports_what_that_run_does() {
    // The sibling of the test above, at the seam it does not reach. That one
    // runs one adapter on a fresh home, so nothing an earlier adapter did is in
    // play. `--all` is where it is: eight of the eleven share the neutral skill
    // root, so every adapter after the first meets a directory the run itself
    // filled — on disk in a real run, and only in `pending` under a plan.
    //
    // What this does **not** prove: that the record and the contract fence are
    // read through `pending` rather than off the disk. They are, and they
    // should be, but the classification cannot differ on it — one config
    // renders one contract, so every adapter after the first finds the content
    // already right and answers `Unchanged` before either question is asked.
    // Written down because the first version of this comment claimed the guard
    // covered that; reverting the read was tried, and this test passes either
    // way.
    let config = Config::default();
    for foreign in [false, true] {
        let home = tempfile::tempdir().expect("a temporary directory");
        let real = SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            ..SetupOptions::default()
        };
        let planned = SetupOptions {
            dry_run: true,
            ..real.clone()
        };
        // Once on an empty machine, and once over somebody else's skill — the
        // arrangement that makes the two extra questions answer differently.
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
                    &setup_into(adapter, &config, &planned, &mut planning)
                        .expect("the plan is made"),
                )
            })
            .collect();

        let mut running = Pending::new();
        let run: Vec<Vec<String>> = AGENTS
            .iter()
            .map(|adapter| {
                shape(&setup_into(adapter, &config, &real, &mut running).expect("the run happens"))
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
    }
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
    // Two paths write the managed block: `setup` and `config set`. They built
    // it separately, so setting any value replaced setup's block with a shorter
    // one — the paragraph naming `estigia.local.md` vanished, and with it the
    // only place the agent is told the override file exists.
    //
    // The eighth instance of the same shape in this crate: one end written by
    // hand, the other written by hand somewhere else, and nothing crossing them.
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
    // The whole point. Eight of the ten adapters write to the same neutral
    // directory, so a single table there gave every one of them the same
    // routing — and "claude-code on Opus while opencode runs Kimi" was
    // unsayable however the setting was spelled.
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
        &routed("design=opus-5, apply=sonnet-5, orchestrate=fable-5"),
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

    assert_eq!(claude.models.for_phase("apply"), Some("sonnet-5"));
    assert_eq!(opencode.models.for_phase("apply"), Some("kimi-k3"));
    assert_eq!(claude.models.for_phase("design"), Some("opus-5"));
    // Neither one's file leaks into the other's answer, which is the failure
    // this exists to prevent.
    assert_eq!(opencode.models.for_phase("design"), None);
    assert_eq!(claude.models.for_phase("orchestrate"), Some("fable-5"));
    assert_eq!(opencode.models.for_phase("orchestrate"), Some("gpt-5.6"));

    // An adapter with no file of its own still reads the shared table rather
    // than nothing — sharing a directory is the default, not a broken state.
    let third = crate::skill::installed_config_for(root.path(), Some("gemini-cli"))
        .expect("the shared table");
    assert_eq!(third.models, shared.models);

    // And a second write moves only the marked block, so anything an operator
    // added around it survives.
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
            .for_phase("apply"),
        Some("deepseek-v4")
    );
}

#[test]
fn only_the_rows_the_gate_decides_carry_a_caveat_and_the_list_is_pinned() {
    use crate::config::{SETTINGS, Setting};

    // The default for a row nobody has thought about is `Held` — "Estigia
    // enforces this for you" — and for an agent Estigia cannot gate, that is an
    // overclaim waiting for the next setting somebody adds. So the exceptions
    // are written out here rather than derived: adding a gate-enforced setting
    // now fails this test until somebody decides which of the three it is.
    //
    // Empty since `Renewal window` left. It was the one row that decided
    // nothing for an ungated agent, and it turned out to be enforced by the
    // gate rather than obeyed by the agent — so it belongs to the repository,
    // and a repository row must not be sealed shut by whichever agent the
    // cursor happens to be on.
    let inert: [Setting; 0] = [];
    let asked = [
        Setting::Delivery,
        Setting::Review,
        Setting::Transitions,
        Setting::Boundaries,
    ];

    for adapter in AGENTS {
        for setting in SETTINGS {
            let applies = adapter.applies(*setting);
            // Review authority permits a handoff; it cannot make a runtime
            // provide the distinct context that performs it. That caveat is
            // true even where Estigia gates the runtime's other tool calls.
            if adapter.can_gate_tools() && *setting != Setting::Review {
                assert_eq!(
                    applies,
                    Applies::Held,
                    "{} qualifies {setting:?} despite gating its tools",
                    adapter.slug
                );
                continue;
            }
            let expected = if inert.contains(setting) {
                "inert"
            } else if asked.contains(setting) || *setting == Setting::Review {
                "asked"
            } else {
                "held"
            };
            let actual = match applies {
                Applies::Held => "held",
                Applies::Asked(_) => "asked",
                Applies::Inert(_) => "inert",
            };
            assert_eq!(
                actual, expected,
                "{setting:?} is {actual} for {} and the pinned list says {expected}",
                adapter.slug
            );
            // Every caveat says why. One that only said "not applicable" would
            // send somebody to read this source to find out what they lost.
            assert_eq!(
                applies.because().is_some(),
                expected != "held",
                "{setting:?} is qualified for {} without saying why",
                adapter.slug
            );
            // And only the inert one is closed to editing: `asked` is a real
            // answer, written into a real contract, and refusing it would take
            // away the only way to tell a contract-only agent what is expected.
            assert_eq!(applies.editable(), expected != "inert");
        }
    }

    // And the refusal itself, over the **variants**, because no adapter
    // produces `Inert` today: measured by mutation, `editable` could answer
    // `true` for everything with this whole matrix green. The same hole the
    // screen's translation guard names in its own words — *the day something
    // returns it, the screen would have shown one English word and no guard
    // would have moved*.
    assert!(
        !Applies::Inert("a caption this test does not read").editable(),
        "a row that applies to nothing here is offered for editing"
    );
    for open in [
        Applies::Held,
        Applies::Asked("a caption this test does not read"),
    ] {
        assert!(
            open.editable(),
            "{open:?} is a real answer and the screen refuses it"
        );
    }
}

#[test]
fn a_plan_over_every_adapter_reports_what_the_whole_run_does() {
    // The promise is setup's fourth, and it was held at the wrong scope: the
    // memory of what a run had already written lived inside **one adapter's**
    // call, which covers the case it was written for — Qwen keeping its gate
    // and its MCP server in one file — and not the larger one.
    //
    // Eight of the eleven adapters share a skill root. Under `--dry-run`
    // nothing is written, so each of the eight read the untouched disk and
    // planned to create the same fifteen files again. Measured against the real
    // binary before this was written: the plan said 182 files, the run did 70.
    //
    // `--dry-run` is the one command whose entire job is to be believed before
    // anything happens.
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
    // each of the eight that share a root, three taken out.
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
    // Eight of the eleven adapters have no skill directory of their own and
    // install into the neutral root. Uninstalling one took the skill with it:
    // running `estigia setup --all` and then `estigia setup opencode
    // --uninstall` left `status` reporting `configured, skill missing` for
    // eight agents that had been asked for nothing.
    let (_home, options) = sandbox();

    let neutral: Vec<&AgentAdapter> = AGENTS
        .iter()
        .filter(|adapter| adapter.skills == SkillsRoot::Neutral)
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

    // And the last one out does take it, or uninstalling everything would
    // leave the skill behind for nobody.
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
    // `estigia setup agents` on a clean machine made seven other agents read
    // as half installed, because each of them saw a skill in its root and no
    // directive of its own. The skill was not theirs.
    let (_home, options) = sandbox();
    let neutral: Vec<&AgentAdapter> = AGENTS
        .iter()
        .filter(|adapter| adapter.skills == SkillsRoot::Neutral)
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

    // An agent with a root of its own answers the other way, so the genuine
    // half-installed state is still reachable.
    let alone = AGENTS
        .iter()
        .find(|adapter| adapter.skills != SkillsRoot::Neutral)
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
    // `config set --agent` writes a file that is not one of `FILES`, because a
    // different command writes it. So it outlived `setup --all --uninstall`
    // and sat in the skill directory of a machine Estigia had been taken off,
    // holding answers for an install that no longer existed.
    let (_home, options) = sandbox();
    let shared = AGENTS
        .iter()
        .find(|adapter| adapter.skills == SkillsRoot::Neutral)
        .expect("an adapter on the shared root");

    setup_into(shared, &Config::default(), &options, &mut Pending::new()).expect("install");
    let root = resolve_paths(shared, &options).expect("paths").skill_root;
    let own = skill::agent_override(&root, shared.slug);
    let theirs = Config {
        merge: crate::config::MergeStrategy::Rebase,
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
    // Estigia layers three files: the contract, the adapter\'s own answers
    // beside it, and the operator\'s local override. It reads all three
    // (`skill::installed_config_for`), and the agent only ever does what the
    // text it reads tells it to. A layer named nowhere is a layer that exists
    // in the code and in no behaviour — `config set --agent opencode` would
    // report a row as set for that agent while the agent went on reading the
    // shared one.
    //
    // The contract needs no naming: it is the file. The other two do.
    let body = skill::configuration_body(&Config::default());
    assert!(
        body.contains(crate::config::LOCAL_FILE),
        "the configuration block never names the operator\'s override file"
    );

    let (_home, options) = sandbox();
    let shared = AGENTS
        .iter()
        .find(|adapter| adapter.skills == SkillsRoot::Neutral)
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
    // a file that will never be written: `config set --agent` writes its
    // contract directly.
    let alone = AGENTS
        .iter()
        .find(|adapter| adapter.skills != SkillsRoot::Neutral)
        .expect("some adapter keeps its own skill directory");
    let paths = resolve_paths(alone, &options).expect("paths");
    let directive = directive_for(alone, &paths);
    assert!(
        !directive.contains(&format!("estigia.{}.md", alone.slug)),
        "{} is sent to a file nothing will ever write:\n{directive}",
        alone.slug
    );
}

/// The plan matches the act for every adapter, in every state a run meets.
///
/// `a_dry_run_reports_exactly_what_the_real_run_does` states the invariant and
/// checks it for **one** adapter, on a clean sandbox, with the default
/// configuration — the case where the two are least likely to differ. The
/// eleven adapters do not write the same files: some carry a plugin, some a
/// hooks entry, some an MCP registration, and the `Change` a run reports
/// depends on what is already there.
///
/// So all three states are walked: nothing installed, installed already, and
/// installed with a different answer underneath. The third is the one that has
/// bitten before — a `Replace` classified from the disk rather than from what
/// the run is about to write agreed with the plan by luck.
/// Every file an adapter's gate lives in is one the gate measures writes to.
///
/// The fragments are hand-written, and what they have to cover is computed:
/// `resolve_paths` says where each adapter's hooks, plugin and skill actually
/// land, and a fragment list that misses one is a gate an agent can remove with
/// an ordinary `Write`.
///
/// This is the half that stops the list going stale. A twelfth adapter, or one
/// that moves its settings file, fails here rather than silently falling
/// outside the population.
#[test]
fn every_control_file_an_adapter_has_is_one_the_gate_measures() {
    // guard:population control-surface — a plain `//` inside the body, because
    // a `///` here is parsed as a second *declaration* of the family rather
    // than as this test claiming it. Same mistake as round six.
    //
    // Every platform, not the host's. `sandbox()` pins `Platform::Unix`, and this
    // is the only thing tying `paths_in` to `is_control_surface` — so it walked
    // one branch of a function full of them. A reviewer measured what that hid:
    // giving Cursor a Windows-only instruction path left the whole suite green
    // with the file unmeasured, while the same drift on a plain directory rename
    // was caught. The neighbouring `a_moved_home_moves_every_root_under_it`
    // already loops all three; this had no reason not to.
    let (_home, base) = sandbox();
    // Two XDG layouts as well as three platforms. A reviewer measured that this
    // walked one of each, and both surfaces a relocated `XDG_CONFIG_HOME` left
    // ungated were found by a person rather than by this test — which is the work
    // it exists to make unnecessary.
    for platform in [Platform::Windows, Platform::MacOs, Platform::Unix] {
        for moved in [false, true] {
            let options = SetupOptions {
                platform: Some(platform),
                config_home: if moved {
                    Some(_home.path().join("moved-xdg"))
                } else {
                    base.config_home.clone()
                },
                ..base.clone()
            };
            for adapter in AGENTS {
                let paths = resolve_paths(adapter, &options).expect("paths resolve");
                // The bare root as well as a file under it. A reviewer measured the
                // split that a trailing slash makes: `surface_of` appends a separator,
                // so `rm <root>` was `Boundary` while a write to the bare directory was
                // `Routine` — and this walk only ever asked about `SKILL.md` inside it.
                // Asking about the directory is what found the same split in four more
                // entries.
                //
                // Held by no test today, and worth saying rather than leaving to be
                // discovered: `names` matches a directory entry by the directory
                // itself, so removing this line changes nothing measurable. What it
                // is for is the adapter added later whose root is a directory — the
                // fixture that covers this behaviour spells its paths by hand and
                // would not know about it.
                let mut watched: Vec<std::path::PathBuf> =
                    vec![paths.skill_root.clone(), paths.skill_root.join("SKILL.md")];
                watched.extend(paths.hooks.clone());
                watched.extend(paths.plugin.clone());
                watched.extend(paths.mcp_config.clone());
                // The instruction file, which this walk did not reach for as long as it
                // existed. It is the file `setup` writes the workflow-authority directive
                // into — the sentence telling an agent this harness holds the authority
                // at all — so an agent that rewrites it removes the reason it obeys, and
                // it answered `Routine`. Eleven paths on any one platform, twelve
                // spellings across all three — gemini-cli is the one adapter whose
                // instruction file moves with the platform. An earlier version of this
                // said thirteen and "the two that differ by platform": thirteen is the
                // issue's own count, which includes two `~/.claude` paths this walk does
                // not touch, and only one adapter has a platform branch.
                watched.push(paths.instructions.clone());
                // And the agent-definition root, which was the one path this change
                // hand-spelled and the one it left uncrossed — a reviewer named that
                // as the shape the change's own prose condemns. `definition_for`
                // reads the tool allowlist it enforces from here.
                watched.extend(paths.agents_root.clone());

                for file in watched {
                    let target = file.display().to_string();
                    // Both roads. `surface_of` splits a command on whitespace before
                    // matching, so a path containing a space can answer `Boundary` on
                    // the write tool and `Routine` through the shell — which is the
                    // road an agent takes to delete something, and the reason the
                    // `cli/hosts.yml` entry exists at all. This walk asked only about
                    // writes, so the next entry of that shape would have arrived
                    // unnoticed; a reviewer named it before it did.
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
                            "{platform:?}/{}: {road} on {target} is where its gate lives and \
                         the gate calls it routine",
                            adapter.slug
                        );
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
        merge: crate::config::MergeStrategy::Squash,
        planning: crate::config::Planning::Sdd {
            openspec: true,
            lite: false,
        },
        ..Config::default()
    };

    for adapter in AGENTS {
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
            let planned = strip(setup(adapter, config, &dry).expect("the plan is produced"));
            let performed = strip(setup(adapter, config, &options).expect("setup succeeds"));
            assert_eq!(
                planned, performed,
                "{}: with {state}, the plan and the act disagree",
                adapter.slug
            );
        }
    }
}

/// What was theirs before is theirs after, in every file and every adapter.
///
/// `uninstall_leaves_no_file_estigia_created` counts **files**, so a file that
/// existed before, was edited by setup and not restored, passes it: the count
/// never moved. `a_gated_agent_s_hooks_file_keeps_everybody_else_s_entries`
/// checks contents, for three adapters and their hooks file only.
///
/// This is the operator's own requirement, in their words: *if I have another
/// skill, my own config or my own `.md`, they stay mine*. So every adapter, and
/// every file it touches — hooks, plugin, MCP registration and the instruction
/// file — seeded with content of the shape that file really holds, and read
/// back **byte for byte** after the round trip.
#[test]
fn every_file_an_adapter_touches_comes_back_exactly_as_it_was() {
    // Twice: once over files that do not carry the envelope's own scaffolding,
    // and once over files that do. The second pass is the operator's real one —
    // `version` is Cursor's required field, not Estigia's invention, and a
    // Cursor user who already keeps hooks has it.
    for with_scaffolding in [false, true] {
        round_trip(with_scaffolding);
    }
}

fn round_trip(with_scaffolding: bool) {
    let seed = |path: &std::path::Path| -> String {
        // Shaped like the file really is: a JSON object stays parseable, a
        // markdown file has headings above and below wherever a fence lands.
        let body = match path.extension().and_then(|kind| kind.to_str()) {
            Some("json") if with_scaffolding => {
                // A Cursor hooks file as a Cursor user really has one: their
                // own entries, and the `version` their format requires. It was
                // being deleted by the uninstall, because the removal that
                // takes Estigia's own scaffolding away could not tell theirs
                // from ours.
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

    for adapter in AGENTS {
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
            Some(paths.instructions.clone()),
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
            // No difference is allowed, in either direction. This used to let
            // a scaffolding key be **left behind** that the file did not
            // carry, on the grounds that the two cases could not be told
            // apart: Estigia wrote `version` into a Cursor hooks document
            // whether or not one was there, so `{"mine": …, "version": 1}`
            // after an uninstall was either their field kept or Estigia's
            // field stranded — the same bytes either way.
            //
            // They are distinguishable now, because the ambiguity was the
            // defect. `render_cursor_hooks` writes `version` only into a file
            // with nothing of anybody's in it, so a key in a file the operator
            // already kept hooks in is theirs by construction. Removing the
            // carve-out is how that fix is held: put it back and this guard
            // stops noticing.
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
    }
}

/// Theirs is theirs even when it names Estigia.
///
/// The push guard was replaced for exactly this reason once: a hook that
/// *mentioned* `estigia` read as Estigia's own. The settings files are merged
/// rather than replaced, so the same mistake here would take an entry out of a
/// file Estigia only ever added one line to.
#[test]
fn an_entry_of_theirs_that_names_estigia_is_still_theirs() {
    for slug in [
        "claude-code",
        "cursor",
        "gemini-cli",
        "codex",
        "crush",
        "continue",
        "qwen",
    ] {
        let home = tempfile::tempdir().expect("a temporary home");
        let options = SetupOptions {
            home_dir: Some(home.path().to_path_buf()),
            ..SetupOptions::default()
        };
        let adapter = agent(slug);
        let paths = resolve_paths(adapter, &options).expect("paths resolve");
        let Some(hooks) = paths.hooks.clone() else {
            continue;
        };
        fs::create_dir_all(hooks.parent().expect("a parent")).expect("the directory");
        // Their own pre-tool hook, running a linter whose name starts with
        // `estigia`, in the envelope this adapter reads.
        let theirs = "{\n  \"mine\": \"do not touch\",\n  \"hooks\": {\n    \"PreToolUse\": [\n      {\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\",\n        \"command\": \"estigia-lint --check\", \"timeout\": 3}]}\n    ]\n  }\n}\n";
        fs::write(&hooks, theirs).expect("their file");

        setup(adapter, &Config::default(), &options).expect("setup runs");
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
    }
}

/// An uninstall takes Estigia's hooks out and leaves the operator's file alone.
///
/// The stripper dropped **every** event key whose array was empty, on the
/// stated grounds that "it was not there before Estigia arrived". That is an
/// assumption about how the key got there, not a check: an operator who wrote
/// `"Notification": []` by hand — a slot they meant to fill — had it deleted by
/// an uninstall that was supposed to remove only Estigia's own entries.
///
/// Worse than the deletion is what it does to invariant two, stated four lines
/// above the bug: a file that never mentioned Estigia is *reported unchanged
/// rather than touched*. With an empty key in it, that file came back rewritten
/// and the uninstall counted it.
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

    // The same rule one level up. An operator's empty *wrapper* is theirs too,
    // and the block that drops it asked whether it was empty rather than
    // whether we emptied it.
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
    // emptied, which does go, because that one *was* ours.
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

/// Cursor's uninstall leaves the operator's file alone too.
///
/// The same rule, the third door. `strip_cursor_hooks` carries the same
/// invariant-two comment as `strip_hooks` and had the same hole under it:
/// it dropped an event key for *being* empty rather than for having been
/// emptied here, so an operator's unfilled slot went out with Estigia's gate.
///
/// Written as its own test rather than folded into the other one because that
/// is the point — the two strippers are separate code, and a fix applied to
/// one of them is exactly what this round found had not reached the other.
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

/// An uninstall leaves the operator's empty MCP container where it was.
///
/// The fourth door of the same rule, and the one that says most about how the
/// rule spread: the comment above this removal cites `strip_hooks` as the
/// model it was copying — *"`strip_hooks` has kept this rule for its own
/// events since it was written; this one did not"*. What it copied was the
/// defect. `strip_hooks` asked whether a container was **empty**, not whether
/// this uninstall had **emptied** it, so an operator holding an unfilled
/// `"mcpServers": {}` had it taken out by an uninstall that removed nothing
/// from it — and the file rewritten to say so.
#[test]
fn an_uninstall_leaves_an_empty_mcp_container_the_operator_wrote_where_it_was() {
    let path = std::path::Path::new("settings.json");

    // Theirs, unfilled, with nothing of Estigia's ever in it.
    for key in ["mcpServers", "mcp"] {
        let theirs = format!("{{\n  \"{key}\": {{}}\n}}\n");
        assert_eq!(
            render::strip_mcp(path, &theirs, render::McpFormat::McpServers).expect("readable"),
            theirs,
            "an uninstall rewrote a file that never mentioned Estigia, and took the operator's \
             own empty {key} with it"
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

/// Installing and uninstalling into an operator's Cursor file is a round trip.
///
/// The other half of the same rule. The four doors found so far were all about
/// *removing* something of the operator's on the way out; this is about
/// *leaving* something of Estigia's. `render_cursor_hooks` puts `version: 1`
/// into whatever file it finds, and `strip_cursor_hooks` never takes it out —
/// so an operator whose hooks file had no version key kept one forever, put
/// there by a tool that had supposedly left.
///
/// Stated as a round trip rather than as two assertions because that is the
/// property that matters and the one neither side can hold alone: what the
/// operator wrote is what they get back.
#[test]
fn cursor_hooks_come_back_as_the_operator_wrote_them() {
    let path = std::path::Path::new("hooks.json");
    let spec = agent("cursor").gate_spec().expect("cursor can be gated");
    let executable = std::path::Path::new("/opt/estigia");

    for theirs in [
        // No version key of their own, which is the case that bit.
        "{\n  \"hooks\": {\n    \"afterFileEdit\": [\n      {\n        \"command\": \"mine\"\n      }\n    ]\n  }\n}\n",
        // And one that does carry it, which must survive untouched.
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

/// Codex's TOML comes back byte for byte when Estigia was never in it.
///
/// The JSON strippers all compare against what they read and hand back the
/// original when they changed nothing — invariant two, stated in a comment
/// above each of them. The TOML one is reached before that comparison, by an
/// early return at the top of `strip_mcp`, and it rewrites unconditionally:
/// CRLF normalised to LF and trailing blank lines trimmed. On Windows, where
/// an operator's `config.toml` is CRLF by default, an uninstall that removed
/// nothing still reported `update` and rewrote every line ending in their file.
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
    // regress: a stripper that hands everything back unchanged is worse than
    // one that reformats.
    let mixed = "[mcp_servers.suyo]\ncommand = \"otro\"\n\n[mcp_servers.estigia]\ncommand = \"estigia\"\nargs = [\"mcp\"]\n";
    let after = render::strip_mcp(path, mixed, render::McpFormat::CodexToml).expect("readable");
    assert!(
        !after.contains("estigia") && after.contains("suyo"),
        "the section was not lifted out, or theirs went with it: {after:?}"
    );
}

/// An instruction file the operator kept empty survives the uninstall.
///
/// The other side of `uninstall_leaves_no_file_estigia_created`, and the two
/// were in tension until the record could tell them apart. A `CLAUDE.md`
/// holding nothing but the directive block is either one Estigia made — which
/// must go — or one that was already there with nothing in it, which is the
/// operator's `.md` and must stay. Afterwards they are the same bytes, so the
/// answer is remembered at install time rather than guessed at removal time.
///
/// Both halves are asserted here on purpose. Fixing this by never deleting the
/// file passes the first half and leaves one empty file per agent behind, which
/// the sibling test refuses; fixing it by always deleting passes the sibling and
/// takes a file that was theirs. Only provenance passes both, so only a test
/// that asks for both holds the fix in place.
#[test]
fn an_instruction_file_the_operator_left_empty_is_not_estigias_to_delete() {
    for adapter in AGENTS {
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
    }
}

/// Every tool the classifier judges is one some matcher wakes the hook for.
///
/// The way back. `every_matcher_names_tools_the_gate_can_classify` walks each
/// matcher and proves the classifier knows what it names; this walks the
/// classifier and proves some matcher names it. The two failures are not the
/// same: a matcher naming a tool the classifier ignores wakes a process to
/// answer "not mine", and a name the classifier judges that no matcher wakes
/// for is a gate that never runs at all.
///
/// Only the second half is here, and deliberately — the first already exists,
/// does it behaviourally by calling `classify` rather than reading the table,
/// and duplicating it would put one rule behind two doors, which is the defect
/// this pair was written to catch.
///
/// What it found: `Update`, which `WRITE_TOOLS` has attributed to Claude Code
/// all along and whose matcher did not name it — a tool this crate believes
/// writes files and had arranged never to be woken for. The guard that should
/// have caught it was crossing a copy of the matcher held in `hook::EVENTS`
/// that nothing installs, against a literal list of four names.
#[test]
fn every_tool_the_classifier_judges_is_one_some_matcher_wakes_for() {
    use crate::harness::{SHELL_TOOLS, WRITE_TOOLS};

    // Names belonging to an agent that carries no matcher at all, which is a
    // gap the honesty contract states rather than one this crossing can close.
    // Spelled out with the reason, so a name arriving here is a decision
    // somebody made rather than one the crossing made for them.
    const NO_MATCHER_TO_BE_IN: &[(&str, &str)] = &[
        ("pre_write_code", "Windsurf, whose event is the tool name"),
        ("pre_run_command", "Windsurf, whose event is the tool name"),
        // OpenCode does register one — a `const GATED = [...]` inside the
        // plugin this crate generates, which `the_plugin_gates_the_tools_the_
        // classifier_judges` crosses. It is invisible from here because it is
        // not on the `GateSpec`, not because it is absent.
        ("patch", "OpenCode, whose matcher lives in its plugin body"),
        (
            "shell",
            "Codex's own name for it; its hook layer sends `Bash`",
        ),
    ];

    let woken: Vec<String> = AGENTS
        .iter()
        .filter_map(|adapter| adapter.gate_spec().and_then(|spec| spec.matcher))
        // Through the reader `doctor` uses on the matcher already on disk, so
        // the tools this crossing thinks a matcher wakes for and the ones that
        // check thinks it wakes for cannot come apart.
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

    // Floors: a crossing over an empty side proves nothing, and a guard that
    // finds nothing reads like one that finds nothing wrong.
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

/// OpenCode's plugin gates the tools the classifier judges, and only those.
///
/// The fourth copy of one rule, and the only one written in another language:
/// the classifier's populations, each adapter's `matcher`, `hook::EVENTS`, and
/// a `const GATED = [...]` inside the JavaScript this crate generates. Nothing
/// crossed the fourth, and it is the one that decides for the adapter whose
/// **plugin is its gate** — OpenCode blocks a call by throwing from
/// `tool.execute.before`, so a tool missing from that array is not a slower
/// gate, it is no gate, behind a `doctor` row reporting the gate as live.
///
/// Found while sweeping literals that appear in two production files. It also
/// corrects `every_tool_the_classifier_judges_is_one_some_matcher_wakes_for`,
/// whose exemption said OpenCode "registers no matcher": it registers one, in
/// the plugin body rather than on the `GateSpec`, which is why the crossing
/// there could not see it.
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

    // The other way, minus the names that belong to other hosts. Each is
    // spelled out with whose it is, because a name arriving in this list has to
    // be a decision somebody made rather than one the crossing made for them.
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
    let doors: [Door; 5] = [
        ("hooks", |path, text| render::strip_hooks(path, text)),
        ("cursor hooks", |path, text| {
            render::strip_cursor_hooks(path, text)
        }),
        ("mcp servers", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::McpServers)
        }),
        ("mcp crush", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::CrushStdio)
        }),
        ("mcp local", |path, text| {
            render::strip_mcp(path, text, render::McpFormat::Local)
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
        merge: crate::config::MergeStrategy::Rebase,
        ..Config::default()
    };
    super::write_repository_configuration(&path, &config, crate::config::EVERYWHERE_SETTINGS)
        .expect("it is written");
    let written = std::fs::read_to_string(&path).expect("it is there");
    assert!(
        written.contains("Merge strategy"),
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
fn a_file_written_twice_in_one_run_keeps_the_endings_it_came_with() {
    let (_home, options) = sandbox();
    let adapter = AGENTS
        .iter()
        .find(|adapter| adapter.slug == "gemini-cli")
        .expect("gemini-cli is an agent this build knows");
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
    setup(adapter, &Config::default(), &options).expect("setup succeeds");

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
    let mut swept = 0;
    for adapter in AGENTS {
        let (_home, options) = sandbox();
        let Ok(paths) = resolve_paths(adapter, &options) else {
            continue;
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
                .clone()
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
            continue;
        }
        uninstall(adapter, &options).expect("uninstall succeeds");

        for (path, text) in &written {
            swept += 1;
            assert_eq!(
                &fs::read_to_string(path).expect("their file survives"),
                text,
                "{}: {} did not come back byte for byte",
                adapter.slug,
                path.display()
            );
        }
    }
    // The floor: a sweep that skipped every adapter would pass in silence.
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
        setup(adapter, &Config::default(), &options).expect("setup succeeds");
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

    let woken: Vec<String> = AGENTS
        .iter()
        .filter_map(|adapter| adapter.gate_spec().and_then(|spec| spec.matcher))
        .flat_map(wiring::names_in)
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
    for name in &woken {
        assert!(
            judged.contains(&name.to_ascii_lowercase()),
            "a matcher wakes the hook for {name:?} and the classifier judges no such tool — the \
             hook fires, answers `Untouched`, and the write goes through while every report says \
             that tool is gated. Add it to `WRITE_TOOLS` or `SHELL_TOOLS`, or take it out of the \
             matcher that names it"
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

    let mut checked = 0;
    for adapter in AGENTS {
        if adapter.gate_spec().is_none() && adapter.instructions != InstructionFile::Cline {
            continue;
        }
        let (home, options) = sandbox();
        setup(adapter, &Config::default(), &options).expect("setup runs");
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
        checked += 1;
    }
    // The floor: a loop that checked nothing would pass every assertion in it,
    // and nine is the count a `setup --all` writes — eight through a settings
    // file and Cline's script.
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
    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup runs");
    }

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

    for adapter in AGENTS {
        uninstall(adapter, &options).expect("uninstall runs");
    }

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
    // and the next adapter added would not be in it.
    let (probe, probe_options) = sandbox();
    for adapter in AGENTS {
        setup(adapter, &Config::default(), &probe_options).expect("setup runs");
    }
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
    let mut written: Vec<std::path::PathBuf> = Vec::new();
    shared(probe.path(), &mut written);
    let relative: Vec<std::path::PathBuf> = written
        .iter()
        .filter_map(|path| path.strip_prefix(probe.path()).ok().map(Path::to_path_buf))
        .collect();
    // The floor: a probe that found nothing would make every assertion below
    // vacuous, and the fleet is the point.
    assert!(
        relative.len() >= 20,
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

    for adapter in AGENTS {
        setup(adapter, &Config::default(), &options).expect("setup runs");
    }
    for adapter in AGENTS {
        uninstall(adapter, &options).expect("uninstall runs");
    }

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

/// The planning phases land where the gate reads them, and only the ones that run.
///
/// The claim this whole feature rests on: other harnesses ship
/// sub-agent definitions carrying a `tools:` line and rely on the host to
/// honour it, and Estigia writes the same kind of file into the same place
/// while sitting at the tool boundary that can hold it. A definition written
/// anywhere else is a `tools:` line nothing enforces — the arrangement this
/// crate criticises in others — so *where* is half the assertion.
///
/// The other half is *which*. `Planning` decides which phases exist at all, and
/// a machine whose disk offers five agents while its configuration runs none is
/// the two ends disagreeing about what this installation does.
#[test]
fn the_phases_installed_are_the_phases_the_protocol_runs() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");

    // `direct` runs no planning phase, so none is written. A file offering
    // `sdd-spec` on a machine configured never to spec is an agent the host can
    // route to and the contract will refuse.
    setup(adapter, &Config::default(), &options).expect("setup runs");
    assert!(
        !agents.exists() || fs::read_dir(&agents).into_iter().flatten().next().is_none(),
        "`direct` installed a planning phase, and `direct` runs none"
    );

    // The short form runs spec and tasks and nothing else.
    let lite = Config {
        planning: crate::config::Planning::Sdd {
            openspec: false,
            lite: true,
        },
        ..Config::default()
    };
    setup(adapter, &lite, &options).expect("setup runs");
    let mut written: Vec<String> = fs::read_dir(&agents)
        .expect("the agents directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    assert_eq!(
        written,
        vec!["sdd-spec.md".to_owned(), "sdd-tasks.md".to_owned()],
        "the short form installed phases it does not run"
    );

    // And the tool list the gate will enforce is there, restrictive, and read
    // by the very function that judges a call.
    let spec = fs::read_to_string(agents.join("sdd-spec.md")).expect("the phase is installed");
    let policy = crate::harness::roles::declared_policy(&spec)
        .expect("an installed phase declares a policy the gate can read");
    assert_eq!(
        policy.verdict("Write"),
        crate::harness::roles::Verdict::Deny,
        "an installed planning phase may write, with the artifacts on the issue"
    );
    assert!(
        !spec.contains("{{"),
        "a placeholder reached the disk: {spec}"
    );

    // The uninstall takes back exactly what it wrote.
    uninstall(adapter, &options).expect("uninstall runs");
    assert!(
        !agents.join("sdd-spec.md").exists(),
        "a phase this run created outlived the uninstall"
    );
}

/// `Model routing`'s phase key becomes the line the host obeys.
///
/// The setting's own note said it was *"a declaration the agent reads, not a
/// dispatch this binary performs"*, and that was true: an operator could write
/// `design=opus` and no code path anywhere consulted it. This is the path that
/// makes it act.
#[test]
fn a_model_named_for_a_phase_reaches_that_phases_definition() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let mut config = Config {
        planning: crate::config::Planning::Sdd {
            openspec: true,
            lite: false,
        },
        ..Config::default()
    };
    config.models = crate::config::ModelRouting::parse("design=opus").expect("a routing");

    setup(adapter, &config, &options).expect("setup runs");
    let agents = home.path().join(".claude").join("agents");

    let design = fs::read_to_string(agents.join("sdd-design.md")).expect("the phase is installed");
    assert!(
        design.contains("model: opus"),
        "the model named for `design` did not reach its definition:\n{design}"
    );
    // A phase nobody named a model for inherits rather than carrying an empty
    // value, which is a frontmatter error and not a default.
    let explore = fs::read_to_string(agents.join("sdd-explore.md")).expect("the phase is there");
    assert!(
        explore.contains("model: inherit"),
        "an unnamed phase did not fall back to `inherit`:\n{explore}"
    );

    // With the artifacts under `openspec/`, the phases that leave one behind can
    // write it — and the two that only think still cannot.
    assert!(
        design.contains("tools: Read, Grep, Glob, Write, Edit"),
        "`openspec` did not give `design` the write it needs:\n{design}"
    );
    // On the `tools:` line, not anywhere in the file: the prose explains the
    // gate's reach and names `Write` while doing it, so a whole-file search
    // answers about the explanation rather than the declaration.
    let declared = explore
        .lines()
        .find(|line| line.starts_with("tools:"))
        .expect("the definition declares a tool list");
    assert!(
        !declared.contains("Write") && !declared.contains("Edit"),
        "`explore` was handed a write it never needs: {declared}"
    );
}

#[test]
fn setup_failures_carry_each_proven_boundary_and_are_exactly_replay_safe() {
    let effective = Config {
        planning: crate::config::Planning::Sdd {
            openspec: false,
            lite: true,
        },
        models: crate::config::ModelRouting::parse("spec=opus").expect("a model route"),
        ..Config::default()
    };
    for (boundary, last_kind) in [
        (SetupFailureBoundary::AfterSkill, ActionKind::Skill),
        (SetupFailureBoundary::AfterDirective, ActionKind::Directive),
        (SetupFailureBoundary::AfterPhase, ActionKind::PhaseAgent),
        (SetupFailureBoundary::AfterHooks, ActionKind::Hooks),
    ] {
        let (_home, options) = sandbox();
        let adapter = agent("claude-code");
        let mut pending = Pending::new();
        inject_setup_failure(adapter.slug, boundary);

        let failure = setup_adapter_into(
            adapter,
            &Config::default(),
            &effective,
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
                .any(|action| action.kind == last_kind && action.change != Change::Unchanged),
            "{boundary:?} lost its last proven action: {:?}",
            failure.result.actions
        );
        assert!(!failure.result.completed);
        assert!(!failure.write_attempted);
        let retry = setup_adapter_into(
            adapter,
            &Config::default(),
            &effective,
            &options,
            &mut pending,
            true,
        )
        .unwrap_or_else(|failure| panic!("{boundary:?} was not exactly replay safe: {failure}"));
        assert!(retry.completed, "{boundary:?} did not complete on retry");
    }
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
        &[crate::config::Setting::Planning],
    )
    .expect_err("the writer treated an I/O error as an absent document");

    assert_eq!(refusal.code, "config-local-unreadable");
    assert!(file.is_dir(), "the writer replaced the unreadable path");
}

/// Narrowing the protocol takes back the phases it no longer runs.
///
/// Found by using it, not by reading it: moving `Planning` from `sdd` to
/// `direct` left all five definitions on disk, so the host could still route to
/// `sdd-spec` on a machine configured to run no planning phase at all. An
/// installer that writes without retracting turns every configuration change
/// into an accumulation.
#[test]
fn a_phase_the_protocol_stopped_running_comes_back_off_the_disk() {
    let (home, options) = sandbox();
    let adapter = agent("claude-code");
    let agents = home.path().join(".claude").join("agents");
    let full = Config {
        planning: crate::config::Planning::Sdd {
            openspec: false,
            lite: false,
        },
        ..Config::default()
    };
    setup(adapter, &full, &options).expect("setup runs");
    assert_eq!(
        fs::read_dir(&agents).expect("the agents directory").count(),
        5,
        "full SDD did not install its five phases"
    );

    // Narrowed to the short form: the three it drops go.
    let lite = Config {
        planning: crate::config::Planning::Sdd {
            openspec: false,
            lite: true,
        },
        ..Config::default()
    };
    setup(adapter, &lite, &options).expect("setup runs");
    let mut left: Vec<String> = fs::read_dir(&agents)
        .expect("the agents directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(
        left,
        vec!["sdd-spec.md".to_owned(), "sdd-tasks.md".to_owned()],
        "the short form left behind a phase it does not run"
    );

    // And all the way back to `direct`, which runs none.
    setup(adapter, &Config::default(), &options).expect("setup runs");
    assert_eq!(
        fs::read_dir(&agents)
            .into_iter()
            .flatten()
            .flatten()
            .count(),
        0,
        "`direct` left a planning phase on disk, and `direct` runs none"
    );
}

/// A phase is written only where this crate can spell that host's dialect.
///
/// The gate reads sub-agent definitions from Claude Code's directory *and*
/// OpenCode's, so the destination list was both — and the payload is written in
/// Claude Code's dialect, where `tools:` is a comma-separated line. OpenCode's
/// schema wants an object. What landed there was not a narrower agent but an
/// invalid file, and the operator's whole `opencode` configuration stopped
/// loading with *Expected object | undefined, got "Read, Grep, Glob"*.
///
/// Sharing a destination is not sharing a format, and a `tools:` line this crate
/// cannot render for a host is a host it must not write to.
#[test]
fn a_phase_is_not_written_in_a_dialect_its_host_cannot_read() {
    let (home, options) = sandbox();
    let sdd = Config {
        planning: crate::config::Planning::Sdd {
            openspec: false,
            lite: false,
        },
        ..Config::default()
    };
    for adapter in AGENTS {
        setup(adapter, &sdd, &options).expect("setup runs");
    }
    let opencode = home.path().join(".config").join("opencode").join("agents");
    assert!(
        !opencode.exists() || fs::read_dir(&opencode).into_iter().flatten().count() == 0,
        "a phase was written into OpenCode's directory in Claude Code's dialect"
    );
    // And the one host whose dialect this payload *is* written in still gets
    // them, or the guard above would be satisfied by installing nothing at all.
    assert_eq!(
        fs::read_dir(home.path().join(".claude").join("agents"))
            .expect("Claude Code's agents directory")
            .count(),
        5,
        "the host this crate can spell for received no phases"
    );
}

/// Where a config home comes from, for every shape a caller can supply.
///
/// The whole table, because a reviewer found the interesting cell by reading
/// rather than by running: routing this through `setup::xdg_config_home` moved
/// one answer. `config_home: Some(<relative>)` with the variable set absolute
/// resolved to `$HOME/.config` before and to the **variable** after, because
/// filtering the override first let it fall through to the fallback. No caller
/// in this tree reaches that cell — every one sets `home_dir` alongside — and
/// the shipped binary has no `--config-home` flag at all, which is exactly why
/// nothing was red. An override that is named badly is a caller's mistake, and
/// inheriting the machine's config home instead is the half-move
/// `Environment::resolve` exists to refuse.
///
/// The absolute paths are temporary directories rather than literals, because a
/// POSIX-looking `/moved/config` is **not** absolute on Windows and the first
/// draft of this test failed for that reason rather than for a real one.
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

    // What a borrowed home falls back to when the variable says nothing. Taken
    // by measurement rather than spelled, since it is the machine's own.
    let borrowed_fallback = with_config_home(std::path::Path::new(""), || resolve(None, None));

    with_config_home(moved.path(), || {
        assert_eq!(
            resolve(None, None),
            moved.path(),
            "a borrowed home takes the variable"
        );
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
        // The cell that moved: a useless override is not a reason to inherit the
        // variable, and both roads have to agree that it is simply absent.
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
