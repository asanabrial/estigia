use super::*;

#[test]
fn a_request_the_mcp_server_cannot_answer_is_still_answered() {
    // Silence is the one reply a client cannot act on: it waits for an id it
    // did send. A batch, and the scalars that reach the same branch, used to get
    // exactly that.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (out, _, _) = run(
        home.path(),
        &["mcp"],
        "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}]
42
",
    );
    assert_eq!(
        out.lines().count(),
        2,
        "two requests, and this many answers:
{out}"
    );
    assert!(out.contains("-32600"), "{out}");
}

#[test]
fn a_configured_value_this_build_cannot_read_is_not_reported_as_a_stale_skill() {
    // Judging whether the installed contract is current takes the operator's
    // configuration — it is what the expected contract is rendered from. Three
    // callers read it with `unwrap_or_default()`, so a table carrying one
    // unrecognised value was compared against the *defaults*, did not match, and
    // was reported as **out of date**.
    //
    // That is the ratchet backwards. "Out of date" names `estigia sync`, and
    // `sync` refuses this contract with `config-value-unrecognised` and changes
    // nothing — the one thing a message is forbidden to do is name a dead end.
    // It reached three audiences: this line, the JSON beside it, and the agent
    // itself at every `SessionStart`.
    //
    // Measured on a machine, not in a unit: the defect was not in any of the
    // three renderers but in the wiring between them and the config read, and
    // each of them looked right on its own.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let installed = std::fs::read_to_string(&contract).expect("the contract is installed");
    assert!(
        installed.contains("| Tracker | github | github |"),
        "the shipped table no longer has the row this test edits"
    );

    // A value that is real, so the baseline is a configured machine and not a
    // broken one. Only the operator's column moves: the third is the shipped
    // default, and changing it makes the file genuinely differ.
    std::fs::write(
        &contract,
        installed.replace(
            "| Tracker | github | github |",
            "| Tracker | github acme/repo | github |",
        ),
    )
    .expect("the table is rewritten");
    let (configured, _, _) = run(home.path(), &["status"], "");
    assert!(
        configured.contains("claude-code    configured\n"),
        "a recognised value did not leave the machine simply configured: {configured}"
    );

    // And now one this build does not know.
    std::fs::write(
        &contract,
        installed.replace(
            "| Tracker | github | github |",
            "| Tracker | jira | github |",
        ),
    )
    .expect("the table is rewritten");

    let (text, _, _) = run(home.path(), &["status"], "");
    assert!(
        !text.contains("out of date"),
        "an unrecognised configured value is still reported as a stale skill, which sends the \
         reader to a `sync` that refuses: {text}"
    );
    assert!(
        text.contains("contract not understood"),
        "the line does not say what is actually wrong: {text}"
    );

    // The machine reader gets it too. It is the one that cannot ask a follow-up
    // question, which is why `Presence` exists as an enum at all.
    let (json, _, _) = run(home.path(), &["status", "--json"], "");
    let report: serde_json::Value = serde_json::from_str(&json).expect("status prints JSON");
    let claude = report["agents"]
        .as_array()
        .expect("an array of agents")
        .iter()
        .find(|row| row["agent"] == "claude-code")
        .expect("claude-code is listed");
    assert_eq!(
        claude["presence"], "unreadable",
        "the JSON collapsed the fourth state back into one of the other three: {claude}"
    );

    // The third audience, and the one that cannot run `status`: the agent, in
    // the one message it is guaranteed to read. It was being told the contract
    // was not this binary's copy and sent to `sync`.
    let (session, _, _) = run(
        home.path(),
        &["hook", "session-start"],
        r#"{"session_id":"abc12345"}"#,
    );
    assert!(
        !session.contains("estigia sync"),
        "the session was sent to a command that refuses this contract: {session}"
    );
    assert!(
        session.contains("does not recognise") && session.contains("estigia doctor"),
        "the session was not told the gate is running on its defaults, nor where the row is: \
         {session}"
    );

    // And `sync` really is the dead end, or the paragraph above is a story
    // rather than a reason.
    let (_, refusal, ok) = run(home.path(), &["sync"], "");
    assert!(
        !ok && refusal.contains("config-value-unrecognised"),
        "`sync` no longer refuses this contract, so \"out of date\" would have been survivable: \
         {refusal}"
    );
}

#[test]
fn an_uninstall_that_leaves_the_operators_files_says_that_is_why_the_directory_is_there() {
    // The uninstaller takes out every file it installed and nothing else, which
    // is the whole requirement — and on a machine where the operator keeps their
    // own `estigia.local.md` or their own notes in that directory, the directory
    // survives. It said nothing about that, so what an operator saw was an
    // uninstall reporting eighteen removals and a skill directory still on disk.
    //
    // The same reasoning the `kept` note already runs on: "eleven unexplained
    // `kept` lines read as an uninstall that failed". A surviving directory with
    // no line about it reads the same way, and the sentence an operator actually
    // wants — *it did not touch my things* — was the one not being said.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let mine = root.join("estigia.local.md");
    let notes = root.join("NOTAS.md");
    std::fs::write(&mine, "| Setting | Value here |\n").expect("the override is written");
    std::fs::write(&notes, "my own notes\n").expect("the notes are written");

    let (text, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");

    // First the requirement itself, because the note is only worth anything if
    // the behaviour it describes is true.
    assert!(
        mine.exists() && notes.exists(),
        "the uninstaller took files it did not install"
    );
    assert!(
        !root.join("SKILL.md").exists() && !root.join("scripts").join("github.py").exists(),
        "the uninstaller left its own files behind"
    );
    assert!(
        text.contains("are not Estigia's and were left there")
            && text.contains("estigia.local.md")
            && text.contains("NOTAS.md"),
        "the surviving directory was not accounted for: {text}"
    );

    // And nothing to say when there is nothing to say: a note that always
    // appears is one nobody reads.
    let bare = tempfile::tempdir().expect("a second temporary home");
    std::fs::create_dir_all(bare.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(bare.path(), &["setup", "claude-code"], "");
    let (quiet, _, _) = run(bare.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(
        !quiet.contains("are not Estigia's"),
        "an uninstall that left nothing behind still claimed it had: {quiet}"
    );
    assert!(
        !bare
            .path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .exists(),
        "nothing of the operator's was there and the directory still is"
    );
}

/// Every entry under `root`, as the path relative to it and its bytes, sorted — directories recorded
/// with a trailing slash and no bytes — the whole tree, so a write of a file or a directory nobody
/// named is caught rather than only the two that were.
fn snapshot(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut queue = vec![root.to_path_buf()];
    while let Some(directory) = queue.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|why| panic!("{}: {why}", directory.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|why| panic!("{}: {why}", directory.display()));
            let path = entry.path();
            let kind = entry
                .file_type()
                .unwrap_or_else(|why| panic!("{}: {why}", path.display()));
            let key = path
                .strip_prefix(root)
                .unwrap_or_else(|why| panic!("{}: {why}", path.display()))
                .display()
                .to_string();
            if kind.is_dir() {
                found.push((format!("{key}/"), Vec::new()));
                queue.push(path);
            } else {
                let bytes =
                    std::fs::read(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()));
                found.push((key, bytes));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn install_under_claude_config_dir_writes_there_and_leaves_the_home_untouched() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let elsewhere = tempfile::tempdir().expect("a relocated claude configuration");
    let claude_config_dir = elsewhere.path().display().to_string();

    // A home that already holds a Claude configuration, so "leaves the home
    // untouched" is proved by comparison rather than by the directory never
    // having existed — the two are not the same claim.
    let sentinels: Vec<(std::path::PathBuf, &[u8])> = vec![
        (
            home.path().join(".claude").join("settings.json"),
            b"{\"existing\": true}\n",
        ),
        (home.path().join(".claude.json"), b"{\"existing\": true}\n"),
    ];
    for (path, bytes) in &sentinels {
        let parent = path
            .parent()
            .unwrap_or_else(|| panic!("a parent for {path:?}"));
        std::fs::create_dir_all(parent).unwrap_or_else(|why| panic!("{parent:?}: {why}"));
        std::fs::write(path.as_path(), *bytes).unwrap_or_else(|why| panic!("{path:?}: {why}"));
    }
    let home_before = snapshot(home.path());

    let (plan, stderr, ok) = run_with_path(
        home.path(),
        home.path(),
        &["setup", "claude-code", "--dry-run"],
        "",
        None,
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.as_str())],
    );
    assert!(ok, "the plan failed: {stderr}");
    assert!(
        plan.contains(&claude_config_dir),
        "the plan did not name the relocated configuration: {plan}"
    );
    let home_claude = home.path().join(".claude").display().to_string();
    assert!(
        !plan.contains(&home_claude),
        "the plan named the home configuration instead: {plan}"
    );

    let (_, stderr, ok) = run_with_path(
        home.path(),
        home.path(),
        &["setup", "claude-code"],
        "",
        None,
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.as_str())],
    );
    assert!(ok, "setup failed: {stderr}");

    assert!(
        elsewhere.path().join("settings.json").is_file(),
        "the hooks file was not written under the relocated configuration"
    );
    assert!(
        elsewhere.path().join("CLAUDE.md").is_file(),
        "the instruction file was not written under the relocated configuration"
    );
    assert!(
        elsewhere.path().join(".claude.json").is_file(),
        "the MCP registration was not written under the relocated configuration"
    );
    assert!(
        elsewhere
            .path()
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("SKILL.md")
            .is_file(),
        "the skill was not written under the relocated configuration"
    );
    assert!(
        elsewhere
            .path()
            .join("agents")
            .join("review-blind.md")
            .is_file(),
        "the reserved reviewer was not written under the relocated configuration"
    );

    // The whole home, not the two files that were seeded: the claim is that
    // nothing under it moved, and a check of the files somebody already named
    // cannot carry that.
    let outside_estigia_state: Vec<_> = snapshot(home.path())
        .into_iter()
        .filter(|(key, _)| !key.starts_with(".estigia"))
        .collect();
    assert_eq!(
        outside_estigia_state, home_before,
        "the home was written to as well as the relocated configuration"
    );
    assert!(
        home.path().join(".estigia").join("roots").is_dir(),
        "the relocated root was not recorded where another profile's uninstall looks"
    );

    let relocated_skill = elsewhere
        .path()
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .display()
        .to_string();
    let (status, _, status_ok) = run_with_path(
        home.path(),
        home.path(),
        &["status"],
        "",
        None,
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.as_str())],
    );
    assert!(
        status_ok,
        "status failed under the variable, so its output below proves nothing: {status}"
    );
    assert!(
        status.contains("claude-code    configured\n") && status.contains(&relocated_skill),
        "status under the variable did not read the installation back from the relocated \
         configuration: {status}"
    );
    // `doctor`'s exit status is bound and asserted, but not pinned to a value:
    // its `gh` row is red in this rig by design, and a red row is a report
    // rather than a failure. What the exit status must never be is non-zero
    // with nothing named, which is what an ignored status would let through.
    let (checks, _, checks_ok) = run_with_path(
        home.path(),
        home.path(),
        &["doctor"],
        "",
        None,
        &[("CLAUDE_CONFIG_DIR", claude_config_dir.as_str())],
    );
    assert!(
        checks_ok || checks.contains("BROKEN"),
        "doctor exited non-zero without naming a broken row: {checks}"
    );
    assert!(
        checks.contains(&format!("claude-code: {relocated_skill}")),
        "doctor under the variable did not name the relocated contract: {checks}"
    );
    let (status, _, _) = run(home.path(), &["status"], "");
    assert!(
        status.contains("claude-code    not configured\n"),
        "without the variable the home is read, and nothing was installed there: {status}"
    );
}

#[test]
fn the_skill_left_for_another_agent_is_not_reported_as_the_operator_s() {
    // The note above answers *did it touch my things?*, and it answers it by
    // walking the skill directory after the removal and calling whatever is
    // still there the operator's. That premise — "everything of ours is gone by
    // now" — is true for the adapters with a root of their own, and false for
    // the ones that share `~/.agents/skills`: their skill is deliberately
    // left standing for the agents still configured, and the run says so one
    // line earlier.
    //
    // So the two notes contradicted each other in one output. Measured on this
    // machine: `estigia setup opencode --uninstall` with cursor and continue
    // still configured printed "14 file(s) are the skill ... it goes out with
    // the last one" and then "16 file(s) in that directory are not Estigia's",
    // listing `SKILL.md`, `scripts/github.py` and Estigia's own install record
    // among them. One of the sixteen was actually the operator's.
    //
    // Both halves are harm. The one sentence that exists to answer *it did not
    // touch my things* buries its real answer in fifteen files that are not the
    // operator's, and it invites them to delete a skill two configured agents
    // are reading — which is precisely what the `shared` note exists to stop.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "opencode"], "");
    assert!(ok, "setup failed: {stderr}");
    let (_, stderr, ok) = run(home.path(), &["setup", "cursor"], "");
    assert!(ok, "setup failed: {stderr}");

    let root = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let notes = root.join("NOTAS.md");
    std::fs::write(
        &notes,
        "my own notes
",
    )
    .expect("the notes are written");

    let (text, _, _) = run(home.path(), &["setup", "opencode", "--uninstall"], "");

    // The floor: this is the shared case at all, and the note under test fired.
    assert!(
        text.contains("it goes out with the last one"),
        "the skill was not left for the other agent, so nothing here is measured: {text}"
    );
    assert!(
        root.join("SKILL.md").is_file(),
        "the skill went out before the last agent that reads it"
    );
    let note = text
        .lines()
        .find(|line| line.contains("are not Estigia's and were left there"))
        .unwrap_or_else(|| panic!("the surviving directory was not accounted for: {text}"));

    assert!(
        note.contains("NOTAS.md"),
        "the operator's own file is the one thing this note exists to name: {note}"
    );
    for ours in [
        "SKILL.md",
        "github.py",
        "installed.json",
        "blind-judges.md",
        "judgment-day.md",
    ] {
        assert!(
            !note.contains(ours),
            "{ours} is Estigia's, left for another agent, and was called the operator's: {note}"
        );
    }
    assert!(
        note.contains("1 file(s)"),
        "one file there is the operator's, and the count is what they read first: {note}"
    );

    // The same premise fails a second way. With the install record deleted the
    // run prints `unknown` against every file — *no record, so nothing here is
    // shown to be Estigia's* — and this note then called those same files "not
    // Estigia's". `Kept` and `Unrecorded` are separate variants precisely
    // because a fact and the absence of one are different sentences; measured,
    // fifteen files got both in one output.
    let blank = tempfile::tempdir().expect("a third temporary home");
    std::fs::create_dir_all(blank.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(blank.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let own = blank
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::write(
        own.join("NOTAS.md"),
        "my own notes
",
    )
    .expect("the notes are written");
    std::fs::remove_file(own.join(".estigia").join("installed.json")).expect("the record goes");

    let (blind, _, _) = run(blank.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(
        blind.contains("no record of installing here"),
        "the record was not missing, so nothing here is measured: {blind}"
    );
    let blind_note = blind
        .lines()
        .find(|line| line.contains("are not Estigia's and were left there"))
        .unwrap_or_else(|| panic!("the surviving directory was not accounted for: {blind}"));
    assert!(
        blind_note.contains("NOTAS.md"),
        "the operator's own file is still the one thing to name: {blind_note}"
    );
    for unshown in ["SKILL.md", "github.py"] {
        assert!(
            !blind_note.contains(unshown),
            "{unshown} was reported as unknown and as the operator's in one output: {blind_note}"
        );
    }
}

#[test]
fn an_override_file_that_will_not_open_is_not_an_absence_of_overrides() {
    // `estigia.local.md` is where an operator narrows what the tool may do, and
    // both ways the configuration is assembled read it as
    // `read_to_string(..).ok()` — so a file that is there and will not open was
    // the same thing as a file that is not there. What ran instead was whatever
    // sat underneath: the shipped default, or, when an abandoned
    // `operator.local.md` was still lying beside it, that one's value.
    //
    // Silently. `doctor` said `ok contract` and `status` said `configured`, so
    // nothing on the machine could be asked which settings were actually in
    // force. That is the declared asymmetry backwards — configuration may only
    // tighten, and an unreadable value must never become a looser default.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let table = "| Setting | Value here | Skill default |\n|---|---|---|\n";
    let mine = root.join("estigia.local.md");
    let older = root.join("operator.local.md");

    // Readable, and honoured: the baseline, or the rest measures a machine that
    // was broken before this test touched it.
    std::fs::write(&mine, format!("{table}| Change size | 250 | 800 |\n"))
        .expect("the override is written");
    let (listed, _, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        ok && listed_value(&listed, "Change size") == "250",
        "a readable override was not honoured: {listed}"
    );

    // The older spelling still works on its own. It is the reason the fallback
    // exists, and a fix that took it out would break every installation that
    // came from issue-flow.
    std::fs::remove_file(&mine).expect("the newer one goes away");
    std::fs::write(&older, format!("{table}| Change size | 600 | 800 |\n"))
        .expect("the legacy override is written");
    let (legacy, _, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        ok && listed_value(&legacy, "Change size") == "600",
        "the older spelling stopped being read: {legacy}"
    );

    // And now the newer one is there and will not open, with that older file
    // still beside it. A directory in its place fails the read with something
    // other than `NotFound`, on every platform.
    std::fs::create_dir_all(&mine).expect("something unreadable in its place");
    let (out, refusal, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        !ok && refusal.contains("config-local-unreadable"),
        "an unreadable override was read as no override: {refusal}{out}"
    );
    assert!(
        !out.contains("600"),
        "the values underneath were served in place of the ones nobody could read: {out}"
    );

    // The two commands an operator would reach for next say it too, rather than
    // reporting a machine in good order.
    let (checks, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        checks.contains("BROKEN") && checks.contains("estigia.local.md"),
        "doctor did not name the file it could not read: {checks}"
    );
    let (state, _, _) = run(home.path(), &["status"], "");
    assert!(
        state.contains("contract not understood"),
        "status reported a configured machine: {state}"
    );

    // Absent is still ordinary. With neither file there the machine runs the
    // shipped table and says nothing, or this fix turned every installation
    // that never wrote an override into a fault.
    std::fs::remove_dir(&mine).expect("the unreadable one goes away");
    std::fs::remove_file(&older).expect("the legacy one goes away");
    let (plain, _, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        ok && listed_value(&plain, "Change size") == "800",
        "a machine with no override file at all was refused: {plain}"
    );
}

/// A fresh installation, with no `config set` at all, reads the balanced
/// posture — the shipped default, measured through `config list`
/// rather than asserted on `Config::default`.
#[test]
fn a_fresh_installation_reads_the_balanced_posture_through_config_list() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let (out, stderr, ok) = run(home.path(), &["config", "list"], "");
    assert!(ok, "config list failed: {stderr}");

    let read = |label: &str| listed_value(&out, label);
    assert_eq!(read("Authorisation posture"), "balanced");
    for label in ["Delivery authorisation", "Transition authorisation"] {
        assert_eq!(
            read(label),
            "auto",
            "a fresh installation does not read the balanced posture for {label}"
        );
    }
}

#[test]
fn a_stand_down_record_nobody_could_read_is_not_a_machine_that_had_none() {
    // The record on disk is state, not history: a later declaration writes over
    // it, and `superseded` in the ledger is the only trace of what was replaced.
    // It was filled from `read_to_string(..).ok().and_then(..ok())`, so a file
    // that was there and would not open arrived as `None` — and the ledger then
    // said `superseded: null`, which means *nothing was in force*. That is the
    // one act whose whole claim is being answerable for itself asserting
    // something it never read.
    //
    // Lifting had the mirror of it: "nothing was in force — the gate was already
    // deciding on its own", said over a file that was there, was not being
    // honoured, and has just been deleted.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    let state = home.path().join(".estigia");
    std::fs::create_dir_all(&state).expect("a state directory");
    let record = state.join("stand-down.json");

    // Declaring over one nothing can parse.
    std::fs::write(&record, "not a record").expect("a corrupt record");
    let (_, _, ok) = run(
        home.path(),
        &["stand-down", "--reason", "over a broken one"],
        "",
    );
    assert!(ok, "declaring over an unreadable record was refused");
    let written = decisions(home.path());
    let last = written.lines().last().expect("a line").to_owned();
    let entry: serde_json::Value = serde_json::from_str(&last).expect("the line is JSON");
    assert_eq!(
        entry["verdict"], "stand-down-declared",
        "the declaration did not reach the ledger: {last}"
    );
    assert!(
        !entry["superseded"].is_null(),
        "the ledger said nothing was in force about a file it never read: {last}"
    );
    assert_eq!(
        entry["superseded"]["unreadable"], true,
        "what it replaced was reported as a record it had read: {last}"
    );

    // And lifting over one.
    std::fs::write(&record, "not a record").expect("a corrupt record");
    let (text, _, ok) = run(
        home.path(),
        &["stand-down", "--lift", "--reason", "tidy"],
        "",
    );
    assert!(ok, "lifting over an unreadable record was refused");
    assert!(
        text.contains("could not be read"),
        "an unreadable record was lifted as though nothing had been there: {text}"
    );
    assert!(
        !record.exists(),
        "the unreadable record survived the lift that reported taking it away"
    );

    // A machine that genuinely has none still says so, or this turned every
    // ordinary lift into a report about a file that was never there.
    let (plain, _, _) = run(
        home.path(),
        &["stand-down", "--lift", "--reason", "again"],
        "",
    );
    assert!(
        plain.contains("nothing was in force") && !plain.contains("could not be read"),
        "a lift with no record at all claimed there had been one: {plain}"
    );
}

#[test]
fn setting_a_row_the_operator_s_local_file_shadows_is_not_reported_as_done() {
    // `config set` wrote the versioned table and said "Change size is now
    // 400". The operator's own `estigia.local.md` overrides row for row, so
    // the value they read was still `250` — a confirmation for something that
    // did not happen, in the configuration of a tool whose entire purpose is
    // refusing exactly that.
    //
    // The fix is issue-flow's own move: write, then read back what the operator
    // will read, and believe the readback.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let local = root.join("estigia.local.md");
    std::fs::write(
        &local,
        "| Setting | Value here | Skill default |
|---|---|---|
| Change size | 250 |          800 |
",
    )
    .expect("the operator's own file");

    let (_, err, ok) = run(home.path(), &["config", "set", "Change size", "400"], "");
    assert!(!ok, "a shadowed row was reported as set");
    assert!(err.contains("setting-shadowed-by-local-file"), "{err}");
    // And it names the file, because that is the only place the operator can
    // act — Estigia does not edit a file it was told not to touch.
    assert!(err.contains("estigia.local.md"), "{err}");

    // With the override gone, the ordinary path is unchanged.
    std::fs::remove_file(&local).expect("remove the override");
    let (out, _, ok) = run(home.path(), &["config", "set", "Change size", "400"], "");
    assert!(ok, "an unshadowed row stopped working");
    assert!(out.contains("400"), "{out}");
}

#[test]
fn what_config_set_writes_for_one_agent_is_what_config_list_reads_back() {
    // Most adapters share one skill root, so an adapter's own
    // answers live in a file beside the contract rather than in it. `config
    // set --agent opencode` wrote that file and said so; `config list --agent
    // opencode` read the shared table and reported the old value. A write this
    // tool confirmed and a read this tool contradicted, on the same row, one
    // command apart.
    //
    // Run as a process against a home of its own, because both commands
    // resolve their paths from the environment.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "setup --all failed: {stderr}");

    let (out, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "developer=opus",
            "--agent",
            "opencode",
        ],
        "",
    );
    assert!(ok, "the write failed: {stderr}");
    assert!(
        out.contains("developer=opus"),
        "the write said nothing: {out}"
    );

    assert_eq!(
        models_of(home.path(), "opencode"),
        "developer=opus",
        "written for opencode and read back as something else"
    );

    // And it is that agent's answer, not everybody's: the adapter beside it on
    // the same root still reads the shipped default.
    assert_eq!(
        models_of(home.path(), "cursor"),
        "unset",
        "one agent's answer moved another agent that shares its skill root"
    );
}

/// A routing key this build no longer accepts is dropped with a report, and the
/// keys that survive are kept — `config set` never rewrites the cell to `unset`
/// in silence.
#[test]
fn config_set_reports_a_routing_key_this_build_no_longer_accepts() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let installed = std::fs::read_to_string(&contract).expect("the contract is installed");
    let row = installed
        .lines()
        .find(|line| line.starts_with("| Model routing |"))
        .unwrap_or_else(|| panic!("the installed contract has no Model routing row:\n{installed}"));
    let replaced = installed.replace(
        row,
        "| Model routing | implementer=opus, reviewer=sonnet, auditor=haiku | unset |",
    );
    assert_ne!(replaced, installed, "the row was not replaced");
    std::fs::write(&contract, replaced).expect("the contract is rewritten");

    // `config set` on another row reads the table tolerantly, drops the two
    // retired keys, keeps `reviewer`, and prints what it dropped.
    let (out, stderr, ok) = run(home.path(), &["config", "set", "Change size", "120"], "");
    assert!(ok, "the write failed: {stderr}");
    assert!(
        out.contains("Model routing"),
        "the report does not name the row: {out}"
    );
    assert!(
        out.contains("implementer") && out.contains("auditor"),
        "the dropped keys were not reported: {out}"
    );

    // The surviving key is kept, and the dropped ones are gone from the cell.
    let after = std::fs::read_to_string(&contract).expect("the contract reads");
    let row = after
        .lines()
        .find(|line| line.starts_with("| Model routing |"))
        .unwrap_or_else(|| panic!("the Model routing row was lost:\n{after}"));
    assert!(
        row.contains("reviewer=sonnet"),
        "the surviving key was lost: {row}"
    );
    assert!(
        !row.contains("implementer"),
        "the renamed key survived: {row}"
    );
    assert!(!row.contains("auditor"), "the retired key survived: {row}");
}

/// An upgrade folds the retired `Delegated workers` row rather than refusing it.
///
/// `setup` is the command an operator runs to move to this release, and it is
/// the only command that rewrites the table the row lives in. Refusing that row
/// made the upgrade unable to clear the state it exists for, and — because the
/// refusal came before any write — left the row where it was, so every later run
/// refused identically. Reported by an operator whose `setup --all` failed on
/// exactly this after the `Delegated workers` removal shipped.
#[test]
fn an_upgrade_folds_the_retired_delegated_workers_row_instead_of_refusing_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let installed = std::fs::read_to_string(&contract).expect("the contract is installed");
    let row = installed
        .lines()
        .find(|line| line.starts_with("| Model routing |"))
        .unwrap_or_else(|| panic!("the installed contract has no Model routing row:\n{installed}"));
    // The earlier shape an installation from before the restructure carries: the
    // routing row gone, the retired row back with both roles named.
    let doctored = installed.replace(row, "| Delegated workers | developer analyst | none |");
    assert_ne!(doctored, installed, "the row was not replaced");
    std::fs::write(&contract, doctored).expect("the contract is doctored");

    // The upgrade the row used to block completes, and says what it folded.
    let (out, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "the upgrade refused the retired row: {stderr}");
    assert!(
        out.contains("Delegated workers"),
        "the fold was not reported:\n{out}"
    );

    // The row is out of the table and the roles it named are installed by the
    // routing, which is what "presence installs it" means now.
    let written = std::fs::read_to_string(&contract).expect("the contract reads");
    assert!(
        !written.contains("| Delegated workers |"),
        "the retired row survived the upgrade:\n{written}"
    );
    let row = written
        .lines()
        .find(|line| line.starts_with("| Model routing |"))
        .unwrap_or_else(|| panic!("the upgrade lost the routing row:\n{written}"));
    assert!(
        row.contains("developer=inherit") && row.contains("analyst=inherit"),
        "the folded roles are not in the routing: {row}"
    );
}

#[test]
fn setting_a_row_the_operator_s_own_file_overrides_is_refused_for_an_agent_too() {
    // The shared table checks this and refuses; the per-agent file returned on
    // the strength of the write alone. So `config set --agent` reported a row
    // as now in force while every run went on reading `estigia.local.md` —
    // which sits *above* the per-agent file by design, making this the likelier
    // path rather than the rarer one.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "setup --all failed: {stderr}");

    let local = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.local.md");
    std::fs::write(
        &local,
        "# local\n\n<!-- estigia:config:start -->\n\
         | Setting | Value here | Skill default |\n|---|---|---|\n\
         | Model routing | developer=opus | unset |\n<!-- estigia:config:end -->\n",
    )
    .expect("the operator writes their own file");

    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "developer=haiku",
            "--agent",
            "opencode",
        ],
        "",
    );
    assert!(
        !ok,
        "a value nobody will ever read was reported as now in force"
    );
    assert!(
        stderr.contains("setting-shadowed-by-local-file"),
        "refused for the wrong reason: {stderr}"
    );
}

#[test]
fn a_repository_wide_setting_is_refused_for_one_agent() {
    // `Tracker` is a fact about the repository, and the gate reads it without
    // asking which agent is holding the tools. Written into one adapter's file
    // it was reported as set and read back as set, and every decision that
    // consults it went on reading the shared row — and once the agent is told
    // to read its own file, the agent and the gate disagree about which
    // tracker they are talking to.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "setup --all failed: {stderr}");

    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Tracker",
            "github acme/repo",
            "--agent",
            "opencode",
        ],
        "",
    );
    assert!(!ok, "a repository-wide row was written for one agent");
    assert!(
        stderr.contains("setting-not-per-agent"),
        "refused for the wrong reason: {stderr}"
    );
    // And it says how to do the thing that does work, which is the only reason
    // to be told at all.
    assert!(
        stderr.contains("estigia config set"),
        "refused without naming the command that would work: {stderr}"
    );

    // A setting that does differ by agent is still written.
    let (out, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Delivery authorisation",
            "ask",
            "--agent",
            "opencode",
        ],
        "",
    );
    assert!(ok, "an agent-scoped row was refused too: {stderr}");
    assert!(out.contains("opencode"), "written somewhere else: {out}");
}

#[test]
fn a_contract_that_will_not_parse_is_not_reported_as_one_that_is_not_there() {
    // One bad row in the operator's own file made `config list` answer `no
    // agent has Estigia installed` and send them to `estigia setup --all` —
    // which reinstalls a skill that was never missing and cannot touch their
    // file. The ratchet, broken: a message named a command that does not
    // discharge the block.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "setup failed: {stderr}");

    let local = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.local.md");
    std::fs::write(
        &local,
        "<!-- estigia:config:start -->\n| Setting | Value here |\n|---|---|\n\
         | Integration | octopus |\n<!-- estigia:config:end -->\n",
    )
    .expect("the operator writes a row nobody can read");

    let (_, stderr, ok) = run(home.path(), &["config", "list", "--agent", "codex"], "");
    assert!(!ok, "a contract that will not parse was read anyway");
    assert!(
        stderr.contains("config-value-unrecognised"),
        "the wrong fault: {stderr}"
    );
    assert!(
        !stderr.contains("nothing-configured"),
        "an installed agent was reported as not installed: {stderr}"
    );
    // And it points at the file rather than at a command that cannot help.
    assert!(
        stderr.contains("estigia.local.md"),
        "it does not say which file: {stderr}"
    );
    assert!(
        !stderr.contains("estigia setup --all"),
        "it names a command that reinstalls what was never missing: {stderr}"
    );
}

#[test]
fn a_named_agent_whose_contract_is_gone_is_not_a_machine_with_nothing_on_it() {
    // The other half of the entry above, and it kept the defect that one closed.
    //
    // `contract_of` walks past `skill-not-installed` because with no agent named
    // it is trying every configured adapter and the next may be installed where
    // this one is not. With an agent **named** there is no next one: the walk
    // falls off the end and answers `nothing-configured` — *no agent has Estigia
    // installed* — about a machine where that agent is registered, another agent
    // is installed and working, and the only thing missing is one file whose
    // path was in hand two lines earlier.
    //
    // Three commands, one machine, measured: `estigia status` says
    // *claude-code: configured, contract not understood*, `estigia doctor` says
    // *nothing at …/SKILL.md → run: estigia setup claude-code*, and
    // `config list --agent claude-code` said nothing is installed anywhere and
    // sent the operator to `estigia setup --all`. The first two name the agent
    // and the file. The third denied the premise and named the widest command
    // there is.
    let home = tempfile::tempdir().expect("a temporary home");
    for agent in ["codex", "claude-code"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "setup {agent} failed: {stderr}");
    }
    // A second agent that is intact, so "nothing is installed" is false about
    // the machine as well as about the one that was asked for.
    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    std::fs::remove_file(&contract).expect("the contract goes");

    let (_, stderr, ok) = run(
        home.path(),
        &["config", "list", "--agent", "claude-code"],
        "",
    );
    assert!(!ok, "a missing contract was read anyway: {stderr}");
    assert!(
        !stderr.contains("nothing-configured"),
        "a registered agent on a machine with a working install was reported as          nothing being installed anywhere: {stderr}"
    );
    // The file it is about, because that is what tells the operator whether
    // this is theirs to put back or Estigia's to rewrite.
    assert!(
        stderr.contains("SKILL.md"),
        "it does not say what is missing: {stderr}"
    );
    // And the command that discharges it, which is the narrow one.
    assert!(
        stderr.contains("estigia setup claude-code"),
        "it does not name the agent whose contract is gone: {stderr}"
    );
    assert!(
        !stderr.contains("estigia install"),
        "`estigia install` is an alias for `setup` and refuses without an agent,          so naming it is naming a dead end: {stderr}"
    );
}

#[test]
fn a_payload_that_names_no_tool_is_a_silence_and_not_a_quiet_success() {
    // The worst shape of the failure the ledger exists for, and it left no
    // trace at all.
    //
    // A `pre-tool-use` payload that is not JSON is recorded as
    // `payload-unreadable`, and one that never arrives as `payload-absent` —
    // both because a payload the gate cannot use is a call that went through
    // undecided, and the operator has to be able to find out. A payload that
    // **parses** and names no tool is exactly as unusable and was recorded as
    // nothing.
    //
    // Which is what a client version bump looks like: send `toolName` where
    // this reads `tool_name` and every call passes ungated, forever, while
    // `estigia doctor` answers *"no call has reached the gate yet — there is no
    // ledger to read"*. Not "the gate let things through" — "the gate has never
    // been called". Measured on the binary, three payloads, zero lines.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    for payload in [
        // No key at all.
        r#"{"session_id":"abc123","tool_input":{"file_path":"a.txt"}}"#,
        // The key, empty.
        r#"{"session_id":"abc123","tool_name":"","tool_input":{}}"#,
        // The key, renamed — the version bump.
        r#"{"session_id":"abc123","toolName":"Write","tool_input":{}}"#,
    ] {
        let (out, _, ok) = run(home.path(), &["hook", "pre-tool-use"], payload);
        // Rule 3 is untouched: a hook that cannot read its payload does not
        // deny. What changes is whether anybody can find out afterwards.
        assert!(ok, "a payload it could not use denied the call: {out}");
    }

    let lines = decisions(home.path());
    let recorded = lines.lines().filter(|line| !line.trim().is_empty()).count();
    assert_eq!(
        recorded, 3,
        "three calls reached the gate with a payload naming no tool and {recorded} were \
         recorded, so nothing on this machine can say they happened: {lines}"
    );

    // And the report says so. Twice over, because the first cut of this fix
    // recorded the calls and left `doctor` filtering on a list of verdicts that
    // did not include the new one — so three records saying *not gated* sat
    // under a row answering **every call the ledger records was decided on**,
    // which is a worse sentence than the one it replaced.
    let (out, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !out.contains("no call has reached the gate yet"),
        "three calls reached it: {out}"
    );
    assert!(
        !out.contains("every call the ledger records was decided on"),
        "three of them were not: {out}"
    );
    // And which repair it is, which is the whole reason the verdicts are apart:
    // this one is an agent sending a shape this build does not read.
    assert!(
        out.contains("names no tool"),
        "it does not say what went wrong, so the operator cannot act on it: {out}"
    );

    // A session hook is not one of these. It carries no tool by design, and
    // counting it here is the defect `Event::decides` was written to end.
    let before = decisions(home.path());
    let (_, _, ok) = run(
        home.path(),
        &["hook", "session-end"],
        r#"{"session_id":"abc123"}"#,
    );
    assert!(ok);
    assert_eq!(
        decisions(home.path()),
        before,
        "a session hook, which decides nothing, was recorded as a call that went through"
    );
}

#[test]
fn a_push_the_guard_could_not_check_says_so_rather_than_passing_in_silence() {
    // `estigia guard` writes a hook into a repository whether or not there is a
    // harness for it to decide with, and it will say `wrote …/pre-push` either
    // way. In a home where nothing is installed, every push through that hook
    // then leaves: exit `0`, nothing on either stream, nothing in the ledger —
    // and `doctor` prints no push-guard row at all, because it stops at the
    // missing skill. Three surfaces, no signal. The operator installed a guard
    // and has no way to learn it decides nothing.
    //
    // The branch above this one already gets it right. A working directory the
    // process cannot read prints a line and lets the push through, with the
    // reason written beside it: *"not blocking every push in a repository is
    // the stated stance at this boundary. Doing it without a word is not the
    // same stance — it is the silence the ledger check exists to find."* The
    // next branch down did it without a word.
    //
    // Still let through. What changes is that the person typing `git push` is
    // told the guard did not check it.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repository = tempfile::tempdir().expect("a repository");
    let git = |arguments: &[&str]| checked_git(repository.path(), arguments);
    let _ = git(&["init", "--quiet"])
        && git(&["config", "user.email", "push@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);

    // The floor: the guard really is installed, so what follows is about a hook
    // that exists rather than about one that was never written.
    let (out, _, ok) = run_in(home.path(), repository.path(), &["guard"], "");
    assert!(
        ok && out.contains("pre-push"),
        "the guard did not install: {out}"
    );

    // Nothing is configured in this home, so the gate has no contract to read.
    let (out, stderr, ok) = run_in(home.path(), repository.path(), &["hook", "pre-push"], "");
    assert!(
        ok,
        "a push was blocked because the harness was missing, which is the one \
         thing this boundary must not do: {stderr}"
    );
    assert!(
        !stderr.trim().is_empty() || !out.trim().is_empty(),
        "the push went through with nothing said on either stream, so nobody can \
         learn the guard did not check it"
    );
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("not checked") || said.contains("was not checked"),
        "it does not say the push went unchecked: {said}"
    );
    // And what to do about it, because there is a command that discharges this.
    assert!(
        said.contains("estigia setup"),
        "it does not name what would make the guard decide again: {said}"
    );
}

#[test]
fn a_read_only_tracker_without_an_assisted_workflow_is_said_to_adjudicate_nothing() {
    // The repository's own file may carry `Tracker linear` without `Workflow:
    // assisted` only by hand — `config set` refuses the pair — but that state
    // reaches the gate all the same. `Tracker::adjudicates_unclaimed_writes` is
    // false there, so the guard refuses nothing. It used to say nothing about
    // that: `inert_tracker` excluded every read-only tracker, so the push went
    // out with no word and `estigia guard` still promised the refusal.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repository = tempfile::tempdir().expect("a repository");
    let git = |arguments: &[&str]| checked_git(repository.path(), arguments);
    let _ = git(&["init", "--quiet"])
        && git(&["config", "user.email", "linear@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);

    let (_, stderr, ok) = run_in(
        home.path(),
        repository.path(),
        &["setup", "claude-code"],
        "",
    );
    assert!(ok, "setup failed: {stderr}");

    let path = estigia::skill::repository_config_path(repository.path());
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    std::fs::write(
        &path,
        "| Setting | Value here | Skill default |\n|---|---|---|\n| Tracker | linear ENG | github |\n",
    )
    .expect("the repository file");

    // `guard` installs and, rather than promising a refusal it will not make,
    // says it adjudicates nothing here and names how to change that.
    let (out, stderr, ok) = run_in(home.path(), repository.path(), &["guard"], "");
    assert!(ok, "the guard did not install: {stderr}");
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("adjudicates nothing"),
        "the guard promised a refusal it does not make: {said}"
    );
    assert!(
        said.contains("estigia config set Workflow assisted --repo"),
        "the guard names no way out: {said}"
    );
    assert!(
        !said.contains("with no claim on a checkout, this refuses every push"),
        "the guard still promises the refusal unconditionally: {said}"
    );

    // The machine surface names it too, so a `--json` consumer is not handed a
    // caveat with no way out. A guard already current shows no caveats at all,
    // so the install is taken out first to reach the writing run again.
    let (_, stderr, ok) = run_in(
        home.path(),
        repository.path(),
        &["guard", "--uninstall"],
        "",
    );
    assert!(ok, "the guard did not come out: {stderr}");
    let (out, stderr, ok) = run_in(home.path(), repository.path(), &["guard", "--json"], "");
    assert!(ok, "the guard did not install: {stderr}");
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("estigia config set Workflow assisted --repo"),
        "the json surface names no way out: {said}"
    );

    // And the push itself: it goes through, and says the guard adjudicated
    // nothing rather than passing in silence.
    let (out, stderr, ok) = run_in(home.path(), repository.path(), &["hook", "pre-push"], "");
    assert!(ok, "the push was blocked: {stderr}");
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("adjudicates nothing"),
        "the push went out with no word that the guard did not check it: {said}"
    );
}

#[test]
fn an_invocation_this_build_cannot_read_is_not_a_refusal() {
    // The failure `run()`'s own comment names, arriving through the code that
    // comment chose.
    //
    // It reasoned about this exactly once and stopped one step short: clap's
    // own usage code is `2`, and `2` is the one code this crate says must never
    // mean anything else, so a mistyped invocation would have told a caller the
    // world may have changed. It moved to `1` — *nothing was written, and
    // nothing was attempted* — which is true and is not what `1` means to the
    // things that read it. Every hook Estigia writes treats `1` and `2` as
    // **decisions**: `guard::script` says so in those words, and propagates
    // them.
    //
    // So a hook file left from another build, passing a flag this one does not
    // take, blocks every push in the repository with a clap usage message —
    // which is the failure that script exists to prevent. Measured: `git push`
    // came back `error: unexpected argument …` and exit 1.
    //
    // A usage error is not a decision. It belongs in the space both readers
    // already handle — anything outside `0`, `1` and `2` — where the push goes
    // through **and is told it went unchecked**.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repository = tempfile::tempdir().expect("a repository");
    let bare = tempfile::tempdir().expect("a remote");
    let git = |at: &std::path::Path, arguments: &[&str]| checked_git(at, arguments);
    let _ = git(bare.path(), &["init", "--quiet", "--bare"])
        && git(repository.path(), &["init", "--quiet"])
        && git(
            repository.path(),
            &["config", "user.email", "skew@estigia.test"],
        )
        && git(repository.path(), &["config", "user.name", "estigia"])
        && git(
            repository.path(),
            &["commit", "--quiet", "--allow-empty", "-m", "a commit"],
        )
        && git(
            repository.path(),
            &[
                "remote",
                "add",
                "origin",
                &bare.path().display().to_string(),
            ],
        );

    run(home.path(), &["setup", "claude-code"], "");
    let (out, _, ok) = run_in(home.path(), repository.path(), &["guard"], "");
    assert!(
        ok && out.contains("pre-push"),
        "the guard did not install: {out}"
    );

    // The skew: a hook written by a build whose `hook` took one more flag.
    let hook = repository
        .path()
        .join(".git")
        .join("hooks")
        .join("pre-push");
    let script = std::fs::read_to_string(&hook).expect("the hook reads");
    assert!(
        script.contains("hook pre-push"),
        "the hook does not call the event this ages: {script}"
    );
    std::fs::write(
        &hook,
        script.replace("hook pre-push", "hook pre-push --from-a-newer-build v2"),
    )
    .expect("the aged hook");

    let pushed = std::process::Command::new("git")
        .arg("-C")
        .arg(repository.path())
        .args(["push", "origin", "HEAD:refs/heads/main"])
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .output()
        .expect("git runs");
    let said = String::from_utf8_lossy(&pushed.stderr).into_owned();
    assert!(
        pushed.status.success(),
        "a hook from another build blocked the push with a usage error, which is the \
         failure that script exists to prevent: {said}"
    );
    // And it is not silent about it: the stance is letting the push through,
    // not letting it through without a word.
    assert!(
        said.contains("did not answer"),
        "the push went out unchecked and nothing said so: {said}"
    );
}

#[test]
fn config_set_agent_preserves_an_override_that_is_not_utf8() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "opencode"], "");
    assert!(ok, "setup failed: {stderr}");
    let override_file = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.opencode.md");
    let invalid = [0xff, 0xfe, 0xfd];
    std::fs::write(&override_file, invalid).expect("the invalid override is written");

    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Delivery authorisation",
            "ask",
            "--agent",
            "opencode",
        ],
        "",
    );

    assert!(!ok, "an unreadable override was replaced: {stderr}");
    assert!(
        stderr.contains("config-local-unreadable")
            && stderr.contains(&override_file.display().to_string())
            && stderr.contains("readable, or moved aside"),
        "the read refusal did not name its cause, path, and resolution: {stderr}"
    );
    assert_eq!(
        std::fs::read(override_file).expect("the override still exists"),
        invalid,
        "config set replaced bytes it could not read"
    );
}

#[test]
fn config_set_repo_preserves_a_repository_document_that_is_not_utf8() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repository = tempfile::tempdir().expect("a repository");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    std::fs::create_dir(repository.path().join(".git")).expect("the checkout marker exists");
    let repository_file = estigia::skill::repository_config_path(repository.path());
    std::fs::create_dir_all(
        repository_file
            .parent()
            .expect("the repository document has a parent"),
    )
    .expect("the repository configuration directory exists");
    let invalid = [0xff, 0xfe, 0xfd];
    std::fs::write(&repository_file, invalid).expect("the invalid repository document is written");

    let (_, stderr, ok) = run_in(
        home.path(),
        repository.path(),
        &["config", "set", "Issue requirement", "required", "--repo"],
        "",
    );

    assert!(
        !ok,
        "an unreadable repository document was replaced: {stderr}"
    );
    assert!(
        stderr.contains("config-local-unreadable")
            && stderr.contains(&repository_file.display().to_string()),
        "the read refusal did not name its cause and path: {stderr}"
    );
    assert_eq!(
        std::fs::read(repository_file).expect("the repository document still exists"),
        invalid,
        "config set --repo replaced bytes it could not read"
    );
}

#[test]
fn a_stand_down_record_that_cannot_be_written_names_the_obstacle() {
    // The same dead end as `setup-write-failed`, one command over, found by
    // walking every `Resolution::run` in the crate and asking of each whether
    // running it clears what produced it.
    //
    // `stand-down-unwritable` says *could not write …/stand-down.json: access
    // denied* and names `estigia doctor`. `doctor` reports; it writes nothing
    // and fixes nothing — and in this state it does not even mention the file,
    // because the record is still perfectly **readable**, so the stand-down row
    // answers about the window that is in force. The operator reads a correct
    // report about something else and is no closer.
    //
    // What discharges it is a person freeing the file, which is what the
    // refusal now asks for, with the path it already had.
    //
    // Not a trap, and that was measured too: `--lift` removes the record rather
    // than rewriting it, so a stand-down declared before the file was locked
    // can still be taken off.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let (_, said, ok) = run(
        home.path(),
        &["stand-down", "--reason", "a fixture", "--minutes", "5"],
        "",
    );
    assert!(ok, "the first stand-down failed: {said}");

    let record = home.path().join(".estigia").join("stand-down.json");
    let mut permissions = std::fs::metadata(&record)
        .expect("the record")
        .permissions();
    permissions.set_readonly(true);
    let Ok(()) = std::fs::set_permissions(&record, permissions) else {
        eprintln!("SKIPPED: this filesystem does not honour a read-only file.");
        return;
    };

    let (_, said, ok) = run(
        home.path(),
        &["stand-down", "--reason", "another", "--minutes", "9"],
        "",
    );
    if ok {
        eprintln!("SKIPPED: the write succeeded, so this filesystem ignores read-only.");
        return;
    }
    // The floor: this is the refusal the test is about.
    assert!(
        said.contains("stand-down-unwritable"),
        "a different refusal: {said}"
    );
    assert!(
        !said.contains("run: estigia doctor"),
        "it names a command that reports and fixes nothing, and does not even mention \
         this file: {said}"
    );
    assert!(
        said.contains("stand-down.json"),
        "it does not say which file could not be written: {said}"
    );
}

#[test]
fn a_worktree_answers_what_its_repository_answers() {
    // This crate's own delivery topology works in worktrees, and a linked
    // worktree's `.git` is a *file*. Joining `.git/estigia/...` onto it gave a
    // path nothing could be written under, and two things followed: `config set
    // --repo` refused with *os error 3* and sent the operator to `estigia
    // doctor`, which fixes nothing about it; and the same repository answered
    // one value in the checkout and another inside its own worktree — silently,
    // because the file the worktree looked for could not exist.
    //
    // A declared boundary is about a repository. It has to hold wherever that
    // repository is being worked on.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let outside = tempfile::tempdir().expect("somewhere to keep checkouts");
    let repo = outside.path().join("main");
    std::fs::create_dir_all(&repo).expect("the checkout");
    let git = |at: &std::path::Path, arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(at)
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let worktree = outside.path().join("wt");
    if !(git(&repo, &["init", "--quiet"])
        && git(&repo, &["config", "user.email", "worktree@estigia.test"])
        && git(&repo, &["config", "user.name", "estigia"])
        && git(
            &repo,
            &["commit", "--quiet", "--allow-empty", "-m", "a commit"],
        )
        && git(
            &repo,
            &[
                "worktree",
                "add",
                "--quiet",
                worktree.to_str().expect("a printable path"),
                "-b",
                "a-branch",
            ],
        ))
    {
        eprintln!("SKIPPED: git is not usable here, so the worktree was not crossed.");
        return;
    }

    let (_, stderr, ok) = run_in(
        home.path(),
        &repo,
        &["config", "set", "Change size", "250", "--repo"],
        "",
    );
    assert!(ok, "the checkout refused its own row: {stderr}");

    let answer = |at: &std::path::Path| {
        let (out, _, ok) = run_in(home.path(), at, &["config", "list", "--json"], "");
        assert!(ok, "config list refused in {}: {out}", at.display());
        let read: serde_json::Value = serde_json::from_str(&out).expect("config list is json");
        read.to_string()
    };
    assert_eq!(
        answer(&worktree),
        answer(&repo),
        "the same repository answers one way in the checkout and another in its own worktree"
    );

    // And the row can be set from inside the worktree, rather than refused with
    // a write error and a command that would not have helped.
    let (_, stderr, ok) = run_in(
        home.path(),
        &worktree,
        &["config", "set", "Change size", "600", "--repo"],
        "",
    );
    assert!(
        ok,
        "the worktree could not set its repository's row: {stderr}"
    );
    assert!(
        answer(&repo).contains("600"),
        "a row set in the worktree did not reach the repository"
    );
}

#[test]
fn a_remote_with_a_space_in_it_is_named_whole() {
    // A path with a space in it is ordinary — `C:\Users\Antonio Sanabria` is a
    // Windows home directory — and `git remote -v` writes the URL after a tab,
    // not after the name and a space. Reading it at the first space made the
    // row answer *which repository holds the issues* with `../el`.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let outside = tempfile::tempdir().expect("somewhere to keep checkouts");
    let repo = outside.path().join("un repo con espacios");
    let remote = outside.path().join("el remoto con espacios.git");
    std::fs::create_dir_all(&repo).expect("the checkout");
    let git = |at: &std::path::Path, arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(at)
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let ready = std::process::Command::new("git")
        .arg("init")
        .arg("--quiet")
        .arg("--bare")
        .arg(&remote)
        .output()
        .is_ok_and(|out| out.status.success())
        && git(&repo, &["init", "--quiet"])
        && git(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                remote.to_str().expect("a printable path"),
            ],
        );
    if !ready {
        eprintln!("SKIPPED: git is not usable here, so the remote was not crossed.");
        return;
    }

    let (out, _, _) = run_in(home.path(), &repo, &["doctor", "--json"], "");
    let read: serde_json::Value = serde_json::from_str(&out).expect("doctor --json is json");
    let detail = read
        .as_array()
        .expect("the checks are a list")
        .iter()
        .find(|check| check["name"] == "remote")
        .expect("the remote is checked")["health"]["detail"]
        .as_str()
        .expect("the detail is text")
        .to_owned();
    assert_eq!(
        detail,
        remote.display().to_string(),
        "the remote row names something other than the remote git was given"
    );
}

#[test]
fn somebody_elses_hook_is_not_this_agents_gate() {
    // `gates_writes` answers *is Estigia's gate registered for this agent*, and
    // `status` and `doctor`'s `gate` row both print what it says. Measured by
    // mutation: making `is_estigia_hook` answer yes to every entry left the
    // whole suite green — because the only file anything crossed it against
    // held Estigia's own hook or held nothing at all.
    //
    // Between those two there is an ordinary machine: an operator with their
    // own `PreToolUse` hook and no Estigia entry beside it, which is what a
    // settings file looks like before `setup` and after somebody takes the
    // entry out by hand. There, a yes is the crate's cardinal failure — `gate
    // on` printed over no gate, and a `doctor` that agrees with it.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // The floor first: with Estigia's own hook in place, this says so. An
    // assertion on the absence of `gate on` would otherwise pass against a
    // `status` that never says it at all.
    let (out, _, _) = run(home.path(), &["status"], "");
    assert!(
        out.contains("gate on"),
        "a freshly registered gate is not reported on: {out}"
    );

    // Now the operator's own hook, alone in the file.
    let settings = home.path().join(".claude").join("settings.json");
    std::fs::write(
        &settings,
        r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "mi-propio-hook"}]}]}}"#,
    )
    .expect("the operator's own settings");

    let (out, _, _) = run(home.path(), &["status"], "");
    assert!(
        out.contains("gate off"),
        "somebody else's hook is reported as this agent's gate: {out}"
    );

    let (out, _, _) = run(home.path(), &["doctor", "--json"], "");
    let read: serde_json::Value = serde_json::from_str(&out).expect("doctor --json is json");
    let gate = read
        .as_array()
        .expect("the checks are a list")
        .iter()
        .find(|check| check["name"] == "gate")
        .expect("the gate is checked")["health"]
        .clone();
    assert_ne!(
        gate["state"], "fine",
        "doctor calls somebody else's hook a registered gate: {gate}"
    );
}

#[test]
fn a_toml_table_codex_reads_is_one_estigia_reads() {
    // `[mcp_servers."estigia"]` is the same table as `[mcp_servers.estigia]`,
    // and Codex reads both. `setup::render` knew that — it writes the section
    // and lifts it back out, and its own predicate lists both spellings with a
    // note saying the two questions *must not drift apart*. Three readers one
    // module over spelled the bare form inline and knew nothing of the other.
    //
    // Measured through the binary: with the quoted spelling in `config.toml`,
    // `status` printed `tools off` and `doctor` said *no tool server
    // registered*, over a server that was registered and would have started.
    // The uninstall, which reads through `render`, took the same section out
    // correctly — one fact, four readers, three of them narrower.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "setup failed: {stderr}");

    let config = home.path().join(".codex").join("config.toml");
    let written = std::fs::read_to_string(&config).expect("codex's config reads");
    assert!(
        written.contains("[mcp_servers.estigia]"),
        "the table was not written where this test edits it:\n{written}"
    );
    std::fs::write(
        &config,
        written.replace("[mcp_servers.estigia]", "[mcp_servers.\"estigia\"]"),
    )
    .expect("the operator's own spelling");

    let (out, _, _) = run(home.path(), &["status"], "");
    assert!(
        out.contains("gate on, tools on"),
        "a registered server spelled the other way is reported off: {out}"
    );

    let (out, _, _) = run(home.path(), &["doctor", "--json"], "");
    let read: serde_json::Value = serde_json::from_str(&out).expect("doctor --json is json");
    let tools = read
        .as_array()
        .expect("the checks are a list")
        .iter()
        .find(|check| check["name"] == "tools")
        .expect("the tools row is checked")["health"]
        .clone();
    assert_eq!(
        tools["state"], "fine",
        "doctor does not see a server Codex would start: {tools}"
    );
    // And it names the binary, which is the other reader that found the table
    // by spelling it: a row that says `registered` and cannot say what it runs
    // is the half-answer this crate keeps finding.
    assert!(
        tools["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("estigia")),
        "the row does not say which binary the entry names: {tools}"
    );

    // The floor: a file with only a *sub-table* of Estigia's registers nothing,
    // or the assertions above would pass on the mere mention of the name.
    std::fs::write(
        &config,
        "model = \"gpt-5\"\n\n[mcp_servers.estigia.env]\nSOMETHING = \"1\"\n",
    )
    .expect("a sub-table and no table");
    let (out, _, _) = run(home.path(), &["status"], "");
    assert!(
        out.contains("tools off"),
        "a sub-table with no server under it is reported as a registered server: {out}"
    );
}

#[test]
fn an_uninstall_with_nothing_to_take_does_not_say_the_install_is_current() {
    // `describe` is shared by `setup`, `sync` and `uninstall`, and its
    // nothing-happened sentence was `already current` for all three. That is a
    // claim about an installation being up to date, printed by the command
    // whose whole job is to take one away: an operator running `uninstall`
    // twice — or once on a machine Estigia had already left — was told their
    // install was fine.
    //
    // The function's own note says this one level down, about a different pair
    // of sentences: *two sentences, and one of them was said for both… read by
    // the same operator to opposite ends*. `estigia guard --uninstall` had the
    // right shape all along: *is not there — nothing to remove*.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // The floor: the install-side sentence still exists and still means what it
    // says, or an assertion on its absence below would pass against a build
    // that never prints it.
    let (out, _, _) = run(home.path(), &["setup", "claude-code"], "");
    assert!(
        out.contains("already current"),
        "a second setup no longer says the installation is current: {out}"
    );

    let (out, _, ok) = run(home.path(), &["uninstall", "claude-code"], "");
    assert!(ok, "the uninstall refused: {out}");
    let (out, _, ok) = run(home.path(), &["uninstall", "claude-code"], "");
    assert!(ok, "the second uninstall refused: {out}");
    assert!(
        !out.contains("already current"),
        "an uninstall with nothing to take says the installation is current: {out}"
    );
    assert!(
        out.contains("nothing of Estigia's is here"),
        "an uninstall with nothing to take does not say so: {out}"
    );
}

#[test]
fn a_hook_and_the_goal_report_agree_on_which_run_each_agent_is() {
    let session = "0198fe1c-2b7a-7d4e-9f01-abcdef012345";
    let payload =
        format!(r#"{{"tool_input":{{"file_path":"src/x.rs"}},"session_id":"{session}"}}"#);

    for (agent, hook_arguments, runtime) in [
        ("codex", vec!["--agent", "codex"], "codex"),
        ("claude-code", vec!["--agent", "claude-code"], "claude"),
        ("no agent", vec![], "claude"),
    ] {
        let home = tempfile::tempdir().expect("a temporary home");
        std::fs::create_dir_all(home.path().join("AppData").join("Roaming"))
            .expect("a roaming dir");
        let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
        assert!(ok, "setup failed: {stderr}");

        let mut arguments = vec!["hook", "pre-tool-use"];
        arguments.extend(hook_arguments);
        arguments.extend(["--dialect", "claude-code"]);
        let (_, _, ok) = run(home.path(), &arguments, &payload);
        assert!(ok, "{agent}: the hook refused");

        let ledger = decisions(home.path());
        let hooked: serde_json::Value =
            serde_json::from_str(ledger.lines().last().expect("a ledger line"))
                .expect("a ledger line is JSON");

        let (out, stderr, ok) = run(
            home.path(),
            &[
                "goal",
                "--json",
                "--session-id",
                session,
                "--runtime",
                runtime,
            ],
            "",
        );
        assert!(ok, "{agent}: goal refused: {stderr}");
        let goal: serde_json::Value = serde_json::from_str(&out).expect("goal --json is JSON");

        assert_eq!(
            hooked["run_id"], goal["run_id"],
            "{agent}: the gate and the goal report name different runs for one session"
        );
        assert_eq!(
            hooked["run_id"],
            estigia::harness::session::run_id(runtime, session),
            "{agent}: the derived identity moved"
        );
    }
}

#[test]
fn a_payload_that_names_no_tool_is_recorded_and_reported() {
    // `tool-unnamed` is one of the four verdicts that mean *this call was not
    // gated*: the payload parsed, it named no tool, and the classifier had
    // nothing to judge. It sits in `UNDECIDED`, it has its own sentence in the
    // silence row's table — and it appeared in no test at all, in either half.
    // Its two siblings, `payload-absent` and `payload-unreadable`, are both
    // exercised; this one and the shape it is written for were not.
    //
    // Rule 3 is what makes it worth a row rather than a denial: a schema this
    // build does not read could be wrapping `Read` as easily as `Write`, so the
    // call goes through — and the whole value of letting it through is that
    // somebody can find out afterwards. If the ledger line or the row went
    // missing, the call would go through in silence, which is the one thing
    // this row exists to prevent.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // Parses, and names no tool.
    let (out, _, ok) = run(
        home.path(),
        &[
            "hook",
            "pre-tool-use",
            "--agent",
            "claude-code",
            "--dialect",
            "claude-code",
        ],
        r#"{"tool_input":{"file_path":"src/x.rs"},"session_id":"abc123def456"}"#,
    );
    assert!(ok, "the hook refused: {out}");
    // Standing aside is an empty object: this does not deny.
    assert_eq!(
        out.trim(),
        "{}",
        "a payload naming no tool was answered with a decision"
    );

    let ledger = decisions(home.path());
    assert!(
        ledger.contains("tool-unnamed"),
        "a call that could not be judged left no line: {ledger}"
    );

    let (out, _, _) = run(home.path(), &["doctor", "--json"], "");
    let read: serde_json::Value = serde_json::from_str(&out).expect("doctor --json is json");
    let silence = read
        .as_array()
        .expect("the checks are a list")
        .iter()
        .find(|check| check["name"] == "silence")
        .expect("the silence row is checked")["health"]
        .clone();
    assert_eq!(
        silence["state"], "broken",
        "a call that went through ungated is not reported: {silence}"
    );
    assert!(
        silence["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("names no tool")),
        "the row does not say which of the four it was: {silence}"
    );
}

#[test]
fn the_three_things_that_make_a_stand_down_a_switch_are_refused() {
    // `stand-down` is the command that lowers the gate, and three things stop it
    // from being a switch: a reason to answer for, a window that covers time,
    // and a cap on how long. `harness::standdown`'s own tests hold all three
    // against `declare`, which is the right level for the rules — this holds
    // the two things that level cannot.
    //
    // The **wiring**: that `--reason` and `--minutes` reach those rules at all,
    // and that each refusal comes back out of the binary under its own code.
    //
    // And the cap's **value**. The test one level down asserts the boundary
    // with `LONGEST / 60`, so it moves when the constant moves: raising the cap
    // from four hours to twenty-four leaves it green. Written out here — 240 is
    // a stand-down, 241 is not — it does not. That is the same tautology this
    // repository found in its shell-verb list and in three of its gate lists: a
    // test driven by the constant cannot measure the constant.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    for (arguments, code) in [
        (
            vec!["stand-down", "--reason", "", "--minutes", "5"],
            "stand-down-needs-a-reason",
        ),
        (
            vec!["stand-down", "--reason", "a reason", "--minutes", "0"],
            "stand-down-covers-no-time",
        ),
        (
            vec!["stand-down", "--reason", "a reason", "--minutes", "241"],
            "stand-down-too-long",
        ),
    ] {
        let (_, stderr, ok) = run(home.path(), &arguments, "");
        assert!(!ok, "`{arguments:?}` was accepted as a stand-down");
        assert!(
            stderr.contains(code),
            "`{arguments:?}` was refused as something other than `{code}`: {stderr}"
        );
    }

    // And the floor, which is the cap's own sentence: the longest window there
    // is, is one. Without this the three refusals above would hold against a
    // command that refuses every stand-down there is.
    let (out, stderr, ok) = run(
        home.path(),
        &["stand-down", "--reason", "a reason", "--minutes", "240"],
        "",
    );
    assert!(ok, "the longest window there is was refused: {stderr}");
    assert!(
        out.contains("stood down"),
        "the gate did not say it had stood down: {out}"
    );
}

#[test]
fn a_file_estigia_cannot_use_never_names_a_command_that_reports_it_again() {
    // Three ways a hand-edited settings file is not what an envelope needs: not
    // JSON, JSON that is not an object, and — the one that was missed — JSON
    // whose *nested* value is the wrong shape. The first two were told to the
    // operator as knowledge only they have. The third arrived as a generic
    // setup failure and was answered with `estigia status`: a command that
    // reads the same file back and changes nothing, which is the dead end the
    // ratchet forbids and which `NotEditable`'s own documentation says it
    // exists to avoid.
    //
    // Run as a process, because what is being checked is the refusal an
    // operator actually sees.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let settings = home.path().join(".claude").join("settings.json");

    for broken in [
        // Not JSON.
        "{ nope",
        // JSON, not an object.
        "[1, 2, 3]",
        // An object whose `hooks` is not one.
        r#"{"hooks": "mine", "mySetting": 42}"#,
        // An object whose event list is not a list.
        r#"{"hooks": {"PreToolUse": "mine"}, "mySetting": 42}"#,
    ] {
        std::fs::write(&settings, broken).expect("the operator edits their file");
        let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
        assert!(
            !ok,
            "a file Estigia cannot use was written to anyway: {broken}"
        );
        assert!(
            stderr.contains("agent-file-not-editable"),
            "{broken} was reported as something other than an unusable file: {stderr}"
        );
        // The point of the code: no command is named, because none would help.
        assert!(
            !stderr.contains("run: estigia"),
            "{broken} sends the operator to a command that changes nothing: {stderr}"
        );
        // And the file is left exactly as they wrote it.
        let after = std::fs::read_to_string(&settings).expect("the file is still there");
        assert_eq!(after, broken, "Estigia rewrote a file it could not read");
    }
}

#[test]
fn taking_estigia_out_of_a_file_it_was_never_in_leaves_the_file_alone() {
    // The mirror of the write path, and it had neither half. A settings file
    // with nothing of Estigia's in it was parsed, reserialised and written
    // back: reported as `update`, and the operator's own whitespace replaced,
    // on an uninstall that removed nothing. Invariant two is that a file which
    // never mentioned Estigia is *reported unchanged rather than touched*.
    //
    // And a file that will not parse was answered with `estigia status`, the
    // dead end the write path had already been taught not to name — in a
    // message that did not even say which file.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let settings = home.path().join(".claude").join("settings.json");

    for foreign in [
        // Somebody else's hook, under the same event Estigia uses.
        r#"{"hooks": {"PreToolUse": [{"name": "theirs"}]}, "mySetting": 42}"#,
        r#"{"hooks": "mine", "mySetting": 42}"#,
        "[1, 2, 3]",
        "{\"mySetting\":42}",
    ] {
        std::fs::write(&settings, foreign).expect("the operator's file");
        let (out, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
        assert_eq!(
            std::fs::read_to_string(&settings).expect("still there"),
            foreign,
            "an uninstall that removed nothing rewrote {foreign}"
        );
        assert!(
            !out.contains("settings.json"),
            "it reported touching a file it left alone: {out}"
        );
    }

    // A file that will not parse: named, and answered with knowledge rather
    // than with a command.
    std::fs::write(&settings, "{ nope").expect("a broken file");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(
        !ok,
        "a file Estigia cannot read was written to on the way out"
    );
    assert!(
        stderr.contains("agent-file-not-editable"),
        "the removal path classifies it differently from the write path: {stderr}"
    );
    assert!(
        stderr.contains("settings.json"),
        "it does not say which file: {stderr}"
    );
    assert!(
        !stderr.contains("run: estigia"),
        "it names a command that changes nothing: {stderr}"
    );
}

#[test]
fn setup_for_one_named_agent_names_what_a_fresh_neighbour_took() {
    // The regression this guards: deleting `report_fresh_roots(...)` at the
    // `setup` call site leaves the whole suite green, because nothing else in
    // this crate drives `estigia setup <slug>` end to end and reads its own
    // stderr for this note. `estigia setup <slug>` is the command the
    // issue was about, and it must still say something when a fresh root
    // inherits from a machine that already has one configured — not only
    // `sync`, which `what_sync_tells_a_person_it_also_tells_a_script` already
    // covers.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup claude-code failed: {stderr}");

    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "setup codex failed: {stderr}");
    // "had none" is common to all three branches `fresh_root_notes` can print,
    // so on its own it would stay green even if `installed_configs` stopped
    // reading `configured(options)` and this fell back to the "nothing else
    // is installed" branch instead of inheriting claude-code's answer. "the
    // machine's answers" is this branch's own, and "nothing else is
    // installed" is the other branch's.
    assert!(
        stderr.contains("the machine's answers") && !stderr.contains("nothing else is installed"),
        "a named agent's fresh root did not say it took the machine's answers: {stderr}"
    );
}

#[test]
fn installing_and_removing_everything_leaves_every_file_that_was_not_estigia_s() {
    // The question an operator actually asks, and the one other tools get
    // wrong: *if I uninstall, do I lose my own things?* Answered here over a
    // home with the operator's content in every place Estigia writes — another
    // skill beside its own, their notes inside its directory, their prose
    // around its block, their keys and their hooks in the files it edits.
    //
    // Text is compared byte for byte. JSON is compared as **data**: Estigia has
    // to parse and reserialise a file it edits inside, so the whitespace is not
    // preserved and the README says so. What must be preserved is every key.
    let home = tempfile::tempdir().expect("a temporary home");
    // The three paths *inside* Estigia's own directory are built from
    // `skill::DIRECTORY` rather than spelled. Spelled, they named `issue-flow`
    // after the install had moved to `flow`, so the operator's files were
    // planted in a directory Estigia never touched and the test asserted they
    // survived an uninstall that was never going to reach them — green, and
    // measuring nothing.
    let installed = estigia::skill::DIRECTORY;
    let mine: Vec<(String, &str)> = vec![
        // A whole skill of the operator's, beside Estigia's own.
        (
            ".agents/skills/mine/SKILL.md".to_owned(),
            "---\nname: mine\n---\n\nMine.\n",
        ),
        (".agents/skills/mine/notes.md".to_owned(), "My notes.\n"),
        (
            ".claude/skills/also-mine/SKILL.md".to_owned(),
            "---\nname: also-mine\n---\n",
        ),
        // Their files *inside* the directory Estigia installs into.
        (
            format!(".agents/skills/{installed}/MY-NOTES.md"),
            "Mine, in Estigia's directory.\n",
        ),
        (
            format!(".agents/skills/{installed}/references/mine.md"),
            "My reference, beside the shipped ones.\n",
        ),
        // The operator override, which Estigia reads and never writes.
        (
            format!(".agents/skills/{installed}/estigia.local.md"),
            "# Mine\n\n<!-- estigia:config:start -->\n| Setting | Value here | Skill default |\n\
             |---|---|---|\n| Model routing | developer=opus | unset |\n<!-- estigia:config:end -->\n\nMy rules.\n",
        ),
        // Their prose, around where the directive goes.
        (
            ".agents/AGENTS.md".to_owned(),
            "# Mine\n\nBefore.\n\n## My section\n\nText.\n",
        ),
        (
            ".claude/CLAUDE.md".to_owned(),
            "# My CLAUDE.md\n\nMy rules.\n",
        ),
        // A file in the same tree that concerns nobody.
        (".agents/loose.txt".to_owned(), "loose\n"),
    ];
    let json: &[(&str, &str)] = &[
        (
            ".claude/settings.json",
            r#"{"mySetting":42,"hooks":{"PreToolUse":[{"name":"mine","hooks":[{"command":"echo mine"}]}],"MyEvent":[{"name":"only-mine"}]},"mcpServers":{"my-server":{"command":"mine"}}}"#,
        ),
        (
            ".cursor/hooks.json",
            r#"{"version":1,"hooks":{"beforeShellExecution":[{"command":"mine"}]}}"#,
        ),
        // No MCP server of their own: the uninstall used to hand this file back
        // with `"mcpServers": {}` in it, which is Estigia leaving something of
        // its own behind on the way out.
        (".qwen/settings.json", r#"{"mySetting":true}"#),
    ];

    for (path, body) in mine
        .iter()
        .map(|(path, body)| (path.as_str(), *body))
        .chain(json.iter().copied())
    {
        let full = home.path().join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("their directory");
        std::fs::write(&full, body).expect("their file");
    }

    let (_, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "setup --all failed: {stderr}");
    let (_, stderr, ok) = run(home.path(), &["setup", "--all", "--uninstall"], "");
    assert!(ok, "uninstall --all failed: {stderr}");

    for (path, body) in &mine {
        let full = home.path().join(path);
        let after = std::fs::read_to_string(&full)
            .unwrap_or_else(|error| panic!("{path} did not survive the uninstall: {error}"));
        assert_eq!(&after, body, "{path} came back changed");
    }
    for (path, body) in json {
        let full = home.path().join(path);
        let after = std::fs::read_to_string(&full)
            .unwrap_or_else(|error| panic!("{path} did not survive the uninstall: {error}"));
        let theirs: serde_json::Value = serde_json::from_str(body).expect("their JSON");
        let back: serde_json::Value =
            serde_json::from_str(&after).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(theirs, back, "{path} came back with different data");
    }
}

#[test]
fn a_settings_file_comes_back_indented_the_way_it_was_written() {
    // The last thing that was not the operator's after a round trip. Their
    // keys survived and their order survived; the whitespace did not, because
    // `to_string_pretty` always uses two spaces. A tool that goes into a file
    // to add one hook and hands it back reindented has changed something that
    // was theirs — and the whole point of taking it back out is that nothing
    // of theirs moves.
    for indent in ["    ", "\t", "  "] {
        let home = tempfile::tempdir().expect("a temporary home");
        let settings = home.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(settings.parent().expect("a parent")).expect("their directory");
        let theirs = format!(
            "{{\n{indent}\"mySetting\": 42,\n{indent}\"nested\": {{\n{indent}{indent}\"key\": \
             true\n{indent}}}\n}}\n"
        );
        std::fs::write(&settings, &theirs).expect("their file");

        let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
        assert!(ok, "setup failed: {stderr}");
        let (_, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
        assert!(ok, "uninstall failed: {stderr}");

        let back = std::fs::read_to_string(&settings).expect("their file survived");
        assert_eq!(
            back, theirs,
            "a file indented with {indent:?} came back indented some other way"
        );
    }
}

#[test]
fn a_file_estigia_is_about_to_write_over_is_not_reported_as_one_of_its_own() {
    // Estigia installs upstream's skill under upstream's name, so an operator
    // who already runs `issue-flow` has files at exactly those paths. Install
    // writes its own copy over them, and uninstall leaves that copy — because
    // the file is not Estigia's to remove. The round trip is therefore lossy
    // for their content, and the word for it was `update`: the same word used
    // for Estigia adjusting a file it wrote itself.
    //
    // Nothing here changes what install does. What changes is that the plan
    // says it, in the tense it is in, before anything happens.
    let home = tempfile::tempdir().expect("a temporary home");
    let skill = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::create_dir_all(&skill).expect("their skill directory");
    std::fs::write(skill.join("SKILL.md"), "# Mine\n\nMy own notes.\n").expect("their file");

    // The plan, which is the protection: it must name the file and say that
    // nothing has happened yet.
    let (plan, stderr, ok) = run(home.path(), &["setup", "codex", "--dry-run"], "");
    assert!(ok, "the plan failed: {stderr}");
    assert!(
        plan.contains("REPLACE"),
        "the plan calls writing over their file an update: {plan}"
    );
    assert!(
        plan.contains("Nothing has been yet"),
        "the plan does not say it is a plan: {plan}"
    );
    // And their file is untouched by a plan.
    assert_eq!(
        std::fs::read_to_string(skill.join("SKILL.md")).expect("still theirs"),
        "# Mine\n\nMy own notes.\n"
    );

    // The run says the same thing in the past tense.
    let (out, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "setup failed: {stderr}");
    assert!(
        out.contains("were written over"),
        "the run does not say what it did: {out}"
    );

    // A second run has nothing to say: the file is now what Estigia ships.
    let (again, _, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok);
    assert!(
        !again.contains("REPLACE"),
        "a file that already matches was reported as written over: {again}"
    );

    // And a file Estigia created itself, **changed by hand**, is an `OVERWRITE`.
    //
    // This asserted `update`, and the reasoning was sound while it held: the
    // record kept names and not contents, so *did you change this, or did an
    // older build write it* had no answer — and calling every stale file
    // somebody's work would announce a false alarm on every installation
    // there is. The record holds a digest of what it last wrote now, so the
    // question has an answer, and this is the case where it is *you did*.
    //
    // Then it said `REPLACE`, which is the word for a file that was never
    // Estigia's — and the note under it said so out loud, about a file
    // `estigia setup` had just installed. The two states have their own words
    // now, and this is the one where the operator has lost an edit.
    let contract = skill.join("references").join("runtime-notes.md");
    std::fs::write(&contract, "changed by hand\n").expect("edit a file Estigia created");
    let (mine, _, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok);
    assert!(
        mine.contains("OVERWRITE"),
        "a file Estigia created and somebody then edited was written over quietly: {mine}"
    );
    assert!(
        !mine.contains("are not Estigia's"),
        "their own edit was reported as a stranger's file: {mine}"
    );
}

#[test]
fn an_ordinary_upgrade_is_not_accused_of_writing_over_somebody_s_work() {
    // `REPLACE` is a serious thing to say, so it is only said when it is
    // knowable. Three cases, and the first draft of this got two of them wrong
    // in opposite directions.
    let e = |home: &std::path::Path, args: &[&str]| -> String {
        let (out, stderr, ok) = run(home, args, "");
        assert!(ok, "{args:?} failed: {stderr}");
        out
    };

    // 1. An upgrade whose record predates the record. Estigia wrote these
    //    files; it just has nothing written down saying so. The contract still
    //    carries its own fence, which is the signal that it has been here.
    let home = tempfile::tempdir().expect("a temporary home");
    e(home.path(), &["setup", "codex"]);
    let skill = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::remove_dir_all(skill.join(".estigia")).expect("forget the record");
    std::fs::write(skill.join("references").join("runtime-notes.md"), "older\n")
        .expect("an older shipped file");
    let plan = e(home.path(), &["setup", "codex", "--dry-run"]);
    assert!(
        !plan.contains("REPLACE"),
        "an ordinary upgrade was told its own files belong to somebody else: {plan}"
    );

    // 2. A directory Estigia has never been in. Nothing vouches for what is
    //    there, and what is there is somebody else's.
    for contract in [
        "# Mine\n",
        // Upstream's own fence is not Estigia's: it carries the `issue-flow`
        // name, which is exactly how the two are told apart.
        "# Mine\n\n<!-- issue-flow:config:start -->\n<!-- issue-flow:config:end -->\n",
    ] {
        let home = tempfile::tempdir().expect("a temporary home");
        let skill = home
            .path()
            .join(".codex")
            .join("skills")
            .join(estigia::skill::DIRECTORY);
        std::fs::create_dir_all(&skill).expect("their directory");
        std::fs::write(skill.join("SKILL.md"), contract).expect("their contract");
        let plan = e(home.path(), &["setup", "codex", "--dry-run"]);
        assert!(
            plan.contains("REPLACE"),
            "a file Estigia has never seen was called its own: {plan}"
        );
    }

    // 3. A clean install, and a rerun of one, have nothing to warn about.
    let home = tempfile::tempdir().expect("a temporary home");
    for args in [
        vec!["setup", "codex", "--dry-run"],
        vec!["setup", "codex"],
        vec!["setup", "codex"],
    ] {
        let out = e(home.path(), &args);
        assert!(!out.contains("REPLACE"), "{args:?} cried wolf: {out}");
    }
}

#[test]
fn config_set_reports_what_the_table_holds_and_not_what_was_typed() {
    // Two branches, two answers to one question. The changed branch resolved
    // the value and the unchanged one echoed the argument, so
    // `config set "Irreversible commands" ""` answered `was already ` with a
    // blank and `NONE` answered `was already NONE`. All of them hold `none`.
    //
    // Agreeing with whoever spoke last is not reporting; it is the same fault
    // as `config set` telling somebody a row is in force when a local file
    // shadows it, and that one already has a refusal of its own.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    for typed in ["", "   ", "NONE", "none"] {
        let (out, _, ok) = run(
            home.path(),
            &["config", "set", "Irreversible commands", typed],
            "",
        );
        assert!(ok, "`{typed}` was refused");
        assert!(
            out.contains("none"),
            "typing {typed:?} was answered with something other than what is held: {out}"
        );
        assert!(
            !out.contains("NONE"),
            "typing {typed:?} was echoed back rather than resolved: {out}"
        );
    }

    // And the per-agent path, which had the same echo.
    let (_, stderr, ok) = run(home.path(), &["setup", "opencode"], "");
    assert!(ok, "setup failed: {stderr}");
    let (out, _, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "UNSET",
            "--agent",
            "opencode",
        ],
        "",
    );
    assert!(ok, "the write was refused");
    assert!(
        out.contains("\"unset\"") && !out.contains("\"UNSET\""),
        "the per-agent write echoed the argument instead of the value: {out}"
    );
}

#[test]
fn config_set_reports_the_retired_row_it_folds_in_another_root() {
    // A retired `Delegated workers` row in a second installed root is folded
    // when `config set` spreads a machine- or repository-wide row into that
    // root's contract — the write is what takes the row out of the table. The
    // fold is a note, not a silence: `setup` already prints it, and the same
    // sentence belongs on the command that performs the same fold.
    let home = tempfile::tempdir().expect("a temporary home");
    for agent in ["claude-code", "codex"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "setup {agent} failed: {stderr}");
    }

    // `claude-code` is the canonical root, so `codex` is the other one this
    // command spreads into. Its contract still carries a row this build
    // retired, as a contract written before the row was.
    let codex_contract = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let text = std::fs::read_to_string(&codex_contract).expect("codex's contract");
    let header = "| Setting | Value here | Skill default |\n|---|---|---|\n";
    assert!(
        text.contains(header),
        "the fixture's table shape has drifted: {text}"
    );
    std::fs::write(
        &codex_contract,
        text.replacen(
            header,
            &format!("{header}| Delegated workers | developer |\n"),
            1,
        ),
    )
    .expect("the rewritten contract");

    let (out, stderr, ok) = run(
        home.path(),
        &["config", "set", "Evidence standard", "measuring"],
        "",
    );
    assert!(ok, "config set failed: {stderr}{out}");
    assert!(
        out.contains("Delegated workers"),
        "the retired row folded in the other root without a note: {out}"
    );
    // The note is `setup`'s, not a second copy: it says where the row's meaning
    // went, which only the one sentence does.
    assert!(
        out.contains("Model routing"),
        "the note does not say where the row's meaning went: {out}"
    );
    // And the write is what took the row out of the other table.
    let after = std::fs::read_to_string(&codex_contract).expect("codex's contract");
    assert!(
        !after.contains("Delegated workers"),
        "the write left the retired row in the table: {after}"
    );
}

#[test]
fn the_commands_that_read_a_contract_find_one_a_skill_only_install_wrote() {
    // `--skill-only` is a documented flag: the skill, the gate and the MCP
    // server, with the instruction file left alone. Three commands decided
    // which agents to work on by asking whether the *directive* was written,
    // which that flag deliberately skips — so `config list`, `config set` and
    // `sync` all answered "no agent has Estigia installed" on a machine where
    // the contract they were about to read was sitting there installed.
    //
    // And each named `estigia setup --all` as the way out: the one command that
    // writes the instruction file the flag exists to leave alone. The ratchet
    // says a message may name a command only when running it discharges the
    // block; this named the command that undoes what the operator asked for.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code", "--skill-only"], "");
    assert!(ok, "the install failed: {stderr}");

    for arguments in [
        vec!["config", "list"],
        vec![
            "config",
            "set",
            "Delivery authorisation",
            "ask",
            "--agent",
            "claude-code",
        ],
        vec!["sync"],
    ] {
        let (out, stderr, ok) = run(home.path(), &arguments, "");
        assert!(
            ok,
            "`{}` was refused on a skill-only install: {stderr}{out}",
            arguments.join(" ")
        );
        assert!(
            !stderr.contains("nothing-configured"),
            "`{}` says nothing is installed: {stderr}",
            arguments.join(" ")
        );
    }

    // The flag still did what it says: the instruction file stays unwritten.
    // `sync` reaching this install is the fix; `sync` writing a directive into
    // it would be the same flag undone by the other end.
    assert!(
        !home.path().join(".claude").join("CLAUDE.md").exists(),
        "the instruction file was written after all"
    );

    // And the ordinary install still gets one, so what changed is *whose shape
    // sync preserves* and not whether it writes a directive at all.
    let ordinary = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(ordinary.path(), &["setup", "claude-code"], "");
    assert!(ok, "the install failed: {stderr}");
    let directive = ordinary.path().join(".claude").join("CLAUDE.md");
    assert!(directive.is_file(), "setup wrote no directive");
    let before = std::fs::read_to_string(&directive).expect("the directive");
    let (_, stderr, ok) = run(ordinary.path(), &["sync"], "");
    assert!(ok, "sync failed: {stderr}");
    assert_eq!(
        std::fs::read_to_string(&directive).expect("the directive"),
        before,
        "sync changed the directive of an agent that had one"
    );
}

#[test]
fn a_row_that_will_not_parse_does_not_cost_the_operator_the_rows_that_will() {
    // `config_for` reads the installed table back so a re-run preserves it —
    // its own words: "an operator who configured `squash` a month ago does not
    // get `merge commit` because the tool was upgraded". It read *absent* and
    // *unreadable* as the same answer, and the fallback for absent is
    // `Config::default()`. So one bad row in `estigia.local.md` — the
    // operator's own file, which Estigia never writes — made `sync` write the
    // defaults over every choice in the file it does write. `squash` and a
    // named role came back `merge commit` and `unset`, under one line reading
    // `update`.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "the install failed: {stderr}");
    for (setting, value, agent) in [
        ("Change size", "400", None),
        ("Model routing", "developer=opus", Some("codex")),
    ] {
        let mut arguments = vec!["config", "set", setting, value];
        arguments.extend(agent.into_iter().flat_map(|slug| ["--agent", slug]));
        let (_, stderr, ok) = run(home.path(), &arguments, "");
        assert!(ok, "setting {setting} failed: {stderr}");
    }

    let skill = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let contract = skill.join("SKILL.md");
    let before = std::fs::read_to_string(&contract).expect("the contract");
    assert!(
        before.contains("| Change size | 400 |"),
        "the premise is wrong: the choice was never written"
    );

    // A value the parser refuses, in the operator's own file.
    std::fs::write(
        skill.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Integration | octopus |\n",
    )
    .expect("their file");

    let (out, stderr, ok) = run(home.path(), &["sync"], "");
    assert!(
        !ok,
        "sync claimed to have moved a contract it could not read"
    );
    assert_eq!(
        std::fs::read_to_string(&contract).expect("the contract"),
        before,
        "sync wrote defaults over rows it could not read back"
    );

    // And the refusal is the one the row deserves, not the generic write
    // failure: nothing was written, so `unknown` would be false, and `estigia
    // status` cannot fix a row in a file Estigia does not edit.
    let said = format!("{stderr}{out}");
    assert!(
        said.contains("estigia.local.md"),
        "the file holding the bad row is not named: {said}"
    );
    assert!(
        !said.contains("estigia status"),
        "a bad row in the operator's own file was answered with `estigia status`: {said}"
    );
}

#[test]
fn a_run_names_every_file_it_puts_on_the_disk_and_every_one_it_takes_away() {
    // The report is what an operator decides from, and `--dry-run` promises it
    // in advance: "Report what would change and write nothing". A file that
    // appears without being named is one they cannot account for, and the three
    // that did were `<skill>/.estigia/installed.json` — the record that decides
    // what `uninstall` is allowed to touch. The least appropriate file on the
    // machine to be invisible.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join(".claude")).expect("their directory");
    std::fs::write(home.path().join(".claude").join("CLAUDE.md"), "# mine\n").expect("their file");

    let before = every_file(home.path());
    let (out, stderr, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(ok, "the install failed: {stderr}");
    let after = every_file(home.path());
    let named = named_in(&out);

    for path in after.difference(&before) {
        assert!(
            named.contains(path),
            "the run put {} on the disk and named it nowhere:\n{out}",
            path.display()
        );
    }

    // Then the machine is **used**, because the files that matter most on the
    // way out are the ones a bare fixture never creates: Estigia's own state.
    // Nothing between the install and the removal touched it, so `~/.estigia`
    // did not exist and the half below was checking agent files only.
    run(
        home.path(),
        &["stand-down", "--reason", "a fixture", "--minutes", "5"],
        "",
    );
    let after = every_file(home.path());
    assert!(
        after
            .iter()
            .any(|path| path.to_string_lossy().contains(".estigia")),
        "the fixture created no state, so this still checks an unused machine"
    );

    // And the way back out, which is where an unnamed file is worse: it goes
    // without anybody being told it was ever there.
    let (out, stderr, ok) = run(home.path(), &["setup", "--all", "--uninstall"], "");
    assert!(ok, "the removal failed: {stderr}");
    let gone = every_file(home.path());
    let named = named_in(&out);
    for path in after.difference(&gone) {
        assert!(
            named.contains(path),
            "the removal took {} away and named it nowhere:\n{out}",
            path.display()
        );
    }
    assert_eq!(gone, before, "the operator's own files did not survive");
}

#[test]
fn the_gate_and_config_list_read_the_same_contract() {
    // Estigia installs upstream's skill under upstream's name on purpose, so a
    // machine that already runs `issue-flow` has a skill root carrying the
    // transport and none of Estigia's configuration. `discover_skill_root`
    // picked the first root with a transport, which is that one — so the gate
    // read a contract with no block in it, which is every setting at its
    // default: no declared boundary is a boundary, and the renewal window is at
    // its widest.
    //
    // Nothing said so. `config list` read the agent's own contract and reported
    // the operator's values back to them, correctly, while the thing that
    // decides read somebody else's file.
    let home = tempfile::tempdir().expect("a temporary home");
    let theirs = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::create_dir_all(theirs.join("scripts")).expect("their checkout");
    std::fs::write(theirs.join("scripts").join("github.py"), "# theirs\n")
        .expect("their transport");
    std::fs::write(theirs.join("SKILL.md"), "# their skill\n").expect("their contract");

    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "the install failed: {stderr}");
    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "developer=opus",
            "--agent",
            "codex",
        ],
        "",
    );
    assert!(ok, "the write failed: {stderr}");

    // Where the harness says the skill is — the same answer `control_surface`
    // hands the gate.
    let (out, _, _) = run(home.path(), &["doctor", "--json"], "");
    let checks: serde_json::Value = serde_json::from_str(&out).expect("doctor prints JSON");
    let found = checks
        .as_array()
        .expect("an array")
        .iter()
        .find(|check| check["name"] == "skill")
        .expect("the skill check")["health"]["detail"]
        .as_str()
        .expect("a path")
        .to_owned();

    // The one that carries what the operator set, not the one that merely has
    // a transport in it.
    let contract = std::fs::read_to_string(std::path::Path::new(&found).join("SKILL.md"))
        .expect("the contract the harness found");
    assert!(
        contract.contains("developer=opus"),
        "the gate reads {found}, which does not hold what `config list` reports"
    );
}

#[test]
fn a_fact_about_the_repository_reaches_every_contract_and_leaves_the_rest_alone() {
    // `config set --agent` refuses a repository-scoped row, and says why: "a
    // setting whose scope is the repository has one answer, and the gate reads
    // it without asking which agent is holding the tools". Written without
    // `--agent` it went into the first contract that answered and no other — so
    // it varied by agent anyway. An operator with two agents installed declared
    // `make deploy` a one-way door and only one of the two contracts said so:
    // the gate read that one and enforced it, the agent reading the other was
    // never told what it had walked into, and uninstalling the first would have
    // taken the boundary with it.
    let home = tempfile::tempdir().expect("a temporary home");
    for agent in ["claude-code", "codex"] {
        let (_, stderr, ok) = run(home.path(), &["setup", agent], "");
        assert!(ok, "installing {agent} failed: {stderr}");
    }
    let claude = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let codex = home
        .path()
        .join(".codex")
        .join("skills")
        .join(estigia::skill::DIRECTORY);

    // One agent's own answer first, so the spread has something to preserve.
    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Model routing",
            "developer=opus",
            "--agent",
            "codex",
        ],
        "",
    );
    assert!(ok, "the per-agent write failed: {stderr}");

    let (out, stderr, ok) = run(
        home.path(),
        &["config", "set", "Irreversible commands", "make deploy"],
        "",
    );
    assert!(ok, "the repository write failed: {stderr}");
    assert!(
        out.contains("other installed contract"),
        "it wrote into more than one contract and said nothing: {out}"
    );

    for (name, root) in [("claude-code", &claude), ("codex", &codex)] {
        let contract = std::fs::read_to_string(root.join("SKILL.md")).expect("the contract");
        assert!(
            contract.contains("make deploy"),
            "{name} was never told about a one-way door declared for this repository"
        );
    }

    // And the row that is genuinely one agent's did not spread with it. Read
    // from the JSON, not the table: the text lists every accepted value beside
    // each row, so `developer=opus` appears in `Model routing`'s help whatever
    // is held.
    let models_of = |slug: &str| -> String {
        let (out, _, _) = run(
            home.path(),
            &["config", "list", "--agent", slug, "--json"],
            "",
        );
        let rows: serde_json::Value = serde_json::from_str(&out).expect("config list prints JSON");
        rows.as_array()
            .expect("an array of rows")
            .iter()
            .find(|row| row["setting"] == "Model routing")
            .expect("Model routing is a setting")["value"]
            .as_str()
            .expect("a value")
            .to_owned()
    };
    assert_eq!(models_of("codex"), "developer=opus", "the premise moved");
    assert_eq!(
        models_of("claude-code"),
        "unset",
        "a per-agent answer was copied onto another agent by a repository-wide write"
    );
}

#[test]
fn a_dry_run_reaches_every_adapters_probe_and_a_failing_probe_surfaces() {
    enum Corruption {
        Bytes(&'static str),
        Directory,
    }
    let cases = [
        ("agents", ".agents/AGENTS.md", Corruption::Directory),
        (
            "claude-code",
            ".claude/settings.json",
            Corruption::Bytes("{ not json"),
        ),
        (
            "codex",
            ".codex/hooks.json",
            Corruption::Bytes("{ not json"),
        ),
        (
            "opencode",
            ".config/opencode/opencode.json",
            Corruption::Bytes("{ not json"),
        ),
        ("pi", ".pi/agent/AGENTS.md", Corruption::Directory),
        (
            "cursor",
            ".cursor/hooks.json",
            Corruption::Bytes("{ not json"),
        ),
        (
            "zcode",
            ".zcode/cli/config.json",
            Corruption::Bytes("{ not json"),
        ),
        ("deepseek-harness", ".dsh/AGENTS.md", Corruption::Directory),
    ];
    assert_eq!(
        cases.len(),
        estigia::setup::AGENTS.len(),
        "an adapter was added without a probe case"
    );
    for (slug, relative, corruption) in cases {
        let home = tempfile::tempdir().expect("a temporary home");
        let path = home.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("their directory");
        match corruption {
            Corruption::Bytes(bytes) => std::fs::write(&path, bytes).expect("their file"),
            Corruption::Directory => std::fs::create_dir(&path).expect("their directory"),
        }
        let before = snapshot(home.path());

        let (out, stderr, ok) = run(home.path(), &["setup", slug, "--dry-run"], "");

        assert!(!ok, "{slug}: the dry run passed over {relative}: {out}");
        assert!(
            stderr.contains(relative.rsplit('/').next().expect("a file name")),
            "{slug}: the refusal does not name {relative}: {stderr}"
        );
        assert_eq!(
            snapshot(home.path()),
            before,
            "{slug}: a dry run wrote something"
        );
    }
}
