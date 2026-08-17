// The same exception the library makes for its own tests, restated because an
// integration test is a separate crate and the crate-level allow does not reach
// it: an assertion that panics is the assertion working, and threading `?`
// through a test buys nothing and costs the reader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! What happens when the reader goes away.
//!
//! A process-level test, because the defect is a process-level one: `println!`
//! panics when the pipe closes, and no unit test that captures output can see
//! it. `estigia status | head` is the most ordinary thing a person does with a
//! command that prints a list, and it ended in a backtrace.
//!
//! It has since become the file where *every verb* is run as a process, for a
//! plainer reason: running them by hand found four defects the whole suite had
//! not, in one afternoon. A method that only works while somebody remembers to
//! apply it is not a method, so it is here.

use std::io::Read;
use std::process::{Command, Stdio};

/// The binary under test, as Cargo built it.
fn estigia() -> &'static str {
    env!("CARGO_BIN_EXE_estigia")
}

#[test]
fn a_reader_that_stops_reading_does_not_produce_a_backtrace() {
    // `status` prints more than one line for any installation, so dropping the
    // pipe after the first read closes it while the process is still writing.
    let mut child = Command::new(estigia())
        .arg("status")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    // Read one chunk, then drop the pipe — which is what `head` does.
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut first = [0u8; 16];
    let _ = stdout.read(&mut first);
    drop(stdout);

    let output = child.wait_with_output().expect("the process ends");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked"),
        "a closed pipe produced a panic:\n{stderr}"
    );
}

#[test]
fn a_refusal_still_reaches_standard_error_and_a_non_zero_code() {
    // The other half: the exit contract has to survive whatever the output
    // handling does. `setup` with no agent is the cheapest refusal there is.
    let output = Command::new(estigia())
        .arg("setup")
        .output()
        .expect("the binary runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agent-not-named"), "{stderr}");
    assert!(stderr.contains("run: estigia setup --all"), "{stderr}");
}

#[test]
fn help_and_version_exit_zero() {
    // The ratchet names `estigia <command> --help` as a resolution, so it has
    // to be a command that succeeds rather than one clap reports as an error.
    for arguments in [["--help"], ["--version"]] {
        let output = Command::new(estigia())
            .args(arguments)
            .output()
            .expect("the binary runs");
        assert!(
            output.status.success(),
            "`estigia {}` exited {:?}",
            arguments[0],
            output.status.code()
        );
    }
}

/// Runs `estigia` against a home of its own.
///
/// Four variables, not one. `APPDATA` and `XDG_CONFIG_HOME` are read directly
/// for two of the six roots, and on Windows `APPDATA` is always set — so a child
/// given only `HOME` answers half its questions about the developer's real
/// machine. That split produced a `status` line claiming an agent was configured
/// on a machine where nothing had ever been installed.
///
/// And one taken **away**. `ESTIGIA_FLAG` is read straight from the environment
/// and it decides a gate refusal — `unflagged-on-trunk` fires or does not
/// depending on it — so a child that inherits the developer's shell answers a
/// question about their machine rather than about the fixture. Nothing here
/// exercises that path today, which is why the suite passes either way; a trap
/// set and not yet stepped on is still a trap, and the first test to reach a
/// trunk delivery would pass or fail by who ran it.
///
/// The whole namespace rather than the one name, so the next variable this crate
/// reads does not put the trap back. Nothing else is cleared: `USER`,
/// `COMPUTERNAME` and their kind end up in a record's prose and decide nothing,
/// and `PATH` has to stay real or the child finds no interpreter.
fn run(home: &std::path::Path, arguments: &[&str], stdin: &str) -> (String, String, bool) {
    run_in(home, home, arguments, stdin)
}

/// The same, standing somewhere other than the home.
///
/// Every command that answers *about this checkout* reads the working directory,
/// so a test that always stands in the home cannot tell the two apart.
/// An absolute path, spelled for the platform running the test.
///
/// `C:/trees` is absolute on Windows and a **relative** path everywhere else, so
/// the `Worktree location` row refused it and the value never reached the file
/// the assertions below read. The row was right; the fixture spelled one
/// platform.
#[cfg(windows)]
const TREES: &str = "C:/trees";
#[cfg(unix)]
const TREES: &str = "/trees";

fn run_in(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &str,
) -> (String, String, bool) {
    run_with_path(home, here, arguments, stdin, None)
}

/// The same, with the search path replaced.
///
/// One `doctor` row is about a program on the machine — `gh` — and its broken
/// state is *this machine has no authenticated GitHub CLI*. It was the last row
/// measured as a builder and unmeasured as report, and the reason given was
/// that forcing it takes a path with no `gh` on it. This is that path.
fn run_with_path(
    home: &std::path::Path,
    here: &std::path::Path,
    arguments: &[&str],
    stdin: &str,
    search_path: Option<&std::path::Path>,
) -> (String, String, bool) {
    let inherited: Vec<String> = std::env::vars()
        .map(|(name, _)| name)
        .filter(|name| name.starts_with("ESTIGIA_"))
        .collect();
    let mut command = Command::new(estigia());
    for name in &inherited {
        command.env_remove(name);
    }
    if let Some(only) = search_path {
        command.env("PATH", only);
    }
    let mut effective = arguments.to_vec();
    if matches!(effective.first(), Some(&"setup" | &"install" | &"sync"))
        && !effective.contains(&"--uninstall")
        && !effective.contains(&"--allow-source-build")
    {
        effective.push("--allow-source-build");
    }
    let mut child = command
        .args(effective)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .current_dir(here)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("the payload is written");
    let output = child.wait_with_output().expect("the process ends");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

#[test]
fn every_verb_runs_as_a_process_on_a_machine_with_nothing_installed() {
    // The state a first-time user is in, which is the state nothing was ever
    // tested in. Each verb has to answer — not hang, not panic, not report
    // success while refusing — and a refusal has to name a command, which is the
    // ratchet applied to the one path nobody exercised.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");

    for (arguments, stdin) in [
        (vec!["status"], ""),
        (vec!["update"], ""),
        (vec!["doctor"], ""),
        (vec!["config", "list"], ""),
        (vec!["sync"], ""),
        (vec!["release", "--run-id", "claude-nothing0"], ""),
        (vec!["gate", "Write", "--input", "{}"], ""),
        (vec!["guard", "--dry-run"], ""),
        (vec!["setup", "--all", "--dry-run"], ""),
        (vec!["uninstall", "--all", "--dry-run"], ""),
        (
            vec!["hook", "pre-tool-use"],
            "{}
",
        ),
        (
            vec!["mcp"],
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}
",
        ),
    ] {
        let (out, err, ok) = run(home.path(), &arguments, stdin);
        let named = arguments.join(" ");
        assert!(
            !err.contains("panicked"),
            "`estigia {named}` panicked:
{err}"
        );
        if !ok {
            // The ratchet: a message may name a command only when running it
            // discharges the block, and a refusal that names none leaves a
            // first-time user with nowhere to go.
            assert!(
                err.contains("run: ") || err.contains("["),
                "`estigia {named}` refused and named nothing to do:
{err}"
            );
        }
        assert!(
            !out.is_empty() || !err.is_empty(),
            "`estigia {named}` said nothing at all"
        );
    }
}

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
fn every_command_that_prints_honours_the_global_json_flag() {
    // `--json` is declared once, on the root parser, and honoured in as many
    // places as there are commands. Nothing crossed the two, and `gate` did not
    // honour it — the one command a program calls, since the OpenCode plugin
    // shells out to `estigia gate <tool> --input <json>` on every edit. A
    // machine had to read a sentence to learn whether it could write.
    //
    // Standard output only: a refusal goes to standard error, and mixing the
    // two is what made this look like malformed JSON rather than none.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");

    // On a machine that has been **used**, not a bare one. The fixture was
    // empty, so every command here reported nothing and printed nothing — and
    // that is how `uninstall --all --json` came to print the array and then two
    // prose lines about the state it removed, for three rounds, under a test
    // written to catch exactly that. A command prints its second half only when
    // there is a second half.
    run(home.path(), &["setup", "--all"], "");
    run(
        home.path(),
        &["stand-down", "--reason", "a fixture", "--minutes", "5"],
        "",
    );
    assert!(
        home.path()
            .join(".estigia")
            .join("stand-down.json")
            .is_file(),
        "the fixture has no state, so this checks the same empty machine as before"
    );

    // And a repository to stand in, because one command here answers only
    // inside one. `guard` was on this list and measured nothing: the home is
    // not a checkout, so `hooks_directory` refused, the refusal went to
    // standard error, standard output came back empty, and the loop below
    // skipped it as *said nothing, which is allowed*. It printed prose on every
    // path in a real repository the whole time — measured on the binary:
    // `wrote …/pre-push` and four lines of caveats under `--json`.
    let repository = tempfile::tempdir().expect("a repository");
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repository.path())
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let ready = git(&["init", "--quiet"])
        && git(&["config", "user.email", "guard@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);
    assert!(
        ready,
        "git is not usable here, and one command needs a checkout"
    );

    for arguments in [
        vec!["status", "--json"],
        vec!["update", "--json"],
        vec!["doctor", "--json"],
        vec!["config", "list", "--json"],
        // The three that were written after this list and never added to it, so
        // all three printed prose under the flag for as long as they existed.
        // `config edit` is the one command left out on purpose: it draws a
        // screen, and there is no prose for `--json` to be an alternative to.
        vec!["config", "set", "Merge strategy", "squash", "--json"],
        vec!["config", "repos", "--json"],
        vec!["config", "forget", "--json"],
        vec!["setup", "--all", "--dry-run", "--json"],
        vec!["uninstall", "--all", "--dry-run", "--json"],
        vec!["sync", "--json"],
        vec!["gate", "Write", "--input", "{}", "--json"],
        vec!["release", "--run-id", "claude-nothing0", "--json"],
        // Both halves of the one command here whose success path needs nothing
        // but a clock. It was absent from this list and printed prose on both:
        // *gate stood down for 5 minutes …* and *stand-down lifted; the gate
        // decides on its own again*. `claim` and `release` had the same shape —
        // the flag never reached the handler — and only reach a refusal here,
        // which goes to standard error and leaves this looking satisfied.
        vec![
            "stand-down",
            "--reason",
            "a fixture",
            "--minutes",
            "5",
            "--json",
        ],
        vec!["stand-down", "--lift", "--reason", "done", "--json"],
        vec![
            "claim",
            "7",
            "--run-id",
            "claude-nothing0",
            "--horizon",
            "2099-01-01T00:00Z",
            "--json",
        ],
        // Last on purpose, and not a dry run. It is the one command here with a
        // second half to print — the state it takes out once no agent is left —
        // and it is the one that was printing that half outside the document.
        // Anywhere but last and it leaves the machine bare for the rest.
        // All three of `guard`'s answers, none of them a dry run: it writes the
        // hook, says it is already current, and takes it out again. Every one
        // of them printed prose.
        vec!["guard", "--json"],
        vec!["guard", "--json"],
        vec!["guard", "--uninstall", "--json"],
        vec!["guard", "--uninstall", "--json"],
        vec!["uninstall", "--all", "--json"],
    ] {
        // `guard` answers about the checkout it stands in, so it is the one
        // command here that is run from the repository rather than the home.
        let here = if arguments.first() == Some(&"guard") {
            repository.path()
        } else {
            home.path()
        };
        let (out, _, _) = run_in(home.path(), here, &arguments, "");
        let named = arguments.join(" ");
        let trimmed = out.trim();
        if trimmed.is_empty() {
            // Saying nothing on standard output is allowed; saying prose is not.
            continue;
        }
        serde_json::from_str::<serde_json::Value>(trimmed).unwrap_or_else(|error| {
            panic!(
                "`estigia {named}` printed something that is not JSON: {error}
{trimmed}"
            )
        });
    }
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
    run(home.path(), &["setup", "claude-code"], "");

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
            "| Tracker | linear | github |",
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
    run(home.path(), &["setup", "claude-code"], "");

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

#[test]
fn the_skill_left_for_another_agent_is_not_reported_as_the_operator_s() {
    // The note above answers *did it touch my things?*, and it answers it by
    // walking the skill directory after the removal and calling whatever is
    // still there the operator's. That premise — "everything of ours is gone by
    // now" — is true for the three adapters with a root of their own, and false
    // for the eight that share `~/.agents/skills`: their skill is deliberately
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
    run(home.path(), &["setup", "opencode"], "");
    run(home.path(), &["setup", "cursor"], "");

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
    for ours in ["SKILL.md", "github.py", "installed.json", "blind-judges.md"] {
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
    run(blank.path(), &["setup", "claude-code"], "");
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
    run(home.path(), &["setup", "claude-code"], "");

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
    std::fs::write(
        &mine,
        format!("{table}| Merge strategy | rebase | merge commit |\n"),
    )
    .expect("the override is written");
    let (listed, _, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        ok && listed.contains("rebase"),
        "a readable override was not honoured: {listed}"
    );

    // The older spelling still works on its own. It is the reason the fallback
    // exists, and a fix that took it out would break every installation that
    // came from issue-flow.
    std::fs::remove_file(&mine).expect("the newer one goes away");
    std::fs::write(
        &older,
        format!("{table}| Merge strategy | squash | merge commit |\n"),
    )
    .expect("the legacy override is written");
    let (legacy, _, ok) = run(home.path(), &["config", "list"], "");
    assert!(
        ok && legacy.contains("squash"),
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
        !out.contains("squash") && !out.contains("merge commit"),
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
        ok && plain.contains("merge commit"),
        "a machine with no override file at all was refused: {plain}"
    );
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
    let ledger = state.join("decisions.jsonl");

    // Declaring over one nothing can parse.
    std::fs::write(&record, "not a record").expect("a corrupt record");
    let (_, _, ok) = run(
        home.path(),
        &["stand-down", "--reason", "over a broken one"],
        "",
    );
    assert!(ok, "declaring over an unreadable record was refused");
    let written = std::fs::read_to_string(&ledger).expect("the ledger");
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
    // `config set` wrote the versioned table and said "Merge strategy is now
    // squash". The operator's own `estigia.local.md` overrides row for row, so
    // the value they read was still `rebase` — a confirmation for something that
    // did not happen, in the configuration of a tool whose entire purpose is
    // refusing exactly that.
    //
    // The fix is issue-flow's own move: write, then read back what the operator
    // will read, and believe the readback.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

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
| Merge strategy | rebase |          merge commit |
",
    )
    .expect("the operator's own file");

    let (_, err, ok) = run(
        home.path(),
        &["config", "set", "Merge strategy", "squash"],
        "",
    );
    assert!(!ok, "a shadowed row was reported as set");
    assert!(err.contains("setting-shadowed-by-local-file"), "{err}");
    // And it names the file, because that is the only place the operator can
    // act — Estigia does not edit a file it was told not to touch.
    assert!(err.contains("estigia.local.md"), "{err}");

    // With the override gone, the ordinary path is unchanged.
    std::fs::remove_file(&local).expect("remove the override");
    let (out, _, ok) = run(
        home.path(),
        &["config", "set", "Merge strategy", "squash"],
        "",
    );
    assert!(ok, "an unshadowed row stopped working");
    assert!(out.contains("squash"), "{out}");
}

#[test]
fn what_config_set_writes_for_one_agent_is_what_config_list_reads_back() {
    // Eight of the eleven adapters share one skill root, so an adapter's own
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
        &["config", "set", "Planning", "sdd", "--agent", "opencode"],
        "",
    );
    assert!(ok, "the write failed: {stderr}");
    assert!(out.contains("sdd"), "the write said nothing: {out}");

    assert_eq!(
        planning_of(home.path(), "opencode"),
        "sdd",
        "written for opencode and read back as something else"
    );

    // And it is that agent's answer, not everybody's: the adapter beside it on
    // the same root still reads the shipped default.
    assert_eq!(
        planning_of(home.path(), "cursor"),
        "direct",
        "one agent\'s answer moved another agent that shares its skill root"
    );
}

/// One agent's planning setting, as `config list` reports it.
///
/// The **value** column, not the line. Every row ends with `accepts:` and the
/// full list of what it accepts, so a test asking whether the line contains
/// `sdd` is answered `yes` by a row whose value is `direct` — which
/// is how the first draft of this guard passed against the fault it was written
/// for.
fn planning_of(home: &std::path::Path, agent: &str) -> String {
    let (out, _, ok) = run(home, &["config", "list", "--agent", agent], "");
    assert!(ok, "`config list --agent {agent}` failed");
    let row = out
        .lines()
        .find(|line| line.starts_with("Planning"))
        .unwrap_or_else(|| panic!("no such row: {out}"));
    let (value, _) = row
        .split_once("accepts:")
        .unwrap_or_else(|| panic!("no accepts column to cut at: {row}"));
    value.trim_start_matches("Planning").trim().to_owned()
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
         | Planning | sdd | direct |\n<!-- estigia:config:end -->\n",
    )
    .expect("the operator writes their own file");

    let (_, stderr, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Planning",
            "sdd openspec",
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
        &["config", "set", "Tracker", "linear", "--agent", "opencode"],
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
        &["config", "set", "Planning", "sdd", "--agent", "opencode"],
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
         | Merge strategy | octopus |\n<!-- estigia:config:end -->\n",
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

    let ledger = home.path().join(".estigia").join("decisions.jsonl");
    let lines = std::fs::read_to_string(&ledger).unwrap_or_default();
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
    let before = std::fs::read_to_string(&ledger).unwrap_or_default();
    let (_, _, ok) = run(
        home.path(),
        &["hook", "session-end"],
        r#"{"session_id":"abc123"}"#,
    );
    assert!(ok);
    assert_eq!(
        std::fs::read_to_string(&ledger).unwrap_or_default(),
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
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repository.path())
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let ready = git(&["init", "--quiet"])
        && git(&["config", "user.email", "push@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);
    assert!(ready, "git is not usable here, and this needs a checkout");

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
    let git = |at: &std::path::Path, arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(at)
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let ready = git(bare.path(), &["init", "--quiet", "--bare"])
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
    assert!(
        ready,
        "git is not usable here, and this needs two repositories"
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
        &["config", "set", "Planning", "sdd", "--agent", "opencode"],
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
        &["config", "set", "Merge strategy", "squash", "--repo"],
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
fn a_guard_under_a_tracker_with_no_transport_does_not_promise_refusals() {
    // The second reason a guard can be inert, and it says the opposite.
    //
    // `Tracker` accepts `linear` and `trello`, and both ship a binding the agent
    // reads and nothing that answers — `Tracker::transport()` returns `None`, so
    // `claim` refuses with `tracker-has-no-transport` and **no run can ever hold
    // an issue on this machine**. `estigia guard` writes the hook anyway and
    // prints its caveats: *"a push from a checkout a live claim holds is refused
    // unless that claim justifies it"* about a claim that cannot exist, and
    // *"this refuses nothing until `estigia claim` has been run"* — naming a
    // command that cannot succeed here, which is the one thing the ratchet
    // forbids.
    //
    // Then the push leaves in silence, the way it did for the missing skill one
    // round ago and for the same reason: the decision is `Outside`, which is
    // indistinguishable from *nobody has claimed yet*.
    //
    // Writing the hook stays right — a tracker is a row somebody can change, and
    // the guard is then already there. What changes is that both moments say the
    // gate is adjudicating nothing.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repository = tempfile::tempdir().expect("a repository");
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(repository.path())
            .args(arguments)
            .output()
            .is_ok_and(|out| out.status.success())
    };
    let ready = git(&["init", "--quiet"])
        && git(&["config", "user.email", "linear@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);
    assert!(ready, "git is not usable here, and this needs a checkout");

    run(home.path(), &["setup", "claude-code"], "");
    let (_, said, ok) = run(home.path(), &["config", "set", "Tracker", "linear"], "");
    assert!(ok, "the tracker could not be set: {said}");

    // The floor: this really is a machine where nothing can swear.
    let (_, refused, ok) = run(
        home.path(),
        &[
            "claim",
            "7",
            "--run-id",
            "claude-aaaaaaaa",
            "--horizon",
            "2099-01-01T00:00Z",
        ],
        "",
    );
    assert!(
        !ok && refused.contains("tracker-has-no-transport"),
        "{refused}"
    );

    let (out, _, ok) = run_in(home.path(), repository.path(), &["guard"], "");
    assert!(
        ok && out.contains("pre-push"),
        "the guard did not install: {out}"
    );
    assert!(
        out.contains("adjudicates nothing") || out.contains("cannot swear"),
        "it wrote a guard that can never refuse and said nothing about it: {out}"
    );

    // And the push says the same thing rather than leaving quietly.
    let (out, said, ok) = run_in(home.path(), repository.path(), &["hook", "pre-push"], "");
    assert!(
        ok,
        "the push was blocked, which this boundary must not do: {said}"
    );
    let both = format!("{out}{said}");
    assert!(
        both.contains("not checked") || both.contains("adjudicates nothing"),
        "the push went out unchecked and nothing said so: {both}"
    );
}

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
    // `push-guard`, `gate`, `tools`, `transport` — are named in the README's
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
    run(home.path(), &["setup", "claude-code"], "");

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
    } else {
        eprintln!("SKIPPED: git is not usable here, so two of the rows were not forced.");
    }

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
    );
    assert!(
        out.contains("BROKEN   gh"),
        "a machine with no GitHub CLI did not report the gh row broken:\n{out}"
    );

    // And the population: every row this build can print is one of the states
    // above, or `transport`, which has no broken state to force — it answers
    // `ok`, or `skipped` for a tracker with no executable. A row added later
    // fails here until somebody either forces it or says why they cannot.
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
        "contract",
        "gate",
        "tools",
        "gh",
        "push-guard",
        "remote",
        "stand-down",
        "run-pointer",
        "silence",
        // No broken state at all, measured: `Health::Fine` or `Health::Skipped`.
        "transport",
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
    let ledger = home.path().join(".estigia").join("decisions.jsonl");
    std::fs::write(
        &ledger,
        "{\"verdict\":\"payload-absent\",\"at\":1785904685,\"event\":\"pre-tool-use\"}\n",
    )
    .expect("the ledger");
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
    let (_, stderr, ok) = run(home.path(), &["setup", "--companion", "engram"], "");
    assert!(!ok, "an unknown companion was accepted");
    assert!(
        stderr.contains("engram") && stderr.contains("leteo"),
        "the refusal names neither what was asked for nor what exists: {stderr}"
    );
}

#[test]
fn a_worktree_answers_what_its_repository_answers() {
    // This crate's own delivery topology works in worktrees, and a linked
    // worktree's `.git` is a *file*. Joining `.git/estigia/...` onto it gave a
    // path nothing could be written under, and two things followed: `config set
    // --repo` refused with *os error 3* and sent the operator to `estigia
    // doctor`, which fixes nothing about it; and the same repository answered
    // `Merge strategy = squash` in the checkout and `merge commit` inside its
    // own worktree — silently, because the file the worktree looked for could
    // not exist.
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
        &["config", "set", "Merge strategy", "squash", "--repo"],
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
        &["config", "set", "Merge strategy", "rebase", "--repo"],
        "",
    );
    assert!(
        ok,
        "the worktree could not set its repository's row: {stderr}"
    );
    assert!(
        answer(&repo).contains("rebase"),
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

    let ledger = std::fs::read_to_string(home.path().join(".estigia").join("decisions.jsonl"))
        .expect("the ledger was written");
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
fn a_blank_run_id_is_refused_before_anything_is_read() {
    // `clap` checks that a flag is present, not that it says anything, so
    // `--run-id ""` satisfied it and went all the way to the tracker. What it
    // would have recorded is a claim under an empty id — exactly "a claim the
    // gate will never match", which is the reason that flag's own help text
    // gives for asking rather than guessing.
    //
    // Run as a process because the fault is `clap`'s to raise, before any of
    // this crate's code sees the arguments: the rule the rest of it states as
    // *everything knowable from the command line is settled before anything on
    // disk is read*.
    let home = tempfile::tempdir().expect("a temporary home");
    for blank in ["", "   "] {
        for arguments in [
            vec![
                "claim",
                "1",
                "--run-id",
                blank,
                "--horizon",
                "2026-12-31T23:00Z",
            ],
            vec!["release", "--run-id", blank],
            vec!["gate", "Write", "--run-id", blank],
        ] {
            let verb = arguments[0];
            let (_, stderr, ok) = run(home.path(), &arguments, "");
            assert!(!ok, "`{verb}` took a blank run id");
            assert!(
                stderr.contains("cannot be blank"),
                "`{verb}` refused a blank run id for some other reason: {stderr}"
            );
            // And it says where a real one comes from, because the operator is
            // usually an agent that was handed one at `SessionStart`.
            assert!(
                stderr.contains("SessionStart"),
                "`{verb}` does not say where a run id comes from: {stderr}"
            );
        }
    }
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
fn a_run_pointer_nobody_can_read_is_not_a_run_that_holds_nothing() {
    // `session::load` keeps these apart on purpose — a pointer that is present
    // and unparseable sets `unreadable`, because "an unknown is not clearance"
    // and the gate has to fail closed on it. `release` read them back as one
    // and answered `holds no issue`, which is a statement of fact about
    // something Estigia does not know: the tracker may still show the issue
    // held by that run, and an operator told there was nothing to put down
    // would leave it there.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // A run that never swore anything.
    let (_, stderr, ok) = run(home.path(), &["release", "--run-id", "never-claimed"], "");
    assert!(!ok);
    assert!(
        stderr.contains("nothing-held"),
        "a run with no pointer is not an unreadable one: {stderr}"
    );

    // A run whose pointer is there and will not parse.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    std::fs::write(runs.join("broken.json"), "not json").expect("a damaged pointer");
    let (_, stderr, ok) = run(home.path(), &["release", "--run-id", "broken"], "");
    assert!(!ok);
    assert!(
        stderr.contains("run-pointer-unreadable"),
        "an unreadable pointer was reported as nothing held: {stderr}"
    );
    // It names the file, because removing it is the operator's move and they
    // cannot make it without knowing which one.
    assert!(
        stderr.contains("broken.json"),
        "it does not say which pointer: {stderr}"
    );
    // And no command, because none of Estigia's would discharge it.
    assert!(
        !stderr.contains("run: estigia"),
        "it names a command that changes nothing: {stderr}"
    );
}

#[test]
fn what_sync_tells_a_person_it_also_tells_a_script() {
    // `sync` says that a skill root with no contract took the defaults rather
    // than inheriting an answer, because there was nothing unambiguous to
    // inherit. It said it on standard output, so under `--json` — where that
    // channel has to stay parseable — it could only be suppressed, and it was:
    // the operator was told and the script was told nothing.
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
    let (_, stderr, ok) = run(
        home.path(),
        &["config", "set", "Planning", "sdd", "--agent", "codex"],
        "",
    );
    assert!(ok, "the fixture could not disagree: {stderr}");
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
            "{mode:?} does not say the agents disagree: {stderr}"
        );
        assert!(
            stderr.contains("had none"),
            "{mode:?} does not name the root that inherited nothing: {stderr}"
        );
        if mode.contains(&"--json") {
            serde_json::from_str::<serde_json::Value>(&out)
                .expect("the document on standard output is still a document");
        }
    }
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
             |---|---|---|\n| Planning | sdd | direct |\n<!-- estigia:config:end -->\n\nMy rules.\n",
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
        &["config", "set", "Planning", "SDD", "--agent", "opencode"],
        "",
    );
    assert!(ok, "the write was refused");
    assert!(
        out.contains("\"sdd\"") && !out.contains("\"SDD\""),
        "the per-agent write echoed the argument instead of the value: {out}"
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
        vec!["config", "set", "Planning", "sdd"],
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
    // defaults over every choice in the file it does write. `squash` and
    // `sdd openspec` came back `merge commit` and `direct`, under one line
    // reading `update`.
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "codex"], "");
    assert!(ok, "the install failed: {stderr}");
    for (setting, value) in [("Merge strategy", "squash"), ("Planning", "sdd openspec")] {
        let (_, stderr, ok) = run(home.path(), &["config", "set", setting, value], "");
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
        before.contains("| Merge strategy | squash |"),
        "the premise is wrong: the choice was never written"
    );

    // A value the parser refuses, in the operator's own file.
    std::fs::write(
        skill.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Merge strategy | octopus |\n",
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

/// Every file under `root`, by path.
fn every_file(root: &std::path::Path) -> std::collections::BTreeSet<std::path::PathBuf> {
    let mut found = std::collections::BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.insert(path);
            }
        }
    }
    found
}

/// The paths a run's report names, whatever it says it did to them.
fn named_in(report: &str) -> std::collections::BTreeSet<std::path::PathBuf> {
    report
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let rest = ["create ", "update ", "replace ", "remove "]
                .iter()
                .find_map(|verb| line.strip_prefix(verb))?;
            Some(std::path::PathBuf::from(rest.trim()))
        })
        .collect()
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
        &["config", "set", "Planning", "sdd openspec"],
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
        contract.contains("sdd openspec"),
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
            "Planning",
            "sdd openspec",
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
    // each row, so `sdd openspec` appears in `Planning`'s help whatever is held.
    let planning_of = |slug: &str| -> String {
        let (out, _, _) = run(
            home.path(),
            &["config", "list", "--agent", slug, "--json"],
            "",
        );
        let rows: serde_json::Value = serde_json::from_str(&out).expect("config list prints JSON");
        rows.as_array()
            .expect("an array of rows")
            .iter()
            .find(|row| row["setting"] == "Planning")
            .expect("Planning is a setting")["value"]
            .as_str()
            .expect("a value")
            .to_owned()
    };
    assert_eq!(planning_of("codex"), "sdd openspec", "the premise moved");
    assert_eq!(
        planning_of("claude-code"),
        "direct",
        "a per-agent answer was copied onto another agent by a repository-wide write"
    );
}

#[test]
fn a_sub_agent_definition_nothing_can_read_does_not_become_a_permission() {
    // The search read each candidate with `.ok()` and stepped over the ones that
    // failed. Four roots are searched, so a definition that is **there and will
    // not open** either handed the role to a file further down the list or ran
    // out — and running out is `None`, which the frontmatter reader spells out
    // means *no policy at all: the sub-agent may use every tool*.
    //
    // So the one gate that depends on nothing else — no claim, no state, no
    // window, only on what the author wrote — turned an unreadable file into
    // clearance. The same harm that file already records for a byte-order mark:
    // "a definition that restricted a sub-agent stopped restricting it because
    // of a character nobody typed."
    //
    // Run through the binary, because the loss was in the search and the gate
    // above it, and each of them reads correctly on its own.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");
    let writing = r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#;

    // The floor first, both ways: a readable definition still denies past its
    // list, and a sub-agent nobody wrote a definition for is still allowed
    // through. A fix that denied everything would satisfy the assertion below
    // and break the product.
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\nRead the code.\n",
    )
    .expect("a definition");
    let (denied, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        denied.contains("tool-outside-declared-role"),
        "a readable definition stopped being enforced: {denied}"
    );

    let (nobody, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(
            r#""agent_type":"unwritten","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
    );
    assert!(
        !nobody.contains("agent-definition-unreadable"),
        "a sub-agent nobody declared was refused as unreadable: {nobody}"
    );

    // And now the definition is there and will not open. A directory in its
    // place fails the read with something other than `NotFound`, on every
    // platform.
    std::fs::remove_file(agents.join("reader.md")).expect("clear the way");
    std::fs::create_dir_all(agents.join("reader.md")).expect("something unreadable in its place");

    let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        out.contains("\"permissionDecision\":\"deny\""),
        "a sub-agent whose declared limits nothing could read was let through: {out}"
    );
    assert!(
        out.contains("agent-definition-unreadable"),
        "the denial did not say which question went unanswered: {out}"
    );
    assert!(
        out.contains("operator-knowledge") && out.contains("readable"),
        "the denial does not carry what to do about it: {out}"
    );

    // Even a tool the readable definition would have allowed: the point is that
    // what it declared is unknown, not that this particular call was outside it.
    let (reading, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(r#""agent_type":"reader","tool_name":"Read","tool_input":{"file_path":"x.rs"}"#),
    );
    assert!(
        reading.contains("agent-definition-unreadable"),
        "a call was judged against limits nothing had read: {reading}"
    );
}

/// A stand-down reaches the unreadable definition, and not the author's list.
///
/// Two refusals sit on the same three lines of `PreToolUse`, and they are not
/// the same kind of no.
///
/// `agent-definition-unreadable` is Estigia's own *an unknown result is not
/// clearance*, and it had `decide_action`'s shape until now: a definition file
/// that will not open denies **every** call that sub-agent makes, and the one
/// command for getting past a gate that is wrong at a bad moment did not reach
/// it — a file on a read-only mount, or one somebody else owns, left an operator
/// with nothing to do but stop.
///
/// `tool-outside-declared-role` enforces a list the sub-agent's *author* wrote.
/// An operator standing Estigia's gate down does not thereby grant permissions
/// somebody else withheld, so it stays refused — and that is asserted here
/// rather than left to be noticed, because an untested asymmetry is one somebody
/// closes by accident.
#[test]
fn a_stand_down_reaches_an_unreadable_definition_and_not_the_authors_list() {
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\nRead the code.\n",
    )
    .expect("a definition");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");
    let writing = r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#;

    // The author's list, with a stand-down in force. Still refused.
    let (denied, _, _) = run_with_stand_down(home.path(), &payload(writing));
    assert!(
        denied.contains("tool-outside-declared-role"),
        "a stand-down granted a permission the sub-agent's author withheld: {denied}"
    );

    // And the definition nothing can read, with the same stand-down. Through.
    std::fs::remove_file(agents.join("reader.md")).expect("clear the way");
    std::fs::create_dir_all(agents.join("reader.md")).expect("something unreadable in its place");
    let (allowed, _, _) = run_with_stand_down(home.path(), &payload(writing));
    assert!(
        allowed.contains("\"permissionDecision\":\"allow\""),
        "a declared stand-down did not reach a refusal Estigia raised about its own read: \
         {allowed}"
    );
    // Naming what it overrode is the whole difference between this and a
    // switch: a trace reading "allowed" and one reading "allowed under a
    // stand-down, over `agent-definition-unreadable`" are not the same record,
    // and only the second can be answered for afterwards.
    assert!(
        allowed.contains("stood down") && allowed.contains("agent-definition-unreadable"),
        "the allowance does not say what it let through: {allowed}"
    );

    // Without one it is still refused, or the assertion above is about nothing.
    let (still, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(writing));
    assert!(
        still.contains("agent-definition-unreadable"),
        "an unreadable definition stopped being refused at all: {still}"
    );
}

/// [`run`], with a stand-down declared first.
fn run_with_stand_down(home: &std::path::Path, payload: &str) -> (String, String, bool) {
    run(
        home,
        &[
            "stand-down",
            "--reason",
            "the definition is not mine to fix",
        ],
        "",
    );
    run(home, &["hook", "pre-tool-use"], payload)
}

#[test]
fn a_sub_agent_reaching_past_its_own_tool_list_is_refused_by_the_process() {
    // The role gate is the one enforcement point that depends on nothing —
    // no claim, no state, no window, only on what the author of the sub-agent
    // wrote. It was exercised in unit tests and had never been run **through
    // the binary**, which is where the payload's field names, the definition
    // search and the dialect's answer shape all have to agree at once.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ndescription: reads and nothing else\ntools: Read, Grep, Glob\n---\nRead the code.\n",
    )
    .expect("a sub-agent definition");

    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = |body: &str| format!("{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",{body}}}");

    // Outside the list it declares: denied, and the refusal carries the four
    // things every refusal here carries.
    let (out, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(r#""agent_type":"reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#),
    );
    assert!(
        out.contains("\"permissionDecision\":\"deny\"")
            && out.contains("tool-outside-declared-role"),
        "a sub-agent wrote past its own tool list: {out}"
    );
    assert!(
        out.contains("nothing was written") && out.contains("operator-knowledge"),
        "the denial does not carry what to do about it: {out}"
    );

    // The alias, because the field arrives under three spellings and only one
    // of them was ever typed into a test.
    let (out, _, _) = run(
        home.path(),
        &["hook", "pre-tool-use"],
        &payload(
            r#""subagent_type":"reader","tool_name":"Bash","tool_input":{"command":"rm -rf x"}"#,
        ),
    );
    assert!(
        out.contains("tool-outside-declared-role"),
        "`subagent_type` is declared an alias and reads as nothing: {out}"
    );

    // And the three ways this must **not** deny. A role gate that refused any
    // of them would be a harness people take out.
    for (why, body) in [
        (
            "a tool the role declares",
            r#""agent_type":"reader","tool_name":"Read","tool_input":{"file_path":"x.rs"}"#,
        ),
        (
            "the main conversation, which no role covers",
            r#""tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
        (
            // Not enforcement from a file outside the agents directory: the
            // name arrives from the host and lands in a path, and a list found
            // somewhere else would be enforced as though somebody had written
            // it for this role.
            "a role name that tries to leave the directory",
            r#""agent_type":"../../reader","tool_name":"Write","tool_input":{"file_path":"x.rs"}"#,
        ),
    ] {
        let (out, _, _) = run(home.path(), &["hook", "pre-tool-use"], &payload(body));
        assert!(
            !out.contains("deny"),
            "{why} was refused by the role gate: {out}"
        );
    }
}

#[test]
fn every_dialect_says_no_in_the_shape_its_agent_reads() {
    // Five shapes, and the flag's own help says what getting one wrong costs:
    // *a hook that runs, decides correctly, and is ignored — which reports
    // success and enforces nothing*. Every one of them had unit tests and none
    // had ever been produced **by the process**, where the payload's fields,
    // the definition search and the answer shape all have to agree at once.
    //
    // The role gate is the vehicle because it denies without a tracker, a
    // claim, a state or a window — the one deterministic refusal this suite can
    // ask for.
    let home = tempfile::tempdir().expect("a temporary home");
    let agents = home.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("the agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ndescription: reads\ntools: Read, Grep, Glob\n---\nRead.\n",
    )
    .expect("a sub-agent definition");
    let cwd = home.path().display().to_string().replace('\\', "\\\\");
    let payload = format!(
        "{{\"session_id\":\"s1\",\"cwd\":\"{cwd}\",\"agent_type\":\"reader\",\
         \"tool_name\":\"Write\",\"tool_input\":{{\"file_path\":\"x.rs\"}}}}"
    );

    // Each dialect's own spelling of no, from the dialect rather than from a
    // list written here: a variant added to the enum arrives with its slug and
    // this has to be told what it looks like.
    for (slug, marker) in [
        ("claude-code", "\"permissionDecision\":\"deny\""),
        ("gemini-cli", "\"decision\":\"deny\""),
        ("cursor", "\"permission\":\"deny\""),
        ("cline", "\"review\":true"),
    ] {
        let (out, _, ok) = run(
            home.path(),
            &["hook", "pre-tool-use", "--dialect", slug],
            &payload,
        );
        assert!(ok, "{slug}: the hook did not exit zero");
        assert!(
            out.contains(marker),
            "{slug} was answered in somebody else's shape: {out}"
        );
    }

    // The odd one out, and the expensive one to get wrong: not JSON at all.
    // The agent reads the status, and the reason goes to standard error.
    let (out, stderr, ok) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "exit-code"],
        &payload,
    );
    assert!(!ok, "an exit-code agent was told to proceed");
    assert!(
        out.trim().is_empty(),
        "a decision was printed where a status was expected: {out}"
    );
    assert!(
        stderr.contains("tool-outside-declared-role"),
        "the reason reached nobody: {stderr}"
    );

    // A slug this build does not have falls back — deliberately, so a renamed
    // dialect does not block every edit — **and says so**. Silence here is the
    // one that costs: the fallback shape is JSON, and an agent that reads a
    // status gets a refusal that prints itself and lets the write through.
    let (out, stderr, _) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "windsurf"],
        &payload,
    );
    assert!(
        out.contains("\"permissionDecision\":\"deny\""),
        "the fallback stopped deciding: {out}"
    );
    assert!(
        stderr.contains("not a dialect this build knows"),
        "an unknown dialect was answered in another's shape without a word: {stderr:?}"
    );

    // And an ordinary run stays quiet, or the note becomes noise nobody reads.
    let (_, stderr, _) = run(
        home.path(),
        &["hook", "pre-tool-use", "--dialect", "claude-code"],
        &payload,
    );
    assert!(
        !stderr.contains("not a dialect"),
        "a known dialect was complained about: {stderr}"
    );
}

#[test]
fn a_session_that_never_said_whose_it_was_forgets_nobody() {
    // `SessionEnd` is the hook's one destructive act, and the name it destroys
    // by is derived from the payload. `session::run_id` answers a session with
    // no identity with `<runtime>-unknown` — which is **a name two runs share**
    // rather than a run. A payload this build cannot parse produces it, and
    // this machine's own ledger carries four entries under it.
    //
    // So a `SessionEnd` with no payload deleted `claude-unknown.json`. If the
    // run that wrote it is still working, the gate then reads `issue: None` and
    // every write it makes goes through ungated while the tracker still shows
    // the issue held — a run that swore reading as one that never did.
    //
    // Through the process, because that is where the payload becomes a name.
    let home = tempfile::tempdir().expect("a temporary home");
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    let nameless = runs.join("claude-unknown.json");
    let named = runs.join("claude-abc12345cc1e83a2.json");
    let write = |path: &std::path::Path, id: &str, issue: u64| {
        std::fs::write(
            path,
            format!("{{\"run_id\":\"{id}\",\"issue\":{issue},\"revision\":1}}"),
        )
        .expect("a run pointer");
    };
    write(&nameless, "claude-unknown", 12);
    write(&named, "claude-abc12345cc1e83a2", 7);

    // No payload at all, and a session id that is only spaces. Neither says
    // whose session this is, and neither may take somebody's claim with it.
    for payload in ["", r#"{"session_id":"   "}"#] {
        run(home.path(), &["hook", "session-end"], payload);
        assert!(
            nameless.is_file(),
            "a session that named nobody forgot the run that had no name either: {payload:?}"
        );
        assert!(
            named.is_file(),
            "and took an unrelated one with it: {payload:?}"
        );
    }

    // And a session that **does** say whose it is still forgets its own — the
    // pointer goes, the claim does not. A guard that stopped this would leave a
    // pointer behind for every session that ever ended.
    run(
        home.path(),
        &["hook", "session-end"],
        r#"{"session_id":"abc12345"}"#,
    );
    assert!(
        !named.exists(),
        "a session that identified itself did not forget its own run"
    );
    assert!(
        nameless.is_file(),
        "forgetting one run took the nameless one with it"
    );
}

#[test]
fn a_session_with_no_identity_is_told_so_rather_than_handed_a_shared_name() {
    // `session::run_id` answers a session with no identity with
    // `<runtime>-unknown`, and `SessionStart` handed that back as *Run id
    // `claude-unknown`.* — a name every unidentifiable session on the machine
    // is given, presented as though it were one run's.
    //
    // Two of them swearing under it means the second overwrites the first's
    // pointer, and the gate goes on measuring the first's writes against the
    // second's issue. The `--run-id` flag's own help states the rule: *a claim
    // recorded under the wrong run-id is a claim the gate will never match, and
    // being asked beats being silently wrong.*
    let home = tempfile::tempdir().expect("a temporary home");
    run(home.path(), &["setup", "claude-code"], "");

    // With an identity: the run id is named, because an agent needs it to
    // claim with.
    let (out, _, _) = run(
        home.path(),
        &["hook", "session-start"],
        r#"{"session_id":"abc12345"}"#,
    );
    assert!(
        out.contains("Run id `claude-abc12345"),
        "an identified session was not given its run id: {out}"
    );

    // Without one: the fact, not a name. An agent told it has no identity can
    // ask for one; an agent told it is `claude-unknown` cannot know to.
    let (out, _, _) = run(home.path(), &["hook", "session-start"], "{}");
    assert!(
        !out.contains("claude-unknown"),
        "a shared name was handed out as an identity: {out}"
    );
    assert!(
        out.contains("no identity") && out.contains("nothing can be sworn"),
        "the session was not told why it cannot swear: {out}"
    );

    // And the same rule at the other door, for an agent that has the name from
    // somewhere else: swearing under it is refused before anything is written.
    let (_, stderr, ok) = run(
        home.path(),
        &[
            "claim",
            "7",
            "--run-id",
            "claude-unknown",
            "--horizon",
            "2099-01-01T00:00Z",
        ],
        "",
    );
    assert!(!ok, "a claim under a name no run owns was accepted");
    assert!(
        stderr.contains("run-id-names-no-run"),
        "it was refused for some other reason: {stderr}"
    );
}

/// An instruction file left behind on an upgrade is named, not just left.
///
/// The record gained a second set in the round that stopped uninstalls deleting
/// a `CLAUDE.md` the operator had kept empty. Every record already on disk is
/// the older shape, which names no instruction file — so on the first uninstall
/// after an upgrade every one of them reads as the operator's and is emptied
/// rather than removed. That is the safe direction and it was chosen; what it
/// did not come with was a word to the operator, who got a stray empty file and
/// a line saying `update`.
///
/// Both halves, because a note that always fires is noise: an ordinary install
/// records what it created and its uninstall takes the file, saying nothing.
#[test]
fn an_instruction_file_kept_on_an_upgrade_is_named_in_the_report() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let record = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join(".estigia")
        .join("installed.json");
    let held: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&record).expect("a record")).expect("json");
    // Exactly what a build from before the second set wrote: a bare array.
    let older = serde_json::to_string_pretty(held.get("created").expect("the created set"))
        .expect("serialises");
    std::fs::write(&record, older).expect("an older record");

    let (out, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(ok, "uninstall failed: {stderr}");
    let said = format!("{out}{stderr}");
    assert!(
        said.contains("instruction file(s) are empty and still on disk"),
        "an empty file was left with no word about it: {said}"
    );
    let left = home.path().join(".claude").join("CLAUDE.md");
    assert!(
        left.is_file() && std::fs::metadata(&left).expect("readable").len() == 0,
        "the fixture did not leave the file this note is about"
    );

    // And the ordinary path stays quiet: the record names the file, so it goes.
    let home = tempfile::tempdir().expect("a second home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");
    let (out, stderr, ok) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(ok, "uninstall failed: {stderr}");
    assert!(
        !format!("{out}{stderr}").contains("instruction file(s) are empty"),
        "a note about a leftover fired on an uninstall that left nothing"
    );
    assert!(
        !home.path().join(".claude").join("CLAUDE.md").exists(),
        "the recorded file was not removed"
    );
}

/// No variable this crate decides on reaches a child from the developer's shell.
///
/// `run` points four variables at the fixture's home and clears everything named
/// `ESTIGIA_*`. The second half is the one worth holding: `ESTIGIA_FLAG` is read
/// straight from the environment and decides whether `unflagged-on-trunk` fires,
/// so a child that inherits it answers a question about whoever ran the suite.
/// Nothing here reaches a trunk delivery today, so the suite passes either way —
/// which is exactly why this is a source crossing rather than a behaviour one.
/// There is no surface that reports the flag back without a live tracker.
///
/// What it catches is the next one: a variable this crate reads to decide
/// something, named outside the namespace the fixture clears.
#[test]
fn no_variable_the_crate_decides_on_escapes_the_fixture() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![root];
    let mut read: Vec<String> = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|kind| kind != "rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for marker in ["var(\"", "var_os(\""] {
                let mut rest = text.as_str();
                while let Some(at) = rest.find(marker) {
                    rest = &rest[at + marker.len()..];
                    let Some(end) = rest.find('"') else { break };
                    let name = &rest[..end];
                    if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') && !name.is_empty()
                    {
                        read.push(name.to_owned());
                    }
                }
            }
        }
    }
    read.sort();
    read.dedup();

    // The ones the fixture answers for, and the ones it deliberately leaves
    // real. Everything else has to be inside the namespace it clears.
    const ANSWERED: &[&str] = &["HOME", "USERPROFILE", "APPDATA", "XDG_CONFIG_HOME"];
    const LEFT_REAL: &[&str] = &[
        // A path a child needs to find an interpreter at all.
        "PATH",
        "PATHEXT",
        // These end up in a record's prose — who declared a stand-down, which
        // machine it was — and decide nothing.
        "USER",
        "USERNAME",
        "LOGNAME",
        "COMPUTERNAME",
        "HOSTNAME",
        // Read only after `HOME` and `USERPROFILE` have both answered nothing,
        // and the fixture sets both.
        "HOMEDRIVE",
        "HOMEPATH",
        // **Written**, not read, and only inside the MCP server process: it is
        // how `gh` is told which repository the operator named, and the spawned
        // transport set it on the child for the same reason. Left real because
        // an operator who exported it in their own shell meant it — Estigia
        // overwrites it only when the table names a repository, and a fixture
        // that cleared it would hide the one case where the two disagree.
        "GH_REPO",
    ];
    for name in &read {
        if ANSWERED.contains(&name.as_str())
            || LEFT_REAL.contains(&name.as_str())
            || name.starts_with("ESTIGIA_")
        {
            continue;
        }
        panic!(
            "{name} is read from the environment and the fixture neither answers it nor clears \
             it, so a child sees whatever the developer's shell had — say which it is here"
        );
    }
    assert!(
        read.len() >= 10,
        "only {} variables were read out of the source, so this proves little",
        read.len()
    );
}

/// What `config set --repo` writes, `config list` reads back.
///
/// Measured by hand, on the installed binary: `config set --repo Tracker linear`
/// named the file it had written, and `config list` one command later answered
/// `github`. A write this tool confirmed and a read this tool contradicted, on
/// the same row, one command apart — which is word for word the defect
/// `contract_of` already records being fixed once, for `--agent`, and which had
/// come straight back through the door beside it.
///
/// As a process, because that is how it was found and because nothing smaller
/// sees it: the layering was right everywhere it was tested and absent from the
/// one command an operator uses to check their work.
#[test]
fn what_config_set_repo_writes_config_list_reads_back() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");

    // The floor: before anything is said about this checkout it answers with
    // the contract, so the assertion below is about the write and not about the
    // command printing the word at all.
    let (before, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    // The value column alone. The line ends in `accepts:` and every answer the
    // row takes, so a test that searched the whole line found `linear` in the
    // list of what is *accepted* and passed against the defect it was written
    // for — which is the shape of vacuity this crate keeps a floor against.
    let tracker = |out: &str| {
        out.lines()
            .find(|line| line.starts_with("Tracker "))
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(
        tracker(&before) == "github",
        "the contract's own answer is not what an unconfigured checkout reports: {before}"
    );

    let (said, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "--repo", "Tracker", "linear"],
        "",
    );
    assert!(ok, "the write was refused: {said}{error}");
    assert!(
        said.contains("linear"),
        "the write did not report itself: {said}"
    );

    let (after, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    assert!(
        tracker(&after) == "linear",
        "what was written into this checkout is not what reading it back reports: {after}"
    );
}

/// `doctor` checks the tracker **this checkout** uses.
///
/// It read the contract and not the layer, so a checkout that had chosen
/// another tracker was still checked against the GitHub CLI — and, with `gh`
/// unauthenticated, told a run it could not swear yet over a program that
/// checkout does not use. The resolution named `gh auth login`, which does not
/// discharge a block about a tracker nobody asked for: the ratchet, broken by a
/// value read one layer too low.
#[test]
fn doctor_checks_the_tracker_this_checkout_chose() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");
    let names_gh = |out: &str| {
        out.lines()
            .any(|line| line.split_whitespace().any(|word| word == "gh"))
    };

    // The floor: on the contract's own tracker, `gh` is one of the things it
    // checks. A doctor that had stopped reporting it at all would satisfy the
    // assertion below without reading anything.
    let (before, _, _) = run_in(home.path(), repo.path(), &["doctor"], "");
    assert!(
        names_gh(&before),
        "the GitHub CLI is not among the checks, so this measures nothing:\n{before}"
    );

    let (_, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "--repo", "Tracker", "trello"],
        "",
    );
    assert!(ok, "the write was refused: {error}");

    let (after, _, _) = run_in(home.path(), repo.path(), &["doctor"], "");
    assert!(
        !names_gh(&after),
        "a checkout that chose another tracker is still checked against the GitHub CLI:\n{after}"
    );

    // And it keeps checking that tracker when some **other** row will not parse.
    // `tracker_in_force` read the configuration strictly and fell back to
    // `Tracker::default()`, so one mistyped value anywhere in any of the three
    // documents put the doctor back on GitHub. Measured on the installed binary,
    // on a machine configured for Linear:
    //
    // ```text
    // $ estigia doctor
    // estigia: gh is not usable, so a run cannot swear yet: the GitHub CLI the
    //          transport reaches the tracker through (environment-not-ready)
    // ```
    //
    // A refusal, out of the command somebody runs *because* something is wrong —
    // and that function's own doc opens by saying it must never be one. It also
    // hides the real fault: the operator is shown a GitHub CLI they do not use
    // instead of the row they mistyped.
    std::fs::write(
        home.path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Renewal window | 30 days |\n",
    )
    .expect("the override is written");

    let (spoilt, spoilt_error, _) = run_in(home.path(), repo.path(), &["doctor"], "");
    assert!(
        !names_gh(&spoilt) && !names_gh(&spoilt_error),
        "one row that would not parse put the doctor back on a tracker this checkout does \
         not use:\n{spoilt}\n{spoilt_error}"
    );
    // The floor: the bad row really is there and really is reported, so a doctor
    // that had gone quiet would not pass this.
    assert!(
        spoilt.contains("Renewal window"),
        "the row that will not parse is not named anywhere:\n{spoilt}"
    );
    // And the summary sends the operator at the row, not at somebody else's
    // tooling. This is the sharp end of it: the blocking check used to be the
    // GitHub CLI, whose way out is *`gh auth login`, which needs a person at a
    // browser* — a command that could not clear a mistyped `Renewal window` if
    // it ran perfectly. Naming a dead end is the one thing the ratchet forbids.
    assert!(
        spoilt_error.contains("contract") && spoilt_error.contains("that row corrected"),
        "the blocking summary points somewhere other than the row:\n{spoilt_error}"
    );
}

/// Setting a row this checkout overrides says so.
///
/// `config set` writes the contract and reads back what a run will read, and it
/// refuses when the operator's own `estigia.local.md` shadows the row: *a value
/// written into a row that file also carries is a value nobody will ever read*.
/// The repository layer added later sits above both and that read-back never
/// learned about it — so, measured on the installed binary:
///
/// ```text
/// $ estigia config set "Merge strategy" rebase
/// Merge strategy is now rebase
/// $ estigia config list
/// Merge strategy    squash
/// ```
///
/// A write this tool confirmed and a read this tool contradicted, on the same
/// row, one command apart — the sentence `contract_of` already carries about
/// `--agent`, arriving through the door beside it.
///
/// **Not a refusal**, and that is the difference from the operator's file. That
/// one shadows the row in every checkout, so the write is in force nowhere.
/// This one shadows it *here*: the value is now the machine's answer everywhere
/// else, which is usually what was wanted. What was missing is the other half
/// of the sentence.
#[test]
fn setting_a_row_this_checkout_overrides_says_so() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a checkout with a git directory");
    let (_, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "--repo", "Merge strategy", "squash"],
        "",
    );
    assert!(ok, "the checkout would not take its own row: {error}");

    // The floor first: away from that checkout the same write says nothing
    // extra, so this is not a line printed on every `config set`.
    let (elsewhere, _, _) = run(
        home.path(),
        &["config", "set", "Merge strategy", "rebase"],
        "",
    );
    assert!(
        !elsewhere.contains("squash"),
        "a checkout that overrides nothing is being warned about one: {elsewhere}"
    );

    let (said, error, ok) = run_in(
        home.path(),
        repo.path(),
        &["config", "set", "Merge strategy", "merge commit"],
        "",
    );
    assert!(ok, "the write itself was refused: {said}{error}");
    assert!(
        said.contains("squash"),
        "this checkout answers `squash` and the write reported itself without saying so: {said}"
    );
    assert!(
        said.contains("--repo"),
        "nothing names the one command that makes the two agree: {said}"
    );

    // And what a run here actually reads is unchanged, which is the fact the
    // line above exists to state.
    let (after, _, _) = run_in(home.path(), repo.path(), &["config", "list"], "");
    let merge = after
        .lines()
        .find(|line| line.starts_with("Merge strategy "))
        .and_then(|line| line.split_whitespace().nth(2))
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        merge, "squash",
        "the checkout's own answer moved, so this test is measuring something else"
    );
}

/// The other door asks a sub-agent's declared tool list too.
///
/// A role's `tools:` line is checked in the lifecycle hook and was checked
/// nowhere else — and `estigia gate` is not a convenience: OpenCode's plugin
/// shells out to it on every edit. So the list an operator wrote went on being
/// a request for exactly the agent whose gate is a plugin.
///
/// The same shape as `both_doors_to_the_gate_read_one_payload_the_same_way`,
/// one feature over. What that one found about `cwd` and a command's spelling,
/// this one finds about the question itself: the second door was not asking it.
#[test]
fn the_other_door_asks_a_sub_agents_declared_tool_list_too() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    let agents = repo.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("an agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read, Grep\n---\n\nReads things.\n",
    )
    .expect("the definition is written");

    let ask = |tool: &str| {
        let payload = format!(
            "{{\"agent_type\":\"reader\",\"file_path\":\"src/main.rs\",\"cwd\":{:?}}}",
            repo.path().display().to_string()
        );
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                tool,
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };

    // The floor: a tool the definition declares is not refused for being one.
    let (said, error, _) = ask("Read");
    assert!(
        !format!("{said}{error}").contains("declared role"),
        "a tool this sub-agent declares was refused: {said}{error}"
    );

    let (said, error, ok) = ask("Edit");
    assert!(
        format!("{said}{error}").contains("declared"),
        "a sub-agent reached past its own tool list and this door said nothing: {said}{error}"
    );
    assert!(!ok, "the call was allowed as well as reported");
}

/// A stand-down reaches a role refusal through both doors, or through neither.
///
/// The hook wraps its role denial in `standdown::over`; the door added beside
/// it returned the refusal straight. So an operator who had stood the gate down
/// found it standing down for every refusal except that one, through the agent
/// gated by a plugin.
///
/// Found by asking the systematic version of the question rather than the
/// one-at-a-time version: *which of the things the hook does does this door
/// also do*. It was the round's own change that had the hole.
#[test]
fn a_stand_down_reaches_a_role_refusal_through_both_doors() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let repo = tempfile::tempdir().expect("a checkout");
    let agents = repo.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).expect("an agents directory");
    std::fs::write(
        agents.join("reader.md"),
        "---\nname: reader\ntools: Read\n---\n\nReads things.\n",
    )
    .expect("the definition is written");

    let payload = format!(
        "{{\"agent_type\":\"reader\",\"file_path\":\"src/main.rs\",\"cwd\":{:?}}}",
        repo.path().display().to_string()
    );
    let ask = || {
        run_in(
            home.path(),
            repo.path(),
            &[
                "gate",
                "Edit",
                "--run-id",
                "claude-aaaa1111",
                "--input",
                &payload,
            ],
            "",
        )
    };

    // The floor: without one, the refusal stands.
    let (said, error, ok) = ask();
    assert!(
        !ok && format!("{said}{error}").contains("declared"),
        "the role refusal is gone, so the stand-down below proves nothing: {said}{error}"
    );

    let (_, error, declared) = run(
        home.path(),
        &["stand-down", "--reason", "measuring", "--minutes", "5"],
        "",
    );
    assert!(declared, "the stand-down was refused: {error}");

    let (said, error, ok) = ask();
    assert!(
        ok,
        "a stand-down reaches every other refusal and not this one: {said}{error}"
    );
}

/// The run `status` names is the run `release` answers to.
///
/// A pointer whose file name and whose `run_id` disagree is one `session::load`
/// already calls unreadable — deliberately, because a pointer that does not
/// name the run it is filed under is not one that run may act on. `holdings`
/// read the same directory by **content** and did not ask, so `status` listed
/// the identity written inside.
///
/// The two then answered differently about the same run, and `status`'s own
/// hint made it worse: it says `estigia release --run-id <id>` puts one down,
/// beside an id that command answers *holds no issue* for. A message naming a
/// command that does not discharge is the one thing the ratchet forbids, and
/// this one names it with the argument filled in.
#[test]
fn the_run_status_names_is_the_run_release_answers_to() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("a state directory");
    // The floor: a pointer whose name and content agree is listed, so this is
    // not a change that stops `status` reporting holdings.
    std::fs::write(
        runs.join("claude-aaaa1111.json"),
        r#"{"run_id":"claude-aaaa1111","issue":12,"verified_at":1785988000}"#,
    )
    .expect("an honest pointer");
    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        said.contains("claude-aaaa1111"),
        "an ordinary holding stopped being reported: {said}"
    );

    // And one that disagrees with itself.
    std::fs::write(
        runs.join("claude-bbbb2222.json"),
        r#"{"run_id":"claude-cccc3333","issue":99,"verified_at":1785988000}"#,
    )
    .expect("a pointer that disagrees with itself");
    let (said, _, _) = run(home.path(), &["status"], "");
    assert!(
        !said.contains("claude-cccc3333"),
        "`status` names a run that no command answers to: {said}"
    );
    assert!(
        said.contains("cannot be read"),
        "the pointer that disagrees with itself is not reported at all: {said}"
    );
}

/// Every command the doctor names clears the check that named it.
///
/// Yesterday's version of this ran one suggestion, for one check. This runs
/// **all of them**: whatever the report says is broken and answers with a
/// command, that command is executed exactly as printed and the same check is
/// asked again.
///
/// It replaced a narrower one that posed the stand-down alone. Two tests for
/// one rule is two places for it to drift, and the floor below carries what
/// that one asserted: with three faults posed, a check that stopped naming a
/// command drops the count under three.
///
/// A check whose resolution is `operator-knowledge` is left alone on purpose —
/// `gh` wants an authenticated CLI and a sandbox has none — and that is the
/// difference the ratchet turns on: naming no command is honest when there is
/// none, and naming one that does not discharge is the one thing it forbids.
/// A command line, split the way a shell splits it.
///
/// Only what a resolution can carry: words, and double-quoted words with spaces
/// in them. No escapes, because nothing here writes one — and a splitter that
/// pretended to handle more would be a second, worse shell.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for character in line.chars() {
        match character {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[test]
fn every_command_the_doctor_names_clears_the_check_that_named_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["install", "claude-code"], "");

    // Four faults, chosen because each is answered by a different command: a
    // contract that is not this build's, a transport that is not there, a
    // stand-down record nothing can read, and a row the chosen tracker does not
    // read.
    //
    // A fixed corpus, and it must grow with the arms. The fourth is here
    // because it was not: an arm added later named `config set "Project board"
    // none`, and every write to that row was refused under that tracker — a
    // health report pointing at a refusal, found by running it by hand rather
    // than by this test, which was passing throughout.
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
    // Answered while the tracker read it, and left behind when the tracker
    // moved. Written before the drift above is undone, so the contract this
    // reads is the one the commands below repair.
    run(
        home.path(),
        &["config", "set", "Project board", "acme/7"],
        "",
    );
    run(home.path(), &["config", "set", "Tracker", "linear"], "");

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
    // The floor: the faults were posed and each answers with a command — and
    // the fourth is named, because a corpus that quietly stops reaching an arm
    // is a guard that passes by posing nothing. That is how the dead end this
    // case exists for got past it.
    assert!(
        broken
            .iter()
            .any(|(_, command)| command.contains("Project board")),
        "the row the tracker no longer reads was not posed: {broken:?}"
    );
    assert!(
        broken.len() >= 3,
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

#[test]
fn a_push_the_guard_decided_on_is_one_the_ledger_holds() {
    // The ledger is written by the tool path and by `stand-down`, and by nothing
    // else. The `PrePush` arm decides and records nothing — so a push this
    // repository's guard refused, or allowed under a live claim, leaves no trace
    // at all, at the boundary the honesty contract calls the unconditional one.
    //
    // It costs twice. Nobody can say afterwards which pushes were adjudicated,
    // which is the question the ledger exists to answer. And `doctor`'s silence
    // row reads that same file to say whether anything has reached the gate: on
    // a machine where every push has been decided and no tool call has, it
    // answers *no call has reached the gate yet*.
    //
    // `Outside` stays unrecorded, deliberately and for the reason `hook::note`
    // already gives: a checkout nobody holds is not this harness's business, and
    // logging every push in every repository would bury the ones that mattered.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repo = tempfile::tempdir().expect("a checkout");
    std::fs::create_dir_all(repo.path().join(".git")).expect("a git directory");

    // A pointer that covers this checkout and will not parse. It reaches a
    // refusal without a tracker: `run-pointers-unreadable` is decided from the
    // file alone, which is what makes this measurable at all.
    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the state directory");
    std::fs::write(runs.join("claude-torn0000.json"), "{ this is not json")
        .expect("the torn pointer is written");

    let refs = "refs/heads/work 1111111111111111111111111111111111111111                 refs/heads/main 2222222222222222222222222222222222222222
";
    let (_, error, ok) = run_in(home.path(), repo.path(), &["hook", "pre-push"], refs);
    assert!(
        !ok && error.contains("run-pointers-unreadable"),
        "the guard did not decide, so this measures nothing: {error}"
    );

    let ledger = home.path().join(".estigia").join("decisions.jsonl");
    let written = std::fs::read_to_string(&ledger).unwrap_or_default();
    assert!(
        written.contains("git push"),
        "the guard refused a push and the ledger says nothing about it: {written:?}"
    );
    assert!(
        written.contains("deny") && written.contains("run-pointers-unreadable"),
        "the entry does not say what was decided or why: {written:?}"
    );
    // And what it was aimed at. git hands a `pre-push` hook the refs it is about
    // to write, on standard input; the arm decided without reading a byte of
    // them, so the ledger could say a push had been adjudicated and never which
    // push. Reading them changes no decision — the honesty contract still
    // records that nothing adjudicates the destination — but it is the
    // difference between a boundary that can be audited afterwards and one that
    // can only be spoken about in the present.
    assert!(
        written.contains("refs/heads/main"),
        "the ledger holds a push decision and not what the push was aimed at: {written:?}"
    );

    // And a push from a checkout nobody holds is still not written down.
    let bare = tempfile::tempdir().expect("a second home");
    std::fs::create_dir_all(bare.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let elsewhere = tempfile::tempdir().expect("another checkout");
    std::fs::create_dir_all(elsewhere.path().join(".git")).expect("a git directory");
    let (_, _, quiet) = run_in(bare.path(), elsewhere.path(), &["hook", "pre-push"], "");
    assert!(quiet, "a push nobody holds was refused");
    assert!(
        !bare
            .path()
            .join(".estigia")
            .join("decisions.jsonl")
            .exists(),
        "a push from a checkout nobody holds was written down"
    );
}

#[test]
fn a_row_another_contract_did_not_take_is_not_one_it_took() {
    // `config set` writes a repository-wide row into every installed contract,
    // and the reason is written beside the loop that does it: *an operator with
    // two agents installed declared `make deploy` a one-way door, and only one
    // of the two contracts said so. The gate read the one that did and enforced
    // it; the agent reading the other was never told what it had walked into.*
    //
    // The primary write is read back and refuses when `estigia.local.md`
    // overrides the row. The propagated ones were written and never read, and
    // the count of them was reported as *"written into N other installed
    // contract(s): it is a fact about this repository, so every agent's copy
    // says it"*.
    //
    // Measured on the installed pair, with the operator's own file in the shared
    // neutral root answering that row:
    //
    // ```text
    // $ estigia config set "Irreversible commands" "terraform apply"
    // Irreversible commands is now terraform apply
    //   written into 1 other installed contract(s) … every agent's copy says it
    // $ estigia config list --agent opencode
    // Irreversible commands   none
    // ```
    //
    // Which is the incident that paragraph describes, reproduced by the fix
    // written to prevent it — on the row that declares a one-way door.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    run(home.path(), &["setup", "opencode"], "");

    let shared = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::write(
        shared.join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Irreversible commands | none |\n",
    )
    .expect("the override is written");

    let (said, error, ok) = run(
        home.path(),
        &["config", "set", "Irreversible commands", "terraform apply"],
        "",
    );

    // The floor: the other contract really does answer differently, so this is
    // measuring a disagreement rather than a message.
    let (theirs, _, _) = run(home.path(), &["config", "list", "--agent", "opencode"], "");
    assert!(
        theirs
            .lines()
            .any(|line| line.starts_with("Irreversible commands") && line.contains("none")),
        "the shared root took the row after all, so this measures nothing:\n{theirs}"
    );

    assert!(
        !ok,
        "a row one agent will never read was reported as written everywhere:\n{said}"
    );
    assert!(
        error.contains("terraform apply") || error.contains("Irreversible commands"),
        "the refusal does not name the row: {error}"
    );
    assert!(
        error.contains(".agents"),
        "the refusal does not name the contract that did not take it: {error}"
    );
}

#[test]
fn a_dry_run_says_what_would_be_left_behind_too() {
    // `--dry-run` is sold as *see exactly what would change first*, and the
    // question an operator runs it to answer, before letting an uninstaller near
    // their home directory, is whether it will take their things.
    //
    // The real run answers it: *1 file(s) in that directory are not Estigia's
    // and were left there*. The dry run said nothing at all — the note was
    // computed only when something had actually been removed, so the one moment
    // it is being read to make a decision is the one moment it is absent.
    //
    // The listing is the same either way: it names the files no action in the
    // plan names, and a plan has actions.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    std::fs::write(root.join("NOTAS.md"), "my own notes\n").expect("the notes are written");

    let (planned, _, _) = run(
        home.path(),
        &["setup", "claude-code", "--uninstall", "--dry-run"],
        "",
    );
    assert!(
        planned.contains("NOTAS.md"),
        "the dry run does not say the operator's file would be left:\n{planned}"
    );
    // Said in the tense of a plan. A dry run reporting that files "were left
    // there" is a dry run claiming to have done something.
    assert!(
        !planned.contains("were left there"),
        "the dry run speaks as though it had already removed anything:\n{planned}"
    );
    // And nothing was touched by asking.
    assert!(
        root.join("SKILL.md").is_file() && root.join("NOTAS.md").is_file(),
        "the dry run removed something"
    );

    // The plan and the act name the same file.
    let (done, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    assert!(
        done.contains("NOTAS.md") && done.contains("were left there"),
        "the real run stopped naming it, so the two are being compared to nothing:\n{done}"
    );
}

#[test]
fn a_guard_that_was_not_there_is_not_a_guard_that_was_removed() {
    // `guard::uninstall` answers with the state the repository is in
    // *afterwards*, and `Absent` is what it answers for two different things:
    // Estigia's hook was here and is gone, and there was never one here. The
    // renderer printed `<path> removed` for both — and for a third, because
    // `--dry-run` takes the same arm and had removed nothing at all.
    //
    // One word for three states, in the command whose job is taking a file off
    // somebody's machine. The arms below it in that same match are there for
    // exactly this reason — *nothing was taken out, and saying so beats
    // reporting a removal that did not happen* — and the main arm collapsed it.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let repo = tempfile::tempdir().expect("a checkout");
    // A real one: the guard asks `git rev-parse --git-path hooks` rather than
    // guessing, so a hand-made `.git` is not a repository to it.
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .arg("init")
            .arg("-q")
            .status()
            .is_ok_and(|status| status.success()),
        "git is not on the path, so this cannot measure anything"
    );
    let hook = repo.path().join(".git").join("hooks").join("pre-push");

    // Nothing here: an uninstall removes nothing and must not say it did.
    let (empty, _, _) = run_in(home.path(), repo.path(), &["guard", "--uninstall"], "");
    assert!(
        !empty.contains("removed"),
        "an uninstall with nothing to remove reported a removal: {empty}"
    );

    run_in(home.path(), repo.path(), &["guard"], "");
    assert!(
        hook.is_file(),
        "the guard did not install, so this measures nothing"
    );

    // A plan removes nothing either, and says so in the tense of a plan.
    let (planned, _, _) = run_in(
        home.path(),
        repo.path(),
        &["guard", "--uninstall", "--dry-run"],
        "",
    );
    assert!(
        hook.is_file(),
        "the dry run removed the hook, which is the one thing it must not do"
    );
    assert!(
        planned.contains("would be removed"),
        "the dry run does not say what it would do: {planned}"
    );
    // And not in the tense the act uses. `would be removed` contains the act's
    // own wording, so the two are told apart by the whole line rather than by a
    // substring — the assertion that read `" removed"` failed on the fix.
    assert!(
        !planned
            .lines()
            .any(|line| line.trim_end().ends_with("pre-push removed")),
        "the dry run spoke as though it had removed the hook: {planned}"
    );

    // And the act says it did.
    let (done, _, _) = run_in(home.path(), repo.path(), &["guard", "--uninstall"], "");
    assert!(!hook.exists(), "the hook survived its own removal");
    assert!(
        done.contains("removed"),
        "the removal that happened was not reported: {done}"
    );
}

#[test]
fn a_row_the_tracker_made_inert_is_named_rather_than_hidden() {
    // The board row stopped being offered on a tracker that has no board, which
    // is right for a row nobody has answered yet — and does nothing for the one
    // already in the contract. Measured: with `acme/7` set and the tracker moved
    // to Linear, `config list` still reports `acme/7`, the contract the **agent
    // reads** still carries `| Project board | acme/7 |`, and nothing anywhere
    // says the transport under that tracker will never look at it.
    //
    // Hiding the row on the screen made the operator stop seeing the problem
    // rather than stop having it. A value reported as configured that nothing
    // reads is the defect this crate has found in its own table three times.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    run(
        home.path(),
        &["config", "set", "Project board", "acme/7"],
        "",
    );
    run(home.path(), &["config", "set", "Tracker", "linear"], "");

    // The floor: the row really is still in the contract, so this is measuring
    // a live value rather than an absence.
    let contract = std::fs::read_to_string(
        home.path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("SKILL.md"),
    )
    .expect("the contract is readable");
    assert!(
        contract.contains("| Project board | acme/7 |"),
        "the tracker change took the row out, so nothing here is inert"
    );

    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        whole.contains("Project board"),
        "a row nothing will read is not named by the command that reports health:\n{whole}"
    );
    assert!(
        whole.contains("linear"),
        "the report does not say which tracker made it inert:\n{whole}"
    );

    // And the command it names clears the row. It did not: writing that row was
    // refused outright under a tracker with no board, `none` included — so the
    // health report pointed at a command that answers with a refusal, which is
    // the one thing the ratchet forbids, produced by two changes that were each
    // right on their own.
    //
    // Taking an inert answer away is not configuring a setting the tracker does
    // not read. It is removing one.
    let (_, refusal, cleared) = run(home.path(), &["config", "set", "Project board", "none"], "");
    assert!(
        cleared,
        "the command the report names is refused: {refusal}"
    );
    let (after, error, _) = run(home.path(), &["doctor"], "");
    assert!(
        !format!("{after}{error}").contains("does not read it"),
        "running what the report named did not clear it:\n{after}"
    );
}

#[test]
fn what_a_half_written_file_left_behind_is_not_the_operators() {
    // Every write goes through a temporary beside its target — `SKILL.md` is
    // written as `SKILL.estigia-<pid>.tmp` and renamed over. The cleanup runs
    // when the write **fails**; a process killed between the create and the
    // rename leaves the temporary where it is, and nothing sweeps it.
    //
    // So the uninstaller walked the directory, found a file no action had named,
    // and told the operator it was theirs — Estigia's own residue, in the one
    // sentence that exists to answer *did it touch my things?*. It is the round
    // that fixed that sentence, one file over: a name Estigia chose, reported as
    // one it had never heard of.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let residue = root.join("SKILL.estigia-4321.tmp");
    std::fs::write(&residue, "half a contract\n").expect("the residue is written");
    std::fs::write(root.join("NOTAS.md"), "my own notes\n").expect("the notes are written");

    let (said, _, _) = run(home.path(), &["setup", "claude-code", "--uninstall"], "");
    let note = said
        .lines()
        .find(|line| line.contains("are not Estigia's"))
        .unwrap_or_else(|| panic!("the surviving directory was not accounted for:\n{said}"));
    assert!(
        note.contains("NOTAS.md"),
        "the operator's own file is the one thing this note exists to name: {note}"
    );
    assert!(
        !note.contains("estigia-4321"),
        "Estigia's own half-written file was reported as the operator's: {note}"
    );

    // And it is taken away. A name this tool chose is a file this tool owns, and
    // an uninstall that leaves it is one that did not remove what it created.
    assert!(
        !residue.exists(),
        "the residue of a write that died survived the uninstall"
    );
    assert!(
        root.join("NOTAS.md").is_file(),
        "the sweep took a file that was not Estigia's"
    );
}

#[test]
fn a_row_that_carried_a_separator_is_not_a_row_that_said_less() {
    // `config set` refuses a value holding a `|` by name — *the value is stored
    // as one cell of a one-line table row, and nothing escapes either* — which
    // is exactly right, and covers the door Estigia writes through.
    //
    // The other door reads a file Estigia never writes. There the `|` is not
    // refused: it is a cell boundary, so the row splits one cell early and the
    // value arrives **truncated**, with nothing anywhere saying so. Measured:
    //
    // ```text
    // the operator wrote : make deploy | tee log
    // the tool believes  : make deploy
    // ```
    //
    // On the row that declares one-way doors, in the file the operator is told
    // to edit by hand. A value read as something the file does not say is the
    // failure this whole tool exists to refuse.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    std::fs::write(
        home.path()
            .join(".claude")
            .join("skills")
            .join(estigia::skill::DIRECTORY)
            .join("estigia.local.md"),
        "| Setting | Value here |\n|---|---|\n| Irreversible commands | make deploy | tee log |\n",
    )
    .expect("the override is written");

    // The floor: the row really is read, and read short. A file nobody read
    // would satisfy the assertion below by saying nothing at all.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("Irreversible commands") && line.contains("make deploy")),
        "the row is not read at all, so nothing here is measured:\n{listed}"
    );

    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        whole.contains("Irreversible commands"),
        "a value the file does not carry is reported by nothing:\n{whole}"
    );
    assert!(
        whole.contains("estigia.local.md"),
        "the report does not name the file the row is in:\n{whole}"
    );
}

#[test]
fn a_line_ending_nobody_can_see_is_not_a_row_that_said_less() {
    // The sibling above found the `|`. The bar is at least *visible*: an
    // operator staring at the row can see what cut it. Eight more characters
    // end a line for both readers of this table — `lines_the_transport_sees`
    // exists because they do — and U+2028 is the one a paste out of a browser
    // or a word processor routinely carries. It draws as nothing.
    //
    // ```text
    // the operator wrote : make deploy<U+2028>npm publish
    // the tool believes  : make deploy
    // ```
    //
    // `npm publish` stops being a declared one-way door. The direction is the
    // whole finding: a value read as less than it says always *loosens*, and
    // this crate's own rule is that configuration may only tighten.
    //
    // Width cannot see it. The bar makes a row wider than its header; a line
    // ending makes it narrower, and turns the rest into a fragment that starts
    // with no `|` and is dropped entirely — which is why the check walks the
    // line an editor draws rather than the line the parser produces.
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let local = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.local.md");
    std::fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
         | Irreversible commands | make deploy\u{2028}npm publish |\n",
    )
    .expect("the override is written");

    // The floor, and it is the finding: the row is read, and read without the
    // half that mattered. An assertion that only checked `doctor` would pass
    // just as well against a file nobody opened.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    // The value column only. The `accepts:` clause beside it spells `npm
    // publish` as its own example, and a floor that read the whole line would
    // have been satisfied by the help text rather than by the value.
    let row = listed
        .lines()
        .find(|line| line.starts_with("Irreversible commands"))
        .unwrap_or_default()
        .split("accepts:")
        .next()
        .unwrap_or_default();
    assert!(
        row.contains("make deploy") && !row.contains("npm publish"),
        "the fixture poses nothing \u{2014} the value is not being cut:\n{listed}"
    );

    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        whole.contains("Irreversible commands") && whole.contains("estigia.local.md"),
        "a value the file does not carry is reported by nothing:\n{whole}"
    );
    // Spelled as a code point, because that is what an operator can search for.
    // The character itself would paste into the message as nothing, and a
    // message telling somebody to look for a `|` in a row that has none sends
    // them hunting for the wrong thing.
    assert!(
        whole.contains("U+2028") && !whole.contains("holds a `|`"),
        "the report does not say what to look for, or names the wrong character:\n{whole}"
    );

    // And the same table, whole, is still fine — or every installation
    // reads as broken and the row teaches nobody anything.
    std::fs::write(
        &local,
        "| Setting | Value here |\n|---|---|\n\
         | Irreversible commands | make deploy, npm publish |\n",
    )
    .expect("the override is rewritten");
    let (said, error, _) = run(home.path(), &["doctor"], "");
    let whole = format!("{said}{error}");
    assert!(
        !whole.contains("Irreversible commands"),
        "an intact row was reported as cut:\n{whole}"
    );
}

/// Using the override file the contract tells the operator to use does not make
/// the skill out of date.
///
/// `presence_of` renders every shipped file from the configuration and compares.
/// It read that configuration with `installed_config`, which layers
/// `estigia.local.md` on top — so it compared the installed `SKILL.md` against a
/// table carrying the operator's override, and `SKILL.md` cannot carry it: the
/// override exists to change what the tool does *without* changing that file.
///
/// Measured on a fresh install and one row:
///
/// ```text
/// | Merge strategy | rebase |   in estigia.local.md
///
/// doctor : BROKEN skill    — "not this binary's copy"
///          BROKEN contract — "the rules the agent reads are not the rules the
///                            gate enforces"
/// then   : "skill is not usable, so a run cannot swear yet"
/// ```
///
/// All three sentences were false. The payload was this binary's, byte for byte.
///
/// The repair the message named made it worse rather than wrong: `estigia sync`
/// discharged it by writing `rebase` **into the versioned block** — the file
/// that is committed and shared, carrying a note that reads *Configure the
/// ignored local file, never this versioned block*. Deleting the override
/// afterwards left the value behind, so a machine-local choice had quietly
/// become the team's.
#[test]
fn the_operators_own_override_does_not_make_the_contract_look_stale() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let versioned = root.join("SKILL.md");

    // The floor. A fresh install has to read as current, or "still current
    // after the override" is a sentence about nothing.
    let (before, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !before.contains("not this binary's copy"),
        "a freshly installed skill already reads as stale:\n{before}"
    );

    std::fs::write(
        root.join("estigia.local.md"),
        "| Setting | Value |\n|---|---|\n| Merge strategy | rebase |\n",
    )
    .expect("the operator's own file");

    // It is in force — this is the override doing its job, and without it the
    // assertion below would pass on a file nobody read.
    let (listed, _, _) = run(home.path(), &["config", "list"], "");
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("Merge strategy") && line.contains("rebase")),
        "the override is not in force, so nothing here is measured:\n{listed}"
    );

    let (after, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !after.contains("not this binary's copy"),
        "using the override file reported the skill as not this binary's copy:\n{after}"
    );
    // And the versioned block still says what this binary wrote, whether or not
    // anything asked it to re-render.
    run(home.path(), &["sync"], "");
    let table = std::fs::read_to_string(&versioned).expect("the contract reads");
    assert!(
        table.contains("| Merge strategy | merge commit |"),
        "sync promoted an `estigia.local.md` row into the versioned block:\n{}",
        table
            .lines()
            .find(|line| line.starts_with("| Merge strategy"))
            .unwrap_or("(the row is gone)")
    );
    // The override still overrides after the sync, or this was fixed by
    // dropping the operator's value instead of by keeping it where it belongs.
    let (still, _, _) = run(home.path(), &["config", "list"], "");
    assert!(
        still
            .lines()
            .any(|line| line.starts_with("Merge strategy") && line.contains("rebase")),
        "sync stopped the override from overriding:\n{still}"
    );

    // And a genuinely old payload is still reported. Fixing the false alarm by
    // never raising the true one is the failure this crate exists to refuse.
    // Aged through the binding, now that the transport is retired and no
    // longer installed. The same fact told about a file that is still
    // there: a payload behind the binary.
    std::fs::write(
        root.join("bindings").join("github.md"),
        "# older
",
    )
    .expect("an aged binding");
    let (aged, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        aged.contains("not this binary's copy"),
        "a skill that really is behind the binary is no longer reported:\n{aged}"
    );
}

/// A mistyped command never means the world may have changed.
///
/// `ExitCode::Indeterminate = 2` carries one sentence in this crate — *the
/// command was interrupted and the world may have changed. This is the code that
/// must never be reused for anything else* — and clap's own exit code for a
/// usage error is 2. So `estigia stand-down --lift` without the arguments it
/// needs, or any flag a build no longer takes, answered with the code that tells
/// a caller to re-read the tracker before retrying.
///
/// The callers are the scripts this crate writes. Every one of them treats `1`
/// and `2` as decisions and anything else as *the gate did not answer* — so a
/// hook file left from an older build, passing a flag this one dropped, would
/// exit 2 and **block every push in the repository** rather than step aside.
/// That is the failure `guard::script` exists to prevent, arriving through the
/// argument parser.
///
/// **And moving it to `1` did not make it step aside.** `1` is a decision too:
/// the scripts propagate it, so the aged hook went on blocking every push, now
/// with a clap usage message instead of a stop. Measured on a real `git push` —
/// `error: unexpected argument --from-a-newer-build found`, push refused, and
/// `an_invocation_this_build_cannot_read_is_not_a_refusal` holds that end.
///
/// So the assertion below is about the property rather than the number: a usage
/// error must be **neither of the two codes that decide**, which is what puts it
/// in the branch both readers already have for *it did not answer*.
#[test]
fn a_usage_error_does_not_answer_with_the_code_that_means_the_world_moved() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");

    // The floor: a command that really is indeterminate is not what this is
    // about, and `--help` still has to work. Both ends of the range first.
    let (helped, _, ok) = run(home.path(), &["--help"], "");
    assert!(
        ok && helped.contains("estigia"),
        "`--help` stopped being a success printed on stdout: {helped}"
    );
    let (versioned, _, ok) = run(home.path(), &["--version"], "");
    assert!(
        ok && !versioned.trim().is_empty(),
        "`--version` prints nothing"
    );

    for wrong in [
        vec!["stand-down", "--lift"],
        vec!["gate"],
        vec!["config", "set", "Tracker"],
        vec!["hook", "pre-tool-use", "--dialect"],
        vec!["setup", "--no-such-flag"],
        vec!["no-such-verb"],
    ] {
        let (_, said, ok) = run(home.path(), &wrong, "");
        assert!(!ok, "`{wrong:?}` was accepted");
        // The status the scripts read. `run` answers success or not, so the code
        // itself is asked of the process directly.
        let status = Command::new(estigia())
            .args(&wrong)
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .current_dir(home.path())
            .output()
            .expect("the binary runs")
            .status;
        assert_ne!(
            status.code(),
            Some(2),
            "`{wrong:?}` answered 2, which says the world may have changed: {said}"
        );
        assert_ne!(
            status.code(),
            Some(1),
            "`{wrong:?}` answered 1, which every script this crate writes propagates as a \
             refusal \u{2014} so a mistyped invocation blocks a push: {said}"
        );
        // And it answered *something*: a usage error that exits 0 is a mistyped
        // command reported as a success.
        assert!(
            status.code().is_some_and(|code| code > 2),
            "`{wrong:?}` answered {:?}, which is not a code the scripts route to \
             `the gate did not answer`",
            status.code()
        );
    }
}

/// A command the operator added beside Estigia's is not Estigia's to remove.
///
/// A hook group is a **matcher and a list**, and the group was dropped whole
/// whenever it held one of ours. So an operator who put their own command in
/// the `PreToolUse` group Estigia had written lost it on the next uninstall —
/// and on the next `sync`, which rewrites the same group.
///
/// `render` says the rule two levels up, twice, in its own words: *an operator's
/// unfilled slot is theirs*, and *a wrapper is ours to drop only when what
/// emptied it was us*. The innermost level is the one holding the thing they
/// typed, and it was the one where the rule was not applied.
#[test]
fn a_hook_the_operator_added_beside_estigias_survives_the_uninstall() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let settings = home.path().join(".claude").join("settings.json");

    let mut root: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings).expect("the settings"))
            .expect("settings are JSON");
    let group = root["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_mut()
        .expect("Estigia wrote a group");
    // The floor: Estigia's own command is in there, so "theirs survives" is not
    // a sentence about an empty group.
    assert_eq!(group.len(), 1, "the group is not the one Estigia wrote");
    group.push(serde_json::json!({"type": "command", "command": "mi-script-propio.sh"}));
    std::fs::write(
        &settings,
        serde_json::to_string_pretty(&root).expect("serialises"),
    )
    .expect("their command is added");

    // A sync rewrites that group, and must not take it either.
    run(home.path(), &["sync"], "");
    let after = std::fs::read_to_string(&settings).expect("the settings");
    assert!(
        after.contains("mi-script-propio.sh"),
        "sync took a command the operator added:\n{after}"
    );
    assert!(
        after.contains("hook pre-tool-use"),
        "sync stopped registering the gate:\n{after}"
    );

    run(home.path(), &["uninstall", "--all"], "");
    let left = std::fs::read_to_string(&settings).unwrap_or_default();
    assert!(
        left.contains("mi-script-propio.sh"),
        "uninstall took a command the operator added beside Estigia's:\n{left}"
    );
    assert!(
        !left.contains("hook pre-tool-use"),
        "uninstall left Estigia's own command behind:\n{left}"
    );
}

/// One unusable file does not cost the other agents their setup, and the code
/// does not claim nothing was written.
///
/// `collect_over` stopped at the first refusal and propagated it with `?`,
/// throwing away every result collected before it. Measured: one hand-edited
/// `~/.cursor/hooks.json` and `estigia setup --all` wrote **58 files** across
/// six agents, left five untouched, printed nothing but *Cursor: … is not
/// JSON*, and exited `1`.
///
/// Two things wrong, and the second is the one this crate is about. The agents
/// behind the failing one had nothing wrong with them and got nothing, over a
/// file that is not theirs. And exit `1` is `ExitCode::Refused`, whose sentence
/// is *the command refused, and nothing was written* — said about a run that had
/// written fifty-eight files and named none of them.
#[test]
fn a_file_one_agent_cannot_take_does_not_stop_the_others_or_hide_the_writes() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let cursor = home.path().join(".cursor");
    std::fs::create_dir_all(&cursor).expect("cursor's directory");
    std::fs::write(cursor.join("hooks.json"), "no es json {\n").expect("a file it cannot take");

    let status = |arguments: &[&str]| {
        Command::new(estigia())
            .args(arguments)
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .env("APPDATA", home.path().join("AppData").join("Roaming"))
            .env("XDG_CONFIG_HOME", home.path().join(".config"))
            .current_dir(home.path())
            .output()
            .expect("the binary runs")
            .status
            .code()
    };

    let (said, refused, ok) = run(home.path(), &["setup", "--all"], "");
    assert!(!ok, "a file it cannot take was accepted");
    assert!(
        refused.contains("cursor") || refused.contains("Cursor"),
        "the refusal does not name the agent whose file it is: {refused}"
    );
    // The report of what it did, which used to be thrown away with the results.
    assert!(
        said.contains("Claude Code") && said.contains("changed"),
        "the run wrote files and reported none of them:\n{said}{refused}"
    );

    // And the refusal says what happened to the rest of the batch. The
    // outcome's own line is written for a single write — *the write landed;
    // what failed came after it* — and a reader handed one refusal has no way
    // to tell from it that ten other agents were done.
    assert!(
        refused.contains("agent(s) were done"),
        "the refusal says nothing about the agents that were done: {refused}"
    );

    // Every other agent is configured, or one operator's hand-edited file has
    // cost ten agents their gate. Cursor itself stays untouched because setup
    // now prevalidates its late hook render before writing its earlier files.
    let (state, _, _) = run(home.path(), &["status"], "");
    let left = state
        .lines()
        .filter(|line| line.contains("not configured"))
        .collect::<Vec<_>>();
    assert_eq!(
        left.len(),
        1,
        "agents other than Cursor were left unconfigured by its file:\n{state}"
    );
    assert!(
        left[0].contains("cursor"),
        "the one untouched agent was not Cursor:\n{state}"
    );

    // And the code says what happened to the world. `1` is *nothing was
    // written*; this run wrote.
    assert_eq!(
        status(&["setup", "--all", "--allow-source-build"]),
        Some(1),
        "a second run wrote nothing and did not answer with the refused code"
    );
    let fresh = tempfile::tempdir().expect("a second home");
    std::fs::create_dir_all(fresh.path().join("AppData").join("Roaming")).expect("a roaming dir");
    std::fs::create_dir_all(fresh.path().join(".cursor")).expect("cursor's directory");
    std::fs::write(
        fresh.path().join(".cursor").join("hooks.json"),
        "no es json {\n",
    )
    .expect("a file it cannot take");
    let first = Command::new(estigia())
        .args(["setup", "--all", "--allow-source-build"])
        .env("HOME", fresh.path())
        .env("USERPROFILE", fresh.path())
        .env("APPDATA", fresh.path().join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", fresh.path().join(".config"))
        .current_dir(fresh.path())
        .output()
        .expect("the binary runs")
        .status
        .code();
    assert_eq!(
        first,
        Some(2),
        "a run that wrote and then refused answered with the code that means nothing was written"
    );
}

/// A tracker with no executable transport is said out loud, not left to the rows.
///
/// `linear` and `trello` ship a binding the agent reads by hand and no
/// transport, so `claim` refuses with `tracker-has-no-transport` — no run can
/// swear, and a run that cannot swear is a run the gate never adjudicates.
///
/// `doctor` printed a `skipped` row about it, said nothing at the end, and
/// exited `0`; `status` said `harness: gate on`. Every write on that machine
/// went through unmeasured, on a machine two commands called healthy.
///
/// Said rather than refused: choosing one of those trackers is a **choice**,
/// not a fault, and exiting non-zero would report a healthy machine as broken.
/// What was missing is the sentence.
#[test]
fn a_tracker_with_no_transport_is_said_in_the_verdict() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    for tracker in ["linear", "trello"] {
        let (_, _, ok) = run(home.path(), &["config", "set", "Tracker", tracker], "");
        assert!(ok, "{tracker} is not a tracker this build offers");
        let (said, _, ok) = run(home.path(), &["doctor"], "");
        // Still a healthy machine: this is a choice, not a fault.
        assert!(
            ok,
            "{tracker} made `doctor` report a broken machine:\n{said}"
        );
        assert!(
            said.contains("no run can swear here"),
            "`doctor` does not say that nothing can be claimed on {tracker}:\n{said}"
        );
        assert!(
            said.contains(tracker),
            "the line does not name the tracker it is about:\n{said}"
        );
    }

    // And on a tracker that has one, the line is absent — or every ordinary
    // machine is told it adjudicates nothing.
    let (_, _, ok) = run(home.path(), &["config", "set", "Tracker", "github"], "");
    assert!(ok, "github stopped being a tracker this build offers");
    let (said, _, _) = run(home.path(), &["doctor"], "");
    assert!(
        !said.contains("no run can swear here"),
        "a tracker with a transport was told it has none:\n{said}"
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

/// Setting one row for a checkout does not answer for the rest of them.
///
/// A repository's file is an **override**: a row that is not in it means *this
/// checkout does not answer for that setting*, and the machine's answer stands.
/// `config set --repo` wrote the whole scope, so every row nobody asked about
/// became an answer — with the **defaults**, because a file that is not there
/// yet is read as `Config::default()`.
///
/// Measured through the binary, one command:
///
/// ```text
/// before   Worktree: C:/trees     Tracker: github acme/web
/// asked    Merge strategy = rebase --repo
/// after    Worktree: unset        Tracker: github
/// ```
///
/// That checkout then pointed at a different tracker than the operator had
/// configured, and made its worktrees somewhere else. Nothing said so.
#[test]
fn setting_one_row_for_a_checkout_leaves_the_others_to_the_machine() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    run(
        home.path(),
        &["config", "set", "Tracker", "github acme/web"],
        "",
    );
    run(
        home.path(),
        &["config", "set", "Worktree location", TREES],
        "",
    );

    let repo = home.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).expect("a checkout");

    // The floor: inside the checkout, the machine's answers are what is read.
    let (before, _, _) = run_in(home.path(), &repo, &["config", "list"], "");
    assert!(
        before.contains("github acme/web") && before.contains(TREES),
        "the checkout does not start from the machine's answers:\n{before}"
    );

    let (_, said, ok) = run_in(
        home.path(),
        &repo,
        &["config", "set", "Merge strategy", "rebase", "--repo"],
        "",
    );
    assert!(ok, "the row was not set: {said}");

    let (after, _, _) = run_in(home.path(), &repo, &["config", "list"], "");
    assert!(
        after.contains("rebase"),
        "the row that was asked for did not land:\n{after}"
    );
    assert!(
        after.contains("github acme/web"),
        "asking about the merge strategy reset this checkout's tracker:\n{after}"
    );
    assert!(
        after.contains(TREES),
        "asking about the merge strategy reset this checkout's worktree location:\n{after}"
    );

    // The file carries the one row, and a second `--repo` adds to it rather
    // than replacing what the first one said.
    let file = repo.join(".git").join("estigia").join("estigia.local.md");
    let written = std::fs::read_to_string(&file).expect("the checkout's file");
    assert!(
        !written.contains("| Tracker |"),
        "the file answers for a row nobody asked it about:\n{written}"
    );
    run_in(
        home.path(),
        &repo,
        &["config", "set", "Integration", "trunk", "--repo"],
        "",
    );
    let written = std::fs::read_to_string(&file).expect("the checkout's file");
    assert!(
        written.contains("| Merge strategy | rebase |")
            && written.contains("| Integration | trunk |"),
        "a second row replaced the first instead of joining it:\n{written}"
    );
}

/// Setting one row for an agent does not answer for the rest of its scope.
///
/// The sibling of the repository's file, and the same rule: an adapter's own
/// file inside a shared skill root is an **override**, so a row it does not
/// carry is one that adapter does not answer for and the shared contract's
/// answer stands. `config set --agent` wrote the whole agent scope, so one
/// question pinned five rows.
///
/// Less costly than the repository's only by luck of where the values come
/// from: this one writes the effective answers rather than the defaults, so
/// nothing is lost at the moment of writing. What is lost is later, quietly,
/// and the operator has no reason to look — measured by moving the shared
/// answer afterwards and watching one adapter follow and the other not.
#[test]
fn setting_one_row_for_an_agent_leaves_the_others_to_the_contract() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    // Two adapters that share the neutral root, which is the only place an
    // adapter has a file of its own.
    run(home.path(), &["setup", "cursor"], "");
    run(home.path(), &["setup", "qwen"], "");

    let (_, said, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "Blind judges",
            "two blind",
            "--agent",
            "cursor",
        ],
        "",
    );
    assert!(ok, "the row was not set: {said}");

    let file = home
        .path()
        .join(".agents")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("estigia.cursor.md");
    let written = std::fs::read_to_string(&file).expect("cursor's own file");
    assert!(
        written.contains("| Blind judges | two blind |"),
        "the row that was asked for did not land:\n{written}"
    );
    assert!(
        !written.contains("| Planning |"),
        "the file answers for a row nobody asked it about:\n{written}"
    );

    // The shared answer moves, and both adapters move with it — the one with a
    // file of its own included, because that file says nothing about this row.
    let (_, _, ok) = run(
        home.path(),
        &["config", "set", "Planning", "sdd", "--agent", "qwen"],
        "",
    );
    assert!(ok, "the shared table would not take the row");
    for agent in ["qwen", "cursor"] {
        let (listed, _, _) = run(home.path(), &["config", "list", "--agent", agent], "");
        let row = listed
            .lines()
            .find(|line| line.starts_with("Planning"))
            .unwrap_or_default();
        assert!(
            row.contains("sdd"),
            "{agent} did not follow the shared answer for a row it never set: {row}"
        );
    }
    // And cursor's own answer is still its own.
    let (listed, _, _) = run(home.path(), &["config", "list", "--agent", "cursor"], "");
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("Blind judges") && line.contains("two blind")),
        "the row that was set for this agent stopped being set:\n{listed}"
    );
}

/// A long write flag on the command line is a write, whatever the command is.
///
/// The population is drawn at *what is visible on the command line*, and
/// `--write` is as visible as a redirect. It was read only for the handful of
/// commands `IN_PLACE` names, so a formatter rewriting every file in a tree
/// classified as **nothing at all** — measured through the gate:
///
/// ```text
/// sed -i s/a/b/ src/x.rs   watched
/// prettier --write src     NOT watched
/// eslint --fix .           NOT watched
/// ```
///
/// Long forms only, and that is what keeps this a shape rather than a
/// catalogue: `-i` is *ignore case* to `grep` and `-w` is *whole word*, so
/// reading either everywhere would report a search as a write.
#[test]
fn a_long_write_flag_is_a_write_whatever_the_command_is() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    let watched = |command: &str| {
        let (said, _, _) = run_in(
            home.path(),
            home.path(),
            &[
                "gate",
                "Bash",
                "--json",
                "--input",
                &format!("{{\"command\":{command:?}}}"),
            ],
            "",
        );
        // The stable name, not the Rust one. This read `"NotWatched"` — the
        // `Debug` spelling `gate --json` used to print — which is how a field
        // built out of a debug rendering becomes an interface without anybody
        // deciding it should be one.
        !said.contains("not-watched")
    };

    // The floor, both ways: a redirect is seen and a read is not, so "seen" and
    // "not seen" are both answers this gate really gives.
    assert!(
        watched("echo hola > f.txt"),
        "a redirect stopped being a write"
    );
    assert!(!watched("cat f.txt"), "a read is being reported as a write");

    for rewrites in [
        "prettier --write src",
        "eslint --fix .",
        "ruff check --fix .",
        "cargo clippy --fix",
        "somethingnew --in-place x",
    ] {
        assert!(
            watched(rewrites),
            "`{rewrites}` rewrites files and the gate does not see it"
        );
    }

    // And a search is still a search. `-i` and `-w` are the reason this reads
    // long forms only.
    for reads in ["grep -i hola src", "grep -w hola src", "sort -i f.txt"] {
        assert!(
            !watched(reads),
            "`{reads}` is a search and the gate reports it as a write"
        );
    }
}

/// An install record that cannot be read stops the uninstall rather than
/// guessing at it.
///
/// The record is the only thing that can tell a file Estigia created from one it
/// wrote over, so an unreadable one is not an empty one. Writing a fresh record
/// would lose the list; removing on no evidence would take somebody else's file.
///
/// Held here because the honesty contract's entry beside it — *by name, and not
/// by content* — describes what this path does when the record **is** readable,
/// and the two answers have to stay different.
#[test]
fn an_unreadable_install_record_stops_the_uninstall_and_says_what_it_costs() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");
    let root = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY);
    let record = root.join(".estigia").join("installed.json");

    // The floor: with the record intact, uninstall really does remove things —
    // otherwise "it removed nothing" below is a sentence about a command that
    // never removes anything.
    let (planned, _, _) = run(home.path(), &["uninstall", "--all", "--dry-run"], "");
    assert!(
        planned
            .lines()
            .filter(|line| line.contains("would remove"))
            .count()
            > 5,
        "a dry run over an intact install plans nothing:\n{planned}"
    );

    std::fs::write(&record, "esto no es json {\n").expect("the record is damaged");
    let (_, said, ok) = run(home.path(), &["uninstall", "--all"], "");
    assert!(!ok, "an unreadable record was treated as an empty one");
    assert!(
        said.contains("install-record-unreadable"),
        "the refusal does not name what could not be read: {said}"
    );
    // What deleting it costs, because that is the only way forward and it is
    // not free.
    assert!(
        said.contains("forgets it created the files already there"),
        "the resolution does not say what starting a new record costs: {said}"
    );
    // And nothing was taken on the way to refusing.
    assert!(
        root.join("SKILL.md").is_file(),
        "the contract was removed under a record nobody could read"
    );
}

/// The two checks the gate makes *after* the tracker agrees, actually reached.
///
/// Both `stale_verdict` and `out_of_phase` are tested directly, in the library,
/// against hand-built runs — and both could be **disconnected from the gate**
/// with the whole suite still green. Measured: putting `&& false &&` in front of
/// each call inside `gate` left 733 library tests, 105 crossings and every
/// integration test passing. The rule this product is named for — *a verdict is
/// bound to exact bytes* — was one edit away from deciding nothing, and nothing
/// would have said so.
///
/// They live behind the tracker read, which is why nothing reached them: a
/// boundary never rides the renewal window, so every path here goes out to `gh`.
/// So the test brings a `gh`.
#[test]
fn the_gate_reaches_the_checks_that_run_after_the_tracker_agrees() {
    // This one carried the reason the others did not: *asserting here would
    // measure the machine rather than the gate*. It is a good reason and it was
    // applied to the wrong thing — a machine without the stand-in cannot measure
    // the gate either way, and returning green says it did.
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let head = rig.head.clone();

    let pointer = |state: &str, reviewed: &str| {
        let run = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": state,
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
            "reviewed_head": reviewed,
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&run).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let merge = |state: &str, reviewed: &str| {
        pointer(state, reviewed);
        let (out, err, _) = run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer(state),
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                r#"{"command":"gh pr merge 12"}"#,
            ],
            "",
        );
        format!("{out}{err}")
    };

    // A verdict published against bytes that have since moved.
    let stale = merge("review", &"0".repeat(40));
    assert!(
        stale.contains("verdict-bound-to-other-bytes"),
        "the gate delivered on a review of other bytes: {stale}"
    );

    // The floor, and it is the whole test: without it, a gate that refused
    // every delivery would pass the assertion above.
    let fresh = merge("review", &head);
    assert!(
        fresh.contains("allow"),
        "a delivery reviewed against these exact bytes was refused: {fresh}"
    );

    // And the phase check, reached through the same seam. `in-progress` is a
    // state where no verdict exists, so landing the work is refused even with
    // the head matching.
    let early = merge("in-progress", &head);
    assert!(
        early.contains("out-of-phase"),
        "the gate landed work from a state where no verdict exists: {early}"
    );
}

/// A local fast-forward to the branch's tracked remote is preparation, not delivery.
///
/// The gate used to read every `git merge` as landing shared work, including the
/// exact `git merge --ff-only origin/main` used to bring an isolated worktree up
/// to date before continuing it. That command changes only this checkout and can
/// neither publish nor create a merge commit, but `in-progress` refused it with
/// the same answer as a pull-request merge.
#[test]
fn an_exact_local_fast_forward_is_allowed_without_widening_other_merges() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let git = |arguments: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .expect("git runs for the fixture");
        assert!(
            output.status.success(),
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let branch = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "--show-current"])
        .output()
        .expect("git names the fixture branch");
    let branch = String::from_utf8_lossy(&branch.stdout).trim().to_owned();
    assert!(!branch.is_empty(), "the fixture is not on a branch");

    // Put one commit on the local tracking ref while leaving the checked-out
    // branch at its clean parent. No network is involved.
    git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--allow-empty",
        "--quiet",
        "-m",
        "upstream",
    ]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&["reset", "--hard", "--quiet", "HEAD^"]);
    git(&["remote", "add", "origin", "https://example.invalid/o/r.git"]);
    git(&["config", &format!("branch.{branch}.remote"), "origin"]);
    git(&[
        "config",
        &format!("branch.{branch}.merge"),
        "refs/heads/main",
    ]);

    let pointer = |state: &str, reviewed_head: Option<&str>| {
        let pointer = serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": state,
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
            "reviewed_head": reviewed_head,
        });
        std::fs::write(
            home.join(".estigia")
                .join("runs")
                .join("claude-abcd1234.json"),
            serde_json::to_string(&pointer).expect("the pointer serialises"),
        )
        .expect("the pointer is written");
    };
    let gate = |state: &str, command: &str| {
        let input = serde_json::json!({"command": command}).to_string();
        run_with_tracker(
            home,
            repo,
            bin,
            &issue_answer(state),
            &[
                "gate",
                "Bash",
                "--run-id",
                "claude-abcd1234",
                "--input",
                &input,
            ],
            "",
        )
    };

    pointer("in-progress", None);
    let (allowed, refused, ok) = gate("in-progress", "git merge --ff-only origin/main");
    assert!(
        ok,
        "the safe local fast-forward was refused: {allowed}{refused}"
    );
    assert!(
        allowed.contains("allow"),
        "the gate did not allow it: {allowed}"
    );

    pointer("in-progress", Some(&"0".repeat(40)));
    let (_, stale, ok) = gate("in-progress", "git merge --ff-only origin/main");
    assert!(!ok, "the local exception bypassed a stale verdict");
    assert!(
        stale.contains("verdict-bound-to-other-bytes"),
        "stale_verdict did not run first: {stale}"
    );
    pointer("in-progress", None);

    let (_, refused, ok) = gate("in-progress", "git merge --ff-only origin/other");
    assert!(!ok, "an untracked target was allowed");
    assert!(refused.contains("out-of-phase"), "wrong refusal: {refused}");
    assert!(
        refused.contains(
            "git merge: this step lands the work and issue #12 is in in-progress, where no verdict exists"
        ),
        "the existing out-of-phase message changed: {refused}"
    );
    assert!(
        refused.contains(
            "a review of this head. Publish the review target, move the issue to review, and deliver once somebody has answered"
        ),
        "the existing out-of-phase guidance changed: {refused}"
    );

    for state in ["analysis", "ready", "blocked"] {
        pointer(state, None);
        let (_, refused, ok) = gate(state, "git merge --ff-only origin/main");
        assert!(!ok, "the local exception widened into {state}");
        assert!(
            refused.contains("out-of-phase")
                && refused.contains(&format!(
                    "git merge: this step lands the work and issue #12 is in {state}, where no verdict exists"
                )),
            "{state} did not retain the existing refusal: {refused}"
        );
    }

    let ledger = std::fs::read_to_string(home.join(".estigia").join("decisions.jsonl"))
        .expect("the gate recorded its decisions");
    assert!(
        ledger.lines().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).expect("a ledger entry");
            entry["verdict"] == "allow" && entry["subject"] == "git merge"
        }),
        "the allowed fast-forward is absent from the ledger: {ledger}"
    );
}

/// Everything the two post-agreement checks need: a home, a checkout with a
/// commit in it, and a `gh` that answers.
struct TrackerRig {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
    bin: tempfile::TempDir,
    head: String,
}

/// The rig, or a failure that says what to run.
///
/// It used to answer `Option` and every caller opened with `let Some(rig) = …
/// else { return; }`, so where the example was missing all sixteen of these tests
/// reported **pass** having executed nothing. Measured at the base commit with
/// the fixture moved aside: `cargo test --test pipe` answered *106 passed*.
///
/// Which invocations are exposed is worth being exact about, because the first
/// version of this comment was not. A bare `cargo test` **does** build examples
/// — cargo documents it, *"examples — to ensure they compile"*, and
/// `cargo test --all-features --no-run` in a cold tree leaves a runnable
/// `fake_process.exe`. What does not build them is a **filtered** invocation:
/// `cargo test --test pipe`, `cargo test --lib <name>`, anything that selects a
/// target. That is not a corner — it is how every mutation measurement in this
/// repository is taken, which is the evidence `docs/honesty.md` is made of. A
/// cold worktree plus a filtered run is a green answer from sixteen functions
/// that returned on their first line. (`cargo clippy --all-targets` does not
/// help: it leaves a `.d` and a zero-byte `.rmeta`, no runnable file.)
///
/// So the skip is gone from the type rather than from the callers. There is no
/// value this can return that means *did not run*, and a missing helper is a
/// loud failure naming the command that fixes it.
fn tracker_rig() -> TrackerRig {
    // Found from the test binary rather than from the manifest, because the
    // profile and the target triple are not knowable from here. `release.yml`
    // runs `cargo test --release --target <triple>`, which puts the example under
    // `target/<triple>/release/examples/` — a hardcoded `target/debug/examples`
    // was absent there, so the first version of this assertion turned a silent
    // skip on all six release targets into a hard failure on all six, and no tag
    // could have been cut. A reviewer measured that before it shipped.
    //
    // `current_exe` is this test binary at `<…>/deps/pipe-<hash>`, so the example
    // sits beside its parent directory whatever the profile and triple are.
    let here = std::env::current_exe().expect("the test binary knows where it is");
    let built = here
        .parent()
        .and_then(std::path::Path::parent)
        .expect("the test binary is under a profile directory")
        .join("examples")
        .join(if cfg!(windows) {
            "fake_process.exe"
        } else {
            "fake_process"
        });
    // The command has to be the one that clears *this* block, not the one that
    // clears the usual one. `cargo build --examples` writes into `debug`, so on a
    // filtered release run it discharges nothing and the next attempt fails
    // identically — which this repository calls worse than naming nothing.
    //
    // So both the profile and the target are read back out of the path the
    // fixture was looked for in. A reviewer measured the half-done version: it
    // carried `--release` and not `--target`, and running exactly what it printed
    // left the triple's directory as empty as it found it. The layout is
    // `target/[<triple>/]<profile>/deps/<binary>`, so the directory above the
    // profile is either `target` itself or the triple.
    let profile = built.parent().and_then(std::path::Path::parent);
    let triple = profile
        .and_then(std::path::Path::parent)
        .filter(|above| above.file_name().is_some_and(|name| name != "target"))
        .and_then(std::path::Path::file_name)
        .map(|name| format!(" --target {}", name.to_string_lossy()))
        .unwrap_or_default();
    let flags = if cfg!(debug_assertions) {
        triple
    } else {
        format!(" --release{triple}")
    };
    assert!(
        built.is_file(),
        "the process fixture is not built, so this test would have measured \
         nothing: run `cargo build --examples{flags}`, or drop the target filter — \
         a bare `cargo test` builds it and `cargo test --test pipe` does not ({})",
        built.display()
    );
    let bin = tempfile::tempdir().expect("a directory for the fake gh");
    // A real executable named `gh`, not a script: on Windows a bare `gh`
    // resolves to `gh.exe` and never to `gh.cmd`, so a script fixture would let
    // the machine's own `gh` answer instead.
    std::fs::copy(
        &built,
        bin.path().join(if cfg!(windows) { "gh.exe" } else { "gh" }),
    )
    .expect("the fake gh is copied onto the path");

    let repo = tempfile::tempdir().expect("a directory for the checkout");
    let git = |arguments: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    let prepared = git(&["init", "--quiet"])
        && git(&[
            "-c",
            "user.email=nobody@example.invalid",
            "-c",
            "user.name=nobody",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "one",
        ]);
    assert!(prepared, "the fixture checkout could not be created");
    let head = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git answers for the fixture checkout");
    let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    assert!(
        !head.is_empty(),
        "the fixture checkout has no commit to name"
    );

    let home = tempfile::tempdir().expect("a directory for the fixture home");
    std::fs::create_dir_all(home.path().join(".estigia").join("runs"))
        .expect("the run pointer directory");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming"))
        .expect("the roaming directory the adapters write under");
    // The control surface, or the gate refuses before it decides anything.
    run_in(home.path(), repo.path(), &["setup", "claude-code"], "");
    TrackerRig {
        home,
        repo,
        bin,
        head,
    }
}

/// An issue the run holds, in the state its pointer says.
///
/// The `op-id` is thirty-two hex characters because `is_operation_id` demands
/// exactly that, and a marker that fails it is not authoritative — an earlier
/// version of this fixture used `op1` and the transport answered *"current live
/// holder is none"*, which looks exactly like a broken gate.
fn issue_answer(state: &str) -> String {
    let marker = format!(
        "<!-- issue-flow: claim run-id=claude-abcd1234 runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    let issue = serde_json::json!({
        "state": "OPEN",
        "labels": [{"name": format!("status:{state}")}],
        "comments": [{
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed by claude-abcd1234.\n\n{marker}\n"),
        }],
    });
    serde_json::to_string(&serde_json::json!([{
        "matches": "issue view",
        "stdout": issue.to_string(),
        "status": 0,
    }]))
    .expect("the script serialises")
}

/// [`run_in`], with a `gh` on the path and a script for it to answer from.
fn run_with_tracker(
    home: &std::path::Path,
    here: &std::path::Path,
    bin: &std::path::Path,
    answers: &str,
    arguments: &[&str],
    stdin: &str,
) -> (String, String, bool) {
    let mut child = tracker_command(home, here, bin, answers)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("the payload is written");
    let output = child.wait_with_output().expect("the process ends");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.success(),
    )
}

/// A child whose GitHub boundary is the stateful fake process.
fn tracker_command(
    home: &std::path::Path,
    here: &std::path::Path,
    bin: &std::path::Path,
    answers: &str,
) -> Command {
    let mut path = std::ffi::OsString::from(bin);
    path.push(if cfg!(windows) { ";" } else { ":" });
    path.push(std::env::var_os("PATH").unwrap_or_default());

    let mut command = Command::new(estigia());
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData").join("Roaming"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("PATH", path)
        .env("ESTIGIA_FAKE_ANSWERS", answers)
        .current_dir(here);
    command
}

/// CI release must refuse a receipt no distinct reviewer accepted, and must
/// refuse it *before* it touches the pull request.
///
/// The ordering assertion this replaces was source text only: a reviewer widened
/// the gate to `qualifying_review_verdict(..).or_else(|| Some(accepted))` — a
/// gate deciding nothing, which is the failure this crate exists to refuse — and
/// the whole suite stayed green, because the identifier was still textually
/// where the string test looked for it. This one drives the real server against
/// the fake tracker and watches for `pr ready` on the wire.
#[test]
fn ci_release_refuses_a_receipt_no_distinct_reviewer_accepted() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "release_ci",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "worktree": repo.display().to_string(),
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(12);
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert!(
        text.contains("qualifying-review-verdict-missing"),
        "an unreviewed receipt was refused for the wrong reason: {text}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("pr ready"),
        "CI release reached the pull request without a verdict: {calls}"
    );
}

/// Neither acquisition route hands an unresolved handoff back to the run that
/// asked for the review.
///
/// This is the whole mechanism of the change, and it was wired by a source-order
/// assertion alone: replacing `require_review_eligibility(..)?` with
/// `let _ = require_review_eligibility(..)` in both `claim` and `reclaim` leaves
/// the identifiers exactly where a string test looks for them, and the suite
/// stayed green while a publishing run could take back the item it is forbidden
/// to review. That is a gate deciding nothing — the failure this crate exists to
/// refuse — so both routes are now driven through the real server.
#[test]
fn no_acquisition_route_returns_an_unresolved_handoff_to_its_requester_over_the_wire() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let run_id = "claude-abcd1234";
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let comments = vec![
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
        comment(
            "IC_handoff",
            "2026-08-14T10:00:00Z",
            marker(
                "review-handoff",
                &[
                    ("run-id", run_id),
                    ("target-op", &target),
                    ("op-id", &"f".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("authority", "ask"),
                    ("requested-at", "2026-08-14T10:00:00Z"),
                    ("deadline", "2026-08-14T10:30:00Z"),
                    ("blocker", "no independent context in this run"),
                    ("discharger", "another run records the verdict"),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // Both doors, one fixture: the requester is refused whichever it tries.
    for (tool, arguments) in [
        (
            "claim",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "horizon": "2099-01-01T00:00Z",
                "state": "review",
            }),
        ),
        (
            "reclaim",
            serde_json::json!({
                "issue": 12,
                "run_id": run_id,
                "horizon": "2099-01-01T00:00Z",
            }),
        ),
    ] {
        let log = trace.path().join(format!("{tool}.log"));
        let count = trace.path().join(format!("{tool}-count.json"));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert_eq!(response["result"]["isError"], true, "{tool}: {response}");
        assert!(
            text.contains("review-handoff-requester-excluded"),
            "{tool} did not refuse the requesting run: {text}"
        );
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !calls.contains("issue comment") && !calls.contains("issue edit"),
            "{tool} wrote to the tracker before refusing: {calls}"
        );
    }
}

/// A handoff names the latest receipt or it names nothing.
///
/// `handoff_review` carries this check twice — before the marker and before the
/// release — and neither copy was held: neutering both left the whole suite
/// green. It is load-bearing, not belt-and-braces. `review_eligibility` collects
/// handoffs only for the *latest* publication, so a handoff recorded against a
/// superseded receipt excludes nobody while the ownership epoch has already been
/// released — the publishing run is immediately eligible again for the item it
/// is the one run forbidden to review. That is this issue's own livelock, with
/// an audit trail.
#[test]
fn a_handoff_against_a_superseded_receipt_is_refused_before_anything_is_written() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let target = "a".repeat(32);
    let superseded = "e".repeat(32);
    let latest = "f".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let publication = |epoch: &str| {
        marker(
            "published",
            &[
                ("run-id", run_id),
                ("epoch", epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        )
    };
    // A live claim, and a republish after the epoch this request names.
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &target),
                ],
            ),
        ),
        comment("IC_first", "2026-08-14T09:10:00Z", publication(&superseded)),
        comment("IC_second", "2026-08-14T09:20:00Z", publication(&latest)),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "handoff_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "target_operation": target,
                // The epoch a republish has already invalidated.
                "epoch": superseded,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": "no independent context in this run",
                "discharger": "another run records the verdict",
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(12);
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert!(
        text.contains("published-receipt-mismatch"),
        "a superseded receipt was refused for the wrong reason: {text}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "a handoff for a superseded receipt was written to the timeline: {calls}"
    );
    assert!(
        !calls.contains("issue edit"),
        "a handoff for a superseded receipt released ownership: {calls}"
    );
}

/// A run that does not hold the live claim cannot hand the review off.
///
/// This check was wired by a string search for `verify_claim(` and nothing else,
/// and it is not cosmetic: `plan_release` accepts a run found live **or** stale,
/// so without it a run whose horizon has passed could still write the immutable
/// handoff marker — which excludes the publisher from both acquisition routes
/// and from the queue — and release the epoch underneath whoever now holds it.
/// The state readback only fires after that write.
#[test]
fn a_run_that_lost_the_claim_cannot_hand_the_review_off() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let publisher = "claude-abcd1234";
    // Nobody else needs to hold it: an expired claim is enough.
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    // The publisher's own claim, expired. This is the case that isolates the
    // check: `plan_release` accepts a run found live **or** stale, so it would
    // happily release this epoch, and an absent claim would have been refused by
    // `plan_release` alone — measuring nothing.
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", publisher),
                    ("runtime", "claude"),
                    ("horizon", "2020-01-01T00:00Z"),
                    ("op-id", &target),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", publisher),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "handoff_review",
            "arguments": {
                "issue": 12,
                "run_id": publisher,
                "target_operation": target,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": "no independent context in this run",
                "discharger": "another run records the verdict",
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(publisher.to_owned());
    run.issue = Some(12);
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(response["result"]["isError"], true, "{response}");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "a run that had lost the claim still wrote the handoff marker: {calls}"
    );
    assert!(
        !calls.contains("issue edit"),
        "a run that had lost the claim still released ownership: {calls}"
    );
}

/// The review queue is the third place the same exclusion is enforced, and it
/// hides the item from its requester without hiding it from anybody else. It
/// also refuses the whole queue when one candidate timeline cannot be read.
///
/// Both were promised in `bindings/github.md` and the tool description while
/// nothing measured either: pushing every candidate to `eligible` regardless of
/// eligibility, or falling back to an empty timeline on an unreadable candidate,
/// each left the suite green.
#[test]
fn the_review_queue_hides_a_handoff_from_its_requester_and_fails_closed_on_an_unreadable_one() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let requester = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let timeline = vec![
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", requester),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
        comment(
            "IC_handoff",
            "2026-08-14T10:00:00Z",
            marker(
                "review-handoff",
                &[
                    ("run-id", requester),
                    ("target-op", &"a".repeat(32)),
                    ("op-id", &"f".repeat(32)),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                    ("authority", "ask"),
                    ("requested-at", "2026-08-14T10:00:00Z"),
                    ("deadline", "2026-08-14T10:30:00Z"),
                    ("blocker", "no independent context in this run"),
                    ("discharger", "another run records the verdict"),
                ],
            ),
        ),
    ];
    let queue = serde_json::json!([{
        "number": 12,
        "title": "a published receipt awaiting review",
        "labels": [{"name": "status:review"}, {"name": "domain:general"}],
        "createdAt": "2026-08-14T08:00:00Z",
    }])
    .to_string();
    let answers = |candidate: serde_json::Value| {
        serde_json::to_string(&serde_json::json!([
            { "matches": "issue list", "stdout": queue, "status": 0 },
            candidate,
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises")
    };
    let readable = answers(serde_json::json!({
        "matches": "issue view",
        "stdout": serde_json::json!({ "comments": timeline }).to_string(),
        "status": 0,
    }));
    // A candidate whose timeline cannot be read. Not an empty queue — an
    // unreadable one, and the difference is the whole point.
    let unreadable = answers(serde_json::json!({
        "matches": "issue view",
        "stdout": "",
        "stderr": "gh: could not read the issue",
        "status": 1,
    }));

    let ask = |run_id: &str, answers: &str, label: &str| -> serde_json::Value {
        let log = trace.path().join(format!("{label}.log"));
        let count = trace.path().join(format!("{label}-count.json"));
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_state",
                "arguments": { "state": "review", "run_id": run_id },
            },
        })
        .to_string();
        let mut child = tracker_command(home, repo, bin, answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "the MCP response is not JSON: {}",
                String::from_utf8_lossy(&output.stdout)
            )
        })
    };

    let mine = ask(requester, &readable, "requester");
    let text = mine["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(mine["result"]["isError"], false, "{mine}");
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        answer["count"], 0,
        "the requesting run was offered its own unresolved handoff: {text}"
    );
    assert_eq!(answer["excluded_count"], 1, "{text}");
    assert_eq!(
        answer["excluded"][0]["reason"], "review-handoff-requester-excluded",
        "{text}"
    );

    let theirs = ask("codex-99999999", &readable, "distinct");
    let text = theirs["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(theirs["result"]["isError"], false, "{theirs}");
    let answer: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("the queue answer is JSON: {text}"));
    assert_eq!(
        answer["count"], 1,
        "a distinct run could not see the handed-off item: {text}"
    );
    assert_eq!(answer["excluded_count"], 0, "{text}");

    let blind = ask("codex-99999999", &unreadable, "unreadable");
    assert_eq!(
        blind["result"]["isError"], true,
        "an unreadable candidate timeline was answered as a queue: {blind}"
    );

    // And the list itself, answering something that is not a list. A failed
    // `gh` call never reaches this — the transport refuses that already — so
    // the case left is a success whose body is not a queue, and treating it as
    // an empty one sends a run to triage believing there is no work.
    let no_list = serde_json::to_string(&serde_json::json!([
        { "matches": "issue list", "stdout": "null", "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let unread = ask("codex-99999999", &no_list, "no-list");
    assert_eq!(
        unread["result"]["isError"], true,
        "an unreadable queue was answered as an empty one: {unread}"
    );
}

/// A verdict needs a live claim, and it cannot credit a run that asked for the
/// review rather than performing it.
///
/// Both halves were unmeasured. Narrowing the distinctness rule to the publisher
/// alone left the suite green, and so did `let _ = verify_claim(..)` — the second
/// being the only thing that stops an excluded publisher recording a verdict for
/// its own handoff and lifting its own exclusion. Each case here writes nothing,
/// which is what a neutered check would give away.
#[test]
fn a_verdict_needs_a_live_claim_and_a_reviewer_nobody_asked_for() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let publisher = "claude-abcd1234";
    let requester = "codex-beef0000";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let claim_by = |run: &str| {
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        )
    };
    let published = comment(
        "IC_publication",
        "2026-08-14T09:10:00Z",
        marker(
            "published",
            &[
                ("run-id", publisher),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
            ],
        ),
    );
    // A handoff written by somebody who is not the publisher, so the requester
    // half of the rule is the only thing that can refuse them.
    let handoff = comment(
        "IC_handoff",
        "2026-08-14T10:00:00Z",
        marker(
            "review-handoff",
            &[
                ("run-id", requester),
                ("target-op", &"a".repeat(32)),
                ("op-id", &"f".repeat(32)),
                ("epoch", &epoch),
                ("pr", "7"),
                ("head", &head),
                ("base", &base),
                ("digest", &digest),
                ("authority", "ask"),
                ("requested-at", "2026-08-14T10:00:00Z"),
                ("deadline", "2026-08-14T10:30:00Z"),
                ("blocker", "no independent context in this run"),
                ("discharger", "another run records the verdict"),
            ],
        ),
    );

    let stale = "9".repeat(32);
    for (label, holder, caller, reviewer, named) in [
        // The claim holder tries to credit the run that asked for the review.
        ("requester", publisher, publisher, requester, &epoch),
        // A run that does not hold the claim tries to record at all.
        ("no-claim", requester, publisher, "gemini-c0ffee00", &epoch),
        // A verdict over an epoch that is not the one on the timeline. The read
        // side ignores a stale receipt anyway, so this is the earlier refusal —
        // the one that keeps a superseded epoch from ever being written down as
        // reviewed.
        ("stale", publisher, publisher, "gemini-c0ffee00", &stale),
    ] {
        let log = trace.path().join(format!("{label}.log"));
        let count = trace.path().join(format!("{label}-count.json"));
        let answers = serde_json::to_string(&serde_json::json!([
            {
                "matches": "issue view",
                "stdout": serde_json::json!({
                    "state": "OPEN",
                    "assignees": [],
                    "labels": [{"name": "status:review"}],
                    "comments": [claim_by(holder), published.clone(), handoff.clone()],
                }).to_string(),
                "status": 0,
            },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "record_review_verdict",
                "arguments": {
                    "issue": 12,
                    "run_id": caller,
                    "reviewer": reviewer,
                    "epoch": named,
                    "pr": 7,
                    "head": head,
                    "base": base,
                    "digest": digest,
                    "outcome": "accepted",
                }
            }
        })
        .to_string();

        let runs = home.join(".estigia").join("runs");
        let _ = std::fs::remove_file(runs.join(format!("{caller}.json")));
        let mut run = estigia::harness::session::Run::new(caller.to_owned());
        run.issue = Some(12);
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        assert_eq!(response["result"]["isError"], true, "{label}: {response}");
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            !calls.contains("issue comment"),
            "{label}: a refused verdict still wrote to the timeline: {calls}"
        );
    }
}

/// The run that published cannot be the run credited with reviewing it.
///
/// `record_review_verdict` had no test of any kind: both of its refusals could
/// be replaced with `if false` and the suite stayed green. This drives the real
/// operation and asserts nothing was written.
#[test]
fn a_verdict_cannot_credit_the_run_that_published_the_receipt() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let comments = vec![
        comment(
            "IC_claim",
            "2026-08-14T09:00:00Z",
            marker(
                "claim",
                &[
                    ("run-id", run_id),
                    ("runtime", "claude"),
                    ("horizon", "2099-01-01T00:00Z"),
                    ("op-id", &"a".repeat(32)),
                ],
            ),
        ),
        comment(
            "IC_publication",
            "2026-08-14T09:10:00Z",
            marker(
                "published",
                &[
                    ("run-id", run_id),
                    ("epoch", &epoch),
                    ("pr", "7"),
                    ("head", &head),
                    ("base", &base),
                    ("digest", &digest),
                ],
            ),
        ),
    ];
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "assignees": [],
                "labels": [{"name": "status:review"}],
                "comments": comments,
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "record_review_verdict",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                // The publishing run, naming itself as its own reviewer.
                "reviewer": run_id,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "outcome": "accepted",
            }
        }
    })
    .to_string();

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(12);
    run.state = Some("review".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");

    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert!(
        text.contains("reviewer-not-distinct"),
        "the publisher was refused for the wrong reason: {text}"
    );
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !calls.contains("issue comment"),
        "a refused verdict still wrote to the timeline: {calls}"
    );
}

/// The compound handoff must make its evidence visible before it releases the
/// named epoch, and only final convergence may clear the local pointer.
#[test]
fn review_handoff_orders_evidence_release_and_pointer_clear() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let count = trace.path().join("count.json");
    let log = trace.path().join("calls.log");
    let run_id = "claude-abcd1234";
    let target = "a".repeat(32);
    let epoch = "e".repeat(32);
    let head = "b".repeat(40);
    let base = "c".repeat(40);
    let digest = "d".repeat(64);
    let blocker = "no independent context in this run";
    let discharger = "another run records the exact-receipt verdict";
    let operation = estigia::transport::claim::review_operation_id(
        "review-handoff",
        &[
            run_id, &target, &epoch, "7", &head, &base, &digest, blocker, discharger,
        ],
    );
    let release =
        estigia::transport::claim::review_operation_id("review-handoff-release", &[&operation]);
    let marker = |kind, fields: &[(&str, &str)]| {
        estigia::transport::markers::render(kind, fields).expect("a protocol marker")
    };
    let claim = marker(
        "claim",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("horizon", "2099-01-01T00:00Z"),
            ("op-id", &target),
        ],
    );
    let publication = marker(
        "published",
        &[
            ("run-id", run_id),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
        ],
    );
    let handoff = marker(
        "review-handoff",
        &[
            ("run-id", run_id),
            ("target-op", &target),
            ("op-id", &operation),
            ("epoch", &epoch),
            ("pr", "7"),
            ("head", &head),
            ("base", &base),
            ("digest", &digest),
            ("authority", "ask"),
            ("requested-at", "2026-08-14T10:00:00Z"),
            ("deadline", "2026-08-14T10:00:00Z"),
            ("blocker", blocker),
            ("discharger", discharger),
        ],
    );
    let unassign = marker(
        "unassign",
        &[
            ("run-id", run_id),
            ("runtime", "claude"),
            ("target-op", &target),
            ("op-id", &release),
        ],
    );
    let comment = |id: &str, at: &str, body: String| {
        serde_json::json!({
            "id": id,
            "createdAt": at,
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": body,
        })
    };
    let before = vec![
        comment("IC_claim", "2026-08-14T09:00:00Z", claim),
        comment("IC_publication", "2026-08-14T09:10:00Z", publication),
    ];
    let mut after_handoff = before.clone();
    after_handoff.push(comment("IC_handoff", "2026-08-14T10:00:00Z", handoff));
    let mut after_release = after_handoff.clone();
    after_release.push(comment("IC_release", "2026-08-14T10:01:00Z", unassign));
    let issue = |state: &str, comments: &[serde_json::Value]| {
        serde_json::json!({
            "state": "OPEN",
            "assignees": [],
            "labels": [{"name": format!("status:{state}")}],
            "comments": comments,
        })
        .to_string()
    };
    let scripted = |final_state: &str| {
        let mut bodies = vec![
            issue("review", &before),
            issue("review", &before),
            issue("review", &before),
            issue("review", &after_handoff),
            issue("review", &after_handoff),
            issue("review", &after_handoff),
        ];
        // Unassign reads once after its marker, then ownership projection takes
        // two complete seven-read samples even when the first two agree.
        bodies.extend((0..16).map(|_| issue("review", &after_release)));
        bodies.push(issue(final_state, &after_release));
        let mut answers = bodies
            .iter()
            .enumerate()
            .map(|(index, stdout)| {
                serde_json::json!({
                    "matches": "issue view",
                    "nth": index + 1,
                    "stdout": stdout,
                    "status": 0,
                })
            })
            .collect::<Vec<_>>();
        answers.push(serde_json::json!({
            "matches": "api user",
            "stdout": "{\"login\":\"fixture\"}",
            "status": 0,
        }));
        serde_json::to_string(&answers).expect("the fake tracker script serialises")
    };
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "handoff_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "target_operation": target,
                "epoch": epoch,
                "pr": 7,
                "head": head,
                "base": base,
                "digest": digest,
                "blocker": blocker,
                "discharger": discharger,
            }
        }
    })
    .to_string();
    let runs = home.join(".estigia").join("runs");
    let put_pointer = || {
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(12);
        run.state = Some("review".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );
    };
    let invoke = |answers: &str| {
        let _ = std::fs::remove_file(&count);
        let _ = std::fs::remove_file(&log);
        let mut child = tracker_command(home, repo, bin, answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        child.wait_with_output().expect("the MCP server exits")
    };

    put_pointer();
    let output = invoke(&scripted("review"));
    assert!(output.status.success(), "the MCP process failed");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(response["result"]["isError"], false, "{response}");
    assert_eq!(
        estigia::harness::session::load(&runs, run_id).issue,
        None,
        "a converged handoff left the publishing run's pointer held"
    );
    let calls = std::fs::read_to_string(&log).expect("the fake tracker logged its calls");
    let calls: Vec<&str> = calls.lines().collect();
    let handoff_write = calls
        .iter()
        .position(|line| line.contains("issue comment 12") && line.contains("12-published.md"))
        .expect("the handoff marker was written");
    let release_write = calls
        .iter()
        .position(|line| line.contains("issue comment 12") && line.contains("unassign-12"))
        .expect("the release marker was written");
    assert!(
        calls[handoff_write + 1..release_write]
            .iter()
            .filter(|line| line.contains("issue view 12"))
            .count()
            >= 3,
        "the operation released before handoff readback and the final receipt check: {calls:#?}"
    );

    put_pointer();
    let output = invoke(&scripted("in-progress"));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP refusal is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(response["result"]["isError"], true, "{response}");
    assert_eq!(
        estigia::harness::session::load(&runs, run_id).issue,
        Some(12),
        "a handoff that failed final state convergence cleared the run pointer"
    );
}

/// What the gate let through, named in the ledger by the real binary.
///
/// The record used to identify what Estigia stopped and not what it allowed: a
/// refusal is prefixed with its command, an allow was not. Measured under one
/// live claim, `git tag v1.0`, `gh release create v1.0` and `git push --force
/// origin main` left three identical lines — `tool=Bash verdict=allow
/// detail=issue #12 is held by claude-abcd1234`.
///
/// Through the binary, because the fix is one argument at a call site and
/// `note` already had its own test: passing `None` there left the whole suite
/// green, which is the shape this file exists to catch.
#[test]
fn the_ledger_names_the_boundary_the_gate_let_through() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let run = serde_json::json!({
        "run_id": "claude-abcd1234",
        "issue": 12,
        "revision": 1,
        "state": "in-progress",
        "repo_dir": repo,
        "worktree": serde_json::Value::Null,
    });
    std::fs::write(
        home.join(".estigia")
            .join("runs")
            .join("claude-abcd1234.json"),
        serde_json::to_string(&run).expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // The two irreversible steps that are **not** deliveries — publishing a
    // review target is how a run reaches review, so `in-progress` lets them
    // through. A `git tag` here would be refused as out-of-phase, which is the
    // first fixture this test had and the reason it is worth writing down.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Bash",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"command":"git push --force origin main"}"#,
        ],
        "",
    );
    // And a write, whose subject is the file.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "gate",
            "Write",
            "--run-id",
            "claude-abcd1234",
            "--input",
            r#"{"file_path":"src/main.rs"}"#,
        ],
        "",
    );

    // A **different** boundary from the one above, and that is the whole of
    // why this catches anything: both doors sending `git push` made the
    // hook's line a duplicate of the gate's, so dropping the hook's subject
    // left the assertions passing on the other door's answer.
    //
    // And once through the agent's door, which is a different call site with
    // the same argument. Driving only `estigia gate` left the hook's own call
    // passing `None` with this test still green — two doors, one rule, and a
    // test that walked one of them.
    //
    // No session id, so the hook asks the checkout who holds it, which is the
    // same position a git hook is in.
    run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &[
            "hook",
            "pre-tool-use",
            "--agent",
            "claude-code",
            "--dialect",
            "claude-code",
        ],
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "gh pr create --fill"},
            "cwd": repo,
        })
        .to_string(),
    );

    let ledger = std::fs::read_to_string(home.join(".estigia").join("decisions.jsonl"))
        .expect("the ledger was written");
    let subjects: Vec<String> = ledger
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|entry| entry.get("verdict").and_then(|v| v.as_str()) == Some("allow"))
        .map(|entry| {
            entry
                .get("subject")
                .and_then(|value| value.as_str())
                .unwrap_or("<unnamed>")
                .to_owned()
        })
        .collect();

    for wanted in ["git push", "gh pr create", "src/main.rs"] {
        assert!(
            subjects.iter().any(|subject| subject == wanted),
            "the ledger does not say {wanted} went through: {subjects:?} in {ledger}"
        );
    }
    // The floor: three steps, three distinguishable lines. Without it a record
    // that named the same thing every time would pass the assertions above.
    assert_eq!(
        subjects.len(),
        3,
        "expected one allowed line per step, got {subjects:?}"
    );
}

/// A gate whose entry this build cannot read stops saying `on`.
///
/// The other half of a fix that landed one round earlier in `doctor` and not
/// here. Two readers disagree by design: `is_gated` recognises the entry by
/// `hook pre-tool-use`, and `wiring::wire_in` requires the executable's own file
/// name to hold `estigia`. A copy renamed to anything else is plain to the first
/// and invisible to the second, so `registered` comes back empty and there is no
/// wire whose fault to report.
///
/// Measured: pointing `.claude/settings.json` at `…\ausente.exe` made `doctor`
/// answer `BROKEN gate` and `status` go on saying `gate on, tools on` about the
/// same machine. `status` is the one people read first — the argument this row
/// was built on — which makes it the half that mattered.
#[test]
fn a_gate_whose_entry_cannot_be_read_stops_saying_it_is_on() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    // The floor: a working install says `on`, or "it stops saying on" is a
    // sentence about a machine that never said it.
    let (before, _, _) = run(home.path(), &["status"], "");
    assert!(
        before.contains("gate on"),
        "a fresh install does not report its gate:\n{before}"
    );
    // And the other floor: the ten agents with no gate at all carry no fault.
    // A report that complains about everybody teaches people to ignore it,
    // which is the one thing a health report cannot survive.
    //
    // It guards the printed report and not the condition behind it: `status`
    // prints a fault only for an agent it lists as present, so dropping the
    // `is_gated` half of that condition changes nothing here. Said rather than
    // left looking measured.
    assert!(
        !before.contains("an entry is registered here"),
        "an agent with no gate registered was told its entry cannot be read:\n{before}"
    );

    let settings = home.path().join(".claude").join("settings.json");
    let text = std::fs::read_to_string(&settings).expect("the settings file was written");
    let renamed = text.replace("estigia.exe", "ausente.exe").replace(
        // The Unix spelling, so this measures the same thing on both platforms.
        //
        // Escaped, because the settings file is JSON and the command it holds is
        // a JSON string: what stands in the file after the path is `\"`, not
        // `"`. Written unescaped, this arm matched nothing on Linux and macOS —
        // the `.exe` arm above carried the test on Windows and the assertion
        // below caught the silence everywhere else, which is what it is for.
        "/estigia\\\"",
        "/ausente\\\"",
    );
    assert_ne!(renamed, text, "the fixture did not rename the executable");
    std::fs::write(&settings, &renamed).expect("their file");

    let (after, _, _) = run(home.path(), &["status"], "");
    assert!(
        !after.contains("gate on"),
        "a gate naming a binary this build cannot recognise is still reported on:\n{after}"
    );
    assert!(
        after.contains("REGISTERED BUT DEAD"),
        "the gate's state is no longer named:\n{after}"
    );
    assert!(
        after.contains("an entry is registered here"),
        "the report does not say what is wrong with it:\n{after}"
    );
    // And the command that rewrites the entry, which is what discharges it.
    assert!(
        after.contains("estigia setup claude-code"),
        "the fault is reported with no command that discharges it:\n{after}"
    );
}

/// A row this build refuses can still be set back to something it accepts.
///
/// The trap a tightening opens. `BoardRef::parse` learned the shape the
/// transport reads a board with, and a `Project board` an older build had
/// written then made **every** `config set` on that machine refuse — naming the
/// file and no command, including the one way out, which is setting the row back
/// to `none`.
///
/// The gate already keeps what parses for exactly this reason. The writer had no
/// such sentence, so a value the tool itself had accepted became a table nothing
/// could repair through the tool that wrote it.
#[test]
fn a_row_this_build_refuses_can_still_be_set_back_to_one_it_accepts() {
    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    run(home.path(), &["setup", "claude-code"], "");

    // A row an older build would have written and this one refuses, put there
    // the way that build would have: straight into the table.
    let contract = home
        .path()
        .join(".claude")
        .join("skills")
        .join(estigia::skill::DIRECTORY)
        .join("SKILL.md");
    let text = std::fs::read_to_string(&contract).expect("the contract is installed");
    let broken = text.replace("| Project board | none |", "| Project board | acme/seven |");
    assert_ne!(broken, text, "the fixture did not reach the board row");
    std::fs::write(&contract, &broken).expect("their file");

    // The floor: this build really does refuse that value, or the test is about
    // a machine that was never in the state it describes.
    let (_, refused, ok) = run(
        home.path(),
        &["config", "set", "Project board", "acme/seven"],
        "",
    );
    assert!(
        !ok && refused.contains("board"),
        "the value this is about is not refused:\n{refused}"
    );

    // And the way out works.
    let (said, err, ok) = run(home.path(), &["config", "set", "Project board", "none"], "");
    assert!(
        ok,
        "a row this build refuses left the table unrepairable:\n{said}{err}"
    );
    let after = std::fs::read_to_string(&contract).expect("the contract survives");
    assert!(
        after.contains("| Project board | none |"),
        "the row was reported set and the table does not hold it:\n{after}"
    );
    // And the rows beside it are still theirs — a bad row costs its own value
    // and nothing else.
    assert!(
        after.contains("| Merge strategy |"),
        "repairing one row cost the others:\n{after}"
    );

    // And the same way out with an agent named, which is a different reader:
    // `--agent` goes through `writable_config` and the line above through
    // `contract_of`. Both had the strict read and only one of them was walked.
    std::fs::write(&contract, &broken).expect("their file");
    let (said, err, ok) = run(
        home.path(),
        &[
            "config",
            "set",
            "--agent",
            "claude-code",
            // A row that **is** an agent's. `Project board` is written for the
            // machine and `--agent` refuses it by design, so asking for that one
            // here would measure the wrong refusal.
            "Blind judges",
            "two blind",
        ],
        "",
    );
    assert!(
        ok,
        "a bad board row stopped an agent's own row being written:\n{said}{err}"
    );
}

/// `release` does not report a release the transport did not perform.
///
/// `unassign` is two calls. The first is **discovery**: it answers
/// `write_performed: false` with the epoch it found and *"repeat unassign with
/// --target-operation and the same operation ID"*. This verb threw that answer
/// away and printed `released: <run> no longer holds #<issue>`.
///
/// Measured on the installed binary: after that sentence the pointer still held
/// the issue — correctly, the harness reads `write_performed` and keeps it — and
/// the next write went through with `allow — issue #12 was verified inside the
/// renewal window`. The gate was right and the message was wrong, in the one
/// command whose whole job is putting a claim down.
#[test]
fn release_does_not_report_a_release_the_transport_did_not_perform() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let pointer = home
        .join(".estigia")
        .join("runs")
        .join("claude-abcd1234.json");
    std::fs::write(
        &pointer,
        serde_json::to_string(&serde_json::json!({
            "run_id": "claude-abcd1234",
            "issue": 12,
            "revision": 1,
            "state": "in-progress",
            "repo_dir": repo,
            "worktree": serde_json::Value::Null,
        }))
        .expect("the pointer serialises"),
    )
    .expect("the pointer is written");

    // A world that answers the timeline and nothing else, so the commit half of
    // the release cannot land. What must not happen is being told it did.
    let (said, err, ok) = run_with_tracker(
        home,
        repo,
        bin,
        &issue_answer("in-progress"),
        &["release", "--run-id", "claude-abcd1234"],
        "",
    );
    let told = format!("{said}{err}");
    assert!(
        !told.contains("no longer holds"),
        "a release the transport did not perform was reported as done:\n{told}"
    );
    assert!(
        !ok,
        "a release that did not happen exited as though it had:\n{told}"
    );

    // And the claim is still held, which is the half the gate got right all
    // along — so the message was the only thing lying.
    let after = std::fs::read_to_string(&pointer).expect("the pointer survives");
    assert!(
        after.contains("\"issue\":12") || after.contains("\"issue\": 12"),
        "the pointer stopped holding the issue nobody released:\n{after}"
    );
}

/// One open pull request, as `gh pr list` answers it.
/// The pull request `published` finds on its second list, once created.
const LISTED_SEVEN: &str = "[{\"number\":7,\"url\":\"https://github.com/o/r/pull/7\",\"headRefOid\":\"0000000000000000000000000000000000000000\",\"baseRefOid\":\"0000000000000000000000000000000000000000\",\"isDraft\":true}]";

const LISTED_PR: &str =
    "[{\"number\":99,\"url\":\"u\",\"headRefOid\":\"x\",\"baseRefOid\":\"y\",\"isDraft\":true}]";

/// A closing keyword refuses **before** the branch reaches the remote.
///
/// It refused after: the push had landed and the pull request was open when
/// `publish_review` answered `closing-keyword-live`, whose outcome line reads
/// *"nothing was written"*. A run that believes it leaves an orphan branch and
/// an orphan pull request nobody knows exist, and the next call fails for an
/// unrelated reason — the operator debugging it starting from a false premise
/// supplied by the tool whose whole job is to be the honest one.
///
/// `Closes #n` in a commit message is readable locally, so the refusal costs
/// nothing where it belongs. The assertion is the acceptance criterion's own:
/// the remote is unchanged.
#[test]
fn a_closing_keyword_refuses_before_the_branch_reaches_the_remote() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-something";

    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success()),
        "the bare origin was not created"
    );
    let origin_url = origin.path().display().to_string();
    assert!(git(&["remote", "add", "origin", &origin_url]));
    assert!(git(&["branch", "-M", "main"]));
    // A tree with something in it: an empty one is not a delivery target, and
    // `clean_target` says so before anything about keywords is decided.
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file is written");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]));
    assert!(
        git(&["push", "-q", "origin", "main"]),
        "the base did not push"
    );

    let body = trace.path().join("pr-body.md");

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    // Both shapes. The reused one is the field report's own — its pull request
    // already existed on the retry — and it is the one that stayed broken when
    // the scan merely moved above the push: `ensure_draft` and `edit_pr` write
    // to the live pull request before it.
    // Two shapes of pull-request list, and two sources of the keyword. The
    // reused shape is the field report's own — its pull request already existed
    // on the retry — and it is the one that stayed broken when the scan merely
    // moved above the push, because `ensure_draft` and `edit_pr` write to the
    // live pull request first. The source is a dimension because the body was
    // the half no test exercised, and the body is what that refusal published.
    for (listed, source, message, body_text) in [
        (
            "[]",
            "commit message",
            "make it better

Closes #12",
            "names nothing",
        ),
        (
            LISTED_PR,
            "commit message",
            "make it better

Closes #12",
            "names nothing",
        ),
        ("[]", "pr body", "make it better", "Closes #12"),
        (LISTED_PR, "pr body", "make it better", "Closes #12"),
    ] {
        // Rebuilt per case so exactly one source carries the keyword, which is
        // what makes each half of the scan load-bearing on its own.
        assert!(git(&["checkout", "-q", "-B", branch, "main"]));
        std::fs::write(
            repo.join("kept.txt"),
            format!(
                "{source}
"
            ),
        )
        .expect("the change");
        assert!(git(&["add", "kept.txt"]));
        assert!(
            git(&[
                "-c",
                "user.email=nobody@example.invalid",
                "-c",
                "user.name=nobody",
                "commit",
                "--quiet",
                "-m",
                message,
            ]),
            "the commit was not made"
        );
        std::fs::write(
            &body,
            format!(
                "{body_text}
"
            ),
        )
        .expect("the body is written");
        let answers = serde_json::to_string(&serde_json::json!([
            {
                "matches": "issue view",
                "stdout": serde_json::json!({
                    "state": "OPEN",
                    "labels": [{"name": "status:in-progress"}],
                    "comments": [{
                        "id": "IC_1",
                        "createdAt": "2026-01-01T00:00Z",
                        "viewerDidAuthor": true,
                        "includesCreatedEdit": false,
                        "body": format!("Claimed.\n\n{claim}\n"),
                    }],
                }).to_string(),
                "status": 0,
            },
            { "matches": "pr list", "stdout": listed, "status": 0 },
            { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
        ]))
        .expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        // The store is revision-guarded, so the second shape needs a clean pointer.
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(12);
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "publish_review",
                "arguments": {
                    "issue": 12,
                    "run_id": run_id,
                    "branch": branch,
                    "base": "main",
                    "pr_title": "Something",
                    "pr_body_file": body.display().to_string(),
                    "worktree": repo.display().to_string(),
                }
            }
        })
        .to_string();

        let log = trace.path().join("calls.log");
        let count = trace.path().join("count.json");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();

        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(
            text.contains("closing-keyword-live"),
            "the keyword was not what refused it: {text}"
        );
        assert!(
            text.contains("nothing was written"),
            "the pre-push refusal did not report the world untouched ({source}): {text}"
        );

        // The acceptance criterion, literally: the remote is unchanged.
        let refs = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["for-each-ref", "--format=%(refname)"])
            .output()
            .expect("the origin is readable");
        let refs = String::from_utf8_lossy(&refs.stdout).into_owned();
        assert!(
            !refs.contains(branch),
            "the branch reached the remote before the refusal: {refs}"
        );
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        for wrote in ["pr create", "pr edit", "pr ready"] {
            assert!(
                !calls.contains(wrote),
                "`{wrote}` reached the remote before the refusal ({listed}): {calls}"
            );
        }
    }
}

/// A refusal that arrives after the push says the write landed.
///
/// The transport had no way to say it: the outcome is derived from the exit
/// code, and a stop is `1` whether it refused before touching anything or after
/// pushing a branch and opening a pull request — so every refusal claimed
/// nothing had happened. The channel that fixes it was itself untested: renaming
/// the field in both producers left the whole suite green, which is this
/// repository's own definition of an untested fix.
///
/// Driven through the readback disagreement, which is a real post-push refusal:
/// the branch is on the remote and the pull request is open by the time the
/// head comes back wrong.
#[test]
fn a_refusal_after_the_push_reports_that_the_write_landed() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-published";

    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    assert!(git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string()
    ]));
    assert!(git(&["branch", "-M", "main"]));
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]));
    assert!(git(&["push", "-q", "origin", "main"]));
    assert!(git(&["checkout", "-q", "-b", branch]));
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        // No keyword: this run must reach the push.
        "a change that names no issue",
    ]));

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let (head_sha, base_sha) = (sha("HEAD"), sha("origin/main"));

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    // Both refusals that can arrive after the push, because each is its own
    // producer of the channel and only one of them was held: the readback
    // disagreeing, and a keyword arriving from the remote side of the pull
    // request — which the local precondition cannot see and does not claim to.
    // A third case whose honest answer is *unconfirmed* rather than *landed*:
    // the pull request is created and the list that follows does not show it.
    // That path is a `Failure::Read`, and the only thing keeping it from saying
    // `nothing was written` after a push is a `map_err` nothing tested.
    for (which, readback_head, remote_body, listed_after, landed) in [
        (
            "readback",
            "f".repeat(40),
            "names nothing",
            LISTED_SEVEN,
            true,
        ),
        ("remote keyword", head_sha, "Closes #12", LISTED_SEVEN, true),
        ("unconfirmed", "f".repeat(40), "names nothing", "[]", false),
    ] {
        let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:in-progress"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        // Nothing open before; the freshly created one after. `published`
        // lists twice, and the second list is how it learns the number.
        { "matches": "pr list", "nth": 1, "stdout": "[]", "status": 0 },
        {
            "matches": "pr list",
            "stdout": listed_after,
            "status": 0,
        },
        { "matches": "pr create", "stdout": "https://github.com/o/r/pull/7\n", "status": 0 },
        // The readback, disagreeing: a head nobody published.
        {
            "matches": "headRefOid",
            "stdout": serde_json::json!({
                "number": 7,
                "url": "https://github.com/o/r/pull/7",
                "headRefOid": readback_head,
                "baseRefOid": base_sha,
                "state": "OPEN",
                "isDraft": true,
            }).to_string(),
            "status": 0,
        },
        // What the remote side of the pull request says, for the second case.
        // The local scan never sees this, which is why the later one stays.
        {
            "matches": "json body",
            "stdout": serde_json::json!({ "body": remote_body }).to_string(),
            "status": 0,
        },
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": "fix/12-published",
                        "baseRefName": "main",
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

        let runs = home.join(".estigia").join("runs");
        // Revision-guarded: the second case needs a clean pointer.
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(12);
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "publish_review",
                "arguments": {
                    "issue": 12,
                    "run_id": run_id,
                    "branch": branch,
                    "base": "main",
                    "pr_title": "Something",
                    "pr_body_file": body.display().to_string(),
                    "worktree": repo.display().to_string(),
                }
            }
        })
        .to_string();

        let log = trace.path().join("calls.log");
        let count = trace.path().join("count.json");
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();

        assert_eq!(response["result"]["isError"], true, "{response}");
        // The branch really did reach the remote, which is what makes the wording
        // load-bearing rather than decorative.
        let refs = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["for-each-ref", "--format=%(refname)"])
            .output()
            .expect("the origin is readable");
        assert!(
            String::from_utf8_lossy(&refs.stdout).contains(branch),
            "the {which} case never got as far as pushing, so it proves nothing"
        );
        // Every case: the sentence this issue exists to eliminate must not be
        // the answer once the branch is on the remote.
        assert!(
            !text.contains("nothing was written"),
            "the {which} refusal after the push claimed nothing was written: {text}"
        );
        if landed {
            assert!(
                text.contains("the write landed"),
                "the {which} refusal did not report the write as landed: {text}"
            );
        }
        assert!(
            !text.contains("nothing was written"),
            "the {which} refusal after the push claimed both: {text}"
        );
    }
}

/// A body this run cannot read refuses before it touches the remote.
///
/// The read was `if let Ok(..)`, which treats a file that is not there as a
/// body with no keyword in it — the sentence `keywords_in_commits` was rewritten
/// to refuse, three lines away in the same commit. Propagating it was claimed
/// and not held: reverting those three lines left the whole suite green, and the
/// reverted code reaches `ensure_draft`, runs `gh pr ready --undo` against the
/// live pull request, and *then* answers *nothing was written*.
///
/// The pull request here is deliberately not a draft, so `ensure_draft` has
/// something to do; that write is the one the revert performs and this refuses.
#[test]
fn an_unreadable_pr_body_refuses_before_the_remote_is_touched() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-unreadable";

    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    assert!(git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string()
    ]));
    assert!(git(&["branch", "-M", "main"]));
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]));
    assert!(git(&["push", "-q", "origin", "main"]));
    assert!(git(&["checkout", "-q", "-b", branch]));
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        // No keyword anywhere: the body being unreadable is the only fault.
        "a change that names no issue",
    ]));

    // Never written. That is the point.
    let body = trace.path().join("absent-body.md");

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:in-progress"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        // Open, and **ready** — so `ensure_draft` has a write to perform. That
        // write is what the reverted read reaches before refusing.
        {
            "matches": "pr list",
            "stdout": serde_json::json!([{
                "number": 99,
                "url": "https://github.com/o/r/pull/99",
                "headRefOid": "0".repeat(40),
                "baseRefOid": "0".repeat(40),
                "isDraft": false,
            }]).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(12);
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "publish_review",
            "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }
        }
    })
    .to_string();

    let log = trace.path().join("calls.log");
    let count = trace.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    assert_eq!(response["result"]["isError"], true, "{response}");
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    for wrote in ["pr ready", "pr edit", "pr create"] {
        assert!(
            !calls.contains(wrote),
            "`{wrote}` reached the remote before an unreadable body was refused: {calls}"
        );
    }
    let refs = Command::new("git")
        .arg("-C")
        .arg(origin.path())
        .args(["for-each-ref", "--format=%(refname)"])
        .output()
        .expect("the origin is readable");
    assert!(
        !String::from_utf8_lossy(&refs.stdout).contains(branch),
        "the branch reached the remote before an unreadable body was refused"
    );
    // And the sentence is true, which is the whole subject of this issue.
    assert!(
        text.contains("nothing was written"),
        "the refusal did not say the world was untouched: {text}"
    );
}

/// `check-closing-keywords` refuses an unreadable commit range too.
///
/// The scan is one function now, and the function's own strictness is held —
/// but the *caller* was not. Restoring `assess_autoclose`'s old tolerant copy,
/// or dropping the `?`, left the whole suite green, so the sentence this change
/// put into the installed contract — *"both this scan and
/// `check-closing-keywords` refuse on it rather than continuing with an empty
/// list"* — could be made false with nothing objecting.
///
/// What tolerating it costs is not abstract: the assessment answers
/// `cause: "branch-link"` where the truth is `closing-keyword`, which points the
/// operator at the one cause no edit can undo.
#[test]
fn the_closing_keyword_check_refuses_a_range_it_cannot_read() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-unreadable-range";

    // A branch that exists, and a base whose remote-tracking ref does not, so
    // `origin/<base>..<branch>` is a range git cannot resolve.
    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(git(&["checkout", "-q", "-b", branch]));

    let answers = serde_json::to_string(&serde_json::json!([
        { "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 },
        {
            "matches": "api graphql",
            "stdout": serde_json::json!({
                "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                    "nodes": [{
                        "number": 7,
                        "state": "OPEN",
                        "headRefName": "fix/12-unreadable-range",
                        "baseRefName": "no-such-base",
                    }],
                    "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                } } } },
            }).to_string(),
            "status": 0,
        },
        // No keyword on the remote side, so the commit range is the only source
        // left and reading it is the only thing that can decide.
        {
            "matches": "json body",
            "stdout": "{\"body\":\"a body that names nothing\"}",
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id.to_owned());
    run.issue = Some(12);
    run.state = Some("in-progress".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "check_closing_keywords",
            "arguments": { "issue": 12, "branch": branch, "base": "no-such-base" }
        }
    })
    .to_string();

    let log = trace.path().join("calls.log");
    let count = trace.path().join("count.json");
    let mut child = tracker_command(home, repo, bin, &answers)
        .arg("mcp")
        .env("ESTIGIA_FAKE_COUNT", &count)
        .env("ESTIGIA_FAKE_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the MCP server runs");
    use std::io::Write;
    writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
        .expect("the request is written");
    let output = child.wait_with_output().expect("the MCP server exits");
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "the MCP response is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    assert_eq!(
        response["result"]["isError"], true,
        "an unreadable commit range was answered as an assessment: {text}"
    );
    assert!(
        !text.contains("branch-link"),
        "an unreadable range was reported as the cause no edit can undo: {text}"
    );
}

/// A closed issue still refuses a write inside the checkout it claimed.
///
/// This is the half of issue 2 that says what must **not** change, and nothing
/// held it: no fixture in this crate had ever put an issue in the closed state,
/// before or after the change. A judge measured what that costs by relaxing the
/// classification to `writes_outside_the_claim(..) || run.state == Some("done")`
/// — every routine repository write standing aside once the run carries the
/// state a delivery leaves it in, which is exactly what the issue puts out of
/// scope — and the whole suite stayed green.
///
/// So the tracker really answers `CLOSED` here, and the write really is inside
/// the claimed checkout.
#[test]
fn a_closed_issue_still_refuses_a_write_inside_the_checkout() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    // Derived the way the hook derives it. Stored under a guessed name the
    // pointer is one nothing loads, the gate stands aside for having sworn
    // nothing, and an empty answer looks exactly like the refusal working.
    let session = "closed-issue-fixture";
    let run_id = estigia::harness::session::run_id("claude", session);

    let claim = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude \
         horizon=2099-01-01T00:00Z op-id={} -->",
        "a".repeat(32)
    );
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                // The state a delivery leaves behind, which is when the refusal
                // this test is about actually fires.
                "state": "CLOSED",
                "labels": [{"name": "status:done"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!("Claimed.\n\n{claim}\n"),
                }],
            }).to_string(),
            "status": 0,
        },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    let runs = home.join(".estigia").join("runs");
    let mut run = estigia::harness::session::Run::new(run_id);
    run.issue = Some(12);
    run.state = Some("done".to_owned());
    run.repo_dir = Some(repo.to_path_buf());
    assert!(
        estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
        "the fixture pointer was not stored"
    );

    // Inside the claimed checkout, and a file that does not exist yet — the
    // shape the classification is most likely to get wrong.
    let inside = repo.join("src").join("new.rs");

    let output = run_with_tracker(
        home,
        repo,
        bin,
        &answers,
        &["hook", "pre-tool-use"],
        &serde_json::json!({
            "session_id": session,
            "tool_name": "Write",
            "tool_input": { "file_path": inside.display().to_string() },
        })
        .to_string(),
    );
    let (stdout, _stderr, _ok) = output;
    assert!(
        stdout.contains("CLOSED") || stdout.contains("issue-not-open"),
        "a write inside the claimed checkout was not refused after the issue closed: {stdout}"
    );

    // And the half the issue is actually about, against the same closed tracker:
    // a write outside every covered checkout is answered without asking it. The
    // unit-level fixture for this never reaches the tracker at all — it stops at
    // `control-surface-not-installed` — so "with the issue in that state" was
    // carried only by the test's name until here.
    let (aside, _stderr, _ok) = run_with_tracker(
        home,
        repo,
        bin,
        &answers,
        &["hook", "pre-tool-use"],
        &serde_json::json!({
            "session_id": session,
            "tool_name": "Write",
            "tool_input": { "file_path": home.join("scratchpad").join("note.md").display().to_string() },
        })
        .to_string(),
    );
    // The empty object, not the absence of one word. A hook that stands aside
    // says nothing at all — the aside's code never reaches this surface — so
    // `{}` is the whole answer and anything else is some refusal. Asserting
    // `!contains("CLOSED")` would also be satisfied by
    // `control-surface-not-installed`, and since the stand-aside moved below
    // that refusal, an absence assertion would pass on the wrong one. The rig
    // installs the contract, so reaching `{}` here means the decision was taken
    // past it and without the tracker's answer.
    assert_eq!(
        aside.trim(),
        "{}",
        "a scratch note did not stand aside on the closed issue: {aside}"
    );
}

/// A rewritten branch republishes through the tool, and stops when the claim moves.
///
/// The two halves the unit tests on `push_to_origin` cannot reach, because both
/// are about what surrounds the push rather than about the push: that a run
/// never has to leave Estigia to land an amended branch, and that the renewal
/// standing immediately before the force-push actually refuses.
///
/// The second half is the one that decays quietly. Take that `verify_claim` out
/// and every unit test still passes — the lease is unchanged, the operation
/// still lands the rewritten branch, and the only thing lost is the answer to
/// *is this run still the holder* at the one boundary where the wrong answer
/// destroys history.
#[test]
fn a_republish_lands_a_rewritten_branch_and_stops_when_the_claim_moved() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-republish";

    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    let sha = |what: &str| -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", what])
            .output()
            .expect("git answers");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let remote_head = || -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["rev-parse", &format!("refs/heads/{branch}")])
            .output()
            .expect("the origin is readable");
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    assert!(git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string()
    ]));
    assert!(git(&["branch", "-M", "main"]));
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]));
    assert!(git(&["push", "-q", "origin", "main"]));

    // The first publication, as an ordinary push: this is the head the receipt
    // records and the head the lease will be taken against.
    assert!(git(&["checkout", "-q", "-b", branch]));
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]));
    assert!(git(&["push", "-q", "-u", "origin", branch]));
    let published_head = sha("HEAD");
    let base_sha = sha("origin/main");

    // The rewrite. After this the branch is no longer a descendant of what the
    // remote holds, which is exactly where the ordinary push gives up and a run
    // used to leave Estigia.
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "--amend",
        "-m",
        "the same change, amended",
    ]));
    let rewritten_head = sha("HEAD");
    assert_ne!(rewritten_head, published_head, "the amend rewrote nothing");
    assert_eq!(remote_head(), published_head, "the fixture did not publish");

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    let claimed_by = |who: &str| {
        format!(
            "<!-- issue-flow: claim run-id={who} runtime=claude \
             horizon=2099-01-01T00:00Z op-id={} -->",
            "a".repeat(32)
        )
    };
    // The receipt the lease is taken from: a complete `published` marker naming
    // the head the remote actually holds.
    let receipt = format!(
        "<!-- issue-flow: published run-id={run_id} pr=7 head={published_head} base={base_sha} \
         digest={} epoch={} -->",
        "c".repeat(64),
        "a".repeat(32)
    );
    // `receipted` is what the third case turns off: an issue that has never been
    // published has no head to lease against, and the operation has to say so
    // rather than force over whatever the remote holds.
    let timeline = |holder: &str, receipted: bool| {
        let mut comments = vec![serde_json::json!({
            "id": "IC_1",
            "createdAt": "2026-01-01T00:00Z",
            "viewerDidAuthor": true,
            "includesCreatedEdit": false,
            "body": format!("Claimed.\n\n{}\n", claimed_by(holder)),
        })];
        if receipted {
            comments.push(serde_json::json!({
                "id": "IC_2",
                "createdAt": "2026-01-01T01:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!("Published draft for review.\n\n{receipt}\n"),
            }));
        }
        serde_json::json!({
            "state": "OPEN",
            "labels": [{"name": "status:in-progress"}],
            "comments": comments,
        })
        .to_string()
    };
    let pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": rewritten_head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": true,
    });

    // `held` is how many `gh issue view` calls still answer with this run as the
    // holder; the one after them is denied. Which call that is comes from the
    // landing run's own log, located **by position** — the first timeline read
    // after the last `pr list` is the renewal standing between the draft
    // conversion and the push.
    //
    // Located by position and not by total, and the difference is the whole
    // test. The first version counted every read and denied the last, which is
    // a measurement the code under test can move: with the renewal deleted the
    // total drops by one, `last` names a read inside the earlier verification,
    // that one refuses instead, and the assertion below passes over a fix that
    // is gone. Measured — the deletion was green. Anchored on `pr list`, the
    // denied call is either the renewal or nothing at all, and nothing at all
    // lands the push and fails loudly.
    let answers = |held: Option<u64>, receipted: bool| {
        let mut script = vec![];
        if let Some(held) = held {
            script.push(serde_json::json!({
                "matches": "issue view", "nth": held + 1,
                "stdout": timeline("claude-99999999", receipted), "status": 0,
            }));
        }
        script.extend([
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, receipted), "status": 0 }),
            serde_json::json!({ "matches": "pr list", "stdout": serde_json::json!([pr]).to_string(), "status": 0 }),
            serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
            serde_json::json!({ "matches": "json body", "stdout": serde_json::json!({"body": "names nothing"}).to_string(), "status": 0 }),
            serde_json::json!({ "matches": "repo view", "stdout": "{\"owner\":{\"login\":\"o\"},\"name\":\"r\"}", "status": 0 }),
            serde_json::json!({
                "matches": "api graphql",
                "stdout": serde_json::json!({
                    "data": { "repository": { "issue": { "closedByPullRequestsReferences": {
                        "nodes": [],
                        "pageInfo": { "hasNextPage": false, "endCursor": serde_json::Value::Null },
                    } } } },
                }).to_string(),
                "status": 0,
            }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]);
        serde_json::to_string(&script).expect("the fake tracker script serialises")
    };

    let republish = |answers: &str, log: &std::path::Path| -> (bool, String) {
        let runs = home.join(".estigia").join("runs");
        // Revision-guarded, so the second case needs a clean pointer.
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(12);
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );
        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "republish_review", "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }}
        })
        .to_string();
        let count = trace.path().join(format!(
            "{}.json",
            log.file_stem().unwrap_or_default().to_string_lossy()
        ));
        let mut child = tracker_command(home, repo, bin, answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_COUNT", &count)
            .env("ESTIGIA_FAKE_LOG", log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        (
            response["result"]["isError"] == true,
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        )
    };

    // First half: the whole route, with the claim standing throughout.
    let landing = trace.path().join("landing.log");
    let (failed, text) = republish(&answers(None, true), &landing);
    assert!(!failed, "the republish did not land: {text}");
    assert!(
        text.contains("\"republished\": true") && text.contains(&published_head),
        "the answer does not say which route ran or what it leased against: {text}"
    );
    assert_eq!(
        remote_head(),
        rewritten_head,
        "the rewritten branch did not reach the remote"
    );

    // The renewal, found where it has to stand: after the last pull-request
    // listing — which is after the draft conversion and the body refresh — and
    // therefore between them and the push. Asserted on its own, because it is
    // the property, and because the denial below is only meaningful if there is
    // a call there to deny.
    let calls: Vec<String> = std::fs::read_to_string(&landing)
        .expect("the call log")
        .lines()
        .map(str::to_owned)
        .collect();
    let listed = calls
        .iter()
        .rposition(|line| line.contains("pr list"))
        .expect("the republish listed the open pull requests");
    assert!(
        calls[listed + 1..]
            .iter()
            .any(|line| line.contains("issue view")),
        "no timeline read stands between the draft conversion and the force-push: {calls:#?}"
    );
    let held = calls[..=listed]
        .iter()
        .filter(|line| line.contains("issue view"))
        .count() as u64;

    // Second half: everything identical, except that the renewal before the push
    // says somebody else holds it. Put the remote back first, so what this
    // measures is this run's push and not the one above.
    assert!(git(&[
        "push",
        "-q",
        "--force",
        "origin",
        &format!("{published_head}:{branch}")
    ]));
    assert_eq!(remote_head(), published_head, "the remote was not restored");

    let denied = trace.path().join("denied.log");
    let (failed, text) = republish(&answers(Some(held), true), &denied);
    assert!(
        failed,
        "a republish went through after the claim moved to another run: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the force-push landed although the claim had moved: {text}"
    );
    // And it says which world it refused in. By this point `edit_pr` has replaced
    // the live pull request's title and body — the call log below proves it — so
    // `nothing was written` is false, and a run believing it leaves somebody
    // else's pull request re-described with nobody told to put it back. The
    // refusal reached here through `stop()`, whose envelope carries no `world`,
    // and the harness read the absence as *untouched*.
    assert!(
        std::fs::read_to_string(&denied)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr edit")),
        "the fixture never rewrote the pull request, so this proves nothing about the report"
    );
    assert!(
        !text.contains("nothing was written"),
        "the refusal reported an untouched world after rewriting the pull request: {text}"
    );
    assert!(
        text.contains("put it back"),
        "the refusal does not say what was left behind: {text}"
    );
    // And the renewal's **own** action survives beside it. The writes are
    // appended, never substituted: the one `Stop` that reaches this wrapper is
    // the renewal, and two of its five actions carry the only instruction the
    // run has — *claim it again* for a lapsed horizon, and acknowledge-and-drop
    // for a stand-down. The sibling wrapper destroyed its caller's action one
    // commit ago and turned the CI-exposure warning into an invitation; this is
    // the same shape, and it is held rather than assumed.
    assert!(
        text.contains("release only your own projection"),
        "the refusal replaced the renewal's own action instead of adding to it: {text}"
    );

    // Third: the refusal this operation exists to produce, driven the whole way
    // rather than at the level of `push_to_origin`. Somebody else moves the
    // remote after the receipt was recorded, so the lease refuses — and what is
    // asserted is what the *agent* is told, which no unit test can see. It was
    // the rarer path (the claim moving) that got a sentence naming the rewritten
    // pull request, while the designed outcome came back saying nothing about it.
    let other = trace.path().join("other");
    assert!(
        Command::new("git")
            .args(["clone", "--quiet", "--"])
            .arg(origin.path())
            .arg(&other)
            .output()
            .is_ok_and(|output| output.status.success())
    );
    let elsewhere = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(&other)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(elsewhere(&["checkout", "-q", branch]));
    std::fs::write(other.join("theirs.txt"), "theirs\n").expect("their change");
    assert!(elsewhere(&["add", "theirs.txt"]));
    assert!(elsewhere(&[
        "-c",
        "user.email=other@example.invalid",
        "-c",
        "user.name=Somebody else",
        "commit",
        "--quiet",
        "-m",
        "a commit this run never saw",
    ]));
    assert!(elsewhere(&["push", "-q", "origin", branch]));
    let theirs = String::from_utf8_lossy(
        &Command::new("git")
            .arg("-C")
            .arg(&other)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git answers")
            .stdout,
    )
    .trim()
    .to_owned();
    assert_eq!(remote_head(), theirs, "the other run did not publish");

    let leased = trace.path().join("leased.log");
    let (failed, text) = republish(&answers(None, true), &leased);
    assert!(
        failed,
        "the lease let a republish over a commit the receipt never named: {text}"
    );
    assert_eq!(
        remote_head(),
        theirs,
        "the force-push destroyed a commit somebody else pushed: {text}"
    );
    assert!(
        !text.contains("nothing was written"),
        "the lease refusal reported an untouched world after rewriting the pull request: {text}"
    );
    // **Exactly** what happened, and nothing else. This fixture's pull request
    // is already draft — which is the normal state of a review target, since
    // `publish_review` drafts it and only `release_ci` makes it ready — so
    // `ensure_draft` writes nothing and only the edit did.
    //
    // The assertion used to be `contains("converted to draft")`, and it passed:
    // the refusal named a conversion nobody performed, and the test pinned the
    // falsehood so that fixing it would have gone red. An operator putting back
    // a draft conversion runs `gh pr ready`, which exposes the branch to CI —
    // the exact outcome `ensure_draft` exists to prevent.
    assert!(
        text.contains("had its title and body replaced"),
        "the lease refusal does not name the edit that did happen: {text}"
    );
    assert!(
        !text.contains("converted back to draft"),
        "the lease refusal claims a draft conversion over a pull request that was already draft: \
         {text}"
    );

    // Put the remote back for the last cases.
    assert!(git(&[
        "push",
        "-q",
        "--force",
        "origin",
        &format!("{published_head}:{branch}")
    ]));

    // Fourth: the earliest refusal that can follow a write, and the one whose
    // report was left behind when the renewal's was fixed. The reused pull
    // request is READY, so `ensure_draft` un-readies it — and then its own
    // readback fails. A `gh pr view` that hits a rate limit is enough. The
    // refusal reached the agent as `Failure::Read`, which is exit 3 and reads
    // *the tracker could not be read; write nothing* — after a write.
    let ready_pr = serde_json::json!({
        "number": 7,
        "url": "https://github.com/o/r/pull/7",
        "headRefOid": rewritten_head,
        "baseRefOid": base_sha,
        "state": "OPEN",
        "isDraft": false,
    });
    let unreadable = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // The readback `ensure_draft` makes, and nothing after it: this is the
        // first `pr view` of the operation, so failing it stops exactly there.
        { "matches": "headRefOid", "stdout": "", "status": 1 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let undone = trace.path().join("undone.log");
    let (failed, text) = republish(&unreadable, &undone);
    assert!(
        failed,
        "an unreadable draft readback was not a refusal: {text}"
    );
    assert!(
        std::fs::read_to_string(&undone)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr ready") && line.contains("--undo")),
        "the fixture never un-readied the pull request, so this proves nothing about the report"
    );
    assert!(
        !text.contains("nothing was written"),
        "the draft readback reported an untouched world after un-readying the PR: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );

    // Fourth-and-a-quarter: `--undo` returns zero and the pull request comes back
    // **still ready**. That is `draft-readback-failed`, and it is two things at
    // once — a report and a gate — neither of which anything measured.
    //
    // The gate: with its condition disabled the operation pushes the rewritten
    // head at a pull request CI is watching, which is the whole reason
    // `ensure_draft` runs before the push.
    //
    // The report: this refusal carries the only action that names that hazard.
    // A round of this change routed it through the shared wording, whose insert
    // **replaces** the action — so the warning was destroyed and what took its
    // place said the pull request *was converted back to draft* beside an
    // `observed` showing it was not, and invited an operator to put that back.
    // Putting it back is `gh pr ready`. The refusal would have talked somebody
    // into the exposure it exists to prevent.
    let still_ready = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // `--undo` is unmatched, so it "succeeds"; the readback disagrees.
        { "matches": "headRefOid", "stdout": ready_pr.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let stuck = trace.path().join("still-ready.log");
    let (failed, text) = republish(&still_ready, &stuck);
    assert!(
        failed,
        "a pull request that stayed ready was not refused: {text}"
    );
    assert!(
        std::fs::read_to_string(&stuck)
            .expect("the call log")
            .lines()
            .any(|line| line.contains("pr ready") && line.contains("--undo")),
        "the fixture never attempted the un-ready, so this proves nothing"
    );
    assert!(
        text.contains("do not push") && text.contains("still ready"),
        "the refusal lost the action naming the CI exposure it exists to prevent: {text}"
    );
    for lie in ["converted back to draft", "put it back"] {
        assert!(
            !text.contains(lie),
            "the refusal said {lie:?} about a pull request it read back as still ready: {text}"
        );
    }
    assert!(
        !text.contains("nothing was written"),
        "the refusal reported an untouched world after `--undo` ran: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "a still-ready pull request did not stop the push: {text}"
    );

    // Fourth-and-a-half: the same window one call later. `ensure_draft` succeeds
    // — the pull request was ready and is now draft — and then `gh pr edit`
    // fails. That `?` was bare, so a transient failure there reported an
    // untouched world one statement after `un_readied` had been written to stop
    // exactly that. Removing the carrier left the whole suite green; this is
    // what measures it.
    let edit_fails = serde_json::to_string(&serde_json::json!([
        { "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 },
        { "matches": "pr list", "stdout": serde_json::json!([ready_pr]).to_string(), "status": 0 },
        // The draft readback succeeds, so `ensure_draft` returns having written.
        { "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 },
        { "matches": "pr edit", "stdout": "", "status": 1 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");
    let edited = trace.path().join("edit-failed.log");
    let (failed, text) = republish(&edit_fails, &edited);
    assert!(failed, "a failing pr edit was not a refusal: {text}");
    let calls = std::fs::read_to_string(&edited).expect("the call log");
    assert!(
        calls.contains("pr ready") && calls.contains("--undo") && calls.contains("pr edit"),
        "the fixture did not un-ready and then attempt the edit: {calls}"
    );
    assert!(
        !text.contains("nothing was written"),
        "the failed edit reported an untouched world after un-readying the PR: {text}"
    );
    assert!(
        text.contains("the pull request was converted back to draft"),
        "the failed edit does not name the un-ready that did happen: {text}"
    );
    assert!(
        !text.contains("had its title"),
        "the failed edit claims an edit that did not land: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );

    // Fifth: an issue that has never been published has no head to lease
    // against. Forcing anyway would be the plain `--force` the issue rules out,
    // wearing a lease over whatever the remote happens to hold — so the refusal
    // is the guard, and until this case existed deleting it left the whole suite
    // green.
    let unpublished = trace.path().join("unpublished.log");
    let (failed, text) = republish(&answers(None, false), &unpublished);
    assert!(
        failed,
        "a republish went through with no recorded publication to lease against: {text}"
    );
    assert!(
        text.contains("published-receipt-missing"),
        "the missing receipt was not what refused it: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved although there was no receipt to lease against: {text}"
    );

    // Sixth, and the other direction of the same honesty: a refusal must not
    // claim a write that did not happen. There is a receipt, but **no open pull
    // request** — a previous one was closed — so `reused` is `None` and nothing
    // has been written to any pull request when the renewal denies the claim.
    //
    // Held because the flag was untested this way round. `rewrote_pr = true` as
    // a constant, and `= reused.is_some()`, both left the whole suite green, and
    // either would tell an operator that a pull request was converted to draft
    // and re-described when none was touched — sending them to undo a change
    // that does not exist. That is the mirror of the falsehood this operation
    // was fixed to stop telling, and it deserved the same measurement.
    let fresh_pr = |held: u64| {
        serde_json::to_string(&serde_json::json!([
            serde_json::json!({
                "matches": "issue view", "nth": held + 1,
                "stdout": timeline("claude-99999999", true), "status": 0,
            }),
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 }),
            // No open pull request: the reused path is never entered.
            serde_json::json!({ "matches": "pr list", "stdout": "[]", "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]))
        .expect("the fake tracker script serialises")
    };
    let untouched = trace.path().join("untouched.log");
    let (failed, text) = republish(&fresh_pr(held), &untouched);
    assert!(
        failed,
        "a republish went through after the claim moved to another run: {text}"
    );
    assert!(
        std::fs::read_to_string(&untouched)
            .expect("the call log")
            .lines()
            .all(|line| !line.contains("pr edit") && !line.contains("pr ready")),
        "the fixture rewrote a pull request, so this proves nothing about the untouched case"
    );
    // Every clause the report can emit, and the frame that carries them. Named
    // one by one rather than by a single phrase, because a single phrase is
    // exactly how this assertion died: the report's wording changed to
    // *"converted **back** to draft"*, the sibling case three was updated to
    // match and this one was not, so it forbade a substring the code could no
    // longer produce. A dead assertion is worse than none — measured, a build
    // that claimed both writes on every path, including this one where no pull
    // request exists at all, passed the entire suite.
    for lie in [
        "converted back to draft",
        "had its title",
        "put it back",
        "changed nothing",
    ] {
        assert!(
            !text.contains(lie),
            "the refusal said {lie:?} when no pull request was touched: {text}"
        );
    }
    // And the channel underneath the words. `world: committed` is what makes the
    // harness report a landed write, so a refusal that reaches here must not
    // carry it however the sentence is phrased.
    assert!(
        text.contains("nothing was written"),
        "the refusal did not report the untouched world it is actually in: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal with no pull request in play: {text}"
    );

    // Seventh: the pull request **was** ready, so `ensure_draft` un-readies it
    // and the edit follows. Now both writes happened, and the refusal has to
    // name both — which is the only case where the sentence the previous round
    // shipped was accurate. Holding the two cases together is the point: one
    // fixture proves the draft clause appears when it should, the other proves
    // it stays away when it should not, and a single boolean cannot satisfy
    // both.
    let was_ready = |held: u64| {
        let ready = serde_json::json!({
            "number": 7, "url": "https://github.com/o/r/pull/7",
            "headRefOid": rewritten_head, "baseRefOid": base_sha,
            "state": "OPEN", "isDraft": false,
        });
        serde_json::to_string(&serde_json::json!([
            serde_json::json!({
                "matches": "issue view", "nth": held + 1,
                "stdout": timeline("claude-99999999", true), "status": 0,
            }),
            serde_json::json!({ "matches": "issue view", "stdout": timeline(run_id, true), "status": 0 }),
            serde_json::json!({ "matches": "pr list", "stdout": serde_json::json!([ready]).to_string(), "status": 0 }),
            // The readback `ensure_draft` makes after `--undo`: now a draft.
            serde_json::json!({ "matches": "headRefOid", "stdout": pr.to_string(), "status": 0 }),
            serde_json::json!({ "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 }),
        ]))
        .expect("the fake tracker script serialises")
    };
    let readied = trace.path().join("readied.log");
    let (failed, text) = republish(&was_ready(held), &readied);
    assert!(
        failed,
        "the claim moved and the republish went through: {text}"
    );
    let calls = std::fs::read_to_string(&readied).expect("the call log");
    assert!(
        calls.contains("pr ready") && calls.contains("--undo") && calls.contains("pr edit"),
        "the fixture did not un-ready and edit, so this proves nothing about naming both: {calls}"
    );
    // The **joined** sentence, not two fragments of it. Two fragments are how
    // the ungrammatical frame survived: *"the pull request was had its title and
    // body replaced"* contains both of them. The unit test beside `describe()`
    // covers every combination; this is the one that proves the frame the MCP
    // server actually hands an agent is the assembled one.
    assert!(
        text.contains(
            "the pull request was converted back to draft and had its title and body replaced \
             before this refusal"
        ),
        "the refusal does not read as one sentence naming both writes: {text}"
    );
    assert_eq!(
        remote_head(),
        published_head,
        "the remote moved on a refusal before the push: {text}"
    );
}

/// The ordinary publication adjudicates its claim, proved by what reaches the remote.
///
/// The test whose **absence** was the finding, and it is about `publish_review`
/// rather than about anything this change added. Nothing anywhere asserted that
/// the ordinary publication refuses when the claim has moved: the only thing
/// holding that property was `honesty.rs` counting the string `verify_claim(` in
/// a function body, and a count cannot see a conditional.
///
/// Measured, five ways, each of which left the whole suite green at some point
/// in this change's history and is red now:
///
/// - the entry verification deleted;
/// - its result discarded with `let _ = …`, which keeps the token a count reads;
/// - the call moved behind `if matches!(push, Push::Leased { .. })`;
/// - the call moved behind `if matches!(push, Push::FastForward)` — the *other*
///   value of the same discriminant, which the first four of these tests missed
///   because they drove only the route that value keeps;
/// - the call moved below the draft conversion, so it still precedes the push.
///
/// The last two are why this runs both entry points and why the fixture has an
/// open pull request. With an empty `pr list` the assertions that no `pr edit`
/// or `pr ready` ran are **vacuous** — `ensure_draft` and `edit_pr` live behind
/// `reused`, so they could not run whatever the mutation. A verification moved
/// below them still stops the push, so the remote alone cannot see it.
///
/// The remote is the assertion that a mutated build cannot fake; the pull
/// request is the one that catches a refusal arriving too late. The message
/// assertion fires first of the three, and is there to prove the refusal is
/// about the claim rather than about something incidental.
#[test]
fn a_publication_refuses_at_entry_when_the_claim_has_moved() {
    let rig = tracker_rig();
    let (home, repo, bin) = (rig.home.path(), rig.repo.path(), rig.bin.path());
    let trace = tempfile::tempdir().expect("a trace directory");
    let origin = tempfile::tempdir().expect("a bare origin");
    let run_id = "claude-abcd1234";
    let branch = "fix/12-entry";

    let git = |arguments: &[&str]| -> bool {
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .output()
            .is_ok_and(|output| output.status.success())
    };
    assert!(
        Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(origin.path())
            .output()
            .is_ok_and(|output| output.status.success())
    );
    assert!(git(&[
        "remote",
        "add",
        "origin",
        &origin.path().display().to_string()
    ]));
    assert!(git(&["branch", "-M", "main"]));
    std::fs::write(repo.join("kept.txt"), "base\n").expect("the base file");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "base content",
    ]));
    assert!(git(&["push", "-q", "origin", "main"]));

    // The branch is committed and **not** pushed. That is what makes the origin
    // able to answer the question: if the entry verification is gone, the
    // publication runs to completion and this ref appears there.
    assert!(git(&["checkout", "-q", "-b", branch]));
    std::fs::write(repo.join("kept.txt"), "changed\n").expect("the change");
    assert!(git(&["add", "kept.txt"]));
    assert!(git(&[
        "-c",
        "user.email=nobody@example.invalid",
        "-c",
        "user.name=nobody",
        "commit",
        "--quiet",
        "-m",
        "a change that names no issue",
    ]));

    let body = trace.path().join("pr-body.md");
    std::fs::write(&body, "A body that names nothing.\n").expect("the body");

    // Somebody else holds it, from the very first read — and the timeline also
    // carries a complete receipt, so `republish_review` gets past its own
    // receipt check and reaches the same entry verification. Without it that
    // route would refuse for an unrelated reason and prove nothing.
    let published_marker = format!(
        "<!-- issue-flow: published run-id=claude-99999999 pr=7 head={} base={} digest={} \
         epoch={} -->",
        "d".repeat(40),
        "b".repeat(40),
        "c".repeat(64),
        "a".repeat(32)
    );
    // **One open draft pull request**, not an empty list. An empty list made
    // `reused` `None`, so `edit_pr` was structurally unreachable and the
    // assertion below that no `pr edit` ran could not fail. With a pull request
    // present it does, and it is what catches an entry verification moved
    // *below* the draft conversion — the push assertion alone cannot see that,
    // because the renewal further down still stops the push.
    //
    // Exactly one of the three assertions bites, and saying "they" would be the
    // same overclaim this round is correcting. `pr create` cannot fire with
    // `reused` set, and `pr ready` cannot fire because this pull request is
    // already draft. That last one leaves a real gap: a verification placed
    // *between* `ensure_draft` and `edit_pr` would, against a **ready** pull
    // request, un-ready somebody else's before refusing, and this fixture
    // cannot see it. Named rather than fixed, because a second fixture for a
    // window one statement wide is more machinery than the risk earns.
    let open_pr = serde_json::json!({
        "number": 7, "url": "https://github.com/o/r/pull/7",
        "headRefOid": "e".repeat(40), "baseRefOid": "b".repeat(40),
        "state": "OPEN", "isDraft": true,
    });
    let answers = serde_json::to_string(&serde_json::json!([
        {
            "matches": "issue view",
            "stdout": serde_json::json!({
                "state": "OPEN",
                "labels": [{"name": "status:in-progress"}],
                "comments": [{
                    "id": "IC_1",
                    "createdAt": "2026-01-01T00:00Z",
                    "viewerDidAuthor": true,
                    "includesCreatedEdit": false,
                    "body": format!(
                        "Claimed.\n\n<!-- issue-flow: claim run-id=claude-99999999 runtime=claude \
                         horizon=2099-01-01T00:00Z op-id={} -->\n{published_marker}\n",
                        "a".repeat(32)
                    ),
                }],
            }).to_string(),
            "status": 0,
        },
        { "matches": "pr list", "stdout": serde_json::json!([open_pr]).to_string(), "status": 0 },
        { "matches": "headRefOid", "stdout": open_pr.to_string(), "status": 0 },
        { "matches": "api user", "stdout": "{\"login\":\"fixture\"}", "status": 0 },
    ]))
    .expect("the fake tracker script serialises");

    // **Both** entry points. The first version drove `publish_review` only, and
    // the entry verification stayed removable from `republish_review` with the
    // whole suite green — the same defect one route further along, which is how
    // three of these rounds have gone. They share a body today; nothing says
    // they always will, and a test that covers one of two routes is a test that
    // stops holding the moment somebody splits them.
    for tool in ["publish_review", "republish_review"] {
        let runs = home.join(".estigia").join("runs");
        // Revision-guarded: the second pass needs a clean pointer.
        let _ = std::fs::remove_file(runs.join(format!("{run_id}.json")));
        let mut run = estigia::harness::session::Run::new(run_id.to_owned());
        run.issue = Some(12);
        run.state = Some("in-progress".to_owned());
        run.repo_dir = Some(repo.to_path_buf());
        assert!(
            estigia::harness::session::store(&runs, &run).expect("the pointer is writable"),
            "the fixture pointer was not stored"
        );

        let request = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": {
                "issue": 12,
                "run_id": run_id,
                "branch": branch,
                "base": "main",
                "pr_title": "Something",
                "pr_body_file": body.display().to_string(),
                "worktree": repo.display().to_string(),
            }}
        })
        .to_string();

        let log = trace.path().join(format!("entry-{tool}.log"));
        let mut child = tracker_command(home, repo, bin, &answers)
            .arg("mcp")
            .env("ESTIGIA_FAKE_LOG", &log)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the MCP server runs");
        use std::io::Write;
        writeln!(child.stdin.take().expect("stdin is piped"), "{request}")
            .expect("the request is written");
        let output = child.wait_with_output().expect("the MCP server exits");
        let response: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
                panic!(
                    "the MCP response is not JSON: {}",
                    String::from_utf8_lossy(&output.stdout)
                )
            });
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned();

        assert_eq!(
            response["result"]["isError"], true,
            "{tool} went through under a claim another run holds: {response}"
        );
        assert!(
            text.contains("not-current-live-holder"),
            "something other than the claim refused {tool}, so this proves nothing about \
             adjudication: {text}"
        );

        // The remote, which a mutated build cannot fake.
        let refs = Command::new("git")
            .arg("-C")
            .arg(origin.path())
            .args(["for-each-ref", "--format=%(refname)"])
            .output()
            .expect("the origin is readable");
        let refs = String::from_utf8_lossy(&refs.stdout).into_owned();
        assert!(
            !refs.contains(branch),
            "{tool} put the branch on the remote under a claim this run does not hold: {refs}"
        );
        // And the pull request, which the push assertion cannot see. A run whose
        // entry verification sits *below* the draft conversion still stops before
        // the push — and has already un-readied and re-described somebody else's
        // pull request by then.
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        for wrote in ["pr create", "pr edit", "pr ready"] {
            assert!(
                !calls.contains(wrote),
                "`{wrote}` ran under a claim this run does not hold, via {tool}: {calls}"
            );
        }
    }
}

/// A per-call working directory decides which holder answers for the call.
///
/// OpenCode's plugin launches the gate from the **project** directory, because
/// its plugin context carries a project and no session identity. When two runs
/// each hold an isolated worktree inside one base checkout, that directory is
/// the base — which both runs cover at equal depth, so `holders_of` returned
/// both and the call was refused `several-runs-hold-this-checkout`.
///
/// The refusal was correct about the directory it was given. What the call
/// actually carries is `args.workdir`, the directory the command will run in,
/// and nothing read it. Measured on 2026-08-16 with two live holders of this
/// repository: a `git commit` explicitly targeting one worktree was refused for
/// ambiguity between two runs that were not ambiguous at all, and the refusal
/// advised releasing one of them — which is the isolation both were using.
///
/// Driven as a process against the real binary, because the defect is in what
/// reaches `gate_context` from the outside. A unit test on `payload_cwd` cannot
/// see the base-checkout fallback that makes the wrong answer look right.
#[test]
fn a_per_call_working_directory_selects_the_holder_that_owns_it() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    // One base checkout, two isolated worktrees inside it. Inside, because that
    // is the arrangement `holders_of` documents as the one that goes ambiguous:
    // work in A's worktree is covered exactly by A and from the base by B.
    let base = tempfile::tempdir().expect("a checkout");
    let worktree_a = base.path().join("wt-a");
    let worktree_b = base.path().join("wt-b");
    std::fs::create_dir_all(&worktree_a).expect("worktree a");
    std::fs::create_dir_all(&worktree_b).expect("worktree b");

    let runs = home.path().join(".estigia").join("runs");
    std::fs::create_dir_all(&runs).expect("the runs directory");
    for (run_id, issue, worktree) in [
        ("claude-aaaa1111", 12u64, &worktree_a),
        ("opencode-bbbb2222", 34u64, &worktree_b),
    ] {
        let pointer = serde_json::json!({
            "run_id": run_id,
            "issue": issue,
            "state": "in-progress",
            "repo_dir": base.path(),
            "worktree": worktree,
            "verified_at": serde_json::Value::Null,
        });
        std::fs::write(
            runs.join(format!("{run_id}.json")),
            serde_json::to_string(&pointer).expect("a pointer serialises"),
        )
        .expect("the pointer writes");
    }

    let ledger = home.path().join(".estigia").join("decisions.jsonl");
    // Both commands the report reproduced it with. The second matters on its
    // own: it is Estigia's own binary being refused, which is what showed the
    // false ambiguity lands before any delivery or Git-head validation.
    let ask_running = |command: &str, workdir: Option<&str>| {
        let _ = std::fs::remove_file(&ledger);
        let payload = match workdir {
            Some(named) => serde_json::json!({ "command": command, "workdir": named }),
            None => serde_json::json!({ "command": command }),
        };
        let payload = serde_json::to_string(&payload).expect("a payload serialises");
        let (out, error, ok) = run_in(
            home.path(),
            base.path(),
            &["gate", "bash", "--input", &payload],
            "",
        );
        let wrote = std::fs::read_to_string(&ledger).unwrap_or_default();
        (format!("{out}{error}"), ok, wrote)
    };

    for command in [
        "git commit -m fixture",
        "estigia config set Planning direct",
    ] {
        let ask = |workdir: Option<&str>| ask_running(command, workdir);

        // The floor. A call whose working directory really is the shared base is
        // genuinely ambiguous, and that refusal is not what this change touches:
        // if it stops firing, the rows below prove nothing. It is also the
        // project-context fallback, which a call carrying no per-call working
        // directory has to keep.
        let (said, ok, _) = ask(None);
        assert!(
            !ok && said.contains("several-runs-hold-this-checkout"),
            "the genuine ambiguity at the base checkout stopped being refused \
             for `{command}`, so nothing below is a measurement: {said}"
        );

        // Each worktree in turn, by absolute path.
        for (named, mine, theirs) in [
            (&worktree_a, "claude-aaaa1111", "opencode-bbbb2222"),
            (&worktree_b, "opencode-bbbb2222", "claude-aaaa1111"),
        ] {
            let (said, _, wrote) = ask(Some(&named.display().to_string()));
            assert!(
                !said.contains("several-runs-hold-this-checkout"),
                "`{command}` naming {} was refused as ambiguous between two runs \
                 that hold different worktrees: {said}",
                named.display()
            );
            // Not merely "some holder answered" — the right one. The ledger is
            // where the attribution is durable, and an answer that named the
            // other run would be the same defect wearing the opposite result.
            assert!(
                wrote.contains(mine),
                "the decision was not attributed to the run that owns {}: {wrote}",
                named.display()
            );
            assert!(
                !wrote.contains(theirs),
                "the decision named the run that does not own {}: {wrote}",
                named.display()
            );
        }

        // Relative, resolved the way OpenCode resolves it: against the directory
        // the call runs in, which is the one the plugin launched this process
        // in.
        let (said, _, wrote) = ask(Some("wt-a"));
        assert!(
            !said.contains("several-runs-hold-this-checkout"),
            "a relative working directory was not resolved for `{command}`, so \
             it selected nobody: {said}"
        );
        assert!(
            wrote.contains("claude-aaaa1111") && !wrote.contains("opencode-bbbb2222"),
            "a relative working directory reached the wrong holder: {wrote}"
        );

        // A directory that is not there. It resolves under the launch directory
        // like any other relative value, so the ordinary ambiguity refusal is
        // what it gets — the answer the same call gets carrying no key at all.
        //
        // The clamp is what holds this now, not the resolution: an unresolvable
        // value is placed before it is compared, and one that lands outside is
        // dropped in favour of the launch directory. Both belts are kept, and
        // the resolution is measured by the unit test rather than by this row.
        let (said, ok, _) = ask(Some("no-such-worktree"));
        assert!(
            !ok,
            "a working directory that names nothing was allowed through: {said}"
        );
        assert!(
            said.contains("several-runs-hold-this-checkout"),
            "a working directory that names nothing escaped the checkout it was \
             resolved against, so no run held the call: {said}"
        );

        // The rows that decide whether reading this key at all is safe.
        //
        // `cwd` is set by the host. `workdir` is a **tool argument**, which on
        // every runtime here means it is written by the model, and a value that
        // names a real directory outside the checkout is not a value that names
        // nothing: it resolves, it is covered by no run, and the gate answers
        // `outside` and exits zero. The command still runs wherever it was
        // going to run. That is a payload steering a write out of the gate,
        // which is strictly worse than the false ambiguity being fixed here and
        // is the failure this crate exists to refuse — a widened gate that looks
        // exactly like working correctly.
        //
        // Every spelling below reaches the same place by a different road, and
        // each one was `several-runs-hold-this-checkout` before this alias was
        // read at all. Nothing may be allowed through.
        let parent = base
            .path()
            .parent()
            .expect("the fixture has somewhere above it")
            .display()
            .to_string();
        let climbed = worktree_a.join("..").join("..").display().to_string();
        // And the road that only opens when the path cannot be **opened**.
        // Comparison resolves both sides and falls back to the spelling when
        // resolution fails, so `..` past an existing component was never
        // cancelled: `wt-a/../../nope` still started with the launch directory,
        // was called inside, and was attributed to the holder of the component
        // it climbed *through*. Measured: not merely let past — allowed, exit
        // zero, under a claim the call had nothing to do with. A run holding one
        // worktree could borrow the other's authority by writing one `..`.
        let through_a = worktree_a.join("..").join("..").join("nope");
        let through_a = through_a.display().to_string();
        let deeper = worktree_a
            .join("..")
            .join("..")
            .join("..")
            .join("nope")
            .display()
            .to_string();
        for escape in [
            "..",
            parent.as_str(),
            climbed.as_str(),
            if cfg!(windows) { "C:\\Windows" } else { "/etc" },
            "wt-a/../../nope",
            through_a.as_str(),
            deeper.as_str(),
        ] {
            let (said, ok, _) = ask(Some(escape));
            assert!(
                !ok,
                "`{command}` steered the gate out of the claim with \
                 `workdir` = {escape:?}, and the write went through unjudged: {said}"
            );
            assert!(
                said.contains("several-runs-hold-this-checkout"),
                "`{command}` with `workdir` = {escape:?} was answered something \
                 other than the refusal the same call gets without the key, so \
                 the payload moved the decision: {said}"
            );
        }
    }

    // The same road, through a tool that carries no working directory of its
    // own. The key was once read for every gated tool rather than only for the
    // one whose host sends it, so a payload that merely included it steered any
    // of them — measured, on `write` and `edit`, before the read was narrowed to
    // Bash. Two things hold it shut now and this row is the floor under both:
    // the tool restriction, and the clamp behind it. Kept out of the loop above
    // because the argument shape differs.
    let ledger = home.path().join(".estigia").join("decisions.jsonl");
    let _ = std::fs::remove_file(&ledger);
    let payload = serde_json::json!({
        "file_path": worktree_a.join("f.txt"),
        "workdir": "..",
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, ok) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    assert!(
        !ok && said.contains("several-runs-hold-this-checkout"),
        "a write naming a file inside the claim was taken out of the gate by a \
         working directory in its payload: {said}"
    );

    // And the other door of the same branch, which nothing crossed before this
    // change restructured the line it lives on. A host's `cwd` is authoritative
    // and has to keep reaching the decision: discarding it entirely — always
    // adjudicating the process directory — left the whole suite green, while the
    // comment above that line cites a measured defect where the same call was
    // judged against two different repositories depending on which door it came
    // through. Sent as the hook nests it, because that nesting is why the door
    // exists at all.
    let _ = std::fs::remove_file(&ledger);
    let payload = serde_json::json!({
        "tool_input": { "cwd": worktree_b, "file_path": worktree_b.join("f.txt") },
    });
    let payload = serde_json::to_string(&payload).expect("a payload serialises");
    let (out, error, _) = run_in(
        home.path(),
        base.path(),
        &["gate", "write", "--input", &payload],
        "",
    );
    let said = format!("{out}{error}");
    let wrote = std::fs::read_to_string(&ledger).unwrap_or_default();
    assert!(
        !said.contains("several-runs-hold-this-checkout"),
        "a checkout the host named was discarded, so the call fell back to the \
         shared base and was refused as ambiguous: {said}"
    );
    assert!(
        wrote.contains("opencode-bbbb2222") && !wrote.contains("claude-aaaa1111"),
        "the decision was not attributed to the run that owns the checkout the \
         host named: {wrote}"
    );
}
