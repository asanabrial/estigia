//! One lock over the process environment, for the tests that need to set one —
//! plus `repository_with_commit` below, a git fixture kept in this module for
//! the reason its own doc comment gives, that takes no lock and touches no
//! environment, and the scripted-`gh` fixtures, which set no process variable at
//! all: they hand the fake `gh` and its answers to the transport through
//! [`crate::transport::with_process_override`], a thread-local the spawning seam
//! reads, so those tests run concurrently.
//!
//! A process has one environment, and `cargo` runs a crate's unit tests as one
//! multi-threaded binary. Two helpers with a `Mutex` each are therefore two
//! locks excluding nothing from one another — and that is what stood here: a
//! copy of this helper lived in `harness::roles::tests` and another in
//! `setup::tests`, each justified by a comment saying *"these are two test
//! binaries and neither can call the other's"*.
//!
//! That comment was false, and measurably so: `cargo test --lib config_home
//! -- --list` names one binary. A reviewer reproduced the consequence — five
//! failures in eighty filtered runs at `--test-threads=4`, none at
//! `--test-threads=1`, with assertions reading *"a borrowed home takes the
//! variable"* and *"an override of `relative/config` fell through to the
//! variable"*. Exactly environment cross-talk, and it bites the filtered run a
//! developer types rather than a full pass, where 970 other tests spread the
//! window out.
//!
//! The same false premise was carrying both `// SAFETY:` notes on the
//! `set_var` calls — and a narrower version of it was still false after the
//! fix above landed: the notes said this lock was *"the only thread reading
//! or writing any of these four variables for the duration"*, but nothing
//! made that true of a *reader*. `with_home_env` sets `HOME` for the length
//! of its own `body`, and a test on another thread that never calls it —
//! `tui::tests::the_walk_starts_where_the_row_is_set_and_falls_back_to_home`,
//! reading `paths::home_dir()` once directly and again through
//! `App::folder_root()` — could read across that window unlocked and see the
//! variable flip between its two reads. Measured at fifteen runs of `cargo
//! test --lib home -- --test-threads=8`: three failures, twelve passes, the
//! victim's own assertion comparing a `TempDir` on the left to the real home
//! on the right. `with_environment_read_locked` below is for exactly that
//! shape: a test that only *reads* these variables but takes more than one
//! read that has to agree, wrapping the whole read-and-assert region rather
//! than each read on its own — locking each separately still leaves the gap
//! *between* the two calls open, which is the gap a concurrent `with_home_env`
//! landed in.
//!
//! The home, config and process setters restore what they changed with
//! straight-line code — `let answer = body(); …restore…` — so a panic inside
//! `body()`, an ordinary failing assertion, unwound past the restore and left
//! the variable pointing at a `TempDir` that the same unwind then deleted.
//! Every later test in the binary resolved into a directory that no longer
//! existed, burying the original failure under one that named nothing the
//! failing test touched. `RestoreEnvOnDrop` below runs on an unwind the same as
//! it runs on success, because it is `Drop`, not a line the unwind can skip
//! past. `with_scripted_gh_across_threads` is the last to take it: the one
//! scripted-`gh` fixture that still writes `PATH` and `ESTIGIA_FAKE_ANSWERS`
//! process-wide, for the two tests whose own threads spawn `gh`.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

/// The one lock. Private, so no caller can take it and forget to give it back.
static ENVIRONMENT: Mutex<()> = Mutex::new(());

/// Puts back the environment variables it was given when dropped — including
/// when it is dropped by an unwind, which a bare `restore` line placed after
/// `body()` is not: an unwind runs past that line without executing it. This
/// runs on either exit, because a `Drop` impl is not a line to skip.
struct RestoreEnvOnDrop {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl Drop for RestoreEnvOnDrop {
    fn drop(&mut self) {
        // SAFETY: every writer of these variables in this module takes
        // `ENVIRONMENT` for the whole of its call, including the unwind that
        // can run this restore, so this is still the only thread writing them
        // for as long as that lock is held — which, by construction, is for
        // as long as this guard can be alive.
        unsafe {
            for (name, previous) in self.saved.drain(..) {
                match previous {
                    Some(previous) => std::env::set_var(name, previous),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

/// Take the one lock over the process environment for the whole of `body`,
/// without setting anything.
///
/// For a test that only *reads* `HOME`, `USERPROFILE` or any variable the
/// setters below serialise, but takes more than one read across that region
/// that must see the same answer. `with_home_env` running on another thread
/// sets and restores its variables inside the same lock, so a read outside it
/// can land in that window; taking this lock once around the whole
/// read-and-assert region closes the window for its duration. Calling it from
/// inside a body already holding `ENVIRONMENT` — a `with_home_env` or
/// `with_config_home` closure — deadlocks, because the lock is not reentrant;
/// this helper is for the reads that stand outside both.
pub(crate) fn with_environment_read_locked<T>(body: impl FnOnce() -> T) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);
    let answer = body();
    drop(guard);
    answer
}

/// Run `body` with `name` set to `value` in the process environment, and put
/// back whatever was there.
///
/// For a single variable no other helper in this module already owns —
/// `GIT_DIFF_OPTS` is the first caller — rather than growing a fifth
/// near-identical setter. `RestoreEnvOnDrop` is what makes this safe to reuse
/// across an early return or a panic in `body`, the same as every setter
/// below.
pub(crate) fn with_process_env<T>(name: &'static str, value: &str, body: impl FnOnce() -> T) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let before = std::env::var_os(name);
    let restore = RestoreEnvOnDrop {
        saved: vec![(name, before)],
    };
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment,
    // and this is the only writer of `name` this call is asked to set, held
    // for the whole of `body` — by `guard` on success, and by `restore`'s own
    // `Drop` impl if `body` panics, which runs before `guard` is released
    // either way. `GIT_DIFF_OPTS`, the first caller, has no in-process
    // reader at all — it is read only by a `git` child process, which sees
    // whatever snapshot `Command::spawn` took at the moment it started —
    // so no unlocked reader in this crate can observe this window.
    unsafe {
        std::env::set_var(name, value);
    }
    let answer = body();
    drop(restore);
    drop(guard);
    answer
}

/// Run `body` with the Linear credential and endpoint set, and put both back.
///
/// The two variables the read-only Linear source reads, under the one lock for
/// the same reason the setters above take it: `crate::transport::linear` reads
/// them from the process environment, and a test on another thread must not see
/// this window. `credential` of `None` stands for unset, which is the missing
/// credential a refusal names.
pub(crate) fn with_linear_env<T>(
    credential: Option<&str>,
    endpoint: &str,
    body: impl FnOnce() -> T,
) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let credential_var = crate::transport::linear::CREDENTIAL_VAR;
    let endpoint_var = crate::transport::linear::ENDPOINT_VAR;
    let saved = vec![
        (credential_var, std::env::var_os(credential_var)),
        (endpoint_var, std::env::var_os(endpoint_var)),
    ];
    let restore = RestoreEnvOnDrop { saved };
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment,
    // and this is the only writer of these two variables in this module, held
    // for the whole of `body` — by `guard` on success, and by `restore`'s own
    // `Drop` impl if `body` panics, which runs before `guard` is released
    // either way. The source reads them only from inside `body`.
    unsafe {
        match credential {
            Some(value) => std::env::set_var(credential_var, value),
            None => std::env::remove_var(credential_var),
        }
        std::env::set_var(endpoint_var, endpoint);
    }
    let answer = body();
    drop(restore);
    drop(guard);
    answer
}

/// Run `body` with `XDG_CONFIG_HOME` set, and put back what was there.
///
/// An empty value stands for *absent*, which is what `absolute_or_none` already
/// makes of it, so a caller wanting "unset" passes an empty path rather than
/// reaching for a second helper.
pub(crate) fn with_config_home<T>(value: &Path, body: impl FnOnce() -> T) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let before = std::env::var_os("XDG_CONFIG_HOME");
    let restore = RestoreEnvOnDrop {
        saved: vec![("XDG_CONFIG_HOME", before)],
    };
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment,
    // and every writer of `XDG_CONFIG_HOME` in this module takes it for the
    // whole of its call — by `guard` on success, and by `restore`'s own
    // `Drop` impl if `body` panics, which runs before `guard` is released
    // either way. So this is still the only thread *writing* the variable for
    // as long as it differs from what was there before this call — narrower
    // than "the only thread reading or writing it", which the module header
    // above already found false of the four-variable version of this call and
    // is no truer here: `harness::tests::the_control_surface_is_a_real_location_rather_than_an_empty_path`
    // reads `XDG_CONFIG_HOME` through `resolve_paths` with no lock at all.
    // What makes that reader tolerable rather than merely undemonstrated is
    // that it asserts a property invariant to *which* value was read — that
    // the resolved path is non-empty and ends in the skill directory — never
    // a value compared against a second, independent read of the same
    // variable, so no interleaving with this write changes its answer.
    //
    // That reader is not the only unlocked one, and this call is the writer the
    // others bear on hardest: `harness::roles::definition_for` reads this same
    // variable unlocked through `setup::xdg_config_home`, and the callers that
    // point it at a directory actually holding an OpenCode definition are in
    // `harness::roles::tests`, not among this module's own. See
    // `with_home_env`'s note below for that family and for why it does not
    // flake today — a reason that is about which names those readers ask for,
    // and is held by nothing.
    //
    // So `set_var`'s documented precondition — no concurrent read anywhere in
    // the process — is narrowed here, not met.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", value);
    }
    let answer = body();
    drop(restore);
    drop(guard);
    answer
}

/// Run `body` with `HOME`, `USERPROFILE`, `APPDATA` and `XDG_CONFIG_HOME` all
/// set and `CLAUDE_CONFIG_DIR`, `DSH_HOME` and `PI_CODING_AGENT_DIR` cleared, and put back whatever
/// was there.
///
/// This road, not `a_moved_home_moves_every_root_under_it`'s: that test
/// sets `SetupOptions.home_dir` directly, which is a road no CLI flag reaches.
/// `Environment::resolve` reads a moved home from `HOME`/`USERPROFILE` instead
/// on that road, and reads `APPDATA`/`XDG_CONFIG_HOME` from the process
/// environment independently of it — so a test that wants to reproduce the
/// defect has to redirect all four together, a temporary home and a *second*,
/// different temporary directory standing in for the operator's real profile.
/// `CLAUDE_CONFIG_DIR` is cleared rather than pointed anywhere, so a borrowed
/// home a test builds this way cannot see the developer's own shell —
/// and `DSH_HOME` for the same reason.
/// [`with_home_env_and_claude_config_dir`] and
/// [`with_home_env_and_dsh_home`] are the siblings that set one.
pub(crate) fn with_home_env<T>(
    home: &Path,
    app_data: &Path,
    config_home: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_home_env_and_optional_claude_config_dir(
        home,
        app_data,
        config_home,
        None,
        None,
        None,
        body,
    )
}

/// [`with_home_env`], with `CLAUDE_CONFIG_DIR` set to `claude_config_dir`
/// rather than cleared.
pub(crate) fn with_home_env_and_claude_config_dir<T>(
    home: &Path,
    app_data: &Path,
    config_home: &Path,
    claude_config_dir: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_home_env_and_optional_claude_config_dir(
        home,
        app_data,
        config_home,
        Some(claude_config_dir),
        None,
        None,
        body,
    )
}

/// [`with_home_env`], with `DSH_HOME` set to `dsh_home` rather than cleared.
///
/// `DSH_HOME` is read only when the home is borrowed from the environment, so
/// this is the road a containment test has to take — `SetupOptions.home_dir`
/// answers on its own and never falls through to the variable.
pub(crate) fn with_home_env_and_dsh_home<T>(
    home: &Path,
    app_data: &Path,
    config_home: &Path,
    dsh_home: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_home_env_and_optional_claude_config_dir(
        home,
        app_data,
        config_home,
        None,
        Some(dsh_home),
        None,
        body,
    )
}

/// [`with_home_env`], with `PI_CODING_AGENT_DIR` set to `pi_agent_dir` rather
/// than cleared.
pub(crate) fn with_home_env_and_pi_agent_dir<T>(
    home: &Path,
    app_data: &Path,
    config_home: &Path,
    pi_agent_dir: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_home_env_and_optional_claude_config_dir(
        home,
        app_data,
        config_home,
        None,
        None,
        Some(pi_agent_dir),
        body,
    )
}

/// The shared body of [`with_home_env`], [`with_home_env_and_claude_config_dir`],
/// [`with_home_env_and_dsh_home`] and [`with_home_env_and_pi_agent_dir`]: every one of the seven variables under one
/// call rather than nesting `with_config_home` inside a new helper for the rest
/// — `ENVIRONMENT` is a plain, non-reentrant `Mutex`, so a thread that already
/// holds it deadlocks trying to take it again.
fn with_home_env_and_optional_claude_config_dir<T>(
    home: &Path,
    app_data: &Path,
    config_home: &Path,
    claude_config_dir: Option<&Path>,
    dsh_home: Option<&Path>,
    pi_agent_dir: Option<&Path>,
    body: impl FnOnce() -> T,
) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let names: [&'static str; 7] = [
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "XDG_CONFIG_HOME",
        "CLAUDE_CONFIG_DIR",
        "DSH_HOME",
        "PI_CODING_AGENT_DIR",
    ];
    let saved: Vec<_> = names
        .iter()
        .map(|name| (*name, std::env::var_os(name)))
        .collect();
    let restore = RestoreEnvOnDrop { saved };
    // SAFETY: `ENVIRONMENT` is the only lock over this process's environment,
    // and every writer of these seven variables in this module takes it for
    // the whole of its call — by `guard` on success, and by `restore`'s own
    // `Drop` impl if `body` panics, which runs before `guard` is released
    // either way. So this is still the only thread *writing* any of these
    // seven variables for as long as any of them differs from what was there
    // before this call — narrower than "the only thread reading or writing
    // them for the duration", which this module's own header names as the
    // previous, false version of this note: replacing "for the duration"
    // with "for as long as any of them differs" changed which interval was
    // claimed, not the "any thread" the header says was never made true.
    // Readers in this crate take these variables with no lock at all, and
    // this is a hand check rather than a count — a count is what an earlier
    // draft got wrong, by naming two on the same day a third was created.
    // The ones this check found: `paths::tests::home_resolves_to_an_absolute_path`
    // reads `HOME` through `home_dir()`;
    // `harness::tests::the_control_surface_is_a_real_location_rather_than_an_empty_path`
    // reads `HOME`, `APPDATA` and `XDG_CONFIG_HOME` through `resolve_paths`;
    // and `harness::tests::the_local_settings_and_the_agent_definitions_are_boundaries_on_both_roads`
    // reads `HOME` once to spell the targets it then classifies.
    // What makes each of those tolerable is one property they share: each
    // asserts something invariant to *which* value was read — that the answer
    // is absolute, that it is non-empty and ends in the skill directory, that
    // `classify` calls it a boundary — never a value compared against a second,
    // independent read of the same variable.
    //
    // The enumeration is not claimed to be all of them, and one family it
    // missed is also the counterexample to the property above:
    // `harness::roles::definition_for` reads `XDG_CONFIG_HOME` unlocked through
    // `setup::xdg_config_home`, from call sites in `harness::roles::tests` that
    // take no lock, and a test asserting `definition_for(..).is_none()` asserts
    // something the value helps decide, since one search root is built from it.
    //
    // Why that does not flake today is narrower than it looks, and stating it
    // wrongly is how an earlier draft of this note failed: a fixture *does*
    // write `opencode/agents/scribe.md` under a temporary config home
    // (`harness::roles::tests`), so the reason is not that none exists. It is
    // that the unlocked readers ask for other names. Name-disjointness, held by
    // nothing.
    //
    // So `set_var`'s documented precondition — no concurrent read anywhere in
    // the process — is narrowed here, not met. What would falsify this note is
    // a new unlocked reader, or a fixture that gives an existing one something
    // to find.
    unsafe {
        std::env::set_var("HOME", home);
        std::env::set_var("USERPROFILE", home);
        std::env::set_var("APPDATA", app_data);
        std::env::set_var("XDG_CONFIG_HOME", config_home);
        match claude_config_dir {
            Some(directory) => std::env::set_var("CLAUDE_CONFIG_DIR", directory),
            None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
        }
        match dsh_home {
            Some(directory) => std::env::set_var("DSH_HOME", directory),
            None => std::env::remove_var("DSH_HOME"),
        }
        match pi_agent_dir {
            Some(directory) => std::env::set_var("PI_CODING_AGENT_DIR", directory),
            None => std::env::remove_var("PI_CODING_AGENT_DIR"),
        }
    }
    let answer = body();
    drop(restore);
    drop(guard);
    answer
}

/// Runs `body` with a fake `gh` in `directory` answering from `answers`.
///
/// The transport resolves the `gh` in `directory` to that copy and hands it
/// `answers` on the child's own environment — see
/// [`crate::transport::with_process_override`]. Nothing process-wide is written,
/// so two fixtures run concurrently without seeing each other. `directory` is
/// expected to hold a real executable named `gh`: a script will not do, because
/// on Windows resolving a bare `gh` finds `gh.exe` and never a `gh.cmd`.
///
/// For a body that spawns its own threads, whose `gh` calls run off this one,
/// use [`with_scripted_gh_across_threads`] instead.
pub(crate) fn with_scripted_gh<T>(directory: &Path, answers: &str, body: impl FnOnce() -> T) -> T {
    with_scripted_gh_window(directory, answers, &[], body)
}

/// [`with_scripted_gh`] with `ESTIGIA_FAKE_COUNT` pointed at `counts` for the
/// same window, so an answer carrying `nth` can change its mind by call number.
pub(crate) fn with_counting_gh<T>(
    directory: &Path,
    answers: &str,
    counts: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_scripted_gh_window(
        directory,
        answers,
        &[("ESTIGIA_FAKE_COUNT", counts.as_os_str().to_owned())],
        body,
    )
}

/// [`with_scripted_gh`] with `ESTIGIA_FAKE_LOG` pointed at `log` for the same
/// window, so every call the fake `gh` is asked is readable afterwards.
pub(crate) fn with_logging_gh<T>(
    directory: &Path,
    answers: &str,
    log: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_scripted_gh_window(
        directory,
        answers,
        &[("ESTIGIA_FAKE_LOG", log.as_os_str().to_owned())],
        body,
    )
}

/// [`with_scripted_gh`] with `ESTIGIA_FAKE_BODY_LOG` pointed at `body_log` for
/// the same window, so the `--body-file` a write stages is readable after the
/// write deletes it.
///
/// The fixture already ships this seam — `examples/fake_process.rs` copies the
/// staged body before the caller removes it — and this exposes it to a unit
/// test the way [`with_counting_gh`] exposes the counter.
pub(crate) fn with_body_log<T>(
    directory: &Path,
    answers: &str,
    body_log: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_scripted_gh_window(
        directory,
        answers,
        &[("ESTIGIA_FAKE_BODY_LOG", body_log.as_os_str().to_owned())],
        body,
    )
}

/// [`with_scripted_gh`] with `ESTIGIA_FAKE_ENV_LOG` pointed at `env_log`, so
/// the `GH_REPO`, `GH_HOST` and `GH_TOKEN` one call's environment held are
/// readable afterwards.
pub(crate) fn with_env_logging_gh<T>(
    directory: &Path,
    answers: &str,
    env_log: &Path,
    body: impl FnOnce() -> T,
) -> T {
    with_scripted_gh_window(
        directory,
        answers,
        &[("ESTIGIA_FAKE_ENV_LOG", env_log.as_os_str().to_owned())],
        body,
    )
}

/// [`with_scripted_gh`] for a body that spawns threads of its own.
///
/// A spawned thread does not inherit the thread-local
/// [`crate::transport::with_process_override`] installs, so a fixture whose
/// racer threads call `gh` puts the fake ahead of the real one on the process
/// `PATH` instead, under [`ENVIRONMENT`], for the whole of `body`. That keeps
/// the two such fixtures correct at the cost of serialising them; every other
/// scripted-`gh` fixture runs concurrently.
pub(crate) fn with_scripted_gh_across_threads<T>(
    directory: &Path,
    answers: &str,
    body: impl FnOnce() -> T,
) -> T {
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);

    let path_before = std::env::var_os("PATH");
    let mut joined = std::ffi::OsString::from(directory);
    if let Some(existing) = &path_before {
        joined.push(if cfg!(windows) { ";" } else { ":" });
        joined.push(existing);
    }
    let answers_before = std::env::var_os("ESTIGIA_FAKE_ANSWERS");
    let restore = RestoreEnvOnDrop {
        saved: vec![
            ("PATH", path_before),
            ("ESTIGIA_FAKE_ANSWERS", answers_before),
        ],
    };
    // SAFETY: `ENVIRONMENT` is held for the whole of `body` and `restore` puts
    // both names back on either exit, so this is the only writer of either for
    // as long as it differs from what was there before. `PATH` has unlocked
    // readers — `setup::companion::resolve_on_path` — but this call only ever
    // prepends a directory holding `gh` ahead of the existing `PATH`, never
    // removing an entry another program's answer could depend on, so no such
    // reader's answer turns on whether this window is open. The one fake
    // variable has no in-process reader at all: only `fake_process`, spawned as
    // a separate process, ever reads it.
    unsafe {
        std::env::set_var("PATH", &joined);
        std::env::set_var("ESTIGIA_FAKE_ANSWERS", answers);
    }
    let answer = body();
    drop(restore);
    drop(guard);
    answer
}

fn with_scripted_gh_window<T>(
    directory: &Path,
    answers: &str,
    extra: &[(&str, OsString)],
    body: impl FnOnce() -> T,
) -> T {
    let mut envs: Vec<(String, OsString)> =
        vec![("ESTIGIA_FAKE_ANSWERS".to_owned(), OsString::from(answers))];
    envs.extend(
        extra
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone())),
    );
    crate::transport::with_process_override(directory, &envs, body)
}

/// A directory holding a real `gh` that answers from `ESTIGIA_FAKE_ANSWERS`.
///
/// Built from the `fake_process` example the same way `tests/pipe.rs`'s own
/// `tracker_rig` finds it: this test binary sits under `deps/`, one level
/// below the profile directory the example's own `examples/` sits beside. A
/// real executable, not a script — `examples/fake_process.rs`'s own module
/// doc says why a script answers nothing on Windows, where resolving a bare
/// `gh` finds `gh.exe` and never a `gh.cmd`.
///
/// Shared rather than written once per test module: `harness::guard::tests`
/// and `harness::doctor`'s own module both need one `gh` to answer a
/// `verify-claim` differently per issue, and a second copy of "where does the
/// example live" is exactly the copy this repository keeps finding.
pub(crate) fn scripted_gh() -> &'static tempfile::TempDir {
    fake_cli_directory(&["gh"])
}

/// A directory holding one copy of the `fake_process` example per name, each
/// spelled as the executable `PATH` resolution would find on this platform.
///
/// One directory per distinct set of names for the whole test binary, not one
/// per call: the fixture is read-only, so a second copy of the same binary is
/// I/O the suite pays for and nothing reads. The lock is held
/// only while the directory is looked up or built, never across a caller's body.
pub(crate) fn fake_cli_directory(names: &[&str]) -> &'static tempfile::TempDir {
    static CACHE: std::sync::OnceLock<
        Mutex<std::collections::HashMap<String, &'static tempfile::TempDir>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let key = names.join(",");
    let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(directory) = cache.get(&key) {
        return directory;
    }
    let directory: &'static tempfile::TempDir =
        Box::leak(Box::new(build_fake_cli_directory(names)));
    cache.insert(key, directory);
    directory
}

#[path = "../tests/support/fixture_flags.rs"]
mod fixture_flags;

fn missing_fixture_message(built: &Path) -> String {
    format!(
        "the process fixture is not built, so this test would measure nothing: run `cargo build \
         --examples{}` ({})",
        fixture_flags::fixture_build_flags(built),
        built.display()
    )
}

#[cfg(unix)]
fn link_fixture(built: &Path, dest: &Path) {
    std::os::unix::fs::symlink(built, dest).expect("the fake CLI is linked onto the path");
}

#[cfg(not(unix))]
fn link_fixture(built: &Path, dest: &Path) {
    std::fs::copy(built, dest).expect("the fake CLI is copied onto the path");
}

fn build_fake_cli_directory(names: &[&str]) -> tempfile::TempDir {
    let here = std::env::current_exe().expect("this test binary knows where it is");
    let built = here
        .parent()
        .and_then(Path::parent)
        .expect("the test binary is under a profile directory")
        .join("examples")
        .join(if cfg!(windows) {
            "fake_process.exe"
        } else {
            "fake_process"
        });
    assert!(built.is_file(), "{}", missing_fixture_message(&built));
    let bin = tempfile::tempdir().expect("a directory for the fake CLIs");
    for name in names {
        let dest = bin.path().join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            (*name).to_owned()
        });
        link_fixture(&built, &dest);
    }
    bin
}

/// Runs `body` with a directory holding the named fake CLIs, and with
/// `variables` set for the fakes to read, then puts the variables back.
///
/// Unless a caller sets `ESTIGIA_FAKE_STDIN_LOG` or
/// `ESTIGIA_FAKE_EXIT_WITHOUT_READING`, the fakes are pointed at the null device
/// so they read their standard input to its end: a fake that exits before the
/// prompt is written turns the write into a broken pipe, which the reviewer
/// rightly reads as a launch failure, and under load the fake wins that race.
///
/// `PATH` is left alone: replacing it process-wide starves every other test
/// that spawns `node`, `sh` or `git` while this one holds the lock, so callers
/// reach the fakes by the directory's path instead.
pub(crate) fn with_fake_clis<T>(
    names: &[&str],
    variables: &[(&'static str, &OsStr)],
    body: impl FnOnce(&Path) -> T,
) -> T {
    let bin = fake_cli_directory(names);
    let guard = ENVIRONMENT.lock().unwrap_or_else(PoisonError::into_inner);
    let discard = OsString::from(if cfg!(windows) { "NUL" } else { "/dev/null" });
    let reads_stdin = variables.iter().any(|(name, _)| {
        matches!(
            *name,
            "ESTIGIA_FAKE_STDIN_LOG" | "ESTIGIA_FAKE_EXIT_WITHOUT_READING"
        )
    });
    let mut all: Vec<(&'static str, &OsStr)> = variables.to_vec();
    if !reads_stdin {
        all.push(("ESTIGIA_FAKE_STDIN_LOG", discard.as_os_str()));
    }
    let saved = all
        .iter()
        .map(|(name, _)| (*name, std::env::var_os(name)))
        .collect();
    let restore = RestoreEnvOnDrop { saved };
    // SAFETY: `ENVIRONMENT` is held for the whole of `body`, `restore` puts every
    // name back on either exit, and the fake variables have no in-process reader.
    unsafe {
        for (name, value) in &all {
            std::env::set_var(name, value);
        }
    }
    let answer = body(bin.path());
    drop(restore);
    drop(guard);
    answer
}

/// A scripted answer matching `gh issue view <issue> --json …`, so two
/// holders naming different issues can be answered differently by one `gh`.
pub(crate) fn issue_view_answer(issue: u64, stdout: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "matches": format!("view {issue} --json"),
        "stdout": stdout.to_string(),
        "status": 0,
    })
}

/// What `gh issue view` answers for an issue the run's own claim never
/// touched: closed, no comments, nothing an own-delivery close could find —
/// which is exactly what `verify_claim` reads as `issue-not-open`.
pub(crate) fn closed_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({ "state": "CLOSED", "labels": [], "comments": [] }),
    )
}

/// What `gh issue view` answers for an issue that is open and does not carry
/// the label `verify_claim` expects — read as `unexpected-state`, a real
/// answer and not the one reconciliation drops a holder on.
pub(crate) fn open_but_unmatched_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({ "state": "OPEN", "labels": [], "comments": [] }),
    )
}

/// What `gh issue view` answers for an issue that is open, whose label
/// matches the default expected state (`status:in-progress`) so the check
/// ahead of ownership passes, and whose comments carry no acquisition for
/// anybody at all — `verify_claim`'s `holding` reduces that to "current live
/// holder is none", the `not-current-live-holder` reason a pointer projecting
/// a claim nobody holds answers with.
pub(crate) fn not_current_live_holder_issue(issue: u64) -> serde_json::Value {
    issue_view_answer(
        issue,
        serde_json::json!({
            "state": "OPEN",
            "labels": [{ "name": "status:in-progress" }],
            "comments": [],
        }),
    )
}

/// What `gh issue view` answers for an issue this exact run genuinely,
/// currently holds — the shape `verify_claim` answers `ok` to, and the one
/// that lets `gate` reach `Decision::Allow`. The same marker shape
/// `tests/pipe.rs`'s own `issue_answer` uses, so a horizon `2099-01-01T00:00Z`
/// ahead of any clock this suite runs on.
pub(crate) fn live_holder_issue(run_id: &str, issue: u64, state: &str) -> serde_json::Value {
    let marker = format!(
        "<!-- issue-flow: claim run-id={run_id} runtime=claude horizon=2099-01-01T00:00Z \
         op-id={} -->",
        "a".repeat(32)
    );
    issue_view_answer(
        issue,
        serde_json::json!({
            "state": "OPEN",
            "labels": [{ "name": format!("status:{state}") }],
            "comments": [{
                "id": "IC_1",
                "createdAt": "2026-01-01T00:00Z",
                "viewerDidAuthor": true,
                "includesCreatedEdit": false,
                "body": format!("Claimed by {run_id}.\n\n{marker}\n"),
            }],
        }),
    )
}

/// A `gh` that cannot answer at all, for whatever `issue view` it is asked.
pub(crate) fn unreachable_tracker_answer() -> serde_json::Value {
    serde_json::json!({
        "matches": "issue view",
        "stderr": "gh: could not resolve to a repository",
        "status": 1,
    })
}

/// `ESTIGIA_FAKE_ANSWERS`, from the entries above.
///
/// The fake `gh` answers the REST open-or-closed question every issue
/// operation now asks from these same `issue view` entries, so a fixture that
/// poses a closed issue through `issue view` keeps meaning it.
pub(crate) fn answers(entries: &[serde_json::Value]) -> String {
    serde_json::Value::Array(entries.to_vec()).to_string()
}

/// A real repository with one commit, so `git worktree add -b` has a start
/// point to branch from — an unborn `HEAD` refuses that outright, unlike the
/// bare `git init` some neighbouring test rigs use.
///
/// Here rather than in either module that needs it. This
/// fixture was once added twice in one change — once in `harness::mcp::tests` and once in
/// `transport::branch::tests`, four identical `git` calls under two names —
/// which is the shape the module doc above is entirely about. A second copy
/// does not merely repeat itself: it drifts, and then the two modules disagree
/// about what "a repository a worktree can be added to" means while both pass.
///
/// It panics rather than answering `None`, the same repair `tracker_rig` got:
/// this is a git workflow harness, so a machine without a usable
/// `git` cannot measure the work built on this at all, and a `return;` after an
/// `eprintln!` reports an unmeasured test as `ok` to every ordinary run. The
/// tolerance the old `Option` carried was also wider than a missing `git` — it
/// covered *any* failure of the four calls, and of `tempfile::tempdir()`
/// besides, so a machine that signs commits or runs a hook took every test
/// built on this to green having measured nothing. `docs/honesty.md` records
/// what the silent form cost.
pub(crate) fn repository_with_commit() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let git = |arguments: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(directory.path())
            .args(arguments)
            .output()
            .unwrap_or_else(|error| {
                panic!(
                    "this fixture needs a usable `git` on PATH and could not run \
                     `git {}`: {error}. Install git, or fix the command, because a test \
                     that reports `ok` without running is a defect this repository has filed against itself.",
                    arguments.join(" ")
                )
            });
        assert!(
            output.status.success(),
            "`git {}` failed: {}. This fixture needs a usable `git` on PATH — \
             `docs/honesty.md` records why an unmeasured test must not report `ok`.",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "nobody@example.invalid"]);
    git(&["config", "user.name", "nobody"]);
    git(&["commit", "--allow-empty", "--quiet", "-m", "base"]);
    directory
}

/// A path for a shell command or a TOML string: forward slashes, so neither
/// reads a `\` as an escape, and on Windows the long name with no `\\?\`
/// prefix, since `~` is not a character the gate's reader accepts in a literal
/// target. A path that does not exist yet resolves through its parent.
pub(crate) fn shell_path(path: &Path) -> String {
    if cfg!(windows) {
        let resolved = path
            .canonicalize()
            .ok()
            .or_else(|| {
                let parent = path.parent()?.canonicalize().ok()?;
                Some(parent.join(path.file_name()?))
            })
            .unwrap_or_else(|| path.to_path_buf());
        crate::paths::remove_windows_verbatim_prefix(resolved)
            .display()
            .to_string()
            .replace('\\', "/")
    } else {
        path.display().to_string()
    }
}

/// A command line whose POSIX absolute literals (`/tmp/…`, `/private/tmp/…`)
/// name somewhere outside any fixture checkout, rewritten on Windows to an
/// absolute path that platform reads as absolute. Unchanged elsewhere.
pub(crate) fn outside_the_checkout(command: &str) -> String {
    if cfg!(windows) {
        let root = shell_path(&std::env::temp_dir());
        command
            .replace("/private/tmp/", "/tmp/")
            .replace("/tmp/", &format!("{root}/tmp/"))
    } else {
        command.to_owned()
    }
}

/// The fixture's own claim: the environment [`with_home_env`] built really does
/// put both inherited roots outside `home`.
///
/// Read from the environment rather than from anything the code under test
/// returns, so that deleting the containment rule reddens a test's escape
/// assertion with the path that escaped, and never this one with a message
/// about the fixture. A guard that fails when the fix is removed sends a
/// mutation sweep to repair the test instead of the code — which two reviewers
/// of publication `bf5bf67c77255ec0aaaea528f1829401` reached independently
/// about the shape this replaced.
///
/// It is not a formality: on Windows `%TEMP%` sits under `%USERPROFILE%`, so
/// two temporary directories are not self-evidently disjoint.
pub(crate) fn assert_inherited_roots_escape(home: &Path) {
    for variable in ["APPDATA", "XDG_CONFIG_HOME"] {
        let held = std::env::var_os(variable)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| panic!("the fixture left {variable} unset"));
        assert!(
            !crate::paths::covers(home, &held),
            "the fixture stopped reproducing the escape: {variable} holds {}, under the home {}",
            held.display(),
            home.display()
        );
    }
}

/// The honesty contract, across the documents it now spans.
///
/// `docs/honesty.md` is the index and the last run lives in
/// `docs/honesty-review-tier.md`, so a caller reading one path would drop
/// whatever moved with it.
pub(crate) fn honesty_contract() -> String {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
        .expect("docs/ ships with the crate")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "honesty.md" || name.starts_with("honesty-"))
        })
        .filter(|path| path.extension().is_some_and(|kind| kind == "md"))
        .collect();
    paths.sort();
    let text: String = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("a readable document"))
        .collect();
    assert!(
        text.len() > 10_000,
        "the honesty contract is {} bytes, too short to be the contract these tests cross",
        text.len()
    );
    text
}

/// An absolute directory this platform reads as absolute: `/work/trees` is
/// drive-relative on Windows, which a `Worktree location` refuses.
pub(crate) fn work_trees() -> &'static str {
    if cfg!(windows) {
        "C:/work/trees"
    } else {
        "/work/trees"
    }
}

/// Every call in a `with_logging_gh` log that would change something on
/// GitHub, as the line `gh` was given.
pub(crate) fn logged_writes(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default())
        .filter(|line| {
            let mut words = vec!["gh"];
            words.extend(line.split_whitespace());
            crate::transport::window::is_outbound_write(&words, crate::transport::How::tolerated())
        })
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod missing_fixture_message_tests {
    use super::missing_fixture_message;
    use std::path::Path;

    #[test]
    fn a_debug_path_names_the_plain_command() {
        let message = missing_fixture_message(Path::new("/w/target/debug/examples/fake_process"));
        assert!(message.contains("`cargo build --examples`"), "{message}");
    }

    #[test]
    fn a_release_path_names_release() {
        let message = missing_fixture_message(Path::new("/w/target/release/examples/fake_process"));
        assert!(
            message.contains("`cargo build --examples --release`"),
            "{message}"
        );
    }

    #[test]
    fn a_target_path_names_the_triple() {
        let message = missing_fixture_message(Path::new(
            "/w/target/aarch64-apple-darwin/release/examples/fake_process",
        ));
        assert!(
            message.contains("`cargo build --examples --release --target aarch64-apple-darwin`"),
            "{message}"
        );
    }

    #[test]
    fn a_custom_profile_path_names_the_profile() {
        let message = missing_fixture_message(Path::new("/w/target/ci/examples/fake_process"));
        assert!(
            message.contains("`cargo build --examples --profile ci`"),
            "{message}"
        );
    }

    #[test]
    fn a_custom_profile_under_a_triple_names_both() {
        let message = missing_fixture_message(Path::new(
            "/w/target/aarch64-apple-darwin/ci/examples/fake_process",
        ));
        assert!(
            message.contains("`cargo build --examples --profile ci --target aarch64-apple-darwin`"),
            "{message}"
        );
    }

    #[test]
    fn a_debug_target_path_names_only_the_triple() {
        let message = missing_fixture_message(Path::new(
            "/w/target/x86_64-unknown-linux-gnu/debug/examples/fake_process",
        ));
        assert!(
            message.contains("`cargo build --examples --target x86_64-unknown-linux-gnu`"),
            "{message}"
        );
    }
}

#[cfg(test)]
mod fake_cli_default_tests {
    use super::with_fake_clis;
    use std::ffi::{OsStr, OsString};
    use std::path::Path;

    fn stdin_log() -> Option<OsString> {
        std::env::var_os("ESTIGIA_FAKE_STDIN_LOG")
    }

    #[test]
    fn the_fakes_drain_stdin_into_the_null_device_unless_the_caller_chose() {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        with_fake_clis(&[], &[], |_| {
            assert_eq!(stdin_log(), Some(OsString::from(null)));
        });
        with_fake_clis(
            &[],
            &[("ESTIGIA_FAKE_STDIN_LOG", OsStr::new("chosen.log"))],
            |_| assert_eq!(stdin_log(), Some(OsString::from("chosen.log"))),
        );
        with_fake_clis(
            &[],
            &[("ESTIGIA_FAKE_EXIT_WITHOUT_READING", OsStr::new("1"))],
            |_| assert_eq!(stdin_log(), None),
        );
    }

    #[test]
    fn a_scripted_gh_answers_while_another_test_holds_the_reviewer_launch_variables() {
        with_fake_clis(
            &[],
            &[("ESTIGIA_FAKE_EXIT_WITHOUT_READING", OsStr::new("1"))],
            |_| {
                let bin = super::scripted_gh();
                let answered = super::with_scripted_gh(bin.path(), "[]", || {
                    crate::transport::gh_json(
                        &["api", "repos/{owner}/{repo}/issues/1/comments"],
                        None,
                    )
                });
                assert_eq!(
                    answered.expect("the scripted gh ran"),
                    Some(serde_json::json!([]))
                );
            },
        );
    }

    #[test]
    fn a_scripted_gh_sees_none_of_the_seven_held_variables_and_every_one_its_fixture_sets() {
        let scratch = tempfile::tempdir().unwrap();
        let path = |name: &str| scratch.path().join(name).into_os_string();
        let names_file = path("names.txt");
        let argv_file = path("argv.txt");
        let held_names = path("held_names.txt");
        let held_argv = path("held_argv.txt");
        let held_stdin = path("held_stdin.txt");
        let flag = OsString::from("1");
        with_fake_clis(
            &[],
            &[
                ("ESTIGIA_FAKE_ARGV_LOG", held_argv.as_os_str()),
                ("ESTIGIA_FAKE_BIG_STDOUT", flag.as_os_str()),
                ("ESTIGIA_FAKE_DELAY_MS", flag.as_os_str()),
                ("ESTIGIA_FAKE_ENV_NAMES", held_names.as_os_str()),
                ("ESTIGIA_FAKE_EXIT_WITHOUT_READING", flag.as_os_str()),
                ("ESTIGIA_FAKE_STDIN_LOG", held_stdin.as_os_str()),
                ("ESTIGIA_FAKE_WRITE_ARG_FILE", flag.as_os_str()),
            ],
            |_| {
                let bin = super::scripted_gh();
                let ask = |own: &[(&str, OsString)]| {
                    super::with_scripted_gh_window(bin.path(), "[]", own, || {
                        let _ = crate::transport::gh_json(&["api", "repos/{owner}/{repo}"], None);
                    });
                };
                ask(&[("ESTIGIA_FAKE_ENV_NAMES", names_file.clone())]);
                ask(&[("ESTIGIA_FAKE_ARGV_LOG", argv_file.clone())]);
            },
        );
        let seen = std::fs::read_to_string(&names_file).expect("the fixture's own log was written");
        let seen: Vec<&str> = seen.lines().collect();
        assert!(seen.contains(&"ESTIGIA_FAKE_ENV_NAMES"), "{seen:?}");
        for removed in [
            "ESTIGIA_FAKE_ARGV_LOG",
            "ESTIGIA_FAKE_BIG_STDOUT",
            "ESTIGIA_FAKE_DELAY_MS",
            "ESTIGIA_FAKE_EXIT_WITHOUT_READING",
            "ESTIGIA_FAKE_STDIN_LOG",
            "ESTIGIA_FAKE_WRITE_ARG_FILE",
        ] {
            assert!(
                !seen.contains(&removed),
                "{removed} reached the child: {seen:?}"
            );
        }
        assert!(
            Path::new(&argv_file).is_file(),
            "the fixture's own variable was not passed"
        );
        assert!(
            !Path::new(&held_names).exists(),
            "a held ENV_NAMES reached the child"
        );
        assert!(!Path::new(&held_argv).exists());
        assert!(!Path::new(&held_stdin).exists());
    }

    #[test]
    fn the_default_is_put_back_when_the_body_returns() {
        with_fake_clis(&[], &[], |_| ());
        assert_eq!(super::with_environment_read_locked(stdin_log), None);
    }
}

#[cfg(all(test, unix))]
mod fake_cli_link_tests {
    use super::fake_cli_directory;

    #[test]
    fn every_fake_cli_is_a_link_to_the_built_fixture_and_not_a_copy_a_fork_can_hold_busy() {
        let directory = fake_cli_directory(&["claude"]);
        let mut seen = 0;
        for entry in std::fs::read_dir(directory.path()).unwrap() {
            let path = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            assert!(metadata.file_type().is_symlink(), "{}", path.display());
            let target = std::fs::read_link(&path).unwrap();
            assert_eq!(target.file_name().unwrap(), "fake_process");
            seen += 1;
        }
        assert_eq!(seen, 1);
    }
}
