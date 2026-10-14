use super::*;

#[test]
fn a_reader_that_stops_reading_does_not_produce_a_backtrace() {
    // `status` prints more than one line for any installation, so dropping the
    // pipe after the first read closes it while the process is still writing.
    let mut child = estigia_command()
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
    let output = estigia_command()
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
        let output = estigia_command()
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

/// `--version` prints this build's embedded identity, not only
/// the crate version every build answered with before it.
///
/// The `gate-version` fixture further down used to treat `", dirty)"` as a
/// branch selector on whatever `--version` happened to print — it would have
/// taken the same branch against a reverted `Cli` whose `version` field named
/// bare `CARGO_PKG_VERSION` again, so nothing in this file would have failed
/// if that half of acceptance criterion 1 regressed. It now asks
/// `estigia::build_info::commit()` and `::dirty()` directly instead, so this
/// test's own job is narrower: it compares the printed line against
/// `estigia::build_info::VERSION` directly. That constant always carries a
/// parenthetical build marker, so a `Cli` wired back to `CARGO_PKG_VERSION`
/// alone prints a bare `estigia 0.1.0` here and this assertion is what would
/// say so.
#[test]
fn version_reports_the_build_identity_not_only_the_crate_version() {
    let output = estigia_command()
        .arg("--version")
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`estigia --version` did not exit 0"
    );
    let printed = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        printed.trim(),
        format!("estigia {}", estigia::build_info::VERSION),
        "`--version` did not print this build's embedded identity"
    );
}

/// The one variable in `CLEARED` this file can hold to an actual assertion
/// rather than to a comment: an ambient `GH_REPO`, exactly as an operator's
/// own shell would export it, does not reach a `gh` a gated command spawns.
///
/// `no_variable_the_crate_decides_on_escapes_the_fixture` cannot fail if
/// `run_with_path_locked`'s three-line `env_remove` loop is deleted outright: the
/// `CLEARED` bucket is built from `var("...")`/`var_os("...")` call sites
/// under `src/`, and `GH_REPO`, `GH_HOST` and `GH_TOKEN` are only ever
/// `set_var`'d there, so removing the loop leaves that scan with nothing to
/// see and the suite green. This is the test that would go red instead: it
/// puts `GH_REPO` on *this* process — standing in for the operator's shell,
/// since `Command` inherits an unset-or-removed variable from exactly there —
/// spawns `estigia doctor` through `run_with_path_locked` with a scripted `gh`
/// ahead of the real one on `PATH`, and reads back what that `gh` actually saw
/// through `ESTIGIA_FAKE_ENV_LOG`, a file `examples/fake_process.rs` writes
/// to on request rather than a claim this file makes about it.
///
/// `doctor`'s own `gh` row is the trigger, not reconciliation: `examine`
/// calls `gh auth status` unconditionally once a skill is installed, so no
/// two-holder fixture is needed to make some `gh` run at all — the property
/// under test, that the operator's own `GH_REPO` never reaches any `gh` this
/// binary spawns through this fixture, does not depend on which call does.
///
/// Setting `GH_REPO` on the test binary's own process rather than on a
/// throwaway child is what makes this a fair test of ambient inheritance —
/// `Command` only omits a variable it was told to omit, never one merely
/// absent from a builder — and no concurrently-scheduled test reads `GH_REPO`
/// by name from its own process: every `run`/`run_in` call clears it in the
/// *child* regardless of what this process holds (that clearing is what this
/// test measures), and `tracker_command`'s fixture answers from
/// `ESTIGIA_FAKE_ANSWERS` by argument matching alone, never consulting the
/// repository a real `gh` would have resolved. That leaves the raw table:
/// `run_with_path_locked`'s own `std::env::vars()` iterates every name at once, so
/// `ENV_LOCK`'s write side, held for the whole `set_var`-to-restore span, is
/// what keeps that iteration from running mid-mutation — see the lock's own
/// doc. The variable is restored, and restored before this function's own
/// assertions run, so a failed assertion here cannot leave it set for
/// whatever the suite schedules next.
#[test]
fn an_operators_gh_repo_never_reaches_a_spawned_gh() {
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
    assert!(
        built.is_file(),
        "the process fixture is not built, so this test would measure nothing: run `cargo \
         build --examples` ({})",
        built.display()
    );
    let fake_bin = tempfile::tempdir().expect("a directory for the fake gh");
    let fake_gh = fake_bin
        .path()
        .join(if cfg!(windows) { "gh.exe" } else { "gh" });
    #[cfg(unix)]
    std::os::unix::fs::symlink(&built, &fake_gh).expect("the fake gh is linked onto the path");
    #[cfg(not(unix))]
    std::fs::copy(&built, &fake_gh).expect("the fake gh is copied onto the path");

    let mut joined = std::ffi::OsString::from(fake_bin.path());
    joined.push(if cfg!(windows) { ";" } else { ":" });
    joined.push(std::env::var_os("PATH").unwrap_or_default());

    let home = tempfile::tempdir().expect("a temporary home");
    std::fs::create_dir_all(home.path().join("AppData").join("Roaming")).expect("a roaming dir");
    let (_, stderr, ok) = run(home.path(), &["setup", "claude-code"], "");
    assert!(ok, "setup failed: {stderr}");

    let env_log = tempfile::NamedTempFile::new().expect("a file for the fake `gh` to answer into");

    // The marker this process holds only for the width of the one spawn below
    // — see the module doc for why this is the operator-shell stand-in and
    // why it is safe here.
    // SAFETY: no other test in this binary reads `GH_REPO` from its own
    // process — every `run`/`run_in` call clears it in the child it spawns
    // regardless of this process's value, and `tracker_command`'s fixture
    // answers by argument matching alone — and the value is restored before
    // this function's own assertions run, below. `ENV_LOCK`'s write side,
    // held for this whole block, is what keeps a concurrently-scheduled
    // `run`/`run_in` call's `std::env::vars()` from reading the process
    // environment while `set_var`/`remove_var` here are between one value
    // and the next — see that lock's own doc.
    let _env_guard = ENV_LOCK.write().expect("the env lock is not poisoned");
    let marker = "operators-own-org/operators-own-repo";
    let before = std::env::var_os("GH_REPO");
    unsafe {
        std::env::set_var("GH_REPO", marker);
    }
    // The actual function under test: the same `run_with_path_locked` every
    // other `run`/`run_in` call in this file reaches through `run_with_path`,
    // not a second copy of its clearing logic — a duplicate here would keep
    // passing after somebody deleted the real one, which is the exact failure
    // this test exists to rule out. Called directly, without going through
    // `run_with_path`, because this thread already holds `ENV_LOCK`'s write
    // side above.
    let (_, stderr, ok) = run_with_path_locked(
        home.path(),
        home.path(),
        &["doctor"],
        "",
        Some(std::path::Path::new(&joined)),
        &[(
            "ESTIGIA_FAKE_ENV_LOG",
            &env_log.path().display().to_string(),
        )],
    );
    unsafe {
        match before {
            Some(previous) => std::env::set_var("GH_REPO", previous),
            None => std::env::remove_var("GH_REPO"),
        }
    }
    drop(_env_guard);

    // This asserts the fixture actually did what it was built for, before
    // trusting an empty log to mean the same thing as a clean one — an
    // unbuilt or unreached `gh` would leave `env_log` empty too, and that
    // must not read as "GH_REPO was cleared".
    assert!(ok, "estigia doctor did not run cleanly: {stderr}");
    let seen = std::fs::read_to_string(env_log.path()).unwrap_or_default();
    assert!(
        !seen.is_empty(),
        "the scripted `gh` was never invoked, so this measures nothing — `doctor`'s own `gh` \
         row is expected to call it unconditionally"
    );
    assert!(
        !seen.contains(marker),
        "the scripted `gh` saw the operator's own `GH_REPO`, which `run_with_path_locked`'s clearing \
         exists to prevent: {seen}"
    );
    assert!(
        seen.contains("GH_REPO=(absent)"),
        "the scripted `gh` did not report `GH_REPO` absent at all: {seen}"
    );
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
        (vec!["flush", "--status"], ""),
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
    let git = |arguments: &[&str]| checked_git(repository.path(), arguments);
    let _ = git(&["init", "--quiet"])
        && git(&["config", "user.email", "guard@estigia.test"])
        && git(&["config", "user.name", "estigia"])
        && git(&["commit", "--quiet", "--allow-empty", "-m", "a commit"]);

    #[rustfmt::skip]
    let session_start = ["session", "start", "--session-id", "s", "--runtime", "pi", "--json"];
    #[rustfmt::skip]
    let session_end = ["session", "end", "--session-id", "s", "--runtime", "pi", "--json"];
    let (out, said, ok) = run_in(home.path(), repository.path(), &session_start, "");
    assert!(ok, "`estigia session start --json` failed: {said}");
    let started: serde_json::Value = serde_json::from_str(out.trim())
        .unwrap_or_else(|error| panic!("`estigia session start --json` printed no JSON: {error}"));
    assert!(
        started["context"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "`estigia session start --json` carried no context: {started}"
    );
    let (out, said, ok) = run_in(home.path(), repository.path(), &session_end, "");
    assert!(
        ok && out.trim().is_empty(),
        "`estigia session end --json` printed {out:?} or failed: {said}"
    );

    let ended_session = estigia::harness::session::run_id("pi", "s");
    #[rustfmt::skip]
    let session_send = ["sessions", "send", ended_session.as_str(), "a note", "--json"];
    #[rustfmt::skip]
    let session_deliver = ["session", "deliver", "--session-id", "s", "--runtime", "pi", "--json"];
    for arguments in [
        vec!["status", "--json"],
        vec!["sessions", "--json"],
        session_send.to_vec(),
        session_deliver.to_vec(),
        vec!["update", "--json"],
        vec!["doctor", "--json"],
        vec!["prune", "--json"],
        vec!["changes", "--json"],
        vec!["store", "import", "--json"],
        vec!["roles", "--json"],
        vec!["flush", "--status", "--json"],
        vec!["config", "list", "--json"],
        // The three that were written after this list and never added to it, so
        // all three printed prose under the flag for as long as they existed.
        // `config edit` is the one command left out on purpose: it draws a
        // screen, and there is no prose for `--json` to be an alternative to.
        vec!["config", "set", "Change size", "400", "--json"],
        vec!["config", "repos", "--json"],
        vec!["config", "forget", "--json"],
        vec!["setup", "--all", "--dry-run", "--json"],
        vec!["uninstall", "--all", "--dry-run", "--json"],
        vec!["sync", "--json"],
        vec!["gate", "Write", "--input", "{}", "--json"],
        vec!["release", "--run-id", "claude-nothing0", "--json"],
        vec![
            "goal",
            "--session-id",
            "ses_nothing_to_hold",
            "--runtime",
            "opencode",
            "--json",
        ],
        // The four subcommands the bare verb grew. `complete` reaches a printed
        // answer from a bare pointer (nothing sworn); `progress` and `blocked`
        // reach a refusal for a run that holds nothing, which goes to standard
        // error and leaves standard output empty — allowed, and the same shape
        // `release` already has on this machine; `stop-after` records a request
        // for a run that holds nothing and prints its answer.
        vec!["goal", "complete", "--run-id", "x", "--json"],
        vec!["goal", "progress", "n", "--run-id", "x", "--json"],
        vec!["goal", "blocked", "r", "--run-id", "x", "--json"],
        vec!["goal", "stop-after", "--run-id", "x", "--json"],
        vec!["goal", "start", "x", "--run-id", "x", "--json"],
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
        vec![
            "recover",
            "7",
            "--epoch",
            "x",
            "--authorized-by",
            "nobody",
            "--reason",
            "a fixture",
            "--json",
        ],
        // The relay's three halves, run on a machine holding no run: each reaches
        // a refusal on standard error and prints nothing on standard output.
        vec!["review", "prompt", "--name", "review-risk", "--json"],
        vec!["review", "model", "--name", "review-risk", "--json"],
        vec!["review", "admit", "--name", "review-risk", "--json"],
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

/// `estigia goal stop-after` records a request that outlives the run pointer,
/// `estigia goal` reports it, and `--clear` takes it back.
#[test]
fn goal_stop_after_records_and_clears_a_request_that_outlives_the_pointer() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repository = tempfile::tempdir().expect("a repository");
    let session = "ses_stop_after_fixture";
    let run_id = estigia::harness::session::run_id("opencode", session);

    let (out, err, ok) = run_in(
        home.path(),
        repository.path(),
        &[
            "goal",
            "stop-after",
            "--reason",
            "the operator asked",
            "--session-id",
            session,
            "--runtime",
            "opencode",
            "--json",
        ],
        "",
    );
    assert!(ok, "`goal stop-after` refused: {err}");
    let answer: serde_json::Value = serde_json::from_str(out.trim()).expect("JSON");
    assert_eq!(answer["run_id"], run_id);
    assert_eq!(answer["stop_after"], "the operator asked");
    assert_eq!(answer["cleared"], false);

    let goal = |arguments: &[&str]| {
        let (out, err, ok) = run_in(home.path(), repository.path(), arguments, "");
        assert!(ok, "`estigia goal` refused: {err}");
        serde_json::from_str::<serde_json::Value>(out.trim()).expect("JSON")
    };

    let reading = goal(&[
        "goal",
        "--session-id",
        session,
        "--runtime",
        "opencode",
        "--json",
    ]);
    assert_eq!(
        reading["stop_after"], "the operator asked",
        "the request did not outlive the run pointer: {reading}"
    );

    let cleared = goal(&[
        "goal",
        "stop-after",
        "--clear",
        "--session-id",
        session,
        "--runtime",
        "opencode",
        "--json",
    ]);
    assert_eq!(cleared["stop_after"], serde_json::Value::Null);
    assert_eq!(cleared["cleared"], true);

    let reading = goal(&[
        "goal",
        "--session-id",
        session,
        "--runtime",
        "opencode",
        "--json",
    ]);
    assert_eq!(reading["stop_after"], serde_json::Value::Null);
}

/// `--clear` works on a run whose pointer cannot be read: clearing is Estigia's
/// own local file and never needs the pointer, and the loop clears through this
/// verb, so a refusal here leaves a stale request that stops every later loop.
#[test]
fn goal_stop_after_clears_while_the_pointer_is_unreadable() {
    let home = tempfile::tempdir().expect("a temporary home");
    let repository = tempfile::tempdir().expect("a repository");
    let session = "ses_stop_after_unreadable";
    let run_id = estigia::harness::session::run_id("opencode", session);
    let runs = estigia::harness::session::state_root(Some(home.path())).expect("a state root");
    std::fs::create_dir_all(&runs).expect("the state directory");
    std::fs::write(
        estigia::harness::session::pointer_path(&runs, &run_id),
        "{not json",
    )
    .expect("a pointer that will not parse");
    estigia::harness::session::set_stop_after(&runs, &run_id, Some("a request"))
        .expect("a stop-after request");
    let request = estigia::harness::session::stop_after_path(&runs, &run_id);
    assert!(request.is_file(), "the fixture request was not written");

    let (out, err, ok) = run_in(
        home.path(),
        repository.path(),
        &[
            "goal",
            "stop-after",
            "--clear",
            "--session-id",
            session,
            "--runtime",
            "opencode",
            "--json",
        ],
        "",
    );
    assert!(
        ok,
        "`goal stop-after --clear` was refused on an unreadable pointer: {err}"
    );
    let answer: serde_json::Value = serde_json::from_str(out.trim()).expect("JSON");
    assert_eq!(answer["cleared"], true);
    assert_eq!(answer["stop_after"], serde_json::Value::Null);
    assert!(
        !request.exists(),
        "the request survived a clear on an unreadable pointer"
    );

    // The loop reads `stop_after` from `estigia goal`, so a cleared request must
    // not keep answering from the unreadable pointer either.
    let (out, err, ok) = run_in(
        home.path(),
        repository.path(),
        &[
            "goal",
            "--session-id",
            session,
            "--runtime",
            "opencode",
            "--json",
        ],
        "",
    );
    assert!(ok, "`goal` refused: {err}");
    let reading: serde_json::Value = serde_json::from_str(out.trim()).expect("JSON");
    assert_eq!(
        reading["stop_after"],
        serde_json::Value::Null,
        "a cleared request still answered: {reading}"
    );
}

/// No variable this crate decides on reaches a child from the developer's shell.
///
/// `run` points four variables at the fixture's home, and the fixture clears
/// everything named `ESTIGIA_*` and `GH_REPO`, `GH_HOST` and `GH_TOKEN`. The
/// clearing is the half worth holding: `ESTIGIA_FLAG` is read straight from the
/// environment and decides whether `unflagged-on-trunk` fires, so a child that
/// inherits it answers a question about whoever ran the suite. Nothing here
/// reaches a trunk delivery today, so the suite passes either way — which is
/// exactly why this is a source crossing rather than a behaviour one. There is
/// no surface that reports the flag back without a live tracker.
///
/// `GH_REPO` used to be `LEFT_REAL` here, on purpose: Estigia only ever writes
/// it, and only when the operator's own table names a repository, so an
/// operator who had it exported for their own work was meant to keep meaning
/// it. That stopped being safe to leave real once reconciliation
/// (`guard::adjudicate_action`, two or more pointers covering one checkout)
/// started asking the tracker at all — a several-holder decision this suite
/// drives against a fixture repository now makes a real `gh issue view` call,
/// and an inherited `GH_REPO` answers it against whatever repository the
/// operator's shell names instead of the fixture's. Checked:
/// `a_per_call_working_directory_selects_the_holder_that_owns_it` is the one
/// test in this file that reconciliation reaches through `run_in` rather than
/// through a scripted `tracker_command`, and it is the one this would have
/// made non-hermetic. `GH_HOST` and `GH_TOKEN` are cleared alongside it as the
/// same class of steering, on no evidence any test needs them real — nothing
/// in this crate writes either, so neither carried the same argument `GH_REPO`
/// did.
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
            let text = estigia::source_text::code_of(&text);
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
    ];
    // Written, not read, and only inside the MCP server process or a gate
    // renewal: how `gh` is told which repository the operator named, or which
    // this fixture's own reconciliation just asked about. Not `LEFT_REAL` —
    // this file's own module doc says why the two disagreeing is exactly the
    // shape that stopped being safe to leave to whatever the developer's shell
    // happened to hold — and not `ANSWERED` either: nothing here substitutes a
    // value, `run_with_path_locked` removes them outright.
    //
    // This bucket, on its own, is documentation rather than an assertion:
    // these three names are only ever `set_var`'d under `src/`, never
    // `var`/`var_os`'d there, so nothing in the scan above can ever place one
    // of them here to be checked — deleting `run_with_path_locked`'s own
    // `env_remove` loop would leave this whole test green. What actually
    // rules that mutation out is a different, narrower test:
    // `an_operators_gh_repo_never_reaches_a_spawned_gh`, above in this file,
    // puts `GH_REPO` on this process, spawns through
    // `run_with_path_locked` with a scripted `gh` on `PATH`, and reads back what
    // that `gh` actually saw — measured directly, by disabling the loop with
    // `if false { … }` while this comment was written: that test failed, and
    // reporting `GH_REPO=operators-own-org/operators-own-repo` back out of
    // the child it spawned.
    const CLEARED: &[&str] = &[
        "GH_REPO",
        "GH_HOST",
        "GH_TOKEN",
        "CLAUDE_CONFIG_DIR",
        "DSH_HOME",
        "PI_CODING_AGENT_DIR",
    ];
    for name in &read {
        if ANSWERED.contains(&name.as_str())
            || LEFT_REAL.contains(&name.as_str())
            || CLEARED.contains(&name.as_str())
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

/// No spawn of the binary in this file bypasses the helper that clears
/// `CLAUDE_CONFIG_DIR`.
///
/// `no_variable_the_crate_decides_on_escapes_the_fixture` above checks names
/// against `CLEARED`, so a second `Command::new(estigia())` that set its own
/// environment and forgot the variable would pass it — which is exactly how a
/// `setup --all` fixture came to write a full installation into a developer's
/// real relocated configuration while the suite reported green.
#[test]
fn every_spawn_of_the_binary_clears_claude_config_dir() {
    let text = estigia::source_text::code_of(crate::sources::SUITE_SOURCES);
    // The needles are concatenated so this guard's own source does not contain
    // them: a search for a literal written here counts itself, which is how a
    // guard comes to assert about its own text.
    let path_spelling = concat!("CARGO_BIN_EXE_", "estigia");
    let call = concat!("estigia", "()");
    let spawn = concat!("Command::new(", "estigia", "())");
    // The path is spelled in exactly one place. Counting only an exact
    // `Command::new(estigia())` read a spawn written with the environment macro
    // inline, or one holding the path in a local before spawning it, as no spawn
    // at all. The mutation that reddens this count and not the other two is such
    // a spawn outside `estigia_command` — `Command::new(env!("CARGO_BIN_EXE_estigia"))`
    // — measured at 2 while this comment was written; it names the path macro,
    // not `estigia()`, so the spawn count and the outside check do not move.
    let spelled = text.matches(path_spelling).count();
    assert_eq!(
        spelled, 1,
        "the binary's path is spelled {spelled} times in the suite; only its own helper may name it"
    );
    // And exactly one spawn: a second `Command::new` inside the helper would
    // clear nothing for the command it built, and the body check below cannot
    // see it, because the helper's body is what that check excises. The mutation
    // recorded earlier — a second `Command::new(estigia())` inside
    // `estigia_command` — reddens this count alone: it spells no
    // `CARGO_BIN_EXE_estigia` and sits in the body the outside check excises.
    let spawns = text.matches(spawn).count();
    assert_eq!(
        spawns, 1,
        "{spawns} direct spawns of the binary in the suite; only `estigia_command` may build one"
    );
    let helper = text
        .find(concat!("fn estigia_command", "()"))
        .expect("the helper is defined");
    let end = text[helper..].find("\n}\n").expect("the helper ends") + helper;
    let body = &text[helper..end];
    assert!(
        body.contains(spawn),
        "the helper no longer spawns the binary through its own path helper:\n{body}"
    );
    assert!(
        body.contains("env_remove(\"CLAUDE_CONFIG_DIR\")"),
        "the helper no longer clears the variable:\n{body}"
    );
    // Every other call to the path helper is inside `estigia_command`'s body or
    // its own definition: a spawn that obtained the path elsewhere is a second
    // call site, and the helper that clears the variable never saw it. The
    // mutation that reddens this check and not the other two is a spawn holding
    // the path in a local — `let bin = estigia(); Command::new(bin)` outside
    // `estigia_command` — measured past the two counts above, which both held.
    let definition = text
        .find(concat!("fn ", "estigia", "()"))
        .expect("the binary path helper is defined");
    let definition_end = text[definition..].find("\n}\n").expect("it ends") + definition;
    let outside = text
        .replace(&text[helper..end], "")
        .replace(&text[definition..definition_end], "");
    assert!(
        !outside.contains(call),
        "a spawn outside `estigia_command` obtains the binary path itself:\n{outside}"
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
        let status = estigia_command()
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

/// Reverting `checked_git_stdout`'s nonzero arm to a bare `.output()` capture
/// leaves every other test in this file green, because the shape it exists to
/// refuse — a failed git whose empty stdout satisfies a negative assertion —
/// is invisible to a suite that only ever runs a working git. The expectation
/// names the bogus field rather than the failure wording: both helpers share
/// one panic format, so the wording alone would let the setup line satisfy it,
/// and `ExitStatus` renders `exit code` only on Windows.
#[test]
#[should_panic(expected = "bogus-field")]
fn a_failed_fixture_git_panics_rather_than_answering_an_empty_capture() {
    let origin = tempfile::tempdir().expect("a directory");
    assert!(checked_git(origin.path(), &["init", "--quiet", "--bare"]));
    checked_git_stdout(origin.path(), &["for-each-ref", "--format=%(bogus-field)"]);
}

#[test]
fn flush_detach_returns_at_once_with_the_process_it_started() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (out, err, ok) = run(home.path(), &["flush", "--detach", "--json"], "");

    assert!(ok, "`estigia flush --detach` refused: {err}");
    let answer: serde_json::Value = serde_json::from_str(out.trim()).expect("one JSON answer");
    assert_eq!(answer["started"], true, "{answer}");
    assert!(answer["pid"].as_u64().is_some(), "{answer}");
}

fn seed_relay_job(home: &std::path::Path, run_id: &str, name: &str, model: Option<&str>) {
    seed_store_with(home, &[(1, "in-progress", Vec::new())]);
    let dir = home.join(format!("job-{name}"));
    std::fs::create_dir_all(&dir).expect("the job directory");
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::json!({ "model": model }).to_string(),
    )
    .expect("the manifest writes");
    let connection = rusqlite::Connection::open(home.join(".estigia").join("state.db"))
        .expect("the fixture store opens");
    connection
        .execute(
            "INSERT INTO review_jobs (id, repo_id, issue, run_id, name, epoch, pr, head, base,
                digest, payload_digest, offered, started_at, deadline_at, dir, state)
             SELECT ?1, id, 1, ?2, ?3, 'e', 1, 'h', 'b', 'd', 'p', '{}', 0, 0, ?4, 'running'
             FROM repos LIMIT 1",
            rusqlite::params![format!("job-{name}"), run_id, name, dir.to_string_lossy()],
        )
        .expect("the relay job records");
}

#[test]
fn review_model_prints_the_recorded_model_bare_and_nothing_when_there_is_none() {
    let home = tempfile::tempdir().expect("a temporary home");
    seed_relay_job(
        home.path(),
        "claude-relay0",
        "review-risk",
        Some("anthropic/claude-x"),
    );
    seed_relay_job(home.path(), "claude-relay0", "review-readability", None);

    let ask = |name: &str, json: bool| {
        let mut arguments = vec![
            "review",
            "model",
            "--name",
            name,
            "--run-id",
            "claude-relay0",
        ];
        if json {
            arguments.push("--json");
        }
        run(home.path(), &arguments, "")
    };

    let (out, err, ok) = ask("review-risk", false);
    assert!(ok, "`estigia review model` refused: {err}");
    assert_eq!(out, "anthropic/claude-x\n");

    let (out, err, ok) = ask("review-readability", false);
    assert!(ok, "`estigia review model` refused: {err}");
    assert_eq!(out, "");

    let (out, _, ok) = ask("review-risk", true);
    assert!(ok);
    let said: serde_json::Value = serde_json::from_str(out.trim()).expect("one JSON answer");
    assert_eq!(said["model"], "anthropic/claude-x", "{said}");
}
