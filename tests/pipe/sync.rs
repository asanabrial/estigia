use super::*;

#[test]
fn a_sync_that_names_no_agent_says_which_root_the_substitution_moved() {
    // On the affected machine, the adapter whose files sit under
    // the dropped root stops being found, `sync` omits it, and the note that
    // would explain the omission is never printed. With no other adapter
    // installed the run refuses with `no agent has Estigia installed`, which
    // carried no note at all.
    let home = tempfile::tempdir().expect("a temporary home");
    let dropped = tempfile::tempdir().expect("a root outside the home");
    let (_, stderr, ok) = run_with_path(
        home.path(),
        home.path(),
        &["sync"],
        "",
        None,
        &[(
            "XDG_CONFIG_HOME",
            dropped.path().to_str().expect("a utf-8 path"),
        )],
    );
    assert!(
        !ok,
        "sync succeeded on a machine with nothing installed: {stderr}"
    );
    assert!(
        stderr.contains("nothing-configured"),
        "the run refused for another reason: {stderr}"
    );
    assert!(
        stderr.contains("XDG_CONFIG_HOME"),
        "the refusal does not say which root was substituted: {stderr}"
    );
    assert!(
        stderr.contains(&home.path().join(".config").display().to_string()),
        "the refusal does not say what was used instead: {stderr}"
    );
    assert!(
        stderr.contains("no agent has Estigia installed"),
        "the refusal no longer names the state it refuses: {stderr}"
    );
}

#[test]
fn a_file_estigia_wrote_and_somebody_edited_is_not_reported_as_a_strangers() {
    // `sync` writes over a payload file whose contents have moved since Estigia
    // last wrote them — which is what `sync` is *for*, and is right. What it
    // said about it was not: *"1 file(s) were already here and are not
    // Estigia's, and were written over"*, about a file `estigia setup` had
    // installed thirty seconds earlier.
    //
    // Two states share one word. `Change::Replace` is assigned both when the
    // record does not claim the path and when it claims it and the digest has
    // moved, and its own documentation names only the first: *"the file was
    // there with different contents and Estigia did **not** put it there"*.
    //
    // The reactions differ, which is why the sentence has to. A stranger's file
    // being overwritten is something the operator may not have known about; an
    // edit of their own being discarded is work they have just lost, and being
    // told it was never Estigia's file sends them looking for the wrong thing.
    // `uninstall` already tells the two apart — `Kept` against `Unrecorded` —
    // and says so on separate lines.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // Their edit, to a file Estigia put there and the record names.
    let mine = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("references")
        .join("runtime-notes.md");
    assert!(
        mine.is_file(),
        "the fixture did not install the file it edits"
    );
    let mut body = std::fs::read_to_string(&mine).expect("the file reads");
    body.push_str("\n<!-- a line the operator added -->\n");
    std::fs::write(&mine, &body).expect("their edit");

    let (out, _, ok) = run(home.path(), &["sync"], "");
    assert!(ok, "sync failed: {out}");
    // The floor: this is the path that reports an overwrite at all. Without it
    // the assertions below pass against a run that did nothing.
    assert!(
        out.contains("OVERWRITE"),
        "the fixture did not provoke an overwrite: {out}"
    );
    assert!(
        !out.contains("are not Estigia's"),
        "a file Estigia installed and the operator edited was called a stranger's: {out}"
    );
    // And it says which of the two it is, because that is what decides whether
    // the operator has lost anything.
    assert!(
        out.contains("edited"),
        "it does not say the file had been edited, which is the whole difference: {out}"
    );
}

#[test]
fn a_file_that_cannot_be_written_names_the_obstacle_and_not_a_round_trip() {
    // The ratchet, on the one path that reaches an operator whose machine says
    // no.
    //
    // A payload file that is read-only — a managed machine, an editor holding
    // it, a `chmod` somebody meant — makes `sync` refuse with
    // `setup-write-failed`, which is right, and send them to `estigia status`,
    // which is not. Running it answers *configured, skill out of date*: true,
    // and it discharges nothing. `doctor` says *not this binary's copy* and
    // names `estigia sync`, which is the command that just failed. A round trip
    // that never names the obstacle, with the obstacle — the path and the
    // operating system's own error — already in the refusal's first line.
    //
    // The arm directly above this one gets it right for a different cause: a
    // settings file that is not a JSON object answers `agent-file-not-editable`
    // and asks for the thing only a person can supply. This is the catch-all
    // that flattens.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // A file this build wants to rewrite, that the machine will not let it.
    let blocked = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("references")
        .join("runtime-notes.md");
    let mut body = std::fs::read_to_string(&blocked).expect("the file reads");
    body.push_str("\nan edit, so this build wants to write it back\n");
    std::fs::write(&blocked, &body).expect("the edit");
    let mut permissions = std::fs::metadata(&blocked).expect("the file").permissions();
    permissions.set_readonly(true);
    let Ok(()) = std::fs::set_permissions(&blocked, permissions) else {
        eprintln!("SKIPPED: this filesystem does not honour a read-only file.");
        return;
    };

    let (_, said, ok) = run(home.path(), &["sync"], "");
    // The floor: the fixture provoked the refusal it is about. A filesystem
    // that ignores the flag would leave this passing on a clean run.
    if ok {
        eprintln!("SKIPPED: the write succeeded, so this filesystem ignores read-only.");
        return;
    }
    assert!(
        said.contains("setup-write-failed"),
        "a different refusal: {said}"
    );
    // It already names the file. What it must not do is send the operator round
    // a loop: `status` reports the same *out of date* that sent them to `sync`.
    assert!(
        !said.contains("run: estigia status"),
        "it names a command that answers `skill out of date` and clears nothing, \
         which is the round trip the ratchet forbids: {said}"
    );
    assert!(
        said.contains("runtime-notes.md"),
        "it does not say which file could not be written: {said}"
    );
}

#[test]
fn what_sync_tells_a_person_it_also_tells_a_script() {
    // `sync` reports what a fresh skill root took, row by row: an
    // `Everywhere`- or `Machine`-scoped row every installed contract agrees on
    // is inherited, a row they disagree on is named and left at the default,
    // and `Agent`-scoped rows are never inherited at all. It said its note on
    // standard output, so under `--json` — where that channel has to stay
    // parseable — it could only be suppressed, and it was: the operator was
    // told and the script was told nothing.
    //
    // The fix is the channel, not the mode. A note for a person goes to
    // standard error in both, and the document on standard output stays a
    // document.
    let home = tempfile::tempdir().expect("a temporary home");
    for agent in ["codex", "claude-code", "agents"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "setup {agent} failed: {stderr}");
    }

    // Two roots that answer differently, and a third with nothing to inherit.
    // The disagreement has to be a written **contract**, not a per-agent
    // override: `installed_configs` reads `skill::contract_config`, which
    // cannot see `estigia.local.md` or an agent's own file — deliberately,
    // since promoting either into a fresh root's versioned block is the
    // defect that was named. `config set` without `--agent` propagates the
    // same value to every installed contract, and `--agent` writes the file
    // this population no longer reads, so neither reaches this fixture
    // through the CLI. Writing the row directly stands for an adapter
    // configured by an earlier build, before this binary's own writer existed
    // to keep every contract's `Change size` row in step.
    let codex_contract = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let text = std::fs::read_to_string(&codex_contract).expect("codex's own contract");
    let stock_row = "| Change size | 800 | 800 |";
    assert!(
        text.contains(stock_row),
        "the fixture's assumed row shape has drifted: {text}"
    );
    std::fs::write(
        &codex_contract,
        text.replace(stock_row, "| Change size | 400 | 800 |"),
    )
    .expect("the rewritten contract");

    let neutral = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");

    for mode in [
        vec!["sync", "--allow-source-build"],
        vec!["--json", "sync", "--allow-source-build"],
    ] {
        std::fs::remove_file(&neutral).expect("a root with no contract");
        let (out, stderr, ok) = run(home.path(), &mode, "");
        assert!(ok, "sync failed: {stderr}");
        assert!(
            stderr.contains("different configurations"),
            "{mode:?} does not say the contracts disagree: {stderr}"
        );
        assert!(
            stderr.contains("had none"),
            "{mode:?} does not name the root that inherited nothing: {stderr}"
        );
        // "had none" is common to all three branches `fresh_root_notes` can
        // print, so on its own it cannot tell which fired. This fixture's
        // branch is the disagreement one, which names the row.
        assert!(
            stderr.contains("except") && stderr.contains("Change size"),
            "{mode:?} does not name the disagreeing row: {stderr}"
        );
        if mode.contains(&"--json") {
            serde_json::from_str::<serde_json::Value>(&out)
                .expect("the document on standard output is still a document");
        }
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

fn entries_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            found.push(path.clone());
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn setup_and_sync_refuse_every_retired_agent_and_write_nothing() {
    let slugs: Vec<&str> = estigia::setup::RETIRED_AGENTS
        .iter()
        .map(|adapter| adapter.slug)
        .collect();
    assert_eq!(slugs, RETIRED_SLUGS, "a retired row was added or dropped");

    for slug in RETIRED_SLUGS {
        for command in ["setup", "sync"] {
            let home = tempfile::tempdir().expect("a temporary home");
            let before = entries_under(home.path());
            let (_, stderr, ok) = run(home.path(), &[command, slug], "");
            assert!(!ok, "{command} {slug} was accepted");
            assert!(
                stderr.contains("agent-retired"),
                "{command} {slug} refused for another reason: {stderr}"
            );
            assert!(
                stderr.contains(&format!("estigia uninstall {slug}")),
                "{command} {slug} names no way out: {stderr}"
            );
            assert_eq!(
                entries_under(home.path()),
                before,
                "{command} {slug} wrote into the home"
            );
        }

        let home = tempfile::tempdir().expect("a temporary home");
        let (_, stderr, ok) = run(home.path(), &["uninstall", slug], "");
        assert!(ok, "uninstall {slug} was refused: {stderr}");
    }
}

#[test]
fn a_sync_that_installs_a_new_contract_leaves_the_store_and_the_configuration_alone() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    for (setting, value, agent) in [
        ("Change size", "400", None),
        (
            "Model routing",
            "developer=sonnet, analyst=opus; developer=medium",
            Some("claude-code"),
        ),
    ] {
        let mut args = vec!["config", "set", setting, value];
        if let Some(agent) = agent {
            args.extend(["--agent", agent]);
        }
        let (_, stderr, ok) = run(home.path(), &args, "");
        assert!(ok, "config set {setting} failed: {stderr}");
    }
    let store = home.path().join(".estigia").join("state.db");
    std::fs::create_dir_all(store.parent().expect("the store's directory")).expect("a store root");
    let held = b"claims, tasks and receipts a run in flight still holds".to_vec();
    std::fs::write(&store, &held).expect("a store in flight");
    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let installed = std::fs::read_to_string(&contract).expect("the installed contract");
    assert!(
        installed.contains("#### FLOW"),
        "the fixture's assumed contract has drifted"
    );
    std::fs::write(
        &contract,
        installed.replace("#### FLOW", "#### The ODD protocol"),
    )
    .expect("an older contract in place");
    let (before, stderr, ok) = run(home.path(), &["config", "list"], "");
    assert!(ok, "config list failed: {stderr}");

    let (_, stderr, ok) = run(home.path(), &["sync", "--allow-source-build"], "");
    assert!(ok, "sync failed: {stderr}");
    let synced = std::fs::read_to_string(&contract).expect("the synced contract");
    assert!(
        synced.contains("#### FLOW") && !synced.contains("#### The ODD protocol"),
        "sync left the older contract installed"
    );

    assert_eq!(
        std::fs::read(&store).expect("the store after sync"),
        held,
        "sync wrote the store a run in flight reads its claim, tasks and receipts from"
    );
    let (after, stderr, ok) = run(home.path(), &["config", "list"], "");
    assert!(ok, "config list failed after sync: {stderr}");
    assert_eq!(
        before, after,
        "sync changed the configuration it was meant to keep"
    );
}
