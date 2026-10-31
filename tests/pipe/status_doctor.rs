use super::*;

#[test]
fn a_row_that_is_broken_comes_out_of_the_report_broken() {
    // The row builders are measured one by one. **The assembled report was
    // not**, and that is where a row is lost.
    //
    // Measured by mutation: downgrading every `Broken` of one row family to
    // `Fine` on the way out of `doctor::full` leaves the whole suite green for
    // ten of the eleven families — `contract`, `gate`, `tools`, `gh`,
    // `push-guard`, `remote`, `stand-down`, `run-pointer`, `silence` and
    // `transport`. Only `skill` is held. So a row could be built correctly and
    // dropped, filtered or softened on the way into the report, and every unit
    // test on its builder would still pass.
    //
    // That is not hypothetical here: `full` once failed to pass the home
    // directory to `state_root`, and the push-guard row vanished entirely under
    // a tracker with no transport. Both were found by running the binary and
    // looking, which is what this does.
    //
    // Three states this can force on any machine, and the rows they must
    // produce. The environment-dependent families — `gh`, `remote`,
    // `push-guard`, `gate`, `tools` — are named in the README's
    // honesty contract as still uncrossed at the assembly level rather than
    // pretended at here.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");

    // 1. Nothing installed: the skill row, which is the one family already held.
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   skill"),
        "an empty machine did not report the skill row broken:\n{out}"
    );

    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // 1b. An install record naming a different asset set: the install is
    //     stale, and the row says so rather than clearing it. Restored with
    //     a real `sync`, which also proves the remedy the row names.
    let record = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join(".estigia")
        .join("installed.json");
    let mut stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).expect("the record reads"))
            .expect("the record parses");
    stored["assets"] = serde_json::json!("sha256:0");
    std::fs::write(
        &record,
        serde_json::to_string_pretty(&stored).expect("renders"),
    )
    .expect("an older install");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   skill-assets"),
        "an install from another asset set did not report the skill-assets row broken:\n{out}"
    );
    let (_, stderr, ok) = run(home.path(), &["sync"], "");
    assert!(ok, "sync failed: {stderr}");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !out.contains("BROKEN   skill-assets"),
        "a synced install still reports stale assets:\n{out}"
    );

    // 2. The contract taken away from under an agent that is registered.
    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    std::fs::remove_file(&contract).expect("the contract goes");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   contract"),
        "a registered agent with no contract did not report the contract row broken:\n{out}"
    );
    run(home.path(), &["sync"], "");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // 3. A stand-down record this build cannot read. Whether the gate is
    //    standing down is then unknown, which is the one thing that row is for.
    let record = home.path().join(".estigia").join("stand-down.json");
    std::fs::create_dir_all(record.parent().expect("a directory")).expect("the directory");
    std::fs::write(&record, "{ this is not a record }").expect("the wreckage");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   stand-down"),
        "an unreadable stand-down did not report the stand-down row broken:\n{out}"
    );
    std::fs::remove_file(&record).expect("the wreckage goes");

    // 4. A run pointer this build cannot read: what that run holds is then
    //    unknown, and the gate measures writes against it.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    std::fs::write(runs.join("claude-aaaaaaaa.json"), "{ not a pointer }").expect("the wreckage");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   run-pointer"),
        "an unreadable run pointer did not report the run-pointer row broken:
{out}"
    );
    std::fs::remove_file(runs.join("claude-aaaaaaaa.json")).expect("the wreckage goes");

    // 5. A registered gate naming a binary that is not there. It is registered,
    //    so nothing looks unconfigured, and it would not run — which is the
    //    difference this row exists to draw.
    let settings = home.path().join(".claude").join("settings.json");
    let written = std::fs::read_to_string(&settings).expect("the settings read");
    // By the file name rather than the whole path: the settings hold it JSON
    // escaped, and the raw path never matches.
    let broken = written
        .replace("estigia.exe", "no-such-estigia.exe")
        // And the Unix spelling, escaped as the JSON holds it. Without this arm
        // the file was unchanged on Linux and macOS, so the gate was still
        // registered to a binary that was there and the row was correctly not
        // broken — the test asserted a repair it had never made.
        .replace("/estigia\\\"", "/no-such-estigia\\\"");
    assert_ne!(
        broken, written,
        "the fixture did not point the gate at a binary that is missing"
    );
    std::fs::write(&settings, broken).expect("the settings are edited underneath it");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   gate"),
        "a gate registered to a binary that is not there did not report the gate row broken:\n{out}"
    );

    // 6. The same for the tool server, which is a different file and a
    //    different sentence.
    let servers = home.path().join(".claude.json");
    let written = std::fs::read_to_string(&servers).expect("the server list reads");
    let broken = written
        .replace("estigia.exe", "no-such-estigia.exe")
        // The Unix spelling, and note the escaping differs from the settings
        // file above. There the command is a *shell string inside* a JSON
        // string, so the path is followed by `\"`; here the command is a JSON
        // string in its own right and it is followed by a plain `"`. One file,
        // one shape — assuming the other silently matched nothing.
        .replace("/estigia\"", "/no-such-estigia\"");
    assert_ne!(
        broken, written,
        "the fixture did not point the tool server at a binary that is missing"
    );
    std::fs::write(&servers, broken).expect("the server list is edited underneath it");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   tools"),
        "a tool server naming a binary that is not there did not report the tools row broken:\n{out}"
    );
    run(home.path(), &["setup", "claude-code"], "");

    // 7. A checkout, for the two rows that are about one. A repository with no
    //    remote cannot say which repository holds the issues; a `pre-push` hook
    //    nothing can read leaves the push boundary's state unknown.
    let repository = tempfile::tempdir().expect("a repository");
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repository.path())
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    if git(&["init", "--quiet"])
        && git(&["config", "user.email", "rows@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"])
    {
        let (out, _, _) = run_in(home.path(), repository.path(), &["doctor"], "");
        assert!(
            out.contains("BROKEN   remote"),
            "a checkout with no remote did not report the remote row broken:\n{out}"
        );

        let hook = repository
            .path()
            .join(".git")
            .join("hooks")
            .join("pre-push");
        std::fs::create_dir_all(hook.parent().expect("a directory")).expect("the hooks directory");
        // Not text. `estigia guard` refuses to replace a hook it cannot
        // identify, so this state stays until somebody looks — and the row is
        // the only thing that tells them to.
        std::fs::write(&hook, [0x23, 0x21, 0x0a, 0xff, 0xfe, 0x0a]).expect("the unreadable hook");
        let (out, _, _) = run_in(home.path(), repository.path(), &["doctor"], "");
        assert!(
            out.contains("BROKEN   push-guard"),
            "a pre-push hook nothing can read did not report the push-guard row broken:\n{out}"
        );

        // 7c. `gate-version`: whether the binary answering this
        // machine's hooks is the checkout's own build. `repository` shares no
        // history with the crate this test binary was built from, so if that
        // binary carries a commit at all, `git merge-base --is-ancestor`
        // cannot answer either *yes* or *no* about it against `repository`'s
        // head — it exits 128, an object neither repository's history holds
        // in common with the other, which is *cannot tell*, not *older*.
        // Shares-no-history is not the same fact as older: an ancestor check
        // that cannot be answered must not be read as a settled stale gate,
        // which is the whole finding this row was rewritten to stop making.
        //
        // Three states are reachable here, not two. A build made from
        // uncommitted changes — which this one usually is, mid-development —
        // cannot make the ancestry comparison at all, for the same *cannot
        // tell* reason as an unrelated history, just reached through the
        // dirty-tree branch instead. And a build made where git was
        // unavailable carries no commit at all — `estigia 0.1.0 (unknown
        // build)`, a configuration `src/build_info.rs` documents — which is a
        // third, distinct *cannot tell*. None is a weaker test: all three are
        // the row's real *cannot tell* branches, asserted on their own words
        // rather than let through as an unrelated pass — there is no
        // reachable state here in which this row reports broken, since
        // nothing in this fixture ever builds two commits with a genuine
        // ancestor relationship. `harness::doctor`'s own
        // `gate_version_reports_fine_and_genuinely_stale_against_real_commits`
        // drives that branch, against a real child commit of this binary's
        // own build.
        //
        // The branch is picked from the identity the **binary** reports about
        // itself, never from `estigia::build_info::dirty()`. That constant is
        // compiled into the test target's copy of the library — a different
        // artifact from the binary printing the row — and asking one artifact
        // about the other is the only way to assert the row against
        // a shape the binary is not in. `--version` prints `estigia <VERSION>`,
        // and `VERSION` always carries a parenthetical build marker, so the
        // binary's own report is what selects the branch.
        //
        // The three cannot-tell shapes are told apart without asking git: `,
        // dirty` is the uncommitted tree, `unknown build` is the build with no
        // commit, and anything else is a commit this checkout's history does
        // not hold. The order mirrors `gate_age_verdict`'s: it reports the
        // dirty cannot-tell before it looks at the commit, so a build that is
        // both dirty and commitless reads as dirty here too.
        let (out, _, _) = run_in(home.path(), repository.path(), &["doctor"], "");
        let (version, _, _) = run_in(home.path(), repository.path(), &["--version"], "");
        if version.contains(", dirty") {
            assert!(
                out.contains("skipped  gate-version") && out.contains("uncommitted changes"),
                "a binary built from an uncommitted tree did not report gate-version as \
                 cannot-tell:\n{out}"
            );
        } else if version.contains("unknown build") {
            assert!(
                out.contains("skipped  gate-version") && out.contains("carries no commit"),
                "a binary built with no git identity did not report gate-version as \
                 cannot-tell:\n{out}"
            );
        } else {
            assert!(
                out.contains("skipped  gate-version") && out.contains("could not be established"),
                "a binary whose commit shares no history with the checkout did not report \
                 gate-version as cannot-tell:\n{out}"
            );
        }
    } else {
        eprintln!("SKIPPED: git is not usable here, so two of the rows were not forced.");
    }

    // 7b. Two installed roots and the operator's own file in one of them, so
    //     the root the gate decides in carries a row the other agent does not
    //     read. This is the state the defect was filed from, and it printed eleven
    //     `ok`s: both halves telling the truth about their own file, and
    //     nothing comparing them.
    let (_, stderr, ok) = run(home.path(), &["setup", "agents"], "");
    assert!(ok, "setup failed: {stderr}");
    let theirs = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.local.md");
    std::fs::write(
        &theirs,
        "| Setting | Value here |\n|---|---|\n| Change size | 120 |\n",
    )
    .expect("the operator's own file");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   canonical"),
        "a gate deciding by rows an agent never reads did not report the canonical row broken:\n\
         {out}"
    );
    std::fs::remove_file(&theirs).expect("their file goes");

    // 8. A machine with no GitHub CLI on it. The row is about a program rather
    //    than about a file Estigia wrote, so it is forced by taking the program
    //    away — an empty search path, which is the only state in this test that
    //    is about the world rather than about the installation.
    let nowhere = tempfile::tempdir().expect("an empty directory");
    let (out, _, _) = run_with_path(
        home.path(),
        home.path(),
        &["doctor"],
        "",
        Some(nowhere.path()),
        &[],
    );
    assert!(
        out.contains("BROKEN   gh"),
        "a machine with no GitHub CLI did not report the gh row broken:\n{out}"
    );

    // 9. A local store that is there and will not open. A directory where the
    //    database file belongs is the state, and it is forced without a second
    //    program: the row is about a file the runtime writes, not one setup does.
    let store_path = home.path().join(".estigia").join("state.db");
    std::fs::create_dir_all(&store_path).expect("a directory where the database belongs");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   store"),
        "a store that will not open did not report the store row broken:\n{out}"
    );
    std::fs::remove_dir_all(&store_path).expect("the stand-in goes");

    // And the population: every row this build can print is one of the states
    // above. A row added later fails here until somebody either forces it or
    // says why they cannot.
    let (out, _, _) = run(home.path(), &["doctor"], "");
    let printed: std::collections::BTreeSet<String> = out
        .lines()
        .filter_map(|line| {
            // A row, not the resolution line under it: the verdict is the first
            // word, and only three words can be one.
            let mut words = line.split_whitespace();
            let verdict = words.next()?;
            ["ok", "skipped", "BROKEN"]
                .contains(&verdict)
                .then(|| words.next())
                .flatten()
                .map(ToOwned::to_owned)
        })
        .collect();
    const FORCED: &[&str] = &[
        "skill",
        // Forced broken in step 1b above, against a record naming another
        // asset set — and restored with the `sync` the row names.
        "skill-assets",
        "contract",
        "canonical",
        "gate",
        "tools",
        "gh",
        "push-guard",
        "remote",
        "stand-down",
        "run-pointer",
        "silence",
        // Exercised in step 7c above, against a checkout sharing no history
        // with this binary — `skipped` (cannot tell) either way, since an
        // unrelated history is unreadable ancestry rather than a settled
        // *older*, and all three routes there (a dirty build, a build
        // carrying no commit at all, and unreadable ancestry) are asserted. Genuinely `Broken`, on a real ancestor pair, is
        // `harness::doctor`'s own
        // `gate_version_reports_fine_and_genuinely_stale_against_real_commits` —
        // not forced through this binary, the same shape as `stale-run-pointer`
        // below.
        "gate-version",
        // Not forced broken here for the same reason: this machine holds no
        // stale run pointer at this point in the fixture, and forcing one
        // needs a `gh` that answers `issue-not-open` for a specific issue
        // rather than the blanket absence step 8 already forces — measured
        // directly, with a scripted `gh`, in `harness::doctor`'s own module
        // instead (`a_readable_stale_pointer_is_reported_and_names_the_
        // release_command`).
        "stale-run-pointer",
        // Not forced broken here either: the fixture holds no merged or closed
        // branch or checkout for the sweep to name, and forcing one needs a
        // remote and a scripted `gh`. Measured directly in `harness::prune`'s
        // own module instead (`the_doctor_row_reports_a_closed_checkout_red`).
        "prune",
        // Skipped here: no Pi is configured. Broken against real settings files in
        // `harness::doctor`'s own module (`pi_settings_that_exclude_the_extension_or_the_server_are_broken`).
        "pi-config",
        // Skipped here for the same reason; reported against real runners in
        // `harness::doctor`'s own module.
        "pi-subagents",
        // Never broken: it reports a mode the operator chose, and an unreadable
        // file is `skipped`. Both outcomes are measured in `harness::doctor`'s
        // own module.
        "permission",
        // Forced broken in step 9 above, against a directory standing where
        // the database file belongs.
        "store",
        // Broken by the same step 9: a store that does not open cannot be read
        // for its counted failures either.
        "store-records",
    ];
    let unforced: Vec<&String> = printed
        .iter()
        .filter(|name| !FORCED.contains(&name.as_str()))
        .collect();
    assert!(
        unforced.is_empty(),
        "these rows are printed and no state above forces them broken: {unforced:?}"
    );
    // The floor: the parse found the rows at all.
    assert!(
        printed.len() >= 8,
        "only {} row name(s) were read out of the report: {printed:?}",
        printed.len()
    );

    // 9. A ledger line saying a call went through ungated.
    let (_, _, _) = run(home.path(), &["hook", "pre-tool-use"], "");
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        out.contains("BROKEN   silence"),
        "a call recorded as ungated did not report the silence row broken:\n{out}"
    );
}

#[test]
fn what_status_prints_is_what_somebody_needs_to_act_on() {
    // The same lens as `doctor`'s rows, on the other assembled report: the
    // pieces are computed correctly and **nothing crossed the page**.
    //
    // Measured by mutation. Three parts of `status` are held — the stand-down
    // line, each agent's state word, and the per-agent `estigia setup <agent>`
    // — and two were not: blanking the **skill root** under a configured agent,
    // and dropping the **companion section** entirely, both left the whole suite
    // green.
    //
    // Neither is decoration. The skill root is the answer to *which directory
    // do I look at*, which is the question somebody has when `status` says
    // their skill is out of date; and the companion section is the only place
    // that says whether the memory this workflow leans on is installed at all.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let (out, _, ok) = run(home.path(), &["status"], "");
    assert!(ok, "status refused: {out}");

    // The agent, its state, and where it reads from.
    assert!(
        out.contains("claude-code"),
        "the configured agent is not named: {out}"
    );
    assert!(
        out.contains("configured"),
        "the agent's state is not said: {out}"
    );
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .display()
        .to_string();
    assert!(
        out.contains(&root),
        "the skill root is not printed, so `out of date` names no directory: {out}"
    );

    // And the companion, whose section is printed whether or not it is there.
    assert!(
        out.contains("leteo"),
        "the companion section is gone, so nothing says whether the memory is installed: {out}"
    );

    // The floor: an unconfigured agent is still listed, or the assertions above
    // would pass against a report that only prints what is installed.
    assert!(
        out.contains("codex"),
        "an agent that is not configured is no longer listed: {out}"
    );
}

#[test]
fn status_says_why_the_tools_half_is_off() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "agents"], "");
    assert!(ok, "setup failed: {stderr}");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    // A configured adapter whose server is gone: the site is known, nothing is
    // registered there, and the row must name the command that fixes it.
    std::fs::remove_file(home.path().join(".claude.json")).expect("claude's mcp config");

    let (out, _, ok) = run(home.path(), &["status"], "");
    assert!(ok, "status refused: {out}");

    assert!(
        out.contains("the agent-neutral root has no MCP client of its own"),
        "an adapter that can never register does not say why: {out}"
    );
    assert!(
        out.contains("run: estigia setup claude-code"),
        "an adapter that can register and is not registered names no command: {out}"
    );

    let (json, _, _) = run(home.path(), &["status", "--json"], "");
    let report: serde_json::Value = serde_json::from_str(&json).expect("status prints JSON");
    let rows = report["agents"].as_array().expect("the agent rows");
    let gap = |slug: &str| {
        rows.iter()
            .find(|row| row["agent"] == slug)
            .expect("the adapter is listed")["tools_gap"]
            .clone()
    };
    assert!(
        gap("agents")
            .as_str()
            .is_some_and(|reason| reason.contains("agent-neutral")),
        "the tools gap is not published: {json}"
    );
    assert!(
        gap("claude-code").is_null(),
        "an adapter that can register was handed a permanent gap: {json}"
    );
}

#[test]
fn what_the_companion_verb_prints_is_crossed() {
    // `estigia setup --companion <slug>` is a whole verb whose output nothing
    // read: replacing its one `say!` with a discard left the suite green.
    //
    // What it prints is where the ratchet is easiest to break. `leteo` is not
    // published, so the honest answer is *there is nothing to run yet* — and
    // the tempting wrong answer, `cargo install leteo`, is a command that would
    // 404. This pins the difference.
    let home = tempfile::tempdir().expect("a temporary home");
    let nowhere = tempfile::tempdir().expect("a directory with no programs in it");
    let (out, _, ok) = run_with_path(
        home.path(),
        home.path(),
        &["setup", "--companion", "leteo"],
        "",
        Some(nowhere.path()),
        &[],
    );
    assert!(ok, "the companion verb refused: {out}");
    assert!(
        out.contains("leteo — persistent memory across sessions"),
        "the companion is not named, or its purpose is not said: {out}"
    );
    assert!(
        out.contains("not found"),
        "a companion that is not on the path was not reported missing: {out}"
    );
    assert!(
        !out.contains("cargo install leteo"),
        "an unpublished companion was answered with an install that would 404: {out}"
    );

    // And a slug that is not a companion is refused by name, with the list.
    let (_, stderr, ok) = run(
        home.path(),
        &["setup", "--companion", "nonexistent-companion"],
        "",
    );
    assert!(!ok, "an unknown companion was accepted");
    assert!(
        stderr.contains("nonexistent-companion") && stderr.contains("leteo"),
        "the refusal names neither what was asked for nor what exists: {stderr}"
    );
}

#[test]
fn a_program_reading_the_json_can_tell_a_stale_skill_from_an_absent_one() {
    // `Presence` exists because "out of date" and "not there" are different
    // sentences that send an operator to different commands — its own doc says
    // so, and the text output has said which since. The JSON carried the
    // two-state `current: false` for both and dropped `presence` on the floor,
    // so the one reader that cannot ask a follow-up question got the collapsed
    // answer the enum was created to stop giving.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "the install failed: {stderr}");

    let presence_of = |home: &std::path::Path| -> String {
        let (out, _, _) = run(home, &["status", "--json"], "");
        let rows: serde_json::Value = serde_json::from_str(&out).expect("status prints JSON");
        rows["agents"]
            .as_array()
            .expect("an array of agents")
            .iter()
            .find(|row| row["agent"] == "codex")
            .expect("codex is listed")["presence"]
            .as_str()
            .expect("a presence")
            .to_owned()
    };
    assert_eq!(presence_of(home.path()), "current");

    let skill = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::remove_file(skill.join("SKILL.md")).expect("take the contract away");
    let damaged = presence_of(home.path());

    std::fs::remove_dir_all(&skill).expect("take the skill away");
    let gone = presence_of(home.path());

    assert_ne!(
        damaged, gone,
        "a skill missing one file and a skill that was never installed read \
         the same to a program, and they are fixed by different commands"
    );
    assert_eq!(gone, "absent");
}

#[test]
fn doctor_and_status_do_not_disagree_about_which_agents_are_gated() {
    // Two mechanisms, one question. `is_gated` says so in as many words — "an
    // agent gated by a plugin rather than by a settings hook... an operator
    // looking at a run that wrote without a claim needs the answer, not the
    // implementation" — and `doctor`'s gate check knew only about the settings
    // hooks. OpenCode gates through a plugin file, so it produced no wiring and
    // `doctor` said `no gate registered` about an agent `status` reported as
    // `gate on`, on the same machine, in the same minute.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "the install failed: {stderr}");

    let (out, _, _) = run(home.path(), &["status", "--json"], "");
    let rows: serde_json::Value = serde_json::from_str(&out).expect("status prints JSON");
    let gated: Vec<String> = rows["agents"]
        .as_array()
        .expect("an array")
        .iter()
        .filter(|row| row["gated"] == true)
        .map(|row| row["agent"].as_str().expect("a slug").to_owned())
        .collect();
    assert!(
        gated.iter().any(|slug| slug == "opencode"),
        "the premise is wrong: opencode is not gated on this machine"
    );

    // `doctor` writes its checks to stdout even when it ends by refusing over
    // something else — a missing `gh`, on a machine that has none.
    let (out, _, _) = run(home.path(), &["doctor", "--json"], "");
    let checks: serde_json::Value = serde_json::from_str(&out).expect("doctor prints JSON");
    for check in checks.as_array().expect("an array") {
        if check["name"] != "gate" {
            continue;
        }
        let detail = check["health"]["detail"].as_str().unwrap_or_default();
        for slug in &gated {
            if detail.starts_with(&format!("{slug}:")) {
                assert!(
                    !detail.contains("no gate registered"),
                    "status says {slug} is gated and doctor says it is not: {detail}"
                );
            }
        }
    }
}

#[test]
fn every_command_the_doctor_names_clears_the_check_that_named_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["install", "claude-code"], "");
    assert!(ok, "install failed: {stderr}");

    // Once four faults, chosen because each was answered by a different
    // command: a contract that is not this build's, a transport that is not
    // there, a stand-down record nothing can read, and a row the chosen
    // tracker does not read. Two of the four are gone now, and each left for a
    // different reason rather than being quietly forgotten: the transport is
    // retired and no longer installed, and the fourth needed a tracker with no
    // board — `Project board`, refused under it — which needed a second
    // tracker to exist at all. Linear and Trello were the only ones, and
    // removing them took the arm with it: **posing it is impossible now, not
    // merely untested**, which is the one shape of loss this corpus cannot
    // notice by staying green. What remains is the two faults nothing else
    // retired.
    let skill = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let contract = skill.join("SKILL.md");
    let held = std::fs::read_to_string(&contract).expect("the contract is installed");
    std::fs::write(&contract, format!("{held}\nnot what this build ships\n")).expect("drift");
    // The transport is retired and no longer installed, so there is nothing
    // to remove here; the contract drift above is what `sync` repairs.
    let record = home.path().join(".estigia").join("stand-down.json");
    std::fs::create_dir_all(record.parent().expect("a parent")).expect("a state directory");
    std::fs::write(&record, "{not json").expect("a record nothing can read");

    let report = |home: &std::path::Path| -> Vec<serde_json::Value> {
        let (said, _, _) = run(home, &["doctor", "--json"], "");
        serde_json::from_str(said.trim()).expect("the doctor answers JSON")
    };

    let broken: Vec<(String, String)> = report(home.path())
        .iter()
        .filter(|check| check["health"]["state"] == "broken")
        .filter_map(|check| {
            let command = check["health"]["resolution"]["command"].as_str()?;
            Some((
                check["name"].as_str().unwrap_or_default().to_owned(),
                command.to_owned(),
            ))
        })
        .collect();
    // The floor: at least two broken checks still answer with a command, so a
    // corpus that quietly stops reaching an arm fails here rather than passing
    // by posing nothing. The stand-down fault is posed too, but its lift is an
    // operator-knowledge resolution now — a person's command — so it names no
    // runnable one and is measured by `doctor`'s own unit test instead.
    assert!(
        broken.len() >= 2,
        "only {} checks named a command, so this ran almost nothing: {broken:?}",
        broken.len()
    );

    for (name, command) in &broken {
        // Quote-aware, because a resolution names the row it repairs and rows
        // have spaces in them. Split on whitespace alone, `config set "Project
        // board" none` became four words, two of them with a quote stuck to
        // them — the command ran, refused for a reason that was not the one
        // under test, and this guard would have reported the wrong fault.
        let argv: Vec<String> = shell_words(command).into_iter().skip(1).collect();
        let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
        let (said, error, ok) = run(home.path(), &argv, "");
        assert!(
            ok,
            "`{command}`, named by `{name}`, was refused: {said}{error}"
        );
    }

    let after = report(home.path());
    for (name, command) in &broken {
        let check = after
            .iter()
            .find(|check| check["name"] == name.as_str())
            .unwrap_or_else(|| panic!("`{name}` stopped being a check this build runs"));
        assert_ne!(
            check["health"]["state"], "broken",
            "`{command}` is what `{name}` told the operator to run, and `{name}` is still broken"
        );
    }
}

/// What `status` says in prose it says in JSON.
///
/// `--json` was honoured in **form** and not in **content**: the report printed
/// the agents and stopped, so a machine could not find out which runs hold
/// which issues, or which pointers cannot be read. Those are the two answers
/// this command exists to give — the first is the incident it was built for,
/// five runs that died after claiming and sat unnoticed; the second is what
/// stops a push from a checkout nothing else holds.
///
/// The age comes back twice on purpose: the seconds a program compares, and
/// the words a person reads. Publishing only the prose would be this same
/// defect facing the other way.
#[test]
fn what_status_says_in_prose_it_says_in_json() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("a state directory");
    std::fs::write(
        runs.join("claude-aaaa1111.json"),
        r#"{"run_id":"claude-aaaa1111","issue":12,"verified_at":1}"#,
    )
    .expect("a holding");
    std::fs::write(runs.join("claude-bbbb2222.json"), "{not json").expect("a torn pointer");

    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        said.contains("claude-aaaa1111") && said.contains("cannot be read"),
        "the prose stopped saying one of the two things: {said}"
    );

    let (json, _, _) = run(home.path(), &["status", "--json"], "");
    let report: serde_json::Value = serde_json::from_str(&json).expect("status prints JSON");
    let held = report["holdings"].as_array().expect("the holdings");
    assert_eq!(held.len(), 1, "the holding is not published: {json}");
    assert_eq!(held[0]["run_id"], "claude-aaaa1111");
    assert_eq!(held[0]["issue"], 12);
    assert!(
        held[0]["silent_for"].as_u64().is_some(),
        "a program has no number to compare: {json}"
    );
    assert!(
        held[0]["last_answer"]
            .as_str()
            .is_some_and(|words| words.contains("ago")),
        "a person has no words to read: {json}"
    );
    assert_eq!(
        report["unreadable_pointers"]
            .as_array()
            .expect("the unreadable pointers")
            .len(),
        1,
        "the pointer nothing can read is not published: {json}"
    );
    // And the agents are still there, under a name rather than as the whole
    // document — the one break this change makes, made once.
    assert!(
        report["agents"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty()),
        "the agents stopped being published: {json}"
    );
}

/// `status` says the tool server is dead in the same words it says the gate is.
///
/// The harness line has two halves. The gate half learned to say `REGISTERED
/// BUT DEAD`, and the argument written beside it is *somebody who reads `gate
/// on` closes the terminal*. The other half went on saying `tools on` about a
/// server naming a binary that is not there — measured by moving the path in
/// `.claude.json`, where the gate half blinked and this one did not.
///
/// `doctor` had the fact all along. It was not carried to the line an operator
/// reads, which is the same shape as the tracker verdict beside it.
#[test]
fn a_dead_tool_server_is_said_where_a_dead_gate_is_said() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    // The floor: a working install says `on`, or "it stops saying on" is a
    // sentence about a machine that never said it.
    let (before, _, _) = run(home.path(), &["status"], "");
    assert!(
        before.contains("tools on"),
        "a fresh install does not report its tool server:\n{before}"
    );

    // The registration Estigia wrote, pointed at a binary that is not there.
    let registration = home.path().join(".claude.json");
    let text = std::fs::read_to_string(&registration).expect("the registration");
    assert!(
        text.contains("estigia"),
        "the tool server is registered somewhere else now:\n{text}"
    );
    std::fs::write(
        &registration,
        text.replace("estigia.exe", "estigia-ghost.exe")
            .replace("/estigia\"", "/estigia-ghost\""),
    )
    .expect("the path is moved");

    let (after, _, _) = run(home.path(), &["status"], "");
    // Read with its wrapping flattened. `status` wraps to the terminal width and
    // indents the continuation, so a phrase this test looks for can be split
    // across two lines by nothing more than a longer path. Measured on CI, where
    // the temporary directory and the checkout are both deeper than they are on
    // a desk: `which is not\n                    there` failed a `contains` for
    // `is not there` while saying exactly the right thing to a reader.
    //
    // The sentence is the claim; where the wrapper broke it is not.
    let flattened = after.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !flattened.contains("tools on"),
        "a tool server that cannot run is still reported as on:\n{after}"
    );
    assert!(
        flattened.contains("is not there"),
        "the line does not say what is wrong with it:\n{after}"
    );
    // And it names the command that fixes it, as the gate half does.
    assert!(
        flattened.contains("estigia setup claude-code"),
        "the line names nothing to run:\n{after}"
    );
    // `doctor` agreed all along, and still does — this did not replace one
    // reader with a second that can disagree with it.
    let (checked, _, ok) = run(home.path(), &["doctor"], "");
    assert!(
        !ok && checked.contains("is not there"),
        "doctor stopped reporting the same fault:\n{checked}"
    );
}

/// `status` says the gate is standing down, in prose and in JSON.
///
/// A stand-down is bounded and forgettable by design — that is the whole point
/// of the cap — so the command an operator runs to see the state of their
/// machine is where it has to appear. It said `harness: gate on` and nothing
/// else, in either shape, while every write went through unadjudicated.
///
/// `doctor` had it all along. The module that declares stand-downs ends on the
/// sentence this broke: *what it does not do is make anything quiet. That is
/// the whole difference.*
#[test]
fn a_gate_that_is_standing_down_is_said_by_status_too() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    // The floor: an ordinary machine says nothing about one, or every operator
    // is told about a stand-down that is not there.
    let (quiet, _, _) = run(home.path(), &["status"], "");
    assert!(
        !quiet.to_lowercase().contains("standing down"),
        "a machine with no stand-down was told it has one:\n{quiet}"
    );

    let (_, _, ok) = run(
        home.path(),
        &["stand-down", "--reason", "una urgencia", "--minutes", "60"],
        "",
    );
    assert!(ok, "the stand-down was not declared");

    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        said.contains("STANDING DOWN"),
        "status does not say the gate is standing down:\n{said}"
    );
    // The three things an operator needs: how long, who, and why.
    assert!(
        said.contains("minute(s)") && said.contains("una urgencia"),
        "the line does not carry how long it lasts or what it is for:\n{said}"
    );
    assert!(
        said.contains("unadjudicated"),
        "the line does not say what it costs:\n{said}"
    );

    // And a machine reading the same command can see it. Publishing the prose
    // only would be the same defect facing the other way.
    let (json, _, _) = run(home.path(), &["--json", "status"], "");
    let root: serde_json::Value = serde_json::from_str(&json).expect("status --json is JSON");
    assert!(
        root["standing_down"]
            .as_str()
            .is_some_and(|line| line.contains("STANDING DOWN")),
        "status --json says nothing about it: {json}"
    );

    // Lifted, it stops being said — or the line outlives the thing it reports.
    run(
        home.path(),
        &["stand-down", "--lift", "--reason", "done"],
        "",
    );
    let (after, _, _) = run(home.path(), &["status"], "");
    assert!(
        !after.contains("STANDING DOWN"),
        "a lifted stand-down is still reported:\n{after}"
    );
}
